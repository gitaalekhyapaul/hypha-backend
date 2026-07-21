package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	authzedv1 "github.com/authzed/authzed-go/proto/authzed/api/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/equivalencefixture"
	"google.golang.org/grpc"
)

type fakeSchemaWriter struct {
	written string
	err     error
}

func (fake *fakeSchemaWriter) WriteSchema(_ context.Context, request *authzedv1.WriteSchemaRequest, _ ...grpc.CallOption) (*authzedv1.WriteSchemaResponse, error) {
	fake.written = request.GetSchema()
	return &authzedv1.WriteSchemaResponse{}, fake.err
}

type fakePermissionsClient struct {
	deletedResources []string
	updates          []*authzedv1.RelationshipUpdate
	checks           []*authzedv1.CheckPermissionRequest
	responses        []authzedv1.CheckPermissionResponse_Permissionship
	writeToken       *authzedv1.ZedToken
	deleteErr        error
	writeErr         error
	checkErr         error
}

func (fake *fakePermissionsClient) DeleteRelationships(_ context.Context, request *authzedv1.DeleteRelationshipsRequest, _ ...grpc.CallOption) (*authzedv1.DeleteRelationshipsResponse, error) {
	filter := request.GetRelationshipFilter()
	fake.deletedResources = append(fake.deletedResources, filter.GetResourceType()+":"+filter.GetOptionalResourceId())
	return &authzedv1.DeleteRelationshipsResponse{DeletionProgress: authzedv1.DeleteRelationshipsResponse_DELETION_PROGRESS_COMPLETE}, fake.deleteErr
}

func (fake *fakePermissionsClient) WriteRelationships(_ context.Context, request *authzedv1.WriteRelationshipsRequest, _ ...grpc.CallOption) (*authzedv1.WriteRelationshipsResponse, error) {
	fake.updates = request.GetUpdates()
	return &authzedv1.WriteRelationshipsResponse{WrittenAt: fake.writeToken}, fake.writeErr
}

func (fake *fakePermissionsClient) CheckPermission(_ context.Context, request *authzedv1.CheckPermissionRequest, _ ...grpc.CallOption) (*authzedv1.CheckPermissionResponse, error) {
	fake.checks = append(fake.checks, request)
	if fake.checkErr != nil {
		return nil, fake.checkErr
	}
	index := len(fake.checks) - 1
	return &authzedv1.CheckPermissionResponse{Permissionship: fake.responses[index]}, nil
}

func TestExecuteFixtureUsesExactWriteSnapshot(t *testing.T) {
	schema := &fakeSchemaWriter{}
	permissions := &fakePermissionsClient{
		writeToken: &authzedv1.ZedToken{Token: "revision-1"},
		responses: []authzedv1.CheckPermissionResponse_Permissionship{
			authzedv1.CheckPermissionResponse_PERMISSIONSHIP_HAS_PERMISSION,
			authzedv1.CheckPermissionResponse_PERMISSIONSHIP_NO_PERMISSION,
			authzedv1.CheckPermissionResponse_PERMISSIONSHIP_CONDITIONAL_PERMISSION,
		},
	}
	fixture := &equivalencefixture.Fixture{
		Schema: []byte("definition user {}\n"),
		Relationships: []equivalencefixture.Relationship{
			{
				Resource: equivalencefixture.ObjectReference{ObjectType: "team", ObjectID: "recon"},
				Relation: "member",
				Subject:  equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
			},
			{
				Resource: equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
				Relation: "viewer",
				Subject:  equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
				Caveat: &equivalencefixture.RelationshipCaveat{
					Name:    "test_scope",
					Context: []equivalencefixture.ContextEntry{{Name: "stored", Value: testScalar(t, `{"int": 7}`)}},
				},
			},
		},
		Cases: []equivalencefixture.Case{
			{
				ID:               "case-a",
				Resource:         equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
				Permission:       "view",
				Subject:          equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
				RequestContext:   []equivalencefixture.ContextEntry{{Name: "flag", Value: testScalar(t, `{"bool": true}`)}},
				CapabilityClaims: []equivalencefixture.ContextEntry{{Name: "task", Value: testScalar(t, `{"string": "task-a"}`)}},
				Expected:         equivalencefixture.DecisionAllow,
			},
			{
				ID:         "case-b",
				Resource:   equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
				Permission: "view",
				Subject:    equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "bob"},
				Expected:   equivalencefixture.DecisionDeny,
			},
			{
				ID:         "case-c",
				Resource:   equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
				Permission: "view",
				Subject:    equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "carol"},
				Expected:   equivalencefixture.DecisionConditional,
			},
		},
	}

	results, err := executeFixture(context.Background(), services{schema: schema, permissions: permissions}, fixture)
	if err != nil {
		t.Fatalf("executeFixture() error = %v", err)
	}
	if schema.written != string(fixture.Schema) {
		t.Error("WriteSchema did not receive the exact schema bytes")
	}
	if got, want := permissions.deletedResources, []string{"document:item", "team:recon"}; !reflect.DeepEqual(got, want) {
		t.Errorf("deleted resources = %v, want %v", got, want)
	}
	if got, want := len(permissions.updates), len(fixture.Relationships); got != want {
		t.Fatalf("update count = %d, want %d", got, want)
	}
	for _, update := range permissions.updates {
		if update.GetOperation() != authzedv1.RelationshipUpdate_OPERATION_CREATE {
			t.Errorf("relationship operation = %s, want CREATE", update.GetOperation())
		}
	}
	if got := permissions.updates[1].GetRelationship().GetOptionalCaveat().GetContext().GetFields()["stored"].GetNumberValue(); got != 7 {
		t.Errorf("stored caveat value = %v, want 7", got)
	}
	if got, want := len(permissions.checks), 3; got != want {
		t.Fatalf("check count = %d, want %d", got, want)
	}
	for _, request := range permissions.checks {
		if got := request.GetConsistency().GetAtExactSnapshot().GetToken(); got != "revision-1" {
			t.Errorf("check revision = %q, want exact write revision", got)
		}
		if request.GetWithTracing() {
			t.Error("check unexpectedly requested a debug trace")
		}
	}
	fields := permissions.checks[0].GetContext().GetFields()
	if len(fields) != 2 || !fields["flag"].GetBoolValue() || fields["task"].GetStringValue() != "task-a" {
		t.Error("request and capability inputs were not merged for SpiceDB")
	}
	if got := permissions.checks[1].GetContext(); got != nil {
		t.Error("empty check context was not left nil")
	}
	if got, want := results, []caseResult{{id: "case-a", decision: equivalencefixture.DecisionAllow}, {id: "case-b", decision: equivalencefixture.DecisionDeny}, {id: "case-c", decision: equivalencefixture.DecisionConditional}}; !reflect.DeepEqual(got, want) {
		t.Errorf("results = %v, want %v", got, want)
	}
}

func TestExecuteFixtureRejectsUnexpectedConditional(t *testing.T) {
	permissions := &fakePermissionsClient{
		writeToken: &authzedv1.ZedToken{Token: "revision-1"},
		responses:  []authzedv1.CheckPermissionResponse_Permissionship{authzedv1.CheckPermissionResponse_PERMISSIONSHIP_CONDITIONAL_PERMISSION},
	}
	fixture := &equivalencefixture.Fixture{
		Schema: []byte("definition user {}\n"),
		Relationships: []equivalencefixture.Relationship{{
			Resource: equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
			Relation: "viewer",
			Subject:  equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
		}},
		Cases: []equivalencefixture.Case{{
			ID:         "case-a",
			Resource:   equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
			Permission: "view",
			Subject:    equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
			Expected:   equivalencefixture.DecisionDeny,
		}},
	}

	_, err := executeFixture(context.Background(), services{schema: &fakeSchemaWriter{}, permissions: permissions}, fixture)
	assertRunnerCode(t, err, codeConditional)
}

func TestExecuteFixtureRejectsDecisionMismatch(t *testing.T) {
	permissions := &fakePermissionsClient{
		writeToken: &authzedv1.ZedToken{Token: "revision-1"},
		responses:  []authzedv1.CheckPermissionResponse_Permissionship{authzedv1.CheckPermissionResponse_PERMISSIONSHIP_NO_PERMISSION},
	}
	fixture := &equivalencefixture.Fixture{
		Schema: []byte("definition user {}\n"),
		Relationships: []equivalencefixture.Relationship{{
			Resource: equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
			Relation: "viewer",
			Subject:  equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
		}},
		Cases: []equivalencefixture.Case{{
			ID:         "case-a",
			Resource:   equivalencefixture.ObjectReference{ObjectType: "document", ObjectID: "item"},
			Permission: "view",
			Subject:    equivalencefixture.ObjectReference{ObjectType: "user", ObjectID: "alice"},
			Expected:   equivalencefixture.DecisionAllow,
		}},
	}

	_, err := executeFixture(context.Background(), services{schema: &fakeSchemaWriter{}, permissions: permissions}, fixture)
	assertRunnerCode(t, err, codeMismatch)
}

func TestNormalizePermissionship(t *testing.T) {
	tests := []struct {
		name      string
		input     authzedv1.CheckPermissionResponse_Permissionship
		decision  equivalencefixture.Decision
		errorCode errorCode
	}{
		{name: "allow", input: authzedv1.CheckPermissionResponse_PERMISSIONSHIP_HAS_PERMISSION, decision: equivalencefixture.DecisionAllow},
		{name: "deny", input: authzedv1.CheckPermissionResponse_PERMISSIONSHIP_NO_PERMISSION, decision: equivalencefixture.DecisionDeny},
		{name: "conditional", input: authzedv1.CheckPermissionResponse_PERMISSIONSHIP_CONDITIONAL_PERMISSION, decision: equivalencefixture.DecisionConditional},
		{name: "unspecified", input: authzedv1.CheckPermissionResponse_PERMISSIONSHIP_UNSPECIFIED, errorCode: codeUnspecified},
		{name: "unknown", input: authzedv1.CheckPermissionResponse_Permissionship(99), errorCode: codeUnspecified},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			decision, err := normalizePermissionship(test.input)
			if test.errorCode != "" {
				assertRunnerCode(t, err, test.errorCode)
				return
			}
			if err != nil {
				t.Fatalf("normalizePermissionship() error = %v", err)
			}
			if decision != test.decision {
				t.Errorf("decision = %s, want %s", decision, test.decision)
			}
		})
	}
}

func TestMergedCheckContextRejectsDuplicateNames(t *testing.T) {
	entry := equivalencefixture.ContextEntry{Name: "task", Value: testScalar(t, `{"string": "private"}`)}
	_, err := mergedCheckContext([]equivalencefixture.ContextEntry{entry}, []equivalencefixture.ContextEntry{entry})
	assertRunnerCode(t, err, codeCheck)
	if strings.Contains(err.Error(), "private") {
		t.Error("duplicate-input error disclosed a context value")
	}
}

func TestWriteResultsEmitsStableLines(t *testing.T) {
	results := []caseResult{
		{id: "case-a", decision: equivalencefixture.DecisionAllow},
		{id: "case-b", decision: equivalencefixture.DecisionDeny},
		{id: "case-c", decision: equivalencefixture.DecisionConditional},
	}
	var stdout bytes.Buffer
	if err := writeResults("-", &stdout, results); err != nil {
		t.Fatalf("writeResults() error = %v", err)
	}
	if got, want := stdout.String(), "case-a\tALLOW\ncase-b\tDENY\ncase-c\tCONDITIONAL\n"; got != want {
		t.Errorf("stdout = %q, want %q", got, want)
	}

	path := filepath.Join(t.TempDir(), "results.txt")
	if err := writeResults(path, &bytes.Buffer{}, results); err != nil {
		t.Fatalf("writeResults(file) error = %v", err)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	if string(data) != stdout.String() {
		t.Error("file and stdout result formats differ")
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat() error = %v", err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Errorf("result mode = %o, want 600", got)
	}
}

func TestParseOptionsLocksTheDedicatedDatastore(t *testing.T) {
	parsed, help, err := parseOptions([]string{"--fixture", "fixture.json", "--output", "-"})
	if err != nil || help {
		t.Fatalf("parseOptions() = %#v, %t, %v", parsed, help, err)
	}
	if parsed.fixturePath != "fixture.json" || parsed.outputPath != "-" {
		t.Errorf("parseOptions() = %#v", parsed)
	}
	if _, _, err := parseOptions([]string{"--fixture", "fixture.json", "--endpoint", "spicedb:50051", "--output", "-"}); err == nil {
		t.Error("parseOptions() accepted a caller-selected datastore")
	}
}

func TestBearerCredentialIsLocalAndDoesNotRequireTLS(t *testing.T) {
	credential := bearerCredential{token: "test-key"}
	metadata, err := credential.GetRequestMetadata(context.Background())
	if err != nil {
		t.Fatalf("GetRequestMetadata() error = %v", err)
	}
	if metadata["authorization"] != "Bearer test-key" {
		t.Error("bearer credential did not set the authorization metadata")
	}
	if credential.RequireTransportSecurity() {
		t.Error("local test credential unexpectedly requires TLS")
	}
}

func testScalar(t *testing.T, source string) equivalencefixture.Scalar {
	t.Helper()
	var scalar equivalencefixture.Scalar
	if err := json.Unmarshal([]byte(source), &scalar); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}
	return scalar
}

func assertRunnerCode(t *testing.T, err error, want errorCode) {
	t.Helper()
	var typed *runnerError
	if !errors.As(err, &typed) {
		t.Fatalf("error = %v, want runnerError", err)
	}
	if typed.code != want {
		t.Fatalf("error code = %s, want %s", typed.code, want)
	}
}
