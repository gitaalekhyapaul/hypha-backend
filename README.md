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

M4 is in active implementation under accepted ADR-0013, ADR-0014, and ADR-0015. `razield` now
runs a bounded internal Tonic ingress. `verifierd` remains a process shell. The BullMQ
`1.2.3` and Redis `8.2.8` conformance gate passes. The pure `razield` admission
boundary validates generated
request fields, derives the raw-key Ed25519 thumbprint, verifies the exact canonical
request commitment, derives the deterministic job ID, and enforces the 75-minute
request window. A separate private-envelope boundary writes only admitted
proof-worker fields into a versioned 16 KiB Protobuf envelope. It uses a `0700`
store, `0600` files, no-replacement atomic publication, strict reads, deletion, and
grace-based orphan cleanup.

The capability-job publisher stages that private envelope first, then publishes
a strict two-field payload with a 256-byte ceiling. The request commitment remains
the BullMQ custom ID. Exact duplicates return the retained Redis record. Confirmed
publication failures delete a newly staged envelope. Ambiguous Redis outcomes keep
it for reconciliation. Jobs use one attempt, global concurrency one, zero stalled
retries, and terminal retention capped at 24 hours and 100 records.

The runnable service validates explicit bind, Redis, queue-prefix, and private-store
settings. It reconciles private files against exact BullMQ records before binding.
The generated server rejects decoded messages above 2 MiB. `CreateCapability` uses
trusted whole-second server time, then calls admission and publication. It returns an
authoritative `QUEUED` job without waiting for proof work.

The proof supervisor uses the official BullMQ worker to claim one job. It
checks the exact public payload and job policy before opening the private envelope.
It rechecks the 75-minute request window at worker start, applies a fixed one-hour
execution deadline, forwards cancellation, deletes the envelope, and publishes only
closed progress and terminal values. The production binary installs only the fixed
Unix-socket executor and strict terminal validator. It cannot inject another executor
or validator.

The B-0022 host builder consumes the exact ordered relationships, secret salt, and
committed root of one selected snapshot. It rebuilds and checks that root, evaluates
the approved PolicyIR with snapshot leaf indices as witness IDs, opens only the
relationships used by the first positive path, and validates the reduced witness set
before returning it. Denied, malformed, oversized, and root-mismatched inputs fail
without returning private witness data. The B-0023 input assembler resolves the
request's exact signed snapshot through `LocalRegistry`, requires the request, policy,
and snapshot to cover the 75-minute proof window, consumes the private snapshot
selection, builds `AuthorizationGuestInputV1`, and runs the shared host authorization
statement. It retains the expected public journal for the later proof comparison.

The B-0024 boundary stores each private relationship snapshot in one immutable,
root-keyed Protobuf file. Loads enforce owner-only modes, strict encoding, exact
registry identity, fixed size limits, and a full Merkle-root rebuild. The feature-gated
`razield-prover` child reads one bounded request from an owner-only Unix socket. Fixed
owner configuration selects `current.pb`, the private snapshot store, guest ELF,
Plonk setup, expected key ID, and proof resource profile. The child calls B-0023,
executes the guest, creates one local Plonk proof, and compares the exact public
journal and key ID before returning a bounded public result. The default API build
does not include the SP1 SDK.

The B-0025 proof profile builds the launcher, prover child, and guest ELF through the
pinned offline toolchain. The persistent sandbox has a 60,129,542,144-byte cap,
zero swap, four CPUs, no network, a read-only root, fixed read-only trusted mounts,
and a bounded one-GiB tmpfs. The supervisor reaches it through an owner-only Unix
socket. One launcher starts one child, forwards cancellation, confirms exit, and
returns one strict bounded result. The supervisor deletes the private envelope, runs
direct Plonk verification, checks every public claim against the request and trusted
registry, then retains only the public capability and aggregate metrics in BullMQ.
Controlled process tests do not run SP1.

Live and focused tests cover
startup cleanup, exact duplicates, bound mutations, request ceilings, publication,
queue lifecycle, worker claim and cleanup, stalled failure, Redis restart recovery,
positive-path selection, caveated relationships, denial, snapshot mismatch, trusted
input assembly, private snapshot persistence, strict child transport, fixed cgroup
checks, launcher cancellation and exit, terminal capability publication, rendered
sandbox controls, and expiry bounds. `GetCapability`, `CancelCapability`, private
snapshot publication, and the retained-proof independent-verifier gate remain
pending.

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

The 64 GiB proof VM uses the separate `proof` profile. It requires at least
66,571,993,088 host bytes, the downloaded Plonk setup, a populated pinned Cargo
cache, and the two approved SP1 program-key values in `.env`. Check the rendered
boundary before startup:

```bash
./scripts/check-proof-sandbox-compose
./scripts/compose --env-file .env --profile services --profile proof up --build
```

Startup builds the fixed proof runtime. It does not generate a proof until a valid
capability job reaches the worker. The complete API proof-flow runbook remains
pending with the M4 lifecycle and verifier endpoints.

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
