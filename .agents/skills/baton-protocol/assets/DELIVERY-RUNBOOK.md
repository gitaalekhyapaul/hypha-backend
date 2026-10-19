# {{OPERATION}} Runbook

- Operator and authority: {{HUMAN_AND_AUTHORIZATION_ID}}
- Applies to release: {{EXACT_COMMITS_PACKAGE_OR_IMAGE_IDENTITIES}}
- Environment/data classification: {{ENVIRONMENT_AND_CLASS}}
- Last verified UTC and verification scope: {{TIME_AND_LOCAL_OR_LIVE_SCOPE}}
- Expected downtime and cost: {{BOUNDS}}

## What Is Ready

{{DEMOABLE_OR_DELIVERED_USER_FLOWS_AND_KNOWN_LIMITS}}

## Preconditions And Stop Conditions

- Required access/services and responsible person: {{PREREQUISITES}}
- Existing jobs/state to preserve: {{STATE}}
- Backup/rollback baseline and migration limits: {{PLAN}}
- Approved operation/retry budget: {{LIMIT}}
- Stop on: {{SPECIFIC_UNEXPECTED_STATE_OR_FAILURE}}

Do not restart an active costly job, delete volumes, or patch server source to work
around a failed command without authority. Obtain safe logs and preserve state.

## Secrets Configuration

| Host And Repo | Config File/Secret Service | Variable Name | Consumer | Restart Needed |
| --- | --- | --- | --- | --- |
| {{LOCATION}} | {{PATH}} | {{NAME_NOT_VALUE}} | {{SERVICE}} | {{RULE}} |

Check path spelling, effective configuration, and readability without printing
values. State whether defaults are safe, required, or demo-only. Keep secrets out of Git.

## Release Compatibility

| Component/Repo | Branch/Ref | Exact Commit/Published Version | Upgrade Order | Previous Version |
| --- | --- | --- | --- | --- |
| {{COMPONENT}} | {{REF}} | {{IDENTITY}} | {{ORDER}} | {{BASELINE}} |

## Commands By Host And Repository

### {{HOST_1}}: {{REPO_1}}

Working directory: `{{VERIFIED_DIRECTORY}}`

```sh
{{VERIFIED_COMMANDS_FOR_THIS_HOST_AND_REPO}}
```

Expected result: {{OBSERVABLE_RESULT_NOT_AN_UNVERIFIED_CLAIM}}

### {{HOST_2}}: {{REPO_2_OR_REMOVE_SECTION}}

Working directory: `{{VERIFIED_DIRECTORY}}`

```sh
{{VERIFIED_COMMANDS_FOR_THIS_HOST_AND_REPO}}
```

Expected result: {{OBSERVABLE_RESULT}}

## Access And Ports

| Browser/Caller | URL/Port | Backend Target | Tunnel/Ingress Owner | Auth |
| --- | --- | --- | --- | --- |
| {{CALLER}} | {{ADDRESS_NO_CREDENTIALS}} | {{TARGET}} | {{OWNER}} | {{METHOD}} |

Do not assume localhost inside a container is the host or another container. State
which host runs each tunnel. Health/liveness, readiness, and the user-flow gate are
separate checks. Never weaken authentication or trust boundaries to get a green check.

## Verification And Demo Sequence

1. {{NARROW_HEALTH_OR_READINESS_CHECK}}
2. {{ACTUAL_USER_ACTION_PANEL_AND_EXPECTED_RESULT}}
3. {{RELEVANT_NEGATIVE_CASE_AND_EXPECTED_RESULT}}
4. {{REUSE_OR_FOLLOW_UP_FLOW_AND_EVIDENCE}}

Label synthetic data, replay, and mocks. Link screenshots to the release and state
shown. Do not call a flow verified until it runs in the stated environment.

## Failure And Rollback

- Safe logs/check and error class: {{COMMAND_OR_ARTIFACT_NO_PRIVATE_PAYLOAD}}
- Preserve before retry: {{STATE}}
- Allowed retry and stop condition: {{BUDGET}}
- Rollback operator, exact action, and non-reversible effects: {{PLAN}}
- Escalation and first action after resolution: {{OWNER_AND_ACTION}}

## Result

| Check | Result | Deployed Identity/Environment | Evidence | UTC |
| --- | --- | --- | --- | --- |
| {{CHECK}} | SKIP, not executed | {{SCOPE}} | None | {{UTC}} |

Record deployment and live acceptance separately. State remaining actions and the
agreed pause/continue boundary.
