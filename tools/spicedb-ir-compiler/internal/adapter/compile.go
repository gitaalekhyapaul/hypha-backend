package adapter

import (
	"bytes"
	"crypto/sha256"
	"fmt"

	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/internal/config"
)

// CompileRequest contains public build-time inputs for one PolicyIR program.
type CompileRequest struct {
	SourceName      string
	Schema          []byte
	CompilerVersion string
	Config          config.Config
}

// Compile runs the pinned SpiceDB compiler and translates its output into PolicyIR.
func Compile(request CompileRequest) (*razielv1.PolicyProgram, error) {
	if request.CompilerVersion != SupportedCompilerVersion {
		return nil, &Error{
			Code:   CodeCompilerVersion,
			Source: request.SourceName,
			Message: fmt.Sprintf(
				"compiler version %q is not supported, expected %q",
				request.CompilerVersion,
				SupportedCompilerVersion,
			),
		}
	}

	schema := bytes.Clone(request.Schema)
	digest := sourceSchemaDigest(schema)
	compiled, err := compileSpiceDB(request.SourceName, schema)
	if err != nil {
		return nil, err
	}
	translator, err := newTranslator(request.SourceName, compiled, request.Config)
	if err != nil {
		return nil, err
	}
	return translator.translate(digest[:])
}

func sourceSchemaDigest(schema []byte) [sha256.Size]byte {
	return sha256.Sum256(schema)
}
