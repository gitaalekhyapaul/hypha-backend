# Current Engineering Baton

- Baton ID: B-0024
- State: READY
- Milestone: M4, one-shot local Plonk executor boundary
- Owner: unassigned
- Started UTC: Not started
- Last updated UTC: 2026-08-22T09:33:01Z
- Branch: `main`
- Worktree: B-0023 is archived at
  `docs/agent/history/B-0023-trusted-authorization-input-assembly.md`. Its scoped
  implementation and the B-0024 reset await one commit after `e7f4d84`.

## Objective

Implement the bounded one-shot prover boundary that obtains trusted local registry
and private snapshot state, calls B-0023 input assembly, generates one local Plonk
proof, compares its complete public journal, and returns a bounded public result to
the proof supervisor. Keep the production supervisor disabled until terminal result
validation exists.

## Acceptance Criteria

- [ ] Fixed owner configuration selects the registry generation, private snapshot
      source, guest ELF, Plonk setup, proof mode, and resource profile.
- [ ] Request data cannot select or override trusted files, proof settings, network
      access, resource limits, or output paths.
- [ ] The one-shot process protocol is versioned, bounded, strict, and redacts every
      private input from debug and error output.
- [ ] The executor uses B-0023 assembly and the approved local Plonk prover. It rejects
      host denial, proof failure, journal mismatch, malformed output, and cancellation.
- [ ] The child has no hosted proving or SP1 network path and cannot outlive the
      supervisor deadline or cancellation grace.
- [ ] Only the bounded proof artifact, approved public values, and safe aggregate
      metrics return to the supervisor.
- [ ] Focused process and failure tests, affected cross-component tests, strict
      Clippy, formatting, privacy scans, and diff checks pass.

## Architecture Constraints

- ADR-0012 governs local Plonk proving, proof artifacts, key identity, and host
  comparison.
- ADR-0013 governs separate supervisor and one-shot prover processes, the 60 GiB
  no-swap cgroup, four visible CPUs, one-hour deadline, no network, read-only root,
  and bounded private scratch.
- B-0023 remains the only service boundary that assembles trusted guest input.
- Relationship tuples, salts, caveat values, paths, openings, and guest input never
  enter Redis, standard logs, public results, or hosted services.
- Production supervisor activation and terminal capability issuance require a later
  strict result validator.

## File Scope

- To be fixed after reading the existing worker, prover, registry-loader, snapshot,
  process-isolation, and service configuration contracts.
- `docs/agent/CURRENT-BATON.md`

## Out of Scope

- Production supervisor activation or terminal queue publication
- Capability signing, `GetCapability`, or `CancelCapability`
- Independent verifier service behavior
- ADR-0002, O-007, M5, M6, M7, or another scenario
- A hosted prover, SP1 network prover, CUDA path, Core capability, or mock production
  proof
- New dependencies, package versions, storage formats, or toolchains without explicit
  maintainer approval

## Decisions

- Inherited: ADR-0012 and ADR-0013 fix the local Plonk profile and process topology.
- Inherited: B-0021 owns BullMQ claim, deadline, cancellation, and envelope cleanup.
- Inherited: B-0023 owns trusted input assembly and expected public journal creation.
- Made: None.

## Open Questions and Blockers

- The approved private snapshot storage representation and loader must be identified.
  B-0023 defines the in-memory selection but no persistent format.
- The existing `CapabilityExecutor` returns only success or a closed failure. B-0024
  must identify the bounded public result contract without enabling terminal issuance.
- No dependency or toolchain change is approved for this baton yet.

## Work Completed

- B-0023 implemented and validated trusted authorization input assembly.
- No B-0024 implementation has started.

## Validation Plan

- Read ADR-0012, ADR-0013, the worker executor contract, `raziel-prover`, registry
  loading, snapshot persistence, and existing cgroup scripts before fixing scope.
- Run focused process-protocol, cancellation, malformed-result, privacy, and journal
  comparison tests.
- Run affected service, prover, verifier, authorization-core, witness, and snapshot
  tests without executing SP1 unless the maintainer approves a proof run.
- Run the pinned non-guest workspace, strict Clippy, formatting, privacy, dependency,
  and diff gates.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0023 archive | PASS | Trusted authorization input assembly | 2026-08-22T09:33:01Z |
| B-0024 implementation | NOT STARTED | No feature files edited | 2026-08-22T09:33:01Z |

## Privacy Checks

- NOT STARTED: Process input, scratch, output, debug, and failure boundaries need
  review before implementation.
- NOT APPLICABLE: No real transaction data or proof run is part of the reset.

## Risks and Private-Data Notes

- A child-process protocol can leak witness data through errors, crash output, scratch,
  inherited environment, or file descriptors even when the proof artifact is private.
- Request-selected paths or proof modes would break the approved trust model.
- An in-process proof can terminate the supervisor through OOM. Process and cgroup
  isolation are required before production activation.

## Exact Next Action

Read the approved one-shot process, cgroup, registry, private snapshot, and
`raziel-prover` contracts. Then claim B-0024 with the smallest complete production
file map and record any approval needed before editing.
