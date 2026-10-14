# Ngrok Auth-File RAM-Mount Repair

- Parent baton: B-0067 deployment follow-up
- Checkpoint UTC: 2026-10-14T22:13:30Z
- Maintainer date: 2026-10-15, Asia/Singapore
- Owner: Codex
- Result: local reproduction and repair checks pass. Live VM acceptance remains pending.
- Scope: ngrok Compose role, focused checks, runbook, and baton evidence.

## Failure

The maintainer's VM role repeatedly fails to create
`/var/lib/ngrok/auth-config.yml` on its read-only root. The pinned official image's
entrypoint writes this file from `NGROK_AUTHTOKEN` before starting the CLI. This
prevents the public endpoint from starting. Agent startup-only logs do not establish
whether a browser request reached the dashboard.

The October 8 config/CLI checks did not test entrypoint token-file creation on the
restricted root. Their result was valid for CLI/config parsing, not authenticated
container startup. This repair exercises the missing boundary with a synthetic token.

## Repair

Keep the pinned image, UID/GID 1000, read-only root, dropped capabilities, and route
policy. Add private tmpfs at `/var/lib/ngrok` with size 1 MiB, mode 0700, noexec,
nosuid, and nodev. The generated provider-auth file disappears with the container.
It has no host bind or persistent volume.

Set `NGROK_CONFIG` to the existing read-only `/etc/raziel-ngrok/agent.yml`. The new
tmpfs hides the image's default base config. This setting makes the stock entrypoint
use our mounted base config instead. No custom entrypoint or new wrapper is needed.

The Compose check requires both settings. All other ingress boundaries stay fixed.
No SDK or agent change, dependency download, real credential, VM edit, live tunnel,
or proof generation occurs in this checkpoint.

## Validation

| Check | Result | Scope |
| --- | --- | --- |
| Docker availability | PASS | Maintainer started existing local Docker 29.8.2 |
| Original HTTP entrypoint | PASS | Exact read-only auth-file failure reproduced with pinned image and disabled network |
| Repaired auth generation | PASS | Stock entrypoint creates the synthetic config in RAM and both auth/base configs validate |
| Runtime mount constraints | PASS | Mode 0700, owner 1000:1000, tmpfs backing, read-only root |
| Actual HTTP CLI startup | PASS | Loopback health API returns an empty tunnel list with no external network |
| Standalone/integrated Compose checks | PASS | Private auth mount, explicit config path, existing image and security constraints |
| M6 Compose constraints | PASS | Proof/verifier resource and network boundaries unchanged |
| Traffic policy vectors | PASS | Nine allowed and twelve denied routes |
| Auth guard negative cases | PASS | Missing mount, missing config setting, and unsafe directory mode all reject |
| Handoff checks | PASS | Shell/runbook syntax, relative links, diff whitespace, and scoped credential/private-artifact scan |
| Sibling repositories | PASS | SDK and agents stay clean and unchanged |
| Real token and public endpoint | SKIP | Requires maintainer VM recreation and credentialed startup |
| Signed SDK connection and real proofs | SKIP | Separate live M6/M7 gate |

A config-check process alone can exit zero after the entrypoint's write error.
The reproduction uses the actual HTTP command and checks its failure output.
An initial bounded HTTP probe assumed timeout termination must return nonzero.
The image returned zero after termination. The final probe checks the live local
health API instead of inferring startup state from a termination code.

## Handoff

Follow [Ngrok Auth-File Startup Repair](../../runbooks/M7-AGENT-DEMO.md#ngrok-auth-file-startup-repair).
Pull in the VM backend repo and recreate only `raziel-ngrok` with `--no-deps`.
Do not rebuild proof binaries, restart agents, delete volumes, or change tokens
for this repair. Test `/api/agent/v1/me` from Agent A without credentials and expect
Raziel JSON with HTTP 401. Then retry signed Save connection with its exact audience
and agent-specific token. Neither check substitutes for proof-backed capability
issuance and the M7 reconciliation gate.
