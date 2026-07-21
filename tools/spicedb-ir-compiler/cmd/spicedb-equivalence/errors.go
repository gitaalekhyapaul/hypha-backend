package main

type errorCode string

const (
	codeUsage             errorCode = "EQUIVALENCE_USAGE"
	codeConnect           errorCode = "EQUIVALENCE_CONNECT"
	codeReadiness         errorCode = "EQUIVALENCE_READINESS"
	codeSchemaWrite       errorCode = "EQUIVALENCE_SCHEMA_WRITE"
	codeRelationshipReset errorCode = "EQUIVALENCE_RELATIONSHIP_RESET"
	codeRelationshipWrite errorCode = "EQUIVALENCE_RELATIONSHIP_WRITE"
	codeCheck             errorCode = "EQUIVALENCE_CHECK"
	codeConditional       errorCode = "EQUIVALENCE_CONDITIONAL"
	codeUnspecified       errorCode = "EQUIVALENCE_UNSPECIFIED"
	codeMismatch          errorCode = "EQUIVALENCE_MISMATCH"
	codeOutput            errorCode = "EQUIVALENCE_OUTPUT"
)

type runnerError struct {
	code    errorCode
	message string
}

func (e *runnerError) Error() string {
	return string(e.code) + ": " + e.message
}

func newRunnerError(code errorCode, message string) error {
	return &runnerError{code: code, message: message}
}
