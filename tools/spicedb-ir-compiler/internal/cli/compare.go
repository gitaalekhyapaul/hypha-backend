package cli

import (
	"errors"
	"flag"
	"fmt"
	"io"

	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/adapter"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/composition"
)

func runCompareSchema(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("compare-schema", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	expected := flags.String("expected", "", "trusted composed schema")
	actual := flags.String("actual", "", "reflected schema")
	version := flags.String("compiler-version", "", "required SpiceDB compiler version")
	err := flags.Parse(args)
	if errors.Is(err, flag.ErrHelp) {
		writeCompareSchemaUsage(stdout)
		return exitOK
	}
	if err != nil || flags.NArg() != 0 || *expected == "" || *actual == "" || *version == "" {
		fmt.Fprintln(stderr, "spicedb-ir-compiler: invalid compare-schema options")
		writeCompareSchemaUsage(stderr)
		return exitUsage
	}
	if *version != adapter.SupportedCompilerVersion || composition.ValidateSchemaFilesEquivalent(*expected, *actual) != nil {
		// Upstream diagnostics can contain private source text and file paths.
		fmt.Fprintln(stderr, "spicedb-ir-compiler: schema comparison failed")
		return exitFailure
	}
	fmt.Fprintln(stdout, "schema=equivalent")
	return exitOK
}

func writeCompareSchemaUsage(writer io.Writer) {
	fmt.Fprintln(writer, "usage: spicedb-ir-compiler compare-schema --expected PATH --actual PATH --compiler-version 1.54.0")
}
