"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

import { capabilityAuthorizesPublishedDataset } from "../lib/agent-capability/readiness.ts";
import type {
  CapabilityJobProjection,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import type { SyntheticDataset } from "../lib/dataset-client/types.ts";
import {
  listDatasets,
  PortalRequestError,
  type ProtectedReconciliationQueryResult,
  queryReconciliation,
} from "../lib/portal-client/index.ts";

interface ReconciliationWorkspaceProps {
  readonly access: PublishedAccessSelection;
  readonly csrfToken: string;
  readonly job: CapabilityJobProjection;
  readonly onSessionExpired: () => void;
}

interface QueryWindow {
  readonly from: string;
  readonly to: string;
}

const MAX_QUERY_RANGE_MILLISECONDS = 31 * 24 * 60 * 60 * 1_000;

export function ReconciliationWorkspace({
  access,
  csrfToken,
  job,
  onSessionExpired,
}: ReconciliationWorkspaceProps) {
  const [dataset, setDataset] = useState<SyntheticDataset | null>(null);
  const [window, setWindow] = useState<QueryWindow>({ from: "", to: "" });
  const [limit, setLimit] = useState("50");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ProtectedReconciliationQueryResult | null>(null);
  const ready = capabilityAuthorizesPublishedDataset(job, access);

  const loadDataset = useCallback(async () => {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const collection = await listDatasets();
      const selected = collection.datasets.find(
        (candidate) => candidate.datasetId === access.datasetId,
      );
      if (selected?.status !== "FINALIZED") {
        setDataset(null);
        setError("The capability dataset must be finalized before protected queries can run.");
        return;
      }
      setDataset(selected);
      setWindow(defaultWindow(selected));
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError("The synthetic dataset metadata is unavailable.");
    } finally {
      setLoading(false);
    }
  }, [access.datasetId, onSessionExpired]);

  useEffect(() => {
    void loadDataset();
  }, [loadDataset]);

  const queryError = useMemo(
    () => validateWindow(dataset, window, limit),
    [dataset, limit, window],
  );

  async function submit(): Promise<void> {
    if (!ready || !dataset || queryError) {
      setError(queryError ?? "The live capability is not ready for this dataset.");
      return;
    }
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(
        await queryReconciliation(
          {
            job_id: job.job_id,
            dataset_id: dataset.datasetId,
            from_occurred_at: inputToUtc(window.from),
            to_occurred_at: inputToUtc(window.to),
            limit: Number(limit),
          },
          csrfToken,
        ),
      );
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError(queryFailureMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="reconciliation-workspace" aria-labelledby="reconciliation-heading">
      <header className="view-header reconciliation-view-header">
        <div>
          <p className="eyebrow">Protected transaction access</p>
          <h1 id="reconciliation-heading">Reconciliation</h1>
          <p className="view-description">
            Sign one bounded query with the live agent key and inspect its public receipt.
          </p>
        </div>
        <div className="reconciliation-header-status">
          <span className="classification-flag">Synthetic</span>
          <div>
            <span>Capability</span>
            <strong>{ready ? "LIVE AND VERIFIED" : "LOCKED"}</strong>
          </div>
        </div>
      </header>

      <div className="reconciliation-layout">
        <aside className="reconciliation-query-panel" aria-labelledby="query-heading">
          <div className="section-heading-row compact-heading">
            <div>
              <p className="section-index">Signed request</p>
              <h2 id="query-heading">Query window</h2>
            </div>
            <span>UTC</span>
          </div>

          <div className="reconciliation-binding">
            <BoundValue label="Dataset" value={access.datasetId} />
            <BoundValue label="Action" value="query" />
            <BoundValue label="Scope" value={job.capability?.resource_scope ?? "Unavailable"} />
          </div>

          {loading ? <p className="empty-state">Loading finalized dataset...</p> : null}
          {!loading && dataset ? (
            <div className="reconciliation-query-fields">
              <label>
                <span>From</span>
                <input
                  type="datetime-local"
                  step="1"
                  min={datasetBounds(dataset).minimum}
                  max={datasetBounds(dataset).maximum}
                  value={window.from}
                  onChange={(event) => {
                    setWindow((current) => ({ ...current, from: event.target.value }));
                    setResult(null);
                  }}
                />
              </label>
              <label>
                <span>To</span>
                <input
                  type="datetime-local"
                  step="1"
                  min={datasetBounds(dataset).minimum}
                  max={datasetBounds(dataset).maximum}
                  value={window.to}
                  onChange={(event) => {
                    setWindow((current) => ({ ...current, to: event.target.value }));
                    setResult(null);
                  }}
                />
              </label>
              <label>
                <span>Row limit</span>
                <input
                  type="number"
                  min="1"
                  max="100"
                  step="1"
                  value={limit}
                  onChange={(event) => {
                    setLimit(event.target.value);
                    setResult(null);
                  }}
                />
              </label>
              <button
                type="button"
                className="primary-button reconciliation-submit"
                disabled={busy || !ready || queryError !== null}
                onClick={() => void submit()}
              >
                {busy ? "Authorizing..." : "Run protected query"}
              </button>
              {queryError ? <p className="field-guidance">{queryError}</p> : null}
            </div>
          ) : null}

          {error ? (
            <p className="form-error reconciliation-message" role="alert">
              {error}
            </p>
          ) : null}
        </aside>

        <div className="reconciliation-results" aria-live="polite">
          {!result ? (
            <div className="reconciliation-empty">
              <span>SYNTHETIC</span>
              <strong>No protected result yet</strong>
              <p>A successful request returns only rows inside the capability dataset scope.</p>
            </div>
          ) : (
            <>
              <div className="reconciliation-result-band">
                <div>
                  <span>Outcome</span>
                  <strong>ACCEPTED</strong>
                </div>
                <div>
                  <span>Rows returned</span>
                  <strong>{result.row_count}</strong>
                </div>
                <div>
                  <span>Verification</span>
                  <strong>{result.verification_path === "CACHE" ? "CACHE HIT" : "VERIFIER"}</strong>
                </div>
                <div>
                  <span>Classification</span>
                  <strong>{result.classification}</strong>
                </div>
              </div>

              <section className="reconciliation-records" aria-labelledby="result-rows-heading">
                <div className="section-heading-row compact-heading">
                  <div>
                    <p className="section-index">Authorized result</p>
                    <h2 id="result-rows-heading">Transactions</h2>
                  </div>
                  <span>{result.row_count} rows</span>
                </div>
                <div className="reconciliation-table-wrap">
                  <table className="reconciliation-table">
                    <thead>
                      <tr>
                        <th>Transaction</th>
                        <th>Occurred at</th>
                        <th>Amount</th>
                        <th>Merchant</th>
                        <th>Account</th>
                        <th>Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {result.rows.map((row) => (
                        <tr key={row.transactionId}>
                          <td>{row.transactionId}</td>
                          <td>{row.occurredAt}</td>
                          <td>
                            {row.amountMinor} {row.currency}
                          </td>
                          <td>{row.merchantReference ?? "-"}</td>
                          <td>{row.accountReference ?? "-"}</td>
                          <td>
                            <span className="reconciliation-row-status">
                              {row.reconciliationStatus}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </section>

              <section className="reconciliation-receipt" aria-labelledby="receipt-heading">
                <div className="section-heading-row compact-heading">
                  <div>
                    <p className="section-index">Public audit record</p>
                    <h2 id="receipt-heading">Receipt summary</h2>
                  </div>
                  <span>{result.receipt.occurred_at}</span>
                </div>
                <div className="reconciliation-receipt-grid">
                  <BoundValue label="Receipt ID" value={result.receipt.receipt_id} />
                  <BoundValue label="Request ID" value={result.receipt.request_id} />
                  <BoundValue label="Capability ID" value={result.receipt.capability_id} />
                  <BoundValue label="Proof job ID" value={result.receipt.proof_job_id} />
                  <BoundValue label="Task ID" value={result.receipt.task_id} />
                  <BoundValue label="Policy record" value={result.receipt.policy_record_id} />
                  <BoundValue label="Policy hash" value={result.receipt.policy_hash} />
                  <BoundValue label="Snapshot ID" value={result.receipt.snapshot_id} />
                  <BoundValue
                    label="Relationship root"
                    value={result.receipt.relationship_snapshot_root}
                  />
                  <BoundValue label="Resource scope" value={result.receipt.resource_scope} />
                  <BoundValue
                    label="Public claims digest"
                    value={result.receipt.public_claims_digest}
                  />
                  <BoundValue label="Proof digest" value={result.receipt.proof_digest} />
                  <BoundValue label="Reason code" value={result.receipt.reason_code} />
                </div>
              </section>
            </>
          )}
        </div>
      </div>
    </section>
  );
}

function BoundValue({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

function defaultWindow(dataset: SyntheticDataset): QueryWindow {
  const values = dataset.rows.map((row) => Date.parse(row.occurredAt)).sort((a, b) => a - b);
  const first = values[0];
  const last = values.at(-1);
  if (first === undefined || last === undefined) {
    return { from: "", to: "" };
  }
  const from = Math.max(first, last - MAX_QUERY_RANGE_MILLISECONDS);
  return { from: utcToInput(from), to: utcToInput(last) };
}

function datasetBounds(dataset: SyntheticDataset): {
  readonly maximum: string;
  readonly minimum: string;
} {
  const values = dataset.rows.map((row) => Date.parse(row.occurredAt)).sort((a, b) => a - b);
  const first = values[0];
  const last = values.at(-1);
  return {
    minimum: first === undefined ? "" : utcToInput(first),
    maximum: last === undefined ? "" : utcToInput(last),
  };
}

function validateWindow(
  dataset: SyntheticDataset | null,
  window: QueryWindow,
  limit: string,
): string | null {
  if (!dataset) {
    return null;
  }
  const from = Date.parse(`${window.from}Z`);
  const to = Date.parse(`${window.to}Z`);
  const bounds = datasetBounds(dataset);
  const minimum = Date.parse(`${bounds.minimum}Z`);
  const maximum = Date.parse(`${bounds.maximum}Z`);
  if (![from, to, minimum, maximum].every(Number.isFinite)) {
    return "Choose a complete whole-second UTC window.";
  }
  if (from > to) {
    return "The start time must not follow the end time.";
  }
  if (from < minimum || to > maximum) {
    return "The query window must stay inside this finalized dataset.";
  }
  if (to - from > MAX_QUERY_RANGE_MILLISECONDS) {
    return "The query window cannot exceed 31 days.";
  }
  if (!/^[0-9]+$/.test(limit) || Number(limit) < 1 || Number(limit) > 100) {
    return "The row limit must be between 1 and 100.";
  }
  return null;
}

function inputToUtc(value: string): string {
  const parsed = Date.parse(`${value}Z`);
  if (!Number.isFinite(parsed)) {
    throw new Error("The query window is invalid.");
  }
  return new Date(parsed).toISOString().replace(".000Z", "Z");
}

function utcToInput(milliseconds: number): string {
  return new Date(milliseconds).toISOString().replace(".000Z", "").replace("Z", "");
}

function queryFailureMessage(cause: unknown): string {
  if (!(cause instanceof PortalRequestError)) {
    return "The protected query service is unavailable.";
  }
  if (cause.code === "INVALID_RECONCILIATION_QUERY") {
    return "The query window or row limit is invalid.";
  }
  if (cause.code === "RECONCILIATION_AUTHORIZATION_FAILED") {
    return "Authorization failed. No protected transaction rows were returned.";
  }
  return "The protected query service is unavailable.";
}
