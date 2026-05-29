import { Landing } from './pages/Landing';
import { Login } from './pages/Login';
import { BrowserList } from './pages/BrowserList';
import { BrowserDetail } from './pages/BrowserDetail';
import { Agents } from './pages/Agents';
import { ApiKeys } from './pages/ApiKeys';
import { AgentTokens } from './pages/AgentTokens';
import { Docs } from './pages/Docs';
import { useAuthStore } from './stores/auth';

export function App() {
  const token = useAuthStore((state) => state.token);
  const tenantId = useAuthStore((state) => state.tenantId);
  const path = window.location.pathname;

  // Public routes
  if (path === '/login') {
    return <Login />;
  }

  // Docs page (works both logged in and out)
  if (path === '/docs') {
    return <Docs />;
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

  const agentTokensMatch = path.match(/^\/tenants\/([^/]+)\/settings\/agent-tokens$/);
  if (agentTokensMatch) {
    return <AgentTokens tenantId={agentTokensMatch[1]} />;
  }

  const docsMatch = path.match(/^\/tenants\/([^/]+)\/docs$/);
  if (docsMatch) {
    return <Docs tenantId={docsMatch[1]} />;
  }

  return <BrowserList tenantId={tenantId} />;
}

