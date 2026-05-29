import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { listBrowsers } from '../api/browser';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId: string;
}

export function BrowserList({ tenantId }: Props) {
  const [filter, setFilter] = useState<'all' | 'online' | 'offline'>('all');
  const [search, setSearch] = useState('');

  const { data, isLoading, error } = useQuery({
    queryKey: ['browsers', tenantId],
    queryFn: () => listBrowsers(tenantId),
    refetchInterval: 5_000,
  });

  const filtered = data?.filter((b) => {
    if (filter === 'online' && b.status !== 'online') return false;
    if (filter === 'offline' && b.status !== 'offline') return false;
    if (search && !b.name.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const onlineCount = data?.filter((b) => b.status === 'online').length ?? 0;
  const offlineCount = data?.filter((b) => b.status !== 'online').length ?? 0;

  return (
    <div className="app-layout">
      <Sidebar activePage="browsers" tenantId={tenantId} browserCount={onlineCount} />
      <main className="main-area">
        <div className="page-shell">
          <div className="page-header">
            <div>
              <p className="eyebrow">Control Plane</p>
              <h1 className="page-title">Browser Instances</h1>
              <p className="page-subtitle">Manage and monitor your remote browser fleet.</p>
            </div>
          </div>

          <div className="toolbar">
            <div className="search-wrap">
              <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" fill="none" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="7" /><path d="M21 21l-4.35-4.35" />
              </svg>
              <input
                type="text"
                placeholder="Search browsers..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          </div>

          <div className="filter-tabs">
            <button
              type="button"
              className={`filter-tab${filter === 'all' ? ' active' : ''}`}
              onClick={() => setFilter('all')}
            >
              All <span>{data?.length ?? 0}</span>
            </button>
            <button
              type="button"
              className={`filter-tab${filter === 'online' ? ' active' : ''}`}
              onClick={() => setFilter('online')}
            >
              Online <span>{onlineCount}</span>
            </button>
            <button
              type="button"
              className={`filter-tab${filter === 'offline' ? ' active' : ''}`}
              onClick={() => setFilter('offline')}
            >
              Offline <span>{offlineCount}</span>
            </button>
          </div>

          {isLoading ? (
            <div className="loading-state">Loading browsers...</div>
          ) : error ? (
            <div className="error-msg">
              {error instanceof Error ? error.message : 'Failed to load browsers'}
            </div>
          ) : filtered && filtered.length > 0 ? (
            <div className="cards-grid">
              {filtered.map((browser) => (
                <a className="browser-card" href={`/tenants/${tenantId}/browsers/${browser.id}`} key={browser.id}>
                  <div className="card-top">
                    <div>
                      <h3 className="card-name">{browser.name}</h3>
                      <div className="card-meta">{browser.browser_type} {browser.browser_version}</div>
                    </div>
                    <span className={`status-badge status-${browser.status}`}>
                      {browser.status}
                    </span>
                  </div>
                  <div className="metrics">
                    <div>
                      <span className="metric-label">Agent</span>
                      <span className="metric-value truncate">{browser.agent_name}</span>
                    </div>
                    <div>
                      <span className="metric-label">Viewers</span>
                      <span className="metric-value">{browser.viewer_count}</span>
                    </div>
                    <div>
                      <span className="metric-label">Active Tab</span>
                      <span className="metric-value truncate">
                        {browser.tabs.find((tab) => tab.active)?.title ?? 'none'}
                      </span>
                    </div>
                  </div>
                </a>
              ))}
            </div>
          ) : (
            <div className="empty-state">
              <p>No browser instances found.</p>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

