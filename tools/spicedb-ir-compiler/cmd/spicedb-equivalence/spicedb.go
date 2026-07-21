package main

import (
	"context"
	"sort"
	"time"

	authzedv1 "github.com/authzed/authzed-go/proto/authzed/api/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/equivalencefixture"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	healthv1 "google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/protobuf/types/known/structpb"
)

const (
	readinessTimeout        = 15 * time.Second
	readinessAttemptTimeout = 500 * time.Millisecond
	rpcTimeout              = 15 * time.Second
	retryInterval           = 100 * time.Millisecond
)

type bearerCredential struct {
	token string
}

func (credential bearerCredential) GetRequestMetadata(context.Context, ...string) (map[string]string, error) {
	return map[string]string{"authorization": "Bearer " + credential.token}, nil
}

func (bearerCredential) RequireTransportSecurity() bool {
	return false
}

type schemaWriter interface {
	WriteSchema(context.Context, *authzedv1.WriteSchemaRequest, ...grpc.CallOption) (*authzedv1.WriteSchemaResponse, error)
}

type permissionsClient interface {
	DeleteRelationships(context.Context, *authzedv1.DeleteRelationshipsRequest, ...grpc.CallOption) (*authzedv1.DeleteRelationshipsResponse, error)
	WriteRelationships(context.Context, *authzedv1.WriteRelationshipsRequest, ...grpc.CallOption) (*authzedv1.WriteRelationshipsResponse, error)
	CheckPermission(context.Context, *authzedv1.CheckPermissionRequest, ...grpc.CallOption) (*authzedv1.CheckPermissionResponse, error)
}

type healthClient interface {
	Check(context.Context, *healthv1.HealthCheckRequest, ...grpc.CallOption) (*healthv1.HealthCheckResponse, error)
}

type services struct {
	schema      schemaWriter
	permissions permissionsClient
	health      healthClient
}

type caseResult struct {
	id       string
	decision equivalencefixture.Decision
}

func runLive(ctx context.Context, endpoint, presharedKey string, fixture *equivalencefixture.Fixture) ([]caseResult, error) {
	connection, err := grpc.NewClient(
		endpoint,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithPerRPCCredentials(bearerCredential{token: presharedKey}),
	)
	if err != nil {
		return nil, newRunnerError(codeConnect, "cannot create the SpiceDB client")
	}
	defer connection.Close()

	clients := services{
		schema:      authzedv1.NewSchemaServiceClient(connection),
		permissions: authzedv1.NewPermissionsServiceClient(connection),
		health:      healthv1.NewHealthClient(connection),
	}
	if err := waitForReady(ctx, clients.health); err != nil {
		return nil, err
	}
	return executeFixture(ctx, clients, fixture)
}

func waitForReady(parent context.Context, client healthClient) error {
	ctx, cancel := context.WithTimeout(parent, readinessTimeout)
	defer cancel()

	for {
		attemptContext, attemptCancel := context.WithTimeout(ctx, readinessAttemptTimeout)
		response, err := client.Check(attemptContext, &healthv1.HealthCheckRequest{})
		attemptCancel()
		if err == nil && response.GetStatus() == healthv1.HealthCheckResponse_SERVING {
			return nil
		}
		timer := time.NewTimer(retryInterval)
		select {
		case <-ctx.Done():
			timer.Stop()
			return newRunnerError(codeReadiness, "SpiceDB did not become ready before the bounded timeout")
		case <-timer.C:
		}
	}
}

func executeFixture(parent context.Context, clients services, fixture *equivalencefixture.Fixture) ([]caseResult, error) {
	ctx, cancel := context.WithTimeout(parent, rpcTimeout)
	_, err := clients.schema.WriteSchema(ctx, &authzedv1.WriteSchemaRequest{Schema: string(fixture.Schema)})
	cancel()
	if err != nil {
		return nil, newRunnerError(codeSchemaWrite, "SpiceDB rejected the exact fixture schema bytes")
	}

	if err := resetRelationships(parent, clients.permissions, fixture.Relationships); err != nil {
		return nil, err
	}
	updates, err := relationshipUpdates(fixture.Relationships)
	if err != nil {
		return nil, err
	}
	ctx, cancel = context.WithTimeout(parent, rpcTimeout)
	writeResponse, err := clients.permissions.WriteRelationships(ctx, &authzedv1.WriteRelationshipsRequest{Updates: updates})
	cancel()
	if err != nil || writeResponse.GetWrittenAt() == nil || writeResponse.GetWrittenAt().GetToken() == "" {
		return nil, newRunnerError(codeRelationshipWrite, "SpiceDB did not commit the fixture relationships")
	}

	results := make([]caseResult, 0, len(fixture.Cases))
	for _, check := range fixture.Cases {
		contextStruct, err := mergedCheckContext(check.RequestContext, check.CapabilityClaims)
		if err != nil {
			return nil, err
		}
		request := &authzedv1.CheckPermissionRequest{
			Consistency: &authzedv1.Consistency{Requirement: &authzedv1.Consistency_AtExactSnapshot{AtExactSnapshot: writeResponse.GetWrittenAt()}},
			Resource:    apiObject(check.Resource),
			Permission:  check.Permission,
			Subject:     &authzedv1.SubjectReference{Object: apiObject(check.Subject)},
			Context:     contextStruct,
		}
		ctx, cancel = context.WithTimeout(parent, rpcTimeout)
		response, rpcErr := clients.permissions.CheckPermission(ctx, request)
		cancel()
		if rpcErr != nil {
			return nil, newRunnerError(codeCheck, "SpiceDB check failed for case "+check.ID)
		}
		decision, err := normalizePermissionship(response.GetPermissionship())
		if err != nil {
			return nil, withCaseID(err, check.ID)
		}
		if decision == equivalencefixture.DecisionConditional && check.Expected != equivalencefixture.DecisionConditional {
			return nil, newRunnerError(codeConditional, "SpiceDB returned an unexpected conditional decision for case "+check.ID)
		}
		if decision != check.Expected {
			return nil, newRunnerError(codeMismatch, "decision mismatch for case "+check.ID)
		}
		results = append(results, caseResult{id: check.ID, decision: decision})
	}
	return results, nil
}

func resetRelationships(parent context.Context, client permissionsClient, relationships []equivalencefixture.Relationship) error {
	resources := map[string]equivalencefixture.ObjectReference{}
	for _, relationship := range relationships {
		key := relationship.Resource.ObjectType + "\x00" + relationship.Resource.ObjectID
		resources[key] = relationship.Resource
	}
	ordered := make([]string, 0, len(resources))
	for key := range resources {
		ordered = append(ordered, key)
	}
	sort.Strings(ordered)
	for _, key := range ordered {
		resource := resources[key]
		ctx, cancel := context.WithTimeout(parent, rpcTimeout)
		response, err := client.DeleteRelationships(ctx, &authzedv1.DeleteRelationshipsRequest{
			RelationshipFilter: &authzedv1.RelationshipFilter{
				ResourceType:       resource.ObjectType,
				OptionalResourceId: resource.ObjectID,
			},
		})
		cancel()
		if err != nil || response.GetDeletionProgress() != authzedv1.DeleteRelationshipsResponse_DELETION_PROGRESS_COMPLETE {
			return newRunnerError(codeRelationshipReset, "SpiceDB did not fully reset fixture relationship state")
		}
	}
	return nil
}

func relationshipUpdates(relationships []equivalencefixture.Relationship) ([]*authzedv1.RelationshipUpdate, error) {
	updates := make([]*authzedv1.RelationshipUpdate, 0, len(relationships))
	for _, relationship := range relationships {
		converted := &authzedv1.Relationship{
			Resource: apiObject(relationship.Resource),
			Relation: relationship.Relation,
			Subject: &authzedv1.SubjectReference{
				Object:           apiObject(relationship.Subject),
				OptionalRelation: relationship.SubjectRelation,
			},
		}
		if relationship.Caveat != nil {
			contextStruct, err := contextToStruct(relationship.Caveat.Context)
			if err != nil {
				return nil, err
			}
			converted.OptionalCaveat = &authzedv1.ContextualizedCaveat{
				CaveatName: relationship.Caveat.Name,
				Context:    contextStruct,
			}
		}
		updates = append(updates, &authzedv1.RelationshipUpdate{
			Operation:    authzedv1.RelationshipUpdate_OPERATION_CREATE,
			Relationship: converted,
		})
	}
	return updates, nil
}

func apiObject(reference equivalencefixture.ObjectReference) *authzedv1.ObjectReference {
	return &authzedv1.ObjectReference{ObjectType: reference.ObjectType, ObjectId: reference.ObjectID}
}

func mergedCheckContext(request, capability []equivalencefixture.ContextEntry) (*structpb.Struct, error) {
	values := make(map[string]any, len(request)+len(capability))
	for _, entries := range [][]equivalencefixture.ContextEntry{request, capability} {
		for _, entry := range entries {
			if _, exists := values[entry.Name]; exists {
				return nil, newRunnerError(codeCheck, "request and capability inputs contain a duplicate parameter name")
			}
			values[entry.Name] = entry.Value.Value()
		}
	}
	if len(values) == 0 {
		return nil, nil
	}
	converted, err := structpb.NewStruct(values)
	if err != nil {
		return nil, newRunnerError(codeCheck, "check context cannot be encoded")
	}
	return converted, nil
}

func contextToStruct(entries []equivalencefixture.ContextEntry) (*structpb.Struct, error) {
	values := make(map[string]any, len(entries))
	for _, entry := range entries {
		if _, exists := values[entry.Name]; exists {
			return nil, newRunnerError(codeRelationshipWrite, "relationship context contains a duplicate parameter name")
		}
		values[entry.Name] = entry.Value.Value()
	}
	converted, err := structpb.NewStruct(values)
	if err != nil {
		return nil, newRunnerError(codeRelationshipWrite, "relationship context cannot be encoded")
	}
	return converted, nil
}

func normalizePermissionship(permissionship authzedv1.CheckPermissionResponse_Permissionship) (equivalencefixture.Decision, error) {
	switch permissionship {
	case authzedv1.CheckPermissionResponse_PERMISSIONSHIP_HAS_PERMISSION:
		return equivalencefixture.DecisionAllow, nil
	case authzedv1.CheckPermissionResponse_PERMISSIONSHIP_NO_PERMISSION:
		return equivalencefixture.DecisionDeny, nil
	case authzedv1.CheckPermissionResponse_PERMISSIONSHIP_CONDITIONAL_PERMISSION:
		return equivalencefixture.DecisionConditional, nil
	case authzedv1.CheckPermissionResponse_PERMISSIONSHIP_UNSPECIFIED:
		return "", newRunnerError(codeUnspecified, "SpiceDB returned an unspecified decision")
	default:
		return "", newRunnerError(codeUnspecified, "SpiceDB returned an unknown decision")
	}
}

func withCaseID(err error, caseID string) error {
	if typed, ok := err.(*runnerError); ok {
		return newRunnerError(typed.code, typed.message+" for case "+caseID)
	}
	return newRunnerError(codeCheck, "decision normalization failed for case "+caseID)
}
