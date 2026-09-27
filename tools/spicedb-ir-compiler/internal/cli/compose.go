package cli

import (
	"errors"
	"flag"
	"fmt"
	"io"

	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/composition"
)

func runCompose(args []string, stdout, stderr io.Writer) int {
	flags := flag.NewFlagSet("compose", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	manifestPath := flags.String("manifest", "", "owner composition manifest")
	outputPath := flags.String("out", "", "private schema output")
	reflectionPath := flags.String("reflection-out", "", "private reflection output")
	version := flags.String("compiler-version", "", "required SpiceDB compiler version")
	err := flags.Parse(args)
	if errors.Is(err, flag.ErrHelp) {
		writeComposeUsage(stdout)
		return exitOK
	}
	if err != nil || flags.NArg() != 0 || *manifestPath == "" || *outputPath == "" || *reflectionPath == "" || *version == "" {
		fmt.Fprintln(stderr, "spicedb-ir-compiler: invalid compose options")
		writeComposeUsage(stderr)
		return exitUsage
	}
	manifest, err := composition.LoadManifest(*manifestPath)
	if err != nil {
		return composeFailure(stderr)
	}
	inputs := []string{*manifestPath}
	for _, entry := range manifest.Packages {
		inputs = append(inputs, entry.SchemaPath, entry.ConfigPath, entry.ExpectedPolicyIRPath)
	}
	if samePath(*outputPath, *reflectionPath) {
		return composeFailure(stderr)
	}
	for _, input := range inputs {
		if samePath(input, *outputPath) || samePath(input, *reflectionPath) {
			return composeFailure(stderr)
		}
	}
	result, err := composition.Compose(manifest, *version)
	if err != nil {
		return composeFailure(stderr)
	}
	schema, err := stageFile(*outputPath, result.Schema)
	if err != nil {
		return composeFailure(stderr)
	}
	defer schema.cleanup()
	reflection, err := stageFile(*reflectionPath, result.Reflection)
	if err != nil {
		return composeFailure(stderr)
	}
	defer reflection.cleanup()
	if installStagedOutputs(schema, reflection) != nil {
		return composeFailure(stderr)
	}
	fmt.Fprintln(stdout, "composition=validated")
	return exitOK
}

func composeFailure(stderr io.Writer) int {
	// Upstream diagnostics can include complete private expressions and paths.
	fmt.Fprintln(stderr, "spicedb-ir-compiler: composition failed")
	return exitFailure
}

func writeComposeUsage(writer io.Writer) {
	fmt.Fprintln(writer, "usage: spicedb-ir-compiler compose --manifest PATH --out PATH --reflection-out PATH --compiler-version 1.54.0")
}
