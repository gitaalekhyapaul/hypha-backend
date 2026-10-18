"use client";

import { useEffect, useId, useRef, useState } from "react";
import type { SyntheticDataset } from "../lib/dataset-client/types.ts";
import type { ApprovedPolicyCatalogueEntry } from "../lib/policy-version/catalogue-types.ts";
import {
  type AgentRole,
  type AgentRoleAssignment,
  type AgentRoleChange,
  type AgentRoleScope,
  type AgentRoles,
  getAgentRoles,
  isAgentRoleId,
  updateAgentRoles,
} from "../lib/portal-client/agent-roles.ts";
import type { AgentRegistration } from "../lib/portal-client/agents.ts";
import { listDatasets, PortalRequestError } from "../lib/portal-client/browser.ts";
import { getApprovedPolicyCatalogue } from "../lib/portal-client/policy-catalogue.ts";

interface AgentRoleWorkspaceProps {
  readonly agent: AgentRegistration;
  readonly csrfToken: string;
  readonly onSessionExpired: () => void;
}

export function AgentRoleWorkspace(props: AgentRoleWorkspaceProps) {
  const id = useId();
  const [policies, setPolicies] = useState<readonly ApprovedPolicyCatalogueEntry[]>([]);
  const [selection, setSelection] = useState("");
  const [datasets, setDatasets] = useState<readonly SyntheticDataset[]>([]);
  const [datasetInput, setDatasetInput] = useState("");
  const [datasetId, setDatasetId] = useState("");
  const [datasetsLoading, setDatasetsLoading] = useState(true);
  const [datasetsError, setDatasetsError] = useState<string | null>(null);
  const [datasetsRevision, setDatasetsRevision] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [busy, setBusy] = useState(false);
  const selected = policies.find((policy) => policyKey(policy) === selection);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setError(null);
    void getApprovedPolicyCatalogue(controller.signal)
      .then((catalogue) => {
        if (controller.signal.aborted) return;
        setPolicies(catalogue.policies);
        setSelection((current) =>
          catalogue.policies.some((policy) => policyKey(policy) === current)
            ? current
            : catalogue.policies[0]
              ? policyKey(catalogue.policies[0])
              : "",
        );
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        setPolicies([]);
        if (cause instanceof PortalRequestError && cause.status === 401) props.onSessionExpired();
        else setError("Approved policies unavailable. Refresh policies to retry.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [props.onSessionExpired, revision]);

  useEffect(() => {
    let active = true;
    setDatasetsLoading(true);
    setDatasetsError(null);
    void listDatasets()
      .then((result) => {
        if (!active) return;
        setDatasets(result.datasets);
        setDatasetInput((current) =>
          result.datasets.some((dataset) => dataset.datasetId === current)
            ? current
            : (result.datasets[0]?.datasetId ?? ""),
        );
        setDatasetId((current) =>
          result.datasets.some((dataset) => dataset.datasetId === current)
            ? current
            : current
              ? ""
              : (result.datasets[0]?.datasetId ?? ""),
        );
      })
      .catch((cause: unknown) => {
        if (!active) return;
        if (cause instanceof PortalRequestError && cause.status === 401) props.onSessionExpired();
        else setDatasetsError("Datasets unavailable. Refresh datasets to retry.");
      })
      .finally(() => {
        if (active) setDatasetsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [props.onSessionExpired, datasetsRevision]);

  return (
    <section className="agent-registration-tokens" aria-labelledby={`${id}-heading`}>
      <div className="section-heading-row">
        <h2 id={`${id}-heading`}>Roles and teams</h2>
        <button
          type="button"
          className="secondary-button compact-button"
          disabled={loading || busy}
          onClick={() => setRevision((current) => current + 1)}
        >
          {loading ? "Refreshing policies..." : "Refresh policies"}
        </button>
      </div>
      <p className="agent-registration-muted">
        Changes apply to fresh access requests. Existing issued capabilities remain unchanged.
      </p>
      {loading ? <p role="status">Loading approved policies...</p> : null}
      {error ? (
        <p className="form-error" role="alert">
          {error}
        </p>
      ) : null}
      {!loading && !error && policies.length === 0 ? (
        <p className="empty-state">No installed approved policy versions.</p>
      ) : null}
      {!loading && !error && policies.length > 0 ? (
        <>
          <form
            className="agent-registration-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (datasets.some((dataset) => dataset.datasetId === datasetInput))
                setDatasetId(datasetInput);
            }}
          >
            <label className="policy-field" htmlFor={`${id}-policy`}>
              <span>Installed policy version</span>
              <select
                id={`${id}-policy`}
                value={selection}
                disabled={busy}
                onChange={(event) => setSelection(event.target.value)}
              >
                {policies.map((policy) => (
                  <option key={policyKey(policy)} value={policyKey(policy)}>
                    {policy.identity.versionId} / v{policy.identity.version} / epoch{" "}
                    {policy.approvalEpoch}
                  </option>
                ))}
              </select>
            </label>
            <div className="agent-registration-token-form">
              <label
                className="policy-field"
                htmlFor={`${id}-dataset`}
                style={{ flex: "1 1 320px", minWidth: 0 }}
              >
                <span>Dataset ID</span>
                <select
                  id={`${id}-dataset`}
                  aria-label="Dataset ID"
                  value={datasetInput}
                  title={datasetInput}
                  style={{ minWidth: 0 }}
                  required
                  disabled={
                    busy || datasetsLoading || datasetsError !== null || datasets.length === 0
                  }
                  onChange={(event) => setDatasetInput(event.target.value)}
                >
                  <option value="" disabled>
                    {datasetsLoading ? "Loading datasets..." : "Select a dataset"}
                  </option>
                  {datasets.map((dataset) => (
                    <option key={dataset.datasetId} value={dataset.datasetId}>
                      {dataset.datasetId} / revision {dataset.revision} /{" "}
                      {dataset.status === "FINALIZED" ? "Finalized" : "Draft"}
                    </option>
                  ))}
                </select>
              </label>
              <button
                type="submit"
                className="secondary-button"
                disabled={
                  busy ||
                  datasetsLoading ||
                  datasetsError !== null ||
                  !datasets.some((dataset) => dataset.datasetId === datasetInput)
                }
              >
                View roles
              </button>
              <button
                type="button"
                className="secondary-button"
                disabled={busy || datasetsLoading}
                onClick={() => setDatasetsRevision((current) => current + 1)}
              >
                {datasetsLoading ? "Refreshing datasets..." : "Refresh datasets"}
              </button>
            </div>
            {datasetsLoading ? <p role="status">Loading saved datasets...</p> : null}
            {datasetsError ? (
              <p className="form-error" role="alert">
                {datasetsError}
              </p>
            ) : null}
            {!datasetsLoading && !datasetsError && datasets.length === 0 ? (
              <p className="empty-state">No saved datasets.</p>
            ) : null}
          </form>
          {selected &&
          !datasetsLoading &&
          !datasetsError &&
          datasets.some((dataset) => dataset.datasetId === datasetId) &&
          datasetInput === datasetId ? (
            <RoleEditor
              key={`${props.agent.agentId}:${selection}:${datasetId}:${revision}`}
              {...props}
              scope={{
                policyVersionId: selected.identity.versionId,
                approvalEpoch: selected.approvalEpoch,
                datasetId,
              }}
              onBusyChange={setBusy}
            />
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function RoleEditor({
  agent,
  csrfToken,
  onSessionExpired,
  scope,
  onBusyChange,
}: AgentRoleWorkspaceProps & {
  readonly scope: AgentRoleScope;
  readonly onBusyChange: (busy: boolean) => void;
}) {
  const id = useId();
  const [state, setState] = useState<AgentRoles | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [role, setRole] = useState<AgentRole | "">("");
  const [teamId, setTeamId] = useState("");
  const [mutation, setMutation] = useState<string | null>(null);
  const [confirmation, setConfirmation] = useState<string | null>(null);
  const alive = useRef(false);
  const pending = useRef(false);
  const selectedRole = state?.availableRoles.includes(role as AgentRole)
    ? (role as AgentRole)
    : state?.availableRoles[0];
  const blocked = loading || loadError !== null || mutation !== null;
  const { policyVersionId, approvalEpoch, datasetId } = scope;
  const thumbprint = Array.from(atob(agent.keyThumbprint), (byte) =>
    byte.charCodeAt(0).toString(16).padStart(2, "0"),
  ).join("");

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    setLoading(true);
    setLoadError(null);
    setConfirmation(null);
    void getAgentRoles(
      agent.agentId,
      { policyVersionId, approvalEpoch, datasetId },
      controller.signal,
    )
      .then((result) => {
        if (controller.signal.aborted) return;
        if (result.agentKeyThumbprint !== thumbprint)
          throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
        setState(result);
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return;
        if (cause instanceof PortalRequestError && cause.status === 401) onSessionExpired();
        else setLoadError("Roles unavailable. Refresh roles to retry.");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [
    agent.agentId,
    policyVersionId,
    approvalEpoch,
    datasetId,
    thumbprint,
    revision,
    onSessionExpired,
  ]);

  async function mutate(key: string, change: AgentRoleChange) {
    if (pending.current || blocked || !state) return;
    if (change.operation === "enable" ? !state.canEnable || state.managed : !state.managed) return;
    pending.current = true;
    setMutation(key);
    onBusyChange(true);
    setFailure(null);
    setNotice(null);
    try {
      let succeeded = false;
      try {
        const result = await updateAgentRoles(agent.agentId, scope, change, csrfToken);
        if (!alive.current) return;
        if (result.agentKeyThumbprint !== thumbprint)
          throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
        setState(result);
        succeeded = true;
      } catch (cause) {
        if (!alive.current) return;
        if (cause instanceof PortalRequestError && cause.status === 401) {
          onSessionExpired();
          return;
        }
        setFailure({ key, message: mutationMessage(cause) });
      }
      if (alive.current) {
        // A lost mutation response is not evidence that the write failed.
        try {
          const latest = await getAgentRoles(agent.agentId, scope);
          if (alive.current) {
            if (latest.agentKeyThumbprint !== thumbprint)
              throw new PortalRequestError("PORTAL_UNAVAILABLE", 503);
            setState(latest);
            setLoadError(null);
            if (succeeded)
              setNotice(
                change.operation === "enable"
                  ? "Operator-managed roles activated."
                  : "Role assignments updated.",
              );
          }
        } catch (cause) {
          if (alive.current) {
            if (cause instanceof PortalRequestError && cause.status === 401) onSessionExpired();
            else setLoadError("Latest roles not confirmed. Refresh roles before another change.");
          }
        }
      }
    } finally {
      if (alive.current) {
        setMutation(null);
        setConfirmation(null);
        onBusyChange(false);
      }
      pending.current = false;
    }
  }

  function rowError(key: string) {
    return failure?.key === key ? (
      <p className="form-error" role="alert">
        {failure.message}
      </p>
    ) : null;
  }

  return (
    <div className="agent-registration-form">
      <div className="section-heading-row">
        <h3>Assignments</h3>
        <button
          type="button"
          className="secondary-button compact-button"
          disabled={loading || mutation !== null}
          onClick={() => {
            setFailure(null);
            setNotice(null);
            setRevision((current) => current + 1);
          }}
        >
          {loading ? "Refreshing roles..." : "Refresh roles"}
        </button>
      </div>
      {loading ? <p role="status">Loading role assignments...</p> : null}
      {loadError ? (
        <p className="form-error" role="alert">
          {loadError}
        </p>
      ) : null}
      {notice ? (
        <p className="form-notice" role="status">
          {notice}
        </p>
      ) : null}
      {failure &&
      state &&
      ((failure.key === "enable" && state.managed) ||
        (failure.key !== "enable" &&
          failure.key !== "add" &&
          !state.assignments.some((assignment) => assignmentKey(assignment) === failure.key))) ? (
        <p className="form-error" role="alert">
          {failure.message}
        </p>
      ) : null}
      {state && !loading ? (
        <>
          <p>
            <strong>
              {state.managed ? "Operator-managed roles" : "Legacy automatic provisioning"}
            </strong>
          </p>
          {state.managed ? (
            <p className="agent-registration-muted">
              Operator-managed mode is permanent for this policy version.
            </p>
          ) : null}
          {!state.managed && state.canEnable && state.availableRoles.length > 0 ? (
            <div aria-busy={mutation === "enable"}>
              <p>
                This choice applies to the entire policy version and cannot be undone. Automatic
                role provisioning stops.
              </p>
              <div className="header-actions">
                <button
                  type="button"
                  className="primary-button"
                  disabled={blocked}
                  onClick={() =>
                    confirmation === "enable"
                      ? void mutate("enable", { operation: "enable" })
                      : setConfirmation("enable")
                  }
                >
                  {mutation === "enable"
                    ? "Activating..."
                    : confirmation === "enable"
                      ? "Confirm operator-managed roles"
                      : "Use operator-managed roles"}
                </button>
                {confirmation === "enable" ? (
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={mutation !== null}
                    onClick={() => setConfirmation(null)}
                  >
                    Cancel
                  </button>
                ) : null}
              </div>
              {rowError("enable")}
            </div>
          ) : null}
          {state.availableRoles.length === 0 ? (
            <p className="empty-state">
              Role editing is unavailable for this policy family. Caveat and organization policies
              are not supported.
            </p>
          ) : !state.managed && !state.canEnable ? (
            <p className="agent-registration-muted">
              This version cannot switch to operator-managed roles. Existing relationships retain
              legacy automatic provisioning. Publish a fresh approved version with no relationships
              to switch.
            </p>
          ) : null}
          {state.availableRoles.length > 0 ? (
            <form
              className="agent-registration-form"
              aria-busy={mutation === "add"}
              onSubmit={(event) => {
                event.preventDefault();
                if (selectedRole)
                  void mutate("add", {
                    operation: "add",
                    role: selectedRole,
                    ...(selectedRole === "team_member" ? { teamId } : {}),
                  });
              }}
            >
              <div className="agent-registration-token-form">
                <label className="policy-field" htmlFor={`${id}-role`}>
                  <span>Role</span>
                  <select
                    id={`${id}-role`}
                    aria-label="Role"
                    value={selectedRole ?? ""}
                    disabled={blocked || !state.managed}
                    onChange={(event) => {
                      setRole(event.target.value as AgentRole);
                      setFailure(null);
                    }}
                  >
                    {state.availableRoles.map((option) => (
                      <option key={option} value={option}>
                        {roleLabel(option)}
                      </option>
                    ))}
                  </select>
                </label>
                {selectedRole === "team_member" ? (
                  <label className="field-group" htmlFor={`${id}-team`}>
                    <span>Team ID</span>
                    <input
                      id={`${id}-team`}
                      value={teamId}
                      required
                      maxLength={128}
                      autoComplete="off"
                      spellCheck={false}
                      disabled={blocked || !state.managed}
                      onChange={(event) => setTeamId(event.target.value)}
                    />
                  </label>
                ) : null}
                <button
                  type="submit"
                  className="primary-button"
                  disabled={
                    blocked ||
                    !state.managed ||
                    !selectedRole ||
                    (selectedRole === "team_member" && !isAgentRoleId(teamId)) ||
                    state.assignments.some(
                      (item) =>
                        item.role === selectedRole &&
                        item.teamId === (selectedRole === "team_member" ? teamId : undefined),
                    )
                  }
                >
                  {mutation === "add" ? "Adding..." : "Add role"}
                </button>
              </div>
              {rowError("add")}
            </form>
          ) : null}
          {state.assignments.length === 0 ? (
            <p className="empty-state">No role assignments for this dataset.</p>
          ) : (
            <div className="agent-registration-table-wrap">
              <table className="agent-registration-token-table" style={{ minWidth: 0 }}>
                <caption className="sr-only">
                  Role assignments for {agent.displayName} on {datasetId}
                </caption>
                <thead>
                  <tr>
                    <th scope="col">Role</th>
                    <th scope="col">Team</th>
                    <th scope="col">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {state.assignments.map((assignment) => {
                    const key = assignmentKey(assignment);
                    return (
                      <tr key={key} aria-busy={mutation === key}>
                        <td>{roleLabel(assignment.role)}</td>
                        <td>{assignment.teamId ? <code>{assignment.teamId}</code> : "-"}</td>
                        <td>
                          {confirmation === key ? (
                            <div className="agent-registration-revoke-confirm">
                              <span>
                                {assignment.role === "team_member"
                                  ? "Remove this agent from this team across all datasets in this policy version?"
                                  : "Remove this role?"}
                              </span>
                              <button
                                type="button"
                                className="secondary-button compact-button agent-registration-danger"
                                disabled={blocked || !state.managed}
                                onClick={() =>
                                  void mutate(key, { operation: "remove", ...assignment })
                                }
                              >
                                {mutation === key ? "Removing..." : "Confirm removal"}
                              </button>
                              <button
                                type="button"
                                className="secondary-button compact-button"
                                disabled={mutation !== null}
                                onClick={() => setConfirmation(null)}
                              >
                                Cancel
                              </button>
                            </div>
                          ) : (
                            <button
                              type="button"
                              className="secondary-button compact-button"
                              disabled={blocked || !state.managed}
                              aria-label={`Remove ${roleLabel(assignment.role)}${assignment.teamId ? ` ${assignment.teamId}` : ""}`}
                              onClick={() => {
                                setConfirmation(key);
                                setFailure(null);
                              }}
                            >
                              Remove
                            </button>
                          )}
                          {rowError(key)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
          {state.availableRoles.includes("team_member") ? (
            <p className="agent-registration-muted">
              Removing a team membership removes this agent from that team across all datasets in
              this policy version.
            </p>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function policyKey(policy: ApprovedPolicyCatalogueEntry): string {
  return `${policy.identity.versionId}:${policy.approvalEpoch}`;
}

function assignmentKey(assignment: AgentRoleAssignment): string {
  return `${assignment.role}:${assignment.teamId ?? ""}`;
}

function roleLabel(role: AgentRole): string {
  return role === "team_member" ? "Team member" : role.charAt(0).toUpperCase() + role.slice(1);
}

function mutationMessage(cause: unknown): string {
  if (cause instanceof PortalRequestError && cause.status === 409)
    return "The policy or role state changed. Review the refreshed assignments before retrying.";
  if (cause instanceof PortalRequestError && cause.status === 403)
    return "Role change rejected. Refresh the operator session before retrying.";
  return "Role change not confirmed. Review the refreshed assignments before retrying.";
}
