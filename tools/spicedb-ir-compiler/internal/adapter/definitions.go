package adapter

import (
	"cmp"
	"fmt"
	"slices"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

const spiceDBEllipsis = "..."

func (t *translator) translateRelation(compiled *core.Relation) (*razielv1.RelationDefinition, error) {
	allowed := compiled.GetTypeInformation().GetAllowedDirectRelations()
	if len(allowed) == 0 {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			fmt.Sprintf("relation %q has no allowed subject types", compiled.GetName()),
			compiled.GetSourcePosition(),
		)
	}

	relation := &razielv1.RelationDefinition{Name: compiled.GetName()}
	for _, candidate := range allowed {
		translated, err := t.translateAllowedSubject(candidate)
		if err != nil {
			return nil, err
		}
		relation.AllowedSubjectTypes = append(relation.AllowedSubjectTypes, translated)
	}

	slices.SortFunc(relation.AllowedSubjectTypes, compareAllowedSubject)
	for index := 1; index < len(relation.AllowedSubjectTypes); index++ {
		if compareAllowedSubject(
			relation.AllowedSubjectTypes[index-1],
			relation.AllowedSubjectTypes[index],
		) == 0 {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				t.source,
				fmt.Sprintf("relation %q contains duplicate allowed subject types", compiled.GetName()),
				compiled.GetSourcePosition(),
			)
		}
	}
	return relation, nil
}

func (t *translator) translateAllowedSubject(
	allowed *core.AllowedRelation,
) (*razielv1.AllowedSubjectType, error) {
	if allowed == nil || allowed.GetNamespace() == "" {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled allowed subject type has no object type",
			positionOfAllowed(allowed),
		)
	}
	if err := validatePolicyIdentifier(
		t.source,
		"allowed subject object type",
		allowed.GetNamespace(),
		allowed.GetSourcePosition(),
	); err != nil {
		return nil, err
	}
	if allowed.GetRequiredExpiration() != nil {
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"expiration traits are outside the PolicyIR subset",
			allowed.GetSourcePosition(),
		)
	}

	result := &razielv1.AllowedSubjectType{ObjectType: allowed.GetNamespace()}
	switch relation := allowed.GetRelationOrWildcard().(type) {
	case *core.AllowedRelation_Relation:
		if relation.Relation == "" {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				t.source,
				"compiled allowed subject relation is empty",
				allowed.GetSourcePosition(),
			)
		}
		if relation.Relation != spiceDBEllipsis {
			if err := validatePolicyIdentifier(
				t.source,
				"allowed subject relation",
				relation.Relation,
				allowed.GetSourcePosition(),
			); err != nil {
				return nil, err
			}
			subjectRelation := relation.Relation
			result.SubjectRelation = &subjectRelation
		}
	case *core.AllowedRelation_PublicWildcard_:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"public wildcards are outside the PolicyIR subset",
			allowed.GetSourcePosition(),
		)
	default:
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled allowed subject has an unknown relation shape",
			allowed.GetSourcePosition(),
		)
	}

	if caveat := allowed.GetRequiredCaveat(); caveat != nil {
		if caveat.GetCaveatName() == "" {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				t.source,
				"compiled allowed subject has an empty caveat name",
				allowed.GetSourcePosition(),
			)
		}
		if err := validatePolicyIdentifier(
			t.source,
			"required caveat",
			caveat.GetCaveatName(),
			allowed.GetSourcePosition(),
		); err != nil {
			return nil, err
		}
		if result.SubjectRelation != nil {
			return nil, locatedError(
				CodeUnsupportedConstruct,
				t.source,
				"caveated subject sets are outside the PolicyIR subset",
				allowed.GetSourcePosition(),
			)
		}
		name := caveat.GetCaveatName()
		result.CaveatName = &name
	}
	return result, nil
}

func compareAllowedSubject(left, right *razielv1.AllowedSubjectType) int {
	if order := cmp.Compare(left.GetObjectType(), right.GetObjectType()); order != 0 {
		return order
	}
	if order := compareOptionalString(left.SubjectRelation, right.SubjectRelation); order != 0 {
		return order
	}
	return compareOptionalString(left.CaveatName, right.CaveatName)
}

func compareOptionalString(left, right *string) int {
	switch {
	case left == nil && right == nil:
		return 0
	case left == nil:
		return -1
	case right == nil:
		return 1
	default:
		return cmp.Compare(*left, *right)
	}
}

func positionOfAllowed(allowed *core.AllowedRelation) *core.SourcePosition {
	if allowed == nil {
		return nil
	}
	return allowed.GetSourcePosition()
}
