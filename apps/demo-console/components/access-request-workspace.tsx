"use client";

import { useCallback, useEffect, useId, useRef, useState } from "react";
import {
  type AccessRequest,
  AccessRequestState,
  cancelAccessRequest,
  getAccessRequest,
  type ListAccessRequestsResponse,
  listAccessRequests,
  rejectAccessRequest,
} from "../lib/portal-client/access-requests.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";
import { AccessRequestApproval } from "./access-request-approval.tsx";
import "../app/access-requests.css";

interface AccessRequestWorkspaceProps {
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
}

const stateLabels: Record<AccessRequestState, string> = {
  [AccessRequestState.UNSPECIFIED]: "Unavailable",
  [AccessRequestState.PENDING_REVIEW]: "Pending review",
  [AccessRequestState.APPROVED_WAITING]: "Approved, waiting",
  [AccessRequestState.PREPARING]: "Preparing",
  [AccessRequestState.AWAITING_SIGNATURE]: "Awaiting signature",
  [AccessRequestState.WAITING_FOR_AGENT]: "Waiting for agent",
  [AccessRequestState.SUBMITTING]: "Submitting",
  [AccessRequestState.QUEUED]: "Queued",
  [AccessRequestState.EXECUTING]: "Executing",
  [AccessRequestState.PROVING]: "Proving",
  [AccessRequestState.CANCELLING]: "Cancellation in progress",
  [AccessRequestState.READY]: "Capability ready",
  [AccessRequestState.REJECTED]: "Rejected",
  [AccessRequestState.CANCELED]: "Canceled",
  [AccessRequestState.EXPIRED]: "Expired",
  [AccessRequestState.FAILED]: "Failed",
};

const terminalStates = new Set([
  AccessRequestState.READY,
  AccessRequestState.REJECTED,
  AccessRequestState.CANCELED,
  AccessRequestState.EXPIRED,
  AccessRequestState.FAILED,
]);

export function AccessRequestWorkspace({
  csrfToken,
  onSessionExpired,
}: AccessRequestWorkspaceProps) {
  const filterId = useId();
  const [page, setPage] = useState<ListAccessRequestsResponse | null>(null);
  const [cursors, setCursors] = useState<readonly (string | undefined)[]>([undefined]);
  const [agentInput, setAgentInput] = useState("");
  const [agentId, setAgentId] = useState<string | undefined>();
  const [filterError, setFilterError] = useState<string | null>(null);
  const [pendingOnly, setPendingOnly] = useState(true);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const cursor = cursors.at(-1);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void listAccessRequests(
      {
        limit: 25,
        ...(cursor === undefined ? {} : { cursor }),
        ...(agentId === undefined ? {} : { agentId }),
      },
      controller.signal,
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        setPage(result);
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        setPage(null);
        if (sessionExpired(cause, onSessionExpired)) return;
        setError("Access requests unavailable.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [cursor, agentId, revision, onSessionExpired]);

  const updateRequest = useCallback((request: AccessRequest) => {
    setSelectedKey(requestKey(request));
    setPage((current) =>
      current
        ? {
            ...current,
            requests: current.requests.map((item) =>
              requestKey(item) === requestKey(request) ? request : item,
            ),
          }
        : current,
    );
  }, []);

  const requests = [...(page?.requests ?? [])].sort(
    (a, b) =>
      Number(
        (b.updatedAtUnixSeconds > b.createdAtUnixSeconds
          ? b.updatedAtUnixSeconds
          : b.createdAtUnixSeconds) -
          (a.updatedAtUnixSeconds > a.createdAtUnixSeconds
            ? a.updatedAtUnixSeconds
            : a.createdAtUnixSeconds),
      ) ||
      Number(b.createdAtUnixSeconds - a.createdAtUnixSeconds) ||
      requestKey(b).localeCompare(requestKey(a)),
  );
  const visible = pendingOnly
    ? requests.filter((item) => item.state === AccessRequestState.PENDING_REVIEW)
    : requests;
  const selected = requests.find((item) => requestKey(item) === selectedKey) ?? visible[0];

  function changePage(next: readonly (string | undefined)[]) {
    setLoading(true);
    setSelectedKey(null);
    setCursors(next);
  }

  function refresh() {
    setLoading(true);
    setRevision((current) => current + 1);
  }

  function filter(next: string) {
    const value = next.trim();
    if (value && !/^[0-9a-f]{32}$/.test(value)) {
      setFilterError("Agent ID must contain 32 lowercase hexadecimal characters.");
      return;
    }
    setFilterError(null);
    setAgentInput(value);
    setAgentId(value || undefined);
    changePage([undefined]);
    setRevision((current) => current + 1);
  }

  return (
    <section className="access-request-view" aria-labelledby="access-request-heading">
      <header className="view-header">
        <div>
          <p className="eyebrow">Operator review</p>
          <h1 id="access-request-heading">Access requests</h1>
        </div>
        <button
          type="button"
          className="secondary-button access-request-icon"
          aria-label="Refresh requests"
          title="Refresh requests"
          disabled={loading}
          onClick={refresh}
        >
          <span aria-hidden="true">&#8635;</span>
        </button>
      </header>
      <p className="access-request-boundary" role="note">
        No request status grants data access. Protected queries require a verified capability.
      </p>
      <div className="access-request-toolbar">
        <fieldset className="access-request-views" aria-label="Request view">
          {[true, false].map((pending) => (
            <button
              key={String(pending)}
              type="button"
              aria-pressed={pendingOnly === pending}
              onClick={() => {
                setPendingOnly(pending);
                setSelectedKey(null);
              }}
            >
              {pending ? "Pending review" : "All requests"}
            </button>
          ))}
        </fieldset>
        <form
          className="access-request-filter"
          onSubmit={(event) => {
            event.preventDefault();
            filter(agentInput);
          }}
        >
          <label htmlFor={filterId}>Agent ID</label>
          <input
            id={filterId}
            value={agentInput}
            onChange={(event) => setAgentInput(event.target.value)}
            aria-invalid={Boolean(filterError)}
            aria-describedby={filterError ? `${filterId}-error` : undefined}
            autoComplete="off"
            autoCapitalize="none"
            spellCheck={false}
            maxLength={32}
          />
          <button type="submit" className="secondary-button" disabled={loading}>
            Filter
          </button>
          {agentId ? (
            <button
              type="button"
              className="secondary-button"
              disabled={loading}
              onClick={() => filter("")}
            >
              Clear
            </button>
          ) : null}
        </form>
      </div>
      {filterError ? (
        <p id={`${filterId}-error`} className="form-error" role="alert">
          {filterError}
        </p>
      ) : null}
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="access-request-layout">
        <aside className="access-request-inbox" aria-label="Request inbox" aria-busy={loading}>
          <div className="access-request-list-heading">
            <h2>{pendingOnly ? "Pending on this page" : "Requests on this page"}</h2>
            <span className="section-count">{loading || error ? "-" : visible.length}</span>
          </div>
          {loading ? (
            <p className="empty-state" role="status">
              Loading requests...
            </p>
          ) : null}
          {!loading && !error && visible.length === 0 ? (
            <p className="empty-state">
              {pendingOnly ? "No pending requests on this page." : "No access requests."}
            </p>
          ) : null}
          {!loading && !error
            ? visible.map((item) => (
                <button
                  key={requestKey(item)}
                  type="button"
                  className="access-request-row"
                  aria-pressed={selected !== undefined && requestKey(selected) === requestKey(item)}
                  onClick={() => setSelectedKey(requestKey(item))}
                >
                  <span className="access-request-row-heading">
                    <strong>{item.taskId}</strong>
                    <RequestStatus state={item.state} />
                  </span>
                  <span>
                    {item.datasetId} / revision {String(item.datasetRevision)}
                  </span>
                  <code>{item.agentId}</code>
                  <span className="access-request-row-time">
                    {formatTime(
                      item.updatedAtUnixSeconds > item.createdAtUnixSeconds
                        ? item.updatedAtUnixSeconds
                        : item.createdAtUnixSeconds,
                    )}
                  </span>
                </button>
              ))
            : null}
          <nav className="access-request-pagination" aria-label="Request pages">
            <button
              type="button"
              className="secondary-button access-request-icon"
              title="Previous page"
              aria-label="Previous page"
              disabled={loading || cursors.length === 1}
              onClick={() => changePage(cursors.slice(0, -1))}
            >
              <span aria-hidden="true">&#8592;</span>
            </button>
            <span>Page {cursors.length}</span>
            <button
              type="button"
              className="secondary-button access-request-icon"
              title="Next page"
              aria-label="Next page"
              disabled={loading || Boolean(error) || !page?.nextCursor}
              onClick={() => changePage([...cursors, page?.nextCursor])}
            >
              <span aria-hidden="true">&#8594;</span>
            </button>
          </nav>
        </aside>
        <div className="access-request-detail">
          {!loading && !error && selected ? (
            <RequestDetail
              key={requestKey(selected)}
              agentId={selected.agentId}
              requestId={selected.requestId}
              csrfToken={csrfToken}
              onSessionExpired={onSessionExpired}
              onUpdated={updateRequest}
            />
          ) : (
            <p className="empty-state">
              {loading
                ? "Loading request..."
                : error
                  ? "Request unavailable."
                  : "No request selected."}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

function RequestDetail({
  agentId,
  requestId,
  csrfToken,
  onSessionExpired,
  onUpdated,
}: AccessRequestWorkspaceProps & {
  readonly agentId: string;
  readonly requestId: string;
  readonly onUpdated: (request: AccessRequest) => void;
}) {
  const [request, setRequest] = useState<AccessRequest | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<"reject" | "cancel" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const alive = useRef(false);
  const mutationPending = useRef(false);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setRequest(null);
    setError(null);
    setConfirm(null);
    void getAccessRequest(agentId, requestId, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        setRequest(result);
        onUpdated(result);
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted || sessionExpired(cause, onSessionExpired)) return;
        setError(
          cause instanceof PortalRequestError && cause.status === 404
            ? "Request no longer available."
            : "Request details unavailable.",
        );
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [agentId, requestId, revision, onSessionExpired, onUpdated]);

  async function mutate() {
    if (!request || !confirm || mutationPending.current) return;
    mutationPending.current = true;
    setBusy(true);
    setError(null);
    setNotice(null);
    const action = confirm;
    setConfirm(null);
    try {
      const result = await (action === "reject" ? rejectAccessRequest : cancelAccessRequest)(
        agentId,
        requestId,
        request.generation,
        csrfToken,
      );
      if (!alive.current) return;
      setRequest(result);
      onUpdated(result);
      setNotice(`Current status: ${stateLabels[result.state]}.`);
    } catch (cause) {
      if (!alive.current || sessionExpired(cause, onSessionExpired)) return;
      const conflict = cause instanceof PortalRequestError && cause.status === 409;
      setRequest(null);
      try {
        const latest = await getAccessRequest(agentId, requestId);
        if (!alive.current) return;
        setRequest(latest);
        onUpdated(latest);
        if (conflict) setNotice("Request changed. Latest status loaded.");
        else
          setError(
            cause instanceof PortalRequestError && cause.status === 403
              ? "Operator action denied. Latest status loaded."
              : "Action not confirmed. Latest status loaded.",
          );
      } catch (refreshCause) {
        if (!alive.current || sessionExpired(refreshCause, onSessionExpired)) return;
        setError(
          conflict
            ? "Request changed. Latest status unavailable."
            : "Action not confirmed. Latest status unavailable.",
        );
      }
    } finally {
      mutationPending.current = false;
      if (alive.current) setBusy(false);
    }
  }

  const canCancel =
    request &&
    !terminalStates.has(request.state) &&
    request.state !== AccessRequestState.CANCELLING;

  return (
    <section aria-label="Request details" aria-busy={loading || busy}>
      <div className="access-request-detail-heading">
        <h2>Request details</h2>
        <button
          type="button"
          className="secondary-button access-request-icon"
          aria-label="Refresh request details"
          title="Refresh request details"
          disabled={loading || busy}
          onClick={() => {
            setNotice(null);
            setRevision((current) => current + 1);
          }}
        >
          <span aria-hidden="true">&#8635;</span>
        </button>
      </div>
      {loading ? (
        <p className="empty-state" role="status">
          Loading request details...
        </p>
      ) : null}
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
      {busy ? (
        <p className="access-request-progress" role="status">
          Updating request...
        </p>
      ) : null}
      {request ? (
        <>
          <div className="access-request-summary">
            <h3>{request.taskId}</h3>
            <RequestStatus state={request.state} />
          </div>
          {request.state === AccessRequestState.APPROVED_WAITING ? (
            <p className="access-request-progress" role="status">
              Approval recorded. Waiting for preparation.
            </p>
          ) : null}
          <dl className="access-request-metadata">
            <div>
              <dt>Purpose</dt>
              <dd>{request.purpose}</dd>
            </div>
            <div>
              <dt>Action</dt>
              <dd>{request.action}</dd>
            </div>
            <div>
              <dt>Dataset</dt>
              <dd>{request.datasetId}</dd>
            </div>
            <div>
              <dt>Dataset revision</dt>
              <dd>{String(request.datasetRevision)}</dd>
            </div>
            <div>
              <dt>Agent ID</dt>
              <dd>
                <code>{request.agentId}</code>
              </dd>
            </div>
            <div>
              <dt>Request ID</dt>
              <dd>
                <code>{request.requestId}</code>
              </dd>
            </div>
            <div>
              <dt>Generation</dt>
              <dd>{String(request.generation)}</dd>
            </div>
            <div>
              <dt>Created (UTC)</dt>
              <dd>
                <RequestTime value={request.createdAtUnixSeconds} />
              </dd>
            </div>
            <div>
              <dt>Updated (UTC)</dt>
              <dd>
                <RequestTime value={request.updatedAtUnixSeconds} />
              </dd>
            </div>
            <div>
              <dt>Deadline (UTC)</dt>
              <dd>
                <RequestTime value={request.deadlineUnixSeconds} />
              </dd>
            </div>
            {request.policyBinding ? (
              <>
                <div>
                  <dt>Policy record</dt>
                  <dd>
                    <code>{request.policyBinding.policyRecordId}</code>
                  </dd>
                </div>
                <div>
                  <dt>Policy hash</dt>
                  <dd>
                    <code>
                      {Array.from(request.policyBinding.policyHash, (byte) =>
                        byte.toString(16).padStart(2, "0"),
                      ).join("")}
                    </code>
                  </dd>
                </div>
                <div>
                  <dt>Resource type</dt>
                  <dd>
                    <code>{request.policyBinding.resourceType}</code>
                  </dd>
                </div>
                <div>
                  <dt>Resource scope</dt>
                  <dd>
                    <code>{request.policyBinding.resourceScope}</code>
                  </dd>
                </div>
              </>
            ) : null}
            {request.proofJobId ? (
              <div>
                <dt>Proof job</dt>
                <dd>
                  <code>{request.proofJobId}</code>
                </dd>
              </div>
            ) : null}
          </dl>
          {request.state === AccessRequestState.PENDING_REVIEW ? (
            <AccessRequestApproval
              request={request}
              csrfToken={csrfToken}
              disabled={busy || loading || confirm !== null}
              onBusyChange={setBusy}
              onSessionExpired={onSessionExpired}
              onUpdated={(result) => {
                setRequest(result);
                onUpdated(result);
              }}
            />
          ) : null}
          {confirm ? (
            <fieldset className="access-request-confirm" aria-label="Confirm request action">
              <strong>
                {confirm === "reject" ? "Reject this request?" : "Cancel this request?"}
              </strong>
              <button type="button" className="secondary-button" onClick={() => setConfirm(null)}>
                Back
              </button>
              <button
                type="button"
                className="secondary-button access-request-danger"
                onClick={() => void mutate()}
              >
                {confirm === "reject" ? "Confirm reject" : "Confirm cancellation"}
              </button>
            </fieldset>
          ) : (
            <div className="access-request-actions">
              {request.state === AccessRequestState.PENDING_REVIEW ? (
                <button
                  type="button"
                  className="secondary-button access-request-danger"
                  disabled={busy || loading}
                  onClick={() => setConfirm("reject")}
                >
                  Reject request
                </button>
              ) : null}
              {canCancel ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busy || loading}
                  onClick={() => setConfirm("cancel")}
                >
                  Cancel request
                </button>
              ) : null}
            </div>
          )}
        </>
      ) : null}
    </section>
  );
}

function RequestStatus({ state }: { readonly state: AccessRequestState }) {
  const tone =
    state === AccessRequestState.PENDING_REVIEW
      ? "pending"
      : state === AccessRequestState.READY
        ? "ready"
        : terminalStates.has(state)
          ? "closed"
          : "active";
  return (
    <span className="access-request-status" data-tone={tone}>
      {stateLabels[state]}
    </span>
  );
}

function RequestTime({ value }: { readonly value: bigint }) {
  return <time dateTime={new Date(Number(value) * 1000).toISOString()}>{formatTime(value)}</time>;
}

function formatTime(value: bigint): string {
  return `${new Date(Number(value) * 1000).toISOString().replace("T", " ").slice(0, -5)} UTC`;
}

function requestKey(request: AccessRequest): string {
  return `${request.agentId}/${request.requestId}`;
}

function sessionExpired(cause: unknown, onSessionExpired: () => void): boolean {
  if (cause instanceof PortalRequestError && cause.status === 401) {
    onSessionExpired();
    return true;
  }
  return false;
}
