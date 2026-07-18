# Current Engineering Baton

- Baton ID: B-0003
- State: READY
- Milestone: M0, repository and contract scaffold
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-07-18T12:24:33Z
- Branch: `main`
- Worktree: Clean after documentation baseline commit, branch is one commit ahead of `origin/main`

## Objective

Create the polyglot workspace shells, pin the toolchain, and define reproducible local
build commands without implementing authorization behavior.

## Acceptance criteria

- [ ] pnpm and Cargo workspaces exist.
- [ ] Next.js console and gateway shells build.
- [ ] Rust crate, service, and SP1 program shells build.
- [ ] Go compiler-adapter shell builds.
- [ ] Buf validates the initial service contract.
- [ ] Docker Compose validates on Apple Silicon.
- [ ] Workspace format, lint, typecheck, build, and test commands exist.
- [ ] Generated code can be reproduced from one documented command.

## Architecture constraints

- Use Next.js with TypeScript for the console and gateway.
- Keep cryptographic semantics in Rust.
- Keep the SpiceDB compiler adapter small and build-time only.
- Keep `verifierd` independent from `razield`.
- Do not add real authorization logic during shell scaffolding.
- Do not install a missing dependency without user approval.
- Select native `linux/arm64` or multi-architecture container images.

## File scope

- root workspace manifests and version files
- `apps/`
- `packages/`
- `services/`
- `crates/`
- `programs/`
- `tools/`
- `proto/`
- `docker-compose.yml`
- CI configuration
- setup sections in `README.md`
- baton and decision records needed by M0

## Out of scope

- PolicyIR evaluation
- SpiceDB schema translation
- Merkle snapshot implementation
- SP1 proving logic
- capability issuance
- payment ingestion
- real transaction data selection

## Decisions

- Inherited: D-001 through D-016 in `docs/spec/POC-TECHNICAL-SPEC.md`
- Inherited: ADR-0001 and proposed ADR-0002 in `docs/decisions/`
- Must close during this baton: O-002, tool and framework version pins
- Made: None

## Open questions and blockers

- Node `26.0.0` and pnpm `10.7.0` are installed but do not match the researched pins.
- Rust, Cargo, rustup, Go, Buf, and SP1 are not installed.
- Dependency installation needs user approval after the final pins and commands are
  recorded.
- ADR-0002 needs maintainer approval before gateway integration. It does not block M0.
- The real transaction source remains O-001. It does not block M0.

## Work completed

- Documentation baseline committed as `2e4b61b`.
- Host architecture confirmed as Apple Silicon `arm64` on macOS 26.5.
- Docker `28.4.0` and Compose `2.39.2` confirmed available.
- Current official version candidates researched without installing software.

## Validation plan

- Select compatible pinned versions and record O-002.
- Record exact approved installation or container bootstrap commands.
- Build each empty workspace shell.
- Run Buf lint and deterministic generation twice.
- Run Docker Compose config validation.
- Run workspace format, lint, typecheck, test, and build commands.

## Validation evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `node --version` | PASS, `v26.0.0` observed | Host | 2026-07-18T12:03:44Z |
| `pnpm --version` | PASS, `10.7.0` observed | Host | 2026-07-18T12:03:44Z |
| `docker --version` | PASS, `28.4.0` observed | Host | 2026-07-18T12:03:44Z |
| `docker compose version` | PASS, `2.39.2` observed | Host | 2026-07-18T12:03:44Z |
| Rust, Cargo, rustup | NOT AVAILABLE | Host | 2026-07-18T12:03:44Z |
| Go | NOT AVAILABLE | Host | 2026-07-18T12:03:44Z |
| Buf | NOT AVAILABLE | Host | 2026-07-18T12:03:44Z |
| SP1 | NOT AVAILABLE | Host | 2026-07-18T12:03:44Z |

## Risks and private-data notes

- Do not commit real transaction files, private keys, witnesses, or proof debug dumps.
- Do not expose PolicyIR through a temporary scaffold API.
- Cross-language hashes cannot use raw Protobuf serialization.
- Scaffold package boundaries must match `ARCHITECTURE.md`.
- SP1 `6.2.2` needs a host and Linux ARM64 compatibility spike with the selected Rust
  toolchain.

## Exact next action

Claim B-0003, record the proposed O-002 pins and reproducible bootstrap commands, then
request approval for every missing or mismatched host tool.
