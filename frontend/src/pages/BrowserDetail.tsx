import { useEffect, useMemo, useState } from 'react';
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
  const [events, setEvents] = useState<string[]>([]);
  const [cdpToken, setCdpToken] = useState<string | null>(null);
  const [addressInput, setAddressInput] = useState('');
  const [previewSegment, setPreviewSegment] = useState<ArrayBuffer | null>(null);
  const socket = useMemo(() => new ControlSocket(), []);

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
    socket.connect(token, (event) => {
      setEvents((current) => [`${new Date().toLocaleTimeString()} ${event.type}`, ...current].slice(0, 20));
      if (event.type === 'browser.state' || event.type === 'tab.list') {
        queryClient.invalidateQueries({ queryKey: ['browser', tenantId, browserId] });
      }
      if (event.type === 'preview.segment') {
        setPreviewSegment(event.payload);
      }
    });
    const timer = window.setTimeout(() => socket.subscribe(browserId), 250);
    return () => {
      window.clearTimeout(timer);
      socket.close();
    };
  }, [browserId, queryClient, socket, tenantId, token]);

  const browser = browserQuery.data;

  // Sync address bar with active tab URL
  useEffect(() => {
    const activeTab = browser?.tabs.find((t) => t.active);
    if (activeTab?.url && activeTab.url !== 'about:blank') {
      setAddressInput(activeTab.url);
    }
  }, [browser]);

  const cdpUrl = cdpToken
    ? `${window.location.origin.replace(/^http/, 'ws')}/cdp/tenants/${tenantId}/browser-instances/${browserId}/devtools/page/${browser?.active_tab_id ?? 'target'}?token=${cdpToken}`
    : null;

  function handleNavigate(e: React.FormEvent) {
    e.preventDefault();
    let url = addressInput.trim();
    if (!url) return;
    // Auto-prepend https:// if no protocol
    if (!/^https?:\/\//i.test(url)) url = `https://${url}`;
    socket.send({
      type: 'navigate.url',
      payload: { browserInstanceId: browserId, url },
    });
    setAddressInput(url);
  }

  if (!browser) {
    return <main className="page"><p>Loading browser...</p></main>;
  }

  return (
    <main className="page detail">
      <header className="topbar">
        <div>
          <a href={`/tenants/${tenantId}/browsers`}>← Browser list</a>
          <h1>{browser.name}</h1>
          <p>{browser.status} · {browser.browser_type} {browser.browser_version}</p>
        </div>
        <button type="button" onClick={() => resetMutation.mutate()}>
          Global reset
        </button>
      </header>

      <section className="detail-grid">
        <div className="card">
          <TabBar
            tabs={browser.tabs}
            onCommand={(payload) => socket.send({ type: 'tab.command', payload: { browserInstanceId: browser.id, ...payload } })}
          />
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
          <InputOverlay
            browser={browser}
            onInput={(payload) => socket.send({ type: 'input.event', payload: { browserInstanceId: browser.id, ...payload } })}
            previewSegment={previewSegment}
          />
        </div>

        <aside className="card side-panel">
          <h2>Diagnostics</h2>
          <dl>
            <dt>Browser ID</dt>
            <dd>{browser.id}</dd>
            <dt>Agent</dt>
            <dd>{browser.agent_name} / {browser.agent_status}</dd>
            <dt>Viewport</dt>
            <dd>{browser.viewport_width}×{browser.viewport_height}</dd>
            <dt>CDP tokens</dt>
            <dd>{tokenQuery.data?.length ?? 0}</dd>
          </dl>
          <button type="button" onClick={() => cdpMutation.mutate()}>
            Create CDP token
          </button>
          {cdpUrl ? (
            <textarea readOnly value={cdpUrl} aria-label="CDP URL" />
          ) : null}
          <h2>Event log</h2>
          <ul className="event-log">
            {events.map((event, index) => <li key={`${event}-${index}`}>{event}</li>)}
          </ul>
        </aside>
      </section>
    </main>
  );
}

