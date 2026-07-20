package adapter

import (
	"cmp"
	"slices"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

func sortPermissionExpressions(expressions []*razielv1.PermissionExpr) error {
	slices.SortFunc(expressions, comparePermissionExpr)
	for index := 1; index < len(expressions); index++ {
		if comparePermissionExpr(expressions[index-1], expressions[index]) == 0 {
			return &Error{
				Code:    CodeInvalidCompiledSchema,
				Message: "permission expression contains duplicate children",
			}
		}
	}
	return nil
}

func comparePermissionExpr(left, right *razielv1.PermissionExpr) int {
	leftRank := permissionRank(left)
	rightRank := permissionRank(right)
	if order := cmp.Compare(leftRank, rightRank); order != 0 {
		return order
	}

	switch leftNode := left.GetNode().(type) {
	case *razielv1.PermissionExpr_RelationRef:
		return cmp.Compare(leftNode.RelationRef.GetName(), right.GetRelationRef().GetName())
	case *razielv1.PermissionExpr_SubjectRelationRef:
		rightNode := right.GetSubjectRelationRef()
		return compareStrings(
			leftNode.SubjectRelationRef.GetRelation(),
			rightNode.GetRelation(),
			leftNode.SubjectRelationRef.GetSubjectRelation(),
			rightNode.GetSubjectRelation(),
		)
	case *razielv1.PermissionExpr_AnyOf:
		return comparePermissionChildren(
			leftNode.AnyOf.GetChildren(),
			right.GetAnyOf().GetChildren(),
		)
	case *razielv1.PermissionExpr_AllOf:
		return comparePermissionChildren(
			leftNode.AllOf.GetChildren(),
			right.GetAllOf().GetChildren(),
		)
	case *razielv1.PermissionExpr_Arrow:
		rightNode := right.GetArrow()
		return compareStrings(
			leftNode.Arrow.GetSourceRelation(),
			rightNode.GetSourceRelation(),
			leftNode.Arrow.GetTargetPermission(),
			rightNode.GetTargetPermission(),
		)
	case *razielv1.PermissionExpr_CaveatedRelation:
		rightNode := right.GetCaveatedRelation()
		return compareStrings(
			leftNode.CaveatedRelation.GetRelation(),
			rightNode.GetRelation(),
			leftNode.CaveatedRelation.GetCaveatName(),
			rightNode.GetCaveatName(),
		)
	default:
		return 0
	}
}

func permissionRank(expression *razielv1.PermissionExpr) int {
	if expression == nil {
		return -1
	}
	switch expression.GetNode().(type) {
	case *razielv1.PermissionExpr_RelationRef:
		return 0
	case *razielv1.PermissionExpr_SubjectRelationRef:
		return 1
	case *razielv1.PermissionExpr_AnyOf:
		return 2
	case *razielv1.PermissionExpr_AllOf:
		return 3
	case *razielv1.PermissionExpr_Arrow:
		return 4
	case *razielv1.PermissionExpr_CaveatedRelation:
		return 5
	default:
		return 6
	}
}

func comparePermissionChildren(left, right []*razielv1.PermissionExpr) int {
	for index := 0; index < min(len(left), len(right)); index++ {
		if order := comparePermissionExpr(left[index], right[index]); order != 0 {
			return order
		}
	}
	return cmp.Compare(len(left), len(right))
}

func sortCaveatExpressions(expressions []*razielv1.CaveatExpr) error {
	slices.SortFunc(expressions, compareCaveatExpr)
	for index := 1; index < len(expressions); index++ {
		if compareCaveatExpr(expressions[index-1], expressions[index]) == 0 {
			return &Error{
				Code:    CodeCaveatExpression,
				Message: "caveat expression contains duplicate children",
			}
		}
	}
	return nil
}

func compareCaveatExpr(left, right *razielv1.CaveatExpr) int {
	leftRank := caveatRank(left)
	rightRank := caveatRank(right)
	if order := cmp.Compare(leftRank, rightRank); order != 0 {
		return order
	}

	switch leftNode := left.GetNode().(type) {
	case *razielv1.CaveatExpr_ConstBool:
		return cmp.Compare(boolRank(leftNode.ConstBool), boolRank(right.GetConstBool()))
	case *razielv1.CaveatExpr_ConstInt:
		return cmp.Compare(leftNode.ConstInt, right.GetConstInt())
	case *razielv1.CaveatExpr_ConstString:
		return cmp.Compare(leftNode.ConstString, right.GetConstString())
	case *razielv1.CaveatExpr_InputRef:
		rightNode := right.GetInputRef()
		return compareIntsAndStrings(
			int32(leftNode.InputRef.GetSource()),
			int32(rightNode.GetSource()),
			leftNode.InputRef.GetName(),
			rightNode.GetName(),
			int32(leftNode.InputRef.GetValueType()),
			int32(rightNode.GetValueType()),
		)
	case *razielv1.CaveatExpr_Equal:
		return compareCaveatBinary(leftNode.Equal, right.GetEqual())
	case *razielv1.CaveatExpr_NotEqual:
		return compareCaveatBinary(leftNode.NotEqual, right.GetNotEqual())
	case *razielv1.CaveatExpr_LessThan:
		return compareCaveatBinary(leftNode.LessThan, right.GetLessThan())
	case *razielv1.CaveatExpr_LessThanOrEqual:
		return compareCaveatBinary(leftNode.LessThanOrEqual, right.GetLessThanOrEqual())
	case *razielv1.CaveatExpr_GreaterThan:
		return compareCaveatBinary(leftNode.GreaterThan, right.GetGreaterThan())
	case *razielv1.CaveatExpr_GreaterThanOrEqual:
		return compareCaveatBinary(leftNode.GreaterThanOrEqual, right.GetGreaterThanOrEqual())
	case *razielv1.CaveatExpr_And:
		return compareCaveatChildren(leftNode.And.GetChildren(), right.GetAnd().GetChildren())
	case *razielv1.CaveatExpr_Or:
		return compareCaveatChildren(leftNode.Or.GetChildren(), right.GetOr().GetChildren())
	case *razielv1.CaveatExpr_Not:
		return compareCaveatExpr(leftNode.Not.GetChild(), right.GetNot().GetChild())
	default:
		return 0
	}
}

func caveatRank(expression *razielv1.CaveatExpr) int {
	if expression == nil {
		return -1
	}
	switch expression.GetNode().(type) {
	case *razielv1.CaveatExpr_ConstBool:
		return 0
	case *razielv1.CaveatExpr_ConstInt:
		return 1
	case *razielv1.CaveatExpr_ConstString:
		return 2
	case *razielv1.CaveatExpr_InputRef:
		return 3
	case *razielv1.CaveatExpr_Equal:
		return 4
	case *razielv1.CaveatExpr_NotEqual:
		return 5
	case *razielv1.CaveatExpr_LessThan:
		return 6
	case *razielv1.CaveatExpr_LessThanOrEqual:
		return 7
	case *razielv1.CaveatExpr_GreaterThan:
		return 8
	case *razielv1.CaveatExpr_GreaterThanOrEqual:
		return 9
	case *razielv1.CaveatExpr_And:
		return 10
	case *razielv1.CaveatExpr_Or:
		return 11
	case *razielv1.CaveatExpr_Not:
		return 12
	default:
		return 13
	}
}

func compareCaveatBinary(left, right *razielv1.CaveatBinaryExpr) int {
	if order := compareCaveatExpr(left.GetLeft(), right.GetLeft()); order != 0 {
		return order
	}
	return compareCaveatExpr(left.GetRight(), right.GetRight())
}

func compareCaveatChildren(left, right []*razielv1.CaveatExpr) int {
	for index := 0; index < min(len(left), len(right)); index++ {
		if order := compareCaveatExpr(left[index], right[index]); order != 0 {
			return order
		}
	}
	return cmp.Compare(len(left), len(right))
}

func compareStrings(values ...string) int {
	for index := 0; index < len(values); index += 2 {
		if order := cmp.Compare(values[index], values[index+1]); order != 0 {
			return order
		}
	}
	return 0
}

func compareIntsAndStrings(
	leftFirst, rightFirst int32,
	leftString, rightString string,
	leftSecond, rightSecond int32,
) int {
	if order := cmp.Compare(leftFirst, rightFirst); order != 0 {
		return order
	}
	if order := cmp.Compare(leftString, rightString); order != 0 {
		return order
	}
	return cmp.Compare(leftSecond, rightSecond)
}

func boolRank(value bool) int {
	if value {
		return 1
	}
	return 0
}
