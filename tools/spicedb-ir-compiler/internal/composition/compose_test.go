package composition

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/adapter"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
	"google.golang.org/protobuf/encoding/protowire"
	"google.golang.org/protobuf/proto"
)

const versionA = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
const versionB = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

func TestComposeIndependentNamespacesAndSharedAgent(t *testing.T) {
	first := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	second := packageFixture(t, versionB, false, familySource(versionB), familyConfig(t, versionB))
	manifest := Manifest{FormatVersion: 1, Packages: []PackageInput{first, second}}
	result, err := Compose(manifest, adapter.SupportedCompilerVersion)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Count(string(result.Schema), "definition agent") != 1 {
		t.Fatal("shared agent must occur once")
	}
	var reflection Reflection
	if err := json.Unmarshal(result.Reflection, &reflection); err != nil {
		t.Fatal(err)
	}
	if len(reflection.Packages) != 2 || len(reflection.Definitions) != 9 || reflection.SchemaSHA256 != digest(result.Schema) {
		t.Fatal("wrong composition inventory")
	}
	for i, entry := range manifest.Packages {
		if reflection.Packages[i].SourceSHA256 != digest(readTest(t, entry.SchemaPath)) {
			t.Fatal("package digest changed to aggregate digest")
		}
	}
	if bytes.Contains(result.Reflection, []byte("source_position")) || bytes.Contains(result.Reflection, []byte("doc_comment")) {
		t.Fatal("nonsemantic source metadata retained")
	}
	manifest.Packages[0], manifest.Packages[1] = manifest.Packages[1], manifest.Packages[0]
	other, err := Compose(manifest, adapter.SupportedCompilerVersion)
	if err != nil || !bytes.Equal(result.Schema, other.Schema) || !bytes.Equal(result.Reflection, other.Reflection) {
		t.Fatalf("composition is not deterministic: %v", err)
	}
}

func TestComposeLegacyExactSource(t *testing.T) {
	legacy := packageFixture(t, "", true, string(readTest(t, "../adapter/testdata/supported.zed")), readTest(t, "../adapter/testdata/supported.json"))
	newPackage := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	manifest := Manifest{FormatVersion: 1, Packages: []PackageInput{newPackage, legacy}}
	if _, err := Compose(manifest, adapter.SupportedCompilerVersion); err != nil {
		t.Fatal(err)
	}
	// Equivalent source is not the historical exact source covered by PolicyIR.
	writeTest(t, legacy.SchemaPath, append(readTest(t, legacy.SchemaPath), '\n'))
	if _, err := Compose(manifest, adapter.SupportedCompilerVersion); !errors.Is(err, ErrPolicy) {
		t.Fatalf("changed legacy source accepted: %v", err)
	}
}

func TestComposeRejectsDuplicateOwnership(t *testing.T) {
	entry := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	legacy := entry
	legacy.Legacy, legacy.VersionID = true, ""
	for _, entries := range [][]PackageInput{{entry, entry}, {entry, legacy}} {
		if _, err := Compose(Manifest{FormatVersion: 1, Packages: entries}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrOwnership) {
			t.Fatalf("duplicate owner accepted: %v", err)
		}
	}
}

func TestComposeRejectsForeignNamespaceAndNonemptyAgent(t *testing.T) {
	for name, source := range map[string]string{
		"foreign definition": familySource(versionB),
		"foreign reference":  strings.ReplaceAll(familySource(versionA), "team_"+versionA, "team_"+versionB),
		"foreign caveat":     strings.ReplaceAll(familySource(versionA), "caveat_"+versionA+"_0", "caveat_"+versionB+"_0"),
		"nonempty agent":     strings.Replace(familySource(versionA), "definition agent {}", "definition agent {\n relation self: agent\n}", 1),
	} {
		t.Run(name, func(t *testing.T) {
			configID := versionA
			if name == "foreign definition" || name == "foreign caveat" {
				configID = versionB
			}
			entry := packageFixture(t, versionA, false, source, familyConfig(t, configID))
			if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrOwnership) {
				t.Fatalf("ownership violation accepted: %v", err)
			}
		})
	}
}

func TestComposeRejectsChangedExpectedPolicyAndConfig(t *testing.T) {
	for _, mutation := range []string{"policy", "config", "source", "unknown", "duplicate", "nested unknown", "malformed"} {
		t.Run(mutation, func(t *testing.T) {
			entry := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
			data := readTest(t, entry.ExpectedPolicyIRPath)
			switch mutation {
			case "policy", "nested unknown":
				var expected razielv1.PolicyProgram
				if err := proto.Unmarshal(data, &expected); err != nil {
					t.Fatal(err)
				}
				if mutation == "policy" {
					expected.Definitions[0].Name = "changed"
				} else {
					expected.Definitions[0].ProtoReflect().SetUnknown([]byte{0xa0, 0x06, 0x01})
				}
				data, _ = proto.Marshal(&expected)
			case "config":
				writeTest(t, entry.ConfigPath, bytes.ReplaceAll(readTest(t, entry.ConfigPath), []byte("REQUEST_CONTEXT"), []byte("RELATIONSHIP_CONTEXT")))
			case "source":
				writeTest(t, entry.SchemaPath, append(readTest(t, entry.SchemaPath), '\n'))
			case "unknown":
				data = append(data, 0xa0, 0x06, 0x01)
			case "duplicate":
				data = append(data, 0x08, 0x01)
			case "malformed":
				data = []byte{0xff}
			}
			writeTest(t, entry.ExpectedPolicyIRPath, data)
			if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrPolicy) {
				t.Fatalf("mutation accepted: %v", err)
			}
		})
	}
}

func TestComposeRejectsUnsupportedSchema(t *testing.T) {
	entry := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	source := strings.Replace(familySource(versionA), "permission read = direct + backup", "permission read = direct - backup", 1)
	writeTest(t, entry.SchemaPath, []byte(source))
	if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrPolicy) {
		t.Fatalf("unsupported exclusion accepted: %v", err)
	}
}

func TestComposePreservesUTF8AndLiteralDelimiters(t *testing.T) {
	source := strings.Replace(familySource(versionA), "task_id == expected_task", "task_id == \"} caf\u00e9 {\" && task_id != expected_task", 1)
	source = "// Unicode caf\u00e9 and fake definition agent {}\n" + source
	entry := packageFixture(t, versionA, false, source, familyConfig(t, versionA))
	result, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, adapter.SupportedCompilerVersion)
	if err != nil || !bytes.Contains(result.Schema, []byte("} caf\u00e9 {")) {
		t.Fatalf("UTF8 source changed: %v", err)
	}
}

func TestComposePackageCountAndFileBounds(t *testing.T) {
	entry := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	for name, bound := range map[string]int{"schema": MaxSchemaBytes, "config": MaxConfigBytes, "policy": MaxPolicyIRBytes} {
		t.Run(name, func(t *testing.T) {
			copy := entry
			path := filepath.Join(t.TempDir(), "oversized")
			writeTest(t, path, bytes.Repeat([]byte("x"), bound+1))
			switch name {
			case "schema":
				copy.SchemaPath = path
			case "config":
				copy.ConfigPath = path
			case "policy":
				copy.ExpectedPolicyIRPath = path
			}
			if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{copy}}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrLimit) {
				t.Fatalf("file bound not enforced: %v", err)
			}
		})
	}
	entries := make([]PackageInput, 33)
	for i := range entries {
		entries[i] = entry
		entries[i].VersionID = fmt.Sprintf("%032x", i)
	}
	if _, err := Compose(Manifest{FormatVersion: 1, Packages: entries}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrLimit) {
		t.Fatalf("33 new packages accepted: %v", err)
	}
	if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, "1.53.0"); !errors.Is(err, ErrPolicy) {
		t.Fatalf("unpinned compiler accepted: %v", err)
	}
}

func TestComposeRetainsPerPolicyGuestCeilings(t *testing.T) {
	configBytes := familyConfig(t, versionA)
	var document map[string]any
	if err := json.Unmarshal(configBytes, &document); err != nil {
		t.Fatal(err)
	}
	document["limits"].(map[string]any)["max_total_private_input_bytes"] = 4097
	configBytes, _ = json.Marshal(document)
	entry := packageFixture(t, versionA, false, familySource(versionA), configBytes)
	if _, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{entry}}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrLimit) {
		t.Fatalf("widened guest limit accepted: %v", err)
	}
}

func TestComposeRejectsOversizedAggregate(t *testing.T) {
	entries := []PackageInput{}
	for _, id := range []string{versionA, versionB} {
		source := strings.Replace(familySource(id), "  relation direct:", "  /*"+strings.Repeat("x", 600*1024)+"*/\n  relation direct:", 1)
		entries = append(entries, packageFixture(t, id, false, source, familyConfig(t, id)))
	}
	if _, err := Compose(Manifest{FormatVersion: 1, Packages: entries}, adapter.SupportedCompilerVersion); !errors.Is(err, ErrLimit) {
		t.Fatalf("oversized aggregate accepted: %v", err)
	}
}

func TestNormalizedDefinitionKeepsExpressions(t *testing.T) {
	compiled, err := compileSchema([]byte(familySource(versionA)))
	if err != nil {
		t.Fatal(err)
	}
	definition := compiled.ObjectDefinitions[len(compiled.ObjectDefinitions)-1]
	first, err := normalizedDefinition(definition)
	if err != nil {
		t.Fatal(err)
	}
	changed := proto.Clone(definition).(*core.NamespaceDefinition)
	changed.Relation[0].Name = "changed"
	second, err := normalizedDefinition(changed)
	if err != nil || proto.Equal(first, second) {
		t.Fatal("semantic mutation was stripped")
	}
}

func TestStrictPolicyWireRejectsConflictingOneof(t *testing.T) {
	expression := &razielv1.PermissionExpr{}
	first := protowire.AppendTag(nil, 1, protowire.BytesType)
	first = protowire.AppendBytes(first, nil)
	second := protowire.AppendTag(first, 2, protowire.BytesType)
	second = protowire.AppendBytes(second, nil)
	if strictWire(second, expression.ProtoReflect().Descriptor(), 0) == nil {
		t.Fatal("duplicate oneof accepted")
	}
}

func familySource(id string) string {
	return fmt.Sprintf(`// A private package comment.
definition agent {}
caveat caveat_%[1]s_0(task_id string, expected_task string) {
  task_id == expected_task
}
definition team_%[1]s {
  relation member: agent
  permission read = member
}
definition organization_%[1]s {
  relation member: agent
  permission read = member
}
definition dataset_%[1]s {
  relation direct: agent with caveat_%[1]s_0
  relation backup: agent
  relation team: team_%[1]s
  relation organization: organization_%[1]s
  permission read = direct + backup + team->read + organization->read
  permission restricted = direct & backup
}
`, id)
}

func familyConfig(t *testing.T, id string) []byte {
	t.Helper()
	var document map[string]any
	if err := json.Unmarshal(readTest(t, "../adapter/testdata/empty-caveats.json"), &document); err != nil {
		t.Fatal(err)
	}
	document["caveats"] = map[string]any{"caveat_" + id + "_0": map[string]string{"task_id": "REQUEST_CONTEXT", "expected_task": "RELATIONSHIP_CONTEXT"}}
	data, err := json.Marshal(document)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func packageFixture(t *testing.T, id string, legacy bool, source string, configBytes []byte) PackageInput {
	t.Helper()
	dir := t.TempDir()
	entry := PackageInput{VersionID: id, Legacy: legacy, SchemaPath: filepath.Join(dir, "schema.zed"), ConfigPath: filepath.Join(dir, "config.json"), ExpectedPolicyIRPath: filepath.Join(dir, "policy.pb")}
	compilerConfig, err := config.Parse(configBytes)
	if err != nil {
		t.Fatal(err)
	}
	policy, err := adapter.Compile(adapter.CompileRequest{SourceName: "fixture", Schema: []byte(source), CompilerVersion: adapter.SupportedCompilerVersion, Config: compilerConfig})
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := proto.Marshal(policy)
	if err != nil {
		t.Fatal(err)
	}
	writeTest(t, entry.SchemaPath, []byte(source))
	writeTest(t, entry.ConfigPath, configBytes)
	writeTest(t, entry.ExpectedPolicyIRPath, encoded)
	return entry
}

func readTest(t *testing.T, path string) []byte {
	t.Helper()
	value, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func writeTest(t *testing.T, path string, data []byte) {
	t.Helper()
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}
