package adapter

import (
	"fmt"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

func (t *translator) validatePermissionGraph(
	permissions []*razielv1.PermissionDefinition,
) error {
	byKey := make(map[permissionKey]*razielv1.PermissionDefinition, len(permissions))
	for _, permission := range permissions {
		byKey[permissionKey{permission.GetObjectType(), permission.GetName()}] = permission
	}
	for _, permission := range permissions {
		key := permissionKey{permission.GetObjectType(), permission.GetName()}
		arrows, pathDepth, err := t.permissionMetrics(key, byKey, map[permissionKey]struct{}{})
		if err != nil {
			return err
		}
		if arrows > 1 {
			return locatedError(
				CodeUnsupportedConstruct,
				t.source,
				fmt.Sprintf("permission %s#%s exceeds one arrow per path", key.objectType, key.name),
				t.permissionPosition(key),
			)
		}
		if pathDepth > t.config.Limits().GetMaxPathDepth() {
			return locatedError(
				CodeProgramLimit,
				t.source,
				fmt.Sprintf("permission %s#%s exceeds max_path_depth", key.objectType, key.name),
				t.permissionPosition(key),
			)
		}
	}
	return nil
}

func (t *translator) permissionMetrics(
	key permissionKey,
	permissions map[permissionKey]*razielv1.PermissionDefinition,
	stack map[permissionKey]struct{},
) (uint32, uint32, error) {
	if _, exists := stack[key]; exists {
		return 0, 0, locatedError(
			CodePermissionCycle,
			t.source,
			fmt.Sprintf("permission %s#%s is recursive", key.objectType, key.name),
			t.permissionPosition(key),
		)
	}
	permission := permissions[key]
	if permission == nil {
		return 0, 0, &Error{
			Code:    CodeMissingReference,
			Source:  t.source,
			Message: fmt.Sprintf("permission %s#%s is missing", key.objectType, key.name),
		}
	}
	stack[key] = struct{}{}
	defer delete(stack, key)
	return t.expressionMetrics(key.objectType, permission.GetExpression(), permissions, stack)
}

func (t *translator) expressionMetrics(
	objectType string,
	expression *razielv1.PermissionExpr,
	permissions map[permissionKey]*razielv1.PermissionDefinition,
	stack map[permissionKey]struct{},
) (uint32, uint32, error) {
	switch node := expression.GetNode().(type) {
	case *razielv1.PermissionExpr_RelationRef, *razielv1.PermissionExpr_CaveatedRelation:
		return 0, 1, nil
	case *razielv1.PermissionExpr_SubjectRelationRef:
		return 0, 2, nil
	case *razielv1.PermissionExpr_AnyOf:
		return t.childrenMetrics(objectType, node.AnyOf.GetChildren(), permissions, stack)
	case *razielv1.PermissionExpr_AllOf:
		return t.childrenMetrics(objectType, node.AllOf.GetChildren(), permissions, stack)
	case *razielv1.PermissionExpr_Arrow:
		source := t.relations[objectType][node.Arrow.GetSourceRelation()]
		var position *core.SourcePosition
		if source != nil {
			position = source.GetSourcePosition()
		}
		targetType, err := t.resolveArrowTarget(objectType, node.Arrow.GetSourceRelation(), position)
		if err != nil {
			return 0, 0, err
		}
		arrows, path, err := t.permissionMetrics(
			permissionKey{targetType, node.Arrow.GetTargetPermission()},
			permissions,
			stack,
		)
		if err != nil {
			return 0, 0, err
		}
		return arrows + 1, path + 1, nil
	default:
		return 0, 0, &Error{
			Code:    CodeInvalidCompiledSchema,
			Source:  t.source,
			Message: "PolicyIR permission contains an unknown expression node",
		}
	}
}

func (t *translator) permissionPosition(key permissionKey) *core.SourcePosition {
	permission := t.relations[key.objectType][key.name]
	if permission == nil {
		return nil
	}
	return permission.GetSourcePosition()
}

func (t *translator) childrenMetrics(
	objectType string,
	children []*razielv1.PermissionExpr,
	permissions map[permissionKey]*razielv1.PermissionDefinition,
	stack map[permissionKey]struct{},
) (uint32, uint32, error) {
	var maxArrows uint32
	var maxPath uint32
	for _, child := range children {
		arrows, path, err := t.expressionMetrics(objectType, child, permissions, stack)
		if err != nil {
			return 0, 0, err
		}
		maxArrows = max(maxArrows, arrows)
		maxPath = max(maxPath, path)
	}
	return maxArrows, maxPath, nil
}
