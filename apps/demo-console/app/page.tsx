import { StatusTable } from "../components/status-table";
import { consoleStatus, countReadyItems } from "../lib/console-status";

export default function ConsolePage() {
  const readyItems = countReadyItems(consoleStatus.workflow);

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <p className="product-name">Raziel Console</p>
          <p className="environment">Local M0 workspace</p>
        </div>
        <span className="phase-badge">Scaffold</span>
      </header>

      <section className="summary" aria-labelledby="summary-heading">
        <div>
          <p className="section-label">Workspace status</p>
          <h1 id="summary-heading">Payment reconciliation</h1>
          <p className="summary-copy">
            Runtime services are not connected. Public workflow artifacts are empty.
          </p>
        </div>
        <div className="readiness">
          <strong>
            {readyItems}/{consoleStatus.workflow.length}
          </strong>
          <span>steps ready</span>
        </div>
      </section>

      <div className="content-grid">
        <section className="panel" aria-labelledby="services-heading">
          <div className="panel-heading">
            <div>
              <p className="section-label">Runtime</p>
              <h2 id="services-heading">Services</h2>
            </div>
            <span className="muted-count">{consoleStatus.services.length}</span>
          </div>
          <StatusTable items={consoleStatus.services} />
        </section>

        <section className="panel" aria-labelledby="workflow-heading">
          <div className="panel-heading">
            <div>
              <p className="section-label">Scenario</p>
              <h2 id="workflow-heading">Workflow</h2>
            </div>
            <span className="muted-count">{consoleStatus.workflow.length}</span>
          </div>
          <StatusTable items={consoleStatus.workflow} />
        </section>
      </div>

      <section className="artifact-band" aria-labelledby="artifacts-heading">
        <div>
          <p className="section-label">Public artifacts</p>
          <h2 id="artifacts-heading">No capability selected</h2>
        </div>
        <p>No proof summary or audit receipt is present.</p>
      </section>
    </main>
  );
}
