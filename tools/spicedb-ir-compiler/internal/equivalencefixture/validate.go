package equivalencefixture

import (
	"fmt"
	"strings"
	"unicode/utf8"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
	"google.golang.org/protobuf/proto"
)

type parameterMetadata struct {
	source    razielv1.CaveatInputSource
	valueType razielv1.CaveatValueType
}

type fixtureMetadata struct {
	caveats map[string]map[string]parameterMetadata
	global  map[string]parameterMetadata
}

func buildMetadata(program *razielv1.PolicyProgram, compilerConfig config.Config) (fixtureMetadata, error) {
	if !proto.Equal(program.GetLimits(), compilerConfig.Limits()) {
		return fixtureMetadata{}, fixtureError(CodeProvenance, "compiler config and PolicyIR limits differ")
	}
	metadata := fixtureMetadata{
		caveats: make(map[string]map[string]parameterMetadata, len(program.GetCaveats())),
		global:  map[string]parameterMetadata{},
	}
	for _, caveat := range program.GetCaveats() {
		if caveat.GetName() == "" {
			return fixtureMetadata{}, fixtureError(CodePolicyIR, "PolicyIR contains an unnamed caveat")
		}
		if _, exists := metadata.caveats[caveat.GetName()]; exists {
			return fixtureMetadata{}, fixtureError(CodePolicyIR, "PolicyIR contains duplicate caveat metadata")
		}
		parameters := make(map[string]parameterMetadata, len(caveat.GetParameters()))
		for _, parameter := range caveat.GetParameters() {
			if parameter.GetName() == "" || parameter.GetSource() == razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_UNSPECIFIED || parameter.GetValueType() == razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_UNSPECIFIED {
				return fixtureMetadata{}, fixtureError(CodePolicyIR, "PolicyIR contains incomplete caveat parameter metadata")
			}
			current := parameterMetadata{source: parameter.GetSource(), valueType: parameter.GetValueType()}
			if _, exists := parameters[parameter.GetName()]; exists {
				return fixtureMetadata{}, fixtureError(CodePolicyIR, "PolicyIR contains duplicate caveat parameter metadata")
			}
			parameters[parameter.GetName()] = current
			if existing, exists := metadata.global[parameter.GetName()]; exists && existing != current {
				return fixtureMetadata{}, fixtureError(CodeProvenance, "a caveat parameter name has ambiguous global provenance or type")
			}
			metadata.global[parameter.GetName()] = current
		}
		metadata.caveats[caveat.GetName()] = parameters
	}

	bindings := compilerConfig.CaveatBindings()
	if len(bindings) != len(metadata.caveats) {
		return fixtureMetadata{}, fixtureError(CodeProvenance, "compiler config and PolicyIR caveat sets differ")
	}
	for caveatName, parameters := range metadata.caveats {
		configured, exists := bindings[caveatName]
		if !exists || len(configured) != len(parameters) {
			return fixtureMetadata{}, fixtureError(CodeProvenance, "compiler config and PolicyIR caveat parameters differ")
		}
		for parameterName, parameter := range parameters {
			if configured[parameterName] != parameter.source {
				return fixtureMetadata{}, fixtureError(CodeProvenance, "compiler config and PolicyIR parameter provenance differ")
			}
		}
	}
	return metadata, nil
}

func validateFixture(fixture *Fixture, metadata fixtureMetadata, limits *razielv1.PolicyLimits) error {
	if len(fixture.Relationships) == 0 {
		return fixtureError(CodeValue, "fixture must contain relationships")
	}
	if len(fixture.Cases) == 0 {
		return fixtureError(CodeValue, "fixture must contain cases")
	}

	seenRelationships := map[string]struct{}{}
	for index := range fixture.Relationships {
		relationship := &fixture.Relationships[index]
		if err := validateObjectReference(relationship.Resource, "relationship resource", limits); err != nil {
			return err
		}
		if err := validateObjectReference(relationship.Subject, "relationship subject", limits); err != nil {
			return err
		}
		if !validIdentifier(relationship.Relation) {
			return fixtureError(CodeValue, "relationship relation syntax is invalid")
		}
		if relationship.SubjectRelation != "" && !validIdentifier(relationship.SubjectRelation) {
			return fixtureError(CodeValue, "relationship subject relation syntax is invalid")
		}
		if relationship.SubjectRelation != "" && relationship.Caveat != nil {
			return fixtureError(CodeValue, "a relationship cannot combine a subject relation and caveat")
		}
		key := relationshipKey(*relationship)
		if _, exists := seenRelationships[key]; exists {
			return fixtureError(CodeDuplicate, "fixture contains a duplicate relationship key")
		}
		seenRelationships[key] = struct{}{}
		if relationship.Caveat != nil {
			if err := validateRelationshipCaveat(*relationship.Caveat, metadata, limits); err != nil {
				return err
			}
		}
	}

	previousCaseID := ""
	for index := range fixture.Cases {
		check := &fixture.Cases[index]
		if !validCaseID(check.ID) {
			return fixtureError(CodeValue, "case ID must use lowercase letters, digits, and hyphens")
		}
		if previousCaseID != "" && check.ID <= previousCaseID {
			if check.ID == previousCaseID {
				return fixtureError(CodeDuplicate, "fixture contains a duplicate case ID")
			}
			return fixtureError(CodeOrder, "case IDs must be sorted")
		}
		previousCaseID = check.ID
		if err := validateObjectReference(check.Resource, "case resource", limits); err != nil {
			return err
		}
		if err := validateObjectReference(check.Subject, "case subject", limits); err != nil {
			return err
		}
		if !validIdentifier(check.Permission) {
			return fixtureError(CodeValue, "case permission syntax is invalid")
		}
		if check.Expected != DecisionAllow && check.Expected != DecisionDeny && check.Expected != DecisionConditional {
			return fixtureError(CodeValue, "case expected decision must be ALLOW, DENY, or CONDITIONAL")
		}
		if err := validateContext(check.RequestContext, razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT, metadata.global, limits); err != nil {
			return err
		}
		if err := validateContext(check.CapabilityClaims, razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM, metadata.global, limits); err != nil {
			return err
		}
		if hasCrossContextDuplicate(check.RequestContext, check.CapabilityClaims) {
			return fixtureError(CodeDuplicate, "request context and capability claims contain the same parameter name")
		}
	}
	return nil
}

func validateRelationshipCaveat(caveat RelationshipCaveat, metadata fixtureMetadata, limits *razielv1.PolicyLimits) error {
	if !validIdentifier(caveat.Name) {
		return fixtureError(CodeValue, "relationship caveat name syntax is invalid")
	}
	parameters, exists := metadata.caveats[caveat.Name]
	if !exists {
		return fixtureError(CodeProvenance, "relationship references unknown caveat metadata")
	}
	if err := validateContext(caveat.Context, razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT, parameters, limits); err != nil {
		return err
	}
	for name, parameter := range parameters {
		if parameter.source != razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT {
			continue
		}
		found := false
		for _, entry := range caveat.Context {
			if entry.Name == name {
				found = true
				break
			}
		}
		if !found {
			return fixtureError(CodeProvenance, "relationship caveat is missing a required stored parameter")
		}
	}
	return nil
}

func validateContext(entries []ContextEntry, source razielv1.CaveatInputSource, parameters map[string]parameterMetadata, limits *razielv1.PolicyLimits) error {
	if uint32(len(entries)) > limits.GetMaxContextEntries() {
		return fixtureError(CodeValue, "context entry count exceeds the PolicyIR limit")
	}
	previousName := ""
	for _, entry := range entries {
		if entry.Name == "" {
			return fixtureError(CodeValue, "context parameter name must not be empty")
		}
		if previousName != "" && entry.Name <= previousName {
			if entry.Name == previousName {
				return fixtureError(CodeDuplicate, "context contains a duplicate parameter name")
			}
			return fixtureError(CodeOrder, "context parameter names must be sorted")
		}
		previousName = entry.Name
		parameter, exists := parameters[entry.Name]
		if !exists || parameter.source != source {
			return fixtureError(CodeProvenance, "context parameter is assigned to the wrong provenance source")
		}
		if !scalarMatchesType(entry.Value, parameter.valueType) {
			return fixtureError(CodeValue, "context scalar type does not match PolicyIR metadata")
		}
		if entry.Value.kind == ScalarString {
			if !utf8.ValidString(entry.Value.text) || uint32(len(entry.Value.text)) > limits.GetMaxStringBytes() {
				return fixtureError(CodeValue, "string scalar exceeds the PolicyIR limit")
			}
		}
	}
	return nil
}

func validateObjectReference(reference ObjectReference, label string, limits *razielv1.PolicyLimits) error {
	if !validIdentifier(reference.ObjectType) || reference.ObjectID == "" || !utf8.ValidString(reference.ObjectID) || strings.ContainsRune(reference.ObjectID, '\x00') || uint32(len(reference.ObjectID)) > limits.GetMaxStringBytes() {
		return fixtureError(CodeValue, label+" must contain object_type and object_id")
	}
	return nil
}

func validIdentifier(value string) bool {
	if len(value) == 0 || len(value) > 256 || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for _, character := range []byte(value[1:]) {
		if character >= 'a' && character <= 'z' || character >= '0' && character <= '9' || character == '_' {
			continue
		}
		return false
	}
	return true
}

func validCaseID(value string) bool {
	if len(value) == 0 || len(value) > 128 {
		return false
	}
	for index, character := range []byte(value) {
		if character >= 'a' && character <= 'z' || character >= '0' && character <= '9' || index > 0 && character == '-' {
			continue
		}
		return false
	}
	return true
}

func scalarMatchesType(value Scalar, valueType razielv1.CaveatValueType) bool {
	switch valueType {
	case razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL:
		return value.kind == ScalarBool
	case razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_INT:
		return value.kind == ScalarInt
	case razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_STRING:
		return value.kind == ScalarString
	default:
		return false
	}
}

func hasCrossContextDuplicate(left, right []ContextEntry) bool {
	leftIndex := 0
	rightIndex := 0
	for leftIndex < len(left) && rightIndex < len(right) {
		switch {
		case left[leftIndex].Name == right[rightIndex].Name:
			return true
		case left[leftIndex].Name < right[rightIndex].Name:
			leftIndex++
		default:
			rightIndex++
		}
	}
	return false
}

func relationshipKey(relationship Relationship) string {
	return fmt.Sprintf("%s\x00%s\x00%s\x00%s\x00%s\x00%s",
		relationship.Resource.ObjectType,
		relationship.Resource.ObjectID,
		relationship.Relation,
		relationship.Subject.ObjectType,
		relationship.Subject.ObjectID,
		relationship.SubjectRelation,
	)
}
