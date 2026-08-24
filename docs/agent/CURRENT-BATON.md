# Current Engineering Baton

- Baton ID: B-0029
- State: READY
- Milestone: M4, independent verifier admission
- Owner: unassigned
- Started UTC: None
- Last updated UTC: 2026-08-24T11:50:17Z
- Branch: `main`
- Worktree: B-0028 is archived. Inspect Git before claiming B-0029.

## Objective

Close the protected-request signature decision, then implement the strict public
admission boundary that prepares one capability for independent verification without
calling `razield` or reading private graph data.

## Acceptance Criteria

- [ ] ADR-0002 is reviewed and explicitly accepted before its signature rules enter
      production code.
- [ ] `VerifyCapabilityRequest` applies the 2 MiB service ceiling and rejects missing,
      malformed, oversized, unknown, and noncanonical public fields.
- [ ] Admission validates agent-key thumbprint binding and the exact protected-request
      Ed25519 signature envelope from ADR-0002.
- [ ] Trusted time, capability expiry, request timestamp, nonce, method, target, and
      body-hash checks fail closed with typed private causes and generic public errors.
- [ ] Verifier-owned registries resolve the exact policy, snapshot, authority keys,
      and program-key identity without a `razield` endpoint.
- [ ] Admission reconstructs the complete expected ADR-0011 public-value bytes and
      emits only a bounded public worker request.
- [ ] Focused canonical, verifier, service, mutation, privacy, formatting, lint, and
      diff gates pass without running SP1.

## Architecture Constraints

- `verifierd` never calls `razield`.
- The capability carries `policy_hash`, not PolicyIR. The verifier resolves the exact
  approved policy from its read-only local registry.
- No relationship, caveat value, salt, opening, path, witness, or private envelope
  enters verifier admission.
- Request data cannot select proof mode, setup, verification key, registry path,
  worker endpoint, or resource profile.
- ADR-0011 fixes proof public values. B-0029 cannot change them.
- The isolated verifier worker and its cgroup activation remain a later bounded baton.
- No package, CLI, runtime, or system dependency is approved by this baton.

## File Scope

- ADR-0002 and O-007 status documentation
- Existing canonical, authority, verifier, prover verification, and Protobuf contracts
- `services/verifierd` pure admission and focused tests
- Current-state README, architecture, specification, and baton documents

## Out of Scope

- Isolated verifier worker process, socket, cgroup, and real retained-proof run
- `razield` lifecycle, queue, prover, witness, or snapshot changes
- Gateway replay database and capability cache
- M5 frontend, M6 real-data acceptance, M7, or another scenario

## Decisions

- Inherited: ADR-0010, ADR-0011, ADR-0012, and ADR-0013.
- Pending maintainer review: ADR-0002 protected-request signature envelope.
- Inherited: B-0028 fixes the eight-CPU proof profile. Verifier admission does not
  alter that profile.

## Open Questions and Blockers

- ADR-0002 remains proposed. Its exact method, target, body-hash, nonce, timestamp,
  capability-ID, and domain-separation rules need maintainer approval before coding.
- O-007 remains open until that approval is recorded.

## Work Completed

- B-0027 completed and archived capability lifecycle RPCs.
- B-0028 completed and archived the validated eight-CPU proof profile.
- `services/verifierd` remains a shell with the approved verification-only dependency
  graph.
- Generated `VerificationService` request and response contracts already compile.

## Validation Plan

- Review ADR-0002 against the current Protobuf request and ADR-0013 verifier boundary.
- Request explicit maintainer approval with a short field-by-field explanation.
- After approval, implement the smallest pure admission path before service or worker
  process activation.
- Run canonical mutation, key binding, time, registry, privacy, formatting, lint, and
  default workspace tests.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0028 archive | PASS | Eight-CPU proof resource profile | 2026-08-24T11:50:17Z |
| ADR-0002 status | BLOCKED, maintainer approval needed | Protected-request signature semantics | 2026-08-24T11:50:17Z |
| B-0029 implementation | NOT AVAILABLE | Baton not claimed | 2026-08-24T11:50:17Z |

## Privacy Checks

- NOT AVAILABLE: B-0029 has not started.
- REQUIRED: verifier admission must process public capability and request-envelope
  data only.

## Risks and Private-Data Notes

- A signature that omits method or canonical target permits cross-route replay.
- Target canonicalization must have one byte representation across the TypeScript
  gateway and Rust verifier.
- Capability proof verification does not replace per-request signature and replay
  checks.
- Replay storage belongs to the gateway in a later baton. B-0029 validates the signed
  replay inputs but does not claim nonce consumption.

## Exact Next Action

Read ADR-0002 against the generated verifier request, claim B-0029, and ask the
maintainer to approve or amend the exact protected-request signature envelope.
