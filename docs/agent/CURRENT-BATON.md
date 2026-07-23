# Current Engineering Baton

- Baton ID: B-0009
- State: BLOCKED
- Milestone: M2, Canonical commitments and snapshots
- Owner: Codex
- Started UTC: 2026-07-23T05:03:36Z
- Last updated UTC: 2026-07-23T05:25:05Z
- Branch: `main`
- Worktree: `main` at `d2427ea` with only the B-0009 baton and proposed ADR-0010
  changed.

## Objective

Define signed snapshot descriptors and local approved-policy and accepted-root
registries. Resolve O-004 signing and key-storage rules before implementation.

## Acceptance Criteria

- [ ] One accepted decision record fixes the snapshot signing algorithm, trusted key
      identifiers and storage, canonical signed bytes, rotation, and failure rules.
- [ ] Rust signs and verifies descriptors that bind the exact policy hash, relationship
      root, SpiceDB revision, epoch, issue time, expiry, and canonicalization version.
- [ ] Local registries fail closed for unapproved policies, unknown roots, revoked
      roots, expired descriptors, unknown keys, and invalid signatures.
- [ ] Deterministic vectors and one-field mutation tests cover descriptor signatures
      and registry status transitions.
- [ ] Focused and affected workspace gates pass.

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

- `ARCHITECTURE.md` and `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/decisions/0010-authority-signatures-and-local-registries.md`
- `proto/raziel/v1/` and frozen descriptor sets for accepted wire corrections
- `crates/canonical/`, `crates/snapshot/`, and `crates/verifier/`
- proposed host-only `crates/authority/`
- proposed Rust operator binary under `tools/raziel-admin/`
- `.dockerignore` and `docker-compose.yml` for exact role-specific mounts
- Deterministic descriptor and registry vectors and tests
- Cargo metadata only after dependency approval
- Baton records

## Out of Scope

- SpiceDB relationship loading and ZedToken retrieval
- Witness path search and PolicyIR evaluation
- SP1 guest execution and proof generation
- Capability issuance, gateway enforcement, SDK, and console
- Gateway cache invalidation, which remains M5 work
- Stopped-`razield` service testing, which remains M4 work
- Real transaction data
- Production hardware key management
- Private-key generation inside Raziel

## Decisions

- Inherited: D-001 through D-017 in `docs/spec/POC-TECHNICAL-SPEC.md`.
- Inherited: ADR-0001 through ADR-0009.
- ADR-0008 fixes canonical SHA-256 framing and makes Rust the commitment authority.
- ADR-0009 fixes the exact relationship root and private witness protocol.
- Proposed ADR-0010 resolves O-004 and O-006 with strict Ed25519, separate role keys,
  canonical signature payloads, external PKCS#8 key provisioning, trust-domain
  binding, monotonic epochs, and local registry generations.

## Open Questions and Blockers

- Maintainer approval of proposed ADR-0010 is required before descriptor, signature,
  registry, wire-contract, or key-mount implementation.
- Dependency approval is required for exact `ed25519-dalek` 3.0.0 with only `alloc`,
  `pkcs8`, `signature`, and `zeroize`.
- Scope approval is required for the host-only `crates/authority` crate, the
  `tools/raziel-admin` binary, and the listed Protobuf corrections.
- The normal development `spicedb` service has a pre-existing command and key
  incompatibility. It does not block descriptor and registry design.
- Deferred follow-up: carry this item through each baton reset. After the M7 exit gate
  passes, remind the maintainer to revisit the separate Hypha relevance-attestation
  scope and the Obsidian `Current Implementation Discussion Post`.

## Work Completed

- B-0008 implemented and validated the accepted blinded relationship-tree protocol.
- B-0008 is archived as
  `docs/agent/history/B-0008-blinded-relationship-tree.md`.
- The remaining M2 deliverables are the signed snapshot descriptor and local policy
  and accepted-root registries.
- Reconciled B-0009 against clean `main` at `d2427ea` and claimed protocol inspection
  and O-004 drafting as the only active phase.
- Inspected the policy and snapshot Protobuf contracts, architecture, technical
  specification, strict PolicyIR ingress, registry placeholders, crate graph, Compose
  mounts, ignored secrets, and current Cargo dependency graph.
- Drafted proposed ADR-0010 without changing accepted behavior, Protobuf, dependencies,
  Compose, or Rust implementation.
- Completed independent cryptographic and repository-compatibility reviews.
- Incorporated findings for strict Ed25519 acceptance, `u64` bytes, deployment trust
  domains, exact policy-record binding, policy renewal epochs, trusted snapshot epoch
  allocation, one-way status, registry generations, cold-verification cache semantics,
  strict persisted Protobuf, role-specific mounts, and crate ownership.
- Prepared a documentation-only checkpoint. It changes no accepted architecture,
  Protobuf contract, dependency, Compose mount, or Rust behavior.

## Validation Plan

- Deterministic canonical descriptor and signature vectors
- Signature, key, field, expiry, revocation, and registry mutation tests
- Rust focused tests and affected workspace tests
- Format, lint, typecheck, build, generation, `git diff --check`, and privacy scan

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0008 archive | PASS | Blinded relationship-tree objective completed with all gates | 2026-07-23T05:00:24Z |
| Descriptor and registry contract inspection | PASS | Existing fields, missing trust rules, crate boundaries, and persistence gaps identified | 2026-07-23T05:08:12Z |
| Official Ed25519 dependency review | PASS | `ed25519-dalek` 3.0.0 supports strict verification, PKCS#8, zeroizing keys, and the pinned Rust toolchain | 2026-07-23T05:10:41Z |
| Two independent ADR reviews | PASS after findings incorporated | Cryptographic, rollback, rotation, cache, persistence, Compose, and scope findings addressed | 2026-07-23T05:22:40Z |
| `./scripts/dev pnpm format:check` | PASS | Proposed ADR and baton do not disturb repository formatting | 2026-07-23T05:14:22Z |
| `git diff --check` | PASS | Proposed ADR-0010 and current baton | 2026-07-23T05:14:30Z |
| Final proposal scope review | PASS | Only proposed ADR-0010 and the B-0009 baton differ from `d2427ea` | 2026-07-23T05:25:05Z |

## Risks and Private-Data Notes

- A descriptor signature is a public trust artifact. Ambiguous bytes, weak key
  identity, or stale registry status can turn a valid signature into an invalid trust
  decision.
- Test keys must be synthetic and marked non-production.
- Private relationship and witness material must never enter descriptor vectors,
  registry records, errors, or logs.

## Exact Next Action

Obtain explicit maintainer approval for ADR-0010, the listed crate and Protobuf scope,
and exact `ed25519-dalek` 3.0.0 dependency. After approval, mark the ADR accepted,
align architecture and specification, then implement the approved B-0009 scope.
