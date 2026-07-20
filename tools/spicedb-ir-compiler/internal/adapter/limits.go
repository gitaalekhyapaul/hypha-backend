package adapter

import (
	"fmt"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
)

func validateDeclaredProgramLimits(source string, program *razielv1.PolicyProgram) error {
	limits := program.GetLimits()
	var permissionNodes uint64
	for _, permission := range program.GetPermissions() {
		permissionNodes += countPermissionNodes(permission.GetExpression())
	}
	if permissionNodes > uint64(limits.GetMaxExpressionNodes()) {
		return programLimitError(
			source,
			"permission expression nodes",
			permissionNodes,
			limits.GetMaxExpressionNodes(),
		)
	}

	var caveatNodes uint64
	for _, caveat := range program.GetCaveats() {
		caveatNodes += countCaveatNodes(caveat.GetExpression())
	}
	if caveatNodes > uint64(limits.GetMaxCaveatNodes()) {
		return programLimitError(
			source,
			"caveat expression nodes",
			caveatNodes,
			limits.GetMaxCaveatNodes(),
		)
	}

	for _, value := range programStrings(program) {
		if uint64(len(value)) > uint64(limits.GetMaxStringBytes()) {
			return &Error{
				Code:    CodeProgramLimit,
				Source:  source,
				Message: fmt.Sprintf("PolicyIR string exceeds max_string_bytes: %q", value),
			}
		}
	}
	return nil
}

func countPermissionNodes(expression *razielv1.PermissionExpr) uint64 {
	if expression == nil {
		return 0
	}
	count := uint64(1)
	if children := expression.GetAnyOf(); children != nil {
		for _, child := range children.GetChildren() {
			count += countPermissionNodes(child)
		}
	}
	if children := expression.GetAllOf(); children != nil {
		for _, child := range children.GetChildren() {
			count += countPermissionNodes(child)
		}
	}
	return count
}

func countCaveatNodes(expression *razielv1.CaveatExpr) uint64 {
	if expression == nil {
		return 0
	}
	count := uint64(1)
	for _, binary := range []*razielv1.CaveatBinaryExpr{
		expression.GetEqual(),
		expression.GetNotEqual(),
		expression.GetLessThan(),
		expression.GetLessThanOrEqual(),
		expression.GetGreaterThan(),
		expression.GetGreaterThanOrEqual(),
	} {
		if binary != nil {
			count += countCaveatNodes(binary.GetLeft())
			count += countCaveatNodes(binary.GetRight())
			return count
		}
	}
	for _, children := range []*razielv1.CaveatChildren{expression.GetAnd(), expression.GetOr()} {
		if children != nil {
			for _, child := range children.GetChildren() {
				count += countCaveatNodes(child)
			}
			return count
		}
	}
	if unary := expression.GetNot(); unary != nil {
		count += countCaveatNodes(unary.GetChild())
	}
	return count
}

func programStrings(program *razielv1.PolicyProgram) []string {
	values := []string{program.GetCompilerName(), program.GetCompilerVersion()}
	for _, definition := range program.GetDefinitions() {
		values = append(values, definition.GetName())
		for _, relation := range definition.GetRelations() {
			values = append(values, relation.GetName())
			for _, allowed := range relation.GetAllowedSubjectTypes() {
				values = append(values, allowed.GetObjectType())
				if allowed.SubjectRelation != nil {
					values = append(values, allowed.GetSubjectRelation())
				}
				if allowed.CaveatName != nil {
					values = append(values, allowed.GetCaveatName())
				}
			}
		}
	}
	for _, permission := range program.GetPermissions() {
		values = append(values, permission.GetObjectType(), permission.GetName())
		collectPermissionStrings(permission.GetExpression(), &values)
	}
	for _, caveat := range program.GetCaveats() {
		values = append(values, caveat.GetName())
		for _, parameter := range caveat.GetParameters() {
			values = append(values, parameter.GetName())
		}
		collectCaveatStrings(caveat.GetExpression(), &values)
	}
	return values
}

func collectPermissionStrings(expression *razielv1.PermissionExpr, values *[]string) {
	if expression == nil {
		return
	}
	if relation := expression.GetRelationRef(); relation != nil {
		*values = append(*values, relation.GetName())
	}
	if userset := expression.GetSubjectRelationRef(); userset != nil {
		*values = append(*values, userset.GetRelation(), userset.GetSubjectRelation())
	}
	if arrow := expression.GetArrow(); arrow != nil {
		*values = append(*values, arrow.GetSourceRelation(), arrow.GetTargetPermission())
	}
	if caveated := expression.GetCaveatedRelation(); caveated != nil {
		*values = append(*values, caveated.GetRelation(), caveated.GetCaveatName())
	}
	for _, children := range []*razielv1.PermissionChildren{expression.GetAnyOf(), expression.GetAllOf()} {
		if children != nil {
			for _, child := range children.GetChildren() {
				collectPermissionStrings(child, values)
			}
		}
	}
}

func collectCaveatStrings(expression *razielv1.CaveatExpr, values *[]string) {
	if expression == nil {
		return
	}
	if constant, ok := expression.GetNode().(*razielv1.CaveatExpr_ConstString); ok {
		*values = append(*values, constant.ConstString)
	}
	if input := expression.GetInputRef(); input != nil {
		*values = append(*values, input.GetName())
	}
	for _, binary := range []*razielv1.CaveatBinaryExpr{
		expression.GetEqual(),
		expression.GetNotEqual(),
		expression.GetLessThan(),
		expression.GetLessThanOrEqual(),
		expression.GetGreaterThan(),
		expression.GetGreaterThanOrEqual(),
	} {
		if binary != nil {
			collectCaveatStrings(binary.GetLeft(), values)
			collectCaveatStrings(binary.GetRight(), values)
			return
		}
	}
	for _, children := range []*razielv1.CaveatChildren{expression.GetAnd(), expression.GetOr()} {
		if children != nil {
			for _, child := range children.GetChildren() {
				collectCaveatStrings(child, values)
			}
			return
		}
	}
	if unary := expression.GetNot(); unary != nil {
		collectCaveatStrings(unary.GetChild(), values)
	}
}

func programLimitError(source, field string, actual uint64, limit uint32) *Error {
	return &Error{
		Code:    CodeProgramLimit,
		Source:  source,
		Message: fmt.Sprintf("%s = %d exceeds declared limit %d", field, actual, limit),
	}
}
