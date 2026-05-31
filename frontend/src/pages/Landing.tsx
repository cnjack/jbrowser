import { Globe, ArrowRight, Terminal, Shield, Zap, Layers, Monitor, Code } from 'lucide-react';

export function Landing() {
  return (
    <div className="landing">
      {/* ── Nav ── */}
      <nav className="landing-nav">
        <div className="landing-nav-inner">
          <a href="/" className="brand-link">
            <span className="brand-icon"><Globe size={18} strokeWidth={2.2} /></span>
            <span className="brand-name">
              <span className="brand-bracket">[</span>
              <span className="brand-j">J</span>
              <span className="brand-text">Browser</span>
              <span className="brand-bracket">]</span>
            </span>
          </a>
          <div className="landing-nav-links">
            <a href="#features">Features</a>
            <a href="#how-it-works">How it works</a>
            <a href="/docs">Docs</a>
          </div>
          <div className="landing-nav-actions">
            <a href="/login" className="landing-btn-ghost">Log In</a>
            <a href="/login" className="landing-btn-primary">Sign Up <ArrowRight size={14} /></a>
          </div>
        </div>
      </nav>

      {/* ── Hero ── */}
      <section className="landing-hero">
        <div className="landing-hero-content">
          <div className="landing-hero-left">
            <h1 className="landing-headline">
              Run headless Chrome<br />
              <span className="landing-headline-accent">in production</span> without<br />
              the maintenance
            </h1>
            <p className="landing-subtitle">
              Your Puppeteer and Playwright scripts stay the same. We handle
              the browsers, Chrome updates, and everything that breaks at scale.
            </p>
            <div className="landing-cta-group">
              <a href="/login" className="landing-btn-primary landing-btn-lg">
                Get your API key <ArrowRight size={16} />
              </a>
              <a href="/docs" className="landing-btn-outline landing-btn-lg">
                Read the Docs
              </a>
            </div>
            <p className="landing-fine-print">
              Self-hosted. Open source. Full control.
            </p>
          </div>
          <div className="landing-hero-right">
            <div className="landing-browser-demo">
              <div className="landing-demo-bar">
                <input
                  className="landing-demo-input"
                  type="text"
                  defaultValue="https://www.google.com"
                  readOnly
                />
                <button className="landing-demo-run" type="button">
                  Run <span className="landing-demo-play">▶</span>
                </button>
              </div>
              <div className="landing-demo-preview">
                <div className="landing-demo-badge">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><path d="M20 6L9 17l-5-5" /></svg>
                  Success
                </div>
                <div className="landing-demo-mockup">
                  <div className="landing-mock-bar">
                    <div className="landing-mock-dots">
                      <span /><span /><span />
                    </div>
                    <div className="landing-mock-url">google.com</div>
                  </div>
                  <div className="landing-mock-body">
                    <div className="landing-mock-logo">Google</div>
                    <div className="landing-mock-search" />
                    <div className="landing-mock-btns">
                      <span>Google Search</span>
                      <span>I'm Feeling Lucky</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── Features ── */}
      <section className="landing-section" id="features">
        <div className="landing-section-inner">
          <p className="eyebrow">Why JBrowser</p>
          <h2 className="landing-section-title">
            Everything you need to run browsers at scale
          </h2>
          <div className="landing-features-grid">
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Zap size={22} /></div>
              <h3>CDP in milliseconds</h3>
              <p>Connect via Chrome DevTools Protocol. Your Puppeteer, Playwright, or custom CDP client works out of the box.</p>
            </div>
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Monitor size={22} /></div>
              <h3>Real-time preview</h3>
              <p>Watch what the browser sees in real time. Click, type, scroll — interact with the remote browser as if it were local.</p>
            </div>
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Layers size={22} /></div>
              <h3>Multi-tenant</h3>
              <p>Isolate teams with tenant-scoped access. Every API call, CDP connection, and action is audit-logged.</p>
            </div>
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Shield size={22} /></div>
              <h3>Zero inbound ports</h3>
              <p>Agents connect outbound only. No public ports, no firewall holes. CDP binds to localhost inside the container.</p>
            </div>
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Terminal size={22} /></div>
              <h3>Self-hosted</h3>
              <p>Deploy on your infrastructure with Docker Compose or Helm. Your data never leaves your network.</p>
            </div>
            <div className="landing-feature-card">
              <div className="landing-feature-icon"><Code size={22} /></div>
              <h3>Multi-tab management</h3>
              <p>Open, close, switch, and navigate tabs. Real-time preview follows the active tab automatically.</p>
            </div>
          </div>
        </div>
      </section>

      {/* ── How it works ── */}
      <section className="landing-section landing-section-warm" id="how-it-works">
        <div className="landing-section-inner">
          <p className="eyebrow">How it works</p>
          <h2 className="landing-section-title">Three steps to production browsers</h2>
          <div className="landing-steps">
            <div className="landing-step">
              <div className="landing-step-num">01</div>
              <h3>Deploy an agent</h3>
              <p>Run the agent container alongside Chrome. It connects to your control plane automatically.</p>
              <div className="landing-code-block">
                <code>
                  <span className="code-comment"># docker-compose.yml</span>{'\n'}
                  <span className="code-key">services</span>:{'\n'}
                  {'  '}<span className="code-key">agent</span>:{'\n'}
                  {'    '}<span className="code-key">image</span>: ghcr.io/cnjack/jbrowser-agent-chromium:latest{'\n'}
                  {'    '}<span className="code-key">environment</span>:{'\n'}
                  {'      '}- CONTROL_PLANE_URL=wss://control.example.com{'\n'}
                  {'      '}- REGISTRATION_TOKEN=${'${TOKEN}'}
                </code>
              </div>
            </div>
            <div className="landing-step">
              <div className="landing-step-num">02</div>
              <h3>Get your API key</h3>
              <p>Create a CDP access token from the dashboard. Use it to connect any CDP-compatible tool.</p>
              <div className="landing-code-block">
                <code>
                  <span className="code-comment"># Create a CDP token</span>{'\n'}
                  curl -X POST /api/v1/tenants/${'${TENANT}'}/tokens/cdp \{'\n'}
                  {'  '}-H &quot;Authorization: Bearer ${'${JWT}'}&quot; \{'\n'}
                  {'  '}-d &apos;{'{"name": "production"}'}&apos;
                </code>
              </div>
            </div>
            <div className="landing-step">
              <div className="landing-step-num">03</div>
              <h3>Connect & automate</h3>
              <p>Point Puppeteer or Playwright at your CDP endpoint. Everything just works.</p>
              <div className="landing-code-block">
                <code>
                  <span className="code-comment">// Connect with Puppeteer</span>{'\n'}
                  <span className="code-key">const</span> browser = <span className="code-key">await</span> puppeteer.connect({'{'}{'\n'}
                  {'  '}browserWSEndpoint:{'\n'}
                  {'    '}<span className="code-string">`wss://control.example.com/cdp/tenants/</span>{'\n'}
                  {'     '}<span className="code-string">${'{'}tenantId{'}'}/browser-instances/${'{'} id{'}'}`</span>{'\n'}
                  {'}'});
                </code>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ── CTA ── */}
      <section className="landing-section landing-section-cta">
        <div className="landing-section-inner" style={{ textAlign: 'center' }}>
          <h2 className="landing-section-title">Ready to stop fighting browsers?</h2>
          <p style={{ color: 'var(--muted)', maxWidth: 480, margin: '0 auto 32px' }}>
            Deploy JBrowser in minutes. Self-hosted, open source, and built for teams that run browsers at scale.
          </p>
          <div className="landing-cta-group" style={{ justifyContent: 'center' }}>
            <a href="/login" className="landing-btn-primary landing-btn-lg">
              Get started <ArrowRight size={16} />
            </a>
            <a href="https://github.com/jbrowser" className="landing-btn-outline landing-btn-lg" target="_blank" rel="noopener noreferrer">
              View on GitHub
            </a>
          </div>
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="landing-footer">
        <div className="landing-footer-inner">
          <div className="brand-name" style={{ fontSize: 14 }}>
            <span className="brand-bracket">[</span>
            <span className="brand-j">J</span>
            <span className="brand-text">Browser</span>
            <span className="brand-bracket">]</span>
          </div>
          <span style={{ color: 'var(--meta)', fontSize: 12 }}>
            Open source remote browser control platform
          </span>
        </div>
      </footer>
    </div>
  );
}
