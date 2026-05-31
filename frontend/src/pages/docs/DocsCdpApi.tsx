import { DocsLayout } from './DocsLayout';

export function DocsCdpApi({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;
  const tid = tenantId || '<tenant_id>';

  return (
    <DocsLayout slug="api/cdp" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">API Reference</p>
          <h1 className="page-title">CDP Endpoints</h1>
          <p className="page-subtitle">
            Chrome DevTools Protocol endpoints for browser automation.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Authentication</h2>
        <p>
          All CDP endpoints require a <strong>CDP token</strong> as a query parameter:
        </p>
        <div className="docs-code">
          <pre><code>{`?token=jbr_cdp_...`}</code></pre>
        </div>
        <p>
          CDP tokens are tenant-scoped. Create them via{' '}
          <a href={tenantId ? `/tenants/${tid}/settings/api-keys` : '/docs/api/rest'} className="docs-link">
            API Keys
          </a>{' '}
          or the REST API.
        </p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Discovery Endpoints</h2>

        <h3 className="docs-h3">GET /json/version</h3>
        <div className="docs-code">
          <pre><code>{`GET /cdp/tenants/:tid/browser-instances/:bid/json/version?token=<cdp_token>

# Response:
{
  "Browser": "Chrome/128.0.6613.137",
  "Protocol-Version": "1.3",
  "webSocketDebuggerUrl": "ws://${host}/cdp/tenants/${tid}/browser-instances/<bid>/devtools/browser/<target>?token=<token>"
}`}</code></pre>
        </div>
        <p>Returns browser version info and the browser-level WebSocket URL.</p>

        <h3 className="docs-h3">GET /json/list</h3>
        <div className="docs-code">
          <pre><code>{`GET /cdp/tenants/:tid/browser-instances/:bid/json/list?token=<cdp_token>

# Response:
[
  {
    "id": "1E33E68E01FCA2DC8DAC6C98EB3085B6",
    "type": "page",
    "title": "Example Domain",
    "url": "https://example.com",
    "webSocketDebuggerUrl": "ws://${host}/cdp/tenants/${tid}/browser-instances/<bid>/devtools/page/1E33E68E...?token=<token>"
  }
]`}</code></pre>
        </div>
        <p>
          Lists all page targets. Each target has a <code>webSocketDebuggerUrl</code> you can
          connect to directly.
        </p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">WebSocket Tunnel</h2>
        <p>
          Connect via WebSocket to send CDP commands and receive responses/events.
          The tunnel forwards messages bidirectionally to Chrome inside the agent container.
        </p>

        <h3 className="docs-h3">Page Target</h3>
        <div className="docs-code">
          <pre><code>{`ws://${host}/cdp/tenants/:tid/browser-instances/:bid/devtools/page/:target_id?token=<cdp_token>`}</code></pre>
        </div>
        <p>Connect to a specific page. Most automation uses this endpoint.</p>

        <h3 className="docs-h3">Browser Target</h3>
        <div className="docs-code">
          <pre><code>{`ws://${host}/cdp/tenants/:tid/browser-instances/:bid/devtools/browser/:target_id?token=<cdp_token>`}</code></pre>
        </div>
        <p>Connect to the browser-level target. Used by Puppeteer/Playwright for <code>browser.newPage()</code> etc.</p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Message Format</h2>
        <p>Standard CDP JSON-RPC over WebSocket:</p>

        <h3 className="docs-h3">Request</h3>
        <div className="docs-code">
          <pre><code>{`{
  "id": 1,
  "method": "Page.navigate",
  "params": { "url": "https://example.com" }
}`}</code></pre>
        </div>

        <h3 className="docs-h3">Response</h3>
        <div className="docs-code">
          <pre><code>{`{
  "id": 1,
  "result": {
    "frameId": "1E33E68E01FCA2DC8DAC6C98EB3085B6",
    "loaderId": "ABC123..."
  }
}`}</code></pre>
        </div>

        <h3 className="docs-h3">Event</h3>
        <div className="docs-code">
          <pre><code>{`{
  "method": "Page.loadEventFired",
  "params": { "timestamp": 1234567.89 }
}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Common Workflows</h2>

        <h3 className="docs-h3">Navigate and Screenshot</h3>
        <div className="docs-code">
          <pre><code>{`→ {"id":1, "method":"Page.navigate", "params":{"url":"https://example.com"}}
← {"id":1, "result":{"frameId":"..."}}
← {"method":"Page.loadEventFired", "params":{"timestamp":...}}
→ {"id":2, "method":"Page.captureScreenshot", "params":{"format":"png"}}
← {"id":2, "result":{"data":"iVBORw0KGgo..."}}`}</code></pre>
        </div>

        <h3 className="docs-h3">Click and Type</h3>
        <div className="docs-code">
          <pre><code>{`// Click at (400, 300)
→ {"id":1, "method":"Input.dispatchMouseEvent",
   "params":{"type":"mousePressed","x":400,"y":300,"button":"left","clickCount":1}}
→ {"id":2, "method":"Input.dispatchMouseEvent",
   "params":{"type":"mouseReleased","x":400,"y":300,"button":"left","clickCount":1}}

// Type text
→ {"id":3, "method":"Input.insertText", "params":{"text":"Hello World"}}`}</code></pre>
        </div>

        <h3 className="docs-h3">Get Accessibility Tree</h3>
        <div className="docs-code">
          <pre><code>{`→ {"id":1, "method":"Accessibility.getFullAXTree", "params":{}}
← {"id":1, "result":{"nodes":[
    {"nodeId":"1","role":{"value":"RootWebArea"},"name":{"value":"Example Domain"},...},
    {"nodeId":"2","role":{"value":"heading"},"name":{"value":"Example Domain"},...},
    ...
  ]}}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Path Parameters</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Parameter</th><th>Description</th><th>Where to find</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">:tid</td><td>Tenant ID (UUID v7)</td><td>Login response → <code>tenants[0].id</code></td></tr>
              <tr><td className="cell-mono">:bid</td><td>Browser instance ID (UUID v7)</td><td><code>GET /api/v1/tenants/:tid/browser-instances</code></td></tr>
              <tr><td className="cell-mono">:target_id</td><td>Chrome target ID (hex string)</td><td><code>/json/list</code> response → <code>id</code> field</td></tr>
              <tr><td className="cell-mono">token</td><td>CDP access token</td><td><code>POST /api/v1/tenants/:tid/tokens/cdp</code></td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </DocsLayout>
  );
}
