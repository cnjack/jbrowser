import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createCdpToken, getBrowser, listCdpTokens, resetBrowser } from '../api/browser';
import { InputOverlay } from '../components/InputOverlay';
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
  const [cdpToken, setCdpToken] = useState<string | null>(null);
  const [addressInput, setAddressInput] = useState('');
  const [previewSegment, setPreviewSegment] = useState<ArrayBuffer | null>(null);
  const [loading, setLoading] = useState(false);
  const [connected, setConnected] = useState(false);
  const [statusUrl, setStatusUrl] = useState('');
  const socket = useMemo(() => new ControlSocket(), []);
  const loadingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const browserQuery = useQuery({
    queryKey: ['browser', tenantId, browserId],
    queryFn: () => getBrowser(tenantId, browserId),
    refetchInterval: 5_000,
  });
  const tokenQuery = useQuery({
    queryKey: ['cdp-tokens', tenantId],
    queryFn: () => listCdpTokens(tenantId),
  });
  const resetMutation = useMutation({
    mutationFn: () => resetBrowser(tenantId, browserId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] }),
  });
  const cdpMutation = useMutation({
    mutationFn: () => createCdpToken(tenantId),
    onSuccess: (result) => {
      setCdpToken(result.token);
      queryClient.invalidateQueries({ queryKey: ['cdp-tokens', tenantId] });
    },
  });

  useEffect(() => {
    if (!token) return;
    socket.connect(
      token,
      (event) => {
        if (event.type === 'browser.state' || event.type === 'tab.list') {
          queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] });
        }
        if (event.type === 'preview.segment') {
          setPreviewSegment(event.payload);
          // Clear loading when first frame arrives
          setLoading(false);
          if (loadingTimerRef.current) clearTimeout(loadingTimerRef.current);
        }
      },
      () => setConnected(true),
      () => setConnected(false),
    );
    const timer = window.setTimeout(() => socket.subscribe(browserId), 250);
    return () => {
      window.clearTimeout(timer);
      socket.close();
    };
  }, [browserId, queryClient, socket, tenantId, token]);

  const browser = browserQuery.data;

  // Sync address bar + status bar with active tab URL
  useEffect(() => {
    const activeTab = browser?.tabs.find((t) => t.active);
    if (activeTab?.url && activeTab.url !== 'about:blank') {
      setAddressInput(activeTab.url);
      setStatusUrl(activeTab.url);
    }
  }, [browser]);

  const cdpUrl = cdpToken
    ? `${window.location.origin.replace(/^http/, 'ws')}/cdp/tenants/${tenantId}/browser-instances/${browserId}/devtools/page/${browser?.active_tab_id ?? 'target'}?token=${cdpToken}`
    : null;

  const startLoading = useCallback(() => {
    setLoading(true);
    if (loadingTimerRef.current) clearTimeout(loadingTimerRef.current);
    // Auto-clear loading after 10s as fallback
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

  if (!browser) {
    return <main className="browser-shell"><p style={{ color: '#888', padding: '2rem' }}>Loading browser...</p></main>;
  }

  return (
    <main className="browser-shell">
      {/* Tab strip */}
      <div className="browser-tabs-strip">
        <a href={`/tenants/${tenantId}/browsers`} className="back-link" title="Back to browsers">←</a>
        <TabBar
          tabs={browser.tabs}
          onCommand={(payload) => socket.send({ type: 'tab.command', payload: { browserInstanceId: browser.id, ...payload } })}
        />
      </div>

      {/* Toolbar */}
      <div className="browser-toolbar">
        <button className="icon-btn" title="Back" onClick={handleBack}>&#8592;</button>
        <button className="icon-btn" title="Forward" onClick={handleForward}>&#8594;</button>
        <button className="icon-btn" title="Reload" onClick={handleReload}>&#8635;</button>
        <form className="address-bar" onSubmit={handleNavigate}>
          <input
            type="text"
            className="address-input"
            value={addressInput}
            onChange={(e) => setAddressInput(e.target.value)}
            placeholder="Enter URL and press Enter…"
            aria-label="Address bar"
          />
          <button type="submit" className="address-go">Go</button>
        </form>
        <div className="divider" />
        <button
          className="icon-btn danger-btn"
          title="Reset browser"
          onClick={() => resetMutation.mutate()}
        >⟳</button>
        <div className={`conn-dot${connected ? ' connected' : ''}`} title={connected ? 'Connected' : 'Disconnected'} />
      </div>

      {/* Progress bar */}
      <div className={`progress-bar${loading ? ' active' : ''}`} />

      {/* Canvas area */}
      <div className="browser-canvas-area">
        <InputOverlay
          browser={browser}
          onInput={(payload) => socket.send({ type: 'input.event', payload: { browserInstanceId: browser.id, ...payload } })}
          previewSegment={previewSegment}
        />
      </div>

      {/* Status bar */}
      <div className="browser-status-bar">
        <span className="status-url" title={statusUrl}>{statusUrl || 'about:blank'}</span>
        <span className="status-right">
          {browser.agent_name} · {browser.viewport_width}×{browser.viewport_height}
          {cdpToken ? (
            <> · <a href="#" onClick={(e) => { e.preventDefault(); navigator.clipboard.writeText(cdpUrl ?? ''); }}>Copy CDP URL</a></>
          ) : (
            <> · <a href="#" onClick={(e) => { e.preventDefault(); cdpMutation.mutate(); }}>Get CDP</a></>
          )}
          · {tokenQuery.data?.length ?? 0} tokens
        </span>
      </div>
    </main>
  );
}

