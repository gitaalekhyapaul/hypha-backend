# M6 Public Go Cache And Console Readiness Repair

- Parent baton: B-0067
- Checkpoint UTC: 2026-10-14T09:32:44Z
- Owner: Codex
- Result: local deployment repair passes focused checks. Full VM startup remains pending.
- Scope: backend preparation, cache recovery, console health, Compose checks, and runbooks.
- SDK and agent repositories: read-only, no release or source change.

## Trigger

The maintainer's VM output confirms that the native-runner prefetch repair passes.
The offline proof build then lacks `raziel-sp1-gnark.mod` in the M6 Go cache. Earlier
targeted instructions omitted the existing Go dependency fetch. The console publishes
port 3000, but ngrok's healthy dependency fails without a console health check.

A fresh local Go fetch hits HTTP 404 for SP1's pinned
`gnark-ignition-verifier` pseudo-version. No dependency version, fork, or proof
feature changes. The maintainer confirmed these VM public cache volumes exist:

- `raziel_razield-proof-go-mod-cache`
- `raziel-m6-live_razield-proof-go-mod-cache`

Volume existence does not establish cache completeness or integrity.

## Repair

`prepare-m6-proof-runtime` orders workspace Rust fetch, SP1 native prefetch, Go
preparation, and the networkless proof build. A failed stage stops later stages.
`prepare-m6-demo` calls this sequence rather than duplicating it.

The explicit `--reuse-go-cache` option reads the legacy public cache without writing
to it. Recovery mounts only that cache, the separate M6 destination, the current
public Cargo registry, and the checker. The container has no network, private
volume, repository mount, credentials, or Docker socket. Its root is read-only.

The checker requires the exact pinned SP1 Go manifest and all original source
checksums. Offline download checks cache completeness. Go verification checks module
integrity. Copy does not replace existing destination files or preserve legacy
ownership. The destination passes the same checks before the offline build runs.

The console health check requests its internal loopback port 3000. It accepts a
successful HTTP response and has a four-second deadline. Ngrok waits for readiness.
The existing proof and verifier limits remain unchanged.

## Validation

| Check | Result | Scope |
| --- | --- | --- |
| Ordered preparation and failure cases | PASS | Focused Node harness, no services or proof |
| Manifest/checksum mismatch and missing cache cases | PASS | Checker rejects invalid inputs without a pass marker |
| Recovery mounts and volume selection | PASS | Source read-only, target separate, no private mounts, no source auto-creation |
| Real Go offline verification | PASS | Small controlled cache using an existing public xid module, not the full SP1 graph |
| Real module mutation and missing module | PASS | Both reject in a networkless, read-only container |
| Actual recovery container command | PASS | Controlled mounts, source content unchanged, existing destination preserved |
| Conflicting destination | PASS | Rejected, conflicting contents not overwritten |
| Console HTTP probe | PASS | Pinned Node accepts 200, rejects 503 and a closed listener |
| Live/policy/ngrok/proof Compose checks | PASS | Rendered readiness, isolation, and existing limits |
| Formatting and lint | PASS | Existing pinned Linux Biome, no dependency install |
| Shell/runbook/diff/privacy checks | PASS | Syntax, relative links, whitespace, scoped credential scan, and no private artifacts staged |
| SDK/agent worktrees | PASS | Both clean and unchanged |
| Fresh local full Go fetch | FAIL | Upstream pinned pseudo-version returns HTTP 404 |
| Full legacy VM cache recovery and proof build | SKIP | Requires the maintainer's pull and VM execution |
| Live ngrok, SDK flows, real proofs, recording | SKIP | Not part of these local repair probes |

The initial real-Go probe assumed two xid checksum rows in the published source.
That source contains only the go.mod row. The corrected controlled cache adds the
already downloaded module's zip checksum. The initial conflict probe tried to write
the finalized read-only manifest. The corrected probe creates the conflicting file
before recovery. Neither correction changes production behavior.

## Handoff

Follow [public Go cache recovery](../../runbooks/M6-LIVE-VM.md#recover-the-existing-public-go-cache)
in the VM backend checkout. Pull the checkpoint, run recovery, then start with
`--ngrok` and inspect health. Send the first useful failure output if validation
stops. Do not delete caches or private volumes. A fresh host without a matching
public cache still needs a verified artifact or a separately approved dependency
change. This checkpoint makes no full-stack or M6/M7 acceptance claim.
