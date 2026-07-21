package equivalencefixture

import "errors"

// ErrorCode identifies one fail-closed fixture validation category.
type ErrorCode string

const (
	CodeJSON           ErrorCode = "EQUIVALENCE_FIXTURE_JSON"
	CodeVersion        ErrorCode = "EQUIVALENCE_FIXTURE_VERSION"
	CodeLimit          ErrorCode = "EQUIVALENCE_FIXTURE_LIMIT"
	CodePath           ErrorCode = "EQUIVALENCE_FIXTURE_PATH"
	CodeSchemaDigest   ErrorCode = "EQUIVALENCE_FIXTURE_SCHEMA_DIGEST"
	CodePolicyIR       ErrorCode = "EQUIVALENCE_FIXTURE_POLICY_IR"
	CodeOrder          ErrorCode = "EQUIVALENCE_FIXTURE_ORDER"
	CodeDuplicate      ErrorCode = "EQUIVALENCE_FIXTURE_DUPLICATE"
	CodeValue          ErrorCode = "EQUIVALENCE_FIXTURE_VALUE"
	CodeProvenance     ErrorCode = "EQUIVALENCE_FIXTURE_PROVENANCE"
	CodeRepositoryRoot ErrorCode = "EQUIVALENCE_REPOSITORY_ROOT"
)

// Error omits filesystem paths and runtime context values from diagnostics.
type Error struct {
	Code    ErrorCode
	Message string
}

func (e *Error) Error() string {
	return string(e.Code) + ": " + e.Message
}

// IsCode reports whether err is a fixture error with the requested code.
func IsCode(err error, code ErrorCode) bool {
	var fixtureError *Error
	return errors.As(err, &fixtureError) && fixtureError.Code == code
}

func fixtureError(code ErrorCode, message string) error {
	return &Error{Code: code, Message: message}
}
