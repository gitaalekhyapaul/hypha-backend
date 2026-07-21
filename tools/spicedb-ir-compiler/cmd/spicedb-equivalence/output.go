package main

import (
	"bytes"
	"io"
	"os"
)

func writeResults(outputPath string, stdout io.Writer, results []caseResult) error {
	var output bytes.Buffer
	for _, result := range results {
		output.WriteString(result.id)
		output.WriteByte('\t')
		output.WriteString(string(result.decision))
		output.WriteByte('\n')
	}
	if outputPath == "-" {
		if _, err := stdout.Write(output.Bytes()); err != nil {
			return newRunnerError(codeOutput, "cannot write results to stdout")
		}
		return nil
	}
	file, err := os.OpenFile(outputPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o600)
	if err != nil {
		return newRunnerError(codeOutput, "cannot open the result file")
	}
	writeErr := error(nil)
	if err := file.Chmod(0o600); err != nil {
		writeErr = newRunnerError(codeOutput, "cannot protect the result file")
	} else if _, err := file.Write(output.Bytes()); err != nil {
		writeErr = newRunnerError(codeOutput, "cannot write the result file")
	}
	if err := file.Close(); writeErr == nil && err != nil {
		writeErr = newRunnerError(codeOutput, "cannot close the result file")
	}
	return writeErr
}
