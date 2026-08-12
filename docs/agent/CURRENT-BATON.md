# Current Engineering Baton

- Baton ID: B-0017
- State: BLOCKED
- Milestone: M4, capability issuance pre-queue admission
- Owner: Codex
- Started UTC: 2026-08-12T08:20:09Z
- Last updated UTC: 2026-08-12T08:30:25Z
- Branch: `main`
- Worktree: Proposed ADR-0014 and this blocked baton are checkpointed in commit
  `a7a8f37`. B-0016 is archived at
  `docs/agent/history/B-0016-capability-window-doc-consistency.md` and commit
  `8fa490b`.

## Objective

Implement a pure, bounded pre-queue admission boundary for generated
`CreateCapabilityRequest` values. Convert transport fields into the existing
ADR-0011 canonical request model, derive the agent-key thumbprint, rebuild the exact
request commitment, verify the Ed25519 issuance signature over that 32-byte digest,
enforce the accepted local Plonk validity window, and derive the deterministic job
identifier. Stop before storing private data or contacting Redis.

## Acceptance Criteria

- [ ] Generated request values convert through strict typed validation into the
      proof-critical canonical request model.
- [ ] Admission derives the agent thumbprint from the supplied Ed25519 public key and
      never trusts `agent_id` for authorization.
- [ ] The request signature verifies over exactly the ADR-0011 request commitment.
- [ ] The deterministic job identifier derives from that commitment.
- [ ] Admission enforces the 75-minute local Plonk window against trusted API time and
      rejects malformed, expired, or insufficient upper bounds.
- [ ] Every request field, context entry, public key, and signature mutation fails or
      changes the derived commitment before any queue operation.
- [ ] Errors and debug output contain no private context values, key bytes, signature
      bytes, or commitment bytes.
- [ ] Focused tests, affected workspace tests, strict Clippy, formatting, and diff
      checks pass.

## Architecture Constraints

- `agent_id` remains presentation metadata and cannot select the PolicyIR subject.
- Raw Protobuf bytes are never commitment inputs.
- Canonical encodings and commitment semantics stay in proof-critical Rust crates.
- Private request context cannot enter public errors or standard logs.
- The API does not publish a BullMQ job until admission succeeds.
- The proof worker must recheck validity bounds against its trusted start time in a
  later baton.
- ADR-0002 governs protected gateway requests, not this approved issuance signature.

## File Scope

- `services/razield/Cargo.toml`
- `services/razield/src/lib.rs`
- `services/razield/src/admission.rs`
- `services/razield/tests/` only for admission boundary coverage
- Existing canonical or authorization-core tests only if a missing exported helper is
  required without changing semantics
- `docs/decisions/0014-agent-key-thumbprint-and-issuance-identity.md`
- Current-status documentation only for behavior proven by this slice
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- BullMQ publication, queue state reads, cancellation, or workers
- Private envelope serialization, filesystem storage, or cleanup
- Witness construction, guest execution, proof generation, or proof verification
- Tonic handlers, listeners, interceptors, TLS, or transport policy
- Capability artifact issuance or protected-request verification
- ADR-0002 or O-007
- Dependencies not already approved and workspace-pinned
- Another proof run, M5, M6, M7, or another scenario

## Decisions

- Inherited: ADR-0011 defines the exact capability request commitment and issuance
  nullifier.
- Inherited: ADR-0013 accepts Ed25519 issuance authentication over that exact digest.
- Inherited: ADR-0013 accepts the 75-minute local Plonk capability window and a
  15-minute minimum usable period after a successful proof.
- Inherited: Existing workspace-pinned Rust crates can be selected by `razield`; no
  new package or version is approved.
- Made: None.

## Open Questions and Blockers

- BLOCKED: ADR-0011 and ADR-0013 do not define how the API derives the 32-byte agent
  key thumbprint from the supplied Ed25519 public key. This key-binding rule needs
  explicit maintainer approval before implementation.
- BLOCKED: ADR-0011 calls the issuance nullifier the duplicate key, but ADR-0013 uses
  the request commitment as BullMQ's custom job ID. Proposed ADR-0014 retains the
  later queue rule and keeps the nullifier as a separate proof-bound value.
- BLOCKED: ADR-0013's mutation test wording includes every request field, but
  `agent_id` is intentionally absent from the signed ADR-0011 commitment. Proposed
  ADR-0014 treats it as unsigned, non-authoritative display metadata.
- Fallback: DER, JWK, or a new domain-separated thumbprint can replace the proposal
  after explicit selection. Each option needs different cross-language canonical
  vectors and produces different PolicyIR subjects.
- Higher-cost fallback: binding `agent_id` changes ADR-0011 commitment bytes, the
  guest ELF, verification-key ID, M3 vectors, and proof artifacts. It requires an
  ADR-0011 amendment and a new M3 proof gate.
- The proof worker must repeat time-window validation at its trusted start time.
- ADR-0002 and O-007 remain open before protected-request gateway integration.
- A later positive isolated-verifier gate still needs explicit proof-run approval.

## Work Completed

- B-0015 compiled generated Rust service contracts.
- B-0016 reconciled the accepted capability-window documentation.
- Existing ADR-0011 canonical request and thumbprint helpers remain to be inspected
  before selecting the smallest adapter surface.
- B-0017 claimed at commit `8fa490b` with a clean worktree.
- Confirmed that no current code, vector, or accepted decision defines the key-to-
  thumbprint derivation.
- Drafted ADR-0014 with a fixed RFC 8032 vector and explicit signature, job-ID,
  metadata, and timestamp rules. No service code changed.
- Recorded migration and fallback costs. The proposed rule leaves M3 proof semantics
  and artifacts unchanged.
- Committed proposed ADR-0014 as `a7a8f37`. Its status remains proposed and no source
  code or dependency changed.
- The same explicit-approval blocker persisted across the initiating goal turn and
  two automatic continuation turns. No independent in-scope implementation remains.

## Validation Plan

- Run focused admission unit and mutation tests.
- Confirm no queue or filesystem operation occurs in the admission module.
- Run affected `razield`, authorization-core, and canonical tests.
- Run strict Clippy and repository formatting.
- Run `git diff --check` and inspect changed-file scope.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0016 archive | PASS | Capability-window documentation consistency | 2026-08-12T08:18:16Z |
| `git status --short` | PASS, empty | B-0017 start state | 2026-08-12T08:20:09Z |
| Active key-binding source search | BLOCKED | No thumbprint derivation formula exists | 2026-08-12T08:24:26Z |
| RFC 8032 key-vector SHA-256 | PASS | Proposed raw-key thumbprint vector | 2026-08-12T08:24:26Z |
| ADR-0014 completeness review | PASS | Trust, privacy, migration, tests, and rejected options recorded | 2026-08-12T08:27:00Z |
| `git status --short` after `a7a8f37` | PASS, empty | Proposed-decision checkpoint | 2026-08-12T08:30:25Z |

## Privacy Checks

- NOT STARTED: Public errors and debug output must not contain private request values.
- NOT STARTED: Tests must use synthetic context values only.
- NOT APPLICABLE: No proof, witness, relationship snapshot, or transaction data is
  processed in this baton.

## Risks and Private-Data Notes

- A transport-to-canonical mapping error can sign one request and prove another.
- Error formatting can leak private caveat context or key material.
- API-time validation alone cannot guarantee a 15-minute usable period after queue
  delay. The proof worker must fail closed at its trusted start time.

## Exact Next Action

Wait for explicit maintainer approval of ADR-0014. After approval, return B-0017 to
ACTIVE, record the approval, and implement the admission adapter against those exact
rules.
