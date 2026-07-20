package adapter

import (
	"errors"

	spicecompiler "github.com/authzed/spicedb/pkg/schemadsl/compiler"
	"github.com/authzed/spicedb/pkg/schemadsl/input"
)

func compileSpiceDB(source string, schema []byte) (*spicecompiler.CompiledSchema, error) {
	compiled, err := spicecompiler.Compile(
		spicecompiler.InputSchema{
			Source:       input.Source(source),
			SchemaString: string(schema),
		},
		spicecompiler.AllowUnprefixedObjectType(),
		spicecompiler.DisallowExpirationFlag(),
		spicecompiler.DisallowImportFlag(),
	)
	if err == nil {
		return compiled, nil
	}

	result := &Error{
		Code:    CodeSchemaCompile,
		Source:  source,
		Message: err.Error(),
		Cause:   err,
	}
	var contextual spicecompiler.WithContextError
	if !errors.As(err, &contextual) {
		return nil, result
	}
	result.Message = contextual.BaseMessage
	startLine, startColumn, positionErr := contextual.SourceRange.Start().LineAndColumn()
	if positionErr == nil {
		result.Line = uint64(startLine) + 1
		result.Column = uint64(startColumn) + 1
	}
	if contextual.Source != "" {
		result.Source = string(contextual.Source)
	}
	return nil, result
}
