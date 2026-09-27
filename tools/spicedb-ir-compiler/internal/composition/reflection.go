package composition

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"

	core "github.com/authzed/spicedb/pkg/proto/core/v1"
	impl "github.com/authzed/spicedb/pkg/proto/impl/v1"
	"github.com/authzed/spicedb/pkg/schemadsl/compiler"
	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

// Reflection is private compiler evidence, not an approval or installation receipt.
type Reflection struct {
	FormatVersion   uint32                 `json:"format_version"`
	CompilerVersion string                 `json:"compiler_version"`
	SchemaSHA256    string                 `json:"schema_sha256"`
	Packages        []PackageReflection    `json:"packages"`
	Definitions     []DefinitionReflection `json:"definitions"`
}

type PackageReflection struct {
	VersionID              string   `json:"version_id,omitempty"`
	Legacy                 bool     `json:"legacy,omitempty"`
	SourceSHA256           string   `json:"source_sha256"`
	ConfigSHA256           string   `json:"config_sha256"`
	ExpectedPolicyIRSHA256 string   `json:"expected_policy_ir_sha256"`
	Definitions            []string `json:"definitions"`
	Caveats                []string `json:"caveats"`
}

type DefinitionReflection struct {
	Name           string          `json:"name"`
	Kind           string          `json:"kind"`
	SemanticSHA256 string          `json:"semantic_sha256"`
	Compiled       json.RawMessage `json:"compiled"`
}

func digest(data []byte) string {
	value := sha256.Sum256(data)
	return hex.EncodeToString(value[:])
}

func normalizedDefinition(definition compiler.SchemaDefinition) (proto.Message, error) {
	copy := proto.Clone(definition)
	stripSourceMetadata(copy.ProtoReflect())
	if caveat, ok := copy.(*core.CaveatDefinition); ok {
		var decoded impl.DecodedCaveat
		if proto.Unmarshal(caveat.SerializedExpression, &decoded) != nil || decoded.GetCel() == nil {
			return nil, ErrComposition
		}
		// Retain checked expression, reference/type maps and node IDs. Source text
		// locations change on composition and do not participate in evaluation.
		decoded.GetCel().SourceInfo = nil
		serialized, err := (proto.MarshalOptions{Deterministic: true}).Marshal(&decoded)
		if err != nil {
			return nil, ErrComposition
		}
		caveat.SerializedExpression = serialized
	}
	return copy, nil
}

func stripSourceMetadata(message protoreflect.Message) {
	message.Range(func(field protoreflect.FieldDescriptor, value protoreflect.Value) bool {
		if field.Message() != nil && field.Message().FullName() == "core.v1.SourcePosition" {
			message.Clear(field)
			return true
		}
		if field.Message() != nil && field.Message().FullName() == "core.v1.Metadata" {
			metadata := value.Message().Interface().(*core.Metadata)
			kept := metadata.MetadataMessage[:0]
			for _, entry := range metadata.MetadataMessage {
				if !entry.MessageIs(&impl.DocComment{}) {
					kept = append(kept, entry)
				}
			}
			metadata.MetadataMessage = kept
			if len(kept) == 0 {
				message.Clear(field)
			}
			return true
		}
		if field.IsMap() && field.MapValue().Message() != nil {
			value.Map().Range(func(_ protoreflect.MapKey, item protoreflect.Value) bool {
				stripSourceMetadata(item.Message())
				return true
			})
		} else if field.IsList() && field.Message() != nil {
			for i := 0; i < value.List().Len(); i++ {
				stripSourceMetadata(value.List().Get(i).Message())
			}
		} else if !field.IsMap() && field.Message() != nil {
			stripSourceMetadata(value.Message())
		}
		return true
	})
}

func reflectDefinition(definition compiler.SchemaDefinition, normalized proto.Message) (DefinitionReflection, error) {
	kind := "definition"
	if _, ok := definition.(*core.CaveatDefinition); ok {
		kind = "caveat"
	}
	encoded, err := (proto.MarshalOptions{Deterministic: true}).Marshal(normalized)
	if err != nil {
		return DefinitionReflection{}, ErrComposition
	}
	compiled, err := (protojson.MarshalOptions{UseProtoNames: true}).Marshal(normalized)
	if err != nil {
		return DefinitionReflection{}, ErrComposition
	}
	return DefinitionReflection{
		Name: definition.GetName(), Kind: kind, SemanticSHA256: digest(encoded), Compiled: compiled,
	}, nil
}
