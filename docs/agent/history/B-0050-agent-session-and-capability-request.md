# Agent Session And Capability Request

- Baton ID: B-0050
- State: COMPLETE
- Milestone: M5, agent session and capability request
- Owner: Codex
- Started UTC: 2026-08-27T00:03:12Z
- Completed UTC: 2026-08-27T00:50:21Z
- Branch: `main`
- Worktree: Approved M5 docs and B-0034 through B-0050 features are uncommitted.

## Objective

Add the desktop Agent and Capability workspace. Create one ephemeral server-side
agent session, sign the exact approved capability request through the TypeScript SDK,
submit it to the existing asynchronous `razield` service, and show public job state.

## Acceptance Criteria

- [x] The screen starts from the active policy, accepted snapshot, and selected
      finalized synthetic dataset already created in the portal.
- [x] The operator can review agent thumbprint, task, purpose, action, resource scope,
      and requested validity before one explicit request action.
- [x] The ephemeral Ed25519 private key remains server-side and leaves no browser or
      log representation.
- [x] The server uses the existing TypeScript SDK for request commitment and signing.
- [x] Submission reaches the existing `razield` API and returns one stable job ID.
- [x] The UI polls queued, proving, succeeded, failed, canceled, or unavailable state
      without holding one HTTP request open.
- [x] Local development can use an explicitly labeled recorded timeline. Recorded
      mode cannot mint a capability or unlock protected transaction data.
- [x] Focused SDK and route checks plus one desktop recorded-state walkthrough pass.

## Architecture Constraints

- The browser cannot receive an agent private key, private witness, PolicyIR,
  relationship tuples, ZedToken, proof internals, or detailed denial paths.
- `razield` cannot approve policy or snapshot records.
- A successful recorded state cannot authorize a protected gateway request.
- Capability request fields match ADR-0011, ADR-0014, and the generated wire contract.
- Real Plonk generation remains asynchronous and runs only on the proof-capable VM.

## Decisions

- M4 remains the owner of the BullMQ-backed proof service and stable job ID.
- The browser controls public request fields. A server-side session owns the private
  agent key and signing operation.
- The Access Graph uses the lowercase Ed25519 key thumbprint as the `agent` subject.
  The human-readable agent ID remains presentation metadata.
- Recorded mode is presentation state only and cannot create an authorization result.
- The M5 capability API runs without snapshot authority or SpiceDB credentials.

## Work Completed

- Replaced the Access Graph display-agent subject with the exact key thumbprint used
  by the guest. Raised the snapshot default from 3,600 to 7,200 seconds.
- Added one process-memory Ed25519 agent session and safe browser projection.
- Added authenticated create, get, and cancel routes plus a bounded Rust Tonic client.
- Added the desktop capability workspace with public states, metrics, cancellation,
  and explicit non-authorizing recorded playback.
- Added a capability-only `razield` entry point.
- Added the `m5-live` Compose profile using the M5 registry, snapshots, Redis worker,
  and 60 GiB no-network proof sandbox.
- Added loopback-only web bindings plus the VM and SSH instructions.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Console tests | PASS, 47 tests | Agent, Access Graph, capability, policy, and dataset behavior | 2026-08-27T00:47Z |
| Console lint and typecheck | PASS | Desktop frontend and server routes | 2026-08-27T00:47Z |
| `cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Capability Tonic transport | 2026-08-27T00:45Z |
| `cargo clippy -p razield --all-targets -- -D warnings` | PASS | Capability-only entry point and service | 2026-08-27T00:40Z |
| Recorded and live Compose config | PASS | Local and VM service models | 2026-08-27T00:45Z |
| Desktop browser walkthrough | PASS | Key-bound snapshot and recorded capability lifecycle | 2026-08-27T00:33Z |
| Live SDK to Tonic submission | PASS, stable queued job and exact duplicate | Capability transport without proof generation | 2026-08-27T00:49Z |
| `git diff --check` | PASS | B-0050 worktree | 2026-08-27T00:47Z |

## Privacy Checks

- Agent responses contain no PolicyIR: PASS
- Agent responses contain no relationship tuple or path: PASS
- Agent responses contain no Merkle opening: PASS
- Logs contain no caveat secrets or witness material: PASS
- Capability claims reveal only approved public fields: PASS
- Verifier reads policy from its own registry: NOT APPLICABLE
- Verifier completes without a prover connection: NOT APPLICABLE
- Real data remains outside Git and generated output: PASS

## Risks And Follow-Up

- Fresh Plonk completion through the portal remains part of the final M5 VM run.
- Agent identity and live job ownership are process-memory state. A console restart
  requires a new agent and snapshot.
- The M5 live backend roles use container UID 0 to match owner-only M5 volumes. They
  drop Linux capabilities, use read-only roots, and expose no public backend port.
  Rootless volume ownership remains a post-PoC hardening item.

## Exact Next Action

Open B-0051 for the protected reconciliation gateway. Accept one signed protected
request, verify the capability through `verifierd`, enforce exact scope and a fresh
nonce, return bounded synthetic rows, and record a public receipt.
