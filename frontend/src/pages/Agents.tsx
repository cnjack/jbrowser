import { useQuery } from '@tanstack/react-query';
import { listAgents } from '../api/browser';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId: string;
}

function timeAgo(dateStr: string | null): string {
  if (!dateStr) return '—';
  const diff = Date.now() - new Date(dateStr).getTime();
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

export function Agents({ tenantId }: Props) {
  const { data, isLoading, error } = useQuery({
    queryKey: ['agents', tenantId],
    queryFn: () => listAgents(tenantId),
    refetchInterval: 5_000,
  });

  const onlineCount = data?.filter((a) => a.status === 'online').length ?? 0;

  return (
    <div className="app-layout">
      <Sidebar activePage="agents" tenantId={tenantId} />
      <main className="main-area">
        <div className="page-shell">
          <div className="page-header">
            <div>
              <p className="eyebrow">Infrastructure</p>
              <h1 className="page-title">Agents</h1>
              <p className="page-subtitle">Monitor agent processes running your browser instances.</p>
            </div>
          </div>

          <div className="agents-summary">
            <div className="summary-stat">
              <span className="summary-value">{data?.length ?? 0}</span>
              <span className="summary-label">Total</span>
            </div>
            <div className="summary-stat">
              <span className="summary-value summary-online">{onlineCount}</span>
              <span className="summary-label">Online</span>
            </div>
            <div className="summary-stat">
              <span className="summary-value summary-offline">{(data?.length ?? 0) - onlineCount}</span>
              <span className="summary-label">Offline</span>
            </div>
          </div>

          {isLoading ? (
            <div className="loading-state">Loading agents...</div>
          ) : error ? (
            <div className="error-msg">
              {error instanceof Error ? error.message : 'Failed to load agents'}
            </div>
          ) : data && data.length > 0 ? (
            <div className="table-wrap">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Status</th>
                    <th>Browser</th>
                    <th>Browser Instance</th>
                    <th>Last Heartbeat</th>
                    <th>Connected</th>
                  </tr>
                </thead>
                <tbody>
                  {data.map((agent) => (
                    <tr key={agent.id}>
                      <td className="cell-name">
                        <span className="agent-name">{agent.name}</span>
                        <span className="agent-id">{agent.id.slice(0, 8)}</span>
                      </td>
                      <td>
                        <span className={`status-badge status-${agent.status}`}>
                          {agent.status}
                        </span>
                      </td>
                      <td className="cell-mono">
                        {agent.browser_type} {agent.browser_version}
                      </td>
                      <td>
                        <a
                          href={`/tenants/${tenantId}/browsers/${agent.browser_instance_id}`}
                          className="cell-link"
                        >
                          {agent.browser_instance_id.slice(0, 8)}…
                        </a>
                      </td>
                      <td className="cell-meta">{timeAgo(agent.last_heartbeat_at)}</td>
                      <td className="cell-meta">{timeAgo(agent.connected_at)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <div>
                <p style={{ fontWeight: 600, marginBottom: 8 }}>No agents connected</p>
                <p style={{ fontSize: 13, color: 'var(--meta)' }}>
                  Deploy an agent container to get started.{' '}
                  <a href={`/tenants/${tenantId}/docs`} style={{ color: 'var(--accent)' }}>View deployment guide →</a>
                </p>
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
