package adapter

import (
	"errors"
	"fmt"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	exprpb "google.golang.org/genproto/googleapis/api/expr/v1alpha1"
)

const (
	celEqual              = "_==_"
	celNotEqual           = "_!=_"
	celLessThan           = "_<_"
	celLessThanOrEqual    = "_<=_"
	celGreaterThan        = "_>_"
	celGreaterThanOrEqual = "_>=_"
	celAnd                = "_&&_"
	celOr                 = "_||_"
	celNot                = "!_"
)

func (t *translator) translateCaveatExpr(
	context caveatContext,
	expression *exprpb.Expr,
) (caveatTranslation, error) {
	if expression == nil {
		return caveatTranslation{}, t.caveatError(context.definition, "CEL expression contains a nil node", nil)
	}

	switch node := expression.GetExprKind().(type) {
	case *exprpb.Expr_ConstExpr:
		return t.translateCaveatConstant(context, node.ConstExpr)
	case *exprpb.Expr_IdentExpr:
		return t.translateCaveatIdentifier(context, node.IdentExpr.GetName())
	case *exprpb.Expr_CallExpr:
		return t.translateCaveatCall(context, node.CallExpr)
	case *exprpb.Expr_SelectExpr:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL select expressions are outside the PolicyIR subset", nil)
	case *exprpb.Expr_ListExpr:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL list expressions are outside the PolicyIR subset", nil)
	case *exprpb.Expr_StructExpr:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL map and struct expressions are outside the PolicyIR subset", nil)
	case *exprpb.Expr_ComprehensionExpr:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL comprehensions are outside the PolicyIR subset", nil)
	default:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL expression contains an unknown node", nil)
	}
}

func (t *translator) translateCaveatConstant(
	context caveatContext,
	constant *exprpb.Constant,
) (caveatTranslation, error) {
	if constant == nil {
		return caveatTranslation{}, t.caveatError(context.definition, "CEL expression contains a nil constant", nil)
	}
	switch value := constant.GetConstantKind().(type) {
	case *exprpb.Constant_BoolValue:
		return caveatTranslation{
			expression: &razielv1.CaveatExpr{Node: &razielv1.CaveatExpr_ConstBool{ConstBool: value.BoolValue}},
			valueType:  razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL,
		}, nil
	case *exprpb.Constant_Int64Value:
		return caveatTranslation{
			expression: &razielv1.CaveatExpr{Node: &razielv1.CaveatExpr_ConstInt{ConstInt: value.Int64Value}},
			valueType:  razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_INT,
		}, nil
	case *exprpb.Constant_StringValue:
		return caveatTranslation{
			expression: &razielv1.CaveatExpr{Node: &razielv1.CaveatExpr_ConstString{ConstString: value.StringValue}},
			valueType:  razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_STRING,
		}, nil
	default:
		return caveatTranslation{}, t.caveatError(context.definition, "CEL constant type is outside the PolicyIR subset", nil)
	}
}

func (t *translator) translateCaveatIdentifier(
	context caveatContext,
	name string,
) (caveatTranslation, error) {
	valueType, exists := context.types[name]
	if !exists {
		return caveatTranslation{}, t.caveatError(
			context.definition,
			fmt.Sprintf("CEL identifier %q is not a declared caveat parameter", name),
			nil,
		)
	}
	source, exists := context.bindings[name]
	if !exists {
		return caveatTranslation{}, t.caveatError(
			context.definition,
			fmt.Sprintf("CEL identifier %q has no provenance binding", name),
			nil,
		)
	}
	return caveatTranslation{
		expression: &razielv1.CaveatExpr{
			Node: &razielv1.CaveatExpr_InputRef{
				InputRef: &razielv1.CaveatInputRef{
					Source:    source,
					Name:      name,
					ValueType: valueType,
				},
			},
		},
		valueType: valueType,
	}, nil
}

func (t *translator) translateCaveatCall(
	context caveatContext,
	call *exprpb.Expr_Call,
) (caveatTranslation, error) {
	if call == nil || call.GetTarget() != nil {
		return caveatTranslation{}, t.caveatError(context.definition, "CEL method calls are outside the PolicyIR subset", nil)
	}

	switch call.GetFunction() {
	case celEqual, celNotEqual, celLessThan, celLessThanOrEqual, celGreaterThan, celGreaterThanOrEqual:
		return t.translateCaveatBinaryCall(context, call.GetFunction(), call.GetArgs())
	case celAnd, celOr:
		return t.translateCaveatBooleanCall(context, call.GetFunction(), call.GetArgs())
	case celNot:
		return t.translateCaveatNot(context, call.GetArgs())
	default:
		return caveatTranslation{}, t.caveatError(
			context.definition,
			fmt.Sprintf("CEL function %q is outside the PolicyIR subset", call.GetFunction()),
			nil,
		)
	}
}

func (t *translator) translateCaveatBinaryCall(
	context caveatContext,
	function string,
	arguments []*exprpb.Expr,
) (caveatTranslation, error) {
	if len(arguments) != 2 {
		return caveatTranslation{}, t.caveatError(context.definition, fmt.Sprintf("CEL function %q requires two arguments", function), nil)
	}
	left, err := t.translateCaveatExpr(context, arguments[0])
	if err != nil {
		return caveatTranslation{}, err
	}
	right, err := t.translateCaveatExpr(context, arguments[1])
	if err != nil {
		return caveatTranslation{}, err
	}
	if left.valueType != right.valueType {
		return caveatTranslation{}, t.caveatError(context.definition, fmt.Sprintf("CEL function %q has mismatched operand types", function), nil)
	}
	if function != celEqual && function != celNotEqual &&
		left.valueType != razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_INT {
		return caveatTranslation{}, t.caveatError(context.definition, fmt.Sprintf("CEL function %q requires integer operands", function), nil)
	}

	binary := &razielv1.CaveatBinaryExpr{Left: left.expression, Right: right.expression}
	result := &razielv1.CaveatExpr{}
	switch function {
	case celEqual:
		result.Node = &razielv1.CaveatExpr_Equal{Equal: binary}
	case celNotEqual:
		result.Node = &razielv1.CaveatExpr_NotEqual{NotEqual: binary}
	case celLessThan:
		result.Node = &razielv1.CaveatExpr_LessThan{LessThan: binary}
	case celLessThanOrEqual:
		result.Node = &razielv1.CaveatExpr_LessThanOrEqual{LessThanOrEqual: binary}
	case celGreaterThan:
		result.Node = &razielv1.CaveatExpr_GreaterThan{GreaterThan: binary}
	case celGreaterThanOrEqual:
		result.Node = &razielv1.CaveatExpr_GreaterThanOrEqual{GreaterThanOrEqual: binary}
	}
	return caveatTranslation{
		expression: result,
		valueType:  razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL,
	}, nil
}

func (t *translator) translateCaveatBooleanCall(
	context caveatContext,
	function string,
	arguments []*exprpb.Expr,
) (caveatTranslation, error) {
	if len(arguments) != 2 {
		return caveatTranslation{}, t.caveatError(context.definition, fmt.Sprintf("CEL function %q requires two arguments", function), nil)
	}

	children := make([]*razielv1.CaveatExpr, 0, 2)
	for _, argument := range arguments {
		translated, err := t.translateCaveatExpr(context, argument)
		if err != nil {
			return caveatTranslation{}, err
		}
		if translated.valueType != razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL {
			return caveatTranslation{}, t.caveatError(context.definition, fmt.Sprintf("CEL function %q requires boolean operands", function), nil)
		}
		children = append(children, flattenBooleanExpression(translated.expression, function)...)
	}
	if err := sortCaveatExpressions(children); err != nil {
		var typed *Error
		if errors.As(err, &typed) {
			return caveatTranslation{}, t.caveatError(context.definition, typed.Message, nil)
		}
		return caveatTranslation{}, err
	}

	result := &razielv1.CaveatExpr{}
	if function == celAnd {
		result.Node = &razielv1.CaveatExpr_And{And: &razielv1.CaveatChildren{Children: children}}
	} else {
		result.Node = &razielv1.CaveatExpr_Or{Or: &razielv1.CaveatChildren{Children: children}}
	}
	return caveatTranslation{
		expression: result,
		valueType:  razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL,
	}, nil
}

func (t *translator) translateCaveatNot(
	context caveatContext,
	arguments []*exprpb.Expr,
) (caveatTranslation, error) {
	if len(arguments) != 1 {
		return caveatTranslation{}, t.caveatError(context.definition, "CEL not operator requires one argument", nil)
	}
	child, err := t.translateCaveatExpr(context, arguments[0])
	if err != nil {
		return caveatTranslation{}, err
	}
	if child.valueType != razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL {
		return caveatTranslation{}, t.caveatError(context.definition, "CEL not operator requires a boolean operand", nil)
	}
	return caveatTranslation{
		expression: &razielv1.CaveatExpr{
			Node: &razielv1.CaveatExpr_Not{
				Not: &razielv1.CaveatUnaryExpr{Child: child.expression},
			},
		},
		valueType: razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL,
	}, nil
}

func flattenBooleanExpression(
	expression *razielv1.CaveatExpr,
	function string,
) []*razielv1.CaveatExpr {
	if function == celAnd {
		if conjunction := expression.GetAnd(); conjunction != nil {
			return conjunction.GetChildren()
		}
	}
	if function == celOr {
		if disjunction := expression.GetOr(); disjunction != nil {
			return disjunction.GetChildren()
		}
	}
	return []*razielv1.CaveatExpr{expression}
}
