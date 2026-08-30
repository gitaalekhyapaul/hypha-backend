"use client";

import { useEffect, useState } from "react";

import {
  loadReceipts,
  PortalRequestError,
  type PublicAuditReceipt,
  type PublicReceiptLedger,
  type ReceiptFilterField,
  receiptQueryPath,
} from "../lib/portal-client/index.ts";

interface ReceiptsWorkspaceProps {
  readonly onSessionExpired: () => void;
}

type ReceiptViewState = "loading" | "ready" | "unavailable";
type FilterChoice = "all" | ReceiptFilterField;

const FILTER_OPTIONS: readonly { readonly value: FilterChoice; readonly label: string }[] = [
  { value: "all", label: "All receipts" },
  { value: "task_id", label: "Task ID" },
  { value: "capability_id", label: "Capability ID" },
  { value: "proof_job_id", label: "Proof job ID" },
  { value: "policy_record_id", label: "Policy record ID" },
  { value: "snapshot_id", label: "Snapshot ID" },
  { value: "request_id", label: "Request ID" },
];

export function ReceiptsWorkspace({ onSessionExpired }: ReceiptsWorkspaceProps) {
  const [state, setState] = useState<ReceiptViewState>("loading");
  const [ledger, setLedger] = useState<PublicReceiptLedger | null>(null);
  const [filterChoice, setFilterChoice] = useState<FilterChoice>("all");
  const [filterValue, setFilterValue] = useState("");

  useEffect(() => {
    void refresh("all", "");
  }, []);

  async function refresh(choice = filterChoice, value = filterValue): Promise<void> {
    const normalized = value.trim();
    setState("loading");
    try {
      const result = await loadReceipts(
        choice === "all" || normalized.length === 0
          ? undefined
          : { field: choice, value: normalized },
      );
      setLedger(result);
      setState("ready");
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setState("unavailable");
    }
  }

  const normalizedFilter = filterValue.trim();
  const activeFilter =
    filterChoice === "all" || normalizedFilter.length === 0
      ? undefined
      : { field: filterChoice, value: normalizedFilter };
  const verifiedRequestIds = new Set(
    ledger?.receipts
      .filter((receipt) => receipt.receipt_type === "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION")
      .map((receipt) => receipt.request_id) ?? [],
  );

  return (
    <section className="receipts-workspace" aria-labelledby="receipts-heading">
      <header className="view-header receipts-view-header">
        <div>
          <p className="eyebrow">Public audit records</p>
          <h1 id="receipts-heading">Receipts</h1>
          <p className="view-description">
            Review validated public identifiers from protected requests and denials.
          </p>
        </div>
        <div className="receipts-count">
          <span>Visible</span>
          <strong>{ledger?.filtered_count ?? 0}</strong>
        </div>
      </header>

      <div className="receipts-toolbar">
        <label>
          <span>Filter field</span>
          <select
            value={filterChoice}
            onChange={(event) => setFilterChoice(event.target.value as FilterChoice)}
          >
            {FILTER_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="receipts-filter-value">
          <span>Exact value</span>
          <input
            type="text"
            value={filterValue}
            disabled={filterChoice === "all"}
            maxLength={256}
            placeholder={filterChoice === "all" ? "No filter" : "Enter exact public ID"}
            onChange={(event) => setFilterValue(event.target.value)}
          />
        </label>
        <button
          type="button"
          className="primary-button"
          disabled={state === "loading"}
          onClick={() => void refresh()}
        >
          {state === "loading" ? "Loading..." : "Apply filter"}
        </button>
        <a
          className="secondary-button receipts-export"
          href={receiptQueryPath(activeFilter, "jsonl")}
          download="raziel-public-receipts.jsonl"
        >
          Export JSONL
        </a>
      </div>

      {state === "unavailable" ? (
        <p className="form-error receipts-message" role="alert">
          Receipt ledger unavailable.
        </p>
      ) : null}

      <div className="receipts-table-wrap">
        <table className="receipts-table">
          <thead>
            <tr>
              <th>Occurred</th>
              <th>Type</th>
              <th>Path</th>
              <th>Outcome</th>
              <th>Reason</th>
              <th>Request</th>
              <th>Capability</th>
              <th>Policy</th>
              <th>Snapshot</th>
            </tr>
          </thead>
          <tbody>
            {ledger?.receipts.map((receipt) => (
              <tr key={receipt.receipt_id}>
                <td>{receipt.occurred_at}</td>
                <td title={receipt.receipt_type}>{shortReceiptType(receipt.receipt_type)}</td>
                <td>{verificationPath(receipt, verifiedRequestIds)}</td>
                <td>
                  <span className="receipt-outcome" data-outcome={receipt.outcome}>
                    {shortOutcome(receipt.outcome)}
                  </span>
                </td>
                <td title={receipt.reason_code}>{shortReason(receipt.reason_code)}</td>
                <td title={receipt.request_id}>{receipt.request_id}</td>
                <td title={receipt.capability_id}>{receipt.capability_id}</td>
                <td title={receipt.policy_record_id}>{receipt.policy_record_id}</td>
                <td title={receipt.snapshot_id}>{receipt.snapshot_id}</td>
              </tr>
            ))}
            {state === "ready" && ledger?.receipts.length === 0 ? (
              <tr>
                <td className="receipts-empty" colSpan={9}>
                  No matching public receipts.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function shortOutcome(value: string): string {
  return value.replace("AUDIT_OUTCOME_", "");
}

function shortReceiptType(value: string): string {
  return value.replace("AUDIT_RECEIPT_TYPE_", "");
}

function verificationPath(
  receipt: PublicAuditReceipt,
  verifiedRequestIds: ReadonlySet<string>,
): "CACHE" | "REPLAY" | "VERIFIER" {
  if (receipt.reason_code === "PUBLIC_ERROR_CODE_REQUEST_REPLAYED") {
    return "REPLAY";
  }
  if (
    receipt.receipt_type === "AUDIT_RECEIPT_TYPE_CAPABILITY_VERIFICATION" ||
    verifiedRequestIds.has(receipt.request_id)
  ) {
    return "VERIFIER";
  }
  return "CACHE";
}

function shortReason(value: string): string {
  return value.replace("PUBLIC_ERROR_CODE_", "");
}
