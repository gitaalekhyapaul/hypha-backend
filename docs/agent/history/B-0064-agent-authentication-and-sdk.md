# B-0064: Agent Authentication And SDK Foundation

- Outcome: implemented and locally validated
- Started UTC: 2026-09-25T08:20:51Z
- Completed UTC: 2026-09-26T19:13:56Z
- Baseline: `c0310a4`, branch `main`
- Approval: ADR-0020 accepted by the maintainer, "Approved, please code now"
- Publication state: local worktree changes only, no commit, push, VM deployment, or npm publication

## Delivered

- Operator Agents view: register a raw Ed25519 public key, inspect registrations,
  create a one-time-visible API token, inspect expiry/status, and revoke tokens.
- Redis-backed registration and token metadata. Bearer secrets are hashed. Atomic
  admission rechecks token revocation and key binding before consuming a nonce.
- Rust domain-separated canonical signature encoding, strict key/signature checks,
  bounded `raziel-agent-auth` helper, and deterministic cross-language vectors.
- Signed `/api/agent/v1/me` and separate operator routes with session/CSRF checks.
- Standalone signer/client source in sibling `raziel-sdk-ts`. The backend's SDK is
  now a compatibility re-export of the pinned package candidate, not duplicate code.
- Compose wiring and helper preparation. Node entry point preserves raw HTTP targets
  and rejects duplicate authentication headers before framework normalization.
- [User and runtime runbook](../../runbooks/AGENT-REGISTRATION.md).

## Scope And Ownership

Coordinator owned facade, Redis, integration, configuration, current docs, and baton.
Rust and UI helpers edited disjoint scopes. SDK helper staged a patch and package.
Maintainer-approved filesystem escalation applied the SDK patch to the sibling repo.
The existing sibling LICENSE and unrelated `.DS_Store` were preserved.

Backend file map: `crates/{canonical,authority}/src/agent_api.rs`, Rust tests and public
vector, `tools/raziel-admin` auth helper, `proto/raziel/v1/agent_api.proto`, console
`lib/agent-auth`, agent/operator routes, Agents component/client/tests, `server.mjs`,
`server/agent-transport.ts`, SDK compatibility package, Compose, and preparation script.

SDK candidate: `@gitaalekhyapaul/raziel-sdk-ts@0.1.0`.
Tarball SHA-256: `21502ba4879122252d67fa2ba08eac4d7fb058067943ada6d1503a69f3740674`.
No new dependency version was introduced. Existing pinned tools supplied validation.

## Validation

| Check | Result | Evidence |
| --- | --- | --- |
| Rust focused encoding, authority, helper, and vector tests | PASS | 42 tests reported by Rust helper |
| Final helper invalid-input CLI tests | PASS | 7 tests, invalid exit 2 has empty stdout/stderr |
| Rust focused Clippy and formatting | PASS | Rust helper report |
| Buf generation and proto-ts build | PASS | New public identity DTO |
| Standalone SDK build, types, lint, format, vectors, native HTTP, and package smoke | PASS | 133 SDK tests, public tarball has 16 files |
| Backend SDK compatibility tests | PASS | 6 tests |
| Gateway tests | PASS | 20 tests |
| Console suite, including actual SDK/Rust/Redis HTTP test | PASS | 127 tests in 17 files |
| Production Next build and TypeScript | PASS | All new routes, including revoke, built |
| Development HTTP entry point | PASS | Root returned 200 on internal port 3002, process stopped |
| Console lint and scoped format checks | PASS | No remaining violations |
| Compose apps configuration | PASS | `./scripts/compose --profile apps config --quiet` |
| Desktop registration, token display, revoke, and narrow-layout check | PASS | UI helper browser check with mocked transport |
| Final integrated in-app browser check | NOT AVAILABLE | Browser reported connection refused, host HTTP probe returned 200 |
| Diff whitespace check | PASS | `git diff --check` |
| NUS VM, proof generation, npm publication | SKIP | Outside B-0064 |

Final HTTP gate used a real production server, Linux Rust helper, Redis, and the
packaged SDK. It covered two registered keys, wrong-key/token rejection, metadata
without secrets, cross-owner revoke rejection, concurrent replay, rotation,
revocation, operator/agent isolation, CSRF, duplicate Authorization, empty query,
and a normalized path with forged internal metadata. No proof job was started.

## Resolved Failures

- Local preview initially received a macOS helper binary. Built a Linux helper in
  Docker. Internal helper startup errors now return unavailable, not invalid key.
- Next development routing missed the newly added deep revoke route. Production
  build included it and passed the live route test.
- Independent review found duplicate Authorization normalization and empty-query
  normalization. Added the raw HTTP boundary and passed native HTTP regression checks.
- SDK validation on the host could not load Linux-native dependencies. Docker checks passed.

## Trust And Remaining Work

Authentication grants no dataset access. The guest, proof public values, existing
capability format, verifier independence, and protected-query checks are unchanged.
No private witness or credential entered Git or validation output. Test-only agent
registrations remain in the isolated local audience. Successful tests revoke their tokens.

The access inbox, policy-version storage/schema migration, request-triggered snapshot
preparation, durable serial dispatcher, final issuance exchange, protected-query
facade, npm release, and two agent dashboards remain unimplemented.

Exact next action: B-0065 implements owner-qualified access intents and operator review,
then pins immutable policy packages for request preparation under ADR-0020.
