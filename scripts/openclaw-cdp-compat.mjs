#!/usr/bin/env node
// End-to-end JBrowser CDP compatibility tests for OpenClaw-style browser usage.
//
// Coverage:
// - Standard Chrome CDP HTTP endpoints and browser/page WebSocket commands.
// - OpenClaw persistent Playwright flow: connectOverCDP, newPage, list, focus, close.
// - OpenClaw raw CDP fallback flow: Target.createTarget, attach, prepare page session.
// - Edge cases: invalid token, missing browser instance, no page targets, agent offline.
//
// By default this script owns setup and teardown via docker compose.
// Usage:
//   node scripts/openclaw-cdp-compat.mjs
//   node scripts/openclaw-cdp-compat.mjs --keep
//   node scripts/openclaw-cdp-compat.mjs --no-setup --no-teardown --base http://localhost:8080

import { createRequire } from 'node:module';
import { mkdtemp, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const openclawRepo = process.env.OPENCLAW_REPO || '/Users/jack/workpath/opensource/openclaw';
const composeFile = path.join(repoRoot, 'docker', 'docker-compose.yml');

const args = new Set(process.argv.slice(2));
const getArgValue = (name, fallback) => {
  const prefix = `${name}=`;
  const direct = process.argv.find((arg) => arg.startsWith(prefix));
  if (direct) return direct.slice(prefix.length);
  const idx = process.argv.indexOf(name);
  if (idx >= 0 && process.argv[idx + 1]) return process.argv[idx + 1];
  return fallback;
};

const BASE = getArgValue('--base', process.env.JBROWSER_BASE || 'http://localhost:8080');
const WS_BASE = BASE.replace('https://', 'wss://').replace('http://', 'ws://');
const setupEnabled = !args.has('--no-setup');
const teardownEnabled = !args.has('--no-teardown') && !args.has('--keep');
const skipPlaywright = args.has('--skip-playwright');
const TIMEOUT_MS = Number(getArgValue('--timeout-ms', process.env.CDP_TEST_TIMEOUT_MS || '15000'));

let passed = 0;
let failed = 0;
let tempDepsDir = null;
let WebSocketImpl = globalThis.WebSocket;
const createdTokenIds = [];
const createdTargetIds = [];

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function redact(text) {
  return String(text).replace(/jbr_(?:cdp|agent|reg)_[A-Za-z0-9_-]+/g, (match) => `${match.slice(0, 12)}...`);
}

function run(command, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: opts.cwd || repoRoot,
      stdio: opts.capture ? ['ignore', 'pipe', 'pipe'] : 'inherit',
      env: { ...process.env, ...(opts.env || {}) },
    });
    let stdout = '';
    let stderr = '';
    if (opts.capture) {
      child.stdout.on('data', (chunk) => { stdout += chunk; });
      child.stderr.on('data', (chunk) => { stderr += chunk; });
    }
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`${command} ${args.join(' ')} exited ${code}${stderr ? `\n${stderr}` : ''}`));
      }
    });
  });
}

async function dockerCompose(...composeArgs) {
  return await run('docker', ['compose', '-f', composeFile, ...composeArgs], { cwd: repoRoot });
}

async function waitFor(name, fn, { timeoutMs = 60000, intervalMs = 500 } = {}) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (err) {
      lastError = err;
    }
    await sleep(intervalMs);
  }
  throw new Error(`${name} timed out${lastError ? `: ${lastError.message}` : ''}`);
}

async function httpJson(url, opts = {}) {
  const res = await fetch(url, opts);
  const text = await res.text();
  let data = null;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }
  }
  return { res, data, text };
}

async function expectHttpJson(url, opts = {}) {
  const out = await httpJson(url, opts);
  if (!out.res.ok) {
    throw new Error(`${url} returned ${out.res.status}: ${redact(out.text.slice(0, 500))}`);
  }
  return out.data;
}

async function expectHttpOk(url, opts = {}) {
  const out = await httpJson(url, opts);
  if (!out.res.ok) {
    throw new Error(`${url} returned ${out.res.status}: ${redact(out.text.slice(0, 500))}`);
  }
  return out.text;
}

class CDP {
  #ws;
  #id = 0;
  #pending = new Map();
  #events = new Map();

  async connect(wsUrl, { timeoutMs = TIMEOUT_MS } = {}) {
    const WebSocketCtor = await installWebSocket();
    return await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`WebSocket connect timeout: ${redact(wsUrl)}`)), timeoutMs);
      this.#ws = new WebSocketCtor(wsUrl);
      this.#ws.onopen = () => {
        clearTimeout(timer);
        resolve();
      };
      this.#ws.onerror = () => {
        clearTimeout(timer);
        reject(new Error(`WebSocket error: ${redact(wsUrl)}`));
      };
      this.#ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        if (msg.id && this.#pending.has(msg.id)) {
          const entry = this.#pending.get(msg.id);
          this.#pending.delete(msg.id);
          clearTimeout(entry.timer);
          if (msg.error) entry.reject(new Error(msg.error.message || JSON.stringify(msg.error)));
          else entry.resolve(msg.result);
          return;
        }
        if (msg.method && this.#events.has(msg.method)) {
          for (const handler of this.#events.get(msg.method)) handler(msg.params || {}, msg);
        }
      };
    });
  }

  send(method, params = undefined, sessionId = undefined, { timeoutMs = TIMEOUT_MS } = {}) {
    const id = ++this.#id;
    const payload = { id, method };
    if (params !== undefined) payload.params = params;
    if (sessionId !== undefined) payload.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (this.#pending.has(id)) {
          this.#pending.delete(id);
          reject(new Error(`Timeout: ${method}`));
        }
      }, timeoutMs);
      this.#pending.set(id, { resolve, reject, timer });
      this.#ws.send(JSON.stringify(payload));
    });
  }

  on(method, handler) {
    if (!this.#events.has(method)) this.#events.set(method, new Set());
    this.#events.get(method).add(handler);
  }

  close() {
    try {
      this.#ws?.close();
    } catch {
      // ignore cleanup
    }
  }
}

async function test(name, fn) {
  const start = Date.now();
  try {
    const detail = await fn();
    passed += 1;
    console.log(`  PASS ${name}${detail ? ` - ${detail}` : ''} (${Date.now() - start}ms)`);
  } catch (err) {
    failed += 1;
    console.log(`  FAIL ${name} - ${err.message} (${Date.now() - start}ms)`);
  }
}

async function setupDocker() {
  console.log('== Setup Docker environment ==');
  await dockerCompose('--profile', 'agent', 'down', '-v', '--remove-orphans');
  await dockerCompose('up', '--build', '-d', 'mysql', 'control');
  await waitFor('control API', async () => {
    const res = await fetch(`${BASE}/api/v1/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email: 'admin@example.com', password: 'jbrowser' }),
    }).catch(() => null);
    return res?.status === 200;
  }, { timeoutMs: 90000, intervalMs: 1000 });
  await dockerCompose('--profile', 'agent', 'up', '--build', '-d', 'agent-chromium');
}

async function teardownDocker() {
  console.log('== Teardown Docker environment ==');
  await dockerCompose('--profile', 'agent', 'down', '-v', '--remove-orphans').catch((err) => {
    console.log(`  WARN docker teardown failed: ${err.message}`);
  });
  if (tempDepsDir) {
    await rm(tempDepsDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function login() {
  const data = await expectHttpJson(`${BASE}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: 'admin@example.com', password: 'jbrowser' }),
  });
  return { token: data.access_token, tenantId: data.tenants[0].id };
}

async function listBrowsers(jwt, tenantId) {
  const data = await expectHttpJson(`${BASE}/api/v1/tenants/${tenantId}/browser-instances`, {
    headers: { Authorization: `Bearer ${jwt}` },
  });
  return data.data || [];
}

async function waitForOnlineBrowser(jwt, tenantId, previousId = null) {
  return await waitFor('online browser', async () => {
    const browsers = await listBrowsers(jwt, tenantId);
    const candidate = previousId
      ? browsers.find((b) => b.id === previousId && String(b.status).toLowerCase() === 'online')
      : browsers.find((b) => String(b.status).toLowerCase() === 'online');
    return candidate || null;
  }, { timeoutMs: 90000, intervalMs: 1000 });
}

async function createCdpToken(jwt, tenantId) {
  const data = await expectHttpJson(`${BASE}/api/v1/tenants/${tenantId}/tokens/cdp`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${jwt}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ name: `compat-${Date.now()}` }),
  });
  if (data.data?.id) createdTokenIds.push(data.data.id);
  return data.token;
}

async function revokeCreatedTokens(jwt, tenantId) {
  for (const tokenId of createdTokenIds.splice(0)) {
    await fetch(`${BASE}/api/v1/tenants/${tenantId}/tokens/cdp/${tokenId}/revoke`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${jwt}` },
    }).catch(() => {});
  }
}

function cdpBaseUrl(tenantId, browserId, cdpToken) {
  return `${BASE}/cdp/tenants/${tenantId}/browser-instances/${browserId}?token=${encodeURIComponent(cdpToken)}`;
}

function cdpEndpoint(tenantId, browserId, pathSuffix, cdpToken) {
  const separator = pathSuffix.includes('?') ? '&' : '?';
  return `${BASE}/cdp/tenants/${tenantId}/browser-instances/${browserId}${pathSuffix}${separator}token=${encodeURIComponent(cdpToken)}`;
}

function wsEndpoint(tenantId, browserId, kind, targetId, cdpToken) {
  return `${WS_BASE}/cdp/tenants/${tenantId}/browser-instances/${browserId}/devtools/${kind}/${targetId}?token=${encodeURIComponent(cdpToken)}`;
}

async function connectBrowserCdp(tenantId, browserId, cdpToken) {
  const version = await expectHttpJson(cdpEndpoint(tenantId, browserId, '/json/version', cdpToken));
  const cdp = new CDP();
  await cdp.connect(version.webSocketDebuggerUrl);
  return { cdp, version };
}

async function connectFirstPageCdp(tenantId, browserId, cdpToken) {
  const targets = await expectHttpJson(cdpEndpoint(tenantId, browserId, '/json/list', cdpToken));
  const page = targets.find((target) => target.type === 'page') || targets[0];
  if (!page) throw new Error('no page target available');
  const cdp = new CDP();
  await cdp.connect(page.webSocketDebuggerUrl || wsEndpoint(tenantId, browserId, 'page', page.id, cdpToken));
  return { cdp, page };
}

async function closeAllPagesViaBrowserCdp(tenantId, browserId, cdpToken) {
  const { cdp } = await connectBrowserCdp(tenantId, browserId, cdpToken);
  try {
    const targets = await cdp.send('Target.getTargets');
    const pages = (targets.targetInfos || []).filter((target) => target.type === 'page');
    for (const page of pages) {
      await cdp.send('Target.closeTarget', { targetId: page.targetId }).catch(() => {});
    }
  } finally {
    cdp.close();
  }
}

async function installPlaywrightCore() {
  if (skipPlaywright) return null;
  const candidates = [
    process.env.PLAYWRIGHT_CORE_REQUIRE_FROM,
    repoRoot,
    openclawRepo,
    path.join(openclawRepo, 'extensions', 'browser'),
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      const require = createRequire(path.join(candidate, 'package.json'));
      return require('playwright-core');
    } catch {
      // try next
    }
  }

  tempDepsDir = await mkdtemp(path.join(tmpdir(), 'jbrowser-cdp-compat-'));
  await run('npm', ['install', '--silent', '--prefix', tempDepsDir, 'playwright-core@1.60.0'], {
    capture: true,
  });
  const require = createRequire(path.join(tempDepsDir, 'package.json'));
  return require('playwright-core');
}

async function installWebSocket() {
  if (WebSocketImpl) return WebSocketImpl;

  const candidates = [
    process.env.WS_REQUIRE_FROM,
    repoRoot,
    openclawRepo,
    path.join(openclawRepo, 'extensions', 'browser'),
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      const require = createRequire(path.join(candidate, 'package.json'));
      const mod = require('ws');
      WebSocketImpl = mod.WebSocket || mod.default || mod;
      return WebSocketImpl;
    } catch {
      // try next
    }
  }

  tempDepsDir ??= await mkdtemp(path.join(tmpdir(), 'jbrowser-cdp-compat-'));
  await run('npm', ['install', '--silent', '--prefix', tempDepsDir, 'ws@8.18.3'], {
    capture: true,
  });
  const require = createRequire(path.join(tempDepsDir, 'package.json'));
  const mod = require('ws');
  WebSocketImpl = mod.WebSocket || mod.default || mod;
  return WebSocketImpl;
}

async function runStandardCdpTests(ctx) {
  console.log('== Standard CDP compatibility ==');
  await test('/json/version returns browser websocket', async () => {
    const version = await expectHttpJson(cdpEndpoint(ctx.tenantId, ctx.browser.id, '/json/version', ctx.cdpToken));
    if (!version.webSocketDebuggerUrl?.includes('/devtools/browser/browser')) {
      throw new Error(`unexpected browser websocket URL: ${redact(version.webSocketDebuggerUrl)}`);
    }
    return version.Browser;
  });

  await test('/json/list returns page targets', async () => {
    const targets = await expectHttpJson(cdpEndpoint(ctx.tenantId, ctx.browser.id, '/json/list', ctx.cdpToken));
    if (!Array.isArray(targets)) throw new Error('json/list did not return an array');
    if (!targets.some((target) => target.type === 'page')) throw new Error('no page target in json/list');
    return `${targets.length} target(s)`;
  });

  await test('browser WS immediate Browser.getVersion', async () => {
    const { cdp } = await connectBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    try {
      const result = await cdp.send('Browser.getVersion');
      if (!result.product) throw new Error('missing product');
      return result.product;
    } finally {
      cdp.close();
    }
  });

  await test('browser WS Target.createTarget and attach prepare session', async () => {
    const { cdp } = await connectBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    try {
      const created = await cdp.send('Target.createTarget', { url: 'about:blank' });
      if (!created.targetId) throw new Error('Target.createTarget returned no targetId');
      createdTargetIds.push(created.targetId);
      const attached = await cdp.send('Target.attachToTarget', {
        targetId: created.targetId,
        flatten: true,
      });
      if (!attached.sessionId) throw new Error('Target.attachToTarget returned no sessionId');
      await cdp.send('Page.enable', undefined, attached.sessionId);
      await cdp.send('Runtime.enable', undefined, attached.sessionId);
      await cdp.send('Network.enable', undefined, attached.sessionId);
      await cdp.send('DOM.enable', undefined, attached.sessionId);
      await cdp.send('Accessibility.enable', undefined, attached.sessionId);
      const evalResult = await cdp.send('Runtime.evaluate', {
        expression: 'document.location.href',
        returnByValue: true,
      }, attached.sessionId);
      await cdp.send('Target.detachFromTarget', { sessionId: attached.sessionId }).catch(() => {});
      return `${created.targetId.slice(0, 8)} ${evalResult.result?.value}`;
    } finally {
      cdp.close();
    }
  });

  await test('page WS navigate/evaluate/screenshot/AX/input/cookies', async () => {
    const { cdp } = await connectFirstPageCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    try {
      await cdp.send('Page.enable');
      await cdp.send('Runtime.enable');
      const nav = await cdp.send('Page.navigate', { url: 'https://example.com' });
      if (nav.errorText) throw new Error(nav.errorText);
      await sleep(1500);
      const title = await cdp.send('Runtime.evaluate', {
        expression: 'document.title',
        returnByValue: true,
      });
      const metrics = await cdp.send('Page.getLayoutMetrics');
      await cdp.send('DOM.getDocument', { depth: 2 });
      await cdp.send('Accessibility.getFullAXTree');
      await cdp.send('Input.dispatchMouseEvent', {
        type: 'mousePressed',
        x: 100,
        y: 100,
        button: 'left',
        clickCount: 1,
      });
      await cdp.send('Input.dispatchMouseEvent', {
        type: 'mouseReleased',
        x: 100,
        y: 100,
        button: 'left',
        clickCount: 1,
      });
      await cdp.send('Network.enable');
      await cdp.send('Network.getAllCookies');
      const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
      if (!shot.data) throw new Error('screenshot missing data');
      return `${title.result?.value}; viewport=${metrics.cssVisualViewport?.clientWidth}x${metrics.cssVisualViewport?.clientHeight}`;
    } finally {
      cdp.close();
    }
  });

  await test('OpenClaw HTTP fallback /json/new, /json/activate, /json/close', async () => {
    const url = cdpEndpoint(
      ctx.tenantId,
      ctx.browser.id,
      `/json/new?${encodeURIComponent('about:blank')}`,
      ctx.cdpToken,
    );
    const created = await expectHttpJson(url, { method: 'PUT' });
    if (!created.id) throw new Error('/json/new returned no id');
    await expectHttpOk(cdpEndpoint(ctx.tenantId, ctx.browser.id, `/json/activate/${created.id}`, ctx.cdpToken));
    await expectHttpOk(cdpEndpoint(ctx.tenantId, ctx.browser.id, `/json/close/${created.id}`, ctx.cdpToken));
    return created.id.slice(0, 8);
  });
}

async function runPlaywrightOpenClawTests(ctx) {
  console.log('== OpenClaw Playwright compatibility ==');
  const pw = await installPlaywrightCore();
  if (!pw) {
    await test('playwright tests skipped', async () => 'skipped by --skip-playwright');
    return;
  }

  await test('connectOverCDP newPage/list/focus/close', async () => {
    const version = await expectHttpJson(cdpEndpoint(ctx.tenantId, ctx.browser.id, '/json/version', ctx.cdpToken));
    const browser = await pw.chromium.connectOverCDP(version.webSocketDebuggerUrl, {
      timeout: 15000,
    });
    try {
      const context = browser.contexts()[0] || await browser.newContext();
      const page = await context.newPage();
      await page.goto('data:text/html,<title>jbrowser-playwright</title><h1 id="ok">ok</h1>');
      const title = await page.title();
      if (title !== 'jbrowser-playwright') throw new Error(`unexpected title: ${title}`);
      const pages = context.pages();
      if (!pages.includes(page)) throw new Error('created page not in context pages');
      await page.bringToFront();
      await page.close();
      return `${pages.length} page(s) before close`;
    } finally {
      await browser.close().catch(() => {});
    }
  });
}

async function runEdgeCaseTests(ctx) {
  console.log('== Edge cases ==');
  await test('invalid CDP token returns 401', async () => {
    const { res } = await httpJson(cdpEndpoint(ctx.tenantId, ctx.browser.id, '/json/version', 'jbr_cdp_invalid'));
    if (res.status !== 401) throw new Error(`expected 401, got ${res.status}`);
    return '401';
  });

  await test('missing browser instance returns 404', async () => {
    const missing = '019e80e9-ffff-7fff-ffff-ffffffffffff';
    const { res } = await httpJson(cdpEndpoint(ctx.tenantId, missing, '/json/version', ctx.cdpToken));
    if (res.status !== 404) throw new Error(`expected 404, got ${res.status}`);
    return '404';
  });

  await test('browser websocket works with all page targets closed', async () => {
    await closeAllPagesViaBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    await sleep(500);
    const { cdp } = await connectBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    try {
      const version = await cdp.send('Browser.getVersion');
      const created = await cdp.send('Target.createTarget', { url: 'about:blank' });
      if (!created.targetId) throw new Error('could not create page after closing all pages');
      createdTargetIds.push(created.targetId);
      return version.product;
    } finally {
      cdp.close();
    }
  });

  await test('agent offline makes CDP websocket unusable and recovers with same browser id', async () => {
    await dockerCompose('stop', 'agent-chromium');
    await waitFor('agent offline', async () => {
      const browsers = await listBrowsers(ctx.jwt, ctx.tenantId);
      const current = browsers.find((b) => b.id === ctx.browser.id);
      return current && String(current.status).toLowerCase() !== 'online';
    }, { timeoutMs: 30000, intervalMs: 1000 });

    const version = await expectHttpJson(cdpEndpoint(ctx.tenantId, ctx.browser.id, '/json/version', ctx.cdpToken));
    const offlineCdp = new CDP();
    let failedAsExpected = false;
    try {
      await offlineCdp.connect(version.webSocketDebuggerUrl, { timeoutMs: 3000 });
      await offlineCdp.send('Browser.getVersion', undefined, undefined, { timeoutMs: 3000 });
    } catch {
      failedAsExpected = true;
    } finally {
      offlineCdp.close();
    }
    if (!failedAsExpected) throw new Error('CDP websocket unexpectedly worked while agent offline');

    await dockerCompose('start', 'agent-chromium');
    const recovered = await waitForOnlineBrowser(ctx.jwt, ctx.tenantId, ctx.browser.id);
    ctx.browser = recovered;
    const { cdp } = await connectBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken);
    try {
      const result = await cdp.send('Browser.getVersion');
      return `recovered ${ctx.browser.id} ${result.product}`;
    } finally {
      cdp.close();
    }
  });
}

async function cleanupCreatedTargets(ctx) {
  if (!ctx?.cdpToken || !ctx?.browser?.id) return;
  const { cdp } = await connectBrowserCdp(ctx.tenantId, ctx.browser.id, ctx.cdpToken).catch(() => ({ cdp: null }));
  if (!cdp) return;
  try {
    for (const targetId of createdTargetIds.splice(0)) {
      await cdp.send('Target.closeTarget', { targetId }).catch(() => {});
    }
  } finally {
    cdp.close();
  }
}

async function main() {
  const ctx = {};
  try {
    if (setupEnabled) {
      await setupDocker();
    }

    console.log('== Auth and discovery ==');
    const auth = await login();
    ctx.jwt = auth.token;
    ctx.tenantId = auth.tenantId;
    ctx.browser = await waitForOnlineBrowser(ctx.jwt, ctx.tenantId);
    ctx.cdpToken = await createCdpToken(ctx.jwt, ctx.tenantId);
    console.log(`  tenant=${ctx.tenantId}`);
    console.log(`  browser=${ctx.browser.id} status=${ctx.browser.status}`);
    console.log(`  cdpBase=${redact(cdpBaseUrl(ctx.tenantId, ctx.browser.id, ctx.cdpToken))}`);

    await runStandardCdpTests(ctx);
    await runPlaywrightOpenClawTests(ctx);
    await runEdgeCaseTests(ctx);

    await cleanupCreatedTargets(ctx);
    await revokeCreatedTokens(ctx.jwt, ctx.tenantId);
  } finally {
    if (teardownEnabled) {
      await teardownDocker();
    }
  }

  console.log(`== Results: ${passed} passed, ${failed} failed ==`);
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(async (err) => {
  console.error(`FATAL ${err.stack || err.message}`);
  if (teardownEnabled) await teardownDocker();
  process.exit(1);
});
