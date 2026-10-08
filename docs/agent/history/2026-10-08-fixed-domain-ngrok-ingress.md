# Fixed-Domain Ngrok Deployment Checkpoint

- Date: 2026-10-08
- Baton: B-0067 bounded deployment follow-up
- Result: local implementation complete, live endpoint and M7 acceptance pending
- Agent source: `71d238ecca44fe5a98c50193fc028c1b2208da6e`
- Backend source: the commit containing this record
- SDK: unchanged public `0.1.0`

## Delivered

The maintainer selected Docker-managed ngrok and supplied
`https://either-anymore-immunize.ngrok-free.dev`. ADR-0022 records that decision.
An opt-in overlay starts the tunnel with the VM backend. Standalone Compose starts
the same image against an existing Mac backend. No host ngrok CLI is required.
One backend uses the domain at a time. Pooling and local inspection are disabled.

The public route policy accepts the existing agent API and protected-query methods.
Other paths return 404. Operator pages stay SSH-forwarded. The tunnel joins no owner
network and mounts no private data. The private token reaches only the tunnel role.
Ngrok's HTTPS edge can see permitted API traffic. This is a synthetic-demo route.

Agent configuration reads `RAZIEL_API_BASE_URL` and prefills both Identity forms.
Retained credentials override the new default. No token, identity, task, or capability
is migrated. The SDK dependency, request signatures, and proof semantics are unchanged.
Dashboard browser origins remain separate from server-side SDK destinations. No new
backend CORS or origin allowlist was added.

## Evidence

| Check | Result | Scope |
| --- | --- | --- |
| Image metadata and approved download | PASS | Official multiarch 3.39.11-alpine, manifest digest 187e588f6c4efe3b29cd3eea9fcd768a1afa7342319e3cee9aeb5af6a9cf64fd |
| Pinned CLI | PASS | Version 3.39.11, v3 config accepted, fixed URL/policy/inspection/pooling flags and health-check tools present |
| Networkless CLI startup | PASS | Local arguments/config parsed; expected cloud connection failure with networking disabled, no endpoint created |
| Standalone and integrated Compose | PASS | Fixed URL, token scope, read-only mounts, isolated ingress networks, no host health port |
| Baseline M6 Compose | PASS | Existing 56 GiB/eight-CPU/no-swap proof and 2 GiB/networkless verifier limits unchanged |
| Route vectors | PASS | Nine allowed routes and twelve rejected paths/methods, static evaluation only |
| Negative preflight | PASS | Missing URL and token rejected before startup |
| Agent TypeScript | PASS | Pinned compiler, no emit, read-only source mount |
| Agent tests | PASS | 14 passed, zero failed/skipped with sequential test files |
| Agent formatter | PASS | 22 files, no fixes |
| Agent image | PASS | Cached frozen production install, both roles read fixed-origin default |
| Documentation | PASS | Relative links and shell examples checked, private settings ignored, diff whitespace checked |
| Live cloud route and signed API | SKIP | Private token not configured for this check |
| VM source/runtime and real proofs | SKIP | No remote edits, pulls, starts, or proof jobs |

Initial checks found two harness issues. Compose renders a network membership as
`null`, so its checker now tests key presence rather than truthiness. An omitted
synthetic PostgreSQL password in the render-only environment was supplied on retry.
Neither result needed a backend network or database change.

The concurrent agent run passed 13 tests but missed the existing message-channel
startup window. That test and the full sequential suite pass without test or channel
code changes. The first pnpm wrapper invocation tried to reconcile a read-only modules
mount. Direct pinned compiler/Node invocations avoided modifying that mount.

A network-disabled image build used a different cache key and could not fetch package
metadata. The normal cache-backed build passed without fetching or changing a package.
The new test's formatter-required line wrap was applied and rechecked.

## Handoff

The maintainer pulls both repository checkpoints on the VM. Add the ngrok token to
ignored backend `.env.m6` and the matching public URL to agent `.env.agents`.
Follow the split [M7 runbook](../../runbooks/M7-AGENT-DEMO.md), start the backend with
`--ngrok`, and verify public route rejection plus signed identity lookup. Then validate
Agent A's real approval-to-data path, Agent B's separate capability, and reconciliation.
The two-proof M7 demo gate remains open. Static checks do not establish cloud acceptance.
