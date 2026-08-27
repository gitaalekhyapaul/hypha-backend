"use client";

import { useEffect, useMemo, useState } from "react";

import type { ActivePolicySelection } from "../lib/access-graph/types.ts";
import type {
  CapabilityContextEntry,
  CapabilityJobProjection,
  CapabilityJobState,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import { CAVEAT_CATALOG } from "../lib/policy-draft/index.ts";
import {
  cancelCapabilityJob,
  loadCapabilityJob,
  PortalRequestError,
  requestCapability,
} from "../lib/portal-client/index.ts";

interface AgentCapabilityWorkspaceProps {
  readonly activePolicy: ActivePolicySelection;
  readonly access: PublishedAccessSelection;
  readonly csrfToken: string;
  readonly job: CapabilityJobProjection | null;
  readonly onJobChange: (job: CapabilityJobProjection | null) => void;
  readonly onSessionExpired: () => void;
}

export function AgentCapabilityWorkspace({
  activePolicy,
  access,
  csrfToken,
  job,
  onJobChange,
  onSessionExpired,
}: AgentCapabilityWorkspaceProps) {
  const [validForSeconds, setValidForSeconds] = useState("5400");
  const [requestedAmount, setRequestedAmount] = useState("25000");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestParameters = useMemo(() => {
    if (activePolicy.builder.mode !== "caveat") {
      return [];
    }
    return (
      CAVEAT_CATALOG.find(
        (entry) => entry.template === activePolicy.builder.caveatTemplate,
      )?.parameters.filter((parameter) => parameter.source === "REQUEST_CONTEXT") ?? []
    );
  }, [activePolicy.builder]);

  useEffect(() => {
    if (!job || isTerminal(job.state)) {
      return;
    }
    let disposed = false;
    const timer = window.setTimeout(async () => {
      try {
        const current = await loadCapabilityJob(job.job_id);
        if (!disposed) {
          onJobChange(current);
          setError(null);
        }
      } catch (cause) {
        if (!disposed) {
          if (cause instanceof PortalRequestError && cause.status === 401) {
            onSessionExpired();
          } else {
            setError("Capability status is temporarily unavailable.");
          }
        }
      }
    }, 1500);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [job, onJobChange, onSessionExpired]);

  async function submit(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const validity = capabilityValidity(validForSeconds, access.snapshotExpiresAtUnixSeconds);
      const requestContext = capabilityContext(requestParameters, requestedAmount);
      onJobChange(
        await requestCapability(
          {
            agent_id: access.agentId,
            task_id: activePolicy.builder.taskId,
            purpose: activePolicy.builder.purpose,
            action: "query",
            resource_type: "dataset",
            resource_scope: activePolicy.builder.resourceScope,
            requested_valid_for_seconds: validity,
            snapshot_id: access.snapshotId,
            snapshot_expires_at_unix_seconds: access.snapshotExpiresAtUnixSeconds,
            request_context: requestContext,
          },
          csrfToken,
        ),
      );
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError(capabilityFailureMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  async function cancel(): Promise<void> {
    if (!job) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      onJobChange(await cancelCapabilityJob(job.job_id, csrfToken));
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError(capabilityFailureMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="capability-workspace">
      <header className="view-header capability-view-header">
        <div>
          <p className="eyebrow">Proof-backed authorization</p>
          <h1>Agent and capability</h1>
          <p className="view-description">
            Bind a signed request to the accepted policy, snapshot, and agent key.
          </p>
        </div>
        <div className="capability-mode-chip" data-mode={job?.mode ?? "pending"}>
          <span>{job?.mode === "live" ? "LIVE PROOF" : "LOCAL MODE"}</span>
          <strong>{job ? jobStateLabel(job.state) : "Ready to request"}</strong>
        </div>
      </header>

      <div className="capability-layout">
        <div className="capability-editor">
          <section className="access-section" aria-labelledby="capability-agent-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">01</p>
                <h2 id="capability-agent-heading">Agent binding</h2>
              </div>
              <span>ED25519</span>
            </div>
            <div className="access-field-grid">
              <ReadOnlyValue label="Agent label" value={access.agentId} />
              <ReadOnlyValue label="Subject type" value={access.agent.subject_type} />
            </div>
            <Commitment label="Key thumbprint" value={access.agent.key_thumbprint} />
          </section>

          <section className="access-section" aria-labelledby="capability-scope-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">02</p>
                <h2 id="capability-scope-heading">Capability scope</h2>
              </div>
              <span>QUERY</span>
            </div>
            <div className="access-field-grid">
              <ReadOnlyValue label="Task" value={activePolicy.builder.taskId} />
              <ReadOnlyValue label="Purpose" value={activePolicy.builder.purpose} />
              <ReadOnlyValue label="Resource type" value="dataset" />
              <ReadOnlyValue label="Resource scope" value={activePolicy.builder.resourceScope} />
              <ReadOnlyValue label="Dataset" value={access.datasetId} />
              <label className="access-field">
                <span>Requested validity</span>
                <div className="input-with-unit">
                  <input
                    inputMode="numeric"
                    value={validForSeconds}
                    disabled={job !== null}
                    onChange={(event) => setValidForSeconds(event.target.value)}
                  />
                  <span>seconds</span>
                </div>
              </label>
            </div>
          </section>

          {requestParameters.some((parameter) => parameter.name === "requested_amount") ? (
            <section className="access-section" aria-labelledby="capability-context-heading">
              <div className="section-heading-row">
                <div>
                  <p className="section-index">03</p>
                  <h2 id="capability-context-heading">Private request context</h2>
                </div>
                <span>CAVEAT INPUT</span>
              </div>
              <label className="access-field compact-access-field">
                <span>Requested amount</span>
                <input
                  inputMode="numeric"
                  value={requestedAmount}
                  disabled={job !== null}
                  onChange={(event) => setRequestedAmount(event.target.value)}
                />
              </label>
            </section>
          ) : null}

          <section className="access-section capability-submit-section">
            <div className="capability-bound-records">
              <Commitment label="Policy hash" value={access.policyHash} />
              <Commitment label="Snapshot ID" value={access.snapshotId} />
            </div>
            <div className="capability-actions">
              <button
                type="button"
                className="primary-button"
                disabled={busy || job !== null}
                onClick={() => void submit()}
              >
                {busy && !job ? "Signing..." : "Request capability"}
              </button>
              {job && !isTerminal(job.state) ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy}
                  onClick={() => void cancel()}
                >
                  {busy ? "Canceling..." : "Cancel request"}
                </button>
              ) : null}
            </div>
            {error ? (
              <p className="form-error access-message" role="alert">
                {error}
              </p>
            ) : null}
          </section>
        </div>

        <CapabilityJobReview job={job} />
      </div>
    </section>
  );
}

function CapabilityJobReview({ job }: { readonly job: CapabilityJobProjection | null }) {
  return (
    <aside className="snapshot-review capability-job-review" aria-labelledby="job-review-heading">
      <div className="snapshot-review-header">
        <div>
          <p className="section-index">Proof job</p>
          <h2 id="job-review-heading">Public status</h2>
        </div>
        <span className="compiled-status" data-active={job !== null}>
          {job ? jobStateLabel(job.state) : "NOT REQUESTED"}
        </span>
      </div>
      {!job ? (
        <div className="candidate-empty snapshot-empty">
          <strong>No proof job</strong>
          <p>The signed request status appears here after submission.</p>
        </div>
      ) : (
        <div className="capability-job-details">
          <div className="job-mode-notice" data-mode={job.mode}>
            <strong>{job.mode === "recorded" ? "Recorded playback" : "Live proof service"}</strong>
            <p>
              {job.mode === "recorded"
                ? "This timeline cannot issue a capability or unlock protected data."
                : "The proof job is authoritative and can issue a capability after verification."}
            </p>
          </div>
          <Commitment label="Job ID" value={job.job_id} />
          <ol className="proof-progress" aria-label="Proof progress">
            {[
              "PROOF_JOB_STATE_QUEUED",
              "PROOF_JOB_STATE_EXECUTING",
              "PROOF_JOB_STATE_PROVING",
              "PROOF_JOB_STATE_SUCCEEDED",
            ].map((state) => (
              <li
                key={state}
                data-current={job.state === state}
                data-passed={statePassed(job.state, state)}
              >
                <span />
                <strong>{jobStateLabel(state as CapabilityJobState)}</strong>
              </li>
            ))}
          </ol>
          {job.metrics ? (
            <dl className="candidate-summary-list capability-metrics">
              <SummaryValue label="Execution" value={`${job.metrics.execution_milliseconds} ms`} />
              <SummaryValue label="Proving" value={`${job.metrics.proving_milliseconds} ms`} />
              <SummaryValue label="Proof size" value={`${job.metrics.proof_size_bytes} bytes`} />
            </dl>
          ) : null}
          {job.capability ? (
            <div className="issued-capability">
              <strong>Capability issued</strong>
              <Commitment label="Capability ID" value={job.capability.capability_id} />
              <Commitment label="Verification key" value={job.capability.verification_key_id} />
            </div>
          ) : null}
          {job.mode === "recorded" && job.recorded_source ? (
            <p className="recorded-source">Source: {job.recorded_source}</p>
          ) : null}
        </div>
      )}
    </aside>
  );
}

function capabilityValidity(input: string, snapshotExpiresAt: string): number {
  if (!/^[0-9]+$/.test(input)) {
    throw new Error("Requested validity must be a whole number.");
  }
  const value = Number(input);
  if (!Number.isSafeInteger(value) || value < 4500 || value > 86_400) {
    throw new Error("Requested validity must be between 4500 and 86400 seconds.");
  }
  const now = Math.floor(Date.now() / 1000);
  if (BigInt(now + value) > BigInt(snapshotExpiresAt)) {
    throw new Error("The snapshot expires before this capability request. Publish a new snapshot.");
  }
  return value;
}

function capabilityContext(
  parameters: readonly { readonly name: string }[],
  requestedAmount: string,
): readonly CapabilityContextEntry[] {
  return parameters.map((parameter) => {
    if (parameter.name !== "requested_amount" || !/^[0-9]+$/.test(requestedAmount)) {
      throw new Error("Requested amount must be a whole number.");
    }
    return { name: parameter.name, value: { kind: "int", value: requestedAmount } };
  });
}

function ReadOnlyValue({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="access-field readonly-access-field">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function Commitment({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="candidate-commitment">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function SummaryValue({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function isTerminal(state: CapabilityJobState): boolean {
  return (
    state === "PROOF_JOB_STATE_SUCCEEDED" ||
    state === "PROOF_JOB_STATE_FAILED" ||
    state === "PROOF_JOB_STATE_CANCELED"
  );
}

function jobStateLabel(state: CapabilityJobState): string {
  return state.replace("PROOF_JOB_STATE_", "").replaceAll("_", " ");
}

function statePassed(current: CapabilityJobState, candidate: string): boolean {
  const states = [
    "PROOF_JOB_STATE_QUEUED",
    "PROOF_JOB_STATE_EXECUTING",
    "PROOF_JOB_STATE_PROVING",
    "PROOF_JOB_STATE_SUCCEEDED",
  ];
  return states.indexOf(candidate) <= states.indexOf(current);
}

function capabilityFailureMessage(cause: unknown): string {
  if (!(cause instanceof PortalRequestError)) {
    return cause instanceof Error ? cause.message : "Capability request failed.";
  }
  switch (cause.code) {
    case "INVALID_CAPABILITY_REQUEST":
      return "The capability scope, context, or expiry is invalid.";
    case "CAPABILITY_NOT_FOUND":
      return "This proof job is no longer attached to the current agent session.";
    case "CAPABILITY_CONFLICT":
      return "This signed request conflicts with an existing proof job.";
    default:
      return "The capability service is unavailable.";
  }
}
