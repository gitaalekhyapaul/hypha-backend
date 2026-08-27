"use client";

import { useMemo, useState } from "react";
import type { ActivePolicySelection } from "../lib/access-graph/types.ts";
import type { PolicyActivationProjection } from "../lib/policy-activation/types.ts";
import {
  buildPolicyCandidateRequest,
  DIRECT_RELATION_OPTIONS,
  defaultPolicyBuilderState,
  POLICY_LEAF_OPTIONS,
  POLICY_MODE_OPTIONS,
  type PolicyBuilderState,
  type PolicyLeafSelection,
} from "../lib/policy-builder/index.ts";
import type { PolicyCandidateProjection } from "../lib/policy-candidate/types.ts";
import {
  CAVEAT_CATALOG,
  type CaveatSelection,
  type DirectRelation,
} from "../lib/policy-draft/index.ts";
import {
  activatePolicyCandidate,
  compilePolicyCandidate,
  loadPolicyCandidate,
  PortalRequestError,
} from "../lib/portal-client/index.ts";

interface PolicyWorkspaceProps {
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
  readonly onPolicyActivated: (selection: ActivePolicySelection) => void;
}

export function PolicyWorkspace({
  csrfToken,
  onSessionExpired,
  onPolicyActivated,
}: PolicyWorkspaceProps) {
  const [builder, setBuilder] = useState<PolicyBuilderState>(defaultPolicyBuilderState);
  const [candidate, setCandidate] = useState<PolicyCandidateProjection | null>(null);
  const [activePolicy, setActivePolicy] = useState<PolicyActivationProjection | null>(null);
  const [candidateIsStale, setCandidateIsStale] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const selectedMode = useMemo(
    () => POLICY_MODE_OPTIONS.find((option) => option.value === builder.mode),
    [builder.mode],
  );

  function update<K extends keyof PolicyBuilderState>(
    field: K,
    value: PolicyBuilderState[K],
  ): void {
    setBuilder((current) => ({ ...current, [field]: value }));
    setCandidateIsStale(candidate !== null);
    setActivePolicy(null);
    setError(null);
    setNotice(null);
  }

  async function compile(): Promise<void> {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const request = buildPolicyCandidateRequest(builder, nextCandidateId());
      const compiled = await compilePolicyCandidate(request, csrfToken);
      setCandidate(compiled);
      setActivePolicy(null);
      setCandidateIsStale(false);
      setNotice("Policy candidate compiled and ready for review.");
    } catch (cause) {
      handleFailure(cause);
    } finally {
      setBusy(false);
    }
  }

  async function refreshCandidate(): Promise<void> {
    if (!candidate) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setCandidate(await loadPolicyCandidate(candidate.candidate_id));
      setNotice("Candidate status refreshed.");
    } catch (cause) {
      handleFailure(cause);
    } finally {
      setBusy(false);
    }
  }

  async function activate(): Promise<void> {
    if (!candidate || candidateIsStale) {
      return;
    }
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const activated = await activatePolicyCandidate(candidate.candidate_id, csrfToken);
      setActivePolicy(activated);
      onPolicyActivated({ activation: activated, candidate, builder: { ...builder } });
      setNotice("Policy approved and published to the trusted registry.");
    } catch (cause) {
      handleFailure(cause);
    } finally {
      setBusy(false);
    }
  }

  function handleFailure(cause: unknown): void {
    if (cause instanceof PortalRequestError && cause.status === 401) {
      onSessionExpired();
      return;
    }
    setError(policyFailureMessage(cause));
  }

  return (
    <section className="policy-workspace">
      <header className="view-header policy-view-header">
        <div>
          <p className="eyebrow">Authorization policy</p>
          <h1>Policy builder</h1>
          <p className="view-description">
            Compose the supported query rules, then compile an immutable review candidate.
          </p>
        </div>
        <div className="policy-header-status">
          <span className="classification-badge">Bounded editor</span>
          <span>SpiceDB 1.54.0 subset</span>
        </div>
      </header>

      <div className="policy-builder-shell">
        <div className="policy-authoring-pane">
          <section className="policy-section" aria-labelledby="policy-identity-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">01</p>
                <h2 id="policy-identity-heading">Candidate identity</h2>
              </div>
              <span>Permission is fixed to query</span>
            </div>
            <div className="policy-field-grid">
              <Field
                label="Policy name"
                value={builder.policyName}
                onChange={(value) => update("policyName", value)}
              />
              <Field
                label="Resource type"
                value={builder.resourceType}
                onChange={(value) => update("resourceType", value)}
              />
              <Field
                label="Policy version"
                inputMode="numeric"
                value={builder.policyVersion}
                onChange={(value) => update("policyVersion", value)}
              />
              <Field
                label="Approval epoch"
                inputMode="numeric"
                value={builder.expectedApprovalEpoch}
                onChange={(value) => update("expectedApprovalEpoch", value)}
              />
            </div>
          </section>

          <section className="policy-section" aria-labelledby="rule-family-heading">
            <div className="section-heading-row">
              <div>
                <p className="section-index">02</p>
                <h2 id="rule-family-heading">Rule family</h2>
              </div>
              <span>{selectedMode?.description}</span>
            </div>
            <div className="policy-mode-selector" role="tablist" aria-label="Rule family">
              {POLICY_MODE_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  role="tab"
                  aria-selected={builder.mode === option.value}
                  data-active={builder.mode === option.value}
                  onClick={() => update("mode", option.value)}
                >
                  {option.label}
                </button>
              ))}
            </div>

            <RuleConfiguration builder={builder} update={update} />
            <RuleDiagram builder={builder} />
          </section>

          <section
            className="policy-section policy-submit-section"
            aria-labelledby="compile-heading"
          >
            <div className="section-heading-row">
              <div>
                <p className="section-index">03</p>
                <h2 id="compile-heading">Compile candidate</h2>
              </div>
              <span>Candidate lifetime</span>
            </div>
            <div className="compile-controls">
              <label>
                <span>Valid for seconds</span>
                <input
                  inputMode="numeric"
                  value={builder.validForSeconds}
                  onChange={(event) => update("validForSeconds", event.target.value)}
                />
              </label>
              <button
                className="primary-button compile-policy-button"
                type="button"
                disabled={busy}
                onClick={() => void compile()}
              >
                {busy ? "Compiling..." : "Compile candidate"}
              </button>
            </div>
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
          </section>
        </div>

        <CandidateReview
          candidate={candidate}
          activePolicy={activePolicy}
          stale={candidateIsStale}
          busy={busy}
          onActivate={() => void activate()}
          onRefresh={() => void refreshCandidate()}
        />
      </div>
    </section>
  );
}

function RuleConfiguration({
  builder,
  update,
}: {
  readonly builder: PolicyBuilderState;
  readonly update: <K extends keyof PolicyBuilderState>(
    field: K,
    value: PolicyBuilderState[K],
  ) => void;
}) {
  switch (builder.mode) {
    case "direct":
      return (
        <div className="rule-control-grid single-rule-control">
          <SelectField
            label="Agent relation"
            value={builder.directRelation}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("directRelation", value as DirectRelation)}
          />
        </div>
      );
    case "team":
      return (
        <div className="fixed-rule-note">
          <strong>team_viewer</strong>
          <span>The related team must contain the requesting agent as a member.</span>
        </div>
      );
    case "alternative":
      return (
        <div className="rule-control-grid">
          <SelectField
            label="First path"
            value={builder.alternativeLeft}
            options={POLICY_LEAF_OPTIONS}
            onChange={(value) => update("alternativeLeft", value as PolicyLeafSelection)}
          />
          <SelectField
            label="Second path"
            value={builder.alternativeRight}
            options={POLICY_LEAF_OPTIONS}
            onChange={(value) => update("alternativeRight", value as PolicyLeafSelection)}
          />
        </div>
      );
    case "dual_control":
      return (
        <div className="rule-control-grid">
          <SelectField
            label="Required relation A"
            value={builder.dualLeft}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("dualLeft", value as DirectRelation)}
          />
          <SelectField
            label="Required relation B"
            value={builder.dualRight}
            options={DIRECT_RELATION_OPTIONS.map((value) => ({
              value,
              label: relationLabel(value),
            }))}
            onChange={(value) => update("dualRight", value as DirectRelation)}
          />
        </div>
      );
    case "caveat": {
      const selectedCaveat = CAVEAT_CATALOG.find(
        (entry) => entry.template === builder.caveatTemplate,
      );
      const bindsClaims =
        builder.caveatTemplate === "task_purpose_scope" ||
        builder.caveatTemplate === "reconciliation_scope";
      return (
        <div className="caveat-controls">
          <div className="rule-control-grid">
            <SelectField
              label="Agent relation"
              value={builder.caveatRelation}
              options={DIRECT_RELATION_OPTIONS.map((value) => ({
                value,
                label: relationLabel(value),
              }))}
              onChange={(value) => update("caveatRelation", value as DirectRelation)}
            />
            <SelectField
              label="Caveat template"
              value={builder.caveatTemplate}
              options={CAVEAT_CATALOG.map((entry) => ({
                value: entry.template,
                label: entry.label,
              }))}
              onChange={(value) => update("caveatTemplate", value as CaveatSelection["template"])}
            />
          </div>
          {selectedCaveat ? (
            <div className="caveat-definition">
              <p>{selectedCaveat.description}</p>
              <ul>
                {selectedCaveat.parameters.map((parameter) => (
                  <li key={parameter.name}>
                    <code>{parameter.name}</code>
                    <span>{sourceLabel(parameter.source)}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {bindsClaims ? (
            <div className="policy-field-grid caveat-claim-fields">
              <Field
                label="Task ID"
                value={builder.taskId}
                onChange={(value) => update("taskId", value)}
              />
              <Field
                label="Purpose"
                value={builder.purpose}
                onChange={(value) => update("purpose", value)}
              />
              <Field
                label="Resource scope"
                value={builder.resourceScope}
                onChange={(value) => update("resourceScope", value)}
              />
            </div>
          ) : null}
        </div>
      );
    }
  }
}

function RuleDiagram({ builder }: { readonly builder: PolicyBuilderState }) {
  const leaves = diagramLeaves(builder);
  const operator =
    builder.mode === "alternative" ? "ANY" : builder.mode === "dual_control" ? "ALL" : null;
  return (
    <fieldset className="rule-diagram">
      <legend className="sr-only">Policy rule preview</legend>
      <div className="rule-diagram-header">
        <span>Visual rule</span>
        <code>permission query</code>
      </div>
      <div className="rule-diagram-body">
        {operator ? <span className="rule-operator">{operator}</span> : null}
        <div className="rule-leaf-row">
          {leaves.map((leaf) => (
            <div className="rule-leaf" key={leaf.key}>
              <span>{leaf.type}</span>
              <strong>{leaf.label}</strong>
              {leaf.detail ? <small>{leaf.detail}</small> : null}
            </div>
          ))}
        </div>
      </div>
    </fieldset>
  );
}

function CandidateReview({
  candidate,
  activePolicy,
  stale,
  busy,
  onActivate,
  onRefresh,
}: {
  readonly candidate: PolicyCandidateProjection | null;
  readonly activePolicy: PolicyActivationProjection | null;
  readonly stale: boolean;
  readonly busy: boolean;
  readonly onActivate: () => void;
  readonly onRefresh: () => void;
}) {
  return (
    <aside className="candidate-review" aria-labelledby="candidate-review-heading">
      <div className="candidate-review-header">
        <div>
          <p className="section-index">Review</p>
          <h2 id="candidate-review-heading">Compiled candidate</h2>
        </div>
        {candidate ? (
          <span className="compiled-status" data-active={activePolicy !== null}>
            {activePolicy?.status ?? candidate.status}
          </span>
        ) : null}
      </div>
      {!candidate ? (
        <div className="candidate-empty">
          <strong>No compiled candidate</strong>
          <p>The safe public projection appears here after the compiler accepts the draft.</p>
        </div>
      ) : (
        <div className="candidate-details">
          {stale ? (
            <p className="candidate-stale">The form changed after this candidate compiled.</p>
          ) : null}
          <dl className="candidate-summary-list">
            <CandidateValue label="Rule" value={candidate.rule_summary} />
            <CandidateValue
              label="Policy"
              value={`${candidate.policy_name} v${candidate.policy_version}`}
            />
            <CandidateValue
              label="Resource"
              value={`${candidate.resource_type}#${candidate.permission}`}
            />
            <CandidateValue label="Approval epoch" value={candidate.expected_approval_epoch} />
            <CandidateValue
              label="Valid until"
              value={formatPublicTimestamp(candidate.valid_until.seconds)}
            />
          </dl>
          <div className="candidate-commitment">
            <span>Policy hash</span>
            <code>{candidate.policy_hash}</code>
          </div>
          <div className="candidate-commitment">
            <span>Source schema digest</span>
            <code>{candidate.source_schema_digest}</code>
          </div>
          <div className="candidate-records">
            <span>Candidate ID</span>
            <code>{candidate.candidate_id}</code>
            <span>Record ID</span>
            <code>{candidate.record_id}</code>
          </div>
          {activePolicy ? (
            <div className="active-policy-summary" role="status">
              <div>
                <span>Registry generation</span>
                <strong>{activePolicy.registry_generation}</strong>
              </div>
              <div>
                <span>Registered</span>
                <strong>{formatPublicTimestamp(activePolicy.registered_at.seconds)}</strong>
              </div>
            </div>
          ) : null}
          <div className="candidate-actions">
            <button
              className="primary-button candidate-activate-button"
              type="button"
              disabled={busy || stale || activePolicy !== null}
              onClick={onActivate}
            >
              {activePolicy ? "Policy active" : busy ? "Publishing..." : "Approve and activate"}
            </button>
            <button
              className="secondary-button candidate-refresh-button"
              type="button"
              disabled={busy}
              onClick={onRefresh}
            >
              Refresh candidate
            </button>
          </div>
        </div>
      )}
      <footer className="candidate-boundary-note">
        <strong>Public review boundary</strong>
        <span>
          Schema source, PolicyIR, compiler configuration, and diagnostics remain private.
        </span>
      </footer>
    </aside>
  );
}

function Field({
  label,
  value,
  inputMode,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly inputMode?: "numeric";
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="policy-field">
      <span>{label}</span>
      <input
        inputMode={inputMode}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: readonly { readonly value: string; readonly label: string }[];
  readonly onChange: (value: string) => void;
}) {
  return (
    <label className="policy-field policy-select-field">
      <span>{label}</span>
      <select value={value} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function CandidateValue({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

interface DiagramLeaf {
  readonly key: string;
  readonly type: string;
  readonly label: string;
  readonly detail?: string;
}

function diagramLeaves(builder: PolicyBuilderState): readonly DiagramLeaf[] {
  switch (builder.mode) {
    case "direct":
      return [directDiagramLeaf(builder.directRelation, "direct")];
    case "team":
      return [{ key: "team", type: "subject set", label: "team_viewer", detail: "team#member" }];
    case "alternative":
      return [
        selectionDiagramLeaf(builder.alternativeLeft, "alternative-left"),
        selectionDiagramLeaf(builder.alternativeRight, "alternative-right"),
      ];
    case "dual_control":
      return [
        directDiagramLeaf(builder.dualLeft, "dual-left"),
        directDiagramLeaf(builder.dualRight, "dual-right"),
      ];
    case "caveat": {
      const caveatLabel = CAVEAT_CATALOG.find(
        (entry) => entry.template === builder.caveatTemplate,
      )?.label;
      return [
        {
          ...directDiagramLeaf(builder.caveatRelation, "caveat"),
          ...(caveatLabel ? { detail: caveatLabel } : {}),
        },
      ];
    }
  }
}

function selectionDiagramLeaf(selection: PolicyLeafSelection, key: string): DiagramLeaf {
  if (selection === "team_viewer") {
    return { key, type: "subject set", label: "team_viewer", detail: "team#member" };
  }
  if (selection === "organization_audit") {
    return { key, type: "delegation", label: "organization", detail: "organization->audit" };
  }
  return directDiagramLeaf(selection, key);
}

function directDiagramLeaf(relation: DirectRelation, key: string): DiagramLeaf {
  return { key, type: "direct relation", label: relation };
}

function relationLabel(relation: DirectRelation): string {
  return relation.charAt(0).toUpperCase() + relation.slice(1);
}

function sourceLabel(source: string): string {
  switch (source) {
    case "CAPABILITY_CLAIM":
      return "Capability claim";
    case "RELATIONSHIP_CONTEXT":
      return "Committed relationship";
    case "REQUEST_CONTEXT":
      return "Request context";
    default:
      return source;
  }
}

function nextCandidateId(): string {
  return `candidate-${Date.now().toString(36)}-${crypto.randomUUID().slice(0, 8)}`;
}

function formatPublicTimestamp(seconds: string): string {
  const milliseconds = Number(seconds) * 1000;
  if (!Number.isFinite(milliseconds)) {
    return "Invalid timestamp";
  }
  return new Intl.DateTimeFormat("en-SG", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "UTC",
  }).format(new Date(milliseconds));
}

function policyFailureMessage(cause: unknown): string {
  if (!(cause instanceof PortalRequestError)) {
    return cause instanceof Error ? cause.message : "Policy draft is invalid.";
  }
  switch (cause.code) {
    case "INVALID_CANDIDATE_REQUEST":
      return "The policy draft contains an invalid or unsupported value.";
    case "CANDIDATE_CONFLICT":
      return "This candidate identifier already belongs to another draft. Compile again.";
    case "COMPILER_FAILED":
      return "The compiler rejected this supported-subset policy.";
    case "INSPECTION_FAILED":
      return "The compiled candidate failed its independent inspection step.";
    case "CANDIDATE_EXPIRED":
      return "This candidate expired. Compile a new candidate from the current draft.";
    case "INVALID_POLICY_ACTIVATION_REQUEST":
      return "This compiled candidate cannot be activated.";
    case "POLICY_ACTIVATION_CONFLICT":
      return "Trusted registry state changed. Compile a candidate with the next approval epoch.";
    case "POLICY_ACTIVATION_REJECTED":
      return "The policy authority rejected this candidate.";
    case "POLICY_ACTIVATION_UNAVAILABLE":
      return "The policy activation service is unavailable.";
    default:
      return "The policy compiler is unavailable.";
  }
}
