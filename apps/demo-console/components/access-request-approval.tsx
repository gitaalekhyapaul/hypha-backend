"use client";

import { useEffect, useId, useRef, useState } from "react";
import type {
  ApprovedPolicyCatalogueEntry,
  GrantInputField,
} from "../lib/policy-version/catalogue-types.ts";
import {
  type AccessRequest,
  type AccessRequestGrant,
  type AccessRequestApproval as ApprovalBody,
  approveAccessRequest,
  getAccessRequest,
} from "../lib/portal-client/access-requests.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";
import { getApprovedPolicyCatalogue } from "../lib/portal-client/policy-catalogue.ts";

const DEFAULT_GRANT: AccessRequestGrant = {
  formatVersion: 1,
  teamId: "finance-operations",
  organizationId: "nus-fintech-lab",
  approvedAmount: "50000",
  isApproved: true,
  isSuspended: false,
  isOverride: false,
};

interface ApprovalProps {
  readonly request: AccessRequest;
  readonly csrfToken: string;
  readonly disabled: boolean;
  readonly onBusyChange: (busy: boolean) => void;
  readonly onUpdated: (request: AccessRequest) => void;
  readonly onSessionExpired: () => void;
}

export function AccessRequestApproval({
  request,
  csrfToken,
  disabled,
  onBusyChange,
  onUpdated,
  onSessionExpired,
}: ApprovalProps) {
  const id = useId();
  const [policies, setPolicies] = useState<readonly ApprovedPolicyCatalogueEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [catalogueError, setCatalogueError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [selection, setSelection] = useState("");
  const [grant, setGrant] = useState(DEFAULT_GRANT);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const alive = useRef(false);
  const pending = useRef(false);
  const retry = useRef<ApprovalBody | null>(null);
  const previousPolicy = useRef("");
  const selected = policies.find((policy) => policyKey(policy) === selection);
  const selectedIsCurrent = selected !== undefined && policyIsCurrent(selected);
  const selectedPolicy = selected
    ? `${policyKey(selected)}:${selected.policyHash}:${selected.requiredGrantFields.join(",")}`
    : "";

  useEffect(() => {
    setError(null);
    setNotice(null);
    if (!selectedPolicy || previousPolicy.current === selectedPolicy) return;
    previousPolicy.current = selectedPolicy;
    retry.current = null;
    setGrant({ ...DEFAULT_GRANT });
  }, [selectedPolicy]);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setCatalogueError(null);
    void getApprovedPolicyCatalogue(controller.signal)
      .then((catalogue) => {
        if (!controller.signal.aborted) setPolicies(catalogue.policies);
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        setPolicies([]);
        if (cause instanceof PortalRequestError && cause.status === 401) onSessionExpired();
        else setCatalogueError("Approved policies unavailable.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [revision, onSessionExpired]);

  function changeGrant(next: AccessRequestGrant) {
    retry.current = null;
    setError(null);
    setNotice(null);
    setGrant(next);
  }

  async function approve() {
    if (pending.current || disabled || loading || catalogueError || !selected) return;
    if (!policyIsCurrent(selected)) {
      setError("Selected approval is no longer current. Refresh approved policies.");
      return;
    }
    pending.current = true;
    setBusy(true);
    onBusyChange(true);
    setError(null);
    setNotice(null);
    // Keep the submitted generation and private inputs unchanged after an uncertain response.
    const body: ApprovalBody = retry.current ?? {
      formatVersion: 1,
      expectedGeneration: String(request.generation),
      policyRecordId: selected.recordId,
      policyVersionId: selected.identity.versionId,
      approvalEpoch: selected.approvalEpoch,
      grant: { ...grant },
    };
    retry.current = body;
    try {
      const result = await approveAccessRequest(
        request.agentId,
        request.requestId,
        body,
        csrfToken,
      );
      if (!alive.current) return;
      retry.current = null;
      onUpdated(result);
    } catch (cause) {
      if (!alive.current) return;
      if (cause instanceof PortalRequestError && cause.status === 401) {
        onSessionExpired();
        return;
      }
      if (cause instanceof PortalRequestError && [404, 409, 410].includes(cause.status)) {
        retry.current = null;
        setRevision((current) => current + 1);
        try {
          const latest = await getAccessRequest(request.agentId, request.requestId);
          if (!alive.current) return;
          onUpdated(latest);
          setNotice("Request or approval changed. Latest status loaded. Review the selection.");
        } catch (refreshCause) {
          if (!alive.current) return;
          if (refreshCause instanceof PortalRequestError && refreshCause.status === 401) {
            onSessionExpired();
          } else setError("Request changed. Latest status unavailable. Refresh request details.");
        }
      } else if (cause instanceof PortalRequestError && cause.status === 400) {
        retry.current = null;
        setError("Approval not accepted. Check the policy and private grant values.");
      } else if (cause instanceof PortalRequestError && cause.status === 403) {
        retry.current = null;
        setError("Operator action denied.");
      } else {
        setError("Approval not confirmed. The same approval is retained for retry.");
      }
    } finally {
      pending.current = false;
      // Approval can remove this form. Release the parent's action lock in that case too.
      onBusyChange(false);
      if (alive.current) setBusy(false);
    }
  }

  return (
    <section className="access-request-approval" aria-labelledby={`${id}-heading`}>
      <div className="access-request-detail-heading">
        <h3 id={`${id}-heading`}>Request approval</h3>
        <button
          type="button"
          className="secondary-button access-request-icon"
          title="Refresh approved policies"
          aria-label="Refresh approved policies"
          disabled={loading || disabled || busy}
          onClick={() => setRevision((current) => current + 1)}
        >
          <span aria-hidden="true">&#8635;</span>
        </button>
      </div>
      {loading ? <p role="status">Loading approved policies...</p> : null}
      {catalogueError ? (
        <p className="form-error" role="alert">
          {catalogueError}
        </p>
      ) : null}
      {!loading && !catalogueError && policies.length === 0 ? (
        <p className="empty-state">No installed approved policy versions.</p>
      ) : null}
      {!loading && !catalogueError && selection && !selectedIsCurrent ? (
        <p className="form-error" role="alert">
          Selected approval is no longer current.
        </p>
      ) : null}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void approve();
        }}
      >
        <fieldset className="access-request-approval-fields" disabled={disabled || busy}>
          <div className="access-request-approval-policy">
            <label htmlFor={`${id}-policy`}>Approved policy version</label>
            <select
              id={`${id}-policy`}
              value={selection}
              disabled={loading || Boolean(catalogueError) || policies.length === 0}
              required
              onChange={(event) => {
                retry.current = null;
                setError(null);
                setNotice(null);
                setGrant({ ...DEFAULT_GRANT });
                setSelection(event.target.value);
              }}
            >
              <option value="">Select approved version</option>
              {selection && !selected ? (
                <option value={selection} disabled>
                  Previous selection unavailable
                </option>
              ) : null}
              {policies.map((policy) => (
                <option
                  key={policyKey(policy)}
                  value={policyKey(policy)}
                  disabled={!policyIsCurrent(policy)}
                >
                  {policy.identity.familyId.slice(0, 8)} / v{policy.identity.version} / epoch{" "}
                  {policy.approvalEpoch} / {policy.policyHash.slice(0, 12)}
                </option>
              ))}
            </select>
            {selected ? <PolicySelection policy={selected} /> : null}
          </div>
          <PrivateGrantFields
            id={id}
            grant={grant}
            requiredFields={selectedIsCurrent ? selected.requiredGrantFields : []}
            onChange={changeGrant}
          />
          {error ? (
            <p className="form-error" role="alert">
              {error}
            </p>
          ) : null}
          {notice ? (
            <p className="form-notice" role="status">
              {notice}
            </p>
          ) : null}
          <div className="access-request-approval-submit">
            <button
              type="submit"
              className="primary-button"
              disabled={loading || Boolean(catalogueError) || !selectedIsCurrent}
            >
              {busy ? "Approving..." : "Approve request"}
            </button>
          </div>
        </fieldset>
      </form>
    </section>
  );
}

function PolicySelection({ policy }: { readonly policy: ApprovedPolicyCatalogueEntry }) {
  return (
    <dl className="access-request-approval-selection">
      <div>
        <dt>Family</dt>
        <dd>
          <code>{policy.identity.familyId}</code>
        </dd>
      </div>
      <div>
        <dt>Policy hash</dt>
        <dd>
          <code>{policy.policyHash}</code>
        </dd>
      </div>
      <div>
        <dt>Valid until (UTC)</dt>
        <dd>
          {new Date(Number(policy.validUntilUnixSeconds) * 1000)
            .toISOString()
            .replace("T", " ")
            .slice(0, 19)}
        </dd>
      </div>
    </dl>
  );
}

function PrivateGrantFields({
  id,
  grant,
  requiredFields,
  onChange,
}: {
  readonly id: string;
  readonly grant: AccessRequestGrant;
  readonly requiredFields: readonly GrantInputField[];
  readonly onChange: (grant: AccessRequestGrant) => void;
}) {
  if (requiredFields.length === 0) return null;
  const fields = new Set(requiredFields);
  return (
    <details className="access-request-grant" open>
      <summary>Private grant options</summary>
      <div className="access-request-grant-fields">
        {(
          [
            { key: "teamId", label: "Team ID" },
            { key: "organizationId", label: "Organization ID" },
          ] as const
        )
          .filter(({ key }) => fields.has(key))
          .map(({ key, label }) => (
            <div key={key}>
              <label htmlFor={`${id}-${key}`}>{label}</label>
              <input
                id={`${id}-${key}`}
                value={grant[key]}
                required
                maxLength={128}
                pattern="[A-Za-z0-9][A-Za-z0-9._:\-]{0,127}"
                autoComplete="off"
                spellCheck={false}
                onChange={(event) => onChange({ ...grant, [key]: event.target.value })}
              />
            </div>
          ))}
        {fields.has("approvedAmount") ? (
          <div>
            <label htmlFor={`${id}-amount`}>Approved amount</label>
            <input
              id={`${id}-amount`}
              type="number"
              min="1"
              max="9007199254740991"
              step="1"
              required
              value={grant.approvedAmount}
              onChange={(event) => onChange({ ...grant, approvedAmount: event.target.value })}
            />
          </div>
        ) : null}
        {requiredFields.some(
          (field) => field === "isApproved" || field === "isSuspended" || field === "isOverride",
        ) ? (
          <div className="access-request-grant-checks">
            {(
              [
                { key: "isApproved", label: "Approved" },
                { key: "isSuspended", label: "Suspended" },
                { key: "isOverride", label: "Override" },
              ] as const
            )
              .filter(({ key }) => fields.has(key))
              .map(({ key, label }) => (
                <label key={key} htmlFor={`${id}-${key}`}>
                  <input
                    id={`${id}-${key}`}
                    type="checkbox"
                    checked={grant[key]}
                    onChange={(event) => onChange({ ...grant, [key]: event.target.checked })}
                  />
                  {label}
                </label>
              ))}
          </div>
        ) : null}
      </div>
    </details>
  );
}

function policyKey(policy: ApprovedPolicyCatalogueEntry): string {
  return `${policy.identity.versionId}:${policy.approvalEpoch}:${policy.recordId}`;
}

function policyIsCurrent(policy: ApprovedPolicyCatalogueEntry): boolean {
  const now = BigInt(Math.floor(Date.now() / 1000));
  return now >= BigInt(policy.validFromUnixSeconds) && now < BigInt(policy.validUntilUnixSeconds);
}
