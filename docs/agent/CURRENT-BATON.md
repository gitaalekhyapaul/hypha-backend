# Current Engineering Baton

- Baton ID: B-0005
- State: READY
- Milestone: M1, PolicyIR and SpiceDB compiler adapter
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-18T18:36:34Z
- Branch: `main`
- Worktree: Expected clean after the scoped B-0004 commit. Verify before claiming.

## Objective

Implement the Go adapter from maintained SpiceDB compiler Protobuf output into the
approved PolicyIR subset. Defer live SpiceDB equivalence to a later M1 baton.

## Acceptance Criteria

- [ ] The adapter consumes maintained SpiceDB compiler output, not ad hoc schema text.
- [ ] Supported relations, subject sets, unions, intersections, one arrow, and named
      caveats translate into typed PolicyIR.
- [ ] Exclusion, recursion, wildcards, unbounded traversal, unsupported CEL, and
      ambiguous target types fail closed.
- [ ] Output follows ADR-0004 structural ordering and is deterministic.
- [ ] Supported and rejected compiler fixtures have focused Go tests.
- [ ] Go tests, Buf generation, and affected workspace gates pass.

## Architecture Constraints

- Follow maintained SpiceDB compiler semantics and Protobuf output.
- Do not reimplement the SpiceDB parser.
- Keep the adapter build-time only.
- Preserve the PolicyIR node set and evaluator semantics from ADR-0004.
- Reject unsupported constructs. Never approximate them.
- Keep policy translation separate from dataset relevance.
- Add no dependency without user approval.

## File Scope

- `tools/spicedb-ir-compiler/`
- `go.mod` and `go.sum` after dependency approval
- focused M1 compiler fixtures under `tests/`
- `docs/agent/CURRENT-BATON.md`
- a decision record only for a durable choice not settled by approved semantics

## Out of Scope

- SpiceDB-versus-PolicyIR live equivalence harness
- canonical commitment bytes or hashes
- snapshots, witnesses, Merkle trees, or SP1 guest changes
- service, gateway, SDK, or console integration
- payment-reconciliation data and scenario work

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0004.
- Made: None.

## Open Questions and Blockers

- Exact SpiceDB Go compiler module versions need inspection and user approval before
  they enter `go.mod`.
- Production PolicyIR ceilings wait for M3 cycle measurement.
- ADR-0002 still needs maintainer approval before gateway integration.
- O-001 remains open for the later real-data acceptance run.

## Work Completed

- B-0004 is archived as
  `docs/agent/history/B-0004-policy-ir-wire-and-rust-semantics.md`.
- The typed PolicyIR contract, Rust validator, and pure evaluator are ready.

## Validation Plan

- focused Go compiler-adapter tests
- supported and rejected fixture tests
- deterministic output comparison
- `./scripts/dev go test ./...`
- `./scripts/dev pnpm generate:check`
- affected workspace format, lint, typecheck, and test gates
- `git diff --check`

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Not run | NOT AVAILABLE | B-0005 | 2026-07-18T18:36:34Z |

## Risks and Private-Data Notes

- Compiler drift can change accepted syntax or compiled graph shapes.
- Fixtures must contain policy examples only and no transaction data.
- Diagnostic output must not contain relationship tuples or caveat values.
- Unknown compiler nodes must produce typed rejection.

## Exact Next Action

Read the current Go adapter shell and pinned SpiceDB compiler APIs. Identify exact Go
modules and versions, then request dependency approval before editing `go.mod`.
