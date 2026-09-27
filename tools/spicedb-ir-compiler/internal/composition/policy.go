package composition

import (
	razielv1 "github.com/gitaalekhyapaul/hypha-backend/tools/spicedb-ir-compiler/gen/raziel/v1"
	"google.golang.org/protobuf/encoding/protowire"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

func decodePolicy(data []byte) (*razielv1.PolicyProgram, error) {
	if len(data) == 0 || len(data) > MaxPolicyIRBytes {
		return nil, ErrLimit
	}
	var policy razielv1.PolicyProgram
	if strictWire(data, policy.ProtoReflect().Descriptor(), 0) != nil {
		return nil, ErrPolicy
	}
	if (proto.UnmarshalOptions{RecursionLimit: 64}).Unmarshal(data, &policy) != nil {
		return nil, ErrPolicy
	}
	return &policy, nil
}

// The protobuf decoder otherwise merges duplicate singular fields and keeps unknowns.
// Reject both before comparing the expected policy with a fresh adapter compilation.
func strictWire(data []byte, descriptor protoreflect.MessageDescriptor, depth int) error {
	if depth > 64 {
		return ErrPolicy
	}
	seen := map[protoreflect.FieldNumber]bool{}
	oneofs := map[protoreflect.FullName]bool{}
	for len(data) > 0 {
		number, kind, n := protowire.ConsumeTag(data)
		if n < 0 {
			return ErrPolicy
		}
		data = data[n:]
		field := descriptor.Fields().ByNumber(number)
		if field == nil || !field.IsList() && seen[number] {
			return ErrPolicy
		}
		seen[number] = true
		if oneof := field.ContainingOneof(); oneof != nil {
			if oneofs[oneof.FullName()] {
				return ErrPolicy
			}
			oneofs[oneof.FullName()] = true
		}
		switch field.Kind() {
		case protoreflect.MessageKind, protoreflect.StringKind, protoreflect.BytesKind:
			if kind != protowire.BytesType {
				return ErrPolicy
			}
			value, size := protowire.ConsumeBytes(data)
			if size < 0 {
				return ErrPolicy
			}
			if field.Kind() == protoreflect.MessageKind && strictWire(value, field.Message(), depth+1) != nil {
				return ErrPolicy
			}
			data = data[size:]
		case protoreflect.Uint32Kind, protoreflect.Uint64Kind, protoreflect.Int32Kind,
			protoreflect.Int64Kind, protoreflect.Sint32Kind, protoreflect.Sint64Kind,
			protoreflect.BoolKind, protoreflect.EnumKind:
			if kind != protowire.VarintType {
				return ErrPolicy
			}
			value, size := protowire.ConsumeVarint(data)
			if size < 0 || field.Kind() == protoreflect.BoolKind && value > 1 ||
				field.Kind() == protoreflect.EnumKind && field.Enum().Values().ByNumber(protoreflect.EnumNumber(value)) == nil ||
				field.Kind() == protoreflect.Uint32Kind && value > 1<<32-1 {
				return ErrPolicy
			}
			data = data[size:]
		default:
			return ErrPolicy
		}
	}
	return nil
}

// These ceilings mirror crates/authorization-core/src/limits.rs. They apply to each
// isolated policy, not the composed schema, and do not change guest limits.
func checkPolicyCeilings(policy *razielv1.PolicyProgram) error {
	limits := policy.GetLimits()
	if limits == nil || len(policy.GetDefinitions()) > 16 || len(policy.GetPermissions()) > 32 || len(policy.GetCaveats()) > 16 {
		return ErrLimit
	}
	for _, pair := range [][2]uint32{
		{limits.GetMaxExpressionNodes(), 64}, {limits.GetMaxCaveatNodes(), 64},
		{limits.GetMaxPathDepth(), 4}, {limits.GetMaxRelationshipWitnesses(), 32},
		{limits.GetMaxMerkleDepth(), 32}, {limits.GetMaxStringBytes(), 256},
		{limits.GetMaxContextEntries(), 16}, {limits.GetMaxTotalPrivateInputBytes(), 4096},
	} {
		if pair[0] == 0 || pair[0] > pair[1] {
			return ErrLimit
		}
	}
	for _, definition := range policy.GetDefinitions() {
		if len(definition.GetRelations()) > 16 {
			return ErrLimit
		}
		for _, relation := range definition.GetRelations() {
			if len(relation.GetAllowedSubjectTypes()) > 16 {
				return ErrLimit
			}
		}
	}
	for _, caveat := range policy.GetCaveats() {
		if len(caveat.GetParameters()) > 16 {
			return ErrLimit
		}
	}
	return nil
}
