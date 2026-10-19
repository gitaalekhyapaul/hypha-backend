# Project Work Board

- Program: {{PROJECT_NAME}}
- Coordinator: {{OWNER_AND_HUMAN_SPONSOR}}
- Accepted board revision: {{REVISION_OR_COMMIT}}
- Claim acknowledgement channel: {{CHANNEL}}
- Last reconciled UTC: {{UTC_TIMESTAMP}}

This board indexes work. The current baton and scoped work records hold the details.
Only the coordinator integrates global status. A local board edit in another clone
is a proposed update until acknowledged.

## Milestones And User Flows

| Milestone/Flow | Observable Exit Gate | Status | Evidence And Environment | Remaining Gap |
| --- | --- | --- | --- | --- |
| {{OUTCOME}} | {{GATE}} | {{STATUS}} | {{LINK_OR_NOT_RUN}} | {{GAP_OR_NONE}} |

Use NOT STARTED, IN PROGRESS, ACCEPTED, DEFERRED, or BLOCKED with the actual reason.
Locally implemented is not live accepted. Deferred work does not silently disappear.

## Ordered Work

| Priority | Work ID/Parent | Bounded Outcome | State | Owner/Sponsor | Dependencies | Record/Next Action |
| --- | --- | --- | --- | --- | --- | --- |
| {{PRIORITY}} | {{RESERVED_ID}} | {{OUTCOME}} | {{STATE}} | {{OWNER}} | {{IDS_OR_NONE}} | {{LINK_OR_ACTION}} |

## Current Claims

| Work ID | Repo/Exclusive Write Scope | Owner/Session | Base/Contract | Acknowledgement | Transfer/Expiry Rule |
| --- | --- | --- | --- | --- | --- |
| {{ID}} | {{SCOPE}} | {{OWNER}} | {{REVISION}} | {{RECEIPT}} | {{RULE}} |

Record shared-file ownership explicitly. Do not treat an old heartbeat as permission
to seize work. Resolve overlapping claims before edits.

## Decisions And Human Actions

| Decision/Resource | Needed For | Authorized Owner | Requested UTC | Evidence/Next Step |
| --- | --- | --- | --- | --- |
| {{NEED}} | {{WORK_ID}} | {{HUMAN}} | {{UTC}} | {{ACTION}} |

## Deferred And Unfinished Gates

| Original Work ID | Disposition And Reason | Unfinished Criteria | Continuation/Owner |
| --- | --- | --- | --- |
| {{ID}} | {{DEFERRED_PAUSED_OR_SUPERSEDED}} | {{CRITERIA}} | {{LINK_OR_QUEUED_OWNER}} |
