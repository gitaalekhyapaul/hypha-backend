# B-0075 Diagnostic Objective Deferred

- Recorded UTC: 2026-10-19T15:25:26Z
- Previous state: READY, unassigned, not started
- Reason: The maintainer requested a reusable, agent-neutral baton protocol skill.
- Replacement authoring objective: B-0076

The Agent B revision-2 proof diagnostic remains unstarted. No fault has been fixed
or reclassified. The existing failure report and unfinished live gate remain valid.

Preserved acceptance criteria:

- Review a closed owner-only diagnostic class/stage without raw child stderr.
- Capture the exact failure class on one maintainer-approved controlled attempt.
- Fix only the confirmed fault without changing proof semantics.
- Preserve tasks, roles, capabilities, keys, and private runtime state.
- Confirm maintainer deployment of the B-0072 ordering checkpoint.
- Resume the unfinished M7 role and two-agent recording gate.

Inherited constraints: ADR-0019, ADR-0023, approved guest/key, generic agent errors,
and no private witness logging. No VM proof retry or active-job restart without
agreement. The older duplicate-task/revision recovery bug remains separate.

Evidence remains in [the revision-2 report](../../reports/2026-10-18-agent-b-revision-2-failure.md),
[B-0072](B-0072-recent-task-and-request-ordering.md), and
[B-0074](B-0074-policy-table-identity-and-order.md). None establishes a completed M7 gate.

Resume action: prepare safe diagnostic capture, not another blind task submission.
