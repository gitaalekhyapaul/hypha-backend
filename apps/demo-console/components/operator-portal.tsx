"use client";

import { useCallback, useEffect, useState } from "react";
import type { ActivePolicySelection } from "../lib/access-graph/types.ts";
import { capabilityAuthorizesPublishedDataset } from "../lib/agent-capability/readiness.ts";
import type {
  AgentSessionProjection,
  CapabilityJobProjection,
  PublishedAccessSelection,
} from "../lib/agent-capability/types.ts";
import {
  type BrowserOperatorSession,
  loginOperator,
  logoutOperator,
  operatorSession,
  PortalRequestError,
} from "../lib/portal-client/index.ts";
import { AccessGraphWorkspace } from "./access-graph-workspace.tsx";
import { AgentCapabilityWorkspace } from "./agent-capability-workspace.tsx";
import { DatasetWorkspace } from "./dataset-workspace.tsx";
import { LoginView } from "./login-view.tsx";
import { PolicyWorkspace } from "./policy-workspace.tsx";
import { ReconciliationWorkspace } from "./reconciliation-workspace.tsx";
import { WorkspaceOverview } from "./workspace-overview.tsx";

type PortalState = "checking" | "signed-out" | "signed-in";
type ActiveView = "access" | "capability" | "dataset" | "policy" | "reconciliation" | "workspace";

const navigation = [
  { id: "workspace", label: "Workspace", available: true },
  { id: "dataset", label: "Dataset", available: true },
  { id: "policy", label: "Policy", available: true },
  { id: "access", label: "Access graph", available: true },
  { id: "capability", label: "Capability", available: true },
  { id: "reconciliation", label: "Reconciliation", available: false },
  { id: "denials", label: "Denial lab", available: false },
  { id: "receipts", label: "Receipts", available: false },
] as const;

export function OperatorPortal() {
  const [state, setState] = useState<PortalState>("checking");
  const [session, setSession] = useState<BrowserOperatorSession | null>(null);
  const [activeView, setActiveView] = useState<ActiveView>("dataset");
  const [busy, setBusy] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [activePolicy, setActivePolicy] = useState<ActivePolicySelection | null>(null);
  const [agentSession, setAgentSession] = useState<AgentSessionProjection | null>(null);
  const [publishedAccess, setPublishedAccess] = useState<PublishedAccessSelection | null>(null);
  const [capabilityJob, setCapabilityJob] = useState<CapabilityJobProjection | null>(null);
  const reconciliationReady = capabilityAuthorizesPublishedDataset(capabilityJob, publishedAccess);

  useEffect(() => {
    operatorSession()
      .then((current) => {
        setSession(current);
        setState("signed-in");
      })
      .catch(() => setState("signed-out"));
  }, []);

  useEffect(() => {
    if (activeView === "reconciliation" && !reconciliationReady) {
      setActiveView("capability");
    }
  }, [activeView, reconciliationReady]);

  async function login(password: string) {
    setBusy(true);
    setLoginError(null);
    try {
      const current = await loginOperator(password);
      setSession(current);
      setState("signed-in");
    } catch (cause) {
      setLoginError(
        cause instanceof PortalRequestError && cause.status === 401
          ? "Invalid operator password"
          : "Operator service unavailable",
      );
    } finally {
      setBusy(false);
    }
  }

  async function logout() {
    if (!session) {
      return;
    }
    setBusy(true);
    setLogoutError(null);
    try {
      await logoutOperator(session.csrfToken);
      signOut();
    } catch {
      setLogoutError("Sign out failed. Your operator session is still active.");
    } finally {
      setBusy(false);
    }
  }

  const signOut = useCallback(() => {
    setActivePolicy(null);
    setAgentSession(null);
    setPublishedAccess(null);
    setCapabilityJob(null);
    setSession(null);
    setState("signed-out");
  }, []);

  if (state === "checking") {
    return (
      <main className="session-loading" aria-live="polite">
        <span className="brand-mark" aria-hidden="true">
          R
        </span>
        <p>Opening workspace...</p>
      </main>
    );
  }

  if (state === "signed-out" || !session) {
    return <LoginView busy={busy} error={loginError} onLogin={login} />;
  }

  return (
    <div className="portal-shell">
      <aside className="portal-sidebar">
        <header className="sidebar-brand">
          <span className="brand-mark" aria-hidden="true">
            R
          </span>
          <div>
            <strong>Raziel</strong>
            <span>Operator console</span>
          </div>
        </header>
        <nav aria-label="Scenario navigation">
          {navigation.map((item) => {
            const available = navigationAvailable(
              item.id,
              activePolicy,
              publishedAccess,
              reconciliationReady,
            );
            return (
              <button
                key={item.id}
                type="button"
                className="nav-item"
                data-active={item.id === activeView}
                disabled={!available}
                onClick={() => {
                  if (available && isActiveView(item.id)) {
                    setActiveView(item.id);
                  }
                }}
              >
                <span>{item.label}</span>
                {!available ? <small>Next</small> : null}
              </button>
            );
          })}
        </nav>
        <footer className="sidebar-footer">
          <span className="environment-dot" aria-hidden="true" />
          <div>
            <strong>Local development</strong>
            <span>Synthetic data</span>
          </div>
        </footer>
      </aside>

      <main className="portal-main">
        <header className="portal-topbar">
          <div>
            <span className="scenario-name">Payment reconciliation</span>
            <span className="scenario-version">M5</span>
          </div>
          <div className="operator-menu">
            <span>{session.operatorId}</span>
            <button type="button" disabled={busy} onClick={() => void logout()}>
              Sign out
            </button>
          </div>
        </header>

        <div className="portal-content">
          {logoutError ? (
            <p className="form-error portal-message" role="alert">
              {logoutError}
            </p>
          ) : null}
          {activeView === "workspace" ? <WorkspaceOverview /> : null}
          {activeView === "dataset" ? (
            <DatasetWorkspace csrfToken={session.csrfToken} onSessionExpired={signOut} />
          ) : null}
          {activeView === "policy" ? (
            <PolicyWorkspace
              csrfToken={session.csrfToken}
              onSessionExpired={signOut}
              onPolicyActivated={(policy) => {
                setActivePolicy(policy);
                setPublishedAccess(null);
                setCapabilityJob(null);
              }}
            />
          ) : null}
          {activeView === "access" && activePolicy ? (
            <AccessGraphWorkspace
              activePolicy={activePolicy}
              agentSession={agentSession}
              csrfToken={session.csrfToken}
              operatorId={session.operatorId}
              onAgentSessionCreated={setAgentSession}
              onSessionExpired={signOut}
              onSnapshotPublished={(selection) => {
                setPublishedAccess(selection);
                setCapabilityJob(null);
              }}
            />
          ) : null}
          {activeView === "capability" && activePolicy && publishedAccess ? (
            <AgentCapabilityWorkspace
              activePolicy={activePolicy}
              access={publishedAccess}
              csrfToken={session.csrfToken}
              job={capabilityJob}
              onJobChange={setCapabilityJob}
              onSessionExpired={signOut}
            />
          ) : null}
          {activeView === "reconciliation" && publishedAccess && capabilityJob ? (
            <ReconciliationWorkspace
              access={publishedAccess}
              csrfToken={session.csrfToken}
              job={capabilityJob}
              onSessionExpired={signOut}
            />
          ) : null}
        </div>
      </main>
    </div>
  );
}

function navigationAvailable(
  id: (typeof navigation)[number]["id"],
  activePolicy: ActivePolicySelection | null,
  publishedAccess: PublishedAccessSelection | null,
  reconciliationReady: boolean,
): boolean {
  if (id === "access") {
    return activePolicy !== null;
  }
  if (id === "capability") {
    return publishedAccess !== null;
  }
  if (id === "reconciliation") {
    return reconciliationReady;
  }
  return id === "workspace" || id === "dataset" || id === "policy";
}

function isActiveView(id: (typeof navigation)[number]["id"]): id is ActiveView {
  return id !== "denials" && id !== "receipts";
}
