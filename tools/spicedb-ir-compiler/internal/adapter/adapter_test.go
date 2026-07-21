package adapter

import (
	"bytes"
	"encoding/hex"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"testing"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

func TestTranslateSupportedSchema(t *testing.T) {
	program := translateFixture(t, "supported.zed", "supported.json")

	if got, want := program.GetIrVersion(), uint32(1); got != want {
		t.Errorf("ir_version = %d, want %d", got, want)
	}
	if got, want := program.GetCompilerName(), CompilerName; got != want {
		t.Errorf("compiler_name = %q, want %q", got, want)
	}
	if got, want := program.GetCompilerVersion(), SupportedCompilerVersion; got != want {
		t.Errorf("compiler_version = %q, want %q", got, want)
	}
	wantDigest := sourceSchemaDigest(readFixture(t, "supported.zed"))
	if !bytes.Equal(program.GetSourceSchemaDigest(), wantDigest[:]) {
		t.Errorf("source_schema_digest = %x, want %x", program.GetSourceSchemaDigest(), wantDigest)
	}

	definitionNames := make([]string, 0, len(program.GetDefinitions()))
	for _, definition := range program.GetDefinitions() {
		definitionNames = append(definitionNames, definition.GetName())
	}
	if want := []string{"organization", "payment_batch", "team", "user"}; !slices.Equal(definitionNames, want) {
		t.Errorf("definition names = %v, want %v", definitionNames, want)
	}

	view := findPermission(t, program, "payment_batch", "view")
	if view.GetExpression().GetAllOf() == nil {
		t.Fatalf("payment_batch#view expression = %T, want all_of", view.GetExpression().GetNode())
	}
	inspect := findPermission(t, program, "payment_batch", "inspect")
	if !proto.Equal(view.GetExpression(), inspect.GetExpression()) {
		t.Error("permission alias did not expand to the referenced permission expression")
	}
	audit := findPermission(t, program, "payment_batch", "audit")
	if got := audit.GetExpression().GetArrow(); got == nil ||
		got.GetSourceRelation() != "organization" || got.GetTargetPermission() != "audit" {
		t.Errorf("payment_batch#audit expression = %v, want organization->audit", audit.GetExpression())
	}
	seenPermissionRanks := map[int]bool{}
	for _, permission := range program.GetPermissions() {
		collectPermissionRanks(permission.GetExpression(), seenPermissionRanks)
	}
	for rank := 0; rank <= 5; rank++ {
		if !seenPermissionRanks[rank] {
			t.Errorf("supported fixture did not produce permission expression rank %d", rank)
		}
	}

	if got, want := len(program.GetCaveats()), 3; got != want {
		t.Fatalf("caveat count = %d, want %d", got, want)
	}
	caveat := findCaveat(t, program, "reconcile_scope")
	parameterNames := make([]string, 0, len(caveat.GetParameters()))
	parameterSources := make([]razielv1.CaveatInputSource, 0, len(caveat.GetParameters()))
	for _, parameter := range caveat.GetParameters() {
		parameterNames = append(parameterNames, parameter.GetName())
		parameterSources = append(parameterSources, parameter.GetSource())
	}
	if want := []string{"requested_amount", "approved_amount", "task_id"}; !slices.Equal(parameterNames, want) {
		t.Errorf("caveat parameter names = %v, want %v", parameterNames, want)
	}
	if want := []razielv1.CaveatInputSource{
		razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_REQUEST_CONTEXT,
		razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_RELATIONSHIP_CONTEXT,
		razielv1.CaveatInputSource_CAVEAT_INPUT_SOURCE_CAPABILITY_CLAIM,
	}; !slices.Equal(parameterSources, want) {
		t.Errorf("caveat parameter sources = %v, want %v", parameterSources, want)
	}
	if caveat.GetExpression().GetAnd() == nil {
		t.Errorf("caveat expression = %T, want and", caveat.GetExpression().GetNode())
	}
	override := findCaveat(t, program, "reconcile_override")
	if override.GetExpression().GetOr() == nil {
		t.Errorf("override caveat expression = %T, want or", override.GetExpression().GetNode())
	}

	seenRanks := map[int]bool{}
	for _, definition := range program.GetCaveats() {
		collectCaveatRanks(definition.GetExpression(), seenRanks)
	}
	for rank := 0; rank <= 12; rank++ {
		if !seenRanks[rank] {
			t.Errorf("supported fixture did not produce caveat expression rank %d", rank)
		}
	}
}

func TestSourceSchemaDigestUsesExactBytes(t *testing.T) {
	digest := sourceSchemaDigest([]byte("abc"))
	if got, want := hex.EncodeToString(digest[:]),
		"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"; got != want {
		t.Errorf("sourceSchemaDigest(abc) = %s, want %s", got, want)
	}

	withNewline := sourceSchemaDigest([]byte("abc\n"))
	if bytes.Equal(digest[:], withNewline[:]) {
		t.Error("adding one newline did not change source schema digest")
	}
	if repeated := sourceSchemaDigest([]byte("abc")); repeated != digest {
		t.Errorf("repeated source digest = %x, want %x", repeated, digest)
	}
}

func TestCompileRejectsCompilerVersionMismatch(t *testing.T) {
	_, err := Compile(CompileRequest{
		SourceName:      "testdata/supported.zed",
		Schema:          readFixture(t, "supported.zed"),
		CompilerVersion: "1.53.0",
	})
	var typed *Error
	if !errors.As(err, &typed) {
		t.Fatalf("Compile() error = %v, want *adapter.Error", err)
	}
	if typed.Code != CodeCompilerVersion {
		t.Errorf("Compile() error code = %s, want %s", typed.Code, CodeCompilerVersion)
	}
}

func TestTranslationIsDeterministic(t *testing.T) {
	first := translateFixture(t, "supported.zed", "supported.json")
	second := translateFixture(t, "supported.zed", "supported.json")

	options := proto.MarshalOptions{Deterministic: true}
	firstBytes, err := options.Marshal(first)
	if err != nil {
		t.Fatalf("Marshal(first) error = %v", err)
	}
	secondBytes, err := options.Marshal(second)
	if err != nil {
		t.Fatalf("Marshal(second) error = %v", err)
	}
	if !bytes.Equal(firstBytes, secondBytes) {
		t.Error("two translations produced different deterministic Protobuf output")
	}
}

func TestTranslationMatchesGoldenPolicyIR(t *testing.T) {
	program := translateFixture(t, "supported.zed", "supported.json")
	binary, err := (proto.MarshalOptions{Deterministic: true}).Marshal(program)
	if err != nil {
		t.Fatalf("proto.Marshal() error = %v", err)
	}
	if want := readFixture(t, "supported.policy.pb"); !bytes.Equal(binary, want) {
		t.Error("compiled PolicyIR Protobuf does not match the reviewed golden")
	}

	var reviewed razielv1.PolicyProgram
	if err := protojson.Unmarshal(readFixture(t, "supported.policy.json"), &reviewed); err != nil {
		t.Fatalf("protojson.Unmarshal(golden) error = %v", err)
	}
	if !proto.Equal(program, &reviewed) {
		t.Error("compiled PolicyIR message does not match the reviewed golden message")
	}
}

func TestTranslateRejectsUnsupportedSchema(t *testing.T) {
	tests := []struct {
		name   string
		schema string
		config string
		code   ErrorCode
	}{
		{name: "exclusion", schema: "exclusion.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "wildcard", schema: "wildcard.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "caveated userset", schema: "caveated-userset.zed", config: "caveated-userset.json", code: CodeUnsupportedConstruct},
		{name: "ambiguous arrow", schema: "ambiguous-arrow.zed", config: "empty-caveats.json", code: CodeAmbiguousReference},
		{name: "functioned arrow", schema: "functioned-arrow.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "unsupported CEL", schema: "unsupported-cel.zed", config: "unsupported-cel.json", code: CodeCaveatExpression},
		{name: "duplicate union", schema: "duplicate-union.zed", config: "empty-caveats.json", code: CodeInvalidCompiledSchema},
		{name: "arrow depth", schema: "arrow-depth.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "recursive permission", schema: "recursive.zed", config: "empty-caveats.json", code: CodePermissionCycle},
		{name: "prefixed object", schema: "prefixed-object.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "nested subject target", schema: "nested-subject-target.zed", config: "empty-caveats.json", code: CodeUnsupportedConstruct},
		{name: "caveated subject target", schema: "caveated-subject-target.zed", config: "caveated-subject-target.json", code: CodeUnsupportedConstruct},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := translateFixtureError(t, test.schema, test.config)
			if err == nil {
				t.Fatal("translation error = nil")
			}
			var typed *Error
			if !errors.As(err, &typed) {
				t.Fatalf("translation error type = %T, want *adapter.Error", err)
			}
			if typed.Code != test.code {
				t.Fatalf("translation error code = %s, want %s; error: %v", typed.Code, test.code, err)
			}
			if typed.Source == "" {
				t.Error("translation error has no source")
			}
			if typed.Line == 0 || typed.Column == 0 {
				t.Errorf("translation error location = %d:%d, want one-based line and column", typed.Line, typed.Column)
			}
		})
	}
}

func TestTranslateRejectsBindingMismatch(t *testing.T) {
	schema := readFixture(t, "supported.zed")
	baseConfig := string(readFixture(t, "supported.json"))
	tests := []struct {
		name   string
		config string
	}{
		{
			name: "missing parameter",
			config: string(bytes.Replace(
				[]byte(baseConfig),
				[]byte(`      "requested_amount": "REQUEST_CONTEXT"`),
				[]byte(`      "not_requested_amount": "REQUEST_CONTEXT"`),
				1,
			)),
		},
		{
			name: "extra caveat",
			config: string(bytes.Replace(
				[]byte(baseConfig),
				[]byte(`  "caveats": {`),
				[]byte(`  "caveats": {"unknown": {},`),
				1,
			)),
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			compilerConfig, err := config.Parse([]byte(test.config))
			if err != nil {
				t.Fatalf("config.Parse() error = %v", err)
			}
			_, err = Compile(CompileRequest{
				SourceName:      "testdata/supported.zed",
				Schema:          schema,
				CompilerVersion: SupportedCompilerVersion,
				Config:          compilerConfig,
			})
			var typed *Error
			if !errors.As(err, &typed) || typed.Code != CodeCaveatBinding {
				t.Fatalf("translation error = %v, want %s", err, CodeCaveatBinding)
			}
		})
	}
}

func TestTranslateEnforcesCompileTimeLimits(t *testing.T) {
	schema := readFixture(t, "supported.zed")
	baseConfig := readFixture(t, "supported.json")
	tests := []struct {
		name         string
		oldValue     string
		limitedValue string
	}{
		{
			name:         "permission expression nodes",
			oldValue:     `"max_expression_nodes": 64`,
			limitedValue: `"max_expression_nodes": 1`,
		},
		{
			name:         "caveat expression nodes",
			oldValue:     `"max_caveat_nodes": 64`,
			limitedValue: `"max_caveat_nodes": 1`,
		},
		{
			name:         "string bytes",
			oldValue:     `"max_string_bytes": 256`,
			limitedValue: `"max_string_bytes": 1`,
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			limitedConfig := bytes.Replace(
				baseConfig,
				[]byte(test.oldValue),
				[]byte(test.limitedValue),
				1,
			)
			compilerConfig, err := config.Parse(limitedConfig)
			if err != nil {
				t.Fatalf("config.Parse() error = %v", err)
			}
			_, err = Compile(CompileRequest{
				SourceName:      "testdata/supported.zed",
				Schema:          schema,
				CompilerVersion: SupportedCompilerVersion,
				Config:          compilerConfig,
			})
			var typed *Error
			if !errors.As(err, &typed) || typed.Code != CodeProgramLimit {
				t.Fatalf("translation error = %v, want %s", err, CodeProgramLimit)
			}
		})
	}
}

func translateFixture(t *testing.T, schemaName, configName string) *razielv1.PolicyProgram {
	t.Helper()
	program, err := translateFixtureError(t, schemaName, configName)
	if err != nil {
		t.Fatalf("translate fixture %q error = %v", schemaName, err)
	}
	return program
}

func translateFixtureError(
	t *testing.T,
	schemaName, configName string,
) (*razielv1.PolicyProgram, error) {
	t.Helper()
	schema := readFixture(t, schemaName)
	compilerConfig, err := config.Parse(readFixture(t, configName))
	if err != nil {
		t.Fatalf("config.Parse(%q) error = %v", configName, err)
	}
	source := filepath.Join("testdata", schemaName)
	return Compile(CompileRequest{
		SourceName:      source,
		Schema:          schema,
		CompilerVersion: SupportedCompilerVersion,
		Config:          compilerConfig,
	})
}

func readFixture(t *testing.T, name string) []byte {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", name))
	if err != nil {
		t.Fatalf("ReadFile(%q) error = %v", name, err)
	}
	return data
}

func findPermission(
	t *testing.T,
	program *razielv1.PolicyProgram,
	objectType, name string,
) *razielv1.PermissionDefinition {
	t.Helper()
	for _, permission := range program.GetPermissions() {
		if permission.GetObjectType() == objectType && permission.GetName() == name {
			return permission
		}
	}
	t.Fatalf("permission %s#%s not found", objectType, name)
	return nil
}

func findCaveat(
	t *testing.T,
	program *razielv1.PolicyProgram,
	name string,
) *razielv1.CaveatDefinition {
	t.Helper()
	for _, caveat := range program.GetCaveats() {
		if caveat.GetName() == name {
			return caveat
		}
	}
	t.Fatalf("caveat %s not found", name)
	return nil
}

func collectCaveatRanks(expression *razielv1.CaveatExpr, seen map[int]bool) {
	if expression == nil {
		return
	}
	seen[caveatRank(expression)] = true
	for _, binary := range []*razielv1.CaveatBinaryExpr{
		expression.GetEqual(),
		expression.GetNotEqual(),
		expression.GetLessThan(),
		expression.GetLessThanOrEqual(),
		expression.GetGreaterThan(),
		expression.GetGreaterThanOrEqual(),
	} {
		if binary != nil {
			collectCaveatRanks(binary.GetLeft(), seen)
			collectCaveatRanks(binary.GetRight(), seen)
			return
		}
	}
	for _, children := range []*razielv1.CaveatChildren{expression.GetAnd(), expression.GetOr()} {
		if children != nil {
			for _, child := range children.GetChildren() {
				collectCaveatRanks(child, seen)
			}
			return
		}
	}
	if unary := expression.GetNot(); unary != nil {
		collectCaveatRanks(unary.GetChild(), seen)
	}
}

func collectPermissionRanks(expression *razielv1.PermissionExpr, seen map[int]bool) {
	if expression == nil {
		return
	}
	seen[permissionRank(expression)] = true
	for _, children := range []*razielv1.PermissionChildren{
		expression.GetAnyOf(),
		expression.GetAllOf(),
	} {
		if children != nil {
			for _, child := range children.GetChildren() {
				collectPermissionRanks(child, seen)
			}
		}
	}
}
