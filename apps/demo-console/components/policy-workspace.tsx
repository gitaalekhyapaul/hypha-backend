"use client";

import { useState } from "react";
import type { ActivePolicySelection } from "../lib/access-graph/types.ts";
import type { PolicyActivationProjection } from "../lib/policy-activation/types.ts";
import {
  buildPolicyCandidateRequest,
  defaultPolicyBuilderState,
  type PolicyBuilderState,
} from "../lib/policy-builder/index.ts";
import type { PolicyCandidateProjection } from "../lib/policy-candidate/types.ts";
import {
  activatePolicyCandidate,
  compilePolicyCandidate,
  loadPolicyCandidate,
  PortalRequestError,
} from "../lib/portal-client/index.ts";

import { PolicyCandidateReview } from "./policy-candidate-review.tsx";
import { PolicyField, PolicyRuleEditor } from "./policy-rule-controls.tsx";
import { PolicyVersionWorkspace } from "./policy-version-workspace.tsx";

interface PolicyWorkspaceProps {
  readonly workflow?: "legacy" | "versioned";
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
  readonly onPolicyActivated: (selection: ActivePolicySelection) => void;
}

export function PolicyWorkspace({ workflow = "legacy", ...props }: PolicyWorkspaceProps) {
  return workflow === "versioned" ? (
    <PolicyVersionWorkspace csrfToken={props.csrfToken} onSessionExpired={props.onSessionExpired} />
  ) : (
    <LegacyPolicyWorkspace {...props} />
  );
}

function LegacyPolicyWorkspace({
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
              <PolicyField
                label="Policy name"
                value={builder.policyName}
                onChange={(value) => update("policyName", value)}
              />
              <PolicyField
                label="Resource type"
                value={builder.resourceType}
                onChange={(value) => update("resourceType", value)}
              />
              <PolicyField
                label="Policy version"
                inputMode="numeric"
                value={builder.policyVersion}
                onChange={(value) => update("policyVersion", value)}
              />
              <PolicyField
                label="Approval epoch"
                inputMode="numeric"
                value={builder.expectedApprovalEpoch}
                onChange={(value) => update("expectedApprovalEpoch", value)}
              />
            </div>
          </section>

          <PolicyRuleEditor builder={builder} update={update} />
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

        <PolicyCandidateReview
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

function nextCandidateId(): string {
  return `candidate-${Date.now().toString(36)}-${crypto.randomUUID().slice(0, 8)}`;
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
