import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Plus, Trash2 } from 'lucide-react';
import { createAgentToken, listAgentTokens, revokeAgentToken } from '../api/browser';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId: string;
}

export function AgentTokens({ tenantId }: Props) {
  const queryClient = useQueryClient();
  const [newToken, setNewToken] = useState<string | null>(null);
  const [tokenName, setTokenName] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['agent-tokens', tenantId],
    queryFn: () => listAgentTokens(tenantId),
  });

  const createMutation = useMutation({
    mutationFn: (name: string) => createAgentToken(tenantId, name || undefined),
    onSuccess: (result) => {
      setNewToken(result.token);
      setTokenName('');
      queryClient.invalidateQueries({ queryKey: ['agent-tokens', tenantId] });
    },
  });

  const revokeMutation = useMutation({
    mutationFn: (tokenId: string) => revokeAgentToken(tenantId, tokenId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['agent-tokens', tenantId] }),
  });

  const activeTokens = data?.filter((t) => !t.revoked_at) ?? [];
  const revokedTokens = data?.filter((t) => t.revoked_at) ?? [];

  return (
    <div className="app-layout">
      <Sidebar activePage="agent-tokens" tenantId={tenantId} />
      <main className="main-area">
        <div className="page-shell">
          <div className="page-header">
            <div>
              <p className="eyebrow">Settings</p>
              <h1 className="page-title">Agent Tokens</h1>
              <p className="page-subtitle">
                Registration tokens allow agents to connect to the control plane. One token can register multiple agents.
              </p>
            </div>
          </div>

          {/* Create new token */}
          <div className="token-create-card">
            <h3>Create registration token</h3>
            <p className="token-create-desc">
              Pass this token to your agent container via the <code>REGISTRATION_TOKEN</code> environment variable.
            </p>
            <form
              className="token-create-form"
              onSubmit={(e) => {
                e.preventDefault();
                createMutation.mutate(tokenName);
              }}
            >
              <input
                type="text"
                placeholder="Token name (e.g. production-fleet)"
                value={tokenName}
                onChange={(e) => setTokenName(e.target.value)}
                className="token-name-input"
              />
              <button type="submit" className="btn-primary" disabled={createMutation.isPending}>
                <Plus size={14} /> Create token
              </button>
            </form>
          </div>

          {/* Newly created token banner */}
          {newToken && (
            <div className="token-reveal">
              <div className="token-reveal-header">
                <strong>Your new registration token</strong>
                <span className="token-reveal-warn">Copy it now — you won't see it again.</span>
              </div>
              <div className="token-reveal-value">
                <code>{newToken}</code>
                <button
                  type="button"
                  className="icon-btn-sm"
                  title="Copy"
                  onClick={() => navigator.clipboard.writeText(newToken)}
                >
                  <Copy size={14} />
                </button>
              </div>
              <button
                type="button"
                className="token-reveal-dismiss"
                onClick={() => setNewToken(null)}
              >
                I've copied it
              </button>
            </div>
          )}

          {/* Usage example */}
          <div className="cdp-url-card">
            <h3>Usage</h3>
            <p className="token-create-desc">Pass the token when starting your agent:</p>
            <div className="cdp-url-example">
              <code>
                docker run -e CONTROL_PLANE_URL=wss://{window.location.host} \{'\n'}
                {'  '}-e REGISTRATION_TOKEN=<span className="code-placeholder">{'<token>'}</span> \{'\n'}
                {'  '}jbrowser/agent-chromium:latest
              </code>
            </div>
          </div>

          {/* Active tokens */}
          {isLoading ? (
            <div className="loading-state">Loading tokens...</div>
          ) : activeTokens.length > 0 ? (
            <div className="table-wrap">
              <h3 className="table-title">Active tokens</h3>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Prefix</th>
                    <th>Created</th>
                    <th style={{ width: 80 }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {activeTokens.map((token) => (
                    <tr key={token.id}>
                      <td className="cell-name">{token.name || '—'}</td>
                      <td className="cell-mono">{token.token_prefix}…</td>
                      <td className="cell-meta">{new Date(token.created_at).toLocaleDateString()}</td>
                      <td>
                        <div className="cell-actions">
                          <button
                            type="button"
                            className="icon-btn-sm danger"
                            title="Revoke"
                            onClick={() => revokeMutation.mutate(token.id)}
                          >
                            <Trash2 size={13} />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">
              <p>No active registration tokens. Create one above to register agents.</p>
            </div>
          )}

          {/* Revoked tokens */}
          {revokedTokens.length > 0 && (
            <div className="table-wrap" style={{ opacity: 0.6 }}>
              <h3 className="table-title">Revoked tokens</h3>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Prefix</th>
                    <th>Created</th>
                    <th>Revoked</th>
                  </tr>
                </thead>
                <tbody>
                  {revokedTokens.map((token) => (
                    <tr key={token.id}>
                      <td className="cell-name">{token.name || '—'}</td>
                      <td className="cell-mono">{token.token_prefix}…</td>
                      <td className="cell-meta">{new Date(token.created_at).toLocaleDateString()}</td>
                      <td className="cell-meta">{token.revoked_at ? new Date(token.revoked_at).toLocaleDateString() : '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}
