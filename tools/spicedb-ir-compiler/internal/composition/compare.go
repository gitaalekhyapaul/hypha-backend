package composition

import (
	"errors"
	"unicode/utf8"

	"google.golang.org/protobuf/proto"
)

var ErrSchemaComparison = errors.New("schema comparison failed")

// ValidateSchemaEquivalent compares pinned-compiler structures, not source digests.
// It ignores definition order, documentation and source positions, but preserves
// names, definition kinds and expression structure. The expected schema must come
// from trusted composition; equality alone establishes no policy approval.
func ValidateSchemaEquivalent(expected, actual []byte) error {
	want, err := schemaDefinitions(expected)
	if err != nil {
		return err
	}
	got, err := schemaDefinitions(actual)
	if err != nil {
		return err
	}
	if len(want) != len(got) {
		return ErrSchemaComparison
	}
	for name, definition := range want {
		if !proto.Equal(definition, got[name]) {
			return ErrSchemaComparison
		}
	}
	return nil
}

// ValidateSchemaFilesEquivalent reads two regular files, each bounded to 1 MiB,
// then compares their compiled structures. It performs no writes or network I/O.
func ValidateSchemaFilesEquivalent(expectedPath, actualPath string) error {
	expected, err := readBounded(expectedPath, MaxSchemaBytes)
	if err != nil {
		return err
	}
	actual, err := readBounded(actualPath, MaxSchemaBytes)
	if err != nil {
		return err
	}
	return ValidateSchemaEquivalent(expected, actual)
}

func schemaDefinitions(source []byte) (map[string]proto.Message, error) {
	if len(source) > MaxSchemaBytes {
		return nil, ErrLimit
	}
	if !utf8.Valid(source) {
		return nil, ErrInput
	}
	// The same top-level grammar as composition rejects imports and use flags.
	// The pinned compiler accepts unused flags even with DisallowExpirationFlag.
	if _, err := definitionSources(source); err != nil {
		return nil, ErrSchemaComparison
	}
	compiled, err := compileSchema(source)
	if err != nil {
		return nil, ErrSchemaComparison
	}
	definitions := make(map[string]proto.Message, len(compiled.OrderedDefinitions))
	for _, definition := range compiled.OrderedDefinitions {
		name := definition.GetName()
		if name == "" || definitions[name] != nil {
			return nil, ErrSchemaComparison
		}
		normalized, err := normalizedDefinition(definition)
		if err != nil {
			return nil, ErrSchemaComparison
		}
		definitions[name] = normalized
	}
	return definitions, nil
}
