# Current Engineering Baton

- Baton ID: B-0031
- State: READY
- Milestone: M4, retained proof through the independent verifier
- Owner: unassigned
- Started UTC: not started
- Last updated UTC: 2026-08-25T07:22:35Z
- Branch: `main`
- Worktree: No B-0031 implementation changes. B-0030 is the prior scoped commit.

## Objective

Verify one retained public synthetic Plonk artifact through the isolated worker with
every `razield` role stopped, then measure the verification-only resource profile.

## Acceptance Criteria

- [ ] The maintainer explicitly approves one synthetic Plonk rerun and retention of
      its public zero-knowledge proof artifact.
- [ ] The approved VM profile produces the proof without retaining witness, private
      journal input, or debug traces.
- [ ] `verifierd` verifies the retained proof through its isolated worker with all
      `razield` roles stopped.
- [ ] Mutated proof, public journal, key identity, policy, root, scope, expiry, and
      request-signature cases fail closed.
- [ ] Verification-only peak memory stays under the provisional 2 GiB cap or a new
      decision records the measured requirement.
- [ ] Durable public evidence records source commit, artifact checksum, key ID,
      timing, peak memory, and zero swap.

## Architecture Constraints

- `verifierd` never calls `razield`.
- Retain only the public Plonk proof artifact. Delete all witness and private inputs.
- Use direct `sp1-verifier 6.3.1` in the fixed one-shot worker.
- Do not change public values, canonical hashes, proof mode, or resource limits inside
  the measurement task.

## File Scope

- Retained public proof fixture and provenance manifest
- Isolated-verifier integration and mutation tests
- Verifier benchmark evidence
- M4 current-state and baton documents

## Out Of Scope

- Gateway nonce storage, cache, and TypeScript canonicalization
- M5 frontend and synthetic transaction workflow
- M6 real transaction acceptance
- M7 or another scenario

## Decisions

- Inherited: accepted ADR-0012 and ADR-0013 permit local Plonk only.
- Inherited: B-0030 supplies the strict live service and isolated verifier worker.
- Pending: explicit maintainer approval for one synthetic Plonk rerun and public
  artifact retention.

## Open Questions And Blockers

- BLOCKER: the proof rerun and retained public artifact need explicit maintainer
  approval before B-0031 can become active.

## Work Completed

- B-0030 completed the strict live verifier service and controlled isolated worker.
- The verification-only release binaries build and run in the approved Compose
  sandbox.
- No retained public proof fixture exists yet.

## Validation Plan

- Run the approved proof once on the NUS VM profile.
- Transfer only the public artifact and public provenance evidence.
- Verify positive and mutation cases through the isolated worker with `razield`
  stopped.
- Rerun feature-tree, privacy, formatting, lint, workspace, and Compose gates.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0030 archive | PASS | Strict live verifier and isolated worker | 2026-08-25T07:22:35Z |
| Retained real proof | NOT AVAILABLE | Maintainer approval required | 2026-08-25T07:22:35Z |

## Privacy Checks

- PASS: the planned retained artifact is public zero-knowledge proof material only.
- REQUIRED: no witness, private snapshot, path, opening, salt, or caveat value enters
  the fixture or logs.
- REQUIRED: verifier success must occur with every `razield` role stopped.

## Risks And Private-Data Notes

- The public proof artifact is safe to retain only after a no-witness and no-debug
  audit.
- The provisional 2 GiB cap lacks a real verification-only peak measurement.
- The proof rerun is computationally expensive and belongs on the NUS VM.

## Exact Next Action

Obtain explicit maintainer approval for one synthetic Plonk rerun and retention of
the public proof artifact, then claim B-0031.
