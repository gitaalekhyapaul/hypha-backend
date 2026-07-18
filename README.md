# Hypha Backend: Raziel PoC

This repository contains the first Raziel proof of concept. Raziel turns an approved
SpiceDB authorization result into an independently verifiable, proof-backed capability
for an AI agent.

The first scenario is payment reconciliation. The acceptance goal uses a real,
provenance-recorded transaction source. Test fixtures do not count as the product
acceptance run.

## Current status

The repository is at M0, contract and workspace scaffolding. Application packages and
dependencies have not been created yet.

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

## Planned stack

- Next.js and TypeScript for the demo console, gateway, and agent SDK
- Rust for PolicyIR, commitments, snapshots, SP1 proving, and verification
- Go for the build-time SpiceDB schema compiler adapter
- Protobuf, Buf, Tonic, and Connect-ES for service contracts
- Docker Compose with Apple Silicon compatible Linux images for local services

Setup commands will be added during M0 after runtime and dependency versions are
approved.
