import { Globe, LogOut } from 'lucide-react';
import { useAuthStore } from '../../stores/auth';
import { Sidebar } from '../../components/Sidebar';

// ── Docs navigation structure ────────────────────────────────────────────────

export interface DocNav {
  label: string;
  items: { slug: string; title: string }[];
}

export const docsNav: DocNav[] = [
  {
    label: 'Getting Started',
    items: [
      { slug: '', title: 'Overview' },
      { slug: 'quickstart', title: 'Quickstart' },
    ],
  },
  {
    label: 'Guides',
    items: [
      { slug: 'guides/deploy-agent', title: 'Deploy an Agent' },
      { slug: 'guides/cdp-connect', title: 'Connect with CDP' },
      { slug: 'guides/ai-agents', title: 'AI Agent Integration' },
    ],
  },
  {
    label: 'API Reference',
    items: [
      { slug: 'api/rest', title: 'REST API' },
      { slug: 'api/cdp', title: 'CDP Endpoints' },
      { slug: 'api/websocket', title: 'WebSocket Protocol' },
    ],
  },
  {
    label: 'Concepts',
    items: [
      { slug: 'concepts/architecture', title: 'Architecture' },
      { slug: 'concepts/security', title: 'Security & Tokens' },
    ],
  },
];

// ── Layout component ─────────────────────────────────────────────────────────

interface Props {
  tenantId?: string;
  slug: string;
  children: React.ReactNode;
}

function docsBasePath(tenantId?: string) {
  return tenantId ? `/tenants/${tenantId}/docs` : '/docs';
}

function DocsNav({ slug, tenantId }: { slug: string; tenantId?: string }) {
  const base = docsBasePath(tenantId);
  return (
    <nav className="docs-nav">
      {docsNav.map((group) => (
        <div key={group.label} className="docs-nav-group">
          <div className="docs-nav-label">{group.label}</div>
          {group.items.map((item) => (
            <a
              key={item.slug}
              href={item.slug ? `${base}/${item.slug}` : base}
              className={`docs-nav-item${slug === item.slug ? ' active' : ''}`}
            >
              {item.title}
            </a>
          ))}
        </div>
      ))}
    </nav>
  );
}

export function DocsLayout({ tenantId, slug, children }: Props) {
  const token = useAuthStore((s) => s.token);
  const tid = tenantId ?? useAuthStore((s) => s.tenantId) ?? '';

  const inner = (
    <div className="docs-layout">
      <DocsNav slug={slug} tenantId={token && tid ? tid : undefined} />
      <div className="docs-content">
        {children}
      </div>
    </div>
  );

  if (token && tid) {
    return (
      <div className="app-layout">
        <Sidebar activePage="docs" tenantId={tid} />
        <main className="main-area">{inner}</main>
      </div>
    );
  }

  // Standalone (not logged in)
  return (
    <div className="docs-standalone">
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
          <div className="landing-nav-actions">
            <a href="/login" className="landing-btn-ghost">Log In</a>
          </div>
        </div>
      </nav>
      <main className="docs-standalone-main">{inner}</main>
    </div>
  );
}
