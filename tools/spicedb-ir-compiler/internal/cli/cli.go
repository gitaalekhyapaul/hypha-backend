package cli

import (
	"fmt"
	"io"
)

const (
	name        = "spicedb-ir-compiler"
	toolVersion = "0.1.0-m1"

	exitOK      = 0
	exitFailure = 1
	exitUsage   = 2
)

// Run executes the command without terminating the calling process.
func Run(args []string, stdout, stderr io.Writer) int {
	if isVersionRequest(args) {
		fmt.Fprintf(stdout, "%s %s\n", name, toolVersion)
		return exitOK
	}
	if isHelpRequest(args) {
		writeRootUsage(stdout)
		return exitOK
	}
	if len(args) > 0 && args[0] == "compile" {
		return runCompile(args[1:], stdout, stderr)
	}

	if len(args) == 0 {
		fmt.Fprintf(stderr, "%s: command is required\n", name)
	} else {
		fmt.Fprintf(stderr, "%s: unknown command %q\n", name, args[0])
	}
	writeRootUsage(stderr)
	return exitUsage
}

func isVersionRequest(args []string) bool {
	return len(args) == 1 && (args[0] == "version" || args[0] == "--version")
}

func isHelpRequest(args []string) bool {
	return len(args) == 1 && (args[0] == "help" || args[0] == "-h" || args[0] == "--help")
}

func writeRootUsage(writer io.Writer) {
	fmt.Fprintf(writer, "usage: %s <command> [options]\n", name)
	fmt.Fprintln(writer, "commands:")
	fmt.Fprintln(writer, "  compile  compile a SpiceDB schema into Raziel PolicyIR")
	fmt.Fprintln(writer, "  version  print the compiler adapter version")
}
