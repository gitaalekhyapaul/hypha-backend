# Current Engineering Baton

- Baton ID: B-0036
- State: REVIEW
- Milestone: M5, bounded policy draft and owner activation decision
- Owner: Codex
- Started UTC: 2026-08-26T19:55:14Z
- Last updated UTC: 2026-08-26T20:05:18Z
- Branch: `main`
- Worktree: Approved planning docs plus completed B-0034 and B-0035 features are uncommitted.

## Objective

Define the M5 visual policy draft, safe browser projection, separate policy-authority
approval, SpiceDB write, registry activation, and revision handoff boundaries.

## Acceptance Criteria

- [x] One ADR defines the typed rule subset and deterministic schema rendering.
- [x] The ADR keeps PolicyIR, compiler diagnostics, authority keys, relationships,
      caveat values, and ZedTokens out of browser responses and standard logs.
- [x] The ADR defines candidate, approval, registry, SpiceDB, and snapshot states with
      fail-closed transitions.
- [x] The ADR keeps policy signing outside Next.js and `razield`.
- [x] The ADR selects either serialized offline registry publication or a reviewed
      online mutation contract.
- [x] The ADR defines operator authentication, CSRF, request bounds, command execution,
      and one-time approval requirements.
- [x] The maintainer explicitly approves the ADR before implementation.

## Architecture Constraints

- `razield` cannot approve its own policy.
- `verifierd` resolves only signed records from its trusted local registry.
- The browser receives a bounded draft and public review summary, never PolicyIR.
- Relationship submission is write-only and cannot become a graph-read API.
- Public proof values and guest semantics do not change.

## File Scope

- `docs/decisions/0018-bounded-policy-draft-and-owner-activation.md`
- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/spec/M5-IMPLEMENTATION-PLAN.md`
- `docs/agent/CURRENT-BATON.md`

## Out Of Scope

- TypeScript, Rust, Go, Protobuf, Compose, or package changes
- Policy, SpiceDB, registry, snapshot, gateway, or console implementation
- Online policy-registry mutation implementation
- Free-form SpiceDB or CEL editing

## Decisions

- Inherited: the approved M5 plan selects typed visual rule blocks and a separate
  owner-only policy authority.
- Made: use a separate policy-authority process for exact candidate signing and extend
  the registry writer to append already-signed policy approvals.
- Made: keep relationship submission write-only, capture the pre-publication ZedToken
  in owner storage, and pass only a symbolic secret identifier to `BuildSnapshot`.

## Open Questions And Blockers

- None.

## Work Completed

- Audited the compiler, `raziel-admin`, registry writer, SpiceDB client, M4 flow, and
  runtime snapshot path.
- Confirmed that candidate hash inspection, production SpiceDB writes, runtime policy
  publication, and revision handoff are missing.
- Drafted ADR-0018 with the bounded draft, safe projection, key separation, registry
  append, SpiceDB write, revision handoff, failure rules, and required tests.
- Corrected the draft to preserve ADR-0016 public revision behavior and make the
  registry writer assign mutable registry metadata from trusted local state.
- Maintainer approved ADR-0018 on 2026-08-27 local time.

## Validation Plan

- Check ADR-0018 against architecture invariants, ADR-0010, ADR-0016, the M5 plan,
  and the current CLIs.
- Run Markdown formatting and `git diff --check`.
- Confirm that no implementation or dependency file changes in B-0036.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Owner workflow source audit | PASS | Current compiler, administration, SpiceDB, registry, snapshot, and capability paths | 2026-08-26T19:55:14Z |
| `git diff --check` | PASS | Current uncommitted M5 work | 2026-08-26T20:00:34Z |
| Maintainer review | PASS, ADR-0018 approved | Bounded policy draft and owner activation decision | 2026-08-26T20:05:18Z |

## Risks And Private-Data Notes

- A policy-drafting surface can become a policy-discovery oracle if it returns
  approved policy bodies or detailed diagnostics.
- A policy key inside Next.js would merge the operator UI and policy authority.
- Online policy mutation changes verifier-registry trust and needs focused tests.

## Exact Next Action

Open B-0037 for typed policy draft validation and deterministic rendering.
