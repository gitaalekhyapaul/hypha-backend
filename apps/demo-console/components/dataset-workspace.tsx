"use client";

import { useCallback, useEffect, useId, useMemo, useState } from "react";

import type {
  ReconciliationStatus,
  SyntheticDataset,
  SyntheticDatasetDraft,
  SyntheticTransactionDraft,
} from "../lib/dataset-client/index.ts";
import {
  cloneDataset,
  finalizeDataset,
  listDatasets,
  PortalRequestError,
  saveDataset,
} from "../lib/portal-client/index.ts";

interface DatasetWorkspaceProps {
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
}

type MutationState = "clone" | "finalize" | "save" | null;

export function DatasetWorkspace({ csrfToken, onSessionExpired }: DatasetWorkspaceProps) {
  const [datasets, setDatasets] = useState<readonly SyntheticDataset[]>([]);
  const [draft, setDraft] = useState<SyntheticDatasetDraft>(() => sampleDraft());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [cloneId, setCloneId] = useState("");
  const [loading, setLoading] = useState(true);
  const [mutation, setMutation] = useState<MutationState>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const selected = useMemo(
    () => datasets.find((dataset) => dataset.datasetId === selectedId),
    [datasets, selectedId],
  );
  const immutable = selected?.status === "FINALIZED";

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await listDatasets();
      setDatasets(result.datasets);
      const first = result.datasets[0];
      if (first) {
        selectDataset(first, setSelectedId, setDraft, setCloneId);
      } else {
        setSelectedId(null);
      }
    } catch (cause) {
      handleFailure(cause, onSessionExpired, setError);
    } finally {
      setLoading(false);
    }
  }, [onSessionExpired]);

  useEffect(() => {
    void load();
  }, [load]);

  function beginNewDraft() {
    setSelectedId(null);
    setDraft(emptyDraft());
    setCloneId("");
    setError(null);
    setNotice(null);
  }

  function choose(dataset: SyntheticDataset) {
    selectDataset(dataset, setSelectedId, setDraft, setCloneId);
    setError(null);
    setNotice(null);
  }

  async function persistDraft() {
    await runMutation("save", async () => {
      const saved = await saveDataset(draft, csrfToken);
      setDatasets((current) => upsertDataset(current, saved));
      selectDataset(saved, setSelectedId, setDraft, setCloneId);
      setNotice("Draft saved");
    });
  }

  async function finalizeCurrent() {
    if (!selectedId) {
      return;
    }
    await runMutation("finalize", async () => {
      const finalized = await finalizeDataset(selectedId, csrfToken);
      setDatasets((current) => upsertDataset(current, finalized));
      selectDataset(finalized, setSelectedId, setDraft, setCloneId);
      setNotice("Dataset finalized");
    });
  }

  async function cloneCurrent() {
    if (!selectedId) {
      return;
    }
    await runMutation("clone", async () => {
      const cloned = await cloneDataset(selectedId, cloneId, csrfToken);
      setDatasets((current) => upsertDataset(current, cloned));
      selectDataset(cloned, setSelectedId, setDraft, setCloneId);
      setNotice("Revision cloned");
    });
  }

  async function runMutation(next: Exclude<MutationState, null>, action: () => Promise<void>) {
    setMutation(next);
    setError(null);
    setNotice(null);
    try {
      await action();
    } catch (cause) {
      handleFailure(cause, onSessionExpired, setError);
    } finally {
      setMutation(null);
    }
  }

  function updateDraft<K extends keyof SyntheticDatasetDraft>(
    key: K,
    value: SyntheticDatasetDraft[K],
  ) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  function updateRow(index: number, row: SyntheticTransactionDraft) {
    setDraft((current) => ({
      ...current,
      rows: current.rows.map((candidate, position) => (position === index ? row : candidate)),
    }));
  }

  function addRow() {
    setDraft((current) => ({ ...current, rows: [...current.rows, blankRow(current.rows.length)] }));
  }

  function removeRow(index: number) {
    setDraft((current) => ({
      ...current,
      rows: current.rows.filter((_, position) => position !== index),
    }));
  }

  return (
    <section className="dataset-view" aria-labelledby="dataset-heading">
      <header className="view-header">
        <div>
          <p className="eyebrow">Payment reconciliation</p>
          <h1 id="dataset-heading">Synthetic dataset</h1>
        </div>
        <div className="header-actions">
          <span className="classification-flag">Synthetic</span>
          <button className="secondary-button" type="button" onClick={beginNewDraft}>
            New draft
          </button>
        </div>
      </header>

      <div className="dataset-layout">
        <aside className="dataset-revisions" aria-label="Dataset revisions">
          <div className="section-heading-row compact-heading">
            <h2>Revisions</h2>
            <span className="section-count">{datasets.length}</span>
          </div>
          {loading ? <p className="empty-state">Loading...</p> : null}
          {!loading && datasets.length === 0 ? (
            <p className="empty-state">No saved revisions</p>
          ) : null}
          <div className="revision-list">
            {datasets.map((dataset) => (
              <button
                key={dataset.datasetId}
                type="button"
                className="revision-row"
                data-selected={dataset.datasetId === selectedId}
                onClick={() => choose(dataset)}
              >
                <span>
                  <strong>{dataset.datasetId}</strong>
                  <small>Revision {dataset.revision}</small>
                </span>
                <span className="status-text" data-status={dataset.status}>
                  {dataset.status}
                </span>
              </button>
            ))}
          </div>
        </aside>

        <div className="dataset-editor">
          <div className="dataset-meta-grid">
            <Field
              label="Dataset ID"
              value={draft.datasetId}
              disabled={selectedId !== null}
              onChange={(value) => updateDraft("datasetId", value)}
            />
            <Field
              label="Source ID"
              value={draft.sourceId}
              disabled={immutable}
              onChange={(value) => updateDraft("sourceId", value)}
            />
            <Field
              label="Source label"
              value={draft.sourceLabel}
              disabled={immutable}
              onChange={(value) => updateDraft("sourceLabel", value)}
            />
            <div className="field-group read-only-field">
              <span>Status</span>
              <strong>{selected?.status ?? "NEW"}</strong>
            </div>
          </div>

          {selected?.finalization ? <FinalizationBand dataset={selected} /> : null}

          <section className="transaction-section" aria-labelledby="transactions-heading">
            <div className="section-heading-row">
              <div>
                <p className="eyebrow">Records</p>
                <h2 id="transactions-heading">Transactions</h2>
              </div>
              <div className="table-actions">
                <span className="section-count">{draft.rows.length} rows</span>
                <button
                  className="secondary-button compact-button"
                  type="button"
                  disabled={immutable}
                  onClick={addRow}
                >
                  Add row
                </button>
              </div>
            </div>

            <div className="transaction-table-wrap">
              <table className="transaction-table">
                <thead>
                  <tr>
                    <th>Transaction</th>
                    <th>Occurred at</th>
                    <th>Amount minor</th>
                    <th>Currency</th>
                    <th>Merchant</th>
                    <th>Account</th>
                    <th>Type</th>
                    <th>Status</th>
                    <th aria-label="Row actions" />
                  </tr>
                </thead>
                <tbody>
                  {draft.rows.map((row, index) => (
                    <TransactionRow
                      key={`${row.transactionId}-${index}`}
                      row={row}
                      disabled={immutable}
                      onChange={(next) => updateRow(index, next)}
                      onRemove={() => removeRow(index)}
                    />
                  ))}
                </tbody>
              </table>
            </div>
            {draft.rows.length === 0 ? <p className="empty-state table-empty">No rows</p> : null}
          </section>

          {error ? (
            <p className="form-error editor-message" role="alert">
              {error}
            </p>
          ) : null}
          {notice ? (
            <p className="form-notice editor-message" role="status">
              {notice}
            </p>
          ) : null}

          <footer className="editor-footer">
            {immutable && selected ? (
              <div className="clone-controls">
                <label htmlFor="clone-dataset-id">Next revision ID</label>
                <input
                  id="clone-dataset-id"
                  value={cloneId}
                  onChange={(event) => setCloneId(event.target.value)}
                />
                <button
                  className="primary-button"
                  type="button"
                  disabled={mutation !== null || cloneId.length === 0}
                  onClick={() => void cloneCurrent()}
                >
                  {mutation === "clone" ? "Cloning..." : "Clone revision"}
                </button>
              </div>
            ) : (
              <div className="draft-controls">
                <button
                  className="secondary-button"
                  type="button"
                  disabled={mutation !== null || selectedId === null || draft.rows.length === 0}
                  onClick={() => void finalizeCurrent()}
                >
                  {mutation === "finalize" ? "Finalizing..." : "Finalize"}
                </button>
                <button
                  className="primary-button"
                  type="button"
                  disabled={mutation !== null}
                  onClick={() => void persistDraft()}
                >
                  {mutation === "save" ? "Saving..." : "Save draft"}
                </button>
              </div>
            )}
          </footer>
        </div>
      </div>
    </section>
  );
}

function Field({
  label,
  value,
  disabled,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly disabled?: boolean;
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="field-group">
      <span>{label}</span>
      <input value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

function TransactionRow({
  row,
  disabled,
  onChange,
  onRemove,
}: {
  readonly row: SyntheticTransactionDraft;
  readonly disabled: boolean;
  readonly onChange: (row: SyntheticTransactionDraft) => void;
  readonly onRemove: () => void;
}) {
  function field<K extends keyof SyntheticTransactionDraft>(
    key: K,
    value: SyntheticTransactionDraft[K],
  ) {
    onChange({ ...row, [key]: value });
  }

  return (
    <tr>
      <CellInput
        label="Transaction ID"
        value={row.transactionId}
        disabled={disabled}
        onChange={(value) => field("transactionId", value)}
      />
      <CellInput
        label="Occurred at"
        value={row.occurredAt}
        disabled={disabled}
        onChange={(value) => field("occurredAt", value)}
      />
      <CellInput
        label="Amount minor"
        value={row.amountMinor}
        disabled={disabled}
        onChange={(value) => field("amountMinor", value)}
      />
      <CellInput
        label="Currency"
        value={row.currency}
        disabled={disabled}
        onChange={(value) => field("currency", value.toUpperCase())}
      />
      <CellInput
        label="Merchant"
        value={row.merchantReference ?? ""}
        disabled={disabled}
        onChange={(value) => field("merchantReference", value)}
      />
      <CellInput
        label="Account"
        value={row.accountReference ?? ""}
        disabled={disabled}
        onChange={(value) => field("accountReference", value)}
      />
      <CellInput
        label="Type"
        value={row.transactionType ?? ""}
        disabled={disabled}
        onChange={(value) => field("transactionType", value)}
      />
      <td>
        <label className="sr-only" htmlFor={`status-${row.transactionId}`}>
          Reconciliation status
        </label>
        <select
          id={`status-${row.transactionId}`}
          value={row.reconciliationStatus}
          disabled={disabled}
          onChange={(event) =>
            field("reconciliationStatus", event.target.value as ReconciliationStatus)
          }
        >
          <option value="UNMATCHED">Unmatched</option>
          <option value="MATCHED">Matched</option>
          <option value="REVIEW">Review</option>
        </select>
      </td>
      <td className="row-action-cell">
        <button
          className="remove-row-button"
          type="button"
          disabled={disabled}
          onClick={onRemove}
          title="Remove row"
        >
          Remove
        </button>
      </td>
    </tr>
  );
}

function CellInput({
  label,
  value,
  disabled,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly disabled: boolean;
  readonly onChange: (value: string) => void;
}) {
  const inputId = useId();

  return (
    <td>
      <label className="sr-only" htmlFor={inputId}>
        {label}
      </label>
      <input
        id={inputId}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </td>
  );
}

function FinalizationBand({ dataset }: { readonly dataset: SyntheticDataset }) {
  const finalization = dataset.finalization;
  if (!finalization) {
    return null;
  }
  return (
    <dl className="finalization-band">
      <div>
        <dt>Finalized</dt>
        <dd>{formatTimestamp(finalization.finalizedAt)}</dd>
      </div>
      <div>
        <dt>Rows</dt>
        <dd>{finalization.rowCount}</dd>
      </div>
      <div>
        <dt>Checksum</dt>
        <dd title={finalization.checksum}>{shortDigest(finalization.checksum)}</dd>
      </div>
      <div>
        <dt>Schema</dt>
        <dd title={finalization.schemaFingerprint}>
          {shortDigest(finalization.schemaFingerprint)}
        </dd>
      </div>
    </dl>
  );
}

function selectDataset(
  dataset: SyntheticDataset,
  setSelectedId: (value: string) => void,
  setDraft: (value: SyntheticDatasetDraft) => void,
  setCloneId: (value: string) => void,
) {
  setSelectedId(dataset.datasetId);
  setDraft({
    datasetId: dataset.datasetId,
    sourceId: dataset.source.sourceId,
    sourceLabel: dataset.source.sourceLabel,
    rows: dataset.rows.map(({ sourceRecordDigest: _digest, ...row }) => row),
  });
  setCloneId(`${dataset.datasetId}-revision-${dataset.revision + 1}`);
}

function upsertDataset(
  datasets: readonly SyntheticDataset[],
  replacement: SyntheticDataset,
): readonly SyntheticDataset[] {
  return [
    ...datasets.filter((dataset) => dataset.datasetId !== replacement.datasetId),
    replacement,
  ].sort((left, right) => left.datasetId.localeCompare(right.datasetId));
}

function handleFailure(
  cause: unknown,
  onSessionExpired: () => void,
  setError: (value: string) => void,
) {
  if (cause instanceof PortalRequestError && cause.status === 401) {
    onSessionExpired();
    return;
  }
  if (cause instanceof PortalRequestError) {
    setError(cause.message.replaceAll("_", " "));
    return;
  }
  setError("Dataset unavailable");
}

function emptyDraft(): SyntheticDatasetDraft {
  return {
    datasetId: "reconciliation-demo",
    sourceId: "portal-local",
    sourceLabel: "Portal-created demonstration rows",
    rows: [],
  };
}

function sampleDraft(): SyntheticDatasetDraft {
  return {
    ...emptyDraft(),
    rows: [
      {
        transactionId: "txn-001",
        occurredAt: "2026-08-01T10:00:00Z",
        amountMinor: "12500",
        currency: "SGD",
        merchantReference: "merchant-a",
        accountReference: "account-1",
        transactionType: "payment",
        reconciliationStatus: "UNMATCHED",
      },
      {
        transactionId: "txn-002",
        occurredAt: "2026-08-02T11:30:00Z",
        amountMinor: "4899",
        currency: "SGD",
        merchantReference: "merchant-b",
        accountReference: "account-1",
        transactionType: "payment",
        reconciliationStatus: "MATCHED",
      },
      {
        transactionId: "txn-003",
        occurredAt: "2026-08-03T09:15:00Z",
        amountMinor: "-2500",
        currency: "SGD",
        merchantReference: "merchant-b",
        accountReference: "account-1",
        transactionType: "refund",
        reconciliationStatus: "REVIEW",
      },
    ],
  };
}

function blankRow(index: number): SyntheticTransactionDraft {
  return {
    transactionId: `txn-${String(index + 1).padStart(3, "0")}`,
    occurredAt: "2026-08-01T00:00:00Z",
    amountMinor: "0",
    currency: "SGD",
    reconciliationStatus: "UNMATCHED",
  };
}

function shortDigest(value: string): string {
  return `${value.slice(0, 10)}...${value.slice(-6)}`;
}

function formatTimestamp(value: string): string {
  return new Intl.DateTimeFormat("en-SG", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "Asia/Singapore",
  }).format(new Date(value));
}
