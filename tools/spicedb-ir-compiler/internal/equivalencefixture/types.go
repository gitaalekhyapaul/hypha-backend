package equivalencefixture

import (
	"bytes"
	"encoding/json"
	"fmt"
)

const (
	// SupportedVersion is the only shared fixture contract accepted by this harness.
	SupportedVersion uint32 = 1
	maxExactInteger  int64  = 1<<53 - 1
)

// Decision is the test result shared by SpiceDB and the Rust host evaluator.
type Decision string

const (
	DecisionAllow       Decision = "ALLOW"
	DecisionDeny        Decision = "DENY"
	DecisionConditional Decision = "CONDITIONAL"
)

// ScalarKind identifies one approved caveat scalar type.
type ScalarKind uint8

const (
	ScalarBool ScalarKind = iota + 1
	ScalarInt
	ScalarString
)

// Scalar stores a typed caveat value without providing a String method that could
// accidentally disclose it in diagnostics.
type Scalar struct {
	kind    ScalarKind
	boolean bool
	integer int64
	text    string
}

// Kind returns the externally tagged scalar type.
func (s Scalar) Kind() ScalarKind {
	return s.kind
}

// Value returns the primitive accepted by structpb.NewStruct.
func (s Scalar) Value() any {
	switch s.kind {
	case ScalarBool:
		return s.boolean
	case ScalarInt:
		return s.integer
	case ScalarString:
		return s.text
	default:
		return nil
	}
}

func (s *Scalar) UnmarshalJSON(data []byte) error {
	type scalarWire struct {
		Bool   json.RawMessage `json:"bool"`
		Int    json.RawMessage `json:"int"`
		String json.RawMessage `json:"string"`
	}

	var wire scalarWire
	if err := decodeStrictJSON(data, &wire); err != nil {
		return err
	}

	fields := 0
	for _, raw := range []json.RawMessage{wire.Bool, wire.Int, wire.String} {
		if len(raw) > 0 {
			fields++
		}
	}
	if fields != 1 {
		return fmt.Errorf("scalar must contain exactly one of bool, int, or string")
	}

	switch {
	case len(wire.Bool) > 0:
		var value bool
		if err := decodePrimitive(wire.Bool, &value); err != nil {
			return fmt.Errorf("bool scalar must contain a Boolean")
		}
		*s = Scalar{kind: ScalarBool, boolean: value}
	case len(wire.Int) > 0:
		var value int64
		if err := decodePrimitive(wire.Int, &value); err != nil {
			return fmt.Errorf("int scalar must contain an integer")
		}
		if value < -maxExactInteger || value > maxExactInteger {
			return fmt.Errorf("int scalar exceeds the exact JSON integer range")
		}
		*s = Scalar{kind: ScalarInt, integer: value}
	case len(wire.String) > 0:
		var value string
		if err := decodePrimitive(wire.String, &value); err != nil {
			return fmt.Errorf("string scalar must contain a string")
		}
		*s = Scalar{kind: ScalarString, text: value}
	}
	return nil
}

func decodePrimitive(data []byte, destination any) error {
	if bytes.Equal(bytes.TrimSpace(data), []byte("null")) {
		return fmt.Errorf("null is not an approved scalar")
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(destination); err != nil {
		return err
	}
	return requireJSONEOF(decoder)
}

// ObjectReference identifies one SpiceDB object.
type ObjectReference struct {
	ObjectType string `json:"object_type"`
	ObjectID   string `json:"object_id"`
}

// ContextEntry keeps caveat parameter names sorted and values typed.
type ContextEntry struct {
	Name  string `json:"name"`
	Value Scalar `json:"value"`
}

// RelationshipCaveat contains only relationship-provenance caveat inputs.
type RelationshipCaveat struct {
	Name    string         `json:"name"`
	Context []ContextEntry `json:"context"`
}

// Relationship is one shared relationship tuple.
type Relationship struct {
	Resource        ObjectReference     `json:"resource"`
	Relation        string              `json:"relation"`
	Subject         ObjectReference     `json:"subject"`
	SubjectRelation string              `json:"subject_relation,omitempty"`
	Caveat          *RelationshipCaveat `json:"caveat,omitempty"`
}

// Case is one authorization or partial-evaluation check shared by both evaluators.
type Case struct {
	ID               string          `json:"id"`
	Resource         ObjectReference `json:"resource"`
	Permission       string          `json:"permission"`
	Subject          ObjectReference `json:"subject"`
	RequestContext   []ContextEntry  `json:"request_context"`
	CapabilityClaims []ContextEntry  `json:"capability_claims"`
	Expected         Decision        `json:"expected"`
}

// Fixture contains validated in-memory data. Source paths never leave the loader.
type Fixture struct {
	Version       uint32
	Schema        []byte
	Relationships []Relationship
	Cases         []Case
}

type fixtureDocument struct {
	FixtureVersion     uint32         `json:"fixture_version"`
	SchemaPath         string         `json:"schema_path"`
	CompilerConfigPath string         `json:"compiler_config_path"`
	PolicyIRPath       string         `json:"policy_ir_path"`
	SchemaSHA256       string         `json:"schema_sha256"`
	Relationships      []Relationship `json:"relationships"`
	Cases              []Case         `json:"cases"`
}
