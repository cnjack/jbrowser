import { FormEvent, useState } from 'react';
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
      <form className="card login-card" onSubmit={handleSubmit}>
        <p className="eyebrow">Remote Browser Control</p>
        <h1>Sign in to JBrowser</h1>
        <label>
          Email
          <input value={email} onChange={(event) => setEmail(event.target.value)} />
        </label>
        <label>
          Password
          <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} />
        </label>
        {error ? <p className="error">{error}</p> : null}
        <button type="submit">Sign in</button>
      </form>
    </main>
  );
}

