import { DocsLayout } from './DocsLayout';

export function DocsRestApi({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;

  return (
    <DocsLayout slug="api/rest" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">API Reference</p>
          <h1 className="page-title">REST API</h1>
          <p className="page-subtitle">
            Manage browser instances, agents, tokens, and audit logs.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Authentication</h2>
        <p>
          All API endpoints (except <code>/api/v1/auth/login</code>) require a JWT bearer token:
        </p>
        <div className="docs-code">
          <pre><code>{`Authorization: Bearer <jwt_token>`}</code></pre>
        </div>

        <h3 className="docs-h3">POST /api/v1/auth/login</h3>
        <p>Authenticate and receive a JWT token.</p>
        <div className="docs-code">
          <pre><code>{`curl -X POST https://${host}/api/v1/auth/login \\
  -H 'Content-Type: application/json' \\
  -d '{"email":"admin@example.com","password":"jbrowser"}'

# Response:
{
  "access_token": "eyJ...",
  "token_type": "Bearer",
  "user": { "id": "...", "email": "...", "display_name": "..." },
  "tenants": [{ "id": "...", "name": "...", "slug": "..." }]
}`}</code></pre>
        </div>

        <h3 className="docs-h3">GET /api/v1/auth/me</h3>
        <p>Get the current authenticated user.</p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Browser Instances</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances</td><td>List all browser instances in tenant</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances/:id</td><td>Get browser instance details (tabs, status, viewport)</td></tr>
              <tr><td className="cell-mono">DELETE</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances/:id</td><td>Delete browser instance</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances/:id/reset</td><td>Reset browser (close tabs, clear cookies, navigate to about:blank)</td></tr>
            </tbody>
          </table>
        </div>

        <h3 className="docs-h3">Browser Instance Object</h3>
        <div className="docs-code">
          <pre><code>{`{
  "id": "019e...",
  "tenant_id": "019e...",
  "agent_id": "019e...",
  "name": "chromium-019e...",
  "status": "online",          // online | offline | restarting
  "browser_type": "chromium",
  "browser_version": "128.0.6613.137",
  "active_tab_id": "ABC123...",
  "tabs": [
    { "id": "ABC123...", "title": "Example", "url": "https://example.com", "active": true }
  ],
  "viewport_width": 1280,
  "viewport_height": 720,
  "viewer_count": 0,
  "agent_name": "my-agent",
  "agent_status": "online",
  "last_heartbeat_at": "2024-01-01T00:00:00Z"
}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Agents</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/agents</td><td>List all agents</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/agents/:id</td><td>Get agent details</td></tr>
              <tr><td className="cell-mono">DELETE</td><td className="cell-mono">/api/v1/tenants/:tid/agents/:id</td><td>Delete agent and its browser instance</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">CDP Tokens</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp</td><td>List CDP tokens (hash only, not the raw token)</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp</td><td>Create CDP token — returns raw token once</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp/:id/revoke</td><td>Revoke a CDP token</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp/:id/rotate</td><td>Rotate a CDP token (revoke old, issue new)</td></tr>
            </tbody>
          </table>
        </div>

        <h3 className="docs-h3">Create CDP Token</h3>
        <div className="docs-code">
          <pre><code>{`curl -X POST https://${host}/api/v1/tenants/<tid>/tokens/cdp \\
  -H 'Authorization: Bearer <jwt>' \\
  -H 'Content-Type: application/json' \\
  -d '{"name":"my-token"}'

# Response:
{
  "data": {
    "id": "019e...",
    "token_type": "tenant_cdp_access",
    "name": "my-token",
    "token_prefix": "jbr_cdp_xxxx"
  },
  "token": "jbr_cdp_full_token_here..."   // ← save this, shown only once
}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Agent Registration Tokens</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/agent-registration-tokens</td><td>List registration tokens</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/agent-registration-tokens</td><td>Create registration token</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/agent-registration-tokens/:id/revoke</td><td>Revoke registration token</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Audit Logs</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/audit-logs</td><td>List audit log entries</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Health &amp; Ops</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Method</th><th>Endpoint</th><th>Auth</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/health</td><td>No</td><td>Health check</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/ready</td><td>No</td><td>Readiness check</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/metrics</td><td>No</td><td>Prometheus metrics</td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </DocsLayout>
  );
}
