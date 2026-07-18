# Hypha Backend: Raziel PoC

This repository contains the first Raziel proof of concept. Raziel turns an approved
SpiceDB authorization result into an independently verifiable, proof-backed capability
for an AI agent.

The first scenario is payment reconciliation. The acceptance goal uses a real,
provenance-recorded transaction source. Test fixtures do not count as the product
acceptance run.

## Current status

The M0 scaffold is complete. TypeScript, Rust, Go, Protobuf, Docker Compose, and CI
shells pass their root gates. M1 PolicyIR work is ready and not yet implemented.

## Read first

- [`ARCHITECTURE.md`](ARCHITECTURE.md): human overview, trust boundaries, diagrams,
  stack choices, and the detailed agent implementation dump
- [`docs/spec/POC-TECHNICAL-SPEC.md`](docs/spec/POC-TECHNICAL-SPEC.md): service,
  protocol, PolicyIR, capability, test, and milestone requirements
- [`AGENTS.md`](AGENTS.md): canonical instructions for Codex, Cursor, and Claude
- [`docs/agent/CURRENT-BATON.md`](docs/agent/CURRENT-BATON.md): active engineering
  state and exact next action
- [`docs/spec/SCENARIO-AUTHORING.md`](docs/spec/SCENARIO-AUTHORING.md): work needed to
  add a scenario after reconciliation passes

## Stack

- Next.js and TypeScript for the demo console, gateway, and agent SDK
- Rust for PolicyIR, commitments, snapshots, SP1 proving, and verification
- Go for the build-time SpiceDB schema compiler adapter
- Protobuf, Buf, Tonic, and Connect-ES for service contracts
- Docker Compose with Apple Silicon compatible Linux images for local services

Exact versions and image digests live in
[`tools/toolchain-versions.env`](tools/toolchain-versions.env). ADR-0003 records their
sources and compatibility gates.

## Bootstrap

Docker with Compose is the only host prerequisite. The first bootstrap downloads the
pinned images, language packages, Buf plugins, and SP1 toolchain. Run it only after
maintainer approval:

```bash
./scripts/compose build toolchain
./scripts/compose run --rm toolchain ./scripts/update-locks
```

Review and commit `pnpm-lock.yaml` and `Cargo.lock`. The exact Buf plugin versions and
revisions live in `buf.gen.yaml`. A clean checkout then uses frozen dependency state:

```bash
./scripts/compose run --rm toolchain ./scripts/bootstrap
```

Use the same toolchain for repository commands:

```bash
./scripts/dev pnpm format:check
./scripts/dev pnpm lint
./scripts/dev pnpm typecheck
./scripts/dev pnpm build
./scripts/dev pnpm test
./scripts/dev pnpm test:e2e
./scripts/dev pnpm generate:check
```

Start the local application and service shells with:

```bash
./scripts/compose --profile apps --profile services up --build
```
