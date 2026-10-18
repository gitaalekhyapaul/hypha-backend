"use client";

import { useEffect, useId, useState } from "react";
import {
  type ApprovedPolicyCatalogueEntry,
  recentApprovedPolicies,
} from "../lib/policy-version/catalogue-types.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";
import { getApprovedPolicyCatalogue } from "../lib/portal-client/policy-catalogue.ts";

export function PolicyCatalogue({
  selectedFamilyId,
  selectionDisabled,
  refreshKey,
  onSelectFamily,
  onSessionExpired,
}: {
  readonly selectedFamilyId: string | undefined;
  readonly selectionDisabled: boolean;
  readonly refreshKey: string | undefined;
  readonly onSelectFamily: (familyId: string) => void;
  readonly onSessionExpired: () => void;
}) {
  const headingId = useId();
  const [policies, setPolicies] = useState<readonly ApprovedPolicyCatalogueEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    setPolicies([]);
    void getApprovedPolicyCatalogue(controller.signal)
      .then((catalogue) => {
        if (!controller.signal.aborted) setPolicies(recentApprovedPolicies(catalogue.policies));
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        if (cause instanceof PortalRequestError && cause.status === 401) {
          setError("Operator session expired.");
          onSessionExpired();
        } else setError("Approved policy catalogue unavailable.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [onSessionExpired, revision, refreshKey]);

  return (
    <section className="policy-section" aria-labelledby={headingId} aria-busy={loading}>
      <div className="section-heading-row compact-heading">
        <h2 id={headingId}>Installed approvals</h2>
        <div className="header-actions">
          <span className="section-count">{loading ? "..." : policies.length}</span>
          <button
            className="secondary-button compact-button"
            type="button"
            title="Refresh approved policies"
            aria-label="Refresh approved policies"
            disabled={loading}
            style={{ width: 32, height: 32, padding: 0, fontSize: 20 }}
            onClick={() => setRevision((current) => current + 1)}
          >
            <span aria-hidden="true">&#8635;</span>
          </button>
        </div>
      </div>
      {loading ? (
        <p className="empty-state" role="status">
          Loading approved policies...
        </p>
      ) : error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : policies.length === 0 ? (
        <p className="empty-state">No installed approved policy versions.</p>
      ) : (
        <div className="agent-registration-table-wrap" style={{ maxHeight: 360 }}>
          <table className="agent-registration-token-table">
            <caption className="sr-only">Installed approved policy versions</caption>
            <thead>
              <tr>
                <th scope="col">Version ID</th>
                <th scope="col">Family ID</th>
                <th scope="col">Version</th>
                <th scope="col">Approval epoch</th>
                <th scope="col" aria-sort="descending">
                  Valid from (UTC) <span aria-hidden="true">&#8595;</span>
                </th>
                <th scope="col">Valid until (UTC)</th>
                <th scope="col">Draft family</th>
              </tr>
            </thead>
            <tbody>
              {policies.map((entry) => {
                const selected = selectedFamilyId === entry.identity.familyId;
                return (
                  <tr key={entry.identity.versionId}>
                    <td>
                      <code style={{ width: "auto", maxWidth: 260 }}>
                        {entry.identity.versionId}
                      </code>
                    </td>
                    <td>
                      <code style={{ width: "auto", maxWidth: 260 }}>
                        {entry.identity.familyId}
                      </code>
                    </td>
                    <td>v{entry.policyVersion}</td>
                    <td>{entry.approvalEpoch}</td>
                    <td>
                      <PolicyTime seconds={entry.validFromUnixSeconds} />
                    </td>
                    <td>
                      <PolicyTime seconds={entry.validUntilUnixSeconds} />
                    </td>
                    <td>
                      <button
                        type="button"
                        className="secondary-button compact-button"
                        style={{ minWidth: 104 }}
                        disabled={selectionDisabled || selected}
                        aria-pressed={selected}
                        aria-label={`Use family ${entry.identity.familyId} for a new version`}
                        onClick={() => onSelectFamily(entry.identity.familyId)}
                      >
                        {selected ? "Selected" : "Use family"}
                      </button>
                      <details style={{ marginTop: 8 }}>
                        <summary>Approval details</summary>
                        <dl>
                          <dt>Policy record ID</dt>
                          <dd style={{ marginLeft: 0 }}>
                            <code>{entry.recordId}</code>
                          </dd>
                          <dt>Policy hash</dt>
                          <dd style={{ marginLeft: 0 }}>
                            <code>{entry.policyHash}</code>
                          </dd>
                          <dt>Resource type</dt>
                          <dd style={{ marginLeft: 0 }}>
                            <code>{entry.resourceType}</code>
                          </dd>
                        </dl>
                      </details>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function PolicyTime({ seconds }: { readonly seconds: string }) {
  const iso = new Date(Number(seconds) * 1_000).toISOString();
  return (
    <time dateTime={iso} style={{ whiteSpace: "nowrap" }}>
      {iso.replace("T", " ").slice(0, 19)}
    </time>
  );
}
