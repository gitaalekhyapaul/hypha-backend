package cli

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
)

func writeProgramOutputs(binaryPath, diagnosticPath string, program *razielv1.PolicyProgram) error {
	binary, err := (proto.MarshalOptions{Deterministic: true}).Marshal(program)
	if err != nil {
		return fmt.Errorf("encode PolicyIR Protobuf: %w", err)
	}
	diagnostic, err := marshalDiagnostic(program)
	if err != nil {
		return fmt.Errorf("encode PolicyIR diagnostic: %w", err)
	}

	binaryOutput, err := stageFile(binaryPath, binary)
	if err != nil {
		return fmt.Errorf("write PolicyIR Protobuf %q: %w", binaryPath, err)
	}
	defer binaryOutput.cleanup()
	diagnosticOutput, err := stageFile(diagnosticPath, diagnostic)
	if err != nil {
		return fmt.Errorf("write PolicyIR diagnostic %q: %w", diagnosticPath, err)
	}
	defer diagnosticOutput.cleanup()

	return installStagedOutputs(binaryOutput, diagnosticOutput)
}

func marshalDiagnostic(program *razielv1.PolicyProgram) ([]byte, error) {
	protobufJSON, err := (protojson.MarshalOptions{UseProtoNames: true}).Marshal(program)
	if err != nil {
		return nil, err
	}

	decoder := json.NewDecoder(bytes.NewReader(protobufJSON))
	decoder.UseNumber()
	var document any
	if err := decoder.Decode(&document); err != nil {
		return nil, err
	}
	formatted, err := json.MarshalIndent(document, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(formatted, '\n'), nil
}

type stagedFile struct {
	destination string
	temporary   string
}

func stageFile(path string, data []byte) (*stagedFile, error) {
	if info, err := os.Lstat(path); err == nil && info.IsDir() {
		return nil, fmt.Errorf("destination is a directory")
	} else if err != nil && !os.IsNotExist(err) {
		return nil, err
	}

	directory := filepath.Dir(path)
	temporary, err := os.CreateTemp(directory, "."+filepath.Base(path)+".tmp-*")
	if err != nil {
		return nil, err
	}
	temporaryPath := temporary.Name()
	succeeded := false
	defer func() {
		if !succeeded {
			temporary.Close()
			os.Remove(temporaryPath)
		}
	}()

	if _, err := temporary.Write(data); err != nil {
		return nil, err
	}
	if err := temporary.Sync(); err != nil {
		return nil, err
	}
	if err := temporary.Chmod(0o600); err != nil {
		return nil, err
	}
	if err := temporary.Close(); err != nil {
		return nil, err
	}
	succeeded = true
	return &stagedFile{destination: path, temporary: temporaryPath}, nil
}

func (file *stagedFile) commit() error {
	if err := os.Rename(file.temporary, file.destination); err != nil {
		return err
	}
	file.temporary = ""
	return nil
}

func (file *stagedFile) cleanup() {
	if file != nil && file.temporary != "" {
		os.Remove(file.temporary)
	}
}

func installStagedOutputs(binaryOutput, diagnosticOutput *stagedFile) error {
	backup, err := backupDestination(diagnosticOutput.destination)
	if err != nil {
		return fmt.Errorf("prepare PolicyIR diagnostic replacement: %w", err)
	}
	defer backup.discard()

	if err := diagnosticOutput.commit(); err != nil {
		return backup.restoreFailure(fmt.Errorf(
			"install PolicyIR diagnostic %q: %w",
			diagnosticOutput.destination,
			err,
		))
	}
	if err := binaryOutput.commit(); err != nil {
		return backup.restoreFailure(fmt.Errorf(
			"install PolicyIR Protobuf %q: %w",
			binaryOutput.destination,
			err,
		))
	}
	return nil
}

type destinationBackup struct {
	destination string
	temporary   string
}

func backupDestination(path string) (*destinationBackup, error) {
	info, err := os.Lstat(path)
	if os.IsNotExist(err) {
		return &destinationBackup{destination: path}, nil
	}
	if err != nil {
		return nil, err
	}
	if info.IsDir() {
		return nil, errors.New("destination is a directory")
	}

	temporary, err := os.CreateTemp(filepath.Dir(path), "."+filepath.Base(path)+".backup-*")
	if err != nil {
		return nil, err
	}
	temporaryPath := temporary.Name()
	if err := temporary.Close(); err != nil {
		os.Remove(temporaryPath)
		return nil, err
	}
	if err := os.Remove(temporaryPath); err != nil {
		return nil, err
	}
	if err := os.Rename(path, temporaryPath); err != nil {
		return nil, err
	}
	return &destinationBackup{destination: path, temporary: temporaryPath}, nil
}

func (backup *destinationBackup) restoreFailure(installError error) error {
	if err := os.Remove(backup.destination); err != nil && !os.IsNotExist(err) {
		return errors.Join(installError, fmt.Errorf("remove failed diagnostic: %w", err))
	}
	if backup.temporary == "" {
		return installError
	}
	if err := os.Rename(backup.temporary, backup.destination); err != nil {
		return errors.Join(installError, fmt.Errorf("restore previous diagnostic: %w", err))
	}
	backup.temporary = ""
	return installError
}

func (backup *destinationBackup) discard() {
	if backup != nil && backup.temporary != "" {
		os.Remove(backup.temporary)
	}
}

func samePath(left, right string) bool {
	leftCanonical := canonicalPath(left)
	rightCanonical := canonicalPath(right)
	if strings.EqualFold(leftCanonical, rightCanonical) {
		return true
	}
	leftInfo, leftErr := os.Stat(left)
	rightInfo, rightErr := os.Stat(right)
	return leftErr == nil && rightErr == nil && os.SameFile(leftInfo, rightInfo)
}

func canonicalPath(path string) string {
	absolute, err := filepath.Abs(path)
	if err != nil {
		absolute = filepath.Clean(path)
	}
	resolvedParent, err := filepath.EvalSymlinks(filepath.Dir(absolute))
	if err == nil {
		absolute = filepath.Join(resolvedParent, filepath.Base(absolute))
	}
	return filepath.Clean(absolute)
}
