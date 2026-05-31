import { lazy, Suspense } from 'react';
import { DocsLayout } from './DocsLayout';

const Mermaid = lazy(() =>
  import('../../components/Mermaid').then((m) => ({ default: m.Mermaid })),
);

function MermaidDiagram({ chart }: { chart: string }) {
  return (
    <Suspense fallback={<div className="mermaid-wrap mermaid-loading">Loading diagram…</div>}>
      <Mermaid chart={chart} />
    </Suspense>
  );
}

export function DocsArchitecture({ tenantId }: { tenantId?: string }) {
  return (
    <DocsLayout slug="concepts/architecture" tenantId={tenantId}>
      <div className="page-header">
        <div>
          <p className="eyebrow">Concepts</p>
          <h1 className="page-title">Architecture</h1>
          <p className="page-subtitle">
            How JBrowser connects your tools to cloud browsers.
          </p>
        </div>
      </div>

      <section className="docs-section">
        <h2 className="docs-h2">System Overview</h2>
        <MermaidDiagram chart={`
flowchart LR
  subgraph Tools["Your Tools"]
    PP["Puppeteer / Playwright"]
    AI["AI Agents / Raw CDP"]
    UI["Web UI"]
  end
  subgraph CP["Control Plane"]
    REST["REST API"]
    CDPT["CDP Tunnel"]
    VR["Video Relay"]
    AUTH["Auth (JWT + Tokens)"]
    SPA["Frontend SPA"]
  end
  subgraph AG["Agent Container"]
    Chrome["Chrome (headless)"]
    CDP9222["CDP :9222"]
    SC["Screencast"]
  end
  PP -- "WebSocket (CDP)" --> CDPT
  AI -- "WebSocket (CDP)" --> CDPT
  UI -- "WebSocket + HTTP" --> SPA
  CDPT -- "WebSocket tunnel" --> Chrome
  VR -- "WebSocket" --> UI
  SC -- "JPEG frames" --> VR
  Chrome --- CDP9222
  Chrome --- SC
`} />
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Components</h2>
        <dl>
          <dt>Control Plane</dt>
          <dd>
            Rust/Axum HTTP+WebSocket server. Handles authentication, tenant management,
            agent registration, CDP tunneling, and serves the web UI.
            All state is in-memory (MVP — database-backed in production).
          </dd>

          <dt>Agent</dt>
          <dd>
            Rust binary running inside a container alongside headless Chrome.
            Connects <em>outbound</em> to the control plane via WebSocket.
            Manages Chrome lifecycle, screencast, input forwarding, and CDP tunneling.
          </dd>

          <dt>Frontend</dt>
          <dd>
            React SPA served by the control plane. Provides browser management UI,
            real-time preview (JPEG screencast via WebSocket), mouse/keyboard input
            overlay, tab management, and this documentation.
          </dd>
        </dl>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Data Flow</h2>

        <h3 className="docs-h3">CDP Tunnel</h3>
        <MermaidDiagram chart={`
sequenceDiagram
  participant C as CDP Client
  participant CP as Control Plane
  participant AG as Agent
  participant CH as Chrome :9222

  C->>CP: WS connect /cdp/.../devtools/page/:target?token=...
  CP->>CP: Validate CDP token
  CP->>AG: cdp.tunnel.open {target_id}
  AG->>CH: WS connect localhost:9222/devtools/page/:target
  loop Bidirectional forwarding
    C->>CP: CDP command (JSON)
    CP->>AG: Forward frame
    AG->>CH: Forward to Chrome
    CH-->>AG: CDP response / event
    AG-->>CP: Forward frame
    CP-->>C: Forward to client
  end
  C->>CP: WS disconnect
  CP->>AG: cdp.tunnel.close
  AG->>CH: Close local WS
`} />

        <h3 className="docs-h3">Video Preview</h3>
        <MermaidDiagram chart={`
sequenceDiagram
  participant AG as Agent
  participant CH as Chrome
  participant CP as Control Plane
  participant WU as Web UI

  AG->>CH: Page.startScreencast
  loop Every frame
    CH-->>AG: Page.screencastFrame (JPEG)
    AG->>AG: Encode binary wire frame
    AG->>CP: Binary WS frame
    CP->>CP: Broadcast to subscribers
    CP-->>WU: Binary WS frame
    WU->>WU: Decode & render <img>
  end
`} />

        <h3 className="docs-h3">Input Events</h3>
        <MermaidDiagram chart={`
sequenceDiagram
  participant U as User
  participant WU as Web UI
  participant CP as Control Plane
  participant AG as Agent
  participant CH as Chrome

  U->>WU: Mouse / keyboard event on overlay
  WU->>WU: mapCoordinates() → viewport space
  WU->>CP: input.event via /ws/control
  CP->>AG: Forward input.event
  AG->>CH: Input.dispatchMouseEvent / Input.dispatchKeyEvent (CDP)
`} />
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Agent Connection</h2>
        <p>
          Agents connect <strong>outbound</strong> to the control plane — never the other way.
          This design means:
        </p>
        <ul>
          <li>No inbound ports required on agent containers</li>
          <li>Works behind NAT, firewalls, and in Kubernetes pods</li>
          <li>Chrome's CDP port is only accessible on <code>127.0.0.1</code> inside the container</li>
          <li>If the agent disconnects, it automatically reconnects and re-registers</li>
        </ul>
      </section>

      <section className="docs-section">
        <h2 className="docs-h2">Multi-Tenancy</h2>
        <p>
          Every resource (browser, agent, token, audit log) is scoped to a tenant.
          JWT tokens carry tenant claims, and every API query filters by <code>tenant_id</code>.
          CDP tokens are also tenant-scoped — a token for Tenant A cannot access Tenant B's browsers.
        </p>
      </section>
    </DocsLayout>
  );
}
