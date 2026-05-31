import { DocsLayout } from './DocsLayout';

export function DocsDeployAgent({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;
  const tid = tenantId || '<tenant_id>';

  return (
    <DocsLayout slug="guides/deploy-agent" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Guides</p>
          <h1 className="page-title">Deploy an Agent</h1>
          <p className="page-subtitle">
            Agents are headless Chrome containers that connect to your control plane.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Prerequisites</h2>
        <ul>
          <li>A running JBrowser control plane</li>
          <li>An agent registration token (create one in <a href={`/tenants/${tid}/agents`} className="docs-link">Agents</a> or via the REST API)</li>
          <li>Docker or Kubernetes</li>
        </ul>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Docker Compose</h2>
        <div className="docs-code">
          <pre><code>{`services:
  agent:
    image: ghcr.io/cnjack/jbrowser-agent-chromium:latest
    shm_size: "1gb"
    environment:
      CONTROL_PLANE_HTTP_URL: https://${host}
      CONTROL_PLANE_WS_URL: wss://${host}
      REGISTRATION_TOKEN: <token from Agents page>
      AGENT_NAME: my-agent-1
      RUST_LOG: info,jbrowser_agent=debug
    restart: unless-stopped`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Docker Run</h2>
        <div className="docs-code">
          <pre><code>{`docker run -d \\
  --name jbrowser-agent \\
  --shm-size=1g \\
  -e CONTROL_PLANE_HTTP_URL=https://${host} \\
  -e CONTROL_PLANE_WS_URL=wss://${host} \\
  -e REGISTRATION_TOKEN=<token> \\
  -e AGENT_NAME=my-agent \\
  ghcr.io/cnjack/jbrowser-agent-chromium:latest`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Kubernetes (Helm)</h2>
        <div className="docs-code">
          <pre><code>{`helm install jbrowser-agent ./helm/jbrowser \\
  --set agent.controlPlaneUrl=wss://${host} \\
  --set agent.registrationToken=<token> \\
  --set agent.replicas=3`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Environment Variables</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Variable</th><th>Required</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">CONTROL_PLANE_HTTP_URL</td><td>Yes</td><td>HTTP URL of the control plane (for registration)</td></tr>
              <tr><td className="cell-mono">CONTROL_PLANE_WS_URL</td><td>No</td><td>WebSocket URL (derived from HTTP URL if not set)</td></tr>
              <tr><td className="cell-mono">REGISTRATION_TOKEN</td><td>Yes</td><td>Agent registration token from your tenant</td></tr>
              <tr><td className="cell-mono">AGENT_NAME</td><td>No</td><td>Display name (default: <code>chromium-agent</code>)</td></tr>
              <tr><td className="cell-mono">BROWSER_TYPE</td><td>No</td><td>Browser identifier (default: <code>chromium</code>)</td></tr>
              <tr><td className="cell-mono">RUST_LOG</td><td>No</td><td>Logging level (default: <code>info</code>)</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Important Notes</h2>
        <div className="docs-note">
          <strong>Shared memory:</strong> Always configure <code>shm_size: 1gb</code> (or mount <code>/dev/shm</code> as
          a tmpfs). Chrome needs shared memory for rendering — without it, tabs will crash.
        </div>
        <div className="docs-note">
          <strong>Security:</strong> The agent connects <em>outbound</em> to the control plane. No inbound ports
          are required. Chrome's CDP port (<code>:9222</code>) is bound to <code>127.0.0.1</code> only inside the container.
        </div>
        <div className="docs-note">
          <strong>Auto-reconnect:</strong> If the control plane restarts, agents will automatically reconnect
          and re-register. The registration token is persisted locally in the container.
        </div>
      </section>
    </DocsLayout>
  );
}
