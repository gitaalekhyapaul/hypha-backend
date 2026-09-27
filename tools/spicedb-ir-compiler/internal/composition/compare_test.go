package composition

import (
	"bytes"
	"strings"
	"testing"
)

func TestValidateSchemaEquivalentIgnoresFormattingAndDocumentation(t *testing.T) {
	expected := familySource(versionA)
	actual := strings.ReplaceAll(expected, "  ", "\t")
	actual = strings.ReplaceAll(actual, " == ", "  ==  ")
	actual = strings.ReplaceAll(actual, "definition ", "/** Updated documentation. */\ndefinition ")
	actual = strings.ReplaceAll(actual, "relation direct:", "/** Updated relation documentation. */\n\trelation direct:")
	actual = strings.ReplaceAll(actual, "caveat caveat_", "/** Updated caveat documentation. */\ncaveat caveat_")
	actual = strings.Replace(actual, "definition agent {}", "definition agent {\n}", 1)
	if digest([]byte(expected)) == digest([]byte(actual)) {
		t.Fatal("test must use different source bytes")
	}
	if err := ValidateSchemaEquivalent([]byte(expected), []byte(actual)); err != nil {
		t.Fatal(err)
	}
}

func TestValidateSchemaEquivalentMatchesComposedNamespaces(t *testing.T) {
	first := packageFixture(t, versionA, false, familySource(versionA), familyConfig(t, versionA))
	second := packageFixture(t, versionB, false, familySource(versionB), familyConfig(t, versionB))
	result, err := Compose(Manifest{FormatVersion: 1, Packages: []PackageInput{first, second}}, "1.54.0")
	if err != nil {
		t.Fatal(err)
	}
	// ReadSchema can change top-level order without changing namespace ownership.
	actual := familySource(versionB) + strings.Replace(familySource(versionA), "definition agent {}", "", 1)
	if err := ValidateSchemaEquivalent(result.Schema, []byte(actual)); err != nil {
		t.Fatal(err)
	}
}

func TestValidateSchemaEquivalentRejectsSemanticChanges(t *testing.T) {
	expected := familySource(versionA)
	for name, actual := range map[string]string{
		"caveat expression":     strings.Replace(expected, "task_id == expected_task", "task_id != expected_task", 1),
		"caveat literal":        strings.Replace(expected, "task_id == expected_task", "task_id == \"expected_task\"", 1),
		"caveat parameter":      strings.Replace(expected, "expected_task string", "expected_task int", 1),
		"relation type":         strings.Replace(expected, "relation backup: agent", "relation backup: team_"+versionA+"#member", 1),
		"caveat binding":        strings.Replace(expected, "agent with caveat_"+versionA+"_0", "agent", 1),
		"permission expression": strings.Replace(expected, "direct & backup", "direct + backup", 1),
		"extra definition":      expected + "\ndefinition foreign {}\n",
		"renamed definition":    strings.ReplaceAll(expected, "team_"+versionA, "team_"+versionB),
		"renamed caveat":        strings.ReplaceAll(expected, "caveat_"+versionA+"_0", "caveat_"+versionB+"_0"),
		"changed namespace":     familySource(versionB),
		"missing definition":    strings.Replace(expected, "definition agent {}", "", 1),
	} {
		t.Run(name, func(t *testing.T) {
			if ValidateSchemaEquivalent([]byte(expected), []byte(actual)) == nil {
				t.Fatal("changed semantics or ownership accepted")
			}
			if ValidateSchemaEquivalent([]byte(actual), []byte(expected)) == nil {
				t.Fatal("changed expected semantics or ownership accepted")
			}
		})
	}
}

func TestValidateSchemaEquivalentRejectsDefinitionKindChanges(t *testing.T) {
	if ValidateSchemaEquivalent([]byte("definition same {}"), []byte("caveat same(value bool) { value }")) == nil {
		t.Fatal("definition kind change accepted")
	}
}

func TestValidateSchemaEquivalentRejectsUnsupportedOrInvalidInputs(t *testing.T) {
	for name, source := range map[string][]byte{
		"imports":              []byte("import \"private.zed\"\ndefinition agent {}"),
		"expiration flag":      []byte("use expiration\ndefinition agent {}"),
		"expiration relation":  []byte("use expiration\ndefinition agent {}\ndefinition dataset {\n relation reader: agent with expiration\n}"),
		"malformed":            []byte("private_invalid_schema_value"),
		"duplicate definition": []byte("definition agent {}\ndefinition agent {}"),
		"invalid utf8":         append([]byte("// "), 0xff),
		"oversized":            bytes.Repeat([]byte(" "), MaxSchemaBytes+1),
	} {
		t.Run(name, func(t *testing.T) {
			if ValidateSchemaEquivalent(source, source) == nil {
				t.Fatal("invalid identical inputs accepted")
			}
			if ValidateSchemaEquivalent([]byte("definition agent {}"), source) == nil {
				t.Fatal("invalid actual schema accepted")
			}
		})
	}
}
