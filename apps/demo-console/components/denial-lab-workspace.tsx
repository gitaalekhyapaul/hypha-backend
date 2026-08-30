"use client";

import { useState } from "react";

import type {
  CapabilityJobProjection,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import { PortalRequestError, queryReconciliation } from "../lib/portal-client/index.ts";

interface DenialLabWorkspaceProps {
  readonly access: PublishedAccessSelection;
  readonly csrfToken: string;
  readonly job: CapabilityJobProjection;
  readonly onSessionExpired: () => void;
}

type DenialState = "ready" | "submitting" | "denied" | "error";

export function DenialLabWorkspace({
  access,
  csrfToken,
  job,
  onSessionExpired,
}: DenialLabWorkspaceProps) {
  const [state, setState] = useState<DenialState>("ready");

  async function submitScopeDenial(): Promise<void> {
    setState("submitting");
    const occurredAt = wholeSecondNow();
    try {
      await queryReconciliation(
        {
          job_id: job.job_id,
          dataset_id: access.datasetId,
          request_mode: "OUT_OF_SCOPE",
          from_occurred_at: occurredAt,
          to_occurred_at: occurredAt,
          limit: 1,
        },
        csrfToken,
      );
      setState("error");
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      if (
        cause instanceof PortalRequestError &&
        cause.status === 403 &&
        cause.code === "RECONCILIATION_AUTHORIZATION_FAILED"
      ) {
        setState("denied");
        return;
      }
      setState("error");
    }
  }

  return (
    <section className="reconciliation-workspace" aria-labelledby="denial-lab-heading">
      <header className="view-header reconciliation-view-header">
        <div>
          <p className="eyebrow">Protected request controls</p>
          <h1 id="denial-lab-heading">Denial lab</h1>
          <p className="view-description">
            Send one signed request beyond the live capability scope and confirm that no rows
            return.
          </p>
        </div>
        <div className="reconciliation-header-status">
          <span className="classification-flag">Synthetic</span>
          <div>
            <span>Expected result</span>
            <strong>DENIED</strong>
          </div>
        </div>
      </header>

      <div className="reconciliation-layout">
        <aside className="reconciliation-query-panel" aria-labelledby="scope-denial-heading">
          <div className="section-heading-row compact-heading">
            <div>
              <p className="section-index">Controlled case</p>
              <h2 id="scope-denial-heading">Out-of-scope dataset</h2>
            </div>
            <span>Signed</span>
          </div>

          <div className="reconciliation-binding">
            <BoundValue label="Capability scope" value={access.datasetId} />
            <BoundValue label="Action" value="query" />
            <BoundValue label="Agent key" value={access.agent.key_thumbprint} />
          </div>

          <button
            type="button"
            className="primary-button reconciliation-submit"
            disabled={state === "submitting"}
            onClick={() => void submitScopeDenial()}
          >
            {state === "submitting" ? "Submitting signed request..." : "Test scope denial"}
          </button>
        </aside>

        <div className="reconciliation-results" aria-live="polite">
          <div className="reconciliation-empty">
            <span>{state === "denied" ? "DENIED" : "READY"}</span>
            <strong>{denialTitle(state)}</strong>
            <p>{denialDetail(state)}</p>
          </div>
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

function wholeSecondNow(): string {
  return new Date(Math.floor(Date.now() / 1_000) * 1_000).toISOString().replace(".000Z", "Z");
}

function denialTitle(state: DenialState): string {
  if (state === "denied") {
    return "Authorization rejected";
  }
  if (state === "error") {
    return "Denial check unavailable";
  }
  if (state === "submitting") {
    return "Checking signed request";
  }
  return "No denial check yet";
}

function denialDetail(state: DenialState): string {
  if (state === "denied") {
    return "The gateway returned a generic denial, no protected rows, and one public rejected receipt.";
  }
  if (state === "error") {
    return "The gateway did not return the expected generic denial.";
  }
  return "The test uses the live agent key, a fresh nonce, and a fixed dataset outside the capability scope.";
}
