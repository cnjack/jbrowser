import { useQuery } from '@tanstack/react-query';
import { listBrowsers } from '../api/browser';
import { useAuthStore } from '../stores/auth';

interface Props {
  tenantId: string;
}

export function BrowserList({ tenantId }: Props) {
  const logout = useAuthStore((state) => state.logout);
  const { data, isLoading, error } = useQuery({
    queryKey: ['browsers', tenantId],
    queryFn: () => listBrowsers(tenantId),
    refetchInterval: 5_000,
  });

  return (
    <main className="page">
      <header className="topbar">
        <div>
          <p className="eyebrow">Tenant {tenantId}</p>
          <h1>Browser instances</h1>
        </div>
        <button type="button" onClick={() => { logout(); window.location.href = '/login'; }}>
          Logout
        </button>
      </header>

      {isLoading ? <p>Loading browsers...</p> : null}
      {error ? <p className="error">{error instanceof Error ? error.message : 'Failed to load browsers'}</p> : null}

      <section className="grid">
        {data?.map((browser) => (
          <a className="card browser-card" href={`/tenants/${tenantId}/browsers/${browser.id}`} key={browser.id}>
            <div className={`status ${browser.status}`}>{browser.status}</div>
            <h2>{browser.name}</h2>
            <p>{browser.browser_type} {browser.browser_version}</p>
            <dl>
              <dt>Agent</dt>
              <dd>{browser.agent_name} / {browser.agent_status}</dd>
              <dt>Active tab</dt>
              <dd>{browser.tabs.find((tab) => tab.active)?.title ?? 'none'}</dd>
              <dt>Viewers</dt>
              <dd>{browser.viewer_count}</dd>
              <dt>Heartbeat</dt>
              <dd>{browser.last_heartbeat_at ?? 'never'}</dd>
            </dl>
          </a>
        ))}
      </section>
    </main>
  );
}

