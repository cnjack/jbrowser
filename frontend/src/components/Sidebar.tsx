import { Globe, LogOut } from 'lucide-react';
import { useAuthStore } from '../stores/auth';

interface Props {
  activePage: 'browsers' | 'agents' | 'api-keys' | 'agent-tokens' | 'docs';
  tenantId: string;
  browserCount?: number;
}

export function Sidebar({ activePage, tenantId, browserCount }: Props) {
  const userEmail = useAuthStore((state) => state.userEmail);
  const logout = useAuthStore((state) => state.logout);

  function handleLogout() {
    logout();
    window.location.href = '/login';
  }

  const initial = userEmail ? userEmail[0].toUpperCase() : 'U';

  return (
    <aside className="sidebar">
      <div className="brand">
        <a href={`/tenants/${tenantId}/browsers`} className="brand-link">
          <span className="brand-icon"><Globe size={18} strokeWidth={2.2} /></span>
          <span className="brand-name">
            <span className="brand-bracket">[</span>
            <span className="brand-j">J</span>
            <span className="brand-text">Browser</span>
            <span className="brand-bracket">]</span>
          </span>
        </a>
      </div>

      <nav className="nav-group">
        <div className="nav-label">Platform</div>
        <a
          href={`/tenants/${tenantId}/browsers`}
          className={`nav-item${activePage === 'browsers' ? ' active' : ''}`}
        >
          <svg viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="3" /><path d="M3 9h18" /><circle cx="7" cy="6" r="1" fill="currentColor" stroke="none" /><circle cx="10" cy="6" r="1" fill="currentColor" stroke="none" /></svg>
          Browsers
          {browserCount !== undefined && (
            <span className="count-badge">{browserCount}</span>
          )}
        </a>
        <a
          href={`/tenants/${tenantId}/agents`}
          className={`nav-item${activePage === 'agents' ? ' active' : ''}`}
        >
          <svg viewBox="0 0 24 24"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h4M6 14h3" /><circle cx="17" cy="12" r="2" /></svg>
          Agents
        </a>
      </nav>

      <nav className="nav-group">
        <div className="nav-label">Settings</div>
        <a
          href={`/tenants/${tenantId}/settings/api-keys`}
          className={`nav-item${activePage === 'api-keys' ? ' active' : ''}`}
        >
          <svg viewBox="0 0 24 24"><path d="M21 2l-2 2m-7.61 7.61a5.5 5.5 0 11-7.778 7.778 5.5 5.5 0 017.777-7.777zm0 0L15.5 7.5m0 0l3 3L22 7l-3-3m-3.5 3.5L19 4" /></svg>
          API Keys
        </a>
        <a
          href={`/tenants/${tenantId}/settings/agent-tokens`}
          className={`nav-item${activePage === 'agent-tokens' ? ' active' : ''}`}
        >
          <svg viewBox="0 0 24 24"><path d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" /></svg>
          Agent Tokens
        </a>
      </nav>

      <nav className="nav-group">
        <a
          href={`/tenants/${tenantId}/docs`}
          className={`nav-item${activePage === 'docs' ? ' active' : ''}`}
        >
          <svg viewBox="0 0 24 24"><path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" /></svg>
          Docs
        </a>
      </nav>

      <div className="sidebar-footer">
        <div className="user-card">
          <div className="user-avatar">{initial}</div>
          <div className="user-meta">
            <strong>{userEmail}</strong>
            <span>Tenant</span>
          </div>
        </div>
        <button type="button" className="logout-btn" onClick={handleLogout} title="Sign out">
          <LogOut size={15} />
        </button>
      </div>
    </aside>
  );
}
