package cli

import (
	"fmt"
	"io"
)

const (
	name    = "spicedb-ir-compiler"
	version = "0.0.0-m0"

	exitOK    = 0
	exitUsage = 2
)

// Run executes the M0 command surface without terminating the calling process.
func Run(args []string, stdout, stderr io.Writer) int {
	if isVersionRequest(args) {
		fmt.Fprintf(stdout, "%s %s\n", name, version)
		return exitOK
	}

	fmt.Fprintf(stderr, "%s: unsupported invocation; schema compilation is not available in M0\n", name)
	fmt.Fprintf(stderr, "usage: %s version\n", name)
	return exitUsage
}

func isVersionRequest(args []string) bool {
	if len(args) != 1 {
		return false
	}

	return args[0] == "version" || args[0] == "--version"
}
