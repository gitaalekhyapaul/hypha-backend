# Hypha Backend: Raziel PoC

This repository contains the first Raziel proof of concept. Raziel turns an approved
SpiceDB authorization result into an independently verifiable, proof-backed capability
for an AI agent.

The first scenario is payment reconciliation. The acceptance goal uses a real,
provenance-recorded transaction source. Test fixtures do not count as the product
acceptance run.

## Current status

M0 through M3 are complete. M3 produced and independently verified a real local SP1
Plonk proof on 2026-08-11. The public benchmark records a 2,537-byte proof,
1,742,106-millisecond proving time, 479-millisecond verification time, and a
46,301,016,064-byte memory peak. See the
[`cluster-20260810T174732Z-76324` benchmark](docs/benchmarks/sp1-plonk/cluster-20260810T174732Z-76324.md).

M4 is in active implementation under accepted ADR-0013 and ADR-0014. The `razield`
and `verifierd` binaries remain process shells. The BullMQ `1.2.3` and Redis `8.2.8`
conformance gate passes. The pure `razield` admission boundary now validates generated
request fields, derives the raw-key Ed25519 thumbprint, verifies the exact canonical
request commitment, derives the deterministic job ID, and enforces the 75-minute
request window. It performs no Redis, filesystem, or proof operation. The generated
Tonic contracts and isolated verification-only build pass their existing gates. RPC
handlers, listeners, private envelopes, queue publication, proof execution, and the
retained-proof verifier gate remain pending.

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
./scripts/test-bullmq-conformance
```

Start the local application and service shells with:

```bash
./scripts/compose --profile apps --profile services up --build
```

## Authority administration

Create separate policy and snapshot keys outside the repository:

```bash
openssl genpkey -algorithm ED25519 -outform DER -out policy-authority.pk8
openssl genpkey -algorithm ED25519 -outform DER -out snapshot-authority.pk8
chmod 0400 policy-authority.pk8 snapshot-authority.pk8
```

Copy `.env.example` to `.env`. Set `HOST_UID` from `id -u` and `HOST_GID` from
`id -g`. Set both key paths to absolute host paths.

Run the admin image as a one-off command. This example creates a public policy-key
record:

```bash
./scripts/compose --env-file .env --profile admin run --rm raziel-admin-policy \
  inspect-key \
  --key /run/secrets/raziel/policy-authority.pk8 \
  --role policy \
  --trust-domain raziel-local-dev \
  --valid-from 1700000000 \
  --valid-until 1731536000 \
  --generation 1 \
  --output /var/lib/raziel/registry/drafts/policy-key.pb \
  --json-output /var/lib/raziel/registry/drafts/policy-key.json
```

The remaining command forms are:

```text
approve-policy
  --policy PATH --key PATH --trust-domain ID --record-id ID
  --policy-name NAME --policy-version VERSION --approval-epoch N
  --valid-from UNIX --valid-until UNIX --approved-by ID
  --registered-at UNIX --generation N --output PATH --json-output PATH

publish-registry
  --root PATH --trust-domain ID --generation N --published-at UNIX
  --observed-now UNIX --output-directory RELATIVE_NEW_DIRECTORY
  --output-manifest PATH --output-json PATH [--previous-manifest PATH]
  [--authority-key RELATIVE_PATH] [--policy RELATIVE_PATH]
  [--root-record RELATIVE_PATH]

revoke-record
  --kind key|policy|root --input PATH --output NEW_PATH --json-output NEW_PATH
  --trust-domain ID --generation N --revoked-at UNIX
  --observed-now UNIX [--reason TEXT]
```

`publish-registry` requires a new output directory for each generation. Draft and
revocation outputs cannot replace existing files. The manifest changes only after the
complete candidate generation passes verifier validation.
