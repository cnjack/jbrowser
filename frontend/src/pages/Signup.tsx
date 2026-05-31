import { FormEvent, useState } from 'react';
import { Globe } from 'lucide-react';
import { signup } from '../api/client';
import { useAuthStore } from '../stores/auth';

export function Signup() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const setAuth = useAuthStore((s) => s.setAuth);

  // Check for invite_token in URL
  const params = new URLSearchParams(window.location.search);
  const inviteToken = params.get('invite_token') ?? undefined;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }
    if (password !== confirmPassword) {
      setError('Passwords do not match');
      return;
    }
    setLoading(true);
    try {
      const res = await signup(email, password, displayName, inviteToken);
      setAuth(res.access_token, res.user, res.tenants);
      window.history.pushState(null, '', `/tenants/${res.tenants[0]?.id}/browsers`);
      window.location.reload();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Signup failed');
    } finally {
      setLoading(false);
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
        <form className="login-card" onSubmit={handleSubmit}>
          <p className="eyebrow">Get started</p>
          <h2>Create account</h2>
          <p>Sign up for a new JBrowser account.</p>
          {error ? <div className="error-msg">{error}</div> : null}
          <div className="field">
            <label htmlFor="displayName">Display Name</label>
            <input
              id="displayName"
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder="Your name"
              required
            />
          </div>
          <div className="field">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@company.com"
              required
            />
          </div>
          <div className="field">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Min. 8 characters"
              required
              minLength={8}
            />
          </div>
          <div className="field">
            <label htmlFor="confirmPassword">Confirm Password</label>
            <input
              id="confirmPassword"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder="Re-enter password"
              required
            />
          </div>
          <button type="submit" className="btn-primary" disabled={loading}>
            {loading ? 'Creating…' : 'Create Account'}
          </button>
          <p className="auth-alt-link">
            Already have an account? <a href="/login">Sign in</a>
          </p>
        </form>
      </section>
    </main>
  );
}
