# Current Engineering Baton

- Baton ID: B-0025
- State: READY
- Milestone: M4, isolated prover launcher and terminal result validation
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T18:56:52Z
- Branch: `main`
- Worktree: B-0024 implementation and documentation are complete, validated, and
  staged for a scoped commit after `dc90446`. B-0024 is archived at
  `docs/agent/history/B-0024-private-snapshot-and-one-shot-prover-boundary.md`.
  The branch was six commits ahead of `origin/main` before the B-0024 commit.

## Objective

Connect the B-0021 proof supervisor to the fixed ADR-0015 Docker sandbox launcher.
Enforce one-child execution, deadline, cancellation, cleanup, and confirmed exit.
Strictly validate the returned proof result before any terminal success or production
worker activation.

## Acceptance Criteria

- [ ] The rendered Compose proof sandbox has the exact memory, zero-swap, four-CPU,
      no-network, read-only-root, fixed read-only mount, owner-only socket, and bounded
      scratch controls from ADR-0015.
- [ ] A fixed launcher accepts one bounded request at a time and starts exactly one
      `razield-prover` child without Docker-socket or cgroup-administration access in
      the supervisor.
- [ ] The supervisor enforces the one-hour deadline, forwards cancellation, confirms
      child exit within the 30-second grace period, and removes per-job private state
      before any terminal transition.
- [ ] Crash, OOM, socket close, malformed output, timeout, cancellation, host denial,
      witness failure, proof failure, and journal mismatch map to closed typed results
      with no retry and no private output.
- [ ] Terminal success requires independent validation of the proof artifact, complete
      public journal, request commitment, correlation, and trusted program key.
- [ ] The production activation token can be constructed only with the fixed launcher
      and strict terminal validator installed.
- [ ] Focused process tests, affected service tests, rendered Compose checks, strict
      Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- ADR-0013 fixes the API-job topology, BullMQ lifecycle, one-hour deadline, 30-second
  exit grace, one attempt, zero stalled retries, and terminal cleanup order.
- ADR-0015 fixes the private Unix-socket protocol and Docker sandbox controls.
- The supervisor receives neither Docker-socket access nor delegated host cgroup
  administration.
- The launcher and prover have no network. Request data cannot select mounts, paths,
  keys, proof mode, limits, or output locations.
- Private envelopes, relationships, salts, caveat values, paths, openings, witnesses,
  and guest input never enter Redis, public results, or standard logs.
- A child result is untrusted until the terminal validator checks the proof and every
  bound public field.
- A real SP1 proof run still needs separate maintainer approval.

## File Scope

- `compose.yaml` and existing Compose validation scripts
- `services/razield/src/one_shot/`
- `services/razield/src/worker/`
- `services/razield/src/bin/`
- `services/razield/Cargo.toml` and `Cargo.lock` only for existing approved
  workspace packages and features
- Focused launcher, lifecycle, terminal-validation, and rendered-Compose tests
- Current-state documentation for behavior proven by this baton
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- `GetCapability` and `CancelCapability` RPC implementation beyond the existing
  supervisor cancellation bridge
- Independent `verifierd` service behavior
- M5, M6, M7, another scenario, or real-data acceptance
- A hosted prover, SP1 network prover, CUDA path, Core capability, or mock production
  proof
- New dependencies, package versions, toolchains, proof runs, or architecture changes
  without explicit maintainer approval

## Decisions

- Inherited: ADR-0012 fixes local CPU Plonk proof creation and key identity.
- Inherited: ADR-0013 fixes service roles, queue lifecycle, resource limits, deadline,
  cleanup order, and activation gating.
- Inherited: ADR-0015 fixes the persistent proof sandbox, owner-only Unix socket,
  strict child protocol, trusted mounts, and B-0025 test requirements.
- Inherited: B-0024 supplies the real child, strict request and result codecs, private
  snapshot store, trusted registry loading, and bounded public result.
- Made: None.

## Open Questions and Blockers

- None before code inspection.
- A real SP1 process test remains approval-gated and is not required for protocol,
  launcher, Compose, or controlled terminal-validation tests.

## Work Completed

- B-0024 completed the child-first boundary and archived its passing evidence.
- No B-0025 implementation has started.

## Validation Plan

- Inspect the current Compose services, B-0021 supervisor, B-0024 child protocol,
  capability types, proof verifier API, and activation-token gate.
- Add focused controlled launcher tests that do not invoke SP1.
- Run the affected issuer, prover, verifier, queue, and Compose checks.
- Run pinned formatting, strict Clippy, privacy, dependency, and diff gates.
- Run a real Plonk process test only after separate maintainer approval.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0024 archive | PASS | Child, protocol, private snapshot, and registry boundary | 2026-08-22T18:56:52Z |
| Not run | NOT AVAILABLE | B-0025 implementation | 2026-08-22T18:56:52Z |

## Privacy Checks

- NOT APPLICABLE: B-0025 has not started.

## Risks and Private-Data Notes

- The launcher becomes a trusted local process boundary. Socket ownership, one-child
  behavior, fixed command selection, output redaction, and cleanup need direct tests.
- A syntactically valid child success is not proof of authorization. Terminal success
  must remain closed until independent artifact and public-value validation passes.
- Compose configuration is part of the security boundary. Unit tests alone cannot
  establish the required cgroup, mount, network, root-filesystem, and scratch controls.

## Exact Next Action

Claim B-0025, then inspect `compose.yaml`, the supervisor executor handoff, activation
token, capability construction, and existing direct Plonk verifier APIs.
