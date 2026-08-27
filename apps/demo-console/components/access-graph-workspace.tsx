"use client";

import { useMemo, useState } from "react";

import {
  buildAccessGraphRelationships,
  defaultAccessGraphForm,
  graphValiditySeconds,
} from "../lib/access-graph/model.ts";
import type {
  AccessGraphFormState,
  AccessGraphPublicationProjection,
  ActivePolicySelection,
} from "../lib/access-graph/types.ts";
import type {
  AgentSessionProjection,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import { CAVEAT_CATALOG } from "../lib/policy-draft/index.ts";
import {
  createAgentSession,
  PortalRequestError,
  publishAccessGraph,
} from "../lib/portal-client/index.ts";

interface AccessGraphWorkspaceProps {
  readonly activePolicy: ActivePolicySelection;
  readonly agentSession: AgentSessionProjection | null;
  readonly csrfToken: string;
  readonly operatorId: string;
  readonly onAgentSessionCreated: (agent: AgentSessionProjection) => void;
  readonly onSessionExpired: () => void;
  readonly onSnapshotPublished: (selection: PublishedAccessSelection) => void;
}

export function AccessGraphWorkspace({
  activePolicy,
  agentSession,
  csrfToken,
  operatorId,
  onAgentSessionCreated,
  onSessionExpired,
  onSnapshotPublished,
}: AccessGraphWorkspaceProps) {
  const [form, setForm] = useState<AccessGraphFormState>(defaultAccessGraphForm);
  const [publication, setPublication] = useState<AccessGraphPublicationProjection | null>(null);
  const [busy, setBusy] = useState(false);
  const [agentBusy, setAgentBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const preview = useMemo(() => {
    if (!agentSession) {
      return { error: "Create the key-bound agent identity to preview relationships." };
    }
    try {
      return {
        relationships: buildAccessGraphRelationships(
          activePolicy.builder,
          form,
          agentSession.subject_id,
        ),
      };
    } catch (cause) {
      return { error: cause instanceof Error ? cause.message : "Graph values are invalid." };
    }
  }, [activePolicy.builder, agentSession, form]);
  const requiresTeam = preview.relationships?.some(
    (relationship) => relationship.subject_type === "team",
  );
  const requiresOrganization = preview.relationships?.some(
    (relationship) => relationship.subject_type === "organization",
  );
  const caveat = CAVEAT_CATALOG.find(
    (entry) => entry.template === activePolicy.builder.caveatTemplate,
  );
  const relationshipParameters =
    activePolicy.builder.mode === "caveat"
      ? (caveat?.parameters.filter((parameter) => parameter.source === "RELATIONSHIP_CONTEXT") ??
        [])
      : [];

  function update<K extends keyof AccessGraphFormState>(
    field: K,
    value: AccessGraphFormState[K],
  ): void {
    if (publication) {
      return;
    }
    setForm((current) => ({ ...current, [field]: value }));
    setError(null);
  }

  async function prepareAgent(): Promise<void> {
    setAgentBusy(true);
    setError(null);
    try {
      onAgentSessionCreated(await createAgentSession(csrfToken));
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError("The agent identity service is unavailable.");
    } finally {
      setAgentBusy(false);
    }
  }

  async function publish(): Promise<void> {
    if (!agentSession || !preview.relationships) {
      setError(preview.error ?? "Graph values are invalid.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await publishAccessGraph(
        {
          candidate_id: activePolicy.candidate.candidate_id,
          valid_for_seconds: graphValiditySeconds(form.validForSeconds),
          relationships: preview.relationships,
        },
        csrfToken,
      );
      setPublication(result);
      onSnapshotPublished({
        agent: agentSession,
        agentId: form.agentId,
        datasetId: form.datasetId,
        snapshotId: result.snapshot_id,
        policyHash: result.policy_hash,
        relationshipRoot: result.relationship_root,
        snapshotExpiresAtUnixSeconds: result.expires_at.seconds,
      });
    } catch (cause) {
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      setError(accessGraphFailureMessage(cause));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="access-workspace">
      <header className="view-header access-view-header">
        <div>
          <p className="eyebrow">Relationship snapshot</p>
          <h1>Access graph</h1>
          <p className="view-description">
            Assign the active policy to the reconciliation agent and publish one private snapshot.
          </p>
        </div>
        <div className="access-policy-chip">
          <span>{activePolicy.activation.status}</span>
          <strong>{activePolicy.candidate.rule_summary}</strong>
        </div>
      </header>

      <div className="access-layout">
        <div className="access-editor">
          <section className="access-section" aria-labelledby="access-agent-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">01</p>
                <h2 id="access-agent-heading">Key-bound agent</h2>
              </div>
              <span>{agentSession ? "READY" : "REQUIRED"}</span>
            </div>
            {agentSession ? (
              <div className="agent-identity-summary">
                <ReadOnlyField label="Subject type" value={agentSession.subject_type} />
                <Commitment label="Key thumbprint and subject ID" value={agentSession.subject_id} />
              </div>
            ) : (
              <div className="agent-identity-empty">
                <div>
                  <strong>No agent key prepared</strong>
                  <p>The private Ed25519 key stays in this console server process.</p>
                </div>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={agentBusy}
                  onClick={() => void prepareAgent()}
                >
                  {agentBusy ? "Creating..." : "Create agent identity"}
                </button>
              </div>
            )}
          </section>

          <section className="access-section" aria-labelledby="access-context-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">02</p>
                <h2 id="access-context-heading">Assignment context</h2>
              </div>
              <span>{activePolicy.candidate.resource_type}#query</span>
            </div>
            <div className="access-field-grid">
              <ReadOnlyField label="Operator" value={operatorId} />
              <AccessField
                label="Agent ID"
                value={form.agentId}
                disabled={publication !== null}
                onChange={(value) => update("agentId", value)}
              />
              <AccessField
                label="Dataset ID"
                value={form.datasetId}
                disabled={publication !== null}
                onChange={(value) => update("datasetId", value)}
              />
              <ReadOnlyField label="Task ID" value={activePolicy.builder.taskId} />
              <ReadOnlyField label="Purpose" value={activePolicy.builder.purpose} />
              <ReadOnlyField label="Resource scope" value={activePolicy.builder.resourceScope} />
              {requiresTeam ? (
                <AccessField
                  label="Team ID"
                  value={form.teamId}
                  disabled={publication !== null}
                  onChange={(value) => update("teamId", value)}
                />
              ) : null}
              {requiresOrganization ? (
                <AccessField
                  label="Organization ID"
                  value={form.organizationId}
                  disabled={publication !== null}
                  onChange={(value) => update("organizationId", value)}
                />
              ) : null}
            </div>
          </section>

          {relationshipParameters.length > 0 ? (
            <section className="access-section" aria-labelledby="access-caveat-heading">
              <div className="section-heading-row">
                <div>
                  <p className="section-index">03</p>
                  <h2 id="access-caveat-heading">Committed caveat values</h2>
                </div>
                <span>{caveat?.label}</span>
              </div>
              <div className="caveat-value-grid">
                {relationshipParameters.some(
                  (parameter) => parameter.name === "approved_amount",
                ) ? (
                  <AccessField
                    label="Approved amount"
                    value={form.approvedAmount}
                    inputMode="numeric"
                    disabled={publication !== null}
                    onChange={(value) => update("approvedAmount", value)}
                  />
                ) : null}
                {relationshipParameters.some((parameter) => parameter.name === "is_approved") ? (
                  <ToggleField
                    label="Approved"
                    checked={form.isApproved}
                    disabled={publication !== null}
                    onChange={(value) => update("isApproved", value)}
                  />
                ) : null}
                {relationshipParameters.some((parameter) => parameter.name === "is_suspended") ? (
                  <ToggleField
                    label="Suspended"
                    checked={form.isSuspended}
                    disabled={publication !== null}
                    onChange={(value) => update("isSuspended", value)}
                  />
                ) : null}
                {relationshipParameters.some((parameter) => parameter.name === "is_override") ? (
                  <ToggleField
                    label="Override"
                    checked={form.isOverride}
                    disabled={publication !== null}
                    onChange={(value) => update("isOverride", value)}
                  />
                ) : null}
              </div>
            </section>
          ) : null}

          <section className="access-section" aria-labelledby="access-preview-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">{relationshipParameters.length > 0 ? "04" : "03"}</p>
                <h2 id="access-preview-heading">Graph preview</h2>
              </div>
              <span>{preview.relationships?.length ?? 0} relationships</span>
            </div>
            {preview.relationships ? (
              <div className="relationship-preview">
                {preview.relationships.map((relationship) => (
                  <div className="relationship-row" key={relationshipKey(relationship)}>
                    <div className="relationship-node">
                      <span>{relationship.subject_type}</span>
                      <strong>{relationship.subject_id}</strong>
                    </div>
                    <div className="relationship-edge">
                      <span>{relationship.relation}</span>
                    </div>
                    <div className="relationship-node relationship-resource-node">
                      <span>{relationship.resource_type}</span>
                      <strong>{relationship.resource_id}</strong>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="form-error" role="alert">
                {preview.error}
              </p>
            )}
          </section>

          <section className="access-section access-publish-section">
            <label className="access-field compact-access-field">
              <span>Snapshot lifetime</span>
              <div className="input-with-unit">
                <input
                  inputMode="numeric"
                  value={form.validForSeconds}
                  disabled={publication !== null}
                  onChange={(event) => update("validForSeconds", event.target.value)}
                />
                <span>seconds</span>
              </div>
            </label>
            <button
              type="button"
              className="primary-button access-publish-button"
              disabled={busy || publication !== null || !agentSession || !preview.relationships}
              onClick={() => void publish()}
            >
              {publication ? "Snapshot accepted" : busy ? "Publishing..." : "Publish access graph"}
            </button>
            {error ? (
              <p className="form-error access-message" role="alert">
                {error}
              </p>
            ) : null}
          </section>
        </div>

        <SnapshotReview activePolicy={activePolicy} publication={publication} />
      </div>
    </section>
  );
}

function SnapshotReview({
  activePolicy,
  publication,
}: {
  readonly activePolicy: ActivePolicySelection;
  readonly publication: AccessGraphPublicationProjection | null;
}) {
  return (
    <aside className="snapshot-review" aria-labelledby="snapshot-review-heading">
      <div className="snapshot-review-header">
        <div>
          <p className="section-index">Snapshot</p>
          <h2 id="snapshot-review-heading">Public record</h2>
        </div>
        <span className="compiled-status" data-active={publication !== null}>
          {publication?.status ?? "NOT PUBLISHED"}
        </span>
      </div>
      {!publication ? (
        <div className="candidate-empty snapshot-empty">
          <strong>No accepted snapshot</strong>
          <p>The signed public snapshot record appears after graph publication.</p>
        </div>
      ) : (
        <div className="snapshot-details">
          <dl className="candidate-summary-list">
            <SummaryValue
              label="Relationships"
              value={String(publication.accepted_relationship_count)}
            />
            <SummaryValue label="Snapshot epoch" value={publication.epoch} />
            <SummaryValue label="Registry generation" value={publication.registry_generation} />
            <SummaryValue
              label="Expires"
              value={formatPublicTimestamp(publication.expires_at.seconds)}
            />
          </dl>
          <Commitment label="Snapshot ID" value={publication.snapshot_id} />
          <Commitment label="Relationship root" value={publication.relationship_root} />
          <Commitment label="Policy hash" value={publication.policy_hash} />
        </div>
      )}
      <footer className="candidate-boundary-note">
        <strong>{activePolicy.candidate.policy_name}</strong>
        <span>Private relationships and caveat values are excluded from this record.</span>
      </footer>
    </aside>
  );
}

function AccessField({
  label,
  value,
  disabled,
  inputMode,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly disabled: boolean;
  readonly inputMode?: "numeric";
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="access-field">
      <span>{label}</span>
      <input
        value={value}
        disabled={disabled}
        inputMode={inputMode}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

function ReadOnlyField({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="access-field readonly-access-field">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ToggleField({
  label,
  checked,
  disabled,
  onChange,
}: {
  readonly label: string;
  readonly checked: boolean;
  readonly disabled: boolean;
  readonly onChange: (value: boolean) => void;
}) {
  return (
    <label className="toggle-field">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span aria-hidden="true" />
      <strong>{label}</strong>
    </label>
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

function Commitment({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="candidate-commitment">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function relationshipKey(relationship: {
  readonly resource_type: string;
  readonly resource_id: string;
  readonly relation: string;
  readonly subject_type: string;
  readonly subject_id: string;
}): string {
  return [
    relationship.resource_type,
    relationship.resource_id,
    relationship.relation,
    relationship.subject_type,
    relationship.subject_id,
  ].join(":");
}

function formatPublicTimestamp(seconds: string): string {
  return new Intl.DateTimeFormat("en-SG", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "UTC",
  }).format(new Date(Number(seconds) * 1000));
}

function accessGraphFailureMessage(cause: unknown): string {
  if (!(cause instanceof PortalRequestError)) {
    return cause instanceof Error ? cause.message : "Access graph publication failed.";
  }
  switch (cause.code) {
    case "INVALID_ACCESS_GRAPH_REQUEST":
      return "The graph contains an invalid identifier or caveat value.";
    case "ACCESS_GRAPH_CONFLICT":
      return "This policy candidate already has a different relationship batch.";
    case "ACCESS_GRAPH_REJECTED":
      return "The active policy rejected this relationship shape.";
    default:
      return "The access graph service is unavailable.";
  }
}
