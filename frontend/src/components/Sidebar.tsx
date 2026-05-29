import { useAuthStore } from '../stores/auth';

interface Props {
  activePage: 'browsers' | 'tokens';
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
        <a href={`/tenants/${tenantId}/browsers`}>
          <span className="brand-mark" />
          <span className="brand-name">JBrowser</span>
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
      </nav>

      <div className="sidebar-footer">
        <div className="user-card">
          <div className="user-avatar">{initial}</div>
          <div className="user-meta">
            <strong>{userEmail}</strong>
            <span>Tenant</span>
          </div>
        </div>
        <button type="button" className="logout-btn" onClick={handleLogout}>
          Sign out
        </button>
      </div>
    </aside>
  );
}
