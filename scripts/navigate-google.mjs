#!/usr/bin/env node
// navigate-google.mjs — Connect to JBrowser CDP page target and navigate to Google

import { WebSocket } from 'ws';
import { writeFileSync } from 'fs';

const CDP_TOKEN = 'jbr_cdp_QMxCC9zyHBGqhAMExUkdXw7iSB-FH2giAK55U0LlKfE';
const PAGE_WS_URL = `ws://localhost:8080/cdp/tenants/019e7c5c-2e23-7263-be9c-80dc8b5e72d9/browser-instances/019e7c5c-2ff5-7ad2-8f54-36340a6e1ec6/devtools/page/1E33E68E01FCA2DC8DAC6C98EB3085B6?token=${CDP_TOKEN}`;
const TIMEOUT = 15000;
const sleep = (ms) => new Promise(r => setTimeout(r, ms));

class CDP {
  #ws; #id = 0; #pending = new Map();

  async connect(wsUrl) {
    return new Promise((resolve, reject) => {
      this.#ws = new WebSocket(wsUrl);
      this.#ws.on('open', () => resolve());
      this.#ws.on('error', (e) => reject(new Error('WS error: ' + (e.message || e.type))));
      this.#ws.on('message', (data) => {
        const msg = JSON.parse(data.toString());
        if (msg.id && this.#pending.has(msg.id)) {
          const { resolve, reject } = this.#pending.get(msg.id);
          this.#pending.delete(msg.id);
          if (msg.error) reject(new Error(msg.error.message));
          else resolve(msg.result);
        }
      });
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

  close() { this.#ws?.close(); }
}

async function main() {
  console.log('🔗 Connecting to page CDP target...\n');

  const cdp = new CDP();
  await cdp.connect(PAGE_WS_URL);
  await sleep(1000);
  console.log('✅ Connected to page target');

  // 1. Navigate to Google
  console.log('\n🌐 Navigating to https://www.google.com ...');
  const navResult = await cdp.send('Page.navigate', { url: 'https://www.google.com' });
  if (navResult.errorText) throw new Error(`Navigation failed: ${navResult.errorText}`);
  console.log(`   frameId: ${navResult.frameId}`);

  // Wait for page load
  await sleep(4000);

  // 2. Verify
  const titleResult = await cdp.send('Runtime.evaluate', {
    expression: 'document.title',
    returnByValue: true,
  });
  console.log(`📄 Page title: "${titleResult.result.value}"`);

  const urlResult = await cdp.send('Runtime.evaluate', {
    expression: 'window.location.href',
    returnByValue: true,
  });
  console.log(`🔗 Page URL: ${urlResult.result.value}`);

  // 3. Screenshot
  console.log('\n📸 Taking screenshot...');
  const shot = await cdp.send('Page.captureScreenshot', { format: 'png' });
  const buf = Buffer.from(shot.data, 'base64');
  const path = '/root/workpath/open/jbrowser/google-screenshot.png';
  writeFileSync(path, buf);
  console.log(`   Saved: ${path} (${(buf.length / 1024).toFixed(1)} KB)`);

  // 4. Get page text snippet
  const textResult = await cdp.send('Runtime.evaluate', {
    expression: 'document.body?.innerText?.substring(0, 200) || "(empty)"',
    returnByValue: true,
  });
  console.log(`\n📝 Page text preview:\n   ${textResult.result.value.replace(/\n/g, '\n   ')}`);

  cdp.close();
  console.log('\n✅ Done! Successfully navigated to https://www.google.com');
}

main().catch(e => {
  console.error(`\n💥 Error: ${e.message}`);
  process.exit(1);
});
