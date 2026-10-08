# Hypha Backend: Raziel PoC

This repository contains the first Raziel proof of concept. Raziel turns an approved
SpiceDB authorization result into an independently verifiable, proof-backed capability
for an AI agent.

The first scenario is payment reconciliation. The current goal is a public SDK and
three-dashboard demo with two external agents, labeled synthetic transaction records,
and real local Plonk proofs. Real-data acceptance is deferred under
[ADR-0019](docs/decisions/0019-public-sdk-and-two-agent-reconciliation.md).

## Current status

M0 through M5 are complete. M3 produced and independently verified a real local SP1
Plonk proof. The approved eight-CPU profile records a 2,537-byte proof,
929,660-millisecond proving time, 438-millisecond verification time, and a
45,592,842,240-byte memory peak. See the
[`cluster-20260813T063649Z-8cpu` benchmark](docs/benchmarks/sp1-plonk/cluster-20260813T063649Z-8cpu.md).

M4 is complete under accepted ADR-0013 through ADR-0017. `razield` runs bounded
CapabilityService and SnapshotService Tonic ingress. `verifierd`
now has a live strict public service boundary under accepted ADR-0002. It rejects
unknown and noncanonical Protobuf, validates the agent key and exact protected-request
signature, checks trusted time and local registry state, and reconstructs the complete
expected proof journal. It sends only bounded public proof input to a networkless
one-shot verifier under a 2 GiB, zero-swap, read-only, 60-second sandbox. Only direct
SP1 verification success promotes candidate claims. Controlled crash, malformed
output, timeout, and rejection tests pass. B-0031 retained one public Plonk fixture
and verified it with every `razield` role stopped. The proof took 932,309 milliseconds
and peaked at 46,271,315,968 bytes. The isolated verifier accepted it in 447
milliseconds, peaked at 8,146,944 bytes, used no swap, and rejected eight bound-field
mutations. See the [proof benchmark](docs/benchmarks/sp1-plonk/cluster-20260825T182353Z-18591.md)
and [verifier benchmark](docs/benchmarks/sp1-verifier/verifier-20260825T192214Z-34749.md).
The final B-0032 run submitted one signed request, queued one BullMQ job, generated
a real local Plonk proof, published the terminal capability, stopped the issuer and
prover, and verified the result independently. Proving took 955,516 milliseconds and
peaked at 47,003,119,616 bytes under the 56 GiB cap with no swap or OOM event. The
verifier rejected an altered capability and a stolen key binding. See the
[complete M4 service-flow report](docs/benchmarks/m4-service-flow/m4-service-20260826T090300Z-199729.md).
The BullMQ `1.2.3` and Redis `8.2.8` conformance gate passes. The
pure `razield` admission
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
zero swap, eight CPUs, no network, a read-only root, fixed read-only trusted mounts,
and a bounded one-GiB tmpfs. The supervisor reaches it through an owner-only Unix
socket. One launcher starts one child, forwards cancellation, confirms exit, and
returns one strict bounded result. The supervisor deletes the private envelope, runs
direct Plonk verification, checks every public claim against the request and trusted
registry, then retains only the public capability and aggregate metrics in BullMQ.
Controlled process tests do not run SP1.

The B-0026 snapshot path resolves one approved PolicyIR record and active snapshot
authority key, reflects and reads one exact SpiceDB revision, generates a fresh
32-byte salt, and publishes the immutable private snapshot before any public root.
`razield` asks a separate networkless `raziel-registry-writer` process to append or
revoke only snapshot-root records. The writer owns the writable registry mount and
atomically advances a fully validated generation. `razield` keeps a read-only mount.
`BuildSnapshot`, `GetSnapshot`, and `RevokeSnapshot` pass a live Compose lifecycle
gate against SpiceDB `v1.54.0` through the pinned Authzed `v1.53.0` generated API.

The B-0027 capability lifecycle reads strict public state from retained BullMQ
records. `GetCapability` returns queued, executing, proving, succeeded, failed, or
canceled jobs. `CancelCapability` publishes one durable command on a separate
`cancellation-commands` queue. The proof supervisor removes waiting jobs or signals
active work, waits for private cleanup, and retains a closed cancellation outcome.
Bounded snapshot retries cover BullMQ state transitions. Startup recovery finishes
commands retained before or after proof-job removal.

Live and focused tests cover
startup cleanup, exact duplicates, bound mutations, request ceilings, publication,
queue lifecycle, worker claim and cleanup, stalled failure, Redis restart recovery,
positive-path selection, caveated relationships, denial, snapshot mismatch, trusted
input assembly, private snapshot persistence, strict child transport, fixed cgroup
checks, launcher cancellation and exit, terminal capability publication, rendered
sandbox controls, exact-revision snapshot publication, atomic root registration,
revocation, expiry bounds, public lifecycle projection, queued and active
cancellation, duplicate cancellation, and cancellation recovery. The retained-proof
independent-verifier gate and the complete B-0032 service-flow gate pass.

M5 completed the synthetic desktop demonstration on the NUS VM. The portal created
and finalized a synthetic transaction revision, compiled and activated a visual
SpiceDB policy, published a key-bound snapshot, and generated one live Plonk
capability. The first protected query used the independent verifier. The second used
the verified-capability cache with a fresh nonce. Scope, replay, expired, altered,
and unsigned requests returned no rows. Exact receipt filtering and JSONL export
passed. The [M5 portal runbook](docs/runbooks/M5-LOCAL-PORTAL.md) contains the public
evidence and screenshots.

The revised M6 delivers per-agent registration, access-request review, sequential
proof dispatch, and the public `@gitaalekhyapaul/raziel-sdk-ts` package. M7 delivers
Agent A and Agent B dashboards, their message channel, and the complete two-ledger
reconciliation demonstration. B-0064 implements agent registration, token management,
signed identity lookup, and a standalone SDK candidate. B-0065 adds durable agent
requests and an operator inbox with detail, approve, reject, and cancel actions.
Private snapshot preparation passes locally against real SpiceDB with exact retry
reuse. B-0066 adds serial preparation, final signature acceptance, proof-service
handoff, and retained public results. The SDK `0.1.0` was published on 2026-10-01.
Its public registry metadata, `latest` tag, and archive integrity are verified.
B-0067 adds
both agent dashboards, persistent separate keys, bounded LLM tools, an authenticated
message channel, and editable synthetic processor/bank seed drafts. Local checks and
Docker desktop startup pass. The two-agent proof-backed VM flow remains pending.
Owner-allocated policy identities and namespaced candidate compilation
pass local checks. The composed owner publication path passed a two-version test
against pinned SpiceDB. The visual builder now publishes versions in the isolated,
opt-in M6 runtime. Its two-version browser flow and installed-approval catalogue pass
locally. Approval writes the immutable dataset mapping checked by the gateway.
B-0066 adds packaged SDK signing, bounded polling, and protected-query reuse.
Cold/cache delivery passes with a controlled verifier. The new live M6 overlay
connects the production roles and uses dedicated PostgreSQL-backed SpiceDB.
Live VM proof validation remains pending. Agent source now pins the public SDK
`0.1.0` and passes frozen-install, agent-check, and production-build gates. The VM
has not received this dependency migration. The backend shim keeps its candidate
pin. See the
[publication record](docs/agent/history/2026-10-01-sdk-publication-attempt.md).
No new M6 proof-backed demonstration is claimed. See the
[feature and user-flow plan](docs/spec/M6-M7-AGENT-DEMO-PLAN.md). The former real-data
and second-scenario gates are deferred. Existing runbooks describe the completed M5
stack. The [access inbox runbook](docs/runbooks/AGENT-ACCESS-INBOX.md#local-m6-preview)
documents the partial local M6 preview. The [M6 VM runbook](docs/runbooks/M6-LIVE-VM.md)
documents the separate provisional live deployment and its remaining proof gate.
The [three-dashboard runbook](docs/runbooks/M7-AGENT-DEMO.md) adds the agent containers,
SSH port forwards, free HTTPS tunnel option, and reconciliation recording flow.
The [agent registration runbook](docs/runbooks/AGENT-REGISTRATION.md) covers the new
authentication slice and its current limits. The
[access inbox runbook](docs/runbooks/AGENT-ACCESS-INBOX.md) covers the connected local
request flow and ownership boundaries.

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

The `services` profile fails closed until `.env` points to a valid published registry,
snapshot authority key, SpiceDB bearer-token file, and exact ZedToken file. The two
token files must contain one value with no trailing newline and no group or public
permission bits.

The 64 GiB proof VM uses the separate `proof` profile. It requires at least
66,571,993,088 host bytes, the downloaded Plonk setup, a populated pinned Cargo
cache, and the two approved SP1 program-key values in `.env`. Check the rendered
boundary before startup:

```bash
./scripts/check-proof-sandbox-compose
./scripts/compose --env-file .env --profile services --profile proof up --build
```

Startup builds the fixed proof and verifier runtimes. It does not generate a proof
until a valid capability job reaches the proof worker. The
[B-0032 service-flow runbook](docs/runbooks/B-0032-M4-SERVICE-FLOW.md) reproduces the
completed M4 API, queue, proof, and independent-verifier acceptance run. The separate
[B-0031 retained-proof verifier runbook](docs/runbooks/B-0031-RETAINED-PLONK-VERIFIER.md)
covers the one approved public-fixture generation and isolated verification run.

## Authority administration

Create separate policy and snapshot keys outside the repository:

```bash
openssl genpkey -algorithm ED25519 -outform DER -out policy-authority.pk8
openssl genpkey -algorithm ED25519 -outform DER -out snapshot-authority.pk8
chmod 0400 policy-authority.pk8 snapshot-authority.pk8
```

Copy `.env.example` to `.env`. Set `HOST_UID` from `id -u` and `HOST_GID` from
`id -g`. Set both key paths and both token paths to absolute host paths. The SpiceDB
token file contains the configured preshared key. The ZedToken file contains the
exact revision returned after the approved relationship import. Do not add a newline
or commit either file.

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

At runtime, `raziel-registry-writer` is the only service with a writable registry
mount. Its owner-only Unix socket accepts signed snapshot append and one-way snapshot
revocation operations. Policy and authority-key changes still require the offline
admin commands above and a writer restart.
