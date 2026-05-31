import { DocsLayout } from './DocsLayout';

export function DocsOverview({ tenantId }: { tenantId?: string }) {
  return (
    <DocsLayout slug="" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Documentation</p>
          <h1 className="page-title">JBrowser Documentation</h1>
          <p className="page-subtitle">
            Cloud-hosted headless Chrome — connect via CDP, automate with AI agents.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">What is JBrowser?</h2>
        <p>
          JBrowser gives you managed headless Chrome instances accessible via the standard
          Chrome DevTools Protocol (CDP). Deploy agents, connect your automation tools
          or AI agents, and let JBrowser handle browser lifecycle, scaling, and security.
        </p>

        <div className="docs-cards">
          <div className="docs-card">
            <h3>For Automation Engineers</h3>
            <p>
              Connect Puppeteer, Playwright, or any CDP client to cloud browsers.
              No local Chrome installation needed.
            </p>
            <a href={tenantId ? `/tenants/${tenantId}/docs/guides/cdp-connect` : '/docs/guides/cdp-connect'} className="docs-link">
              CDP Connection Guide →
            </a>
          </div>
          <div className="docs-card">
            <h3>For AI Agent Developers</h3>
            <p>
              Give your AI agent (Claude, GPT, custom) a real browser.
              Navigate, screenshot, click, type — all via CDP.
            </p>
            <a href={tenantId ? `/tenants/${tenantId}/docs/guides/ai-agents` : '/docs/guides/ai-agents'} className="docs-link">
              AI Agent Integration →
            </a>
          </div>
          <div className="docs-card">
            <h3>For Platform Operators</h3>
            <p>
              Deploy and scale browser agents with Docker or Kubernetes.
              Multi-tenant isolation out of the box.
            </p>
            <a href={tenantId ? `/tenants/${tenantId}/docs/guides/deploy-agent` : '/docs/guides/deploy-agent'} className="docs-link">
              Deploy an Agent →
            </a>
          </div>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Key Concepts</h2>
        <div className="docs-concepts">
          <dl>
            <dt>Control Plane</dt>
            <dd>
              The central server that manages agents, browser instances, authentication,
              and proxies CDP connections. Your automation tools connect here.
            </dd>
            <dt>Agent</dt>
            <dd>
              A container running headless Chrome. Agents connect outbound to the control
              plane — no inbound ports needed. Deploy as many as you need.
            </dd>
            <dt>Browser Instance</dt>
            <dd>
              A managed Chrome browser running inside an agent. Each browser has its own
              tabs, cookies, and viewport. Accessible via CDP.
            </dd>
            <dt>CDP Token</dt>
            <dd>
              A tenant-scoped API key that grants CDP access to all browsers in the tenant.
              Used as a query parameter (<code>?token=...</code>) on CDP endpoints.
            </dd>
          </dl>
        </div>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Quick Links</h2>
        <ul className="docs-quick-links">
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/quickstart` : '/docs/quickstart'}>5-Minute Quickstart</a></li>
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/api/cdp` : '/docs/api/cdp'}>CDP Endpoint Reference</a></li>
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/api/rest` : '/docs/api/rest'}>REST API Reference</a></li>
          <li><a href={tenantId ? `/tenants/${tenantId}/docs/concepts/security` : '/docs/concepts/security'}>Security &amp; Token Types</a></li>
        </ul>
      </section>
    </DocsLayout>
  );
}
