import { Globe } from 'lucide-react';
import { useAuthStore } from '../stores/auth';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId?: string;
}

export function Docs({ tenantId }: Props) {
  const token = useAuthStore((s) => s.token);
  const tid = tenantId ?? useAuthStore((s) => s.tenantId) ?? '';

  const host = window.location.host;

  // If logged in, show with sidebar. Otherwise show standalone.
  const content = (
    <div className="page-shell docs-page">
      <div className="page-header">
        <div>
          <p className="eyebrow">Documentation</p>
          <h1 className="page-title">Getting Started</h1>
          <p className="page-subtitle">Deploy agents, connect browsers, and start automating.</p>
        </div>
      </div>

      {/* Step 1 */}
      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">1</span>
          Deploy an Agent
        </h2>
        <p>
          Go to <a href={`/tenants/${tid}/agents`} className="docs-link">Agents</a> and click{' '}
          <strong>New agent</strong>. A registration token will be auto-generated and bound to your tenant.
          Follow the setup instructions shown there to start an agent container.
        </p>

        <h3 className="docs-h3">Docker Compose</h3>
        <div className="docs-code">
          <pre><code>{`services:
  agent:
    image: ghcr.io/cnjack/jbrowser-agent-chromium:latest
    shm_size: "1gb"
    environment:
      CONTROL_PLANE_URL: wss://${host}
      REGISTRATION_TOKEN: <token from Agents page>
    restart: unless-stopped`}</code></pre>
        </div>

        <h3 className="docs-h3">Docker Run</h3>
        <div className="docs-code">
          <pre><code>{`docker run -d \\
  --name jbrowser-agent \\
  --shm-size=1g \\
  -e CONTROL_PLANE_URL=wss://${host} \\
  -e REGISTRATION_TOKEN=<token> \\
  ghcr.io/cnjack/jbrowser-agent-chromium:latest`}</code></pre>
        </div>

        <h3 className="docs-h3">Kubernetes (Helm)</h3>
        <div className="docs-code">
          <pre><code>{`helm install jbrowser-agent ./helm/jbrowser \\
  --set agent.controlPlaneUrl=wss://${host} \\
  --set agent.registrationToken=<token> \\
  --set agent.replicas=3`}</code></pre>
        </div>

        <div className="docs-note">
          <strong>Important:</strong> Always configure <code>shm_size: 1gb</code> (or mount <code>/dev/shm</code> as
          a tmpfs) — Chrome needs shared memory for rendering. The agent runs as non-root by default.
        </div>
      </section>

      {/* Step 2 */}
      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">2</span>
          Get a CDP API Key
        </h2>
        <p>
          Go to <a href={`/tenants/${tid}/settings/api-keys`} className="docs-link">Settings → API Keys</a> and
          create a CDP access token. This token grants access to all browser instances in your tenant.
        </p>
        <div className="docs-note docs-note-warn">
          <strong>Security:</strong> CDP tokens are tenant-scoped and grant full browser control.
          Treat them as secrets — rotate regularly and never commit to version control.
        </div>
      </section>

      {/* Step 3 */}
      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">3</span>
          Connect with CDP
        </h2>
        <p>
          Use the CDP endpoint to connect Puppeteer, Playwright, or any CDP-compatible tool.
        </p>

        <h3 className="docs-h3">Puppeteer</h3>
        <div className="docs-code">
          <pre><code>{`const puppeteer = require('puppeteer-core');

const browser = await puppeteer.connect({
  browserWSEndpoint:
    'wss://${host}/cdp/tenants/${tid || '<tenant_id>'}/browser-instances/<browser_id>/devtools/browser/<target_id>?token=<api_key>'
});

const page = await browser.newPage();
await page.goto('https://example.com');
console.log(await page.title());`}</code></pre>
        </div>

        <h3 className="docs-h3">Playwright</h3>
        <div className="docs-code">
          <pre><code>{`const { chromium } = require('playwright');

const browser = await chromium.connectOverCDP(
  'wss://${host}/cdp/tenants/${tid || '<tenant_id>'}/browser-instances/<browser_id>/devtools/browser/<target_id>?token=<api_key>'
);

const page = browser.contexts()[0].pages()[0];
await page.goto('https://example.com');`}</code></pre>
        </div>
      </section>

      {/* API Reference */}
      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">4</span>
          REST API Reference
        </h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr>
                <th>Method</th>
                <th>Endpoint</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/auth/login</td><td>Authenticate and get JWT</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances</td><td>List browser instances</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances/:id</td><td>Get browser instance</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/browser-instances/:id/reset</td><td>Reset browser</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/agents</td><td>List agents</td></tr>
              <tr><td className="cell-mono">GET</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp</td><td>List CDP tokens</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp</td><td>Create CDP token</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp/:id/revoke</td><td>Revoke CDP token</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/tokens/cdp/:id/rotate</td><td>Rotate CDP token</td></tr>
              <tr><td className="cell-mono">POST</td><td className="cell-mono">/api/v1/tenants/:tid/agent-registration-tokens</td><td>Generate agent registration token (auto-bound to tenant)</td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );

  if (token && tid) {
    return (
      <div className="app-layout">
        <Sidebar activePage="docs" tenantId={tid} />
        <main className="main-area">{content}</main>
      </div>
    );
  }

  // Standalone (not logged in)
  return (
    <div className="docs-standalone">
      <nav className="landing-nav">
        <div className="landing-nav-inner">
          <a href="/" className="brand-link">
            <span className="brand-icon"><Globe size={18} strokeWidth={2.2} /></span>
            <span className="brand-name">
              <span className="brand-bracket">[</span>
              <span className="brand-j">J</span>
              <span className="brand-text">Browser</span>
              <span className="brand-bracket">]</span>
            </span>
          </a>
          <div className="landing-nav-actions">
            <a href="/login" className="landing-btn-ghost">Log In</a>
          </div>
        </div>
      </nav>
      <main className="docs-standalone-main">{content}</main>
    </div>
  );
}
