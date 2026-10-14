# SP1 Native-Runner Dependency Cache Repair

- Date: 2026-10-14
- Baton: B-0067 bounded deployment repair
- Baseline: `dbfacdd77d375760697221d069e3bebb9a944b32`
- Result: exact error reproduced and offline metadata repaired locally; VM rebuild pending

## Cause

The maintainer's VM startup passed the earlier mount repairs and initializers.
`m6-proof-build` failed in SP1 6.3.1's native-runner build script. Its nested Cargo
metadata call starts from the published runner manifest, not Raziel's workspace.
The published graph includes optional `gecko_profile` 0.4.0. That graph was not
prepared by the workspace's `cargo fetch --locked`.

The initially unhealthy access API later reported healthy, zero failing streak,
and five successful checks. No access-API code or health timeout was changed.
That TCP health report does not prove the complete API workflow.

## Change And Approval

The maintainer approved the two published SP1 runner dependency graphs. The new
`scripts/prepare-sp1-native-dependencies` finds the exact cached 6.3.1 manifests,
validates their published Cargo.lock hashes, performs locked fetches, and resolves
both metadata graphs offline. It rejects missing/duplicate manifests and changed
lockfiles. Its `--check` mode never fetches dependencies.

Standard bootstrap and M6 preparation call the helper after the workspace fetch.
The shared proof-runtime build calls the offline check before compilation. The
networkless build and read-only caches stay intact. No workspace dependency pin,
profiling feature, proof statement, guest, public value, SDK, or API changed.

Lockfile pins verified against the original crates.io archives:

- `sp1-core-executor-runner` 6.3.1: `2443cc90f9b258c097389d84361efff27fdf7ccf38043686f3337b4e97cdf24b`
- `sp1-core-executor-runner-binary` 6.3.1: `2e977fcb9f7801e7922c7d50ed9525a5ddc4138072c32e62ccc1c497013e7f86`

## Evidence

| Check | Result | Scope |
| --- | --- | --- |
| Fresh workspace-only fetch | PASS | Reproduces the exact VM gecko_profile error with networking disabled and read-only Cargo cache |
| Original published artifacts | PASS | Both crate archive SHA-256 values match the workspace lockfile; embedded lockfiles match helper pins |
| Approved fetch | PASS | Both real published graphs prepared in the existing pinned Docker toolchain |
| Actual offline metadata | PASS | Both metadata calls succeed with network none, read-only root and Cargo cache, bounded temporary storage |
| Focused helper cases | PASS | Fetch/check modes and changed/missing/duplicate inputs, mutation, and invalid-option rejection |
| SP1 feature gates | PASS | Blocking/native-gnark retained; hosted/CUDA/experimental proving excluded; verifier and RPC isolation pass |
| Workspace manifests and lockfile | PASS | No changes to Cargo.lock, Cargo.toml, or crates/prover/Cargo.toml |
| Compose render | PASS | Baseline and integrated ngrok checks preserve approved limits and transport boundaries |
| Shell/docs/diff checks | PASS | Edited shell syntax, runbook shell blocks and local links, staged whitespace |
| Full runtime compile and live proof | SKIP | No VM build or service action; no proof generated locally |

Local fetches created public M6 toolchain caches only. They started no owner service,
database, agent, tunnel, or proof worker. Disposable probe containers used `--rm`.
Temporary helper fixtures used isolated storage and were removed. Ignored public
dependency caches remain available for later builds. No credentials or private
runtime state were read, changed, or logged.

## Exact Next Action

The maintainer pulls this repair and follows the targeted native-cache preparation
in [M6 Live VM](../../runbooks/M6-LIVE-VM.md#offline-sp1-build-cache). Rebuild only
`m6-proof-build`, then retry normal startup. Retain settings and every runtime volume.
Full connected M6/M7 acceptance remains open.
