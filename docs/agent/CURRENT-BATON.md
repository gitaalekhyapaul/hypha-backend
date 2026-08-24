# Current Engineering Baton

- Baton ID: B-0030
- State: READY
- Milestone: M4, isolated verifier worker and service activation
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-24T12:43:48Z
- Branch: `main`
- Worktree: B-0029 implementation and archive are ready for a scoped commit on top of
  `8670c9a`.

## Objective

Connect strict verifier admission to a resource-isolated local proof-verification
worker and expose the bounded `VerificationService` without any `razield` dependency.

## Acceptance Criteria

- [ ] Live verifier ingress cannot bypass the frozen-descriptor 2 MiB strict decoder.
- [ ] `verifierd` loads fixed local registry and paired program-key configuration with
      no `razield` endpoint.
- [ ] A versioned, bounded owner-only local protocol sends only proof bytes, expected
      public values, and trusted program-key identity to the verifier worker.
- [ ] The worker runs direct SP1 Plonk verification under the accepted provisional 2
      GiB, zero-swap, no-network, read-only-root, bounded-tmpfs, 60-second profile.
- [ ] Worker crash, timeout, malformed output, and resource exhaustion return typed
      failures without crashing `verifierd` or releasing candidate claims.
- [ ] Only successful worker output promotes admitted candidates to verified claims.
- [ ] Focused protocol, process, service, feature-tree, privacy, formatting, lint,
      type, and default workspace gates pass without generating a new proof.

## Architecture Constraints

- `verifierd` and its worker never call `razield` or read private graph data.
- Request data cannot select a socket, executable, proof mode, key, registry path, or
  resource profile.
- The worker receives no PolicyIR, relationship, caveat, salt, opening, path, witness,
  private envelope, or agent signing material.
- ADR-0011 public values and ADR-0002 request-signature semantics remain fixed.
- Direct `sp1-verifier 6.3.1` remains the verification engine. Proving and network
  dependencies stay excluded from the verifier build.
- No real SP1 proof may run or enter Git without separate maintainer approval.

## File Scope

- `services/verifierd/`
- Existing verifier-only crates and generated `VerificationService` contracts
- Verifier process and Compose configuration
- Focused scripts and tests for the verifier boundary
- Current-state README, architecture, specification, and baton documents

## Out Of Scope

- A new SP1 proof run or retained proof artifact
- `razield`, BullMQ, witness, prover, or snapshot behavior changes
- Gateway nonce storage, cache, and TypeScript canonicalization
- M5 frontend, M6 real-data acceptance, M7, or another scenario

## Decisions

- Inherited: accepted ADR-0002, ADR-0011, ADR-0012, and ADR-0013.
- Inherited: B-0029 strict admission emits candidate claims and a bounded public
  worker request.
- Inherited: the verifier profile has a provisional 2 GiB memory cap, zero swap, no
  network, read-only root, bounded tmpfs, and a 60-second deadline.
- Made: None.

## Open Questions And Blockers

- None for controlled implementation and tests.
- A later positive retained-proof gate needs explicit maintainer approval.

## Work Completed

- B-0029 completed and archived independent verifier admission.
- The verification-only dependency graph and generated service contracts compile.
- No isolated verifier process or live service binding exists yet.

## Validation Plan

- Inspect generated Tonic codec boundaries before selecting the strict ingress hook.
- Add controlled worker protocol, launcher, and failure tests without SP1.
- Run verifier feature-tree, service, privacy, formatting, lint, type, and default
  workspace checks.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0029 archive | PASS | Strict verifier admission and ADR-0002 | 2026-08-24T12:43:48Z |
| Isolated verifier worker | NOT AVAILABLE | B-0030 not started | 2026-08-24T12:43:48Z |

## Privacy Checks

- PASS: B-0029 admission accepts public request and capability fields only.
- REQUIRED: the worker request must remain public proof material only.
- REQUIRED: no proof or candidate claim can become verified before direct SP1
  verification succeeds.

## Risks And Private-Data Notes

- The generated Tonic server hardcodes its Prost codec. Strict raw-byte validation
  needs a service-edge design that cannot be bypassed by typed decoding.
- Process isolation must supply hard memory and deadline controls. Rust error handling
  alone cannot contain an OOM.
- The 2 GiB worker cap remains provisional until a real retained proof is measured.

## Exact Next Action

Commit the completed B-0029 admission boundary, then claim B-0030 and inspect the
generated Tonic service codec and existing one-shot launcher patterns.
