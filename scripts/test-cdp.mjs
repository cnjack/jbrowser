#!/usr/bin/env node
// test-cdp.mjs — End-to-end CDP tunnel test for JBrowser
//
// Tests the full CDP tunnel chain:
//   CDP client → control-plane WS tunnel → agent → Chrome CDP
//
// Inspired by pasky/chrome-cdp-skill patterns:
//   list tabs, navigate, evaluate JS, take screenshot, click, type, accessibility tree
//
// Usage:
//   node scripts/test-cdp.mjs [base_url]
//   base_url defaults to http://localhost:8080

const BASE = process.argv[2] || 'http://localhost:8080';
const WS_BASE = BASE.replace('https://', 'wss://').replace('http://', 'ws://');
const TIMEOUT = 15000;

// ── Helpers ──────────────────────────────────────────────────────────────────

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

class CDP {
  #ws; #id = 0; #pending = new Map(); #eventHandlers = new Map();

  async connect(wsUrl) {
    return new Promise((resolve, reject) => {
      this.#ws = new WebSocket(wsUrl);
      this.#ws.onopen = () => resolve();
      this.#ws.onerror = (e) => reject(new Error('WebSocket error: ' + (e.message || e.type)));
      this.#ws.onmessage = (ev) => {
        const msg = JSON.parse(ev.data);
        if (msg.id && this.#pending.has(msg.id)) {
          const { resolve, reject } = this.#pending.get(msg.id);
          this.#pending.delete(msg.id);
          if (msg.error) reject(new Error(msg.error.message));
          else resolve(msg.result);
        } else if (msg.method && this.#eventHandlers.has(msg.method)) {
          for (const handler of [...this.#eventHandlers.get(msg.method)]) {
            handler(msg.params || {}, msg);
          }
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
          reject(new Error(`Timeout: ${method}`));
        }
      }, TIMEOUT);
    });
  }

  onEvent(method, handler) {
    if (!this.#eventHandlers.has(method)) this.#eventHandlers.set(method, new Set());
    this.#eventHandlers.get(method).add(handler);
    return () => this.#eventHandlers.get(method)?.delete(handler);
  }

  close() { this.#ws?.close(); }
}

let passed = 0;
let failed = 0;

function ok(name, detail) {
  passed++;
  console.log(`  ✅ ${name}${detail ? ' — ' + detail : ''}`);
}

function fail(name, err) {
  failed++;
  console.log(`  ❌ ${name} — ${err}`);
}

async function test(name, fn) {
  try {
    const result = await fn();
    ok(name, result);
  } catch (e) {
    fail(name, e.message);
  }
}

// ── Step 1: Login ────────────────────────────────────────────────────────────

async function login() {
  const res = await fetch(`${BASE}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: 'admin@example.com', password: 'jbrowser' }),
  });
  if (!res.ok) throw new Error(`Login failed: ${res.status}`);
  const data = await res.json();
  return {
    token: data.access_token,
    tenants: data.tenants,
  };
}

// ── Step 2: Get browser instances ────────────────────────────────────────────

async function getBrowsers(token, tenantId) {
  const res = await fetch(`${BASE}/api/v1/tenants/${tenantId}/browser-instances`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) throw new Error(`List browsers failed: ${res.status}`);
  const data = await res.json();
  return data.data;
}

// ── Step 3: Create CDP token ─────────────────────────────────────────────────

async function createCdpToken(token, tenantId) {
  const res = await fetch(`${BASE}/api/v1/tenants/${tenantId}/tokens/cdp`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ name: 'test-cdp-token' }),
  });
  if (!res.ok) throw new Error(`Create CDP token failed: ${res.status}`);
  const data = await res.json();
  return data.token;
}

// ── Step 4: CDP tunnel tests ─────────────────────────────────────────────────

async function testCdpJsonVersion(tenantId, browserId, cdpToken) {
  const res = await fetch(
    `${BASE}/cdp/tenants/${tenantId}/browser-instances/${browserId}/json/version?token=${cdpToken}`
  );
  if (!res.ok) throw new Error(`/json/version failed: ${res.status}`);
  return await res.json();
}

async function testCdpJsonList(tenantId, browserId, cdpToken) {
  const res = await fetch(
    `${BASE}/cdp/tenants/${tenantId}/browser-instances/${browserId}/json/list?token=${cdpToken}`
  );
  if (!res.ok) throw new Error(`/json/list failed: ${res.status}`);
  return await res.json();
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`\n🔧 JBrowser CDP Tunnel Test — ${BASE}\n`);

  // ── Auth & setup ───────────────────────────────────────────────────────
  console.log('── Auth & Setup ──');

  let auth;
  await test('Login', async () => {
    auth = await login();
    return `token=${auth.token.slice(0, 20)}…, tenants=${auth.tenants.length}`;
  });

  const tenantId = auth.tenants[0].id;
  let browsers;
  await test('List browsers', async () => {
    browsers = await getBrowsers(auth.token, tenantId);
    return `found ${browsers.length} browser(s)`;
  });

  // Find an online browser
  const browser = browsers.find(b => b.status === 'online') || browsers[0];
  if (!browser) throw new Error('No browser instances found');
  console.log(`  → Using browser: ${browser.id} (${browser.name}, status=${browser.status})`);

  let cdpToken;
  await test('Create CDP token', async () => {
    cdpToken = await createCdpToken(auth.token, tenantId);
    return `prefix=${cdpToken.slice(0, 12)}…`;
  });

  // ── CDP HTTP endpoints ─────────────────────────────────────────────────
  console.log('\n── CDP HTTP Endpoints ──');

  let versionInfo;
  await test('/json/version', async () => {
    versionInfo = await testCdpJsonVersion(tenantId, browser.id, cdpToken);
    return `Browser=${versionInfo.Browser}`;
  });

  let targets;
  await test('/json/list', async () => {
    targets = await testCdpJsonList(tenantId, browser.id, cdpToken);
    return `${targets.length} target(s): ${targets.map(t => t.url).join(', ')}`;
  });

  // ── CDP WebSocket tunnel ───────────────────────────────────────────────
  console.log('\n── CDP WebSocket Tunnel ──');

  const target = targets[0];
  if (!target) throw new Error('No CDP targets available');

  const tunnelWsUrl = `${WS_BASE}/cdp/tenants/${tenantId}/browser-instances/${browser.id}/devtools/page/${target.id}?token=${cdpToken}`;
  console.log(`  → Connecting: ${tunnelWsUrl.replace(cdpToken, cdpToken.slice(0, 12) + '…')}`);

  const cdp = new CDP();
  await test('WebSocket connect', async () => {
    await cdp.connect(tunnelWsUrl);
    // Give the agent a moment to open the CDP tunnel to Chrome
    await sleep(1000);
    return 'connected';
  });

  // ── chrome-cdp-skill style tests ───────────────────────────────────────
  console.log('\n── CDP Commands (chrome-cdp-skill patterns) ──');

  // 1. Navigate to a page
  await test('Page.navigate (→ example.com)', async () => {
    const result = await cdp.send('Page.navigate', { url: 'https://example.com' });
    if (result.errorText) throw new Error(result.errorText);
    await sleep(2000); // wait for page load
    return `frameId=${result.frameId}`;
  });

  // 2. Evaluate JS expression (like cdp.mjs eval)
  await test('Runtime.evaluate (document.title)', async () => {
    await cdp.send('Runtime.enable');
    const result = await cdp.send('Runtime.evaluate', {
      expression: 'document.title',
      returnByValue: true,
    });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
    return `title="${result.result.value}"`;
  });

  // 3. Get page URL
  await test('Runtime.evaluate (location.href)', async () => {
    const result = await cdp.send('Runtime.evaluate', {
      expression: 'window.location.href',
      returnByValue: true,
    });
    return `url="${result.result.value}"`;
  });

  // 4. Get page content (like cdp.mjs html)
  await test('Runtime.evaluate (document HTML length)', async () => {
    const result = await cdp.send('Runtime.evaluate', {
      expression: 'document.documentElement.outerHTML.length',
      returnByValue: true,
    });
    return `html length=${result.result.value} chars`;
  });

  // 5. Screenshot (like cdp.mjs shot)
  await test('Page.captureScreenshot', async () => {
    const result = await cdp.send('Page.captureScreenshot', { format: 'png' });
    const sizeKB = Math.round(result.data.length * 3 / 4 / 1024);
    return `screenshot ${sizeKB}KB (base64 len=${result.data.length})`;
  });

  // 6. Get layout metrics (used by chrome-cdp-skill for coordinate mapping)
  await test('Page.getLayoutMetrics', async () => {
    const result = await cdp.send('Page.getLayoutMetrics');
    const vp = result.cssVisualViewport || result.visualViewport || {};
    return `viewport ${vp.clientWidth}×${vp.clientHeight}`;
  });

  // 7. DOM tree (like cdp.mjs html with selector)
  await test('DOM.getDocument', async () => {
    const result = await cdp.send('DOM.getDocument', { depth: 2 });
    return `root nodeId=${result.root.nodeId}, children=${result.root.childNodeCount}`;
  });

  // 8. Accessibility tree (like cdp.mjs snap)
  await test('Accessibility.getFullAXTree', async () => {
    const result = await cdp.send('Accessibility.getFullAXTree');
    const pageNodes = result.nodes.filter(n => n.role?.value !== 'none');
    return `${result.nodes.length} nodes (${pageNodes.length} semantic)`;
  });

  // 9. Network info (like cdp.mjs net)
  await test('Runtime.evaluate (performance entries)', async () => {
    const result = await cdp.send('Runtime.evaluate', {
      expression: `JSON.stringify(performance.getEntriesByType('resource').map(e => ({
        name: e.name.substring(0, 80), type: e.initiatorType,
        duration: Math.round(e.duration), size: e.transferSize
      })))`,
      returnByValue: true,
    });
    const entries = JSON.parse(result.result.value);
    return `${entries.length} resource(s)`;
  });

  // 10. Mouse input (like cdp.mjs clickxy)
  await test('Input.dispatchMouseEvent (click at 100,100)', async () => {
    await cdp.send('Input.dispatchMouseEvent', {
      type: 'mousePressed', x: 100, y: 100, button: 'left', clickCount: 1,
    });
    await cdp.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased', x: 100, y: 100, button: 'left', clickCount: 1,
    });
    return 'click dispatched';
  });

  // 11. Keyboard input (like cdp.mjs type)
  await test('Input.insertText', async () => {
    // First click on the page body to focus it
    await cdp.send('Input.dispatchMouseEvent', {
      type: 'mousePressed', x: 300, y: 300, button: 'left', clickCount: 1,
    });
    await cdp.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased', x: 300, y: 300, button: 'left', clickCount: 1,
    });
    await cdp.send('Input.insertText', { text: 'Hello from JBrowser CDP test!' });
    return 'text inserted';
  });

  // 12. Navigate to another page and verify
  await test('Page.navigate (→ httpbin.org/get)', async () => {
    const result = await cdp.send('Page.navigate', { url: 'https://httpbin.org/get' });
    if (result.errorText) throw new Error(result.errorText);
    await sleep(3000);
    const titleResult = await cdp.send('Runtime.evaluate', {
      expression: 'document.title || document.body.innerText.substring(0, 80)',
      returnByValue: true,
    });
    return `loaded, content="${(titleResult.result.value || '').substring(0, 60)}"`;
  });

  // 13. Cookie / storage operations (like browser.reset patterns)
  await test('Network.getAllCookies', async () => {
    await cdp.send('Network.enable');
    const result = await cdp.send('Network.getAllCookies');
    return `${result.cookies.length} cookie(s)`;
  });

  // 14. Browser version info
  await test('Browser.getVersion', async () => {
    const result = await cdp.send('Browser.getVersion');
    return `${result.product}, protocol=${result.protocolVersion}`;
  });

  // 15. Emulation: device metrics (like chrome-cdp-skill coordinate mapping)
  await test('Emulation.setDeviceMetricsOverride', async () => {
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: 1280, height: 720, deviceScaleFactor: 1, mobile: false,
    });
    return 'viewport set to 1280×720 DPR=1';
  });

  // Final screenshot to confirm everything works
  await test('Final Page.captureScreenshot', async () => {
    const result = await cdp.send('Page.captureScreenshot', { format: 'jpeg', quality: 80 });
    const sizeKB = Math.round(result.data.length * 3 / 4 / 1024);
    return `final screenshot ${sizeKB}KB`;
  });

  cdp.close();

  // ── Summary ────────────────────────────────────────────────────────────
  console.log(`\n${'═'.repeat(50)}`);
  console.log(`  Results: ${passed} passed, ${failed} failed, ${passed + failed} total`);
  console.log(`${'═'.repeat(50)}\n`);

  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => {
  console.error(`\n💥 Fatal: ${e.message}`);
  process.exit(1);
});
