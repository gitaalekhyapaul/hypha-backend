# B-0009: Authority Signatures and Local Registries

- Final state: completed
- Milestone: M2, Canonical commitments and snapshots
- Owner: Codex
- Started UTC: 2026-07-23T05:03:36Z
- Closed UTC: 2026-07-23T11:45:01Z
- Branch: `main`
- Base commit: `2aa73f2`

## Objective

Define signed snapshot descriptors and local approved-policy and accepted-root
registries. Resolve O-004 signing and key-storage rules before implementation.

## Acceptance Criteria

- [x] One accepted decision record fixes the snapshot signing algorithm, trusted key
      identifiers and storage, canonical signed bytes, rotation, and failure rules.
- [x] Rust signs and verifies descriptors that bind the exact policy hash,
      relationship root, SpiceDB revision, epoch, issue time, expiry, and
      canonicalization version.
- [x] Local registries fail closed for unapproved policies, unknown roots, revoked
      roots, expired descriptors, unknown keys, and invalid signatures.
- [x] Deterministic vectors and one-field mutation tests cover descriptor signatures
      and registry status transitions.
- [x] Focused and affected workspace gates pass.

## Architecture Constraints

- `razield` cannot mark its own PolicyIR as approved.
- `verifierd` resolves `policy_hash` through its trusted local policy registry.
- Every proof binds an approved policy record and signed snapshot descriptor.
- The verifier never calls `razield`.
- Descriptor bytes and signatures contain no relationship tuple, salt, opening,
  satisfying path, or stored caveat value.
- Raw Protobuf bytes are never signature or commitment inputs.
- Policy authorization remains separate from dataset relevance.

## File Scope

- `ARCHITECTURE.md`, `README.md`, and `docs/spec/POC-TECHNICAL-SPEC.md`
- ADR-0010 and baton records
- Authority, policy, and snapshot Protobuf contracts
- `crates/canonical/`, `crates/snapshot/`, `crates/authority/`, and
  `crates/verifier/`
- `tools/raziel-admin/`
- TypeScript vector parsing and generated exports
- Compose, local environment examples, ignore rules, and workspace command fixes
- Deterministic authority signature vectors and tests

## Out of Scope

- SpiceDB relationship loading and ZedToken retrieval
- Witness path search and PolicyIR evaluation
- SP1 guest execution and proof generation
- Capability issuance, gateway enforcement, SDK, and console
- Rust service process wiring, which remains M4 work
- Real transaction data
- Production hardware key management
- Private-key generation inside Raziel

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0009.
- Accepted ADR-0010: strict Ed25519 signatures, separate role keys, canonical
  signature frames, external PKCS#8 key provisioning, trust-domain binding,
  monotonic epochs, and immutable local registry generations.
- The maintainer approved the exact `ed25519-dalek = 3.0.0` dependency.
- Registry JSON files are operator projections. Verification reads strict Protobuf.
- A trusted snapshot publisher allocates each next epoch from its accepted
  high-water mark.

## Open Questions and Blockers

- None for B-0009.
- The normal development `spicedb` service has a pre-existing command and key
  mismatch. It did not block M2.
- Deferred follow-up: after the M7 exit gate passes and the PoC meets its definition
  of done, remind the maintainer about the separate Hypha relevance-attestation
  scope, the Obsidian `Current Implementation Discussion Post`, and the TEE-backed
  local-authorization experiment in `docs/spec/POC-TECHNICAL-SPEC.md`.

## Work Completed

- Added canonical policy-approval and snapshot-descriptor claims and signature
  frames.
- Added host-only role-safe Ed25519 signing, strict verification, derived key
  identifiers, secure PKCS#8 loading, and trusted snapshot epoch allocation.
- Added strict frozen-descriptor Protobuf ingress and verifier-local policy, root,
  authority-key, and manifest models.
- Added generation rollback checks, one-way revocation, policy rotation rules, clock
  rollback checks, status checks, and local policy revalidation on every install.
- Added `raziel-admin` commands for key inspection, policy approval, immutable
  registry publication, and revocation records.
- Added role-specific read-only Compose mounts. `verifierd` receives public registry
  data and no private key.
- Added deterministic Rust and TypeScript authority vectors with 23 negative cases.
- Added bounded reads, symbolic-link rejection, file-identity checks, owner-only key
  permission checks, durable file sync, and no-replace publication.
- Updated the root build to run Next packages sequentially and disable Go VCS
  stamping inside the container mount.
- Updated Go file discovery with a command-scoped trusted `/workspace` mount. No
  global Git setting changes.
- Completed two independent reviews. Both final reviews reported no actionable
  finding.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm format:check` | PASS | Biome formatting, Rustfmt, and Gofmt | 2026-07-23T11:45:01Z |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy with warnings denied, Go vet, and Buf | 2026-07-23T11:40:33Z |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust workspace, and Go compile checks | 2026-07-23T11:37:18Z |
| `./scripts/dev pnpm test` | PASS | TypeScript, Rust workspace, and Go tests | 2026-07-23T11:38:07Z |
| `./scripts/dev pnpm generate:check` | PASS | Deterministic Protobuf generation | 2026-07-23T11:38:13Z |
| `./scripts/dev pnpm build` | PASS | Next.js, TypeScript, Rust, SP1 guest, and Go builds | 2026-07-23T11:39:05Z |
| `./scripts/dev pnpm bootstrap:check` | PASS | Pinned Linux ARM64 toolchain | 2026-07-23T11:41:02Z |
| `./scripts/dev pnpm test:e2e` | SKIP | No package defines an end-to-end test script | 2026-07-23T11:41:02Z |
| Compose configuration | PASS | Service and admin profiles | 2026-07-23T11:41:02Z |
| Admin Compose dispatch | PASS | CLI entrypoint reached and rejected missing required options | 2026-07-23T11:42:02Z |
| Service UID checks | PASS | `razield` and `verifierd` run as UID 1000 | 2026-07-23T11:42:02Z |
| Guest dependency tree | PASS | Guest depends only on `sp1-zkvm` | 2026-07-23T11:41:02Z |
| Frozen descriptor checksum | PASS | SHA-256 `610b682ae00a2cfa0f2efa5beeb7e9e6e47735b0b2570a978a51a017e536aa28` | 2026-07-23T11:41:02Z |
| Two independent final reviews | PASS | No remaining actionable finding | 2026-07-23T11:34:20Z |
| `git diff --check` and private-key scan | PASS | No whitespace error, key file, or private-key block | 2026-07-23T11:42:02Z |

## Privacy Checks

- PASS: No private key file or PEM private-key block entered the worktree.
- PASS: Authority vectors use deterministic synthetic test keys.
- PASS: Descriptor and registry artifacts contain no relationship tuple, salt,
  opening, satisfying path, or stored caveat value.
- PASS: `verifierd` has no private-key mount.
- PASS: Host-only authority code does not enter the SP1 guest dependency graph.

## Risks and Private-Data Notes

- Operator-managed key files need owner-read-only mode and storage outside the
  repository.
- Registry generation publication must remain monotonic and immutable.
- M4 service wiring must consume these libraries without adding a `verifierd` to
  `razield` call.
- Future proof public values must not expose policy text, graph data, relationship
  data, caveat values, paths, salts, or openings.

## Follow-Up

Start B-0010. Define the exact SP1 guest input and public-value contract, then
implement execute-mode equivalence before local proof generation.
