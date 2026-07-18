package cli

import (
	"bytes"
	"testing"
)

func TestRunIdentifiesCompiler(t *testing.T) {
	tests := []struct {
		name string
		args []string
	}{
		{name: "version command", args: []string{"version"}},
		{name: "version flag", args: []string{"--version"}},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer

			exitCode := Run(test.args, &stdout, &stderr)

			if exitCode != exitOK {
				t.Fatalf("Run() exit code = %d, want %d", exitCode, exitOK)
			}
			if got, want := stdout.String(), "spicedb-ir-compiler 0.0.0-m0\n"; got != want {
				t.Errorf("Run() stdout = %q, want %q", got, want)
			}
			if got := stderr.String(); got != "" {
				t.Errorf("Run() stderr = %q, want empty output", got)
			}
		})
	}
}

func TestRunRejectsUnsupportedInvocation(t *testing.T) {
	tests := []struct {
		name string
		args []string
	}{
		{name: "no arguments"},
		{name: "compile command", args: []string{"compile"}},
		{name: "extra version argument", args: []string{"version", "schema.zed"}},
	}

	const wantStderr = "spicedb-ir-compiler: unsupported invocation; schema compilation is not available in M0\n" +
		"usage: spicedb-ir-compiler version\n"

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer

			exitCode := Run(test.args, &stdout, &stderr)

			if exitCode != exitUsage {
				t.Fatalf("Run() exit code = %d, want %d", exitCode, exitUsage)
			}
			if got := stdout.String(); got != "" {
				t.Errorf("Run() stdout = %q, want empty output", got)
			}
			if got := stderr.String(); got != wantStderr {
				t.Errorf("Run() stderr = %q, want %q", got, wantStderr)
			}
		})
	}
}
