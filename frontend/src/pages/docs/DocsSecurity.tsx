import { DocsLayout } from './DocsLayout';

export function DocsSecurity({ tenantId }: { tenantId?: string }) {
  return (
    <DocsLayout slug="concepts/security" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Concepts</p>
          <h1 className="page-title">Security &amp; Tokens</h1>
          <p className="page-subtitle">
            Authentication model, token types, and security practices.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Token Types</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Token</th><th>Prefix</th><th>Purpose</th><th>Scope</th></tr>
            </thead>
            <tbody>
              <tr>
                <td>JWT (Bearer)</td>
                <td className="cell-mono">eyJ...</td>
                <td>User authentication — REST API and WebSocket</td>
                <td>User's tenants, 12h expiry</td>
              </tr>
              <tr>
                <td>CDP Access Token</td>
                <td className="cell-mono">jbr_cdp_</td>
                <td>CDP endpoint access — <code>?token=</code> on CDP URLs</td>
                <td>Tenant (all browsers)</td>
              </tr>
              <tr>
                <td>Agent Registration Token</td>
                <td className="cell-mono">jbr_reg_</td>
                <td>Agent self-registration on first connect</td>
                <td>Tenant</td>
              </tr>
              <tr>
                <td>Agent Runtime Token</td>
                <td className="cell-mono">jbr_agent_</td>
                <td>Agent WebSocket auth (issued after registration)</td>
                <td>Single agent</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Authentication Flow</h2>

        <h3 className="docs-h3">User Authentication</h3>
        <ol>
          <li>User sends <code>POST /api/v1/auth/login</code> with email + password</li>
          <li>Password verified with Argon2 hash</li>
          <li>JWT issued with user ID, email, and tenant claims</li>
          <li>JWT used as <code>Authorization: Bearer</code> header for REST API calls</li>
          <li>JWT sent as first message on <code>/ws/control</code> WebSocket</li>
        </ol>

        <h3 className="docs-h3">Agent Authentication</h3>
        <ol>
          <li>Agent sends <code>POST /api/v1/agents/register</code> with registration token</li>
          <li>Control plane validates token, creates agent + browser instance</li>
          <li>Returns a runtime token (<code>jbr_agent_...</code>)</li>
          <li>Agent stores runtime token locally and uses it for WebSocket auth</li>
          <li>On reconnect, agent reuses the runtime token</li>
        </ol>

        <h3 className="docs-h3">CDP Authentication</h3>
        <ol>
          <li>User creates a CDP token via REST API or web UI</li>
          <li>Token hash is stored; raw token shown only once</li>
          <li>CDP client passes token as <code>?token=jbr_cdp_...</code> query parameter</li>
          <li>Control plane validates token hash against tenant</li>
        </ol>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Tenant Isolation</h2>
        <p>Every data access is scoped by <code>tenant_id</code>:</p>
        <ul>
          <li>JWT claims carry the user's tenant memberships — only those tenants are accessible</li>
          <li>Every REST API query filters <code>WHERE tenant_id = ?</code></li>
          <li>CDP tokens are created under a tenant and only grant access to that tenant's browsers</li>
          <li>Agent registration tokens are tenant-scoped — registered agents belong to that tenant</li>
          <li>Audit logs are per-tenant</li>
        </ul>
        <div className="docs-note docs-note-warn">
          <strong>Invariant:</strong> A CDP token for Tenant A cannot access browsers belonging to Tenant B.
          This is enforced at the control plane level for every request.
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Token Management</h2>
        <ul>
          <li><strong>Revoke</strong> — immediately invalidates a token. Existing CDP tunnel sessions remain open until they disconnect.</li>
          <li><strong>Rotate</strong> — revokes the old token and issues a new one atomically. Use for scheduled rotation.</li>
          <li><strong>Prefix</strong> — tokens are stored as SHA-256 hashes. Only the prefix (<code>jbr_cdp_xxxx</code>) is visible in the UI for identification.</li>
        </ul>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Network Security</h2>
        <ul>
          <li>
            <strong>Agent → Control Plane only:</strong> Agents connect outbound via WebSocket.
            No inbound ports are opened on agent containers.
          </li>
          <li>
            <strong>Chrome CDP localhost only:</strong> Chrome's <code>--remote-debugging-port=9222</code> binds
            to <code>127.0.0.1</code> inside the container. It's never exposed to the network.
          </li>
          <li>
            <strong>Use TLS in production:</strong> Always run the control plane behind TLS
            (<code>wss://</code> / <code>https://</code>) to protect tokens in transit.
          </li>
        </ul>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Best Practices</h2>
        <ul>
          <li>Never commit CDP tokens to version control</li>
          <li>Use environment variables or secret managers for tokens</li>
          <li>Rotate CDP tokens regularly (weekly or on personnel change)</li>
          <li>Create separate tokens per integration (one for CI, one for the agent, etc.)</li>
          <li>Monitor audit logs for unexpected access patterns</li>
          <li>Revoke tokens immediately when they're no longer needed</li>
        </ul>
      </section>
    </DocsLayout>
  );
}
