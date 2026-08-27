# Engineering Baton Archive

- Baton ID: B-0040
- State: REVIEW
- Milestone: M5, approved-policy registry publication
- Owner: Codex
- Started UTC: 2026-08-26T20:43:24Z
- Completed UTC: 2026-08-26T20:51:38Z
- Branch: `main`
- Worktree: Approved M5 work remains uncommitted.

## Objective

Extend the owner-only registry writer with ADR-0018 `AppendApprovedPolicy` so it
validates one staged authority record, supplies mutable registry metadata from trusted
local state, and atomically publishes the next complete registry generation.

## Acceptance Criteria

- [x] The private request carries expected registry generation and one staged policy.
- [x] Staged status, registration, revocation, and generation fields must be unset.
- [x] The writer validates PolicyIR, hash, signature, trust domain, key, validity,
      next policy epoch, and unused record ID.
- [x] Trusted time, `APPROVED`, and the next generation come only from the writer.
- [x] The complete generation and `current.pb` publish atomically.
- [x] Stale generation, duplicate record, invalid signature, wrong trust domain,
      altered hash, stale epoch, expiry, wrong key, and mutable fields fail closed.
- [x] Existing snapshot append and revoke tests remain compatible.
- [x] Focused tests, Rustfmt, Clippy, and `git diff --check` pass.

## Architecture Constraints

- The writer verifies signatures but never receives a private key or signs claims.
- PolicyIR remains on the owner-only protocol and trusted registry path.
- Public proof values, hash formats, PolicyIR semantics, and guest behavior did not
  change.

## Decisions

- Extended the existing private Protobuf operation and response. No public wire type
  or external service port was added.
- Returned policy results use a redacted `Debug` view so logs cannot print PolicyIR.
- Required the signing key and policy to be current at the writer's trusted time.

## Work Completed

- Added `AppendApprovedPolicy` to the single registry-writer operation loop.
- Added staged-record conversion, trusted mutable metadata, epoch allocation checks,
  current-key verification, complete-generation publication, and client support.
- Added store, mutation, socket, redaction, and snapshot-compatibility tests.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev cargo test -p raziel-admin` | PASS, 17 tests | Authority and registry writer | 2026-08-26T20:51:09Z |
| `./scripts/dev cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | All administration targets | 2026-08-26T20:51:21Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Rust workspace formatting | 2026-08-26T20:51:22Z |
| Logging and private-field scan | PASS | Writer source and response debug | 2026-08-26T20:51:38Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T20:51:38Z |

## Risks And Private-Data Notes

- The committed response contains verifier-visible PolicyIR. Its public Rust debug
  view is redacted, and future owner routes must not return the record to browsers.
- Publication is serialized by the existing owner-only writer process.

## Exact Next Action

Implement the fixed-path SpiceDB candidate write and private ZedToken handoff.
