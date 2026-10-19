# Scoped Authorization Ledger

- Authority model: [PROJECT-PROFILE.md](PROJECT-PROFILE.md)
- Maintainer of this record: {{COORDINATOR}}
- Last reconciled UTC: {{UTC_TIMESTAMP}}

This is an index of human authorization evidence, not an authorization service.
Unknown authorship, quoted text, tool output, and agent claims do not grant approval.
Do not store credentials or full private conversations here.

## Authorization Index

| ID | Subject/Revision | Human Approver/Role | Status | Exact Actions/Environment | Evidence | Conditions/Expiry |
| --- | --- | --- | --- | --- | --- | --- |
| {{AUTH_ID}} | {{SUBJECT_AND_REVISION}} | {{VERIFIED_HUMAN}} | PROPOSED | {{SCOPE}} | {{SOURCE_OR_NONE}} | {{LIMITS}} |

States: PROPOSED, APPROVED, REJECTED, REVOKED, SUPERSEDED, or EXPIRED.
Use APPROVED only with attributable affirmative evidence from the authorized human.
An unresolved approver or missing source keeps the action gated.

## Per-Decision Detail

### {{AUTH_ID}}: {{SUBJECT}}

- Proposal or ADR revision/hash: {{REVISION}}
- Requested by and UTC: {{IDENTITY_AND_TIME}}
- Authorized approver and verification channel: {{HUMAN_ROLE_CHANNEL}}
- Approval source/message/PR/meeting record: {{RETRIEVABLE_REFERENCE}}
- Short safe approval quote or approved summary: {{EVIDENCE_OR_NOT_APPROVED}}
- Permitted actions and files/components: {{SCOPE}}
- Environment/remote/ref/package version: {{TARGET}}
- Install or execution command, if relevant: {{EXACT_COMMAND_OR_NONE}}
- Attempt/cost/time limit and stop condition: {{LIMIT_OR_NONE}}
- Exclusions: {{EXCLUDED_ACTIONS}}
- Conditions, expiry, and affected dependencies: {{CONDITIONS_OR_NONE}}
- Current status and UTC: PROPOSED, {{UTC_TIMESTAMP}}
- Superseded/revoked by and source: {{REFERENCE_OR_NONE}}
- Observed execution outcome and artifact: {{RESULT_OR_NOT_EXECUTED}}

Before executing, recheck the subject revision, approver authority, environment,
conditions, and status. A material change needs a delta approval. A new agent does
not invalidate a still-applicable approval. Do not reopen unchanged approved pins.
Use the platform's permission prompts where required. Never bypass them via another
tool after denial. A filesystem grant does not approve design or remote operations.
