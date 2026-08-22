# Current Engineering Baton

- Baton ID: B-0022
- State: READY
- Milestone: M4, canonical positive-path witness construction
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T08:56:28Z
- Branch: `main`
- Worktree: B-0021 is archived at
  `docs/agent/history/B-0021-bullmq-proof-supervisor-handoff.md`. Its scoped
  implementation and the B-0022 reset await one commit after `598ba02`.

## Objective

Implement the bounded host-side builder that evaluates approved PolicyIR against a
selected blinded relationship snapshot and returns the minimum canonical
`RelationshipWitnessSetV1` for the first positive path. Validate the witness set
before returning it. Do not run SP1 or connect the builder to the proof supervisor in
this baton.

## Acceptance Criteria

- [ ] The builder indexes selected snapshot relationships through the existing
      PolicyIR resolver contract without changing evaluator semantics.
- [ ] One allowed request selects the evaluator's first canonical positive path and
      includes only relationships reported as used by that evaluation.
- [ ] Used relationships map to the exact snapshot indices and Merkle openings that
      commit to the selected root.
- [ ] The returned witness set has deterministic ordering, no duplicate structural
      identity, and passes the existing snapshot witness validator.
- [ ] Denied, missing, ambiguous, malformed, oversized, and commitment-mismatched
      inputs fail through typed errors with no private values in debug or display
      output.
- [ ] Focused witness tests, affected cross-component tests, strict Clippy,
      formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- Relationship tuples, satisfying paths, Merkle openings, salts, caveat values, and
  evaluation traces remain Raziel-private witness data.
- The baseline guest proves `ALLOW` only. A denied evaluation cannot produce a
  witness set.
- The builder consumes approved PolicyIR and a selected snapshot. It cannot approve
  either input or choose a different policy or snapshot.
- Existing PolicyIR semantics, snapshot leaf construction, canonical encodings,
  commitments, public values, and capability fields do not change in this baton.
- Scenario adapters cannot alter the builder or guest semantics.
- No private witness value enters logs, public errors, Redis, Tonic, or client data.

## File Scope

- `crates/witness/`
- `crates/witness/Cargo.toml`
- `Cargo.lock` only for existing internal workspace selections
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Proof-supervisor wiring or a production worker binary
- One-shot child-process isolation, cgroups, or SP1 execution
- Capability packaging or terminal queue publication
- Policy and snapshot registry loading
- `GetCapability`, `CancelCapability`, `verifierd`, ADR-0002, or O-007
- M5, real transaction data, M6 acceptance, M7, or another scenario
- New external dependencies, package versions, toolchains, or proof runs

## Decisions

- Inherited: The architecture keeps PolicyIR, relationship tuples, selected paths,
  Merkle openings, salts, and caveat values inside the data-owner proof boundary.
- Inherited: ADR-0009 fixes blinded snapshot leaves, one snapshot salt, left-balanced
  Merkle openings, and canonical witness limits.
- Inherited: ADR-0010 fixes first-canonical-positive-path trace selection and bounded
  witness accounting.
- Inherited: ADR-0013 places witness construction in the local proof path and bars
  hosted proving.
- Made: None.

## Open Questions and Blockers

- No approval blocker is known. The first implementation step must confirm the exact
  PolicyIR resolver identity and snapshot-index mapping from current code.
- No new dependency is planned.

## Work Completed

- B-0021 implemented and validated the BullMQ proof-supervisor claim and private
  handoff boundary.
- No B-0022 implementation has started.

## Validation Plan

- Run focused `raziel-witness` tests for one allowed path, canonical selection,
  caveat use, denied evaluation, malformed snapshot input, and private error output.
- Run affected PolicyIR, snapshot, authorization-core, and witness tests.
- Run the non-guest workspace gate after the focused feature checks pass.
- Run strict Clippy, repository formatting, privacy scans, and `git diff --check`.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0021 archive | PASS | BullMQ supervisor claim and private handoff | 2026-08-22T08:56:28Z |
| B-0022 implementation | NOT STARTED | No feature files edited | 2026-08-22T08:56:28Z |

## Privacy Checks

- NOT STARTED: Builder errors and debug output must omit relationships, caveat values,
  salts, paths, openings, and trace data.
- NOT APPLICABLE: No real transaction data or proof run is part of this baton.

## Risks and Private-Data Notes

- Resolver identity and snapshot structural identity must match exactly. A positional
  shortcut can select the wrong opening.
- The evaluator can inspect more relationships than the selected positive path uses.
  The builder must include only the reported used witness IDs.
- Witness order and duplicate handling are proof inputs. The builder must defer to
  existing canonical validators rather than creating a second rule set.

## Exact Next Action

Read `crates/witness`, the PolicyIR traced evaluator, snapshot tree and witness types,
their tests, and the relevant FR-040 through FR-046 requirements. Then claim B-0022
with a bounded implementation file map.
