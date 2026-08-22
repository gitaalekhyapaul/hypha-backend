# Engineering Baton History

- Baton ID: B-0022
- State: REVIEW
- Milestone: M4, canonical positive-path witness construction
- Owner: Codex
- Started UTC: 2026-08-22T09:01:08Z
- Last updated UTC: 2026-08-22T09:09:59Z
- Branch: `main`
- Worktree: B-0022 implementation, current-status updates, archive, and B-0023 reset
  are ready to commit on `main` after `5b2f84c`. The branch remains four commits
  ahead of `origin/main` before this commit. B-0021 is archived at
  `docs/agent/history/B-0021-bullmq-proof-supervisor-handoff.md`.

## Objective

Implement the bounded host-side builder that evaluates approved PolicyIR against a
selected blinded relationship snapshot and returns the minimum canonical
`RelationshipWitnessSetV1` for the first positive path. Validate the witness set
before returning it. Do not run SP1 or connect the builder to the proof supervisor in
this baton.

## Acceptance Criteria

- [x] The builder indexes selected snapshot relationships through the existing
      PolicyIR resolver contract without changing evaluator semantics.
- [x] One allowed request selects the evaluator's first canonical positive path and
      includes only relationships reported as used by that evaluation.
- [x] Used relationships map to the exact snapshot indices and Merkle openings that
      commit to the selected root.
- [x] The returned witness set has deterministic ordering, no duplicate structural
      identity, and passes the existing snapshot witness validator.
- [x] Denied, missing, ambiguous, malformed, oversized, and commitment-mismatched
      inputs fail through typed errors with no private values in debug or display
      output.
- [x] Focused witness tests, affected cross-component tests, strict Clippy,
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
- Made: Use each validated snapshot leaf index as the PolicyIR `witness_id`. This
  matches the guest resolver and gives a direct opening lookup after evaluation.
- Made: Build against the caller-selected relationship set, salt, and expected root.
  Rebuild and compare the root before policy evaluation, then validate the reduced
  witness set before returning it.
- Made: Add only the existing internal `raziel-policy-ir` and `raziel-snapshot` crates.
  No new external package or version is needed.

## Open Questions and Blockers

- No approval blocker is known. The PolicyIR resolver identity and snapshot-index
  mapping are confirmed from the evaluator, snapshot tree, and guest resolver.
- No new dependency is planned.

## Work Completed

- B-0021 implemented and validated the BullMQ proof-supervisor claim and private
  handoff boundary.
- Read FR-040 through FR-046, the traced evaluator, snapshot tree construction,
  witness-set validation, and the guest's verified-witness resolver.
- Confirmed that sorted snapshot leaf indices already form the evaluator-to-opening
  identity contract.
- Implemented `build_positive_witness`. It takes ownership of one selected
  relationship vector, rebuilds the blinded tree, checks the expected root, evaluates
  PolicyIR, opens only used leaf IDs, and validates the reduced witness set.
- Added a snapshot-backed resolver with the same record-to-PolicyIR mapping as the
  guest's verified-witness resolver.
- Added closed typed errors for invalid snapshots, root mismatch, evaluation failure,
  denial, invalid traces, and invalid constructed witnesses.
- Moved the snapshot count ceiling ahead of resolver construction so excess input
  fails before private-field cloning.
- Added four focused integration cases. They cover first-union selection,
  intersection branches, a stored caveat, denial, root mismatch, malformed order,
  count excess, and private debug output.
- Updated current-state documents without claiming worker integration or proof work.

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
| `cargo test -p raziel-witness --all-targets --no-fail-fast` | PASS | Four focused builder cases | 2026-08-22T09:06:00Z |
| Affected four-crate test run | PASS, 68 active tests | PolicyIR, snapshot, authorization-core, and witness | 2026-08-22T09:07:00Z |
| `cargo clippy -p raziel-witness --all-targets -- -D warnings` | PASS | Strict witness lint | 2026-08-22T09:07:58Z |
| Pinned-container non-guest workspace | PASS, 202 active tests | Full Rust workspace. Seventeen proof and live-service tests ignored | 2026-08-22T09:09:00Z |
| Pinned-container `pnpm format:check` | PASS | Biome, Rustfmt, and Gofmt | 2026-08-22T09:09:00Z |
| `cargo tree -p raziel-witness --depth 1` | PASS | Only the existing PolicyIR and snapshot crates are direct dependencies | 2026-08-22T09:09:59Z |
| Production logging and stale-claim scans | PASS | No logging macro and no superseded current-state claim | 2026-08-22T09:09:59Z |
| `git diff --check` and file-scope review | PASS | B-0022 patch hygiene and bounded scope | 2026-08-22T09:09:59Z |

## Privacy Checks

- PASS: Builder errors and witness debug output omit relationships, caveat
  values, salts, paths, openings, and trace data.
- PASS: Production witness source has no logging macro. Public errors contain closed
  classes and no request or relationship value.
- NOT APPLICABLE: No real transaction data or proof run is part of this baton.

## Risks and Private-Data Notes

- Resolver identity and snapshot structural identity must match exactly. A positional
  shortcut can select the wrong opening.
- The evaluator can inspect more relationships than the selected positive path uses.
  The builder must include only the reported used witness IDs.
- Witness order and duplicate handling are proof inputs. The builder must defer to
  existing canonical validators rather than creating a second rule set.

## Exact Next Action

Archive B-0022, reset the current baton to registry-backed proof input assembly, and
commit the scoped witness feature.
