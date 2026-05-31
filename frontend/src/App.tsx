import React from 'react';
import { Landing } from './pages/Landing';
import { Login } from './pages/Login';
import { BrowserList } from './pages/BrowserList';
import { BrowserDetail } from './pages/BrowserDetail';
import { Agents } from './pages/Agents';
import { ApiKeys } from './pages/ApiKeys';
import {
  DocsOverview,
  DocsQuickstart,
  DocsDeployAgent,
  DocsCdpConnect,
  DocsAiAgents,
  DocsRestApi,
  DocsCdpApi,
  DocsWebSocket,
  DocsArchitecture,
  DocsSecurity,
} from './pages/docs';
import { useAuthStore } from './stores/auth';

// ── Docs slug → component mapping ───────────────────────────────────────────

const docsPages: Record<string, React.FC<{ tenantId?: string }>> = {
  '': DocsOverview,
  quickstart: DocsQuickstart,
  'guides/deploy-agent': DocsDeployAgent,
  'guides/cdp-connect': DocsCdpConnect,
  'guides/ai-agents': DocsAiAgents,
  'api/rest': DocsRestApi,
  'api/cdp': DocsCdpApi,
  'api/websocket': DocsWebSocket,
  'concepts/architecture': DocsArchitecture,
  'concepts/security': DocsSecurity,
};

function resolveDocsPage(
  slug: string,
  tenantId?: string,
): React.ReactElement | null {
  const Component = docsPages[slug];
  if (!Component) return null;
  return <Component tenantId={tenantId} />;
}

export function App() {
  const token = useAuthStore((state) => state.token);
  const tenantId = useAuthStore((state) => state.tenantId);
  const path = window.location.pathname;

  // Public routes
  if (path === '/login') {
    return <Login />;
  }

  // Docs pages (works both logged in and out)
  // Public: /docs, /docs/quickstart, /docs/guides/cdp-connect, …
  const publicDocsMatch = path.match(/^\/docs(?:\/(.*))?$/);
  if (publicDocsMatch) {
    const slug = publicDocsMatch[1] || '';
    const page = resolveDocsPage(slug);
    return page ?? <DocsOverview />;
  }

  // Landing page for unauthenticated root
  if (!token || !tenantId) {
    if (path === '/' || path === '') {
      return <Landing />;
    }
    return <Login />;
  }

  // Authenticated routes
  const detailMatch = path.match(/^\/tenants\/([^/]+)\/browsers\/([^/]+)$/);
  if (detailMatch) {
    return <BrowserDetail tenantId={detailMatch[1]} browserId={detailMatch[2]} />;
  }

  const agentsMatch = path.match(/^\/tenants\/([^/]+)\/agents$/);
  if (agentsMatch) {
    return <Agents tenantId={agentsMatch[1]} />;
  }

  const apiKeysMatch = path.match(/^\/tenants\/([^/]+)\/settings\/api-keys$/);
  if (apiKeysMatch) {
    return <ApiKeys tenantId={apiKeysMatch[1]} />;
  }

  // Authenticated docs: /tenants/:tid/docs, /tenants/:tid/docs/quickstart, …
  const docsMatch = path.match(/^\/tenants\/([^/]+)\/docs(?:\/(.*))?$/);
  if (docsMatch) {
    const tid = docsMatch[1];
    const slug = docsMatch[2] || '';
    const page = resolveDocsPage(slug, tid);
    return page ?? <DocsOverview tenantId={tid} />;
  }

  return <BrowserList tenantId={tenantId} />;
}

