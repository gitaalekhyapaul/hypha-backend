# Engineering Baton Archive

- Baton ID: B-0026
- Final state: completed and archived
- Milestone: M4, private snapshot publication
- Owner: Codex
- Started UTC: 2026-08-23T17:01:20Z
- Completed UTC: 2026-08-24T05:48:03Z
- Branch: `main`
- Base commit: `ba12fcc`

## Objective

Implement the trusted administrative publication path that reads one exact SpiceDB
revision, creates the blinded relationship snapshot, stores the immutable private
record, and publishes the signed public snapshot record used by capability requests
and proof generation.

## Acceptance Criteria

- [x] The build path validates one exact SpiceDB revision and reads every approved
      PolicyIR resource type at that revision.
- [x] One fresh 32-byte salt creates the ADR-0009 root and stays out of public RPC
      output, Redis, logs, and the signed descriptor.
- [x] The ADR-0015 private snapshot publishes before the public registry record and
      stays bound to the exact snapshot ID, root, and policy.
- [x] Public publication signs with the configured snapshot authority key and commits
      through an atomic trusted-registry generation update.
- [x] Stale, duplicate, malformed, policy, SpiceDB, private-store, signing, and writer
      failures close with typed private errors and generic RPC errors.
- [x] Focused, live, privacy, formatting, lint, Compose, workspace, and diff gates pass
      without an SP1 proof run.

## Architecture Constraints

- ADR-0009 fixes blinded relationship leaves, tree order, salt size, and descriptor
  fields.
- ADR-0010 fixes snapshot signatures and trusted registry validation.
- ADR-0015 fixes immutable private storage and private-before-public ordering.
- ADR-0016 fixes exact SpiceDB reads, symbolic secret references, root-derived IDs,
  and the owner-only registry writer.
- `razield` cannot approve PolicyIR or write the trusted registry directly.
- The agent receives no PolicyIR, relationship, caveat value, salt, path, or opening.
- No SP1 proof run belongs to this baton.

## File Scope

- Approved dependencies and lockfile
- `services/razield` SnapshotService, SpiceDB adapter, tests, and runtime composition
- `tools/raziel-admin` reusable library and registry-writer process
- Compose service, secret mounts, private volumes, and profile controls
- ADR-0016, README, architecture, technical specification, and baton documents

## Decisions

- Accepted: `spicedb-rs-proto = "=1.53.0"` supplies generated Authzed API bindings.
- Accepted: `getrandom = "=0.4.3"` fills each fresh 32-byte snapshot salt.
- Accepted: a networkless `raziel-registry-writer` owns the writable registry mount.
- Made: the writer protocol is one strict, versioned, length-prefixed Protobuf frame
  over a `0700` directory and `0600` Unix socket.
- Made: the writer accepts only signed snapshot append and one-way snapshot revoke.
- Made: one reflection and at most 16 per-definition reads use one exact ZedToken and
  an aggregate 257-record rejection boundary.
- Made: the root-derived identifier is `snapshot-v1:<64 lowercase root hex>`.
- Made: the normal services profile initializes private snapshots and writer IPC. The
  proof profile does not start the registry writer.

## Work Completed

- Added the direct Authzed `v1.53.0` generated client and strict relationship adapter.
- Added exact revision evidence, bounded streams, canonical sorting, duplicate checks,
  wildcard and expiration rejection, and strict caveat scalar conversion.
- Added SnapshotService configuration, build, read, revoke, stable generic RPC errors,
  key and policy checks, fresh salt creation, signing, and registry confirmation.
- Added the strict registry-writer client, server, protocol, complete-generation
  rewrite, transition validation, atomic publication, and revocation path.
- Added the live SpiceDB compatibility and public SnapshotService lifecycle gates.
- Added Compose mounts, owner modes, writer health checks, secret paths, and the
  explicit `razield` binary command.
- Changed the pinned SpiceDB development command from removed `serve-testing` flags to
  the supported `serve` form.
- Updated current-state architecture, technical specification, README, and ADR status.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| Live Authzed `v1.53.0` client against SpiceDB `v1.54.0` | PASS | Schema reflection and exact relationship read |
| Live SnapshotService Build, Get, Revoke, Get | PASS | Public RPC, private store, writer, and registry reload |
| `cargo test -p raziel-admin` | PASS, 6 tests | Admin CLI and registry writer |
| `cargo test -p razield` | PASS | Service, adapter, private stores, queue, and worker |
| Affected policy, snapshot, authority, and verifier tests | PASS | Shared trust and commitment boundaries |
| `./scripts/dev pnpm test` | PASS | TypeScript, default Rust workspace, and Go suites |
| `./scripts/dev pnpm format:check` | PASS | Repository formatting |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, and Buf |
| `./scripts/check-proof-sandbox-compose` | PASS | Existing proof boundary unchanged |
| Services Compose render | PASS | Mounts, profiles, commands, and dependencies |
| Proof-profile service list | PASS | Registry writer absent |
| Redis, public registry, logs, and mode scan | PASS | Private snapshot inputs absent from public state |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | Outside B-0026 |

## Privacy Checks

- PASS: Redis contains only BullMQ library and concurrency metadata from this run.
- PASS: Public registry files contain no relationship ID, subject ID, symbolic secret
  reference, endpoint reference, or bearer token.
- PASS: Service logs contain no token, tuple, caveat value, salt, witness, or private
  snapshot content.
- PASS: The private snapshot directory is `0700` and its immutable file is `0600`.
- PASS: The writer has no network, signing key, SpiceDB token, Redis, or private
  snapshot mount.
- NOT APPLICABLE: Independent `verifierd` behavior remains a later M4 baton.

## Remaining Risks and Follow-Up

- A private snapshot can remain orphaned after a writer failure. Owner maintenance
  must never delete a snapshot referenced by an accepted root or active job.
- The exact opaque SpiceDB revision remains public under ADR-0010 and ADR-0016.
- Remote SpiceDB TLS and external secret stores remain outside the PoC profile.
- `GetCapability`, `CancelCapability`, and independent `verifierd` behavior remain.
- The VM API E2E runbook remains deferred until those backend paths form one flow.

## Exact Next Action

Claim B-0027 for authoritative `GetCapability` and `CancelCapability` lifecycle
behavior over the retained BullMQ job state.
