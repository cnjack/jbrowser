import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Plus, RotateCw, Trash2 } from 'lucide-react';
import { createCdpToken, listCdpTokens, revokeCdpToken, rotateCdpToken } from '../api/browser';
import { Sidebar } from '../components/Sidebar';

interface Props {
  tenantId: string;
}

export function ApiKeys({ tenantId }: Props) {
  const queryClient = useQueryClient();
  const [newToken, setNewToken] = useState<string | null>(null);
  const [tokenName, setTokenName] = useState('');

  const { data, isLoading } = useQuery({
    queryKey: ['cdp-tokens', tenantId],
    queryFn: () => listCdpTokens(tenantId),
  });

  const createMutation = useMutation({
    mutationFn: (name: string) => createCdpToken(tenantId, name || undefined),
    onSuccess: (result) => {
      setNewToken(result.token);
      setTokenName('');
      queryClient.invalidateQueries({ queryKey: ['cdp-tokens', tenantId] });
    },
  });

  const revokeMutation = useMutation({
    mutationFn: (tokenId: string) => revokeCdpToken(tenantId, tokenId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cdp-tokens', tenantId] }),
  });

  const rotateMutation = useMutation({
    mutationFn: (tokenId: string) => rotateCdpToken(tenantId, tokenId),
    onSuccess: (result) => {
      setNewToken(result.token);
      queryClient.invalidateQueries({ queryKey: ['cdp-tokens', tenantId] });
    },
  });

  const activeTokens = data?.filter((t) => !t.revoked_at) ?? [];
  const revokedTokens = data?.filter((t) => t.revoked_at) ?? [];

  return (
    <div className="app-layout">
      <Sidebar activePage="api-keys" tenantId={tenantId} />
      <main className="main-area">
        <div className="page-shell">
          <div className="page-header">
            <div>
              <p className="eyebrow">Settings</p>
              <h1 className="page-title">API Keys</h1>
              <p className="page-subtitle">
                Manage CDP access tokens for connecting external tools (Puppeteer, Playwright, etc.) to your browser instances.
              </p>
            </div>
          </div>

          {/* Create new token */}
          <div className="token-create-card">
            <h3>Create new API key</h3>
            <p className="token-create-desc">
              API keys grant full CDP access to all browser instances in this tenant.
              Treat them as secrets.
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
                placeholder="Token name (optional)"
                value={tokenName}
                onChange={(e) => setTokenName(e.target.value)}
                className="token-name-input"
              />
              <button type="submit" className="btn-primary" disabled={createMutation.isPending}>
                <Plus size={14} /> Create key
              </button>
            </form>
          </div>

          {/* Newly created token banner */}
          {newToken && (
            <div className="token-reveal">
              <div className="token-reveal-header">
                <strong>Your new API key</strong>
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

          {/* CDP URL example */}
          <div className="cdp-url-card">
            <h3>CDP Connection URL</h3>
            <p className="token-create-desc">Use this format to connect CDP clients to a browser instance:</p>
            <div className="cdp-url-example">
              <code>
                wss://{window.location.host}/cdp/tenants/{tenantId}/browser-instances/<span className="code-placeholder">{'<browser_id>'}</span>/devtools/page/<span className="code-placeholder">{'<target_id>'}</span>?token=<span className="code-placeholder">{'<api_key>'}</span>
              </code>
            </div>
          </div>

          {/* Active tokens */}
          {isLoading ? (
            <div className="loading-state">Loading tokens...</div>
          ) : activeTokens.length > 0 ? (
            <div className="table-wrap">
              <h3 className="table-title">Active keys</h3>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Prefix</th>
                    <th>Created</th>
                    <th style={{ width: 100 }}>Actions</th>
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
                            className="icon-btn-sm"
                            title="Rotate"
                            onClick={() => rotateMutation.mutate(token.id)}
                          >
                            <RotateCw size={13} />
                          </button>
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
              <p>No active API keys. Create one above to get started.</p>
            </div>
          )}

          {/* Revoked tokens */}
          {revokedTokens.length > 0 && (
            <div className="table-wrap" style={{ opacity: 0.6 }}>
              <h3 className="table-title">Revoked keys</h3>
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
