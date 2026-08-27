# Engineering Baton Archive

- Baton ID: B-0039
- State: REVIEW
- Milestone: M5, isolated policy-authority signing service
- Owner: Codex
- Started UTC: 2026-08-26T20:22:33Z
- Completed UTC: 2026-08-26T20:43:24Z
- Branch: `main`
- Worktree: Approved M5 work remains uncommitted.

## Objective

Implement the ADR-0018 owner-only policy-authority process that reloads one fixed
candidate, derives the next policy epoch from the trusted registry, signs exact
ADR-0010 claims, and persists one idempotent staged policy record.

## Acceptance Criteria

- [x] `inspect-policy` writes a strict `0600` private candidate manifest beside the
      safe projection without adding PolicyIR to browser output.
- [x] The authority request carries only candidate ID, expected hash and epoch, the
      reviewed validity window, operator identity, and one 32-byte approval nonce.
- [x] The authority derives candidate paths, trust domain, metadata, registry state,
      approval epoch, and signing key identity from fixed local state.
- [x] Path selection, stale epochs, duplicate records, expiry, wrong keys, altered
      hashes, malformed frames, oversized frames, and trailing bytes fail closed.
- [x] Success creates one signed staged record with unset mutable registry metadata.
- [x] A same-nonce retry returns the same record. A different nonce cannot replace it.
- [x] The key and raw nonce stay out of responses, errors, logs, environment values,
      and candidate files. Public debug output redacts nonce and PolicyIR-bearing data.
- [x] Focused tests, Rustfmt, Clippy, and `git diff --check` pass.

## Architecture Constraints

- The authority never writes the trusted registry or contacts SpiceDB.
- Callers cannot supply PolicyIR, local paths, trust domain, epoch allocation, key
  identity, or mutable registry metadata.
- The staged record remains untrusted until the registry writer commits it.
- Public proof values, PolicyIR semantics, guest code, and public Protobuf contracts
  did not change.

## Decisions

- Persist SHA-256 of the approval nonce, not the raw nonce, beside the staged record.
- Use private canonical Protobuf files and one length-prefixed Unix-socket request.
- Include the reviewed validity window in the request, then require an exact match
  with the private candidate manifest as specified by ADR-0018.
- Revalidate stored candidate files on an idempotent retry, but allow the same signed
  result to be recovered after the candidate approval window closes.

## Work Completed

- Added the `raziel-policy-authority` binary, owner-only socket server, client, wire
  codec, fixed-path candidate store, registry-derived epoch allocation, and signing.
- Extended `inspect-policy` with the private canonical candidate manifest.
- Added strict private-directory and file-mode checks, same-file checks, canonical
  frame checks, immutable staged output, and generic public failures.
- Added real Ed25519, trusted-registry, replay, mutation, and Unix-socket tests.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev cargo test -p raziel-admin` | PASS, 14 tests | Admin, authority, and writer | 2026-08-26T20:40:45Z |
| `./scripts/dev cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | All administration targets | 2026-08-26T20:41:02Z |
| `./scripts/dev cargo fmt --all -- --check` | PASS | Rust workspace formatting | 2026-08-26T20:41:08Z |
| Private-field and logging scan | PASS | Authority source and public errors | 2026-08-26T20:43:24Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T20:43:24Z |

## Risks And Private-Data Notes

- The UI and owner adapter originate a candidate. Local storage does not make it
  trusted. The authority recomputes PolicyIR-derived fields and validates every
  signed field before using the key.
- The staged record carries verifier-visible PolicyIR over an owner-only socket. It
  cannot enter a browser response or become trusted without registry publication.

## Exact Next Action

Implement `AppendApprovedPolicy` in the registry writer and set mutable registry
metadata only after full signature, policy, trust-domain, epoch, and transition checks.
