import { gatewayStatus } from "../src/service-status";

export default function GatewayPage() {
  return (
    <main className="gateway-shell">
      <header>
        <p className="product-name">Raziel Gateway</p>
        <span className="status-chip">{gatewayStatus.status}</span>
      </header>

      <section aria-labelledby="gateway-heading">
        <p className="section-label">M0 service shell</p>
        <h1 id="gateway-heading">Gateway runtime</h1>
        <dl>
          <div>
            <dt>Service</dt>
            <dd>{gatewayStatus.service}</dd>
          </div>
          <div>
            <dt>Authorization</dt>
            <dd>Not implemented</dd>
          </div>
          <div>
            <dt>Public endpoint</dt>
            <dd>
              <a href="/api/health">/api/health</a>
            </dd>
          </div>
        </dl>
      </section>
    </main>
  );
}
