# Current Engineering Baton

- Baton ID: B-0038
- State: REVIEW
- Milestone: M5, non-signing policy inspection and safe projection
- Owner: Codex
- Started UTC: 2026-08-26T20:16:10Z
- Last updated UTC: 2026-08-26T20:22:33Z
- Branch: `main`
- Worktree: Approved M5 docs plus completed B-0034, B-0035, and B-0037 features are uncommitted.

## Objective

Add a non-signing `raziel-admin inspect-policy` command that validates compiled
PolicyIR, calculates `policy_hash`, checks the selected entrypoint and review
metadata, and emits only the ADR-0018 browser-safe projection.

## Acceptance Criteria

- [x] The command strictly decodes PolicyIR under current trusted ceilings and
      calculates the canonical policy hash.
- [x] The command verifies that the selected resource and `query` permission exist.
- [x] Candidate IDs, policy metadata, approval epoch, and time windows are bounded.
- [x] The projection contains only ADR-0018 review fields and no PolicyIR body,
      schema, compiler diagnostics, authority material, relationship data, or paths.
- [x] The command never loads a signing key or creates a signature.
- [x] Positive, malformed, mismatched-entrypoint, invalid-window, unknown-option,
      output-atomicity, and projection-redaction tests pass.
- [x] Rust formatting, Clippy, focused tests, and `git diff --check` pass.

## Architecture Constraints

- Policy hash ownership stays in `crates/canonical`.
- PolicyIR strict ingress stays in `crates/policy-ir`.
- Browser output cannot include PolicyIR or compiler diagnostics.
- This command cannot approve, sign, register, activate, or revoke a policy.
- Public proof values, guest semantics, and wire contracts do not change.

## File Scope

- `tools/raziel-admin/src/lib.rs`
- `tools/raziel-admin/src/policy.rs`
- `tools/raziel-admin/src/projection.rs`
- `tools/raziel-admin/src/error.rs`
- `tools/raziel-admin/src/io.rs`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0038-non-signing-policy-inspection.md`

## Out Of Scope

- Policy-authority process and signing protocol
- Registry-writer policy append
- SpiceDB writes and revision handoff
- Next.js process orchestration or routes
- Protobuf changes and new dependencies

## Decisions

- Inherited: ADR-0018 requires a non-signing safe policy projection.
- Local: the command accepts candidate review metadata through fixed CLI options and
  derives compiler version, source schema digest, and policy hash from PolicyIR.
- Local: output uses create-new atomic writes and never replaces an existing review.

## Open Questions And Blockers

- None.

## Work Completed

- B-0037 archived after the typed draft and renderer passed its gates.
- Confirmed that current `approve-policy` combines hash calculation and signing, so a
  separate non-signing command is required.
- Live inspection found that shared atomic outputs inherited `0644` from the process
  umask instead of the ADR-0018 owner-only `0600` mode.
- Added `inspect-policy` with strict PolicyIR ingress, canonical hash calculation,
  entrypoint checks, bounded review metadata, and create-new safe JSON output.
- Added positive, malformed, entrypoint, time-window, unknown-option, output
  atomicity, and projection-redaction tests.
- Changed shared atomic administration outputs to request `0600` at file creation and
  added Unix mode assertions.

## Validation Plan

- Add focused module tests around the command and safe JSON projection.
- Run `cargo test -p raziel-admin`, Rustfmt, and Clippy through the pinned toolchain.
- Scan projection keys and output for private or signing material.
- Run `git diff --check`.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev cargo test -p raziel-admin` | PASS, 9 tests | Administration CLI and registry writer | 2026-08-26T20:20:37Z |
| `./scripts/dev cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Administration targets | 2026-08-26T20:20:53Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Rust workspace formatting | 2026-08-26T20:21:52Z |
| Live `inspect-policy` on generated `dataset#query` PolicyIR | PASS | B-0037 compiler output and safe projection | 2026-08-26T20:21:32Z |
| Output mode check | PASS, `0600` | Live review projection | 2026-08-26T20:21:42Z |
| Projection private-field scan | PASS | Live review projection | 2026-08-26T20:21:42Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T20:21:42Z |

## Risks And Private-Data Notes

- A full policy projection would expose verifier-only PolicyIR to the browser.
- Caller-provided resource and permission values must match validated PolicyIR.
- Candidate review metadata is not signed at this stage. The later authority must
  reload the candidate and recompute all signed claims.

## Exact Next Action

Archive B-0038 and open the separate policy-authority signing baton.
