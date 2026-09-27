import type { PolicyWorkflow } from "../lib/policy-workflow.ts";

export function WorkspaceOverview({ workflow = "legacy" }: { readonly workflow?: PolicyWorkflow }) {
  const versioned = workflow === "versioned";
  return (
    <section className="workspace-view" aria-labelledby="workspace-heading">
      <header className="view-header">
        <div>
          <p className="eyebrow">Payment reconciliation</p>
          <h1 id="workspace-heading">Workspace</h1>
        </div>
        <span className="classification-flag">Synthetic</span>
      </header>

      <dl className="workspace-metrics">
        <div>
          <dt>Policy mode</dt>
          <dd>{versioned ? "Immutable versions" : "Visual rules"}</dd>
        </div>
        <div>
          <dt>{versioned ? "Agent proof dispatch" : "Proof mode"}</dt>
          <dd>{versioned ? "Pending" : "Local Plonk"}</dd>
        </div>
        <div>
          <dt>Verifier</dt>
          <dd>Independent</dd>
        </div>
        <div>
          <dt>Data class</dt>
          <dd>Synthetic</dd>
        </div>
      </dl>

      <section className="workflow-band" aria-labelledby="workflow-heading">
        <div className="section-heading-row">
          <div>
            <p className="eyebrow">Scenario state</p>
            <h2 id="workflow-heading">Workflow</h2>
          </div>
          <span className="section-count">{versioned ? "4 / 7 available" : "1 / 7 available"}</span>
        </div>
        <ol className="workflow-list">
          <WorkflowItem label="Dataset" state="Available" active />
          {versioned ? (
            <>
              <WorkflowItem label="Agent registration" state="Available" active />
              <WorkflowItem label="Policy versions" state="Available" active />
              <WorkflowItem label="Request inbox" state="Available" active />
              <WorkflowItem label="Approval and snapshot" state="Pending" />
              <WorkflowItem label="Agent capabilities" state="Pending" />
              <WorkflowItem label="Reconciliation" state="Pending" />
            </>
          ) : (
            <>
              <WorkflowItem label="Policy" state="Pending" />
              <WorkflowItem label="Access graph" state="Pending" />
              <WorkflowItem label="Capability" state="Pending" />
              <WorkflowItem label="Reconciliation" state="Pending" />
              <WorkflowItem label="Denial lab" state="Pending" />
              <WorkflowItem label="Receipts" state="Pending" />
            </>
          )}
        </ol>
      </section>
    </section>
  );
}

function WorkflowItem({
  label,
  state,
  active = false,
}: {
  readonly label: string;
  readonly state: string;
  readonly active?: boolean;
}) {
  return (
    <li data-active={active}>
      <span className="workflow-index" aria-hidden="true" />
      <strong>{label}</strong>
      <span>{state}</span>
    </li>
  );
}
