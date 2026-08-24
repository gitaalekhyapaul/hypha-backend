# Engineering Baton Archive

- Baton ID: B-0028
- Final state: completed and archived
- Milestone: M4, proof resource profile reconciliation
- Owner: Codex
- Started UTC: 2026-08-24T11:37:49Z
- Completed UTC: 2026-08-24T11:50:17Z
- Branch: `main`
- Base commit: `92d4e51`

## Objective

Adopt the validated eight-CPU SP1 Plonk profile across production Compose, proof
checks, approved decisions, current documentation, and durable benchmark evidence.

## Acceptance Criteria

- [x] Production and benchmark proof profiles use CPUs `0-7`, `GOMAXPROCS=8`, and
      `RAYON_NUM_THREADS=8`.
- [x] Proof controls retain the 56 GiB memory cap, zero swap, one-hour proof deadline,
      and 75-minute capability admission window.
- [x] Host and Compose checks require eight visible proof CPUs and reject drift.
- [x] ADR-0012 and ADR-0015 record the approved resource-profile amendment.
- [x] The later VM benchmark is durable public evidence with its source, timing, peak
      memory, verification result, and zero-OOM result.
- [x] Current-state documentation no longer presents four CPUs as the approved
      production profile.
- [x] Focused checks, formatting, lint, package tests, privacy scans, and diff gates
      pass without generating another SP1 proof.

## Architecture Constraints

- Proof semantics, public values, keys, artifacts, and witness handling did not
  change.
- The 56 GiB memory and memory-plus-swap caps remain equal.
- The proof process retains no network and uses a read-only root filesystem.
- The one-hour proof deadline and 75-minute admission window remain unchanged.
- No new SP1 proof ran. B-0028 imports existing operator-provided VM evidence.
- No package, CLI, runtime, or system dependency was added.

## Decisions

- Approved by the maintainer on 2026-08-24: adopt eight CPUs as the production proof
  profile after B-0027.
- Set the canonical cluster profile name to `cluster-56g-8cpu`.
- Preserve the four-CPU run as historical benchmark evidence.
- Record missing source commit, source tree, run-bundle digest, and toolchain image ID
  as unknown. Do not reconstruct absent provenance.

## Work Completed

- Replaced production and benchmark CPU sets with `0-7` and both worker controls with
  eight.
- Updated Docker, Apptainer, Slurm, host preflight, result rendering, production child
  startup validation, and drift checks.
- Added the maintainer-supplied public benchmark. It records 929,660 ms proving, 438
  ms verification, 45,592,842,240 peak bytes, zero swap, and zero OOM.
- Amended ADR-0012, ADR-0013, and ADR-0015 with the approved resource profile.
- Updated README, architecture, technical specification, and the cluster runbook.
- Left memory, swap, network, filesystem, proof deadline, capability window, fixture,
  proof mode, guest, keys, and public values unchanged.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `./scripts/check-proof-sandbox-compose` | PASS | Production CPU, memory, swap, network, and mount controls |
| `./scripts/cluster/check-sp1-plonk-plumbing` | PASS | Docker, Slurm, profile, exporter, and drift checks |
| `cargo test -p razield one_shot` | PASS, 13 active and 1 ignored | One-shot protocol and launcher |
| `./scripts/dev pnpm format:check` | PASS | Repository formatting |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, and Buf |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust, and Go compile checks |
| `./scripts/dev pnpm test` | PASS | Default TypeScript, Rust, Go, and doc tests |
| Current-state four-CPU scan | PASS | Remaining matches are historical, negative-test, or preparation-only records |
| Benchmark private-field scan | PASS | Public aggregate evidence only |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | Existing 2026-08-13 evidence imported |

The default workspace suite ignored the real SP1 execute and proof tests as designed.
It ran all default package and documentation tests.

## Privacy Checks

- PASS: the benchmark record contains no host name, username, scheduler ID, internal
  path, proof byte, public journal byte, or private input.
- PASS: no relationship, caveat, salt, opening, witness, or private envelope entered
  a new runtime field.
- PASS: resource-profile edits expose only aggregate public controls and metrics.
- NOT APPLICABLE: no proof or private input was generated in this baton.

## Remaining Risks and Follow-Up

- Eight CPUs cut this fixture's proof time by 46.6 percent. One QEMU VM run does not
  establish broad hardware scaling behavior.
- Higher parallelism can change memory use for later fixtures. The fixed 56 GiB hard
  cap remains the fail-closed boundary.
- The imported manual transcript lacks four provenance fields. The record states each
  missing value. Future canonical runs use the wrapper and exporter.
- Independent retained-proof verification remains pending in M4.

## Exact Next Action

Claim B-0029 for the independent `verifierd` capability verification boundary. Keep
the invariant that `verifierd` never calls `razield`.
