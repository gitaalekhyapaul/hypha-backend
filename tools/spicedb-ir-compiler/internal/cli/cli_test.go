package cli

import (
	"bytes"
	"crypto/sha256"
	"os"
	"path/filepath"
	"strings"
	"testing"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/adapter"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

func TestRunIdentifiesCompiler(t *testing.T) {
	for _, args := range [][]string{{"version"}, {"--version"}} {
		var stdout bytes.Buffer
		var stderr bytes.Buffer

		exitCode := Run(args, &stdout, &stderr)

		if exitCode != exitOK {
			t.Fatalf("Run(%q) exit code = %d, want %d", args, exitCode, exitOK)
		}
		if got, want := stdout.String(), "spicedb-ir-compiler 0.1.0-m1\n"; got != want {
			t.Errorf("Run(%q) stdout = %q, want %q", args, got, want)
		}
		if got := stderr.String(); got != "" {
			t.Errorf("Run(%q) stderr = %q, want empty output", args, got)
		}
	}
}

func TestRunPrintsHelp(t *testing.T) {
	for _, args := range [][]string{{"help"}, {"--help"}, {"compile", "--help"}} {
		var stdout bytes.Buffer
		var stderr bytes.Buffer

		exitCode := Run(args, &stdout, &stderr)

		if exitCode != exitOK {
			t.Fatalf("Run(%q) exit code = %d, want %d", args, exitCode, exitOK)
		}
		if stdout.Len() == 0 {
			t.Errorf("Run(%q) stdout is empty", args)
		}
		if stderr.Len() != 0 {
			t.Errorf("Run(%q) stderr = %q, want empty output", args, stderr.String())
		}
	}
}

func TestRunCompileWritesDeterministicOutputs(t *testing.T) {
	temporary := t.TempDir()
	firstBinary := filepath.Join(temporary, "first.pb")
	firstDiagnostic := filepath.Join(temporary, "first.json")
	secondBinary := filepath.Join(temporary, "second.pb")
	secondDiagnostic := filepath.Join(temporary, "second.json")

	runSuccessfulCompile(t, firstBinary, firstDiagnostic)
	runSuccessfulCompile(t, secondBinary, secondDiagnostic)

	firstBytes := readFile(t, firstBinary)
	secondBytes := readFile(t, secondBinary)
	if !bytes.Equal(firstBytes, secondBytes) {
		t.Error("two CLI compilations produced different Protobuf bytes")
	}
	firstText := readFile(t, firstDiagnostic)
	secondText := readFile(t, secondDiagnostic)
	if !bytes.Equal(firstText, secondText) {
		t.Error("two CLI compilations produced different diagnostic bytes")
	}
	if want := readFile(t, fixturePath("supported.policy.json")); !bytes.Equal(firstText, want) {
		t.Error("CLI diagnostic does not match the reviewed golden")
	}

	var binaryProgram razielv1.PolicyProgram
	if err := proto.Unmarshal(firstBytes, &binaryProgram); err != nil {
		t.Fatalf("proto.Unmarshal() error = %v", err)
	}
	var diagnosticProgram razielv1.PolicyProgram
	if err := protojson.Unmarshal(firstText, &diagnosticProgram); err != nil {
		t.Fatalf("protojson.Unmarshal() error = %v", err)
	}
	if !proto.Equal(&binaryProgram, &diagnosticProgram) {
		t.Error("binary and diagnostic outputs contain different PolicyIR programs")
	}

	schema := readFile(t, fixturePath("supported.zed"))
	wantDigest := sha256.Sum256(schema)
	if !bytes.Equal(binaryProgram.GetSourceSchemaDigest(), wantDigest[:]) {
		t.Errorf("source_schema_digest = %x, want %x", binaryProgram.GetSourceSchemaDigest(), wantDigest)
	}
	if got, want := binaryProgram.GetCompilerVersion(), adapter.SupportedCompilerVersion; got != want {
		t.Errorf("compiler_version = %q, want %q", got, want)
	}
	assertFileMode(t, firstBinary, 0o600)
	assertFileMode(t, firstDiagnostic, 0o600)
}

func TestWriteProgramOutputsStagesBothBeforeReplacing(t *testing.T) {
	temporary := t.TempDir()
	binaryPath := filepath.Join(temporary, "policy.pb")
	oldBinary := []byte("existing binary")
	if err := os.WriteFile(binaryPath, oldBinary, 0o600); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
	diagnosticPath := filepath.Join(temporary, "missing", "policy.json")

	err := writeProgramOutputs(binaryPath, diagnosticPath, &razielv1.PolicyProgram{IrVersion: 1})

	if err == nil {
		t.Fatal("writeProgramOutputs() error = nil")
	}
	if got := readFile(t, binaryPath); !bytes.Equal(got, oldBinary) {
		t.Errorf("binary output = %q, want existing bytes %q", got, oldBinary)
	}
}

func TestInstallStagedOutputsRestoresDiagnosticOnBinaryFailure(t *testing.T) {
	temporary := t.TempDir()
	binaryPath := filepath.Join(temporary, "policy.pb")
	diagnosticPath := filepath.Join(temporary, "policy.json")
	oldDiagnostic := []byte("existing diagnostic")
	if err := os.WriteFile(diagnosticPath, oldDiagnostic, 0o600); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	binaryOutput, err := stageFile(binaryPath, []byte("new binary"))
	if err != nil {
		t.Fatalf("stageFile(binary) error = %v", err)
	}
	defer binaryOutput.cleanup()
	diagnosticOutput, err := stageFile(diagnosticPath, []byte("new diagnostic"))
	if err != nil {
		t.Fatalf("stageFile(diagnostic) error = %v", err)
	}
	defer diagnosticOutput.cleanup()
	if err := os.Mkdir(binaryPath, 0o700); err != nil {
		t.Fatalf("Mkdir(binary destination) error = %v", err)
	}

	err = installStagedOutputs(binaryOutput, diagnosticOutput)

	if err == nil {
		t.Fatal("installStagedOutputs() error = nil")
	}
	if got := readFile(t, diagnosticPath); !bytes.Equal(got, oldDiagnostic) {
		t.Errorf("diagnostic output = %q, want restored bytes %q", got, oldDiagnostic)
	}
}

func TestSamePathRejectsAliases(t *testing.T) {
	if !samePath("policy.pb", "Policy.pb") {
		t.Error("samePath() accepted case-only output names")
	}

	temporary := t.TempDir()
	realDirectory := filepath.Join(temporary, "real")
	if err := os.Mkdir(realDirectory, 0o700); err != nil {
		t.Fatalf("Mkdir() error = %v", err)
	}
	aliasDirectory := filepath.Join(temporary, "alias")
	if err := os.Symlink(realDirectory, aliasDirectory); err != nil {
		t.Fatalf("Symlink() error = %v", err)
	}
	if !samePath(
		filepath.Join(realDirectory, "policy.pb"),
		filepath.Join(aliasDirectory, "policy.pb"),
	) {
		t.Error("samePath() accepted output paths through aliased parent directories")
	}
}

func TestRunCompileRejectsCompilerVersionMismatch(t *testing.T) {
	temporary := t.TempDir()
	binaryPath := filepath.Join(temporary, "policy.pb")
	diagnosticPath := filepath.Join(temporary, "policy.json")
	args := compileArgs(binaryPath, diagnosticPath)
	args[6] = "1.53.0"

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := Run(args, &stdout, &stderr)

	if exitCode != exitFailure {
		t.Fatalf("Run() exit code = %d, want %d", exitCode, exitFailure)
	}
	if !strings.Contains(stderr.String(), string(adapter.CodeCompilerVersion)) {
		t.Errorf("Run() stderr = %q, want typed version mismatch", stderr.String())
	}
	assertFileDoesNotExist(t, binaryPath)
	assertFileDoesNotExist(t, diagnosticPath)
}

func TestRunCompileReportsSchemaLocation(t *testing.T) {
	temporary := t.TempDir()
	schemaPath := filepath.Join(temporary, "invalid.zed")
	if err := os.WriteFile(schemaPath, []byte("definition user {\n  relation viewer:\n}\n"), 0o600); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
	args := compileArgs(filepath.Join(temporary, "policy.pb"), filepath.Join(temporary, "policy.json"))
	args[2] = schemaPath

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := Run(args, &stdout, &stderr)

	if exitCode != exitFailure {
		t.Fatalf("Run() exit code = %d, want %d", exitCode, exitFailure)
	}
	if got := stderr.String(); !strings.Contains(got, string(adapter.CodeSchemaCompile)) ||
		!strings.Contains(got, schemaPath+":3:1:") {
		t.Errorf("Run() stderr = %q, want typed source location", got)
	}
}

func TestRunRejectsInvalidUsage(t *testing.T) {
	tests := []struct {
		name string
		args []string
		want string
	}{
		{name: "no command", want: "command is required"},
		{name: "unknown command", args: []string{"unknown"}, want: "unknown command"},
		{name: "missing compile flags", args: []string{"compile"}, want: "--schema is required"},
		{
			name: "same output path",
			args: compileArgs("policy.pb", "policy.pb"),
			want: "must name different files",
		},
		{
			name: "case-only output alias",
			args: compileArgs("policy.pb", "Policy.pb"),
			want: "must name different files",
		},
		{
			name: "output overwrites schema",
			args: compileArgs(fixturePath("supported.zed"), "policy.json"),
			want: "must not overwrite --schema",
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer

			exitCode := Run(test.args, &stdout, &stderr)

			if exitCode != exitUsage {
				t.Fatalf("Run() exit code = %d, want %d", exitCode, exitUsage)
			}
			if !strings.Contains(stderr.String(), test.want) {
				t.Errorf("Run() stderr = %q, want text %q", stderr.String(), test.want)
			}
			if stdout.Len() != 0 {
				t.Errorf("Run() stdout = %q, want empty output", stdout.String())
			}
		})
	}
}

func runSuccessfulCompile(t *testing.T, binaryPath, diagnosticPath string) {
	t.Helper()
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	exitCode := Run(compileArgs(binaryPath, diagnosticPath), &stdout, &stderr)
	if exitCode != exitOK {
		t.Fatalf("Run() exit code = %d, want %d, stderr = %q", exitCode, exitOK, stderr.String())
	}
	if !strings.Contains(stdout.String(), binaryPath) || !strings.Contains(stdout.String(), diagnosticPath) {
		t.Errorf("Run() stdout = %q, want both output paths", stdout.String())
	}
	if stderr.Len() != 0 {
		t.Errorf("Run() stderr = %q, want empty output", stderr.String())
	}
}

func compileArgs(binaryPath, diagnosticPath string) []string {
	return []string{
		"compile",
		"--schema", fixturePath("supported.zed"),
		"--config", fixturePath("supported.json"),
		"--compiler-version", adapter.SupportedCompilerVersion,
		"--out", binaryPath,
		"--diagnostic-out", diagnosticPath,
	}
}

func fixturePath(name string) string {
	return filepath.Join("..", "adapter", "testdata", name)
}

func readFile(t *testing.T, path string) []byte {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile(%q) error = %v", path, err)
	}
	return data
}

func assertFileDoesNotExist(t *testing.T, path string) {
	t.Helper()
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Errorf("Stat(%q) error = %v, want file not to exist", path, err)
	}
}

func assertFileMode(t *testing.T, path string, want os.FileMode) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat(%q) error = %v", path, err)
	}
	if got := info.Mode().Perm(); got != want {
		t.Errorf("mode for %q = %o, want %o", path, got, want)
	}
}
