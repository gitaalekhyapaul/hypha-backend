package adapter

import (
	"fmt"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
)

// ErrorCode is a stable compiler-adapter failure category.
type ErrorCode string

const (
	CodeCompilerVersion       ErrorCode = "COMPILER_VERSION_MISMATCH"
	CodeSchemaCompile         ErrorCode = "SCHEMA_COMPILE_ERROR"
	CodeInvalidCompiledSchema ErrorCode = "INVALID_COMPILED_SCHEMA"
	CodeUnsupportedConstruct  ErrorCode = "UNSUPPORTED_CONSTRUCT"
	CodeMissingReference      ErrorCode = "MISSING_REFERENCE"
	CodeAmbiguousReference    ErrorCode = "AMBIGUOUS_REFERENCE"
	CodePermissionCycle       ErrorCode = "PERMISSION_CYCLE"
	CodeCaveatBinding         ErrorCode = "CAVEAT_BINDING_ERROR"
	CodeCaveatExpression      ErrorCode = "CAVEAT_EXPRESSION_ERROR"
	CodeProgramLimit          ErrorCode = "PROGRAM_LIMIT_EXCEEDED"
)

// Error carries a typed code and an optional one-based schema location.
type Error struct {
	Code    ErrorCode
	Source  string
	Line    uint64
	Column  uint64
	Message string
	Cause   error
}

func (e *Error) Error() string {
	location := e.Source
	if e.Line > 0 && e.Column > 0 {
		location = fmt.Sprintf("%s:%d:%d", e.Source, e.Line, e.Column)
	}
	if location == "" {
		return fmt.Sprintf("%s: %s", e.Code, e.Message)
	}
	return fmt.Sprintf("%s: %s: %s", location, e.Code, e.Message)
}

func (e *Error) Unwrap() error {
	return e.Cause
}

func locatedError(code ErrorCode, source, message string, position *core.SourcePosition) *Error {
	err := &Error{Code: code, Source: source, Message: message}
	if position != nil {
		err.Line = position.GetZeroIndexedLineNumber() + 1
		err.Column = position.GetZeroIndexedColumnPosition() + 1
	}
	return err
}
