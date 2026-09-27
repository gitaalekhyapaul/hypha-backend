import type { PolicyActivationProjection } from "../lib/policy-activation/types.ts";
import type { PolicyCandidateProjection } from "../lib/policy-candidate/types.ts";
import type { PolicyVersionIdentity } from "../lib/policy-draft/types.ts";
export function PolicyCandidateReview({
  candidate,
  activePolicy,
  stale,
  busy,
  onActivate,
  onRefresh,
  identity,
  publishing = busy,
}: {
  readonly candidate: PolicyCandidateProjection | null;
  readonly activePolicy: PolicyActivationProjection | null;
  readonly stale: boolean;
  readonly busy: boolean;
  readonly onActivate: () => void;
  readonly onRefresh: () => void;
  readonly identity?: PolicyVersionIdentity;
  readonly publishing?: boolean;
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
            {identity ? (
              <>
                <span>Policy family ID</span>
                <code>{identity.familyId}</code>
                <span>Version ID</span>
                <code>{identity.versionId}</code>
                <span>Version</span>
                <strong>{identity.version}</strong>
              </>
            ) : null}
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
              {activePolicy
                ? identity
                  ? "Version published"
                  : "Policy active"
                : publishing
                  ? "Publishing..."
                  : identity
                    ? "Publish version"
                    : "Approve and activate"}
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

function CandidateValue({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
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
