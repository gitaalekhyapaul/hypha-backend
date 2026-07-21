package equivalencefixture

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"google.golang.org/protobuf/proto"
)

const (
	validRelationship = `{
      "resource": {"object_type": "document", "object_id": "item"},
      "relation": "viewer",
      "subject": {"object_type": "user", "object_id": "alice"},
      "caveat": {
        "name": "test_scope",
        "context": [{"name": "stored", "value": {"int": 7}}]
      }
    }`
	validCaseA = `{
      "id": "case-a",
      "resource": {"object_type": "document", "object_id": "item"},
      "permission": "view",
      "subject": {"object_type": "user", "object_id": "alice"},
      "request_context": [{"name": "flag", "value": {"bool": true}}],
      "capability_claims": [{"name": "task", "value": {"string": "task-a"}}],
      "expected": "DENY"
    }`
	validCaseB = `{
      "id": "case-b",
      "resource": {"object_type": "document", "object_id": "item"},
      "permission": "view",
      "subject": {"object_type": "user", "object_id": "bob"},
      "request_context": [],
      "capability_claims": [],
      "expected": "DENY"
    }`
)

type testRepository struct {
	root        string
	fixturePath string
	schema      []byte
	digest      string
}

func TestLoadAcceptsStrictVersionOneFixture(t *testing.T) {
	repository := newTestRepository(t)

	root, err := FindRepositoryRoot(repository.fixturePath)
	if err != nil {
		t.Fatalf("FindRepositoryRoot() error = %v", err)
	}
	fixture, err := Load(root, repository.fixturePath)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}

	if fixture.Version != SupportedVersion {
		t.Errorf("Version = %d, want %d", fixture.Version, SupportedVersion)
	}
	if got := string(fixture.Schema); got != string(repository.schema) {
		t.Errorf("Schema bytes changed during loading")
	}
	if got, want := len(fixture.Relationships), 1; got != want {
		t.Fatalf("relationship count = %d, want %d", got, want)
	}
	stored := fixture.Relationships[0].Caveat.Context[0].Value
	if stored.Kind() != ScalarInt || stored.Value() != int64(7) {
		t.Errorf("stored scalar was not decoded as the exact int64 value")
	}
	flag := fixture.Cases[0].RequestContext[0].Value
	if flag.Kind() != ScalarBool || flag.Value() != true {
		t.Errorf("request scalar was not decoded as a Boolean")
	}
	task := fixture.Cases[0].CapabilityClaims[0].Value
	if task.Kind() != ScalarString || task.Value() != "task-a" {
		t.Errorf("capability scalar was not decoded as a string")
	}
}

func TestRepositoryFixtureLoads(t *testing.T) {
	repoRoot, err := filepath.Abs(filepath.Join("..", "..", "..", ".."))
	if err != nil {
		t.Fatalf("filepath.Abs() error = %v", err)
	}
	fixturePath := filepath.Join(repoRoot, "tests", "equivalence", "fixtures", "v1.json")
	fixture, err := Load(repoRoot, fixturePath)
	if err != nil {
		t.Fatalf("Load(repository fixture) error = %v", err)
	}
	if len(fixture.Cases) == 0 {
		t.Error("repository fixture contains no cases")
	}
	if len(fixture.Relationships) == 0 {
		t.Error("repository fixture contains no relationships")
	}
}

func TestLoadRejectsMalformedAndDriftingFixtures(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(string) string
		code   ErrorCode
	}{
		{
			name: "duplicate JSON key",
			mutate: func(input string) string {
				return strings.Replace(input, `"fixture_version": 1,`, `"fixture_version": 1, "fixture_version": 1,`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "unknown field",
			mutate: func(input string) string {
				return strings.Replace(input, `"fixture_version": 1,`, `"fixture_version": 1, "unknown": true,`, 1)
			},
			code: CodeJSON,
		},
		{
			name:   "trailing JSON",
			mutate: func(input string) string { return input + ` {}` },
			code:   CodeJSON,
		},
		{
			name: "unsupported version",
			mutate: func(input string) string {
				return strings.Replace(input, `"fixture_version": 1`, `"fixture_version": 2`, 1)
			},
			code: CodeVersion,
		},
		{
			name: "float scalar",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"int": 7.5}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "null scalar",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"int": null}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "integer outside exact range",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"int": 9007199254740992}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "multiple scalar tags",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"int": 7, "bool": true}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "unknown scalar tag",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"number": 7}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "duplicate scalar tag",
			mutate: func(input string) string {
				return strings.Replace(input, `{"int": 7}`, `{"int": 7, "int": 8}`, 1)
			},
			code: CodeJSON,
		},
		{
			name: "schema digest mismatch",
			mutate: func(input string) string {
				return strings.Replace(input, `"schema_sha256": "`, `"schema_sha256": "00`, 1)
			},
			code: CodeSchemaDigest,
		},
		{
			name: "wrong provenance source",
			mutate: func(input string) string {
				return strings.Replace(input, `{"name": "task", "value": {"string": "task-a"}}`, `{"name": "flag", "value": {"bool": true}}`, 1)
			},
			code: CodeProvenance,
		},
		{
			name: "invalid expected decision",
			mutate: func(input string) string {
				return strings.Replace(input, `"expected": "DENY"`, `"expected": "UNKNOWN"`, 1)
			},
			code: CodeValue,
		},
		{
			name: "subject relation combined with caveat",
			mutate: func(input string) string {
				return strings.Replace(input, `"subject": {"object_type": "user", "object_id": "alice"},`, `"subject": {"object_type": "user", "object_id": "alice"}, "subject_relation": "member",`, 1)
			},
			code: CodeValue,
		},
		{
			name: "unsafe case ID",
			mutate: func(input string) string {
				return strings.Replace(input, `"id": "case-a"`, `"id": "case-a\nforged"`, 1)
			},
			code: CodeValue,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			repository := newTestRepository(t)
			original := readTestFile(t, repository.fixturePath)
			writeTestFile(t, repository.fixturePath, []byte(test.mutate(string(original))))

			_, err := Load(repository.root, repository.fixturePath)
			if err == nil {
				t.Fatal("Load() error = nil")
			}
			if !IsCode(err, test.code) {
				t.Fatalf("Load() error = %v, want code %s", err, test.code)
			}
			if strings.Contains(err.Error(), repository.root) {
				t.Fatalf("Load() error disclosed a filesystem path: %v", err)
			}
		})
	}
}

func TestLoadRejectsDuplicateAndUnsortedCollections(t *testing.T) {
	tests := []struct {
		name          string
		relationships string
		cases         string
		code          ErrorCode
	}{
		{name: "duplicate relationship", relationships: validRelationship + "," + validRelationship, cases: validCaseA, code: CodeDuplicate},
		{name: "duplicate case ID", relationships: validRelationship, cases: validCaseA + "," + validCaseA, code: CodeDuplicate},
		{name: "unsorted case IDs", relationships: validRelationship, cases: validCaseB + "," + validCaseA, code: CodeOrder},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			repository := newTestRepositoryWithCollections(t, test.relationships, test.cases)
			_, err := Load(repository.root, repository.fixturePath)
			if !IsCode(err, test.code) {
				t.Fatalf("Load() error = %v, want code %s", err, test.code)
			}
		})
	}
}

func TestLoadRejectsUnsortedAndDuplicateContextEntries(t *testing.T) {
	tests := []struct {
		name        string
		replacement string
		code        ErrorCode
	}{
		{
			name:        "duplicate context name",
			replacement: `[{"name": "flag", "value": {"bool": true}}, {"name": "flag", "value": {"bool": false}}]`,
			code:        CodeDuplicate,
		},
		{
			name:        "unsorted context names",
			replacement: `[{"name": "flag", "value": {"bool": true}}, {"name": "alpha", "value": {"bool": false}}]`,
			code:        CodeOrder,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			repository := newTestRepository(t)
			fixture := string(readTestFile(t, repository.fixturePath))
			fixture = strings.Replace(fixture, `[{"name": "flag", "value": {"bool": true}}]`, test.replacement, 1)
			writeTestFile(t, repository.fixturePath, []byte(fixture))

			_, err := Load(repository.root, repository.fixturePath)
			if !IsCode(err, test.code) {
				t.Fatalf("Load() error = %v, want code %s", err, test.code)
			}
		})
	}
}

func TestLoadRejectsMissingStoredContextAndOversizedFixture(t *testing.T) {
	repository := newTestRepository(t)
	fixture := string(readTestFile(t, repository.fixturePath))
	fixture = strings.Replace(fixture, `[{"name": "stored", "value": {"int": 7}}]`, `[]`, 1)
	writeTestFile(t, repository.fixturePath, []byte(fixture))

	_, err := Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodeProvenance) {
		t.Fatalf("Load(missing stored context) error = %v, want code %s", err, CodeProvenance)
	}

	writeTestFile(t, repository.fixturePath, bytes.Repeat([]byte{' '}, int(maxFixtureBytes+1)))
	_, err = Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodeLimit) {
		t.Fatalf("Load(oversized fixture) error = %v, want code %s", err, CodeLimit)
	}
}

func TestLoadAcceptsExactJSONIntegerBoundary(t *testing.T) {
	repository := newTestRepository(t)
	fixture := string(readTestFile(t, repository.fixturePath))
	fixture = strings.Replace(fixture, `{"int": 7}`, `{"int": 9007199254740991}`, 1)
	writeTestFile(t, repository.fixturePath, []byte(fixture))

	loaded, err := Load(repository.root, repository.fixturePath)
	if err != nil {
		t.Fatalf("Load() error = %v", err)
	}
	if got := loaded.Relationships[0].Caveat.Context[0].Value.Value(); got != int64(9007199254740991) {
		t.Errorf("integer = %v, want exact boundary", got)
	}
}

func TestLoadRejectsReferencedPathOutsideRepository(t *testing.T) {
	repository := newTestRepository(t)
	outside := filepath.Join(t.TempDir(), "outside.zed")
	writeTestFile(t, outside, repository.schema)
	fixture := string(readTestFile(t, repository.fixturePath))
	fixture = strings.Replace(fixture, `"schema.zed"`, strconv.Quote(outside), 1)
	writeTestFile(t, repository.fixturePath, []byte(fixture))

	_, err := Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodePath) {
		t.Fatalf("Load() error = %v, want code %s", err, CodePath)
	}
	if strings.Contains(err.Error(), outside) || strings.Contains(err.Error(), repository.root) {
		t.Fatalf("Load() error disclosed a filesystem path: %v", err)
	}
}

func TestLoadRejectsSymlinkEscape(t *testing.T) {
	repository := newTestRepository(t)
	outside := filepath.Join(t.TempDir(), "outside.zed")
	writeTestFile(t, outside, repository.schema)
	link := filepath.Join(repository.root, "linked.zed")
	if err := os.Symlink(outside, link); err != nil {
		t.Fatalf("Symlink() error = %v", err)
	}
	fixture := string(readTestFile(t, repository.fixturePath))
	fixture = strings.Replace(fixture, `"schema.zed"`, `"linked.zed"`, 1)
	writeTestFile(t, repository.fixturePath, []byte(fixture))

	_, err := Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodePath) {
		t.Fatalf("Load() error = %v, want code %s", err, CodePath)
	}
}

func TestLoadRejectsPolicyAndSchemaDrift(t *testing.T) {
	repository := newTestRepository(t)
	changedSchema := append(append([]byte(nil), repository.schema...), '\n')
	writeTestFile(t, filepath.Join(repository.root, "schema.zed"), changedSchema)
	digest := sha256.Sum256(changedSchema)
	fixture := string(readTestFile(t, repository.fixturePath))
	fixture = strings.Replace(fixture, repository.digest, hex.EncodeToString(digest[:]), 1)
	writeTestFile(t, repository.fixturePath, []byte(fixture))

	_, err := Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodePolicyIR) {
		t.Fatalf("Load() error = %v, want code %s", err, CodePolicyIR)
	}
}

func TestLoadRejectsCompilerLimitDrift(t *testing.T) {
	repository := newTestRepository(t)
	configPath := filepath.Join(repository.root, "config.json")
	configJSON := string(readTestFile(t, configPath))
	configJSON = strings.Replace(configJSON, `"max_path_depth": 2`, `"max_path_depth": 3`, 1)
	writeTestFile(t, configPath, []byte(configJSON))

	_, err := Load(repository.root, repository.fixturePath)
	if !IsCode(err, CodeProvenance) {
		t.Fatalf("Load() error = %v, want code %s", err, CodeProvenance)
	}
}

func newTestRepository(t *testing.T) testRepository {
	t.Helper()
	return newTestRepositoryWithCollections(t, validRelationship, validCaseA)
}

func newTestRepositoryWithCollections(t *testing.T, relationships, cases string) testRepository {
	t.Helper()
	root := t.TempDir()
	writeTestFile(t, filepath.Join(root, "AGENTS.md"), []byte("test repository\n"))
	writeTestFile(t, filepath.Join(root, "go.work"), []byte("go 1.26.5\n"))
	schema := []byte("definition user {}\ndefinition document { relation viewer: user }\n")
	writeTestFile(t, filepath.Join(root, "schema.zed"), schema)
	digest := sha256.Sum256(schema)
	digestHex := hex.EncodeToString(digest[:])

	configJSON := `{
  "config_version": 1,
  "limits": {
    "max_expression_nodes": 16,
    "max_caveat_nodes": 16,
    "max_path_depth": 2,
    "max_relationship_witnesses": 8,
    "max_merkle_depth": 8,
    "max_string_bytes": 64,
    "max_context_entries": 8,
    "max_total_private_input_bytes": 1024
  },
  "caveats": {
    "test_scope": {
      "alpha": "REQUEST_CONTEXT",
      "flag": "REQUEST_CONTEXT",
      "stored": "RELATIONSHIP_CONTEXT",
      "task": "CAPABILITY_CLAIM"
    }
  }
}`
	writeTestFile(t, filepath.Join(root, "config.json"), []byte(configJSON))

	program := &razielv1.PolicyProgram{
		IrVersion:          1,
		SourceSchemaDigest: digest[:],
		Limits: &razielv1.PolicyLimits{
			MaxExpressionNodes:        16,
			MaxCaveatNodes:            16,
			MaxPathDepth:              2,
			MaxRelationshipWitnesses:  8,
			MaxMerkleDepth:            8,
			MaxStringBytes:            64,
			MaxContextEntries:         8,
			MaxTotalPrivateInputBytes: 1024,
		},
		Caveats: []*razielv1.CaveatDefinition{{
			Name: "test_scope",
			Parameters: []*razielv1.CaveatParameter{
				{Name: "alpha", Source: razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT, ValueType: razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL},
				{Name: "flag", Source: razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT, ValueType: razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL},
				{Name: "stored", Source: razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT, ValueType: razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_INT},
				{Name: "task", Source: razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM, ValueType: razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_STRING},
			},
		}},
	}
	policyBytes, err := proto.Marshal(program)
	if err != nil {
		t.Fatalf("proto.Marshal() error = %v", err)
	}
	writeTestFile(t, filepath.Join(root, "policy.pb"), policyBytes)

	fixturePath := filepath.Join(root, "fixture.json")
	fixtureJSON := fmt.Sprintf(`{
  "fixture_version": 1,
  "schema_path": "schema.zed",
  "compiler_config_path": "config.json",
  "policy_ir_path": "policy.pb",
  "schema_sha256": "%s",
  "relationships": [%s],
  "cases": [%s]
}`, digestHex, relationships, cases)
	writeTestFile(t, fixturePath, []byte(fixtureJSON))
	return testRepository{root: root, fixturePath: fixturePath, schema: schema, digest: digestHex}
}

func writeTestFile(t *testing.T, path string, data []byte) {
	t.Helper()
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
}

func readTestFile(t *testing.T, path string) []byte {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	return data
}
