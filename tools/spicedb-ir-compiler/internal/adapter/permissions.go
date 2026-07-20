package adapter

import (
	"errors"
	"fmt"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

type permissionKey struct {
	objectType string
	name       string
}

func (t *translator) translatePermission(
	objectType string,
	permission *core.Relation,
	stack map[permissionKey]struct{},
) (*razielv1.PermissionExpr, error) {
	key := permissionKey{objectType: objectType, name: permission.GetName()}
	if _, exists := stack[key]; exists {
		return nil, locatedError(
			CodePermissionCycle,
			t.source,
			fmt.Sprintf("permission %s#%s is recursive", objectType, permission.GetName()),
			permission.GetSourcePosition(),
		)
	}
	stack[key] = struct{}{}
	defer delete(stack, key)

	return t.translateRewrite(objectType, permission.GetUsersetRewrite(), stack)
}

func (t *translator) translateRewrite(
	objectType string,
	rewrite *core.UsersetRewrite,
	stack map[permissionKey]struct{},
) (*razielv1.PermissionExpr, error) {
	if rewrite == nil {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled permission has no userset rewrite",
			nil,
		)
	}

	switch operation := rewrite.GetRewriteOperation().(type) {
	case *core.UsersetRewrite_Union:
		return t.translateSetOperation(objectType, operation.Union, false, stack, rewrite.GetSourcePosition())
	case *core.UsersetRewrite_Intersection:
		return t.translateSetOperation(objectType, operation.Intersection, true, stack, rewrite.GetSourcePosition())
	case *core.UsersetRewrite_Exclusion:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"exclusion is outside the PolicyIR subset",
			rewrite.GetSourcePosition(),
		)
	default:
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled permission has an unknown rewrite operation",
			rewrite.GetSourcePosition(),
		)
	}
}

func (t *translator) translateSetOperation(
	objectType string,
	operation *core.SetOperation,
	intersection bool,
	stack map[permissionKey]struct{},
	position *core.SourcePosition,
) (*razielv1.PermissionExpr, error) {
	if operation == nil || len(operation.GetChild()) == 0 {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled set operation has no children",
			position,
		)
	}

	children := make([]*razielv1.PermissionExpr, 0, len(operation.GetChild()))
	for _, child := range operation.GetChild() {
		translated, err := t.translatePermissionChild(objectType, child, stack)
		if err != nil {
			return nil, err
		}
		children = append(children, translated)
	}
	if len(children) == 1 {
		return children[0], nil
	}
	if err := sortPermissionExpressions(children); err != nil {
		var typed *Error
		if errors.As(err, &typed) {
			return nil, locatedError(typed.Code, t.source, typed.Message, position)
		}
		return nil, err
	}

	if intersection {
		return &razielv1.PermissionExpr{
			Node: &razielv1.PermissionExpr_AllOf{
				AllOf: &razielv1.PermissionChildren{Children: children},
			},
		}, nil
	}
	return &razielv1.PermissionExpr{
		Node: &razielv1.PermissionExpr_AnyOf{
			AnyOf: &razielv1.PermissionChildren{Children: children},
		},
	}, nil
}

func (t *translator) translatePermissionChild(
	objectType string,
	child *core.SetOperation_Child,
	stack map[permissionKey]struct{},
) (*razielv1.PermissionExpr, error) {
	if child == nil {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled set operation contains a nil child",
			nil,
		)
	}

	switch node := child.GetChildType().(type) {
	case *core.SetOperation_Child_ComputedUserset:
		return t.translateComputedUserset(objectType, node.ComputedUserset, stack, child.GetSourcePosition())
	case *core.SetOperation_Child_TupleToUserset:
		return t.translateArrow(objectType, node.TupleToUserset, child.GetSourcePosition())
	case *core.SetOperation_Child_UsersetRewrite:
		return t.translateRewrite(objectType, node.UsersetRewrite, stack)
	case *core.SetOperation_Child_FunctionedTupleToUserset:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"functioned arrows are outside the PolicyIR subset",
			child.GetSourcePosition(),
		)
	case *core.SetOperation_Child_XNil:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"nil expressions are outside the PolicyIR subset",
			child.GetSourcePosition(),
		)
	case *core.SetOperation_Child_XSelf:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"self expressions are outside the PolicyIR subset",
			child.GetSourcePosition(),
		)
	case *core.SetOperation_Child_XThis:
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"deprecated this expressions are outside the PolicyIR subset",
			child.GetSourcePosition(),
		)
	default:
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled set operation contains an unknown child",
			child.GetSourcePosition(),
		)
	}
}

func (t *translator) translateComputedUserset(
	objectType string,
	computed *core.ComputedUserset,
	stack map[permissionKey]struct{},
	position *core.SourcePosition,
) (*razielv1.PermissionExpr, error) {
	if computed == nil || computed.GetRelation() == "" {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"computed userset has no relation name",
			position,
		)
	}
	if computed.GetObject() != core.ComputedUserset_TUPLE_OBJECT {
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"computed userset uses an unsupported object mode",
			position,
		)
	}

	target := t.relations[objectType][computed.GetRelation()]
	if target == nil {
		return nil, locatedError(
			CodeMissingReference,
			t.source,
			fmt.Sprintf("permission references missing member %s#%s", objectType, computed.GetRelation()),
			position,
		)
	}
	kind, _ := classifyMember(target)
	if kind == memberPermission {
		return t.translatePermission(objectType, target, stack)
	}
	return t.relationReferenceExpression(target, position)
}

func (t *translator) relationReferenceExpression(
	relation *core.Relation,
	position *core.SourcePosition,
) (*razielv1.PermissionExpr, error) {
	translated, err := t.translateRelation(relation)
	if err != nil {
		return nil, err
	}

	expressions := make([]*razielv1.PermissionExpr, 0)
	hasDirect := false
	subjectRelations := map[string]string{}
	caveats := map[string]struct{}{}
	for _, allowed := range translated.GetAllowedSubjectTypes() {
		switch {
		case allowed.SubjectRelation != nil:
			existingType, exists := subjectRelations[allowed.GetSubjectRelation()]
			if exists && existingType != allowed.GetObjectType() {
				return nil, locatedError(
					CodeAmbiguousReference,
					t.source,
					fmt.Sprintf(
						"relation %q maps subject relation %q to more than one object type",
						relation.GetName(),
						allowed.GetSubjectRelation(),
					),
					position,
				)
			}
			subjectRelations[allowed.GetSubjectRelation()] = allowed.GetObjectType()
		case allowed.CaveatName != nil:
			caveats[allowed.GetCaveatName()] = struct{}{}
		default:
			hasDirect = true
		}
	}

	if hasDirect {
		expressions = append(expressions, &razielv1.PermissionExpr{
			Node: &razielv1.PermissionExpr_RelationRef{
				RelationRef: &razielv1.PermissionRelationRef{Name: relation.GetName()},
			},
		})
	}
	for subjectRelation := range subjectRelations {
		if err := t.validateSubjectRelationTarget(
			subjectRelations[subjectRelation],
			subjectRelation,
			position,
		); err != nil {
			return nil, err
		}
		expressions = append(expressions, &razielv1.PermissionExpr{
			Node: &razielv1.PermissionExpr_SubjectRelationRef{
				SubjectRelationRef: &razielv1.PermissionSubjectRelationRef{
					Relation:        relation.GetName(),
					SubjectRelation: subjectRelation,
				},
			},
		})
	}
	for caveatName := range caveats {
		expressions = append(expressions, &razielv1.PermissionExpr{
			Node: &razielv1.PermissionExpr_CaveatedRelation{
				CaveatedRelation: &razielv1.PermissionCaveatedRelation{
					Relation:   relation.GetName(),
					CaveatName: caveatName,
				},
			},
		})
	}
	if len(expressions) == 0 {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			fmt.Sprintf("relation %q has no translatable subject form", relation.GetName()),
			position,
		)
	}
	if len(expressions) == 1 {
		return expressions[0], nil
	}
	if err := sortPermissionExpressions(expressions); err != nil {
		return nil, err
	}
	return &razielv1.PermissionExpr{
		Node: &razielv1.PermissionExpr_AnyOf{
			AnyOf: &razielv1.PermissionChildren{Children: expressions},
		},
	}, nil
}

func (t *translator) validateSubjectRelationTarget(
	objectType, relationName string,
	position *core.SourcePosition,
) error {
	target := t.relations[objectType][relationName]
	if target == nil {
		return locatedError(
			CodeMissingReference,
			t.source,
			fmt.Sprintf("subject set references missing relation %s#%s", objectType, relationName),
			position,
		)
	}
	kind, _ := classifyMember(target)
	if kind != memberRelation {
		return locatedError(
			CodeUnsupportedConstruct,
			t.source,
			fmt.Sprintf("subject set target %s#%s is not a relation", objectType, relationName),
			position,
		)
	}
	for _, allowed := range target.GetTypeInformation().GetAllowedDirectRelations() {
		if allowed.GetRelation() != spiceDBEllipsis ||
			allowed.GetRequiredCaveat() != nil ||
			allowed.GetRequiredExpiration() != nil {
			return locatedError(
				CodeUnsupportedConstruct,
				t.source,
				fmt.Sprintf(
					"subject set target %s#%s must contain only uncaveated direct memberships",
					objectType,
					relationName,
				),
				position,
			)
		}
	}
	return nil
}

func (t *translator) translateArrow(
	objectType string,
	arrow *core.TupleToUserset,
	position *core.SourcePosition,
) (*razielv1.PermissionExpr, error) {
	if arrow == nil || arrow.GetTupleset() == nil || arrow.GetComputedUserset() == nil {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled arrow is incomplete",
			position,
		)
	}
	sourceRelation := arrow.GetTupleset().GetRelation()
	targetPermission := arrow.GetComputedUserset().GetRelation()
	if sourceRelation == "" || targetPermission == "" {
		return nil, locatedError(
			CodeInvalidCompiledSchema,
			t.source,
			"compiled arrow has an empty source relation or target permission",
			position,
		)
	}
	if arrow.GetComputedUserset().GetObject() != core.ComputedUserset_TUPLE_USERSET_OBJECT {
		return nil, locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"arrow target uses an unsupported object mode",
			position,
		)
	}

	targetType, err := t.resolveArrowTarget(objectType, sourceRelation, position)
	if err != nil {
		return nil, err
	}
	target := t.relations[targetType][targetPermission]
	if target == nil {
		return nil, locatedError(
			CodeMissingReference,
			t.source,
			fmt.Sprintf("arrow references missing permission %s#%s", targetType, targetPermission),
			position,
		)
	}
	kind, _ := classifyMember(target)
	if kind != memberPermission {
		return nil, locatedError(
			CodeMissingReference,
			t.source,
			fmt.Sprintf("arrow target %s#%s is not a permission", targetType, targetPermission),
			position,
		)
	}

	return &razielv1.PermissionExpr{
		Node: &razielv1.PermissionExpr_Arrow{
			Arrow: &razielv1.PermissionArrow{
				SourceRelation:   sourceRelation,
				TargetPermission: targetPermission,
			},
		},
	}, nil
}

func (t *translator) resolveArrowTarget(
	objectType, sourceRelation string,
	position *core.SourcePosition,
) (string, error) {
	source := t.relations[objectType][sourceRelation]
	if source == nil {
		return "", locatedError(
			CodeMissingReference,
			t.source,
			fmt.Sprintf("arrow references missing relation %s#%s", objectType, sourceRelation),
			position,
		)
	}
	kind, _ := classifyMember(source)
	if kind != memberRelation {
		return "", locatedError(
			CodeUnsupportedConstruct,
			t.source,
			fmt.Sprintf("arrow source %s#%s is not a relation", objectType, sourceRelation),
			position,
		)
	}
	allowed := source.GetTypeInformation().GetAllowedDirectRelations()
	if len(allowed) != 1 {
		return "", locatedError(
			CodeAmbiguousReference,
			t.source,
			fmt.Sprintf("arrow source %s#%s must have one direct target type", objectType, sourceRelation),
			position,
		)
	}
	target := allowed[0]
	if target.GetRelation() != spiceDBEllipsis ||
		target.GetRequiredCaveat() != nil ||
		target.GetRequiredExpiration() != nil {
		return "", locatedError(
			CodeUnsupportedConstruct,
			t.source,
			"arrow source must be one uncaveated direct relation",
			position,
		)
	}
	return target.GetNamespace(), nil
}
