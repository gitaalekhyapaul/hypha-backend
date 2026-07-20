package adapter

import (
	"fmt"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
)

func validatePolicyIdentifier(
	source, kind, value string,
	position *core.SourcePosition,
) error {
	if isPolicyIdentifier(value) {
		return nil
	}
	return locatedError(
		CodeUnsupportedConstruct,
		source,
		fmt.Sprintf("%s %q is outside the PolicyIR identifier subset", kind, value),
		position,
	)
}

func isPolicyIdentifier(value string) bool {
	if len(value) == 0 || value[0] < 'a' || value[0] > 'z' {
		return false
	}
	for index := 1; index < len(value); index++ {
		character := value[index]
		if (character < 'a' || character > 'z') &&
			(character < '0' || character > '9') &&
			character != '_' {
			return false
		}
	}
	return true
}
