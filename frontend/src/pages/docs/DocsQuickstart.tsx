import { DocsLayout } from './DocsLayout';

export function DocsQuickstart({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;
  const tid = tenantId || '<tenant_id>';

  return (
    <DocsLayout slug="quickstart" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Getting Started</p>
          <h1 className="page-title">5-Minute Quickstart</h1>
          <p className="page-subtitle">
            From zero to CDP connection in three steps.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">1</span>
          Login &amp; Get Your Tenant ID
        </h2>
        <div className="docs-code">
          <pre><code>{`# Login to get your JWT token
curl -s -X POST https://${host}/api/v1/auth/login \\
  -H 'Content-Type: application/json' \\
  -d '{"email":"admin@example.com","password":"jbrowser"}' \\
  | jq '{token: .access_token, tenant: .tenants[0].id}'`}</code></pre>
        </div>
        <p>Save the <code>tenant_id</code> from the response — you'll need it for all subsequent calls.</p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">2</span>
          Create a CDP Token
        </h2>
        <div className="docs-code">
          <pre><code>{`# Create a CDP access token
curl -s -X POST https://${host}/api/v1/tenants/${tid}/tokens/cdp \\
  -H 'Authorization: Bearer <jwt_token>' \\
  -H 'Content-Type: application/json' \\
  -d '{"name":"my-cdp-token"}' \\
  | jq '{token: .token, prefix: .data.token_prefix}'`}</code></pre>
        </div>
        <div className="docs-note docs-note-warn">
          <strong>Save the token!</strong> It's only shown once. You'll use it as <code>?token=...</code> on CDP endpoints.
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">
          <span className="docs-step-badge">3</span>
          Connect via CDP
        </h2>

        <h3 className="docs-h3">Discover available targets</h3>
        <div className="docs-code">
          <pre><code>{`# List browser targets
curl -s 'https://${host}/cdp/tenants/${tid}/browser-instances/<browser_id>/json/list?token=<cdp_token>' | jq .

# Response:
# [
#   {
#     "id": "ABC123...",
#     "type": "page",
#     "title": "...",
#     "url": "...",
#     "webSocketDebuggerUrl": "wss://${host}/cdp/.../devtools/page/ABC123...?token=..."
#   }
# ]`}</code></pre>
        </div>

        <h3 className="docs-h3">Connect with Puppeteer</h3>
        <div className="docs-code">
          <pre><code>{`import puppeteer from 'puppeteer-core';

const browser = await puppeteer.connect({
  browserWSEndpoint:
    'wss://${host}/cdp/tenants/${tid}/browser-instances/<browser_id>/devtools/browser/<target_id>?token=<cdp_token>'
});

const page = await browser.newPage();
await page.goto('https://example.com');
console.log(await page.title()); // "Example Domain"`}</code></pre>
        </div>

        <h3 className="docs-h3">Connect with Playwright</h3>
        <div className="docs-code">
          <pre><code>{`import { chromium } from 'playwright';

const browser = await chromium.connectOverCDP(
  'wss://${host}/cdp/tenants/${tid}/browser-instances/<browser_id>/devtools/browser/<target_id>?token=<cdp_token>'
);

const page = browser.contexts()[0].pages()[0];
await page.goto('https://example.com');
console.log(await page.title());`}</code></pre>
        </div>

        <h3 className="docs-h3">Connect with raw WebSocket (Node.js)</h3>
        <div className="docs-code">
          <pre><code>{`// Node.js 22+ (built-in WebSocket)
const ws = new WebSocket(
  'wss://${host}/cdp/tenants/${tid}/browser-instances/<browser_id>/devtools/page/<target_id>?token=<cdp_token>'
);

ws.onopen = () => {
  ws.send(JSON.stringify({
    id: 1,
    method: 'Page.navigate',
    params: { url: 'https://example.com' }
  }));
};

ws.onmessage = (e) => console.log(JSON.parse(e.data));`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Next Steps</h2>
        <ul className="docs-quick-links">
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/guides/ai-agents` : '/docs/guides/ai-agents'}>Integrate with AI agents</a></li>
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/api/cdp` : '/docs/api/cdp'}>Full CDP endpoint reference</a></li>
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/guides/deploy-agent` : '/docs/guides/deploy-agent'}>Deploy your own agents</a></li>
        </ul>
      </section>
    </DocsLayout>
  );
}
