# Engineering Baton Archive

- Baton ID: B-0025
- Final state: completed and archived
- Milestone: M4, isolated prover launcher and terminal result validation
- Owner: Codex
- Started UTC: 2026-08-23T04:38:30Z
- Completed UTC: 2026-08-23T05:35:06Z
- Branch: `main`
- Base commit: `64ecc91`

## Objective

Connect the BullMQ proof supervisor to the fixed ADR-0015 Docker sandbox launcher.
Enforce one-child execution, deadline, cancellation, cleanup, and confirmed exit.
Validate every returned proof result before terminal success or production activation.

## Acceptance Criteria

- [x] The rendered sandbox has the exact 60,129,542,144-byte memory and memory-swap
      caps, four CPUs, no network, read-only root, fixed read-only trusted mounts,
      owner-only socket, and bounded scratch controls.
- [x] The fixed launcher handles one bounded request and starts one fixed child at a
      time without Docker-socket or cgroup-administration access in the supervisor.
- [x] The supervisor applies the one-hour deadline, forwards cancellation, requires
      an exact correlated cancellation acknowledgment, confirms exit within 30
      seconds, and removes per-job storage before terminal validation.
- [x] Crash, socket close, malformed output, timeout, cancellation, host denial,
      witness failure, proof failure, journal mismatch, and resource failure close the
      job without proof retry or private output.
- [x] Success requires direct Plonk verification plus exact public journal, request,
      snapshot, policy, root, time, nullifier, and trusted program-key checks.
- [x] Production startup can install only the fixed Unix-socket executor and terminal
      validator.
- [x] Focused process, service, Redis, Compose, feature, formatting, lint, privacy,
      and diff gates pass.

## Architecture Constraints

- ADR-0013 fixes the API-job topology, BullMQ lifecycle, one-hour deadline, 30-second
  exit grace, one attempt, zero stalled retries, and cleanup order.
- ADR-0015 fixes the private Unix-socket protocol and Docker sandbox controls.
- The supervisor receives no Docker socket or host cgroup authority.
- The launcher and prover have no network. Request data cannot select trusted paths,
  keys, proof mode, limits, or outputs.
- Private envelopes, relationships, salts, caveat values, paths, openings, witnesses,
  and guest input never enter Redis, public results, or standard logs.
- A real SP1 proof run remains separately approval-gated.

## File Scope

- `.env.example`, `.gitignore`, and `docker-compose.yml`
- `services/razield/Cargo.toml`
- `services/razield/src/bin/`
- `services/razield/src/one_shot/`
- `services/razield/src/worker/`
- `scripts/check-proof-sandbox-compose`
- `scripts/prepare-proof-sandbox-runtime`
- `scripts/test-bullmq-conformance`
- Current-state architecture, specification, README, and baton documents

## Decisions

- Inherited: ADR-0012 fixes local CPU Plonk proving and key identity.
- Inherited: ADR-0013 fixes service roles, queue lifecycle, limits, deadlines, cleanup,
  and activation gating.
- Inherited: ADR-0015 fixes the persistent proof sandbox, owner-only socket, strict
  child protocol, trusted mounts, and process test requirements.
- Made: Default `razield` activates verification only. The `one-shot-prover` feature
  activates SP1 SDK proving for the isolated child build.
- Made: Production supervisor startup constructs the fixed executor and validator
  inside `razield`. Callers cannot inject either component.
- Made: The proof profile publishes only the launcher, prover child, and guest ELF
  from the pinned offline build into a read-only runtime volume.
- Made: Cancellation completion requires an exact `Cancelled` failure and matching
  correlation ID. Socket and parent ownership and modes are checked per connection.
- Made: BullMQ success stores a complete public capability Protobuf as lowercase hex
  plus aggregate metrics only after private-envelope deletion and terminal checks.

## Work Completed

- Added the bounded async Unix-socket executor and strict frame transport.
- Added the persistent launcher, one-child process control, fixed command and
  environment, cancellation, deadline, exit confirmation, and closed error mapping.
- Added verification-only terminal validation and public capability construction.
- Added the production proof-worker and proof-launcher binaries.
- Added the exact Docker proof profile, owner-only IPC setup, pinned runtime builder,
  and rendered Compose policy check.
- Updated the live BullMQ gate to prove successful public terminal retention.
- Narrowed the root Go `bin/` ignore rule so Rust source binaries remain tracked.
- Updated README, architecture, and technical specification current state.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `cargo test -p razield` | PASS, 86 executed and 9 environment-gated ignores | Full service surface |
| `./scripts/test-bullmq-conformance` | PASS | Live Redis lifecycle, terminal result, cleanup, zero retry, and restart |
| `cargo test -p raziel-prover --no-default-features --features verification --lib` | PASS, 6 tests | Verification-only prover |
| `cargo test -p raziel-verifier` | PASS, 12 tests | Trusted registry and verifier |
| `cargo test -p raziel-authorization-core` | PASS, 16 tests | Commitment and public values |
| `cargo fmt --all -- --check` | PASS | Rust formatting |
| `cargo clippy -p razield --all-targets -- -D warnings` | PASS | Strict service lint |
| `./scripts/check-proof-sandbox-compose` | PASS | Exact rendered proof controls |
| `sh -n scripts/prepare-proof-sandbox-runtime scripts/check-proof-sandbox-compose scripts/test-bullmq-conformance` | PASS | Changed shell scripts |
| `./scripts/check-sp1-features` | PASS | Feature and dependency boundaries |
| `./scripts/dev cargo check --locked --offline -p razield --features one-shot-prover --bins` | PASS | Pinned real-child feature compile |
| Privacy source scan | PASS | No logging or private markers in new production boundary code |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | Separate approval gate |

## Privacy Checks

- PASS: Agent-facing output contains no PolicyIR, relationships, paths, openings, or
  stored caveat values.
- PASS: Debug and error output redact paths, proof bytes, private envelopes, and
  validator configuration.
- PASS: BullMQ stores only the public capability and aggregate metrics.
- PASS: The proof sandbox has no network, Docker socket, host cgroup control, source
  mount, writable registry, or writable private snapshot mount.
- NOT APPLICABLE: Independent `verifierd` behavior remains a later M4 baton.
- NOT RUN: Live proof-container output scan. The proof profile did not generate SP1.

## Remaining Risks and Follow-Up

- The separate `verifierd` retained-proof gate remains required. The supervisor's
  direct verifier is local defense in depth, not the independent verifier service.
- The one-shot metrics version does not carry execution cycles. Public job metrics
  use zero for that field.
- Private snapshot publication and `GetCapability` and `CancelCapability` RPCs remain
  pending.
- The complete VM API E2E runbook must be written after those backend paths and the
  verifier endpoint form one runnable flow.

## Exact Next Action

Claim B-0026 for the next dependency-ordered M4 backend feature, with private snapshot
publication ahead of full proof-flow E2E execution.
