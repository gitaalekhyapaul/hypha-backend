"use client";

import { type FormEvent, useState } from "react";

interface LoginViewProps {
  readonly busy: boolean;
  readonly error: string | null;
  readonly onLogin: (password: string) => Promise<void>;
}

export function LoginView({ busy, error, onLogin }: LoginViewProps) {
  const [password, setPassword] = useState("");

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onLogin(password);
  }

  return (
    <main className="login-shell">
      <header className="login-brand">
        <span className="brand-mark" aria-hidden="true">
          R
        </span>
        <div>
          <strong>Raziel</strong>
          <span>Proof-carrying authorization</span>
        </div>
      </header>

      <section className="login-panel" aria-labelledby="login-heading">
        <div className="login-context">
          <span className="classification-flag">Synthetic workspace</span>
          <h1 id="login-heading">Operator sign in</h1>
          <dl className="login-facts">
            <div>
              <dt>Scenario</dt>
              <dd>Payment reconciliation</dd>
            </div>
            <div>
              <dt>Environment</dt>
              <dd>Local development</dd>
            </div>
            <div>
              <dt>Session</dt>
              <dd>8 hours</dd>
            </div>
          </dl>
        </div>

        <form className="login-form" onSubmit={submit}>
          <label htmlFor="operator-password">Operator password</label>
          <input
            id="operator-password"
            name="password"
            type="password"
            autoComplete="current-password"
            value={password}
            disabled={busy}
            onChange={(event) => setPassword(event.target.value)}
            required
          />
          {error ? (
            <p className="form-error" role="alert">
              {error}
            </p>
          ) : null}
          <button className="primary-button login-button" type="submit" disabled={busy}>
            {busy ? "Signing in..." : "Sign in"}
          </button>
        </form>
      </section>

      <footer className="login-footer">
        <span>Raziel M5</span>
        <span>Owner boundary</span>
      </footer>
    </main>
  );
}
