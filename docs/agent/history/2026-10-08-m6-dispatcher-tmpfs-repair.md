# M6 Dispatcher Tmpfs Startup Repair

- Date: 2026-10-08
- Baton: B-0067 bounded deployment repair
- Result: local runtime reproduction and fix pass, VM retry pending

## Cause And Change

The dispatcher used an unquoted YAML flow list for its temporary filesystem.
Rendering produced `"/tmp:rw"`, `"exec"`, `"nosuid"`, `"nodev"`, `"mode=0700"`,
and `"size=268435456"` as six entries. Docker rejected the relative mount targets.
The maintainer observed `mode=0700`. Local reproduction reported the size entry.
Both errors have the same cause.

Quote the existing string in `docker-compose.m6-live.yml`. Preserve 0700 permissions,
256 MiB size, and mount flags. Both affected Compose check scripts now reject
relative tmpfs entries. The baseline check verifies the dispatcher's one exact mount.
No app, dependency, proof, SDK, sibling source, or credential changes.

## Evidence

| Check | Result | Scope |
| --- | --- | --- |
| Before-fix Compose and Docker | PASS | Six rendered entries reproduced daemon invalid mount path in isolated probe |
| M6 baseline render check | PASS | Correct mount and existing proof/verifier limits |
| M6 ngrok render check | PASS | Correct mount with the fixed-domain overlay |
| Actual repaired mount | PASS | Networkless, read-only container starts; Node checks 0700 mode and 268435456-byte tmpfs |
| Flow-list scan | PASS | Only two tmpfs flow lists exist in backend Compose, both quoted after repair |
| Shell syntax and whitespace | PASS | Edited check scripts and Git diff |
| VM stack startup | SKIP | No remote edit or runtime action; maintainer pulls and retries |

The isolated probe used the existing pinned toolchain, no secrets or private mounts,
and `--rm`. It did not run the dispatcher, a proof job, or the live service stack.

## Exact Next Action

The maintainer pulls the repair on the VM, runs `./scripts/check-m6-live-compose`,
then retries `./scripts/start-m6-demo --ngrok`. Retain `.env.m6` and all volumes.
No image rebuild, dependency preparation, or proof regeneration is required by this
mount-format repair. Continue signed Agent A validation after startup succeeds.
