# Permanent M6 Zero-Swap Enforcement

- Baton: B-0067 deployment follow-up
- Date: 2026-10-15, Asia/Singapore
- Authorization: maintainer confirmed the immediate runtime correction works and
  requested a permanent repair.
- Scope: M6 proof/verifier Compose definitions, existing render check, architecture,
  M6/M7 runbooks, and coordinating baton.

## Observed Failure

The retained job began at 2026-10-14T22:40:00.654Z and finished at
22:40:00.689Z, a 35 ms interval. Its public error was SERVICE_UNAVAILABLE and last
progress was PROVING. That phase is published before executor entry, not after
cryptographic proving starts. Sandbox OOM counters were zero.

Read-only SSH inspection confirmed Docker's requested proof and verifier
memory-swap values equaled their RAM limits. Both actual cgroup swap limits were
max, and their systemd scopes exposed MemorySwapMax=infinity. The host reports
Docker 29.8.2, runc 1.5.1, systemd 255.4, systemd cgroup driver, and cgroup v2.
The approved child startup check rejects this profile before receiving private
input. The maintainer reports that the targeted runtime systemd correction works.
This does not establish a completed proof or the full M7 gate.

The behavior matches
[opencontainers/cgroups issue 74](https://github.com/opencontainers/cgroups/issues/74).
No host-wide daemon reload was executed to reproduce drift on the active VM.

## Repair

Exactly m6-proof-sandbox and m6-verifier-sandbox now carry the OCI annotation
org.systemd.property.MemorySwapMax with value "uint64 0". This is the documented
[runc auxiliary-property mechanism](https://github.com/opencontainers/runc/blob/main/docs/systemd.md#auxiliary-properties).
Systemd owns an explicit zero for each created scope. No host-wide drop-in or
additional sandbox privilege is required. The cgroupfs runtime retains its
existing Docker-enforced limits.

Both health checks read the actual RAM limit and require memory.swap.max=0. They
retain the existing socket, mode, and owner checks. Swap rejection prints only
a fixed diagnostic with no authorization context. Worker and verifier API
startup still require the corresponding sandbox to be healthy.

The RAM limits, CPU profile, guest, witness rules, public values, proof mode,
network isolation, SDK, and queue behavior are unchanged. This implements the
already approved ADR-0015 resource boundary, not a new security contract.

## Validation

| Check | Result | Scope |
| --- | --- | --- |
| VM annotated container | PASS | Existing image, 64 MiB ceiling, no network or mounts; systemd property and actual swap both zero |
| VM Compose recreation | PASS | Minimal Compose service uses the rendered annotation; two successive fresh containers each report systemd=0 and cgroup=0 |
| Probe cleanup | PASS | Disposable container and Compose projects removed; no private state mounted |
| Native Mac cgroupfs health | PASS | Both rendered health commands accept zero swap and reject unlimited swap with the fixed diagnostic, four real-container cases |
| M6 live check | PASS | New annotation and actual health assertions, original memory/network/privilege checks retained |
| Integrated ngrok render | PASS | New annotations and health commands survive overlay composition; property appears on exactly two sandbox services |
| M6 policy and baseline sandboxes | PASS | Existing policy-preview, proof-sandbox, and verifier-sandbox render checks |
| Initial ngrok invocations | FAIL, resolved | Required synthetic render settings were missing; explicit dummy values resolve the harness input, no app change |
| Syntax and diff whitespace | PASS | Edited shell check parses and git diff --check passes |
| Runbook checks | PASS | 27 documented shell blocks parse and relative file links resolve |
| Independent focused review | PASS | No actionable annotation, systemd type, inheritance, or Mac cgroupfs finding; reviewer made no edits or runtime changes |
| Host daemon-reload test | SKIP | Do not change global runtime state with active demo work; registered per-unit zero verified directly |
| Production sandbox deployment | SKIP | Maintainer must pull and recreate only after admitted proofs finish or reach terminal cancellation |
| Full proof and SDK gate | SKIP | No proof submission, private input, or protected query in this repair |

## Handoff

Follow [Permanent Zero-Swap Repair](../../runbooks/M6-LIVE-VM.md#permanent-zero-swap-repair).
No image download, dependency change, binary rebuild, VM source edit, sudo command,
production restart, or proof submission was performed by this repair. The
disposable runtime probes used only the existing toolchain image.
The immediate correction remains on the live containers until their safe update.
Do not interrupt a working proof to deploy a configuration-only repair.
