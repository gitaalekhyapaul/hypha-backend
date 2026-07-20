# ADR-0006: Source Schema Digest

- Status: accepted
- Date: 2026-07-20
- Milestone: M1

## Context

`PolicyProgram.source_schema_digest` records which SpiceDB schema source produced a
PolicyIR candidate. The wire contract requires the field, but it did not define the
input bytes or hash algorithm.

The compiler adapter accepts a `.zed` schema file as bytes. An unambiguous digest rule
is needed for reproducible output and source review.

## Decision

The compiler adapter calculates:

```text
source_schema_digest = SHA-256(schema_file_bytes)
```

`schema_file_bytes` is the exact byte sequence read from the path supplied through
`--schema`. The adapter performs no newline conversion, Unicode conversion, comment
removal, whitespace normalization, parsing, or reserialization before hashing.

The adapter stores the raw 32-byte SHA-256 result in
`PolicyProgram.source_schema_digest`. The source path is not part of the digest.

The digest records source provenance. It does not approve the policy, authenticate a
SpiceDB revision, commit relationship tuples, or contain runtime caveat values. The
policy authority still reviews and approves canonical PolicyIR. The later
`policy_hash` binds that approved PolicyIR under its own domain-separated encoding.

A future digest algorithm or normalization rule requires a versioned migration and a
new decision record.

## Consequences

- Identical schema bytes produce the same digest on every supported machine.
- Any byte change produces a new digest candidate, including comments, spaces, and
  newline style.
- Moving or renaming an unchanged schema file does not change the digest.
- Review tools can compare the digest against an exact source artifact.
- The implementation uses Go's standard `crypto/sha256` package and adds no module
  dependency.

## Rejected Options

### Normalize schema text

Normalization introduces another representation and can drift from SpiceDB parser
behavior. It also loses exact source provenance.

### Hash compiled SpiceDB Protobuf

Compiled Protobuf records depend on compiler internals and serialization choices. The
field identifies source input, not compiler output.

### Include the source path

Paths vary across machines and do not change schema semantics.
