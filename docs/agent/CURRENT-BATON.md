# Current Engineering Baton

- Baton ID: B-0063
- State: READY
- Milestone: M6, external agent contract and policy provisioning design
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-09-25T07:22:18Z
- Branch: `main`
- Baseline: B-0062 completed the documentation plan. Runtime remains the M0-M5
  baseline at `26b92a7`. Sibling repositories exist and were not edited.

## Objective

Specify the versioned external-agent authentication and access-request contracts,
plus the policy provisioning migration needed for the approved M6-M7 plan.

## Acceptance Criteria

- [ ] Signed agent requests bind the registered key, token identity, method, target,
      payload, freshness, and replay fields through an explicit reviewed encoding.
- [ ] Request and proof-job states, error types, ownership, idempotency, dispatch
      reservations, cancellation, expiry, and offline-agent behavior are concrete.
- [ ] Independent policy publication preserves immutable versions and existing jobs.
- [ ] Shared SpiceDB schema provisioning and durable policy packages have a viable
      migration path without a proof-long publication lock.
- [ ] SDK interfaces, transport packaging, compatibility, and dependency pins are listed.
- [ ] B-0064 has a bounded first implementation slice and focused validation plan.

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

- `docs/decisions/` for the concrete protocol decision record
- `docs/spec/M6-M7-AGENT-DEMO-PLAN.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `ARCHITECTURE.md`
- `proto/raziel/v1/` for read-only comparison until the contract is settled
- `packages/sdk-ts/` and current service/portal modules for read-only migration analysis
- `docs/agent/CURRENT-BATON.md` and the B-0063 history record

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
- Exact protocol fields, runtime limits, and storage choices remain design work.

## Open Questions And Blockers

- No product question blocks the contract design.
- Sibling write access must be obtained before cross-repository implementation.
- Live endpoint/model credentials and npm ownership/license metadata are release inputs.
- Existing ADR-0018 cleanup and shared-schema behavior need migration analysis.

## Work Completed

- M0-M5 remain complete with their recorded evidence.
- B-0062 produced ADR-0019 and the plan with 15 user flows and six implementation steps.
- The old B-0061 real-data objective is archived as deferred and uncompleted.
- Current roadmap documents now agree on the revised M6-M7 scope.
- Both sibling repositories exist. The SDK repo's untracked `.DS_Store` was preserved.

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
| Runtime tests | SKIP | No runtime code changed | 2026-09-25T07:22:18Z |
| New API implementation | NOT AVAILABLE | This baton has not started | 2026-09-25T07:22:18Z |

## Risks And Private-Data Notes

- Authentication signatures do not replace proof capability authorization.
- Waiting requests must not exhaust the signed proof window or duplicate issuance.
- Publishing a policy must not retarget an admitted proof or invalidate a snapshot.
- LLM output and business evidence do not prove reconciliation accuracy.
- No keys, API tokens, private graph material, or real transaction data enter Git.

## Exact Next Action

Claim B-0063 and draft the concrete agent API/signature contract with the per-policy
SpiceDB provisioning and durable request-state migration.
