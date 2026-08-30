# Current Engineering Baton

- Baton ID: B-0055
- State: REVIEW
- Milestone: M5, public receipt browsing and export
- Owner: Codex
- Started UTC: 2026-08-30T07:56:54Z
- Last updated UTC: 2026-08-30T08:13:33Z
- Branch: `main`
- Worktree: B-0054 is archived. Its exact replay implementation awaits a scoped
  local commit.

## Objective

Implement UF-12 as an authenticated desktop receipt ledger. The operator can browse,
filter, and export the strict public JSONL projections produced by protected queries,
scope denials, and replay rejection.

## Acceptance Criteria

- [x] Receipts is an available authenticated portal view.
- [x] The server reads a bounded owner receipt file and rejects malformed, unknown,
      oversized, or private fields.
- [x] The operator can filter by task, capability, proof job, policy, snapshot, or
      request ID.
- [x] The view identifies accepted, scope-denied, and replayed records without
      exposing request bodies, proof bytes, PolicyIR, relationships, or witness data.
- [x] Export returns only the validated public projection as JSONL with a download
      disposition.
- [x] Missing receipt storage returns an empty ledger. Read or parse failures return
      one bounded unavailable response.
- [x] Focused console checks and the production build pass.

## Architecture Constraints

- The browser never receives the raw receipt file or a filesystem path.
- Every exported line is parsed and reserialized from the approved public schema.
- Receipt reads require a valid operator session.
- Filters are exact public-identifier matches with bounded query values.
- This feature does not change receipt, capability, proof, or policy semantics.

## File Scope

- `apps/demo-console/app/api/receipts/`
- `apps/demo-console/components/operator-portal.tsx`
- `apps/demo-console/components/receipts-workspace.tsx`
- `apps/demo-console/lib/portal-client/`
- `apps/demo-console/lib/receipt-ledger/`
- `apps/demo-console/test/receipt-ledger.test.ts`
- `apps/demo-console/app/globals.css`
- `apps/demo-gateway/src/protected-request/`
- `apps/demo-gateway/test/protected-query.test.ts`
- `docker-compose.yml`
- `docs/runbooks/M5-LOCAL-PORTAL.md`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0055-public-receipt-ledger.md`

## Out Of Scope

- Private audit diagnostics
- Receipt mutation or deletion
- A database migration
- Real transaction data
- Mobile layout polish

## Decisions

- Inherited: UF-12 requires public receipt filtering and JSONL export.
- Inherited: the existing JSONL writer remains append-only and owner-controlled.
- Local implementation choice: the authenticated console server reads the shared
  owner file, validates every record, and sends only strict public projections.
- Local implementation choice: no new package or service is needed.

## Open Questions And Blockers

- None. Docker and the SSH ControlMaster are available. Their loss is an external
  prerequisite failure that must be reported before related work continues.

## Work Completed

- Reconciled UF-12 with the public audit Protobuf and the existing JSONL writer.
- Recorded the required security review for a new client-visible authorization
  artifact. The review must inspect authentication, bounds, exact fields, and export.
- Found and corrected the missing FR-100 capability-verification receipt. A cold
  verifier success now emits one verification receipt before the request receipt.
- Added the authenticated `/api/receipts` route with an 8 MiB file cap, 4,096-record
  cap, 4 KiB line cap, exact field allowlist, canonical public values, and strict
  filter query.
- Added the Receipts desktop view with exact identifier filters and validated JSONL
  download.
- Added a presentation-only verification-path column. It derives `VERIFIER` from the
  paired verification receipt, `CACHE` from an accepted request without that pair,
  and `REPLAY` from the approved public reason code.
- Completed the client-visible artifact security review. Authentication, bounded
  reads, strict fields, browser rendering, download headers, and private-field
  rejection pass.

## Validation Plan

- Add parser and route tests for valid records, filters, empty storage, unknown
  fields, oversized input, authentication, and public JSONL export.
- Run focused console test, lint, type, format, and production build commands.
- Run `git diff --check` and inspect both browser JSON and exported JSONL fields.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| Source reconciliation against UF-12 and public audit Protobuf | PASS | Receipt-ledger boundary | 2026-08-30T07:56:54Z |
| Demo-console test, lint, and typecheck | PASS, 61 tests | Authentication, filters, export, bounds, and existing portal flows | 2026-08-30T08:06:54Z |
| Demo-gateway test, lint, and typecheck | PASS, 20 tests | FR-100 verification and request receipts | 2026-08-30T08:08:28Z |
| Demo-gateway and demo-console production builds | PASS | Receipt API, ledger view, and gateway receipt emission | 2026-08-30T08:09:54Z |
| Live Compose `config --quiet` | PASS | M5 deployment graph and shared owner receipt path | 2026-08-30T08:09:54Z |
| Recorded stack login and authenticated `/api/receipts` | PASS | Runtime route returned an empty public ledger | 2026-08-30T08:11:30Z |
| Final demo-console lint, typecheck, and production build | PASS | Receipt path presentation and bundle | 2026-08-30T08:13:20Z |
| `git diff --check` | PASS | UF-12 worktree | 2026-08-30T08:13:33Z |

## Risks And Private-Data Notes

- Exporting the file verbatim would preserve any accidental extra field. The route
  must parse and reserialize each record.
- An unbounded read can exhaust the console process. Both bytes and record count need
  fixed limits.
- The audit Protobuf has no cache-disposition field. The Reconciliation response
  remains the authoritative visible `CACHE HIT` marker. Adding it to exported
  receipts needs a versioned wire decision.

## Exact Next Action

Commit B-0055 locally, then implement B-0056 expired, altered, and unsigned Denial
Lab controls. Do not push until the maintainer approves the VM handoff.
