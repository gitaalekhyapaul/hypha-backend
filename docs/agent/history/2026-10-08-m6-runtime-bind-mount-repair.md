# M6 Runtime Bind-Mount Startup Repair

- Date: 2026-10-08
- Baton: B-0067 bounded deployment repair
- Result: local cold-start mounts pass, VM retry pending
- Baseline: `c160480d6d233439d93382b52a53ab281a18672f`

## Cause And Change

The maintainer pulled the dispatcher tmpfs repair. Both VM render checks passed,
and Docker created the dispatcher. Startup then failed during policy-tools container
creation. Docker could not create `/workspace/.raziel/m6` inside the read-only
workspace mount. The fresh VM workspace lacked that nested mountpoint.

The repair maps the same host runtime directory to `/var/lib/raziel/m6` in
`demo-console-policy-tools`, `demo-console-init`, `demo-console`, and `demo-gateway`.
The initializer and declared operator, policy, transaction, and receipt paths use
that target. The preview source stays `.raziel/m6`. The live source stays
`.raziel/m6-live`. Existing records and policy subdirectory mounts stay in place.

Initializers retain their read-only source mounts. No permission relaxation,
state migration, dependency, image pin, API, SDK, proof, or sibling source change.
All three Compose checks now verify the four runtime targets. Policy and live
checks verify the corresponding configured paths and read-only initializer sources.

## Evidence

| Check | Result | Scope |
| --- | --- | --- |
| Before-fix actual Docker mount | PASS | Fresh workspace reproduces read-only mountpoint creation at the old nested target |
| Policy preview Compose | PASS | Apps/all profiles, ports 3000/3011, all four runtime mounts and configured paths |
| Live M6 Compose | PASS | All runtime paths, read-only initializer sources, existing 56 GiB/eight CPU/no-swap proof and 2 GiB/networkless verifier limits |
| Integrated ngrok Compose | PASS | Synthetic settings render, fixed domain and all four runtime mounts; no tunnel started |
| After-fix actual Docker mounts | PASS | Four networkless, read-only-root disposable containers share the new writable runtime with a fresh workspace |
| Source protection | PASS | Both initializer probes reject source writes with EROFS; no old nested mountpoint is created |
| Operator initialization | PASS | Actual script provisions a 0600 record at the configured path, then retains its contents unchanged on restart |
| Shell syntax, documentation links, whitespace | PASS | Edited scripts, runbook/archive links, and Git diff |
| Full VM startup and connected proof | SKIP | Maintainer pulls and retries; no remote changes, live tunnel, or proof job ran |

The probes used the existing pinned toolchain and generated temporary directories.
They mounted no real secrets or runtime state and started no service dependencies.
The operator password and ngrok settings were synthetic. Probe containers used
`--rm`, and their temporary host files were removed after validation.

## Exact Next Action

The maintainer pulls this backend repair, runs `./scripts/check-m6-live-compose`,
then retries `./scripts/start-m6-demo --ngrok`. Retain `.env.m6` and every volume.
Normal Compose startup recreates the changed containers. No image rebuild,
dependency preparation, proof rerun, SDK release, or manual nested directory
creation is required. Continue Agent A's connected validation after startup passes.
