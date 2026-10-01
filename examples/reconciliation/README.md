# Synthetic Reconciliation Ledgers

This seed extends the existing `payment-reconciliation` scenario. It uses no real
transactions. It changes no proof, capability, policy, or snapshot format.

The operator action **Seed reconciliation ledgers** sends an authenticated,
CSRF-protected `POST /api/datasets/seed-reconciliation`. The console uses
`SyntheticDatasetClient.createDraft` and the gateway's create-only
`POST /api/datasets` to create these datasets:

| Dataset | Source classification | Generator/source ID | Rows | Initial status |
| --- | --- | --- | --- | --- |
| `processor-demo-v1` | `SYNTHETIC` | `reconciliation-ledgers-v1` | 3 | `DRAFT` |
| `bank-demo-v1` | `SYNTHETIC` | `reconciliation-ledgers-v1` | 3 | `DRAFT` |

Source labels name the synthetic ledger and deterministic generator version. The
generator uses fixed identifiers, amounts, references, and UTC timestamps on
2026-10-01. There is no random input or clock-based transaction generation.

## Reference Mapping

`merchantReference` carries the shared reconciliation reference, not a merchant
identity. Ledger-local `transactionId` identifies a processor payment or bank
settlement. Matching candidates share a reference, currency, and amount.

| Shared `merchantReference` | Processor transaction | Bank settlement candidates | Case |
| --- | --- | --- | --- |
| `recon-exact-001` | `processor-001`, 12500 | `bank-001`, 12500 | Exact match |
| `recon-missing-002` | `processor-002`, 4899 | None | Missing settlement |
| `recon-ambiguous-003` | `processor-003`, 9900 | `bank-002` and `bank-003`, 9900 each | Ambiguous settlement |

Every amount is a canonical decimal string of integer minor units. The currency is
`SGD`. For example, `12500` represents S$125.00. Processor rows use `payment` and
bank rows use `settlement` in `transactionType`. All rows start `UNMATCHED`.
`accountReference` is the synthetic value `synthetic-account-001`.

The seed contains six rows, below the existing 500-row limit per dataset. Agents
must derive outcomes from stored query results. The gateway returns no preset
reconciliation answer.

## Draft Lifecycle And Retries

Seeding does not finalize records or publish access policy. New ledgers appear in
the dataset workspace. The operator can select, edit, save, and finalize each
ledger through the existing controls. Finalization records the gateway-generated
checksum, schema fingerprint, row count, and timestamp. Row digests come from the
existing store, not the seed helper.

A repeat call validates stored source metadata and every seed row field. Exact
existing drafts or finalized ledgers remain unchanged. Row order and lifecycle
timestamps do not change the comparison. A modified ledger returns HTTP 409 with
`DATASET_CONFLICT` and its dataset ID. The helper uses `get` and `createDraft`, never
the operator's `putDraft`. A conflict found during pair validation creates no
missing partner.

The two creations are not one store transaction. A gateway failure after the first
creation can leave one draft. A retry validates that draft and creates the missing
partner. It never rolls back or deletes saved data.

## Verified Create-Only Gate

Concurrent seed calls serialize within one console process. The helper rechecks a
missing ID immediately before creation. The gateway checks existence and creates
the draft inside the store's existing exclusive mutation. A new ID returns HTTP
201. An existing ID returns HTTP 409 without a write, for drafts and finalized
records. Existing operator PUT edits retain their prior behavior.

On a create conflict, the seed reads and validates the stored record. A concurrent
exact seed is preserved and reported in `preservedDatasetIds`. An operator change
returns `DATASET_CONFLICT` without replacement. Partial retries and lost create
responses preserve committed drafts and create the missing partner.

This local gate passed on 2026-10-01 with the existing pinned Linux arm64 Docker
toolchain. The seed tests use mock HTTP transport backed by the real filesystem
store. Gateway tests invoke the real POST and PUT handlers against temporary stores.

All container checks used this command prefix:

```sh
docker compose --env-file tools/toolchain-versions.env run --rm --no-deps --pull never toolchain
```

| Command after the prefix | Result |
| --- | --- |
| `pnpm --filter @raziel/demo-console test test/reconciliation-seed.test.ts test/dataset-route.test.ts` | 43 passed: 40 seed and 3 existing console dataset-route tests |
| `pnpm --filter @raziel/demo-gateway test test/reconciliation-store.test.ts test/dataset-binding.test.ts` | 15 passed: 11 store/route and 4 dataset-binding tests |
| `pnpm --filter @raziel/demo-console exec tsc --noEmit --incremental false` | Passed |
| `pnpm --filter @raziel/demo-gateway exec tsc --noEmit --incremental false` | Passed |

The gate retains the deployed singleton owner-store assumption. It does not cover
multiple store instances or processes writing the same file. No live HTTP deployment
or proof-backed demonstration ran for this gate.

## Authorization Boundary

Dataset creation grants no agent access. Each agent still needs operator approval,
its own key-bound capability, and protected signed queries against finalized
revisions. This seed alone establishes neither live proof-backed demo completion
nor real-data acceptance.
