package adapter

import (
	"cmp"
	"fmt"
	"slices"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	impl "github.com/authzed/spicedb/pkg/proto/impl/v1"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"google.golang.org/protobuf/proto"
)

type caveatTranslation struct {
	expression *razielv1.CaveatExpr
	valueType  razielv1.CaveatValueType
}

type caveatContext struct {
	definition *core.CaveatDefinition
	bindings   map[string]razielv1.CaveatInputSource
	types      map[string]razielv1.CaveatValueType
}

func (t *translator) translateCaveats() ([]*razielv1.CaveatDefinition, error) {
	bindings := t.config.CaveatBindings()
	if err := t.validateCaveatBindings(bindings); err != nil {
		return nil, err
	}

	caveats := make([]*razielv1.CaveatDefinition, 0, len(t.compiled.CaveatDefinitions))
	for _, compiled := range t.compiled.CaveatDefinitions {
		translated, err := t.translateCaveat(compiled, bindings[compiled.GetName()])
		if err != nil {
			return nil, err
		}
		caveats = append(caveats, translated)
	}
	slices.SortFunc(caveats, func(left, right *razielv1.CaveatDefinition) int {
		return cmp.Compare(left.GetName(), right.GetName())
	})
	return caveats, nil
}

func (t *translator) validateCaveatBindings(
	bindings map[string]map[string]razielv1.CaveatInputSource,
) error {
	for _, caveat := range t.compiled.CaveatDefinitions {
		configured, exists := bindings[caveat.GetName()]
		if !exists {
			return locatedError(
				CodeCaveatBinding,
				t.source,
				fmt.Sprintf("caveat %q has no provenance binding", caveat.GetName()),
				caveat.GetSourcePosition(),
			)
		}

		for _, parameterName := range sortedKeys(caveat.GetParameterTypes()) {
			if _, exists := configured[parameterName]; !exists {
				return locatedError(
					CodeCaveatBinding,
					t.source,
					fmt.Sprintf(
						"caveat %q parameter %q has no provenance binding",
						caveat.GetName(),
						parameterName,
					),
					caveat.GetSourcePosition(),
				)
			}
		}
		for _, parameterName := range sortedKeys(configured) {
			if _, exists := caveat.GetParameterTypes()[parameterName]; !exists {
				return locatedError(
					CodeCaveatBinding,
					t.source,
					fmt.Sprintf(
						"caveat %q has an extra provenance binding for %q",
						caveat.GetName(),
						parameterName,
					),
					caveat.GetSourcePosition(),
				)
			}
		}
	}

	for _, caveatName := range sortedKeys(bindings) {
		if _, exists := t.compiledCaveats[caveatName]; !exists {
			return &Error{
				Code:    CodeCaveatBinding,
				Source:  t.source,
				Message: fmt.Sprintf("compiler config has an extra caveat binding for %q", caveatName),
			}
		}
	}
	return nil
}

func (t *translator) translateCaveat(
	compiled *core.CaveatDefinition,
	bindings map[string]razielv1.CaveatInputSource,
) (*razielv1.CaveatDefinition, error) {
	parameters := make([]*razielv1.CaveatParameter, 0, len(compiled.GetParameterTypes()))
	types := make(map[string]razielv1.CaveatValueType, len(compiled.GetParameterTypes()))
	for _, parameterName := range sortedKeys(compiled.GetParameterTypes()) {
		valueType, err := t.translateCaveatType(compiled, parameterName)
		if err != nil {
			return nil, err
		}
		types[parameterName] = valueType
		parameters = append(parameters, &razielv1.CaveatParameter{
			Source:    bindings[parameterName],
			Name:      parameterName,
			ValueType: valueType,
		})
	}
	slices.SortFunc(parameters, func(left, right *razielv1.CaveatParameter) int {
		if order := cmp.Compare(left.GetSource(), right.GetSource()); order != 0 {
			return order
		}
		return cmp.Compare(left.GetName(), right.GetName())
	})

	decoded := &impl.DecodedCaveat{}
	if err := proto.Unmarshal(compiled.GetSerializedExpression(), decoded); err != nil {
		return nil, t.caveatError(compiled, "serialized CEL expression is malformed", err)
	}
	if decoded.GetName() != compiled.GetName() {
		return nil, t.caveatError(
			compiled,
			fmt.Sprintf(
				"serialized CEL name %q does not match compiled caveat name %q",
				decoded.GetName(),
				compiled.GetName(),
			),
			nil,
		)
	}
	checked := decoded.GetCel()
	if checked == nil || checked.GetExpr() == nil {
		return nil, t.caveatError(compiled, "serialized caveat has no checked CEL expression", nil)
	}

	context := caveatContext{definition: compiled, bindings: bindings, types: types}
	translated, err := t.translateCaveatExpr(context, checked.GetExpr())
	if err != nil {
		return nil, err
	}
	if translated.valueType != razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL {
		return nil, t.caveatError(compiled, "caveat root expression is not boolean", nil)
	}
	return &razielv1.CaveatDefinition{
		Name:       compiled.GetName(),
		Parameters: parameters,
		Expression: translated.expression,
	}, nil
}

func (t *translator) translateCaveatType(
	caveat *core.CaveatDefinition,
	parameterName string,
) (razielv1.CaveatValueType, error) {
	reference := caveat.GetParameterTypes()[parameterName]
	if reference == nil || len(reference.GetChildTypes()) != 0 {
		return razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_UNSPECIFIED,
			t.caveatError(caveat, fmt.Sprintf("parameter %q does not use a supported scalar type", parameterName), nil)
	}
	switch reference.GetTypeName() {
	case "bool":
		return razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_BOOL, nil
	case "int":
		return razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_INT, nil
	case "string":
		return razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_STRING, nil
	default:
		return razielv1.CaveatValueType_CAVEAT_VALUE_TYPE_UNSPECIFIED,
			t.caveatError(
				caveat,
				fmt.Sprintf("parameter %q uses unsupported CEL type %q", parameterName, reference.GetTypeName()),
				nil,
			)
	}
}

func (t *translator) caveatError(
	caveat *core.CaveatDefinition,
	message string,
	cause error,
) *Error {
	err := locatedError(CodeCaveatExpression, t.source, message, caveat.GetSourcePosition())
	err.Cause = cause
	return err
}

func sortedKeys[M ~map[string]V, V any](values M) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	slices.Sort(keys)
	return keys
}
