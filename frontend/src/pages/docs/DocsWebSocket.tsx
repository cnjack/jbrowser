import { DocsLayout } from './DocsLayout';

export function DocsWebSocket({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;

  return (
    <DocsLayout slug="api/websocket" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">API Reference</p>
          <h1 className="page-title">WebSocket Protocol</h1>
          <p className="page-subtitle">
            Real-time communication between frontend, control plane, and agents.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Control WebSocket</h2>
        <div className="docs-code">
          <pre><code>{`ws://${host}/ws/control`}</code></pre>
        </div>
        <p>
          Used by the web UI and custom clients for real-time browser interaction.
          Requires JWT authentication as the first message.
        </p>

        <h3 className="docs-h3">Connection Flow</h3>
        <div className="docs-code">
          <pre><code>{`// 1. Connect
const ws = new WebSocket('ws://${host}/ws/control');

// 2. Authenticate (must be first message)
ws.send(JSON.stringify({
  type: 'auth',
  payload: { token: '<jwt_token>' }
}));
// ← {"type":"auth.ok","payload":{}}

// 3. Subscribe to a browser instance
ws.send(JSON.stringify({
  type: 'browser.subscribe',
  payload: { browserInstanceId: '<browser_id>' }
}));
// ← {"type":"browser.state","payload":{...}}  (current state)
// ← {"type":"tab.list","payload":{"tabs":[...]}}  (current tabs)
// ← Binary frames (video stream — JPEG frames)`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Client → Server Messages</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Type</th><th>Payload</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr>
                <td className="cell-mono">auth</td>
                <td className="cell-mono">{"{"} token: string {"}"}</td>
                <td>Authenticate with JWT (required first message)</td>
              </tr>
              <tr>
                <td className="cell-mono">ping</td>
                <td className="cell-mono">{"{}"}</td>
                <td>Keepalive ping</td>
              </tr>
              <tr>
                <td className="cell-mono">browser.subscribe</td>
                <td className="cell-mono">{"{"} browserInstanceId {"}"}</td>
                <td>Subscribe to browser video + events</td>
              </tr>
              <tr>
                <td className="cell-mono">input.event</td>
                <td className="cell-mono">{"{"} type, x, y, ... {"}"}</td>
                <td>Mouse/keyboard input events</td>
              </tr>
              <tr>
                <td className="cell-mono">navigate.url</td>
                <td className="cell-mono">{"{"} url {"}"}</td>
                <td>Navigate browser to URL</td>
              </tr>
              <tr>
                <td className="cell-mono">tab.command</td>
                <td className="cell-mono">{"{"} command, tabId?, url? {"}"}</td>
                <td>Tab operations: new, close, activate</td>
              </tr>
              <tr>
                <td className="cell-mono">browser.reset</td>
                <td className="cell-mono">{"{}"}</td>
                <td>Reset browser to clean state</td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Server → Client Messages</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>Type</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">auth.ok</td><td>Authentication successful</td></tr>
              <tr><td className="cell-mono">auth.error</td><td>Authentication failed</td></tr>
              <tr><td className="cell-mono">pong</td><td>Response to ping</td></tr>
              <tr><td className="cell-mono">browser.state</td><td>Browser instance state snapshot</td></tr>
              <tr><td className="cell-mono">tab.list</td><td>Updated tab list from agent</td></tr>
              <tr><td className="cell-mono">error</td><td>Error message</td></tr>
            </tbody>
          </table>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Binary Frames (Video)</h2>
        <p>
          After subscribing to a browser, binary WebSocket frames contain JPEG video frames
          in a custom wire format:
        </p>
        <div className="docs-code">
          <pre><code>{`// Binary frame layout:
// [1 byte]  frame_type  (0x01 = JPEG)
// [1 byte]  stream_id   (0x00)
// [8 bytes] sequence    (big-endian u64)
// [8 bytes] timestamp   (big-endian u64, ms since epoch)
// [rest]    payload     (JPEG bytes)`}</code></pre>
        </div>
        <p>
          The web UI decodes these frames and renders them via an <code>&lt;img&gt;</code> tag.
          For headless automation, use the CDP tunnel instead (no video decoding needed).
        </p>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Input Event Types</h2>
        <div className="docs-api-table">
          <table className="data-table">
            <thead>
              <tr><th>type</th><th>Fields</th><th>Description</th></tr>
            </thead>
            <tbody>
              <tr><td className="cell-mono">click</td><td>x, y, button, modifiers</td><td>Mouse click (press + release)</td></tr>
              <tr><td className="cell-mono">mousedown</td><td>x, y, button, clickCount, modifiers</td><td>Mouse button press</td></tr>
              <tr><td className="cell-mono">mouseup</td><td>x, y, button, clickCount, modifiers</td><td>Mouse button release</td></tr>
              <tr><td className="cell-mono">mousemove</td><td>x, y, modifiers</td><td>Mouse movement</td></tr>
              <tr><td className="cell-mono">wheel</td><td>x, y, deltaX, deltaY</td><td>Scroll wheel</td></tr>
              <tr><td className="cell-mono">keydown</td><td>key, code, text, modifiers, keyCode</td><td>Key press</td></tr>
              <tr><td className="cell-mono">keyup</td><td>key, code, text, modifiers, keyCode</td><td>Key release</td></tr>
            </tbody>
          </table>
        </div>
      </section>
    </DocsLayout>
  );
}
