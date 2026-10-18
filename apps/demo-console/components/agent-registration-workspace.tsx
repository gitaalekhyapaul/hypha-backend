"use client";

import { useEffect, useId, useRef, useState } from "react";
import {
  type AgentRegistration,
  type AgentTokenMetadata,
  type CreatedAgentToken,
  createAgentToken,
  DEFAULT_AGENT_TOKEN_DAYS,
  listAgentRegistrations,
  listAgentTokens,
  MAX_AGENT_TOKEN_DAYS,
  registerAgent,
  revokeAgentToken,
} from "../lib/portal-client/agents.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";
import { AgentRoleWorkspace } from "./agent-role-workspace.tsx";

interface AgentRegistrationWorkspaceProps {
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
}

export function AgentRegistrationWorkspace({
  csrfToken,
  onSessionExpired,
}: AgentRegistrationWorkspaceProps) {
  const [agents, setAgents] = useState<readonly AgentRegistration[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [registering, setRegistering] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const selected = agents.find((agent) => agent.agentId === selectedId);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void listAgentRegistrations(controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        setAgents(result.agents);
        setSelectedId((current) =>
          result.agents.some((agent) => agent.agentId === current)
            ? current
            : (result.agents[0]?.agentId ?? null),
        );
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted) {
          handleFailure(cause, onSessionExpired, setError, "Agent registrations unavailable.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [onSessionExpired, revision]);

  return (
    <section className="agent-registration-view" aria-labelledby="agent-registration-heading">
      <header className="view-header">
        <div>
          <p className="eyebrow">Operator administration</p>
          <h1 id="agent-registration-heading">Agents</h1>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={loading || registering}
            onClick={() => setRevision((current) => current + 1)}
          >
            {loading ? "Refreshing..." : "Refresh agents"}
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={registering || loading}
            onClick={() => setRegistering(true)}
          >
            Register agent
          </button>
        </div>
      </header>
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="agent-registration-layout">
        <aside
          className="agent-registration-list"
          aria-label="Registered agents"
          aria-busy={loading}
        >
          <div className="section-heading-row compact-heading">
            <h2>Registered agents</h2>
            <span className="section-count">{agents.length}</span>
          </div>
          {loading ? (
            <p className="empty-state" role="status">
              Loading agents...
            </p>
          ) : null}
          {!loading && !error && agents.length === 0 ? (
            <p className="empty-state">No registered agents.</p>
          ) : null}
          {agents.map((agent) => (
            <button
              key={agent.agentId}
              type="button"
              className="agent-registration-row"
              aria-pressed={!registering && selectedId === agent.agentId}
              onClick={() => {
                setSelectedId(agent.agentId);
                setRegistering(false);
              }}
            >
              <strong>{agent.displayName}</strong>
              <code>{agent.agentId}</code>
            </button>
          ))}
        </aside>
        <div className="agent-registration-detail">
          {registering ? (
            <RegistrationForm
              csrfToken={csrfToken}
              onSessionExpired={onSessionExpired}
              onCancel={() => setRegistering(false)}
              onRegistered={(agent) => {
                setAgents((current) => [
                  agent,
                  ...current.filter((item) => item.agentId !== agent.agentId),
                ]);
                setSelectedId(agent.agentId);
                setRegistering(false);
              }}
            />
          ) : selected ? (
            <RegisteredAgent
              key={selected.agentId}
              agent={selected}
              csrfToken={csrfToken}
              onSessionExpired={onSessionExpired}
            />
          ) : (
            <p className="empty-state">
              {loading
                ? "Loading registration..."
                : error
                  ? "Registration unavailable."
                  : "No agent selected."}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

function RegistrationForm({
  csrfToken,
  onSessionExpired,
  onCancel,
  onRegistered,
}: AgentRegistrationWorkspaceProps & {
  readonly onCancel: () => void;
  readonly onRegistered: (agent: AgentRegistration) => void;
}) {
  const id = useId();
  const [displayName, setDisplayName] = useState("");
  const [publicKey, setPublicKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useAlive();

  async function submit() {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const agent = await registerAgent(displayName, publicKey, csrfToken);
      if (alive.current) onRegistered(agent);
    } catch (cause) {
      if (alive.current) {
        handleFailure(
          cause,
          onSessionExpired,
          setError,
          "Registration not confirmed. Refresh the agent list before retrying.",
        );
      }
    } finally {
      if (alive.current) setBusy(false);
    }
  }

  return (
    <form
      className="agent-registration-form"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <h2>Register agent</h2>
      <label className="field-group" htmlFor={`${id}-name`}>
        <span>Display name</span>
        <input
          id={`${id}-name`}
          value={displayName}
          required
          maxLength={128}
          autoComplete="off"
          disabled={busy}
          onChange={(event) => setDisplayName(event.target.value)}
        />
      </label>
      <label className="field-group" htmlFor={`${id}-key`}>
        <span>Ed25519 public key (64 hexadecimal characters)</span>
        <textarea
          id={`${id}-key`}
          value={publicKey}
          required
          rows={3}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="none"
          disabled={busy}
          onChange={(event) => setPublicKey(event.target.value)}
        />
      </label>
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      <div className="header-actions">
        <button
          type="submit"
          className="primary-button"
          disabled={busy || !displayName.trim() || !publicKey.trim()}
        >
          {busy ? "Registering..." : "Register public key"}
        </button>
        <button type="button" className="secondary-button" disabled={busy} onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

function RegisteredAgent({
  agent,
  csrfToken,
  onSessionExpired,
}: AgentRegistrationWorkspaceProps & { readonly agent: AgentRegistration }) {
  const [tokens, setTokens] = useState<readonly AgentTokenMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [days, setDays] = useState(String(DEFAULT_AGENT_TOKEN_DAYS));
  const [mutation, setMutation] = useState<string | null>(null);
  const [confirmRevoke, setConfirmRevoke] = useState<string | null>(null);
  const [created, setCreated] = useState<CreatedAgentToken | null>(null);
  const [needsRefresh, setNeedsRefresh] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const alive = useAlive();
  const lifetimeId = useId();
  const validDays = /^\d+$/.test(days) && Number(days) >= 1 && Number(days) <= MAX_AGENT_TOKEN_DAYS;
  const blocked = loading || loadError !== null || mutation !== null || needsRefresh;

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setLoadError(null);
    void listAgentTokens(agent.agentId, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        setTokens(result.tokens);
        setNeedsRefresh(false);
      })
      .catch((cause: unknown) => {
        if (!controller.signal.aborted) {
          handleFailure(cause, onSessionExpired, setLoadError, "Token metadata unavailable.");
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [agent.agentId, onSessionExpired, revision]);

  async function create() {
    if (blocked || created || !validDays) return;
    setMutation("create");
    setError(null);
    setNotice(null);
    setConfirmRevoke(null);
    try {
      const result = await createAgentToken(agent.agentId, Number(days), csrfToken);
      if (!alive.current) return;
      setCreated(result);
      setTokens((current) => [result.metadata, ...current]);
    } catch (cause) {
      if (!alive.current) return;
      setNeedsRefresh(true);
      handleFailure(
        cause,
        onSessionExpired,
        setError,
        "Token creation not confirmed. Refresh metadata and revoke any token you cannot recover before creating another.",
      );
    } finally {
      if (alive.current) setMutation(null);
    }
  }

  async function revoke(tokenId: string) {
    if (blocked) return;
    setMutation(tokenId);
    setError(null);
    setNotice(null);
    try {
      const metadata = await revokeAgentToken(agent.agentId, tokenId, csrfToken);
      if (!alive.current) return;
      setTokens((current) =>
        current.map((token) => (token.tokenId === tokenId ? metadata : token)),
      );
      setCreated((current) => (current?.metadata.tokenId === tokenId ? null : current));
      setNotice("Token revoked.");
    } catch (cause) {
      if (!alive.current) return;
      setNeedsRefresh(true);
      handleFailure(
        cause,
        onSessionExpired,
        setError,
        "Revocation not confirmed. Refresh token metadata before retrying.",
      );
    } finally {
      if (alive.current) {
        setMutation(null);
        setConfirmRevoke(null);
      }
    }
  }

  return (
    <>
      <div className="section-heading-row">
        <h2>{agent.displayName}</h2>
        <span className="section-count">Ed25519</span>
      </div>
      <dl className="agent-registration-metadata">
        <div>
          <dt>Agent ID</dt>
          <dd>
            <code>{agent.agentId}</code>
          </dd>
        </div>
        <div>
          <dt>Public key (base64)</dt>
          <dd>
            <code>{agent.publicKey}</code>
          </dd>
        </div>
        <div>
          <dt>Key thumbprint (base64)</dt>
          <dd>
            <code>{agent.keyThumbprint}</code>
          </dd>
        </div>
        <div>
          <dt>Registered (UTC)</dt>
          <dd>{utcTime(agent.createdAtUnixSeconds)}</dd>
        </div>
      </dl>
      <section className="agent-registration-tokens" aria-labelledby={`${lifetimeId}-heading`}>
        <div className="section-heading-row">
          <h2 id={`${lifetimeId}-heading`}>API tokens</h2>
          <button
            type="button"
            className="secondary-button compact-button"
            disabled={loading || mutation !== null}
            onClick={() => {
              setConfirmRevoke(null);
              setRevision((current) => current + 1);
            }}
          >
            {loading ? "Refreshing..." : "Refresh tokens"}
          </button>
        </div>
        <form
          className="agent-registration-token-form"
          onSubmit={(event) => {
            event.preventDefault();
            void create();
          }}
        >
          <label className="field-group" htmlFor={lifetimeId}>
            <span>Lifetime (days, 1-90)</span>
            <input
              id={lifetimeId}
              type="number"
              min={1}
              max={MAX_AGENT_TOKEN_DAYS}
              step={1}
              required
              value={days}
              disabled={mutation !== null || created !== null}
              onChange={(event) => setDays(event.target.value)}
            />
          </label>
          <button
            type="submit"
            className="primary-button"
            disabled={blocked || created !== null || !validDays}
          >
            {mutation === "create" ? "Creating token..." : "Create token"}
          </button>
        </form>
        {created ? (
          <OneTimeToken
            key={created.metadata.tokenId}
            created={created}
            onDismiss={() => setCreated(null)}
          />
        ) : null}
        {loadError ? (
          <p className="form-error" role="alert">
            {loadError}
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
        {loading ? (
          <p className="empty-state" role="status">
            Loading token metadata...
          </p>
        ) : null}
        {!loading && !loadError && tokens.length === 0 ? (
          <p className="empty-state">No tokens issued.</p>
        ) : null}
        {tokens.length > 0 ? (
          <div className="agent-registration-table-wrap">
            <table className="agent-registration-token-table">
              <caption className="sr-only">Token metadata for {agent.displayName}</caption>
              <thead>
                <tr>
                  <th scope="col">Token ID</th>
                  <th scope="col">Status</th>
                  <th scope="col">Created (UTC)</th>
                  <th scope="col">Expires (UTC)</th>
                  <th scope="col">Revoked (UTC)</th>
                  <th scope="col">Action</th>
                </tr>
              </thead>
              <tbody>
                {tokens.map((token) => {
                  const status =
                    token.revokedAtUnixSeconds !== undefined
                      ? "Revoked"
                      : Number(token.expiresAtUnixSeconds) * 1_000 <= now
                        ? "Expired"
                        : "Active";
                  return (
                    <tr key={token.tokenId}>
                      <td>
                        <code>{token.tokenId}</code>
                      </td>
                      <td>
                        <span className="agent-registration-token-status" data-status={status}>
                          {status}
                        </span>
                      </td>
                      <td>{utcTime(token.createdAtUnixSeconds)}</td>
                      <td>{utcTime(token.expiresAtUnixSeconds)}</td>
                      <td>
                        {token.revokedAtUnixSeconds === undefined
                          ? "-"
                          : utcTime(token.revokedAtUnixSeconds)}
                      </td>
                      <td>
                        {status === "Revoked" ? (
                          <span className="agent-registration-muted">Revoked</span>
                        ) : confirmRevoke === token.tokenId ? (
                          <div className="agent-registration-revoke-confirm">
                            <span>Revoke this token?</span>
                            <button
                              type="button"
                              className="secondary-button compact-button agent-registration-danger"
                              disabled={blocked}
                              onClick={() => void revoke(token.tokenId)}
                            >
                              {mutation === token.tokenId ? "Revoking..." : "Confirm revoke"}
                            </button>
                            <button
                              type="button"
                              className="secondary-button compact-button"
                              disabled={mutation !== null}
                              onClick={() => setConfirmRevoke(null)}
                            >
                              Cancel
                            </button>
                          </div>
                        ) : (
                          <button
                            type="button"
                            className="secondary-button compact-button"
                            disabled={blocked}
                            onClick={() => setConfirmRevoke(token.tokenId)}
                            aria-label={`Revoke token ${token.tokenId}`}
                          >
                            Revoke
                          </button>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : null}
      </section>
      <AgentRoleWorkspace agent={agent} csrfToken={csrfToken} onSessionExpired={onSessionExpired} />
    </>
  );
}

function OneTimeToken({
  created,
  onDismiss,
}: {
  readonly created: CreatedAgentToken;
  readonly onDismiss: () => void;
}) {
  const [copyState, setCopyState] = useState<"idle" | "copying" | "copied" | "failed">("idle");
  const alive = useAlive();
  const id = useId();

  async function copy() {
    setCopyState("copying");
    try {
      await navigator.clipboard.writeText(created.token);
      if (alive.current) setCopyState("copied");
    } catch {
      if (alive.current) setCopyState("failed");
    }
  }

  return (
    <section className="agent-registration-secret" aria-labelledby={id}>
      <h3 id={id}>New token</h3>
      <p>Shown once. Dismissal or leaving this view removes the token.</p>
      <code className="agent-registration-secret-value">{created.token}</code>
      <div className="header-actions">
        <button
          type="button"
          className="secondary-button compact-button"
          disabled={copyState === "copying"}
          onClick={() => void copy()}
        >
          {copyState === "copying" ? "Copying..." : "Copy token"}
        </button>
        <button type="button" className="secondary-button compact-button" onClick={onDismiss}>
          Dismiss token
        </button>
      </div>
      {copyState === "copied" ? <p role="status">Token copied.</p> : null}
      {copyState === "failed" ? (
        <p className="form-error" role="alert">
          Clipboard unavailable. Select the token to copy it manually.
        </p>
      ) : null}
    </section>
  );
}

function useAlive() {
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  return alive;
}

function utcTime(seconds: string): string {
  return new Date(Number(seconds) * 1_000).toISOString().replace("T", " ").replace(".000Z", "");
}

function handleFailure(
  cause: unknown,
  onSessionExpired: () => void,
  setError: (message: string) => void,
  fallback: string,
) {
  if (cause instanceof PortalRequestError) {
    if (cause.status === 401) {
      onSessionExpired();
      return;
    }
    const messages: Readonly<Record<string, string>> = {
      INVALID_PUBLIC_KEY:
        "Enter a raw 32-byte Ed25519 public key as exactly 64 hexadecimal characters.",
      INVALID_DISPLAY_NAME: "Enter an agent display name.",
      INVALID_TOKEN_LIFETIME: "Token lifetime must be a whole number from 1 to 90 days.",
      INVALID_REQUEST: "The request was rejected. Check the supplied values.",
      CONFLICT: "This public key is already registered. Refresh the agent list.",
      AGENT_ALREADY_REGISTERED: "This public key is already registered. Refresh the agent list.",
      OPERATOR_CSRF_REJECTED: "Session security check failed. Sign out and sign in again.",
      OPERATOR_ORIGIN_REJECTED: "The operator request origin was rejected.",
    };
    setError(
      messages[cause.code] ??
        (cause.status === 403
          ? "Operator request rejected. Sign out and sign in again."
          : fallback),
    );
    return;
  }
  setError(fallback);
}
