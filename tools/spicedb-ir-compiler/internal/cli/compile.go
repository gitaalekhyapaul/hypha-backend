package cli

import (
	"errors"
	"flag"
	"fmt"
	"io"
	"os"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/adapter"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
)

type compileOptions struct {
	schemaPath      string
	configPath      string
	compilerVersion string
	outputPath      string
	diagnosticPath  string
}

func runCompile(args []string, stdout, stderr io.Writer) int {
	options, help, err := parseCompileOptions(args)
	if help {
		writeCompileUsage(stdout)
		return exitOK
	}
	if err != nil {
		fmt.Fprintf(stderr, "%s: %v\n", name, err)
		writeCompileUsage(stderr)
		return exitUsage
	}

	program, err := compileSchema(options)
	if err != nil {
		fmt.Fprintf(stderr, "%s: %v\n", name, err)
		return exitFailure
	}
	if err := writeProgramOutputs(options.outputPath, options.diagnosticPath, program); err != nil {
		fmt.Fprintf(stderr, "%s: %v\n", name, err)
		return exitFailure
	}

	fmt.Fprintf(stdout, "wrote PolicyIR Protobuf to %s\n", options.outputPath)
	fmt.Fprintf(stdout, "wrote PolicyIR diagnostic to %s\n", options.diagnosticPath)
	return exitOK
}

func parseCompileOptions(args []string) (compileOptions, bool, error) {
	var options compileOptions
	flags := flag.NewFlagSet("compile", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.StringVar(&options.schemaPath, "schema", "", "SpiceDB .zed schema path")
	flags.StringVar(&options.configPath, "config", "", "compiler JSON configuration path")
	flags.StringVar(&options.compilerVersion, "compiler-version", "", "required SpiceDB compiler version")
	flags.StringVar(&options.outputPath, "out", "", "PolicyIR Protobuf output path")
	flags.StringVar(&options.diagnosticPath, "diagnostic-out", "", "PolicyIR text diagnostic output path")

	if err := flags.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return compileOptions{}, true, nil
		}
		return compileOptions{}, false, err
	}
	if flags.NArg() != 0 {
		return compileOptions{}, false, fmt.Errorf("compile accepts no positional arguments, got %q", flags.Arg(0))
	}
	for _, required := range []struct {
		name  string
		value string
	}{
		{name: "--schema", value: options.schemaPath},
		{name: "--config", value: options.configPath},
		{name: "--compiler-version", value: options.compilerVersion},
		{name: "--out", value: options.outputPath},
		{name: "--diagnostic-out", value: options.diagnosticPath},
	} {
		if required.value == "" {
			return compileOptions{}, false, fmt.Errorf("%s is required", required.name)
		}
	}
	if samePath(options.outputPath, options.diagnosticPath) {
		return compileOptions{}, false, errors.New("--out and --diagnostic-out must name different files")
	}
	for _, input := range []struct {
		name string
		path string
	}{
		{name: "--schema", path: options.schemaPath},
		{name: "--config", path: options.configPath},
	} {
		if samePath(options.outputPath, input.path) || samePath(options.diagnosticPath, input.path) {
			return compileOptions{}, false, fmt.Errorf("output path must not overwrite %s", input.name)
		}
	}
	return options, false, nil
}

func compileSchema(options compileOptions) (*razielv1.PolicyProgram, error) {
	schema, err := os.ReadFile(options.schemaPath)
	if err != nil {
		return nil, fmt.Errorf("read schema %q: %w", options.schemaPath, err)
	}
	configBytes, err := os.ReadFile(options.configPath)
	if err != nil {
		return nil, fmt.Errorf("read compiler config %q: %w", options.configPath, err)
	}
	compilerConfig, err := config.Parse(configBytes)
	if err != nil {
		return nil, fmt.Errorf("parse compiler config %q: %w", options.configPath, err)
	}

	return adapter.Compile(adapter.CompileRequest{
		SourceName:      options.schemaPath,
		Schema:          schema,
		CompilerVersion: options.compilerVersion,
		Config:          compilerConfig,
	})
}

func writeCompileUsage(writer io.Writer) {
	fmt.Fprintf(writer, "usage: %s compile \\\n", name)
	fmt.Fprintln(writer, "  --schema <schema.zed> \\")
	fmt.Fprintln(writer, "  --config <compiler.json> \\")
	fmt.Fprintln(writer, "  --compiler-version <version> \\")
	fmt.Fprintln(writer, "  --out <policy.pb> \\")
	fmt.Fprintln(writer, "  --diagnostic-out <policy.json>")
}
