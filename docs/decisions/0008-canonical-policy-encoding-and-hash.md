# ADR-0008: Canonical Policy Encoding and Hash

- Status: accepted
- Date: 2026-07-22
- Milestone: M2
- Approved by maintainer: 2026-07-22

## Context

`PolicyProgram` Protobuf is the cross-language transport and review contract. Raw
Protobuf bytes are not stable commitment inputs. Protobuf permits field reordering,
multiple valid integer encodings, duplicate singular fields, and unknown fields that
generated decoders can discard.

Raziel needs one versioned byte encoding for approved PolicyIR. The verifier and SP1
guest must reproduce the same `policy_hash` from the validated typed program. The
encoding must preserve the exact string semantics evaluated by SpiceDB and Rust.

## Decision

### Hash and preimage framing

Use SHA-256 and return its raw 32-byte digest.

```text
domain = ASCII("raziel/policy/v1")
payload = canonical_policy_ir_v1

policy_hash = SHA-256(
  u32be(len(domain)) ||
  domain ||
  u32be(len(payload)) ||
  payload
)
```

Reject a domain or payload whose byte length cannot fit in `u32`. The domain is fixed
by this decision. A future hash, domain, or framing rule needs a new version and
migration decision.

### Canonical payload header

`canonical_policy_ir_v1` starts with two unsigned 32-bit integers:

```text
u32be(canonical_encoding_version = 1)
u32be(policy_program.ir_version)
```

The canonical encoding version governs byte layout. `ir_version` governs PolicyIR
semantics. Both values enter the commitment.

### Primitive encodings

| Value | Canonical bytes |
| --- | --- |
| `u32` | Four bytes, unsigned, big-endian |
| `i64` | Eight bytes, two's-complement, big-endian |
| `bool` | `00` for false, `01` for true; reject every other tag |
| string | `u32be(byte_length)` followed by exact UTF-8 bytes |
| bytes | `u32be(byte_length)` followed by the bytes |
| optional | `00` for absent, or `01` followed by the encoded value |
| collection | `u32be(item_count)` followed by each encoded item |
| enum or node | One explicit `u8` tag followed by its fixed-layout payload |

No integer uses Protobuf varint or zig-zag encoding. No field uses host byte order.

### UTF-8 policy

Preserve the exact valid UTF-8 bytes stored in the Rust `String`. Perform no Unicode
normalization, case folding, whitespace conversion, or replacement.

Policy identifiers are already restricted to lowercase ASCII. Compiler metadata and
caveat string constants can contain non-ASCII text. Normalizing those strings could
change string-equality semantics relative to SpiceDB. Canonically equivalent Unicode
spellings therefore remain distinct policy values and produce different hashes.

### Fixed record layout

Encode record fields in this exact order:

- `PolicyProgram`: compiler name, compiler version, source schema digest, object
  definitions, permissions, caveats, limits. The header already encoded `ir_version`.
- `ObjectDefinition`: name, relations.
- `RelationDefinition`: name, allowed subject types.
- `AllowedSubjectType`: object type, optional subject relation, optional caveat name.
- `PermissionDefinition`: object type, name, expression.
- `CaveatDefinition`: name, parameters, expression.
- `CaveatParameter`: input source tag, name, value type tag.
- `PolicyLimits`: all eight limits in current model declaration order.

Record layouts contain no omitted default fields. Version 1 has no extension area.

### Tags

Permission expression tags are independent protocol constants:

| Tag | Node |
| --- | --- |
| `01` | relation reference |
| `02` | subject-relation reference |
| `03` | any-of children |
| `04` | all-of children |
| `05` | arrow |
| `06` | caveated relation |

Caveat expression tags are:

| Tag | Node |
| --- | --- |
| `01` | Boolean constant |
| `02` | integer constant |
| `03` | string constant |
| `04` | input reference |
| `05` | equal |
| `06` | not equal |
| `07` | less than |
| `08` | less than or equal |
| `09` | greater than |
| `0a` | greater than or equal |
| `0b` | and |
| `0c` | or |
| `0d` | not |

Input source tags are request context `01`, relationship context `02`, and capability
claim `03`. Value type tags are Boolean `01`, integer `02`, and string `03`. Unknown,
zero, or unsupported tags fail closed.

### Expression payloads

- Relation reference: name.
- Subject-relation reference: relation, subject relation.
- Any-of and all-of: collection of child expressions.
- Arrow: source relation, target permission.
- Caveated relation: relation, caveat name.
- Boolean, integer, and string constants: their primitive value.
- Input reference: source tag, name, value type tag.
- Binary caveat node: left expression, then right expression.
- And and or: collection of child expressions.
- Not: child expression.

### Collection ordering

The canonical encoder accepts only `ValidatedPolicyProgram`. It does not sort or
repair input.

Validation requires strict structural order for definitions, relations, allowed
subject types, permissions, caveats, parameters, and commutative expression children.
The encoder preserves that order. Binary left and right operands keep their semantic
order. Duplicate or unsorted values fail before encoding.

Version 1 uses explicit recursive comparison rules. It does not use Rust-derived enum
ordering:

- definitions by name and relations by name
- allowed subject types by object type, subject relation, then caveat name, with an
  absent option ordered before a present option
- permissions by object type, then name
- caveats by name
- parameters by input-source tag, then name
- commutative expression children by the protocol node tag above, then their encoded
  fields recursively

Integers use signed numeric order. Strings use exact UTF-8 byte order. A source enum or
model declaration reorder cannot change this protocol order.

Recursive collection comparison is lexicographic by child element. Compare each child
pair recursively from left to right. Use collection length only after every shared
position compares equal. The `u32` count prefix belongs to byte encoding and does not
change this structural ordering rule.

### Strict Protobuf ingress

Raw PolicyIR bytes from a compiler, registry import, file, or network boundary must
pass a strict ingress function before commitment:

1. Reject input larger than the caller's explicit PolicyIR byte ceiling.
2. Decode with `prost-reflect` against a checked-in, frozen v1 file descriptor set.
3. Recursively reject unknown fields and extensions in every nested message.
4. Decode the bytes as the generated `raziel.v1.PolicyProgram`.
5. Re-encode the decoded message with pinned Prost 0.14.4.
6. Require byte-for-byte equality with the input.
7. Convert into and validate the typed `PolicyProgram`.
8. Canonicalize and hash only the validated typed program.

The frozen descriptor is a versioned field allowlist. Normal code generation does not
replace it. A new Protobuf field remains unknown to canonical v1 until a reviewed
decision updates the protocol. Reflection alone is insufficient if it uses a newly
generated descriptor that already recognizes the field.

Version 1 embeds
`crates/policy-ir/descriptors/raziel-policy-ir-v1.binpb`. Its SHA-256 checksum is
`56164ff5efd57d1314ecd27426fe915f7c18596d7b3f0d3b650cf180066ebcc3`.
The reviewed regeneration command is:

```bash
./scripts/dev buf build \
  --path proto/raziel/v1/policy_ir.proto \
  --as-file-descriptor-set \
  --exclude-source-info \
  -o crates/policy-ir/descriptors/raziel-policy-ir-v1.binpb
```

Do not run this command as part of normal generation. Replacing the descriptor needs
the same protocol review as adding a canonical v1 field.

The equality check rejects duplicate singular fields, non-minimal varints, explicit
default fields, field reordering, and other alternate Protobuf spellings. The current
1,509-byte deterministic Go compiler golden must round-trip byte-for-byte through
Prost. This strict transport gate does not make Protobuf bytes a commitment input. The
hash still covers only the independent canonical encoding above.

Trusted code can construct and validate the Rust domain model directly. Unknown wire
fields cannot exist in that typed representation. Later registry and service work must
not decode untrusted PolicyIR bytes before the strict ingress gate.

ADR-0006 fixes `source_schema_digest` at 32 bytes. The validated Rust boundary must
store it as `[u8; 32]`, not a variable-length byte vector. Strict ingress rejects every
other length before canonical encoding.

## Vector Contract

Check in one versioned JSON vector set under `tests/vectors/`. Each positive case
contains:

- a stable case identifier
- human-readable PolicyIR input
- domain separator and hash algorithm
- canonical payload hex
- complete hash preimage hex
- expected 32-byte digest hex
- expected Rust result
- expected TypeScript parser result

Negative cases cover unknown top-level and nested Protobuf fields, alternate Protobuf
spellings, unsupported versions and tags, unsorted collections, optional presence
mutations, integer boundaries, UTF-8 byte distinctions, length mutations, and digest
mutations.

Rust owns encoding and hashing. TypeScript strictly parses and displays the vector
envelope, byte lengths, tags, and expected fields. The TypeScript parser remains under
`packages/proto-ts/test/` and is not exported in the production package. TypeScript
does not calculate `policy_hash`.

## Trust-Boundary Effect

- The policy authority approves the exact validated PolicyIR represented by the hash.
- The verifier resolves the hash through its local approved policy registry and
  recalculates it from the same typed program.
- Agents receive only `policy_hash`, never PolicyIR or canonical policy bytes.
- Relationship tuples, caveat runtime values, satisfying paths, and Merkle openings do
  not enter this encoding.

## Dependencies

- Add exact `sha2 0.10.9` to `raziel-canonical` for SHA-256.
- Move exact `prost 0.14.4` from a test-only edge to a normal
  `raziel-policy-ir` dependency for strict decode and re-encode.
- Add exact `prost-reflect 0.16.5`, with default features disabled, as an optional
  `raziel-policy-ir` dependency for descriptor-bound unknown-field inspection.
- Add a non-default `strict-protobuf` feature to `raziel-policy-ir`. This feature owns
  the optional reflection dependency and strict byte-ingress API. Host tools and
  services enable it. `raziel-canonical` and the SP1 guest do not.
- Add exact `serde 1.0.228` and `serde_json 1.0.145` as
  `raziel-canonical` test dependencies for the shared JSON vectors.
- Add the local `raziel-policy-ir` crate dependency to `raziel-canonical`.
- Add no Unicode, hex, or new TypeScript package.

`sha2`, `prost`, `serde`, and `serde_json` already exist in `Cargo.lock`.
The maintainer approved these dependency changes on 2026-07-22. `prost-reflect` is
the only new package in the lockfile.

## Migration Cost

No approved policy registry, snapshot, proof, or external consumer exists yet. The
first accepted vectors establish version 1. Later layout changes need a new canonical
encoding version, domain, vectors, and registry migration.

## Test Plan

- Positive bytes and digest vectors for every PolicyIR field and node tag.
- `i64::MIN`, `-1`, `0`, `1`, and `i64::MAX` byte vectors.
- Absent and present optional-field vectors.
- Exact UTF-8 vectors with canonically equivalent but byte-distinct strings.
- One-byte mutation checks for every major record group.
- Strict Protobuf rejection for unknown, duplicate, reordered, and non-minimal input.
- Frozen-descriptor checks for unknown fields at top-level and nested positions.
- Exact 32-byte source schema digest acceptance and adjacent-length rejection.
- Cross-language byte round-trip for the reviewed deterministic Go compiler golden.
- Rust-versus-checked-in canonical byte comparison.
- TypeScript vector parser acceptance and mutation rejection.
- Workspace format, lint, typecheck, test, build, and generation gates.

## Rejected Options

### Hash deterministic Protobuf

This binds the security protocol to Protobuf serialization and generated-library
behavior. It also conflicts with the established raw-Protobuf invariant.

### Unicode NFC normalization

Normalization can make two strings hash identically even though SpiceDB and Rust
compare their original code points as distinct values. Exact UTF-8 preservation keeps
the commitment aligned with evaluation semantics.

### Variable-width signed integers

Variable width needs extra minimality rules and cross-language edge-case handling.
Fixed eight-byte two's-complement encoding is total over PolicyIR `i64` values.

### TypeScript hashing

A second hashing authority expands the proof-critical implementation surface. The
baseline TypeScript package parses vectors and displays Rust-owned results only.

### Decode and re-encode without a frozen descriptor

This rejects fields unknown to the current generated type. A later generated type can
recognize a new field and silently pass it through before canonical v1 supports it.

### Manually scan Protobuf wire fields

A custom recursive wire parser duplicates Protobuf parsing and adds malformed-input
risk. The frozen descriptor provides the independent v1 allowlist through maintained
Protobuf parsing code.
