import { DocsLayout } from './DocsLayout';

export function DocsCdpConnect({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;
  const tid = tenantId || '<tenant_id>';

  return (
    <DocsLayout slug="guides/cdp-connect" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Guides</p>
          <h1 className="page-title">Connect with CDP</h1>
          <p className="page-subtitle">
            Use the Chrome DevTools Protocol to automate browser instances.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">How It Works</h2>
        <p>
          JBrowser exposes standard CDP endpoints for each browser instance.
          The connection path is:
        </p>
        <div className="docs-code">
          <pre><code>{`Your CDP client
  → WebSocket to control plane (wss://${host}/cdp/...)
    → Tunneled to agent
      → Chrome CDP (localhost:9222)`}</code></pre>
        </div>
        <p>
          This means any tool that speaks CDP can connect — Puppeteer, Playwright,
          raw WebSocket, or AI agent frameworks. No direct access to the agent is needed.
        </p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Endpoint Format</h2>
        <p>All CDP endpoints require a <code>?token=&lt;cdp_token&gt;</code> query parameter.</p>

        <h3 className="docs-h3">Discovery</h3>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr>
                <td className="cell-mono">/cdp/tenants/:tid/browser-instances/:bid/json/version</td>
                <td>Browser version info + <code>webSocketDebuggerUrl</code></td>
              </tr>
              <tr>
                <td className="cell-mono">/cdp/tenants/:tid/browser-instances/:bid/json/list</td>
                <td>List of page targets with WebSocket URLs</td>
              </tr>
            </tbody>
          </table>
        </div>

        <h3 className="docs-h3">WebSocket Tunnel</h3>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Endpoint</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr>
                <td className="cell-mono">/cdp/.../devtools/page/:target_id</td>
                <td>Connect to a specific page target</td>
              </tr>
              <tr>
                <td className="cell-mono">/cdp/.../devtools/browser/:target_id</td>
                <td>Connect to the browser target</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Puppeteer</h2>
        <div className="docs-code">
          <pre><code>{`import puppeteer from 'puppeteer-core';

// Get target list to find the WebSocket URL
const res = await fetch(
  'https://${host}/cdp/tenants/${tid}/browser-instances/<bid>/json/list?token=<token>'
);
const targets = await res.json();
const wsUrl = targets[0].webSocketDebuggerUrl;

// Connect and automate
const browser = await puppeteer.connect({ browserWSEndpoint: wsUrl });
const page = await browser.newPage();
await page.goto('https://example.com');
const title = await page.title();
const screenshot = await page.screenshot({ type: 'png' });
console.log('Title:', title);`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Playwright</h2>
        <div className="docs-code">
          <pre><code>{`import { chromium } from 'playwright';

const browser = await chromium.connectOverCDP(
  'wss://${host}/cdp/tenants/${tid}/browser-instances/<bid>/devtools/browser/<target>?token=<token>'
);

const context = browser.contexts()[0];
const page = context.pages()[0] || await context.newPage();
await page.goto('https://example.com');
console.log(await page.title());
await page.screenshot({ path: 'screenshot.png' });`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Raw CDP (Node.js)</h2>
        <p>For maximum control, connect directly via WebSocket and send CDP commands as JSON:</p>
        <div className="docs-code">
          <pre><code>{`// Node.js 22+ (built-in WebSocket, no dependencies)
const ws = new WebSocket(
  'wss://${host}/cdp/tenants/${tid}/browser-instances/<bid>/devtools/page/<target>?token=<token>'
);

let cmdId = 0;
const pending = new Map();

function send(method, params = {}) {
  const id = ++cmdId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

ws.onmessage = (e) => {
  const msg = JSON.parse(e.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
  }
};

ws.onopen = async () => {
  // Navigate
  await send('Page.navigate', { url: 'https://example.com' });

  // Wait a bit, then take screenshot
  setTimeout(async () => {
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    // data is base64-encoded PNG
    require('fs').writeFileSync('shot.png', Buffer.from(data, 'base64'));

    // Evaluate JavaScript
    await send('Runtime.enable');
    const result = await send('Runtime.evaluate', {
      expression: 'document.title',
      returnByValue: true
    });
    console.log('Title:', result.result.value);
  }, 2000);
};`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Common CDP Commands</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Command</th><th>Use Case</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">Page.navigate</td><td>Navigate to a URL</td></tr>
              <tr><td className="cell-mono">Page.captureScreenshot</td><td>Take a screenshot (PNG/JPEG)</td></tr>
              <tr><td className="cell-mono">Runtime.evaluate</td><td>Execute JavaScript in page context</td></tr>
              <tr><td className="cell-mono">DOM.getDocument</td><td>Get the DOM tree</td></tr>
              <tr><td className="cell-mono">Accessibility.getFullAXTree</td><td>Get accessibility tree (semantic page structure)</td></tr>
              <tr><td className="cell-mono">Input.dispatchMouseEvent</td><td>Click, scroll, hover</td></tr>
              <tr><td className="cell-mono">Input.dispatchKeyEvent</td><td>Keyboard input</td></tr>
              <tr><td className="cell-mono">Input.insertText</td><td>Type text (works in iframes)</td></tr>
              <tr><td className="cell-mono">Network.enable</td><td>Monitor network requests</td></tr>
              <tr><td className="cell-mono">Page.getLayoutMetrics</td><td>Get viewport dimensions and DPR</td></tr>
            </tbody>
          </table>
        </div>
        <p>
          Full protocol reference:{' '}
          <a href="https://chromedevtools.github.io/devtools-protocol/" target="_blank" rel="noopener noreferrer" className="docs-link">
            Chrome DevTools Protocol Documentation ↗
          </a>
        </p>
      </section>
    </DocsLayout>
  );
}
