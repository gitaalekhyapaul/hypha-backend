package composition

import (
	"bytes"
	"encoding/json"
	"slices"
	"sort"
	"strings"
	"unicode/utf8"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	"github.com/authzed/spicedb/pkg/schemadsl/compiler"
	"github.com/authzed/spicedb/pkg/schemadsl/input"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/adapter"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
	"google.golang.org/protobuf/proto"
)

type Result struct {
	Schema     []byte
	Reflection []byte
}

// Compose checks each exact source/config against expected binary PolicyIR, then
// recompiles the aggregate and compares its compiled structures with the union.
// It performs no network operations and makes no signed-trust decision.
func Compose(manifest Manifest, compilerVersion string) (Result, error) {
	if err := manifest.validate(); err != nil {
		return Result{}, err
	}
	if compilerVersion != adapter.SupportedCompilerVersion {
		return Result{}, ErrPolicy
	}
	entries := slices.Clone(manifest.Packages)
	sort.Slice(entries, func(i, j int) bool { return entries[i].VersionID < entries[j].VersionID })
	union := map[string]proto.Message{}
	sources := map[string][]byte{}
	reflection := Reflection{FormatVersion: 1, CompilerVersion: compilerVersion}
	for _, entry := range entries {
		compiled, packageSources, evidence, err := compilePackage(entry, compilerVersion)
		if err != nil {
			return Result{}, err
		}
		for _, definition := range compiled.OrderedDefinitions {
			name := definition.GetName()
			normalized, err := normalizedDefinition(definition)
			if err != nil {
				return Result{}, err
			}
			if name == "agent" && !proto.Equal(normalized, &core.NamespaceDefinition{Name: "agent"}) {
				return Result{}, ErrOwnership
			}
			if previous, exists := union[name]; exists {
				if name != "agent" || !proto.Equal(previous, normalized) {
					return Result{}, ErrOwnership
				}
				continue
			}
			union[name] = normalized
			sources[name] = packageSources[name]
		}
		reflection.Packages = append(reflection.Packages, evidence)
	}
	names := make([]string, 0, len(union))
	for name := range union {
		names = append(names, name)
	}
	sort.Strings(names)
	var schema bytes.Buffer
	for _, name := range names {
		if schema.Len()+len(sources[name])+2 > MaxSchemaBytes {
			return Result{}, ErrLimit
		}
		schema.Write(sources[name])
		schema.WriteString("\n\n")
	}
	aggregate, err := compileSchema(schema.Bytes())
	if err != nil || len(aggregate.OrderedDefinitions) != len(union) {
		return Result{}, ErrComposition
	}
	for _, definition := range aggregate.OrderedDefinitions {
		normalized, err := normalizedDefinition(definition)
		if err != nil || !proto.Equal(normalized, union[definition.GetName()]) {
			return Result{}, ErrComposition
		}
		item, err := reflectDefinition(definition, normalized)
		if err != nil {
			return Result{}, err
		}
		reflection.Definitions = append(reflection.Definitions, item)
	}
	reflection.SchemaSHA256 = digest(schema.Bytes())
	encoded, err := json.Marshal(reflection)
	if err != nil {
		return Result{}, ErrComposition
	}
	encoded = append(encoded, '\n')
	if len(encoded) > MaxReflectionBytes || schema.Len()+len(encoded) > MaxTransportBytes {
		return Result{}, ErrLimit
	}
	return Result{Schema: schema.Bytes(), Reflection: encoded}, nil
}

func compilePackage(entry PackageInput, version string) (*compiler.CompiledSchema, map[string][]byte, PackageReflection, error) {
	fail := func(err error) (*compiler.CompiledSchema, map[string][]byte, PackageReflection, error) {
		return nil, nil, PackageReflection{}, err
	}
	schema, err := readBounded(entry.SchemaPath, MaxSchemaBytes)
	if err != nil {
		return fail(err)
	}
	configBytes, err := readBounded(entry.ConfigPath, MaxConfigBytes)
	if err != nil {
		return fail(err)
	}
	expectedBytes, err := readBounded(entry.ExpectedPolicyIRPath, MaxPolicyIRBytes)
	if err != nil {
		return fail(err)
	}
	if !utf8.Valid(schema) || !utf8.Valid(configBytes) || len(schema)+len(configBytes)+len(expectedBytes) > MaxTransportBytes {
		return fail(ErrLimit)
	}
	compilerConfig, err := config.Parse(configBytes)
	if err != nil {
		return fail(ErrPolicy)
	}
	expected, err := decodePolicy(expectedBytes)
	if err != nil {
		return fail(err)
	}
	actual, err := adapter.Compile(adapter.CompileRequest{
		SourceName: "package", Schema: schema, CompilerVersion: version, Config: compilerConfig,
	})
	if err != nil || !proto.Equal(expected, actual) {
		return fail(ErrPolicy)
	}
	if err := checkPolicyCeilings(actual); err != nil {
		return fail(err)
	}
	compiled, err := compileSchema(schema)
	if err != nil {
		return fail(ErrPolicy)
	}
	sources, err := definitionSources(schema)
	if err != nil || len(sources) != len(compiled.OrderedDefinitions) {
		return fail(ErrPolicy)
	}
	evidence := PackageReflection{
		VersionID: entry.VersionID, Legacy: entry.Legacy, SourceSHA256: digest(schema),
		ConfigSHA256: digest(configBytes), ExpectedPolicyIRSHA256: digest(expectedBytes),
		Definitions: []string{}, Caveats: []string{},
	}
	for _, definition := range compiled.ObjectDefinitions {
		name := definition.GetName()
		if !entry.Legacy && name != "agent" && name != "dataset_"+entry.VersionID &&
			name != "team_"+entry.VersionID && name != "organization_"+entry.VersionID {
			return fail(ErrOwnership)
		}
		evidence.Definitions = append(evidence.Definitions, name)
	}
	for _, caveat := range compiled.CaveatDefinitions {
		if !entry.Legacy && !validCaveatName(caveat.GetName(), entry.VersionID) {
			return fail(ErrOwnership)
		}
		evidence.Caveats = append(evidence.Caveats, caveat.GetName())
	}
	sort.Strings(evidence.Definitions)
	sort.Strings(evidence.Caveats)
	if !entry.Legacy && (!slices.Contains(evidence.Definitions, "agent") || !slices.Contains(evidence.Definitions, "dataset_"+entry.VersionID)) {
		return fail(ErrOwnership)
	}
	return compiled, sources, evidence, nil
}

func validCaveatName(name, versionID string) bool {
	index, ok := strings.CutPrefix(name, "caveat_"+versionID+"_")
	if !ok || index == "" || len(name) > 63 || len(index) > 1 && index[0] == '0' {
		return false
	}
	for _, char := range index {
		if char < '0' || char > '9' {
			return false
		}
	}
	return true
}

func compileSchema(source []byte) (*compiler.CompiledSchema, error) {
	return compiler.Compile(
		compiler.InputSchema{Source: input.Source("package"), SchemaString: string(source)},
		compiler.AllowUnprefixedObjectType(), compiler.DisallowExpirationFlag(), compiler.DisallowImportFlag(),
	)
}
