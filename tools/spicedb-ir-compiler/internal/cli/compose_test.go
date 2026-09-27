package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/composition"
)

func TestRunComposePrivateOutputs(t *testing.T) {
	manifest, dir := composeManifestFixture(t)
	output := filepath.Join(dir, "composed.zed")
	reflection := filepath.Join(dir, "reflection.json")
	args := composeArgs(manifest, output, reflection)
	var stdout, stderr bytes.Buffer
	if code := Run(args, &stdout, &stderr); code != exitOK {
		t.Fatalf("compose exit=%d stderr=%s", code, stderr.String())
	}
	if stdout.String() != "composition=validated\n" || stderr.Len() != 0 {
		t.Fatal("unexpected public output")
	}
	assertFileMode(t, output, 0o600)
	assertFileMode(t, reflection, 0o600)
	first := readFile(t, output)
	firstReflection := readFile(t, reflection)
	if len(first) == 0 || len(first) > composition.MaxSchemaBytes || len(firstReflection) > composition.MaxReflectionBytes || len(first)+len(firstReflection) > composition.MaxTransportBytes {
		t.Fatal("output bounds violated")
	}
	if code := Run(args, &stdout, &stderr); code != exitOK {
		t.Fatal("repeat compose failed")
	}
	if !bytes.Equal(first, readFile(t, output)) || !bytes.Equal(firstReflection, readFile(t, reflection)) {
		t.Fatal("repeat compose changed outputs")
	}
}

func TestRunComposeSanitizesErrors(t *testing.T) {
	manifest, dir := composeManifestFixture(t)
	if err := os.WriteFile(filepath.Join(dir, "schema.zed"), []byte("private_secret_value_1234"), 0o600); err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	if code := Run(composeArgs(manifest, filepath.Join(dir, "out"), filepath.Join(dir, "reflection")), &stdout, &stderr); code != exitFailure {
		t.Fatal("invalid input accepted")
	}
	if stdout.Len() != 0 || stderr.String() != "spicedb-ir-compiler: composition failed\n" {
		t.Fatal("private error details exposed")
	}
}

func TestRunComposeRejectsOutputInputAliases(t *testing.T) {
	manifest, dir := composeManifestFixture(t)
	for _, input := range []string{manifest, filepath.Join(dir, "schema.zed"), filepath.Join(dir, "compiler.json"), filepath.Join(dir, "policy.pb")} {
		before := readFile(t, input)
		var stdout, stderr bytes.Buffer
		if Run(composeArgs(manifest, input, filepath.Join(dir, "reflection")), &stdout, &stderr) != exitFailure {
			t.Fatal("output alias accepted")
		}
		if !bytes.Equal(before, readFile(t, input)) {
			t.Fatal("input modified")
		}
	}
	var stdout, stderr bytes.Buffer
	if Run(composeArgs(manifest, filepath.Join(dir, "same"), filepath.Join(dir, "same")), &stdout, &stderr) != exitFailure {
		t.Fatal("duplicate output accepted")
	}
}

func TestRunComposeStagesBothOutputsBeforeReplacement(t *testing.T) {
	manifest, dir := composeManifestFixture(t)
	output := filepath.Join(dir, "out.zed")
	if err := os.WriteFile(output, []byte("previous"), 0o600); err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	if Run(composeArgs(manifest, output, filepath.Join(dir, "absent", "reflection.json")), &stdout, &stderr) != exitFailure {
		t.Fatal("invalid destination accepted")
	}
	if string(readFile(t, output)) != "previous" {
		t.Fatal("partial output replacement")
	}
}

func TestRunComposeUsage(t *testing.T) {
	for _, args := range [][]string{{"compose"}, {"compose", "--private-secret-invalid"}, {"compose", "extra"}} {
		var stdout, stderr bytes.Buffer
		if Run(args, &stdout, &stderr) != exitUsage || bytes.Contains(stderr.Bytes(), []byte("private-secret")) {
			t.Fatal("invalid options not sanitized")
		}
	}
	var stdout, stderr bytes.Buffer
	if Run([]string{"compose", "--help"}, &stdout, &stderr) != exitOK || stderr.Len() != 0 {
		t.Fatal("help failed")
	}
}

func composeManifestFixture(t *testing.T) (string, string) {
	t.Helper()
	dir := t.TempDir()
	for destination, source := range map[string]string{
		"schema.zed": "supported.zed", "compiler.json": "supported.json", "policy.pb": "supported.policy.pb",
	} {
		if err := os.WriteFile(filepath.Join(dir, destination), readFile(t, fixturePath(source)), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	manifest := composition.Manifest{FormatVersion: 1, Packages: []composition.PackageInput{{Legacy: true, SchemaPath: "schema.zed", ConfigPath: "compiler.json", ExpectedPolicyIRPath: "policy.pb"}}}
	data, err := json.Marshal(manifest)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "manifest.json")
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	return path, dir
}

func composeArgs(manifest, output, reflection string) []string {
	return []string{"compose", "--manifest", manifest, "--out", output, "--reflection-out", reflection, "--compiler-version", "1.54.0"}
}

func TestRunCompareSchemaReadOnlyAndPrivate(t *testing.T) {
	dir := t.TempDir()
	expected := filepath.Join(dir, "expected-private.zed")
	actual := filepath.Join(dir, "actual-private.zed")
	source := []byte("definition agent {}\ndefinition dataset {\n relation reader: agent\n permission read = reader\n}\n")
	reflected := bytes.ReplaceAll(source, []byte("\n "), []byte("\n\t"))
	reflected = append([]byte("/** Reflected documentation. */\n"), reflected...)
	for path, data := range map[string][]byte{expected: source, actual: reflected} {
		if err := os.WriteFile(path, data, 0o400); err != nil {
			t.Fatal(err)
		}
	}
	var stdout, stderr bytes.Buffer
	if code := Run(compareSchemaArgs(expected, actual), &stdout, &stderr); code != exitOK {
		t.Fatalf("compare exit=%d stderr=%s", code, stderr.String())
	}
	if stdout.String() != "schema=equivalent\n" || stderr.Len() != 0 {
		t.Fatal("unexpected comparison output")
	}
	if !bytes.Equal(source, readFile(t, expected)) || !bytes.Equal(reflected, readFile(t, actual)) {
		t.Fatal("comparison modified an input")
	}
	assertFileMode(t, expected, 0o400)
	assertFileMode(t, actual, 0o400)
	files, err := os.ReadDir(dir)
	if err != nil || len(files) != 2 {
		t.Fatal("comparison created output files")
	}
}

func TestRunCompareSchemaSanitizesFailuresAndBounds(t *testing.T) {
	for _, failure := range []string{"malformed", "different", "missing", "directory", "symlink", "expected oversized", "actual oversized", "wrong version"} {
		t.Run(failure, func(t *testing.T) {
			dir := t.TempDir()
			expected := filepath.Join(dir, "expected-private.zed")
			actual := filepath.Join(dir, "actual-private.zed")
			source := []byte("definition agent {}")
			for _, path := range []string{expected, actual} {
				if err := os.WriteFile(path, source, 0o600); err != nil {
					t.Fatal(err)
				}
			}
			data := source
			switch failure {
			case "malformed":
				data = []byte("private_invalid_schema_value")
			case "different":
				data = []byte("definition private_other_namespace {}")
			case "expected oversized", "actual oversized":
				data = bytes.Repeat([]byte(" "), composition.MaxSchemaBytes+1)
			}
			path := actual
			if failure == "expected oversized" {
				path = expected
			}
			if err := os.WriteFile(path, data, 0o600); err != nil {
				t.Fatal(err)
			}
			switch failure {
			case "missing":
				actual = filepath.Join(dir, "private-missing.zed")
			case "directory":
				actual = dir
			case "symlink":
				link := filepath.Join(dir, "private-link.zed")
				if err := os.Symlink(actual, link); err != nil {
					t.Fatal(err)
				}
				actual = link
			}
			args := compareSchemaArgs(expected, actual)
			if failure == "wrong version" {
				args[len(args)-1] = "private-unsupported-version"
			}
			var stdout, stderr bytes.Buffer
			if Run(args, &stdout, &stderr) != exitFailure || stdout.Len() != 0 || stderr.String() != "spicedb-ir-compiler: schema comparison failed\n" {
				t.Fatal("failure accepted or private diagnostic exposed")
			}
		})
	}
}

func TestRunCompareSchemaUsage(t *testing.T) {
	for _, args := range [][]string{
		{"compare-schema"},
		{"compare-schema", "--private-invalid-option"},
		{"compare-schema", "private-extra-argument"},
		{"compare-schema", "--expected", "private-path", "--actual", "private-path"},
		append(compareSchemaArgs("private-path", "private-path"), "private-extra-argument"),
	} {
		var stdout, stderr bytes.Buffer
		if Run(args, &stdout, &stderr) != exitUsage || stdout.Len() != 0 || bytes.Contains(stderr.Bytes(), []byte("private")) {
			t.Fatal("invalid compare options accepted or exposed")
		}
	}
	var stdout, stderr bytes.Buffer
	if Run([]string{"compare-schema", "--help"}, &stdout, &stderr) != exitOK || stderr.Len() != 0 {
		t.Fatal("compare help failed")
	}
}

func compareSchemaArgs(expected, actual string) []string {
	return []string{"compare-schema", "--expected", expected, "--actual", actual, "--compiler-version", "1.54.0"}
}
