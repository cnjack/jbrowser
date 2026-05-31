import { DocsLayout } from './DocsLayout';

export function DocsAiAgents({ tenantId }: { tenantId?: string }) {
  const host = window.location.host;
  const tid = tenantId || '<tenant_id>';

  return (
    <DocsLayout slug="guides/ai-agents" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Guides</p>
          <h1 className="page-title">AI Agent Integration</h1>
          <p className="page-subtitle">
            Give your AI agent a real browser — navigate, screenshot, click, and type via CDP.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">Why JBrowser for AI Agents?</h2>
        <p>
          AI agents (Claude, GPT, custom LLM agents) need browser access to interact with
          the web — reading pages, filling forms, clicking buttons, taking screenshots for
          visual understanding. JBrowser provides:
        </p>
        <ul>
          <li><strong>Managed browser lifecycle</strong> — no local Chrome installation or Xvfb setup</li>
          <li><strong>Standard CDP protocol</strong> — works with any CDP client library</li>
          <li><strong>Multi-tenant isolation</strong> — each agent gets its own browser context</li>
          <li><strong>Real-time preview</strong> — watch what the agent is doing via the web UI</li>
          <li><strong>Persistent sessions</strong> — browsers stay alive between agent interactions</li>
        </ul>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Quick Setup</h2>
        <p>Your AI agent needs three things to control a browser:</p>
        <ol>
          <li>A <strong>CDP token</strong> — get one from <a href={`/tenants/${tid}/settings/api-keys`} className="docs-link">Settings → API Keys</a></li>
          <li>A <strong>browser instance ID</strong> — from the <a href={`/tenants/${tid}/browsers`} className="docs-link">Browsers</a> page or REST API</li>
          <li>A <strong>target ID</strong> — from the <code>/json/list</code> endpoint</li>
        </ol>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Agent Toolkit Pattern</h2>
        <p>
          The recommended pattern for AI agent integration is to expose CDP operations as
          "tools" that the agent can call. Here's a minimal toolkit:
        </p>

        <div className="docs-code">
          <pre><code>{`// agent-browser-tools.mjs — CDP tools for AI agents
// Requires: Node.js 22+ (built-in WebSocket)

const CDP_BASE = 'https://${host}/cdp/tenants/${tid}/browser-instances/<bid>';
const CDP_TOKEN = process.env.CDP_TOKEN;

// ── CDP Connection ───────────────────────────────────────────

class BrowserTool {
  #ws; #id = 0; #pending = new Map();

  async connect() {
    // Get the first page target
    const res = await fetch(\`\${CDP_BASE}/json/list?token=\${CDP_TOKEN}\`);
    const targets = await res.json();
    const wsUrl = targets[0].webSocketDebuggerUrl;

    return new Promise((resolve, reject) => {
      this.#ws = new WebSocket(wsUrl);
      this.#ws.onopen = () => resolve();
      this.#ws.onerror = (e) => reject(e);
      this.#ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (msg.id && this.#pending.has(msg.id)) {
          const { resolve, reject } = this.#pending.get(msg.id);
          this.#pending.delete(msg.id);
          msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
        }
      };
    });
  }

  send(method, params = {}) {
    const id = ++this.#id;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#ws.send(JSON.stringify({ id, method, params }));
      setTimeout(() => {
        if (this.#pending.has(id)) {
          this.#pending.delete(id);
          reject(new Error(\`Timeout: \${method}\`));
        }
      }, 15000);
    });
  }

  // ── Tools for AI agent ───────────────────────────────────

  async navigate(url) {
    const result = await this.send('Page.navigate', { url });
    if (result.errorText) throw new Error(result.errorText);
    // Wait for page load
    await new Promise(r => setTimeout(r, 2000));
    return \`Navigated to \${url}\`;
  }

  async screenshot() {
    const { data } = await this.send('Page.captureScreenshot', {
      format: 'png'
    });
    return data; // base64-encoded PNG
  }

  async getPageInfo() {
    await this.send('Runtime.enable');
    const title = await this.send('Runtime.evaluate', {
      expression: 'document.title', returnByValue: true
    });
    const url = await this.send('Runtime.evaluate', {
      expression: 'window.location.href', returnByValue: true
    });
    return {
      title: title.result.value,
      url: url.result.value
    };
  }

  async getAccessibilityTree() {
    const { nodes } = await this.send('Accessibility.getFullAXTree');
    // Return compact semantic tree
    return nodes
      .filter(n => {
        const role = n.role?.value || '';
        const name = n.name?.value || '';
        return role !== 'none' && role !== 'generic' && name !== '';
      })
      .map(n => ({
        role: n.role?.value,
        name: n.name?.value,
        value: n.value?.value
      }));
  }

  async click(x, y) {
    await this.send('Input.dispatchMouseEvent', {
      type: 'mousePressed', x, y, button: 'left', clickCount: 1
    });
    await this.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased', x, y, button: 'left', clickCount: 1
    });
    return \`Clicked at (\${x}, \${y})\`;
  }

  async type(text) {
    await this.send('Input.insertText', { text });
    return \`Typed: \${text}\`;
  }

  async evaluate(expression) {
    await this.send('Runtime.enable');
    const result = await this.send('Runtime.evaluate', {
      expression, returnByValue: true, awaitPromise: true
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text);
    }
    return result.result.value;
  }

  async clickSelector(selector) {
    const result = await this.evaluate(\`
      (function() {
        const el = document.querySelector(\${JSON.stringify(selector)});
        if (!el) return { ok: false, error: 'Element not found' };
        const rect = el.getBoundingClientRect();
        return {
          ok: true,
          x: rect.x + rect.width / 2,
          y: rect.y + rect.height / 2,
          tag: el.tagName
        };
      })()
    \`);
    const r = typeof result === 'string' ? JSON.parse(result) : result;
    if (!r.ok) throw new Error(r.error);
    await this.click(r.x, r.y);
    return \`Clicked <\${r.tag}> matching "\${selector}"\`;
  }
}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Using with AI Frameworks</h2>

        <h3 className="docs-h3">OpenAI Function Calling</h3>
        <div className="docs-code">
          <pre><code>{`// Define tools for the LLM
const tools = [
  {
    type: 'function',
    function: {
      name: 'browser_navigate',
      description: 'Navigate the browser to a URL',
      parameters: {
        type: 'object',
        properties: { url: { type: 'string' } },
        required: ['url']
      }
    }
  },
  {
    type: 'function',
    function: {
      name: 'browser_screenshot',
      description: 'Take a screenshot of the current page',
      parameters: { type: 'object', properties: {} }
    }
  },
  {
    type: 'function',
    function: {
      name: 'browser_click',
      description: 'Click at x,y coordinates',
      parameters: {
        type: 'object',
        properties: {
          x: { type: 'number' },
          y: { type: 'number' }
        },
        required: ['x', 'y']
      }
    }
  },
  // ... more tools
];

// Handle tool calls
async function handleToolCall(tool, browser) {
  switch (tool.function.name) {
    case 'browser_navigate':
      return await browser.navigate(tool.function.arguments.url);
    case 'browser_screenshot':
      return await browser.screenshot();
    case 'browser_click':
      const { x, y } = tool.function.arguments;
      return await browser.click(x, y);
  }
}`}</code></pre>
        </div>

        <h3 className="docs-h3">MCP (Model Context Protocol)</h3>
        <p>
          To use JBrowser as an MCP server, wrap the CDP tools as MCP tool handlers.
          Each CDP operation becomes an MCP tool that AI agents can invoke:
        </p>
        <div className="docs-code">
          <pre><code>{`// Example MCP tool definitions
{
  "tools": [
    {
      "name": "jbrowser_navigate",
      "description": "Navigate browser to URL",
      "inputSchema": {
        "type": "object",
        "properties": {
          "url": { "type": "string", "description": "URL to navigate to" }
        }
      }
    },
    {
      "name": "jbrowser_screenshot",
      "description": "Capture screenshot of current page (returns base64 PNG)"
    },
    {
      "name": "jbrowser_snapshot",
      "description": "Get accessibility tree of current page (semantic structure)"
    },
    {
      "name": "jbrowser_click",
      "description": "Click at CSS pixel coordinates",
      "inputSchema": {
        "type": "object",
        "properties": {
          "x": { "type": "number" },
          "y": { "type": "number" }
        }
      }
    },
    {
      "name": "jbrowser_type",
      "description": "Type text at current focus",
      "inputSchema": {
        "type": "object",
        "properties": {
          "text": { "type": "string" }
        }
      }
    },
    {
      "name": "jbrowser_evaluate",
      "description": "Evaluate JavaScript in page context",
      "inputSchema": {
        "type": "object",
        "properties": {
          "expression": { "type": "string" }
        }
      }
    }
  ]
}`}</code></pre>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Coordinate System</h2>
        <div className="docs-note">
          <strong>CSS pixels vs screenshot pixels:</strong> CDP input events use CSS pixel
          coordinates. If the device pixel ratio (DPR) is not 1, screenshot pixels differ
          from CSS pixels. JBrowser agents default to DPR=1 (1280×720 viewport), so
          screenshot coordinates = CSS coordinates. Use <code>Page.getLayoutMetrics</code> to verify.
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Best Practices</h2>
        <ul>
          <li><strong>Use accessibility tree first</strong> — <code>Accessibility.getFullAXTree</code> gives semantic page structure. Prefer it over screenshot-based visual reasoning when possible.</li>
          <li><strong>Prefer CSS selectors</strong> — <code>Runtime.evaluate</code> with <code>querySelector</code> is more reliable than coordinate-based clicking.</li>
          <li><strong>Wait after navigation</strong> — use <code>Page.loadEventFired</code> event or poll <code>document.readyState</code> before interacting.</li>
          <li><strong>Handle errors</strong> — CDP methods can fail (element not found, navigation error). Always check <code>result.exceptionDetails</code>.</li>
          <li><strong>Rotate tokens</strong> — use the token rotation API to periodically refresh CDP tokens.</li>
        </ul>
      </section>
    </DocsLayout>
  );
}
