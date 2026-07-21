package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"

	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/equivalencefixture"
)

const (
	exitOK                  = 0
	exitFailure             = 1
	exitUsage               = 2
	equivalenceEndpoint     = "spicedb-equivalence:50051"
	equivalencePresharedKey = "raziel-equivalence-only"
)

type options struct {
	fixturePath string
	outputPath  string
}

func run(args []string, stdout, stderr io.Writer) int {
	parsed, help, err := parseOptions(args)
	if help {
		writeUsage(stdout)
		return exitOK
	}
	if err != nil {
		fmt.Fprintln(stderr, newRunnerError(codeUsage, "invalid command arguments"))
		writeUsage(stderr)
		return exitUsage
	}

	repoRoot, err := equivalencefixture.FindRepositoryRoot(parsed.fixturePath)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailure
	}
	fixture, err := equivalencefixture.Load(repoRoot, parsed.fixturePath)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailure
	}

	results, err := runLive(
		context.Background(),
		equivalenceEndpoint,
		equivalencePresharedKey,
		fixture,
	)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailure
	}
	if err := writeResults(parsed.outputPath, stdout, results); err != nil {
		fmt.Fprintln(stderr, err)
		return exitFailure
	}
	return exitOK
}

func parseOptions(args []string) (options, bool, error) {
	parsed := options{outputPath: "-"}
	flags := flag.NewFlagSet("spicedb-equivalence", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.StringVar(&parsed.fixturePath, "fixture", "", "shared equivalence fixture")
	flags.StringVar(&parsed.outputPath, "output", parsed.outputPath, "result file or - for stdout")
	if err := flags.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return options{}, true, nil
		}
		return options{}, false, err
	}
	if flags.NArg() != 0 || parsed.fixturePath == "" || parsed.outputPath == "" {
		return options{}, false, errors.New("all required values must be present")
	}
	return parsed, false, nil
}

func writeUsage(writer io.Writer) {
	fmt.Fprintln(writer, "usage: spicedb-equivalence --fixture <file> --output <file|->")
}
