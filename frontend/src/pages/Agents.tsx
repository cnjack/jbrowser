import { useState } from 'react';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Copy, X } from 'lucide-react';
import { listAgents, generateAgentToken } from '../api/browser';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId: string;
}

type StatusFilter = 'all' | 'online' | 'offline';

export function Agents({ tenantId }: Props) {
  const [filter, setFilter] = useState<StatusFilter>('all');
  const [showSetup, setShowSetup] = useState(false);
  const [setupToken, setSetupToken] = useState<string | null>(null);

  const { data, isLoading, error } = useQuery({
    queryKey: ['agents', tenantId],
    queryFn: () => listAgents(tenantId),
    refetchInterval: 5_000,
  });

  const tokenMutation = useMutation({
    mutationFn: () => generateAgentToken(tenantId),
    onSuccess: (result) => {
      setSetupToken(result.token);
      setShowSetup(true);
    },
  });

  const allAgents = data ?? [];
  const onlineCount = allAgents.filter((a) => a.status === 'online').length;
  const offlineCount = allAgents.length - onlineCount;

  const filtered = allAgents.filter((a) => {
    if (filter === 'online') return a.status === 'online';
    if (filter === 'offline') return a.status !== 'online';
    return true;
  });

  const host = window.location.host;

  function handleNewAgent() {
    tokenMutation.mutate();
  }

  function handleCloseSetup() {
    setShowSetup(false);
    setSetupToken(null);
  }

  return (
    <div className="app-layout">
      <Sidebar activePage="agents" tenantId={tenantId} />
      <main className="main-area">
        <div className="page-shell">
          {/* Header */}
          <div className="page-header">
            <div>
              <h1 className="page-title">Agents</h1>
              <p className="page-subtitle">
                Includes all self-hosted agents connected to this tenant.
              </p>
            </div>
            <button
              type="button"
              className="btn-primary"
              onClick={handleNewAgent}
              disabled={tokenMutation.isPending}
            >
              {tokenMutation.isPending ? 'Generating…' : 'New agent'}
            </button>
          </div>

          {/* New agent setup panel */}
          {showSetup && setupToken && (
            <div className="agent-setup-panel">
              <div className="agent-setup-header">
                <div>
                  <h3>Agent / Create self-hosted agent</h3>
                  <p>Configure and start a new self-hosted agent to connect to this tenant.</p>
                </div>
                <button type="button" className="icon-btn-sm" onClick={handleCloseSetup} title="Close">
                  <X size={16} />
                </button>
              </div>

              <div className="agent-setup-body">
                <div className="agent-setup-section">
                  <h4>Download &amp; Configure</h4>
                  <div className="agent-code-block">
                    <pre><code>{`# Docker Run
docker run -d \\
  --name jbrowser-agent \\
  --shm-size=1g \\
  -e CONTROL_PLANE_URL=wss://${host} \\
  -e REGISTRATION_TOKEN=${setupToken} \\
  ghcr.io/cnjack/jbrowser-agent-chromium:latest`}</code></pre>
                    <button
                      type="button"
                      className="agent-copy-btn"
                      title="Copy"
                      onClick={() =>
                        navigator.clipboard.writeText(
                          `docker run -d \\\n  --name jbrowser-agent \\\n  --shm-size=1g \\\n  -e CONTROL_PLANE_URL=wss://${host} \\\n  -e REGISTRATION_TOKEN=${setupToken} \\\n  ghcr.io/cnjack/jbrowser-agent-chromium:latest`
                        )
                      }
                    >
                      <Copy size={13} /> Copy
                    </button>
                  </div>
                </div>

                <div className="agent-setup-section">
                  <h4>Docker Compose</h4>
                  <div className="agent-code-block">
                    <pre><code>{`services:
  agent:
    image: ghcr.io/cnjack/jbrowser-agent-chromium:latest
    shm_size: "1gb"
    environment:
      CONTROL_PLANE_URL: wss://${host}
      REGISTRATION_TOKEN: ${setupToken}
    restart: unless-stopped`}</code></pre>
                    <button
                      type="button"
                      className="agent-copy-btn"
                      title="Copy"
                      onClick={() =>
                        navigator.clipboard.writeText(
                          `services:\n  agent:\n    image: ghcr.io/cnjack/jbrowser-agent-chromium:latest\n    shm_size: "1gb"\n    environment:\n      CONTROL_PLANE_URL: wss://${host}\n      REGISTRATION_TOKEN: ${setupToken}\n    restart: unless-stopped`
                        )
                      }
                    >
                      <Copy size={13} /> Copy
                    </button>
                  </div>
                </div>

                <div className="agent-setup-note">
                  <strong>Token:</strong>{' '}
                  <code className="agent-token-inline">{setupToken}</code>
                  <button
                    type="button"
                    className="icon-btn-sm"
                    title="Copy token"
                    onClick={() => navigator.clipboard.writeText(setupToken)}
                  >
                    <Copy size={12} />
                  </button>
                  <span className="agent-token-warn">This token is bound to your tenant. Keep it secret.</span>
                </div>
              </div>
            </div>
          )}

          {/* Filter tabs */}
          <div className="agent-tabs">
            {(['all', 'online', 'offline'] as StatusFilter[]).map((tab) => {
              const count = tab === 'all' ? allAgents.length : tab === 'online' ? onlineCount : offlineCount;
              return (
                <button
                  key={tab}
                  type="button"
                  className={`agent-tab${filter === tab ? ' active' : ''}`}
                  onClick={() => setFilter(tab)}
                >
                  {tab.charAt(0).toUpperCase() + tab.slice(1)}
                  <span className="agent-tab-count">{count}</span>
                </button>
              );
            })}
          </div>

          {/* Agents table */}
          {isLoading ? (
            <div className="loading-state">Loading agents…</div>
          ) : error ? (
            <div className="error-msg">
              {error instanceof Error ? error.message : 'Failed to load agents'}
            </div>
          ) : filtered.length > 0 ? (
            <div className="table-wrap agents-table-wrap">
              <table className="data-table agents-table">
                <thead>
                  <tr>
                    <th>Agents</th>
                    <th style={{ width: 120, textAlign: 'right' }}>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.map((agent) => (
                    <tr key={agent.id}>
                      <td>
                        <div className="agent-row-name">
                          <svg className="agent-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                            <rect x="2" y="6" width="20" height="12" rx="2" />
                            <path d="M6 10h4M6 14h3" />
                            <circle cx="17" cy="12" r="2" />
                          </svg>
                          <span className="agent-name">{agent.name}</span>
                        </div>
                      </td>
                      <td style={{ textAlign: 'right', verticalAlign: 'middle' }}>
                        <span className={`agent-status-dot agent-status-${agent.status}`} />
                        <span className="agent-status-text">
                          {agent.status === 'online' ? 'Idle' : 'Offline'}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="agents-empty">
              <p>No agents found.</p>
              <p>
                <button type="button" className="link-btn" onClick={handleNewAgent}>
                  Add a new agent
                </button>{' '}
                to get started.
              </p>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
