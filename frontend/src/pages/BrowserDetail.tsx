import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Copy, Monitor, Server, Terminal } from 'lucide-react';
import { getBrowser, resetBrowser } from '../api/browser';
import { InputOverlay } from '../components/InputOverlay';
import { Sidebar } from '../components/Sidebar';
import { TabBar } from '../components/TabBar';
import { useAuthStore } from '../stores/auth';
import { ControlSocket } from '../ws/control';

interface Props {
  tenantId: string;
  browserId: string;
}

export function BrowserDetail({ tenantId, browserId }: Props) {
  const queryClient = useQueryClient();
  const token = useAuthStore((state) => state.token);
  const [addressInput, setAddressInput] = useState('');
  const [previewSegment, setPreviewSegment] = useState<ArrayBuffer | null>(null);
  const [loading, setLoading] = useState(false);
  const [connected, setConnected] = useState(false);
  const [statusUrl, setStatusUrl] = useState('');
  const [copied, setCopied] = useState<string | null>(null);
  const [panelOpen, setPanelOpen] = useState(true);
  const socket = useMemo(() => new ControlSocket(), []);
  const loadingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const browserQuery = useQuery({
    queryKey: ['browser', tenantId, browserId],
    queryFn: () => getBrowser(tenantId, browserId),
    refetchInterval: 5_000,
  });
  const resetMutation = useMutation({
    mutationFn: () => resetBrowser(tenantId, browserId),
    onSuccess: () => {
      // Don't invalidate immediately — wait for WS reset.completed event
    },
    onError: (error: Error) => {
      console.error('Reset failed:', error.message);
    },
  });

  useEffect(() => {
    if (!token) return;
    const { logout } = useAuthStore.getState();
    socket.connect(
      token,
      (event) => {
        if (event.type === 'error') {
          // Auth error — token likely expired/invalid, redirect to login
          logout();
          window.location.href = '/login';
          return;
        }
        if (event.type === 'browser.state' || event.type === 'tab.list') {
          queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] });
        }
        if (event.type === 'reset.completed') {
          queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] });
        }
        if (event.type === 'reset.failed') {
          queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] });
          console.error('Browser reset failed:', (event.payload as { error: string }).error);
        }
        if (event.type === 'preview.segment') {
          setPreviewSegment(event.payload);
          setLoading(false);
          if (loadingTimerRef.current) clearTimeout(loadingTimerRef.current);
        }
      },
      () => setConnected(true),
      () => setConnected(false),
    );
    socket.subscribe(browserId);
    return () => {
      socket.close();
    };
  }, [browserId, queryClient, socket, tenantId, token]);

  const browser = browserQuery.data;

  useEffect(() => {
    const activeTab = browser?.tabs.find((t) => t.active);
    if (activeTab?.url && activeTab.url !== 'about:blank') {
      setAddressInput(activeTab.url);
      setStatusUrl(activeTab.url);
    }
  }, [browser]);

  const startLoading = useCallback(() => {
    setLoading(true);
    if (loadingTimerRef.current) clearTimeout(loadingTimerRef.current);
    loadingTimerRef.current = setTimeout(() => setLoading(false), 10000);
  }, []);

  function handleNavigate(e: React.FormEvent) {
    e.preventDefault();
    let url = addressInput.trim();
    if (!url) return;
    if (!/^https?:\/\//i.test(url)) url = `https://${url}`;
    socket.send({ type: 'navigate.url', payload: { browserInstanceId: browserId, url } });
    setAddressInput(url);
    startLoading();
  }

  function handleBack() {
    socket.send({ type: 'navigate.back', payload: { browserInstanceId: browserId } });
    startLoading();
  }

  function handleForward() {
    socket.send({ type: 'navigate.forward', payload: { browserInstanceId: browserId } });
    startLoading();
  }

  function handleReload() {
    socket.send({ type: 'navigate.reload', payload: { browserInstanceId: browserId } });
    startLoading();
  }

  function copyText(text: string, label: string) {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(label);
      setTimeout(() => setCopied(null), 2000);
    });
  }

  const cdpBase = `${window.location.protocol}//${window.location.host}`;
  const cdpEndpoint = `${cdpBase}/cdp/tenants/${tenantId}/browser-instances/${browserId}`;
  const wsEndpoint = `${cdpBase.replace('http', 'ws')}/cdp/tenants/${tenantId}/browser-instances/${browserId}`;

  if (!browser) {
    return (
      <div className="app-layout">
        <Sidebar activePage="browsers" tenantId={tenantId} />
        <main className="browser-shell"><p style={{ color: 'var(--muted)', padding: '2rem' }}>Loading browser...</p></main>
      </div>
    );
  }

  return (
    <div className="app-layout">
      <Sidebar activePage="browsers" tenantId={tenantId} />
      <main className="browser-shell">
      {/* Tab strip — Chrome-style */}
      <div className="browser-tabs-strip">
        <TabBar
          tabs={browser.tabs}
          onCommand={(payload) => socket.send({ type: 'tab.command', payload: { browserInstanceId: browser.id, ...payload } })}
        />
      </div>

      {/* Toolbar — Chrome-style omnibox */}
      <div className="browser-toolbar">
        <div className="toolbar-nav-group">
          <button className="icon-btn" title="Back" onClick={handleBack}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M19 12H5M12 19l-7-7 7-7" /></svg>
          </button>
          <button className="icon-btn" title="Forward" onClick={handleForward}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12h14M12 5l7 7-7 7" /></svg>
          </button>
          <button className="icon-btn" title="Reload" onClick={handleReload}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M1 4v6h6" /><path d="M23 20v-6h-6" /><path d="M20.49 9A9 9 0 005.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 013.51 15" /></svg>
          </button>
        </div>
        <form className="omnibox" onSubmit={handleNavigate}>
          <div className={`conn-indicator${connected ? ' connected' : ''}`} title={connected ? 'Connected' : 'Disconnected'} />
          <input
            type="text"
            className="omnibox-input"
            value={addressInput}
            onChange={(e) => setAddressInput(e.target.value)}
            placeholder="Search or enter URL"
            aria-label="Address bar"
          />
        </form>
        <div className="toolbar-actions">
          {!panelOpen && (
            <button
              className="icon-btn"
              title="Show info panel"
              onClick={() => setPanelOpen(true)}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" /><line x1="15" y1="3" x2="15" y2="21" /></svg>
            </button>
          )}
          <button
            className="icon-btn"
            title={browser.status === 'restarting' ? 'Resetting...' : 'Reset browser'}
            disabled={browser.status === 'restarting' || resetMutation.isPending}
            onClick={() => resetMutation.mutate()}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10" /><path d="M15 9l-6 6M9 9l6 6" /></svg>
          </button>
        </div>
      </div>

      {/* Progress bar */}
      <div className={`progress-bar${loading ? ' active' : ''}`} />

      {/* Canvas area */}
      <div className="browser-content-split">
        <div className="browser-canvas-area">
          <InputOverlay
            browser={browser}
            browserStatus={browser.status}
            onInput={(payload) => socket.send({ type: 'input.event', payload: { browserInstanceId: browser.id, ...payload } })}
            previewSegment={previewSegment}
          />
          {browser.status === 'restarting' && (
            <div className="reset-overlay">
              <div className="reset-spinner" />
              <span>Resetting…</span>
            </div>
          )}
        </div>

        {/* Right info panel */}
        {panelOpen && (
          <aside className="browser-info-panel">
            <div className="info-panel-header">
              <h3>Browser Info</h3>
              <button className="icon-btn-sm" title="Close panel" onClick={() => setPanelOpen(false)}>×</button>
            </div>

            <div className="info-section">
              <div className="info-label"><Monitor size={13} /> Instance</div>
              <div className="info-row">
                <span className="info-key">ID</span>
                <span className="info-value mono">{browser.id.slice(0, 8)}…</span>
                <button className="info-copy" title="Copy ID" onClick={() => copyText(browser.id, 'id')}>
                  <Copy size={11} />{copied === 'id' ? ' ✓' : ''}
                </button>
              </div>
              <div className="info-row">
                <span className="info-key">Name</span>
                <span className="info-value">{browser.name}</span>
              </div>
              <div className="info-row">
                <span className="info-key">Status</span>
                <span className={`info-badge ${browser.status}`}>{browser.status}</span>
              </div>
              <div className="info-row">
                <span className="info-key">Viewport</span>
                <span className="info-value mono">{browser.viewport_width}×{browser.viewport_height}</span>
              </div>
            </div>

            <div className="info-section">
              <div className="info-label"><Server size={13} /> Agent</div>
              <div className="info-row">
                <span className="info-key">Name</span>
                <span className="info-value">{browser.agent_name || '—'}</span>
              </div>
              <div className="info-row">
                <span className="info-key">Status</span>
                <span className={`info-badge ${browser.agent_status}`}>{browser.agent_status}</span>
              </div>
            </div>

            <div className="info-section">
              <div className="info-label"><Terminal size={13} /> CDP Endpoint</div>
              <div className="info-copyblock">
                <code>{cdpEndpoint}/json/version</code>
                <button className="info-copy" title="Copy" onClick={() => copyText(`${cdpEndpoint}/json/version`, 'cdp-json')}>
                  <Copy size={11} />{copied === 'cdp-json' ? ' ✓' : ''}
                </button>
              </div>
              <div className="info-copyblock">
                <code>{wsEndpoint}/devtools/browser/…</code>
                <button className="info-copy" title="Copy" onClick={() => copyText(`${wsEndpoint}/devtools/browser/`, 'cdp-ws')}>
                  <Copy size={11} />{copied === 'cdp-ws' ? ' ✓' : ''}
                </button>
              </div>
              <p className="info-hint">Use a CDP API key in the <code>Authorization</code> header.</p>
            </div>
          </aside>
        )}
      </div>

      {/* Status bar */}
      <div className="browser-status-bar">
        <span className="status-url" title={statusUrl}>{statusUrl || 'about:blank'}</span>
        <span className="status-right">
          {browser.agent_name} · {browser.viewport_width}×{browser.viewport_height}
        </span>
      </div>
    </main>
    </div>
  );
}

