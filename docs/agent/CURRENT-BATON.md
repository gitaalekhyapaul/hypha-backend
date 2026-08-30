# Current Engineering Baton

- Baton ID: B-0057
- State: READY
- Milestone: M5, live VM desktop walkthrough
- Owner: unassigned
- Started UTC: 2026-08-30T08:45:14Z
- Last updated UTC: 2026-08-30T08:45:14Z
- Branch: `main`
- Worktree: B-0056 is archived and awaits a scoped local commit. B-0051 through
  B-0055 are committed locally but are not on `origin/main`.

## Objective

Run the complete M5 desktop feature path on the NUS VM in live Plonk mode. Record one
successful proof-backed query, cache reuse, all five Denial Lab controls, and public
receipt export.

## Acceptance Criteria

- [ ] The VM pulls the reviewed B-0051 through B-0056 commits from `origin/main`.
- [ ] `./scripts/start-m5-demo` starts the live stack from Docker-only host tooling.
- [ ] Dataset, policy, access graph, and capability flows complete through one real
      Plonk proof.
- [ ] Reconciliation returns one verifier-backed result and one cache-backed result.
- [ ] Scope, replay, expired, altered, and unsigned controls return no protected rows.
- [ ] Receipts browse, filter, and export the validated public projection.
- [ ] Runtime findings and exact evidence are recorded without private witness data.

## Architecture Constraints

- The NUS VM remains a deployment target, not a source-editing worktree.
- Source changes happen locally and require maintainer approval before each push.
- The live flow uses the networkless 60 GiB Plonk sandbox and independent verifier.
- The walkthrough remains labeled synthetic and cannot satisfy M6.
- If Docker or the SSH ControlMaster becomes unavailable, stop and notify the
  maintainer with the failed prerequisite and blocked step.

## File Scope

- Read-only VM deployment and runtime logs
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0057-live-vm-desktop-walkthrough.md`
- Runtime fixes discovered by the walkthrough only after local scope reconciliation

## Out Of Scope

- Direct source edits on the VM
- M6 real transaction data
- Mobile layout polish
- New proof, policy, capability, or receipt semantics

## Decisions

- Inherited: `scripts/start-m5-demo` is the supported entry point and defaults to live
  mode.
- Inherited: the browser forwards only port 3000 from the VM.
- Inherited: each source fix is made and checked locally before an approved push.

## Open Questions And Blockers

- Maintainer approval is required to push the six local M5 commits to `origin/main`.

## Work Completed

- B-0051 through B-0056 implement the complete synthetic M5 desktop feature set.
- The local recorded stack is running at `http://127.0.0.1:3000`.
- Docker and the SSH ControlMaster are currently available.

## Validation Plan

- Push only after explicit maintainer approval.
- Pull the reviewed commits on the VM without editing source there.
- Start the live stack through `scripts/start-m5-demo`.
- Follow the 16-step walkthrough in `docs/runbooks/M5-LOCAL-PORTAL.md`.
- Record bounded public results and service health without witness data.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0056 demo-console tests, lint, typecheck, and build | PASS | Final local M5 desktop feature controls | 2026-08-30T08:45:14Z |
| Docker Compose running-service check | PASS | Local recorded M5 stack | 2026-08-30T08:42:19Z |

## Risks And Private-Data Notes

- A live proof takes about 16 minutes and peaks near 47 GiB on the accepted VM
  profile.
- VM logs must not contain private relationship tuples, caveat values, Merkle
  openings, witness traces, raw capability bytes, or authority keys.
- A recorded-mode walkthrough cannot validate protected queries or Denial Lab.

## Exact Next Action

Obtain maintainer approval to push B-0051 through B-0056 to `origin/main`, then pull
them on the NUS VM and run `./scripts/start-m5-demo`.
