# Current Engineering Baton

- Baton ID: B-0028
- State: READY
- Milestone: M4, proof resource profile reconciliation
- Owner: unassigned
- Started UTC: None
- Last updated UTC: 2026-08-24T11:33:29Z
- Branch: `main`
- Worktree: B-0027 is archived. Inspect Git before claiming B-0028.

## Objective

Adopt the validated eight-CPU SP1 Plonk profile across production Compose, proof
checks, approved decisions, current documentation, and durable benchmark evidence.

## Acceptance Criteria

- [ ] Production and benchmark proof profiles use CPUs `0-7`, `GOMAXPROCS=8`, and
      `RAYON_NUM_THREADS=8`.
- [ ] Proof controls retain the 56 GiB memory cap, zero swap, one-hour proof deadline,
      and 75-minute capability admission window.
- [ ] Host and Compose checks require eight visible proof CPUs and reject drift.
- [ ] ADR-0012 and ADR-0015 record the approved resource-profile amendment.
- [ ] The later VM benchmark is stored as durable public evidence with its source,
      timing, peak memory, verification result, and zero-OOM result.
- [ ] Current-state documentation no longer presents four CPUs as the approved
      production profile.
- [ ] Focused checks, formatting, lint, package tests, privacy scans, and diff gates
      pass without generating another SP1 proof.

## Architecture Constraints

- Proof semantics, public values, keys, artifacts, and witness handling do not change.
- The existing 56 GiB memory and memory-plus-swap caps remain equal.
- The proof process retains no network and runs with a read-only root filesystem.
- The one-hour proof deadline and 75-minute admission window remain unchanged.
- Existing operator-provided VM output is the evidence source. B-0028 does not run a
  new SP1 proof.
- No package, CLI, runtime, or system dependency is added.

## File Scope

- Production and benchmark proof services in `docker-compose.yml`
- Proof resource checks and cluster benchmark scripts
- ADR-0012 and ADR-0015 amendments
- Public SP1 benchmark evidence
- Current-state README, architecture, runbook, specification, and baton documents

## Out of Scope

- Capability lifecycle behavior completed in B-0027
- Independent `verifierd` service behavior
- Proof protocol, guest, canonical commitments, or capability format
- New proof generation, dependency changes, M5, M6, M7, or another scenario

## Decisions

- Approved by the maintainer on 2026-08-24: adopt eight CPUs as the production proof
  profile after B-0027.
- Evidence supplied by the maintainer: the eight-CPU VM run completed proving in
  929,660 ms, verification in 438 ms, and peaked at 45,592,842,240 bytes with no OOM
  or swap use.
- Inherited: the four-CPU run remains valid historical benchmark evidence.

## Open Questions and Blockers

- None. Confirm the source commit and complete metric set from the supplied VM output
  before writing the benchmark record.

## Work Completed

- B-0027 completed and archived authoritative capability lifecycle RPCs.
- The maintainer compared the later eight-CPU VM result with the checked-in four-CPU
  benchmark and approved the resource-profile amendment.
- The current repository still pins production and benchmark proving to CPUs `0-3`.

## Validation Plan

- Import the supplied VM evidence without changing its measured values.
- Reconcile every current four-CPU resource setting and assertion.
- Keep memory, swap, network, filesystem, timeout, and admission controls unchanged.
- Run focused Compose and cluster checks, formatting, lint, package tests, privacy
  scans, and `git diff --check`.
- Do not run SP1.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0027 archive | PASS | Capability lifecycle RPCs | 2026-08-24T11:33:29Z |
| Eight-CPU profile approval | PASS, accepted by maintainer | Production proof resource profile | 2026-08-24T11:33:29Z |
| Eight-CPU VM proof result | PASS, operator-provided | Plonk prove, verify, resource, and OOM metrics | 2026-08-24T11:33:29Z |
| B-0028 implementation | NOT AVAILABLE | Baton not claimed | 2026-08-24T11:33:29Z |

## Privacy Checks

- PASS: the supplied benchmark output contains public toolchain and aggregate resource
  evidence. It contains no witness, relationship, caveat, salt, or private envelope.
- NOT APPLICABLE: no proof or private input will be generated in this baton.

## Risks and Private-Data Notes

- Eight CPUs reduce elapsed time but do not change the proof's security statement.
- Higher parallelism can change peak memory for later fixtures. The fixed 56 GiB cap
  remains the fail-closed boundary.
- One QEMU VM run supports this PoC profile. It does not establish broad hardware
  scaling behavior.
- Historical four-CPU benchmark records must stay unchanged.

## Exact Next Action

Inspect Git and the supplied eight-CPU VM output, claim B-0028, and map every current
four-CPU production assertion before editing.
