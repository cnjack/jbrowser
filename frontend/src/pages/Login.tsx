import { FormEvent, useState } from 'react';
import { Globe } from 'lucide-react';
import { login } from '../api/browser';
import { useAuthStore } from '../stores/auth';

export function Login() {
  const [email, setEmail] = useState('admin@example.com');
  const [password, setPassword] = useState('jbrowser');
  const [error, setError] = useState<string | null>(null);
  const setSession = useAuthStore((state) => state.setSession);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      const response = await login(email, password);
      setSession(response.access_token, response.user.email, response.tenants);
      window.history.pushState(null, '', `/tenants/${response.tenants[0].id}/browsers`);
      window.location.reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
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
          <p className="eyebrow">Welcome back</p>
          <h2>Sign in</h2>
          <p>Enter your credentials to access the control plane.</p>
          {error ? <div className="error-msg">{error}</div> : null}
          <div className="field">
            <label htmlFor="email">Email</label>
            <input
              id="email"
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              placeholder="you@company.com"
            />
          </div>
          <div className="field">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="Enter password"
            />
          </div>
          <button type="submit" className="btn-primary">Sign in</button>
        </form>
      </section>
    </main>
  );
}

