#!/usr/bin/env node
// cdp-proxy.mjs — Local CDP proxy for JBrowser remote browsers
//
// Makes a remote JBrowser browser instance look like a local Chrome on port 9222.
// Any CDP tool (chrome-cdp-skill, Puppeteer, Playwright, etc.) can connect to it.
//
// Usage:
//   node scripts/cdp-proxy.mjs <jbrowser_url> <tenant_id> <browser_id> <cdp_token> [local_port]
//
// Example:
//   node scripts/cdp-proxy.mjs http://localhost:8080 \
//     019e7c5c-2e23-7263-be9c-80dc8b5e72d9 \
//     019e7c5c-2ff5-7ad2-8f54-36340a6e1ec6 \
//     jbr_cdp_xxx 9222
//
// Then use any CDP tool pointing at localhost:9222:
//   CDP_HOST=127.0.0.1 scripts/cdp.mjs list

import http from 'http';
import { WebSocket, WebSocketServer } from 'ws';

const [,, BASE_URL, TENANT_ID, BROWSER_ID, CDP_TOKEN, PORT_STR] = process.argv;
const LOCAL_PORT = parseInt(PORT_STR || '9222', 10);

if (!BASE_URL || !TENANT_ID || !BROWSER_ID || !CDP_TOKEN) {
  console.error(`Usage: node cdp-proxy.mjs <jbrowser_url> <tenant_id> <browser_id> <cdp_token> [port]`);
  process.exit(1);
}

const REMOTE_BASE = `${BASE_URL}/cdp/tenants/${TENANT_ID}/browser-instances/${BROWSER_ID}`;
const WS_REMOTE_BASE = BASE_URL.replace('https://', 'wss://').replace('http://', 'ws://')
  + `/cdp/tenants/${TENANT_ID}/browser-instances/${BROWSER_ID}`;

// ── HTTP server: proxy /json/* endpoints ─────────────────────────────────────

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://localhost:${LOCAL_PORT}`);

  if (url.pathname === '/json/version' || url.pathname === '/json') {
    try {
      const remote = await fetch(`${REMOTE_BASE}/json/version?token=${CDP_TOKEN}`);
      const data = await remote.json();
      // Rewrite webSocketDebuggerUrl to point at local proxy
      if (data.webSocketDebuggerUrl) {
        const targetId = data.webSocketDebuggerUrl.match(/devtools\/browser\/([^?]+)/)?.[1] || 'browser';
        data.webSocketDebuggerUrl = `ws://127.0.0.1:${LOCAL_PORT}/devtools/browser/${targetId}`;
      }
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(data, null, 2));
    } catch (e) {
      res.writeHead(502, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: e.message }));
    }
    return;
  }

  if (url.pathname === '/json/list' || url.pathname === '/json/list/') {
    try {
      const remote = await fetch(`${REMOTE_BASE}/json/list?token=${CDP_TOKEN}`);
      const data = await remote.json();
      // Rewrite webSocketDebuggerUrl to point at local proxy
      for (const target of data) {
        if (target.webSocketDebuggerUrl) {
          target.webSocketDebuggerUrl = `ws://127.0.0.1:${LOCAL_PORT}/devtools/page/${target.id}`;
        }
        if (target.devtoolsFrontendUrl) {
          target.devtoolsFrontendUrl = `chrome-devtools://devtools/bundled/inspector.html?ws=127.0.0.1:${LOCAL_PORT}/devtools/page/${target.id}`;
        }
      }
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(data, null, 2));
    } catch (e) {
      res.writeHead(502, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: e.message }));
    }
    return;
  }

  // Fallback
  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'not found' }));
});

// ── WebSocket server: proxy CDP connections ──────────────────────────────────

const wss = new WebSocketServer({ server });

wss.on('connection', (clientWs, req) => {
  // Extract target type and ID from the path: /devtools/page/<id> or /devtools/browser/<id>
  const match = req.url.match(/\/devtools\/(page|browser)\/([^/?]+)/);
  if (!match) {
    clientWs.close(4000, 'invalid path');
    return;
  }
  const [, targetType, targetId] = match;
  const remoteWsUrl = `${WS_REMOTE_BASE}/devtools/${targetType}/${targetId}?token=${CDP_TOKEN}`;

  console.log(`[proxy] CDP tunnel → ${targetType}/${targetId.slice(0, 8)}…`);

  const remoteWs = new WebSocket(remoteWsUrl);
  let buffered = [];

  remoteWs.on('open', () => {
    console.log(`[proxy] remote connected`);
    // Flush buffered messages
    for (const msg of buffered) remoteWs.send(msg);
    buffered = null;
  });

  remoteWs.on('message', (data) => {
    if (clientWs.readyState === WebSocket.OPEN) {
      clientWs.send(data);
    }
  });

  remoteWs.on('close', () => {
    console.log(`[proxy] remote disconnected`);
    clientWs.close();
  });

  remoteWs.on('error', (e) => {
    console.error(`[proxy] remote error: ${e.message}`);
    clientWs.close(4002, 'remote error');
  });

  clientWs.on('message', (data) => {
    if (buffered) {
      buffered.push(data);
    } else if (remoteWs.readyState === WebSocket.OPEN) {
      remoteWs.send(data);
    }
  });

  clientWs.on('close', () => {
    remoteWs.close();
  });
});

// ── Start ────────────────────────────────────────────────────────────────────

server.listen(LOCAL_PORT, '127.0.0.1', () => {
  console.log(`\n🔗 JBrowser CDP Proxy running on http://127.0.0.1:${LOCAL_PORT}`);
  console.log(`   Remote: ${REMOTE_BASE}`);
  console.log(`\n   Endpoints:`);
  console.log(`     http://127.0.0.1:${LOCAL_PORT}/json/version`);
  console.log(`     http://127.0.0.1:${LOCAL_PORT}/json/list`);
  console.log(`     ws://127.0.0.1:${LOCAL_PORT}/devtools/page/<targetId>`);
  console.log(`\n   Usage with chrome-cdp-skill:`);
  console.log(`     CDP_PORT_FILE=/tmp/jbrowser-cdp-port node skills/chrome-cdp/scripts/cdp.mjs list`);
  console.log('');

  // Write a fake DevToolsActivePort file so chrome-cdp-skill can find us
  const { writeFileSync } = await import('fs');
  const portFileContent = `${LOCAL_PORT}\n/devtools/browser/jbrowser`;
  const portFilePath = '/tmp/jbrowser-cdp-port';
  writeFileSync(portFilePath, portFileContent);
  console.log(`   DevToolsActivePort written to ${portFilePath}`);
  console.log(`   Press Ctrl+C to stop\n`);
});
