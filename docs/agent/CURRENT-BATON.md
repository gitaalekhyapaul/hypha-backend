# Current Engineering Baton

- Baton ID: B-0063
- State: REVIEW
- Milestone: M6, external agent contract and policy provisioning design
- Owner: Codex
- Started UTC: 2026-09-25T07:27:28Z
- Last updated UTC: 2026-09-25T07:49:10Z
- Branch: `main`
- Worktree: B-0062 planning is committed as `1b05501`. B-0063 adds a proposed ADR
  and API specification with related roadmap corrections. Runtime remains the M0-M5
  baseline. Sibling repositories are unchanged.

## Objective

Specify the versioned external-agent authentication and access-request contracts,
plus the policy provisioning migration needed for the approved M6-M7 plan.

## Acceptance Criteria

- [x] Signed agent requests bind the registered key, token identity, method, target,
      payload, freshness, and replay fields through an explicit reviewed encoding.
- [x] Request and proof-job states, error types, ownership, idempotency, dispatch
      reservations, cancellation, expiry, and offline-agent behavior are concrete.
- [x] Independent policy publication has a design preserving immutable versions and jobs.
- [x] Shared SpiceDB schema provisioning and durable policy packages have a viable
      migration path without a proof-long publication lock.
- [x] SDK interfaces, transport packaging, compatibility, and dependency pins are listed.
- [x] B-0064 has a bounded first implementation slice and focused validation plan.
- [ ] Maintainer approves ADR-0020 before the new security-boundary implementation.

Checked criteria above concern the reviewed design, not running feature code.

## Architecture Constraints

- Follow ADR-0019 and `docs/spec/M6-M7-AGENT-DEMO-PLAN.md`.
- M6 is external agent access and the public SDK. M7 is the three-dashboard demo.
- Use seeded or portal-created synthetic rows and real sequential local Plonk proofs.
- Preserve the guest, canonical commitments, existing capability signatures, and
  independent verifier. New agent API signatures need a separate explicit contract.
- Agents own their private keys. Tokens are bound to registered public keys.
- Policy publication does not create snapshots. Approved access requests trigger them.
- SDK responses never expose PolicyIR, relationship data, salts, or Merkle openings.

## File Scope

- `docs/decisions/0020-agent-api-and-request-provisioning.md`
- `docs/spec/AGENT-API-V1.md`
- `docs/spec/M6-M7-AGENT-DEMO-PLAN.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `ARCHITECTURE.md`
- `proto/raziel/v1/` for read-only comparison until the contract is settled
- `packages/sdk-ts/` and current service/portal modules for read-only migration analysis
- `docs/agent/CURRENT-BATON.md` and the B-0063 history record

Codex owns the documents. Completed read-only SDK/authentication analysis came from
Beauvoir. Boole reviewed policy migration and then API lifecycle consistency. Both
reviews are integrated and the helpers are closed.

## Out Of Scope

- Public npm publication, remote deployment, and remote pushes
- New guest semantics, private-policy evaluation in an agent, and parallel proofs
- Real-data acquisition or a second domain scenario
- Sibling writes before filesystem access is available

## Decisions

- Accepted: ADR-0019 records the maintainer's scope and two final clarifications.
- LLM integration assumes an OpenAI-compatible endpoint with configurable model and key.
- Agent B returns a bounded synthetic match result, settlement amount/reference,
  amount difference, and gateway receipt reference.
- Proposed, not yet approved: ADR-0020 and `docs/spec/AGENT-API-V1.md` specify the
  new signature domain, registered-key tokens, route isolation, request lifecycle,
  serial dispatch, per-version policy identities, and schema composition.
- Preserve strict current guest, registry epoch, capability, and protected-request rules.
- Keep private normalized intent data for the seven-day retry window. Delete private
  proof/issuance envelopes after terminal cleanup. No context enters queue payloads.

## Open Questions And Blockers

- Approval needed: ADR-0020's exact API-authentication boundary and per-version
  policy/schema provisioning amendment. Existing high-level product choices stand.
- Sibling write access must be obtained before cross-repository implementation.
- Live endpoint/model credentials and npm ownership/license metadata are release inputs.
- ADR-0018's single-schema write and candidate cleanup need the documented M6
  amendment. Runtime checks for the replacement remain future implementation work.

## Work Completed

- M0-M5 remain complete with their recorded evidence.
- B-0062 produced ADR-0019 and the plan with 15 user flows and six implementation steps.
- The old B-0061 real-data objective is archived as deferred and uncompleted.
- Current roadmap documents now agree on the revised M6-M7 scope.
- Both sibling repositories exist. The SDK repo's untracked `.DS_Store` was preserved.
- Added a complete proposed API contract with fields, endpoint ownership, error codes,
  expiry/cancellation/retry rules, SDK surface, and a mapping for AF-01 through AF-15.
- Policy migration retains immutable source packages, composes namespaced definitions,
  preserves old verification artifacts, and binds gateway routes to finalized data.
- Integrated six policy review findings and four API review findings. These include
  owner-qualified request IDs, cancellation during uncertain submission, private
  idempotency retention, and refresh of both query signatures on read retries.
- Corrected stale current documentation about SDK repository separation and the
  already-implemented M5 capability hex transport. Historical evidence is unchanged.

## Validation Plan

- Compare proposed signatures and fields with existing ADR-0002, ADR-0011, ADR-0014,
  ADR-0018, Protobuf contracts, registry behavior, and signing vectors.
- Map every approved user flow to an endpoint and owning repository.
- Validate modified documentation links and run `git diff --check`.
- Schedule only the focused cross-language and behavior tests required by later code.

## Validation Evidence

| Check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0062 document links and JSON | PASS | 46 relative targets and plan example | 2026-09-25T07:22:18Z |
| B-0062 whitespace and scope review | PASS | Documentation-only changes | 2026-09-25T07:22:18Z |
| Static policy migration review | PASS | Six findings incorporated into proposed contract, runtime untested | 2026-09-25T07:49:10Z |
| Static API lifecycle review | PASS | Four findings incorporated, existing signature/expiry rules compared | 2026-09-25T07:49:10Z |
| Documentation targets and JSON examples | PASS | 38 relative targets, unchanged Mermaid blocks | 2026-09-25T07:49:10Z |
| `git diff --check` | PASS | Tracked documentation whitespace | 2026-09-25T07:49:10Z |
| Sibling status | PASS | SDK keeps pre-existing `.DS_Store`; agent repo clean | 2026-09-25T07:49:10Z |
| Runtime tests, Docker, SSH | SKIP | Documentation-only work, no runtime changes | 2026-09-25T07:49:10Z |
| New API implementation and cross-language vectors | NOT AVAILABLE | Pending approval and B-0064 | 2026-09-25T07:49:10Z |

## Risks And Private-Data Notes

- Authentication signatures do not replace proof capability authorization.
- Waiting requests must not exhaust the signed proof window or duplicate issuance.
- Publishing a policy must not retarget an admitted proof or invalidate a snapshot.
- LLM output and business evidence do not prove reconciliation accuracy.
- No keys, API tokens, private graph material, or real transaction data enter Git.
- These proposed controls have static review only. They do not establish implemented
  authentication, deployment isolation, or proof dispatch behavior.

## Exact Next Action

Obtain approval for the two decisions in ADR-0020. Then archive B-0063, claim B-0064,
and implement registration/token UI, strict Rust API authentication, `/me`, and the
standalone SDK signer/client foundation. Request scoped sibling write permission when
that first external package edit is ready. No VM deployment or npm publication yet.
