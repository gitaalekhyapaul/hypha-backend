# B-0003: M0 Repository Scaffold

- Final state: completed
- Milestone: M0, repository and contract scaffold
- Owner: Codex
- Started UTC: 2026-07-18T12:26:46Z
- Closed UTC: 2026-07-18T17:53:27Z
- Branch: `main`

## Objective

Create the polyglot workspace shells, pin the toolchain, and define reproducible local
build commands without implementing authorization behavior.

## Acceptance criteria

- [x] pnpm and Cargo workspaces exist.
- [x] Next.js console and gateway shells build.
- [x] Rust crate, service, and SP1 program shells build.
- [x] Go compiler-adapter shell builds.
- [x] Buf validates the initial service contract.
- [x] Docker Compose validates on Apple Silicon.
- [x] Workspace format, lint, typecheck, build, and test commands exist and pass.
- [x] Generated code can be reproduced from one documented command.

## Architecture constraints

- Used Next.js with TypeScript for the console and gateway.
- Kept cryptographic package boundaries in Rust.
- Kept the Go SpiceDB compiler adapter build-time only.
- Kept `verifierd` independent from `razield`.
- Added no authorization, witness, commitment, or proving behavior.
- Used a native Linux ARM64 toolchain image on Apple Silicon.
- Installed no host dependency.

## Files changed

- root pnpm, Cargo, Go, Buf, TypeScript, Biome, and version manifests
- `apps/demo-console/` and `apps/demo-gateway/`
- `packages/sdk-ts/` and `packages/proto-ts/`
- `services/razield/` and `services/verifierd/`
- six host Rust crates and `programs/authorization-guest/`
- `tools/spicedb-ir-compiler/` and the pinned toolchain image
- seven `raziel.v1` Protobuf files
- bootstrap, command, Compose, and CI files
- M0 setup docs, ADR-0003, and the snapshot secret-reference correction

## Decisions

- Inherited: D-001 through D-016 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Added: D-017 selects ADR-0003 for M0 and CI.
- Closed: O-002 through ADR-0003.
- Made: Docker is the sole baseline host runtime.
- Made: Generator versions and revisions live in `buf.gen.yaml`. This module has no
  Buf dependency lock.
- Made: Sharp is the sole dependency allowed to run an install script.
- Made: Host Rust stays at `1.97.1`. The SP1 guest declares Rust `1.91.1` as its
  minimum and builds with SP1's bundled `rustc 1.94.0-dev`.
- Made: Snapshot requests carry `zed_token_secret_ref`, never a credential value.

## Work completed

- Pinned Node, pnpm, Next.js, Rust, Go, Buf, SpiceDB, SP1, generators, and OCI image
  digests.
- Verified Linux ARM64 and AMD64 manifests for every pinned runtime image.
- Built `raziel-toolchain:m0` as Linux ARM64.
- Verified SP1 release assets by version, release commit, and SHA-256 digest.
- Generated and reviewed `pnpm-lock.yaml` and `Cargo.lock`.
- Generated TypeScript and Rust Protobuf code from exact remote plugin revisions.
- Added `.js` specifiers to generated TypeScript imports through generator settings.
- Added empty application, package, service, crate, guest, and Go CLI shells.
- Added root format, lint, typecheck, build, test, end-to-end, and generation commands.
- Added frozen bootstrap and GitHub Actions coverage for the root commands.
- Started both app shells and checked their HTTP endpoints.

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Live release-channel review | PASS | All M0 pins | 2026-07-18T13:08:07Z |
| `docker buildx imagetools inspect` | PASS, ARM64 and AMD64 present | Five runtime images | 2026-07-18T13:08:07Z |
| `./scripts/compose build toolchain` | FAIL, `/pnpm/bin` missing from `PATH` | Toolchain image | 2026-07-18T13:32:58Z |
| `./scripts/compose build toolchain` | PASS | Linux ARM64 toolchain with SP1 6.3.1 | 2026-07-18T13:51:11Z |
| First lock update | FAIL, npm platform package timeout | Lock and generation pass | 2026-07-18T13:51:11Z |
| Second lock update | FAIL, Sharp install script not approved | Lock and generation pass | 2026-07-18T14:09:34Z |
| Third lock update | FAIL, legacy pnpm setting ignored | Lock and generation pass | 2026-07-18T14:20:20Z |
| `./scripts/compose run --rm toolchain ./scripts/update-locks` | PASS | Locks and generated code | 2026-07-18T14:22:49Z |
| First frozen bootstrap | FAIL, SP1 prints commit rather than release version | Tool probes | 2026-07-18T14:22:49Z |
| `./scripts/compose run --rm toolchain ./scripts/bootstrap` | PASS | Frozen dependencies and tool probes | 2026-07-18T14:24:37Z |
| First format check | FAIL, nine differences and one deprecated setting | Workspace | 2026-07-18T14:24:37Z |
| `./scripts/dev pnpm format` | PASS | Biome, rustfmt, and gofmt | 2026-07-18T14:27:41Z |
| First lint | PASS with one Biome warning | Workspace | 2026-07-18T14:27:41Z |
| `./scripts/dev pnpm format:check` | PASS | Workspace | 2026-07-18T14:30:42Z |
| `./scripts/dev pnpm lint` | PASS, no warnings | Biome, Clippy, Go vet, and Buf | 2026-07-18T14:30:42Z |
| First typecheck | FAIL, generated imports lacked `.js` | TypeScript packages | 2026-07-18T14:30:42Z |
| `./scripts/dev pnpm generate:check` | PASS | Two clean generations matched | 2026-07-18T14:33:29Z |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, host Rust, and Go | 2026-07-18T14:33:29Z |
| First build | FAIL, guest Rust minimum exceeded SP1 compiler | SP1 guest | 2026-07-18T14:33:29Z |
| `./scripts/dev pnpm build` | PASS | Next.js, TypeScript, host Rust, SP1, and Go | 2026-07-18T15:04:00Z |
| `./scripts/dev pnpm test` | PASS, 7 TypeScript tests, 8 Rust tests, and Go tests | Workspace | 2026-07-18T15:04:17Z |
| `./scripts/dev pnpm test:e2e` | PASS | Shell end-to-end command | 2026-07-18T17:53:27Z |
| `docker compose --env-file tools/toolchain-versions.env config --quiet` | PASS | Compose model | 2026-07-18T17:53:27Z |
| `docker image inspect raziel-toolchain:m0` | PASS, Linux ARM64 | Toolchain image | 2026-07-18T17:53:27Z |
| Console and gateway HTTP probes | PASS | Ports 3000 and 3001 | 2026-07-18T17:53:27Z |
| Secret and private-artifact scan | PASS | New source files | 2026-07-18T17:53:27Z |
| `git diff --check` | PASS | M0 worktree | 2026-07-18T17:53:27Z |

## Privacy review

- Agent responses contain no PolicyIR: PASS.
- Agent responses contain no relationship tuple or path: PASS.
- Agent responses contain no Merkle opening: PASS.
- Logs contain no caveat secrets or witness material: PASS.
- Capability claims reveal only approved public fields: PASS at wire-contract level.
- Verifier reads policy from its own registry: NOT APPLICABLE before registry work.
- Verifier completes without a prover connection: PASS at package-dependency level.
- Real data remains outside Git and generated output: PASS.

## Remaining risks

- ADR-0002 needs maintainer approval before gateway integration.
- O-001 needs a real transaction source and usage basis before scenario adapter work.
- Service binaries define process boundaries only. They do not serve network traffic.
- Generated protocol code stays ignored and must come from `pnpm generate`.

## Handoff

Continue with B-0004 in `docs/agent/CURRENT-BATON.md`. Claim the bounded PolicyIR wire
and Rust semantics objective before editing M1 files.
