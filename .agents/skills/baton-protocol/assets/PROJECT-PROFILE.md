# Project Operating Profile

- Project: {{PROJECT_NAME}}
- Adopted by: {{AUTHORIZED_HUMAN_AND_EVIDENCE}}
- Profile revision: {{REVISION}}
- Last verified UTC: {{UTC_TIMESTAMP}}
- Program hub: {{REPOSITORY_OR_WORKSPACE}}
- Canonical protocol: [BATON-PROTOCOL.md](BATON-PROTOCOL.md)

This file configures the team's working agreement. It is not access control and
does not override platform permissions, legal obligations, or protected contracts.
Remove unused starter rows and resolve fields before adoption. Unknown authority
stays unresolved and blocks only its gated action.

## Outcome And Priorities

- Current product/research outcome: {{OUTCOME}}
- Next observable user flow: {{USER_FLOW}}
- Current milestone and acceptance: {{MILESTONE_AND_GATE}}
- Explicit non-goals/deferred work: {{EXCLUSIONS}}
- Primary audience and target environment: {{AUDIENCE_AND_ENVIRONMENT}}

## Sources Of Truth

Read repository instructions, this profile, current baton and protocol, relevant
architecture/accepted decisions, then task files and tests. Inspect Git first.

Required project paths: {{VERIFIED_PATHS}}
Contract precedence: {{PROJECT_SPECIFIC_ORDER}}
Resolve contradictions through the named owner. A test expectation or UI label
does not silently supersede an approved contract. Historical results stay dated.

## People And Authority

| Role | Human Or Group | Authority Scope | Verification Channel |
| --- | --- | --- | --- |
| Sponsor | {{SPONSOR}} | Goals and priority | {{CHANNEL}} |
| Design authority | {{DESIGN_AUTHORITY}} | Durable architecture decisions | {{CHANNEL}} |
| Security/data authority | {{SECURITY_AUTHORITY}} | Trust, privacy, data use | {{CHANNEL}} |
| Dependency authority | {{DEPENDENCY_AUTHORITY}} | Installs, pins, downloads | {{CHANNEL}} |
| Release operator | {{RELEASE_OPERATOR}} | Commit/push/publish/deploy as granted | {{CHANNEL}} |
| Coordinator | {{COORDINATOR_AND_SPONSOR}} | Work allocation and shared state | {{CHANNEL}} |

One human can hold several roles. Agent labels do not verify authority. Record
project-wide standing permissions in the authorization ledger with their limits.

## Gita's Working Defaults

Adopt or revise these preferences explicitly for the current team:

- Read canonical docs and the current baton before work. Preserve unknown edits.
- Deliver feature-complete vertical slices. Keep tests proportional to actual risk.
- Explain substantial proposals and edits with a focused diff. Preserve the human's
  wording, structure, examples, filenames, and supplied deliverable format.
- Use ADRs for durable architecture or trust changes. Make routine choices locally.
- Ask before installing dependencies or toolchains. Give the exact pin, command,
  reason, work unlocked, and fallback. Reuse existing approvals within their scope.
- Keep brief factual updates. Explain repeated commands, uncertain diagnoses,
  feature progress, blockers, and the next practical action.
- Give ordered remaining work and rough turn ranges on request. Separate wall time.
- Use small diagrams to explain unfamiliar system interactions on request.
- Split substantial independent work among agents. Keep tiny coupled edits local.
- Pause at the requested checkpoint. Do not begin the next item after a commit/push
  boundary. A status question does not resume paused execution.
- Report each repo's dirty files and release state. Preserve unowned changes rather
  than cleaning them away. Do not imply deployment from a push or live success from mocks.
- Notify the human if required Docker, SSH, credentials, or tunnel access is absent.
  Do not repeatedly retry authentication or change remote source without authority.
- Use direct plain language. Avoid filler, artificial praise, and unsupported claims.

Current task exceptions: {{EXPLICIT_TASK_SPECIFIC_OVERRIDES_OR_NONE}}
Do not import Raziel-specific ports, proof limits, tag names, or acceptance gates.

## Permission Defaults

| Action | Default | Project Override/Evidence |
| --- | --- | --- |
| Read permitted files, inspect Git | Within task scope | {{OVERRIDE_OR_NONE}} |
| Local in-scope implementation/checks | Authorized implementation objective | {{OVERRIDE_OR_NONE}} |
| New dependency, image download, toolchain | Ask first | {{OVERRIDE_OR_NONE}} |
| Architecture/security/public contract change | Reviewed decision and authorized approval | {{OVERRIDE_OR_NONE}} |
| Commit | Explicit request or standing repo permission | {{OVERRIDE_OR_NONE}} |
| Push | Separately granted remote/ref scope | {{OVERRIDE_OR_NONE}} |
| Public package/image publication | Explicit release scope | {{OVERRIDE_OR_NONE}} |
| Deployment/restart/migration/live job | Explicit environment and operation scope | {{OVERRIDE_OR_NONE}} |
| Destructive cleanup/history rewrite/state deletion | Exact action approval, never inferred | {{OVERRIDE_OR_NONE}} |
| Costly/state-changing retry | Defined attempt/cost/time budget | {{OVERRIDE_OR_NONE}} |

## Collaboration And Repositories

- Coordination mode: {{SINGLE_CHECKOUT_OR_ACKNOWLEDGED_MULTI_CLONE}}
- Accepted claim channel and allocator: {{CHANNEL_AND_OWNER}}
- ID format and allocation rule: {{ID_RULE}}
- Work branches/worktrees: {{EXISTING_CONVENTION}}
- Shared-file integrator: {{OWNER}}
- Ownership transfer/expiry rule: {{RULE_NO_SILENT_TAKEOVER}}
- Parallel assignments and review requirements: {{RULE}}

| Repo/Component | Role | Canonical Location | Allowed Write Scope | Release Dependency |
| --- | --- | --- | --- | --- |
| {{REPO}} | {{ROLE}} | {{LOCATION}} | {{SCOPE}} | {{DEPENDENCY_OR_NONE}} |

## Validation And Environments

| Scope | Existing Command Or Check | Environment | Owner | Cost/Stop Limit |
| --- | --- | --- | --- | --- |
| {{SCOPE}} | {{VERIFIED_COMMAND}} | {{ENVIRONMENT}} | {{OWNER}} | {{LIMIT}} |

- Mock/fixture labels and limits: {{RULE}}
- Live acceptance gate: {{OBSERVABLE_GATE_AND_EVIDENCE}}
- Secret storage: {{APPROVED_PATH_OR_SERVICE_NO_VALUES}}
- Approved public/private artifact classes: {{CLASSIFICATION}}
- Redaction and evidence retention: {{RULE}}

## Checkpoint And Release Policy

- Required reviewer for this change class: {{REVIEWER_OR_NONE}}
- Review timing and accepted channel: {{RULE}}
- Commit/push authority and target refs: {{AUTHORIZATION_OR_NONE}}
- Package/image publication authority: {{AUTHORIZATION_OR_NONE}}
- Deployment host/repo/operator and health/user-flow checks: {{RULE}}
- Rollback baseline/tag policy: {{OPT_IN_RULE_AND_STATE_LIMITS}}
- Stop boundary after delivery: {{PAUSE_OR_NEXT_AUTHORIZED_ACTION}}
- Current-state docs to reconcile on milestone close: {{VERIFIED_PATHS}}
