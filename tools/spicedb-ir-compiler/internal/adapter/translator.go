package adapter

import (
	"cmp"
	"fmt"
	"slices"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	spicecompiler "github.com/authzed/spicedb/pkg/schemadsl/compiler"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
)

const (
	CompilerName             = "authzed/spicedb"
	SupportedCompilerVersion = "1.54.0"
	supportedIRVersion       = 1
)

type translator struct {
	source          string
	compiled        *spicecompiler.CompiledSchema
	config          config.Config
	objects         map[string]*core.NamespaceDefinition
	relations       map[string]map[string]*core.Relation
	compiledCaveats map[string]*core.CaveatDefinition
}

func newTranslator(
	source string,
	compiled *spicecompiler.CompiledSchema,
	compilerConfig config.Config,
) (*translator, error) {
	if compiled == nil {
		return nil, &Error{
			Code:    CodeInvalidCompiledSchema,
			Source:  source,
			Message: "SpiceDB compiler returned a nil schema",
		}
	}

	result := &translator{
		source:          source,
		compiled:        compiled,
		config:          compilerConfig,
		objects:         make(map[string]*core.NamespaceDefinition, len(compiled.ObjectDefinitions)),
		relations:       make(map[string]map[string]*core.Relation, len(compiled.ObjectDefinitions)),
		compiledCaveats: make(map[string]*core.CaveatDefinition, len(compiled.CaveatDefinitions)),
	}

	for _, object := range compiled.ObjectDefinitions {
		if object == nil || object.GetName() == "" {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				source,
				"compiled object definition has no name",
				positionOfObject(object),
			)
		}
		if err := validatePolicyIdentifier(
			source,
			"object type",
			object.GetName(),
			object.GetSourcePosition(),
		); err != nil {
			return nil, err
		}
		if _, exists := result.objects[object.GetName()]; exists {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				source,
				fmt.Sprintf("duplicate compiled object definition %q", object.GetName()),
				object.GetSourcePosition(),
			)
		}
		result.objects[object.GetName()] = object

		byName := make(map[string]*core.Relation, len(object.GetRelation()))
		for _, relation := range object.GetRelation() {
			if relation == nil || relation.GetName() == "" {
				return nil, locatedError(
					CodeInvalidCompiledSchema,
					source,
					fmt.Sprintf("object %q contains a relation or permission with no name", object.GetName()),
					positionOfRelation(relation),
				)
			}
			if err := validatePolicyIdentifier(
				source,
				"relation or permission",
				relation.GetName(),
				relation.GetSourcePosition(),
			); err != nil {
				return nil, err
			}
			if _, exists := byName[relation.GetName()]; exists {
				return nil, locatedError(
					CodeInvalidCompiledSchema,
					source,
					fmt.Sprintf("object %q contains duplicate member %q", object.GetName(), relation.GetName()),
					relation.GetSourcePosition(),
				)
			}
			if _, err := classifyMember(relation); err != nil {
				return nil, locatedError(
					CodeInvalidCompiledSchema,
					source,
					err.Error(),
					relation.GetSourcePosition(),
				)
			}
			byName[relation.GetName()] = relation
		}
		result.relations[object.GetName()] = byName
	}

	for _, caveat := range compiled.CaveatDefinitions {
		if caveat == nil || caveat.GetName() == "" {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				source,
				"compiled caveat definition has no name",
				positionOfCaveat(caveat),
			)
		}
		if err := validatePolicyIdentifier(
			source,
			"caveat",
			caveat.GetName(),
			caveat.GetSourcePosition(),
		); err != nil {
			return nil, err
		}
		for parameterName := range caveat.GetParameterTypes() {
			if err := validatePolicyIdentifier(
				source,
				"caveat parameter",
				parameterName,
				caveat.GetSourcePosition(),
			); err != nil {
				return nil, err
			}
		}
		if _, exists := result.compiledCaveats[caveat.GetName()]; exists {
			return nil, locatedError(
				CodeInvalidCompiledSchema,
				source,
				fmt.Sprintf("duplicate compiled caveat %q", caveat.GetName()),
				caveat.GetSourcePosition(),
			)
		}
		result.compiledCaveats[caveat.GetName()] = caveat
	}

	return result, nil
}

func (t *translator) translate(sourceSchemaDigest []byte) (*razielv1.PolicyProgram, error) {
	definitions, err := t.translateDefinitions()
	if err != nil {
		return nil, err
	}
	caveats, err := t.translateCaveats()
	if err != nil {
		return nil, err
	}
	permissions, err := t.translatePermissions()
	if err != nil {
		return nil, err
	}
	if err := t.validatePermissionGraph(permissions); err != nil {
		return nil, err
	}
	if len(definitions) == 0 {
		return nil, &Error{
			Code:    CodeInvalidCompiledSchema,
			Source:  t.source,
			Message: "PolicyIR requires at least one object definition",
		}
	}
	if len(permissions) == 0 {
		return nil, &Error{
			Code:    CodeInvalidCompiledSchema,
			Source:  t.source,
			Message: "PolicyIR requires at least one permission",
		}
	}

	program := &razielv1.PolicyProgram{
		IrVersion:          supportedIRVersion,
		CompilerName:       CompilerName,
		CompilerVersion:    SupportedCompilerVersion,
		SourceSchemaDigest: append([]byte(nil), sourceSchemaDigest...),
		Definitions:        definitions,
		Permissions:        permissions,
		Caveats:            caveats,
		Limits:             t.config.Limits(),
	}
	if err := validateDeclaredProgramLimits(t.source, program); err != nil {
		return nil, err
	}
	return program, nil
}

func (t *translator) translateDefinitions() ([]*razielv1.ObjectDefinition, error) {
	definitions := make([]*razielv1.ObjectDefinition, 0, len(t.compiled.ObjectDefinitions))
	for _, compiledObject := range t.compiled.ObjectDefinitions {
		definition := &razielv1.ObjectDefinition{Name: compiledObject.GetName()}
		for _, member := range compiledObject.GetRelation() {
			kind, _ := classifyMember(member)
			if kind != memberRelation {
				continue
			}
			relation, err := t.translateRelation(member)
			if err != nil {
				return nil, err
			}
			definition.Relations = append(definition.Relations, relation)
		}
		slices.SortFunc(definition.Relations, func(left, right *razielv1.RelationDefinition) int {
			return cmp.Compare(left.GetName(), right.GetName())
		})
		definitions = append(definitions, definition)
	}
	slices.SortFunc(definitions, func(left, right *razielv1.ObjectDefinition) int {
		return cmp.Compare(left.GetName(), right.GetName())
	})
	return definitions, nil
}

func (t *translator) translatePermissions() ([]*razielv1.PermissionDefinition, error) {
	permissions := make([]*razielv1.PermissionDefinition, 0)
	for _, object := range t.compiled.ObjectDefinitions {
		for _, member := range object.GetRelation() {
			kind, _ := classifyMember(member)
			if kind != memberPermission {
				continue
			}
			expression, err := t.translatePermission(
				object.GetName(),
				member,
				map[permissionKey]struct{}{},
			)
			if err != nil {
				return nil, err
			}
			permissions = append(permissions, &razielv1.PermissionDefinition{
				ObjectType: object.GetName(),
				Name:       member.GetName(),
				Expression: expression,
			})
		}
	}
	slices.SortFunc(permissions, func(left, right *razielv1.PermissionDefinition) int {
		if order := cmp.Compare(left.GetObjectType(), right.GetObjectType()); order != 0 {
			return order
		}
		return cmp.Compare(left.GetName(), right.GetName())
	})
	return permissions, nil
}

type memberKind uint8

const (
	memberRelation memberKind = iota + 1
	memberPermission
)

func classifyMember(member *core.Relation) (memberKind, error) {
	hasTypes := member.GetTypeInformation() != nil
	hasRewrite := member.GetUsersetRewrite() != nil
	switch {
	case hasTypes && !hasRewrite:
		return memberRelation, nil
	case !hasTypes && hasRewrite:
		return memberPermission, nil
	default:
		return 0, fmt.Errorf(
			"compiled member %q must contain exactly one of type information or userset rewrite",
			member.GetName(),
		)
	}
}

func positionOfObject(object *core.NamespaceDefinition) *core.SourcePosition {
	if object == nil {
		return nil
	}
	return object.GetSourcePosition()
}

func positionOfRelation(relation *core.Relation) *core.SourcePosition {
	if relation == nil {
		return nil
	}
	return relation.GetSourcePosition()
}

func positionOfCaveat(caveat *core.CaveatDefinition) *core.SourcePosition {
	if caveat == nil {
		return nil
	}
	return caveat.GetSourcePosition()
}
