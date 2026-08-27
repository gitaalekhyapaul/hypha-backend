# Engineering Baton Archive

- Baton ID: B-0048
- State: REVIEW
- Milestone: M5, explicit policy activation
- Owner: Codex
- Started UTC: 2026-08-26T22:55:51Z
- Completed UTC: 2026-08-26T23:24:13Z
- Branch: `main`
- Worktree: Approved M5 work remains uncommitted.

## Objective

Connect one explicit authenticated operator approval action to the existing isolated
policy authority and trusted registry writer, then show only a safe active-policy
projection in the Policy workspace.

## Acceptance Criteria

- [x] A compiled candidate presents an explicit approval action separate from
      compilation.
- [x] The mutation authenticates Origin, session, and CSRF before activation work.
- [x] Caller input cannot supply PolicyIR, local paths, trust domain, authority key,
      registry generation, signing-key identity, approval nonce, or trusted time.
- [x] The policy authority reloads the private candidate and signs one staged record.
- [x] The registry writer publishes the staged record as `APPROVED` in the next
      trusted generation.
- [x] Retry returns the same active policy result without another signature or
      registry generation.
- [x] The browser receives only the approved policy projection.
- [x] Focused service checks, console checks, live desktop activation, and
      `git diff --check` pass.

## Architecture Constraints

- `razield` cannot approve its own policy.
- The console and browser cannot read the policy authority key or trusted PolicyIR
  registry.
- The policy authority cannot write the trusted registry.
- The registry writer verifies signatures but cannot sign records.
- The raw approval nonce stays inside an owner-only activation-state volume.

## Decisions

- Used a separate activation service between the console, policy authority, and
  registry writer.
- Gave the console access only to a safe activation-client socket.
- Split key generation from key inspection. The inspector sees the secret volume as
  read-only, so existing effective-write checks remain intact.
- Stored one private nonce envelope until publication. Successful publication removes
  it, and a later retry resolves the exact registry record.

## Work Completed

- Added the Rust activation server, safe CLI client, private protocol, nonce recovery,
  authority call, registry publication, and public projection.
- Added Docker volumes and services for keys, registry, activation state, and three
  separate private socket boundaries.
- Added reproducible M5 authority-key and generation-1 registry bootstrap.
- Added the authenticated activation route, portal client, `Approve and activate`
  action, approved state, and registry metadata display.
- Updated the M5 runbook.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev cargo test -p raziel-admin` | PASS, 23 tests | Authority, writer, and activation build | 2026-08-26T23:23:29Z |
| `./scripts/dev cargo clippy -p raziel-admin --all-targets -- -D warnings` | PASS | Rust administration targets | 2026-08-26T23:17:31Z |
| Console test, typecheck, lint, and build | PASS, 44 tests and 12 routes | Policy UI and route | 2026-08-26T23:23:29Z |
| Compose app profile | PASS, five M5 services healthy | Isolated runtime wiring | 2026-08-26T23:18:51Z |
| Live API compile, activate, retry | PASS, generation 2 and exact retry | Service path | 2026-08-26T23:19:57Z |
| Desktop Policy interaction | PASS, generation 3 displayed | Browser feature | 2026-08-26T23:21:35Z |
| `git diff --check` | PASS | Current M5 worktree | 2026-08-26T23:24:13Z |

## Risks And Private-Data Notes

- The authority key and trusted registry use named Docker volumes. The console does
  not mount them.
- The browser receives no PolicyIR, signature, signing-key ID, nonce, trust domain,
  registry path, or candidate file path.
- The screenshot under `output/playwright/` is ignored and contains only public demo
  state.

## Exact Next Action

Build the Access Graph screen and connect its publish action to the existing fixed
SpiceDB candidate writer and private snapshot service.
