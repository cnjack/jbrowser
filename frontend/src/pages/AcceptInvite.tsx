import { useEffect, useState } from 'react';
import { Globe } from 'lucide-react';
import { validateInvite, acceptInvite } from '../api/client';
import type { InviteValidation } from '../api/client';
import { useAuthStore } from '../stores/auth';

interface Props {
  token: string;
}

export function AcceptInvite({ token }: Props) {
  const [invite, setInvite] = useState<InviteValidation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [accepting, setAccepting] = useState(false);
  const isLoggedIn = useAuthStore((s) => !!s.token);
  const setAuth = useAuthStore((s) => s.setAuth);

  useEffect(() => {
    validateInvite(token)
      .then((data) => {
        setInvite(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err instanceof Error ? err.message : 'Invalid or expired invitation');
        setLoading(false);
      });
  }, [token]);

  async function handleAccept() {
    setAccepting(true);
    setError(null);
    try {
      const res = await acceptInvite(token);
      setAuth(res.access_token, res.user, res.tenants);
      window.history.pushState(null, '', `/tenants/${res.tenants[0]?.id}/browsers`);
      window.location.reload();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to accept invitation');
      setAccepting(false);
    }
  }

  return (
    <main className="auth-shell">
      <section className="auth-left">
        <div className="brand">
          <span className="brand-icon"><Globe size={18} strokeWidth={2.2} /></span>
          <span className="brand-name">
            <span className="brand-bracket">[</span>
            <span className="brand-j">J</span>
            <span className="brand-text">Browser</span>
            <span className="brand-bracket">]</span>
          </span>
        </div>
        <h1 className="auth-hero">
          Fleet your <em>browsers</em> at scale.
        </h1>
        <ul className="auth-features">
          <li>Headless Chromium on demand</li>
          <li>CDP access in milliseconds</li>
          <li>Multi-tenant, audit-logged</li>
        </ul>
        <div className="auth-stamp">v1.0 · remote browser control</div>
      </section>

      <section className="auth-right">
        <div className="login-card">
          <p className="eyebrow">Team invitation</p>

          {loading && (
            <>
              <h2>Validating…</h2>
              <p>Checking your invitation link.</p>
            </>
          )}

          {error && !loading && (
            <>
              <h2>Invitation error</h2>
              <div className="error-msg">{error}</div>
              <a href="/login" className="btn-primary" style={{ display: 'block', textAlign: 'center', marginTop: 16 }}>
                Go to login
              </a>
            </>
          )}

          {invite && !loading && !error && (
            <>
              <h2>Join {invite.tenant_name}</h2>
              <p>
                You&apos;ve been invited to join <strong>{invite.tenant_name}</strong> as
                a <strong>{invite.role}</strong>.
              </p>

              {isLoggedIn ? (
                <button
                  type="button"
                  className="btn-primary"
                  style={{ width: '100%', marginTop: 16 }}
                  onClick={handleAccept}
                  disabled={accepting}
                >
                  {accepting ? 'Joining…' : `Join ${invite.tenant_name}`}
                </button>
              ) : (
                <div className="invite-actions">
                  <a
                    href={`/login?redirect=${encodeURIComponent(`/invite/${token}`)}`}
                    className="btn-primary"
                    style={{ display: 'block', textAlign: 'center', marginTop: 16 }}
                  >
                    Sign in to join
                  </a>
                  <a
                    href={`/signup?invite_token=${encodeURIComponent(token)}`}
                    className="btn-secondary"
                    style={{ display: 'block', textAlign: 'center', marginTop: 8 }}
                  >
                    Create account
                  </a>
                </div>
              )}
            </>
          )}
        </div>
      </section>
    </main>
  );
}
