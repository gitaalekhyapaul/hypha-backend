# Deferred Real-Data Source Selection

- Baton ID: B-0058
- State: READY
- Milestone: M6, real-data source selection
- Owner: unassigned
- Started UTC: Not started
- Deferred UTC: 2026-08-31T12:58:08Z
- Branch: `main`
- Worktree: M5 implementation and VM validation were complete at `c5264ff`. M6 had
  not started.

## Objective

Close O-001 by selecting and approving one real transaction source, its usage basis,
and its provenance record before any acquisition or ingestion adapter work begins.

## Acceptance Criteria

- [ ] One real transaction source is named and available through either a public URL
      or an owner-controlled custody record.
- [ ] The maintainer approves the usage basis, license or custody authority, and
      permitted demo use.
- [ ] Personal-data presence and any required redaction are recorded.
- [ ] The source can populate every required field in the real-data source manifest.
- [ ] The maintainer approves the completed source choice before download or adapter
      implementation.

## Architecture Constraints

- The payment-reconciliation scenario remains the only M6 acceptance scenario.
- Synthetic fixtures remain valid for tests but cannot satisfy M6.
- Raw transaction data, credentials, and completed local manifests stay outside Git.
- The selected source must follow the manifest schema in the technical specification.
- Scenario work must follow `docs/spec/SCENARIO-AUTHORING.md` and the
  `raziel-scenario` repository skill.
- No scenario adapter can change guest, PolicyIR, snapshot, or capability semantics.

## File Scope

- `docs/spec/POC-TECHNICAL-SPEC.md`
- `docs/spec/SCENARIO-AUTHORING.md`
- `examples/reconciliation/source-manifest.example.yaml`
- `data/manifests/reconciliation/source-manifest.yaml` (Git-ignored local record)
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0058-real-data-source-selection.md`

## Out Of Scope

- Downloading data before source approval
- Building the ingestion adapter
- Running the M6 proof-backed acceptance journey
- A second scenario or M7 work
- Treating synthetic data as acceptance evidence

## Decisions

- Inherited: O-001 must close before scenario adapter implementation.
- Inherited: exactly one of `source_url` and `custody_record` must be non-null.
- Inherited: the completed source manifest and raw data stay outside Git.
- Made: None.

## Open Questions And Blockers

- O-001 is open. No real transaction source or usage basis has maintainer approval.
- The source URL or custody record, license or authority, personal-data status, and
  redaction needs are unknown.
- Acquisition and adapter implementation remain blocked until those facts are
  approved.

## Work Completed

- M0 through M5 are complete.
- B-0057 records the live synthetic M5 VM walkthrough and final regressions.
- The technical specification defines the required source-manifest fields and M6
  exit gate.
- This unstarted baton was deferred after a live M5 alternative-path capability run
  exposed a dataset-to-capability scope mismatch. B-0059 owns that regression.

## Validation Plan

- Review the selected source against the M6 manifest and scenario-authoring contract.
- Record source identity, custody or URL, usage basis, license, timestamps, size,
  row count, checksum, schema fingerprint, personal-data status, and redactions.
- Confirm raw data and the completed local manifest are ignored by Git.
- Obtain maintainer approval before opening the ingestion-adapter baton.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| B-0057 live M5 VM walkthrough and final regressions | PASS | Synthetic M5 exit gate | 2026-08-30T20:42:29Z |
| O-001 source selection | NOT AVAILABLE | No approved real transaction source | 2026-08-30T20:42:29Z |

## Risks And Private-Data Notes

- A source without a documented usage basis cannot enter the M6 workflow.
- Personal or regulated fields need a recorded handling and redaction decision.
- Raw rows, source credentials, completed local manifests, and normal runtime logs
  must not enter Git.
- Dataset relevance remains separate from Raziel authorization policy.

## Exact Next Action

Resume this objective under a new baton after B-0059 closes the live M5 regression.
