# SpiceDB and PolicyIR Equivalence

Run the live harness from the repository root:

```bash
./scripts/test-equivalence
```

The command starts the profile-gated `spicedb-equivalence` service, runs both engines
inside the pinned toolchain container, compares their case decisions, and removes the
test service. It does not stop or modify the normal `spicedb` service.

## Fixture boundary

`fixtures/v1.json` is the single source for test relationships, caveat values,
authorization requests, and expected decisions. Its paths bind each run to:

- the exact maintained SpiceDB `.zed` schema
- the compiler provenance configuration
- the reviewed PolicyIR Protobuf generated from those schema bytes
- the SHA-256 digest of the exact schema bytes

Relationship caveat context stays attached to relationship tuples. Request context
and capability claims remain separate in the fixture and in Rust. The SpiceDB runner
merges those two evaluation-time sources only for `CheckPermission`, whose API exposes
one context object. Duplicate names across the two sources fail the harness.

The fixture can declare `CONDITIONAL` only for an explicit CEL partial-evaluation
probe. The Go runner accepts it only when SpiceDB returns conditional for that exact
case. The Rust test host accepts it only for `MissingCaveatInput`. An unexpected
conditional result still fails with a typed error. Production authorization remains
Boolean-or-error and does not gain a conditional response surface.

The fixture contains no PolicyIR expression, CEL expression, satisfying path, Merkle
opening, production relationship, or production caveat value. Diagnostics report case
identifiers and decisions only.

These synthetic equivalence cases satisfy an M1 test gate. They do not satisfy the
real-data reconciliation gate in M6.
