# Current Engineering Baton

- Baton ID: B-0080
- State: READY
- Milestone: Record the branded reconciliation demonstration
- Owner: unassigned
- Started UTC: not started
- Last updated UTC: 2026-11-04T12:45:40Z
- Branch: `main`
- Prior checkpoint: [B-0079 console release](history/B-0079-console-brand-release.md).
- Rollback: Existing `v1` tags were not moved.

## Objective

Record a three-minute walkthrough on the released Chamber Lantern consoles.
The Beamer presentation reserves the final slide for the new recording.

## Acceptance Criteria

- [ ] Confirm the three forwarded dashboards open for the recording.
- [ ] Capture the operator's scoped approval, both agents' protected reads,
  and the returned settlement evidence using labelled synthetic data.
- [ ] Check the recording for secrets and identifying browser details.
- [ ] Replace the Beamer placeholder only after the new recording is supplied.

## Architecture Constraints

Keep all proof, privacy, registry, signature, and SDK invariants unchanged.
The release health checks do not establish a fresh proof or protected query.

## File Scope

The deployed UI is complete. The editable deck and recording checklist are
under `raziel-final-report/presentation/`. No application edit is claimed.

## Out Of Scope

New proof submissions, resets, service restarts, SDK publication, and work
on deferred B-0077 diagnostics without a separate request.

## Decisions Inherited

- Chamber Lantern is the selected brand. The operator and both agents serve
  the released assets.
- The evaluator format is 17 minutes of slides, three minutes of recording,
  and five minutes of separate Q&A.
- The user will re-record the demo. Do not embed the historical footage.

## Open Questions And Blockers

The new recording has not been made. No UI implementation task remains in
the approved branding scope.

## Work Completed

Published and deployed the operator and agent branding. Published the Beamer
source to the final-report repository. The original report and SDK are unchanged.

## Validation Plan

Use the existing SSH forwards and check the branded dashboards before capture.
Review the new recording before adding it to the presentation.

## Validation Evidence

See the B-0079 release receipt for Git, build, HTTP, asset, and service checks.

## Risks And Private-Data Notes

The operator recovered from a prior JavaScript heap-limit exit through normal
recreation. The deployment did not change its runtime limits. Keep private
data, secrets, and witness material out of the recording.

## Exact Next Action

Open the three refreshed dashboards through the user's existing port forwards
and follow the Beamer recording checklist. Wait for the user's new recording.
