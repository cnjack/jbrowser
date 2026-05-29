import { Login } from './pages/Login';
import { BrowserList } from './pages/BrowserList';
import { BrowserDetail } from './pages/BrowserDetail';
import { useAuthStore } from './stores/auth';

export function App() {
  const token = useAuthStore((state) => state.token);
  const tenantId = useAuthStore((state) => state.tenantId);
  const path = window.location.pathname;

  if (!token || !tenantId || path === '/login') {
    return <Login />;
  }

  const detailMatch = path.match(/^\/tenants\/([^/]+)\/browsers\/([^/]+)$/);
  if (detailMatch) {
    return <BrowserDetail tenantId={detailMatch[1]} browserId={detailMatch[2]} />;
  }

  return <BrowserList tenantId={tenantId} />;
}

