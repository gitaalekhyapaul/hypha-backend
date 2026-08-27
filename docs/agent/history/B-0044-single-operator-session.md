# Engineering Baton Archive

- Baton ID: B-0044
- Final state: REVIEW
- Milestone: M5, single-operator session and CSRF boundary
- Owner: Codex
- Started UTC: 2026-08-26T21:43:50Z
- Completed UTC: 2026-08-26T21:52:40Z
- Branch: `main`

## Objective

Implement the ADR-0018 single-operator authentication boundary with an owner-only
secret record, bounded login input, a signed `HttpOnly` session cookie, exact Origin
checks, and a session-bound CSRF token for state-changing owner routes.

## Acceptance Criteria

- [x] One strict owner-only secret record supplies the operator ID, scrypt password
      verifier, and session HMAC key without environment or log exposure.
- [x] Login accepts one bounded JSON body, compares password material in constant
      time, and returns only operator identity, session expiry, and CSRF token.
- [x] The session cookie is `HttpOnly`, `SameSite=Strict`, path `/`, and `Secure` in
      live mode.
- [x] Session verification checks format, signature, trusted whole-second time,
      operator identity, issuance, and expiry.
- [x] State-changing authorization requires exact configured Origin and the
      session-bound CSRF header with a constant-time comparison.
- [x] Invalid credentials, malformed cookies, wrong Origin, missing CSRF, expired
      sessions, unsafe secret files, and oversized bodies fail with bounded codes.
- [x] Focused tests, console checks, and `git diff --check` pass.

## Architecture Constraints

- The browser never receives the password verifier, HMAC key, secret path, policy
  authority key, or owner service credentials.
- Authentication does not grant policy approval by itself. The later approval route
  still needs one explicit single-use approval nonce.
- Session state does not enter Redis, the proof queue, capabilities, proof public
  values, or audit receipts.

## Files Changed

- `apps/demo-console/lib/operator-auth/`
- `apps/demo-console/app/api/operator/login/route.ts`
- `apps/demo-console/app/api/operator/session/route.ts`
- `apps/demo-console/app/api/operator/logout/route.ts`
- `apps/demo-console/test/operator-auth.test.ts`
- `docs/agent/CURRENT-BATON.md`
- `docs/agent/history/B-0044-single-operator-session.md`

## Decisions

- Used Node's built-in scrypt with fixed `N=32768`, `r=8`, `p=1`, a 16-byte salt,
  and a 32-byte verifier.
- Used a 32-byte HMAC key and domain-separated HMAC-SHA256 over one strict canonical
  session payload.
- Fixed the session lifetime at eight hours. The token carries a random 16-byte
  session ID and random 32-byte CSRF token.
- Local mode uses `raziel_operator`. Live mode uses the secure
  `__Host-raziel_operator` cookie prefix.
- Accepted only regular, single-link, same-owner secret files with mode `0400` or
  `0600`, read through a checked no-follow handle.

## Validation Evidence

| Command or check | Result | Scope | Timestamp UTC |
| --- | --- | --- | --- |
| `./scripts/dev pnpm --filter @raziel/demo-console test` | PASS, 30 tests | Pinned console and route tests | 2026-08-26T21:52:40Z |
| `./scripts/dev pnpm --filter @raziel/demo-console typecheck` | PASS | Pinned TypeScript | 2026-08-26T21:52:40Z |
| `./scripts/dev pnpm --filter @raziel/demo-console lint` | PASS, 29 files | Pinned Biome lint | 2026-08-26T21:52:40Z |
| `./scripts/dev pnpm --filter @raziel/demo-console build` | PASS, three dynamic auth routes | Pinned Next.js build | 2026-08-26T21:52:40Z |
| Direct pinned Biome format check | PASS, 10 files | B-0044 source and tests | 2026-08-26T21:52:40Z |
| Auth response secret-field scan | PASS | Public auth routes and types | 2026-08-26T21:52:40Z |
| `git diff --check` | PASS | Worktree | 2026-08-26T21:52:40Z |

## Risks And Private-Data Notes

- Logout clears the browser cookie. Sessions are stateless and remain
  cryptographically valid until their eight-hour expiry. M5 does not add a
  distributed session-revocation database.
- Deployment still needs a provisioning command that writes the strict owner secret
  record without placing the password in source control or shell history.
- Candidate and approval routes must call `authorizeMutation` before reading or
  acting on owner input.

## Exact Next Action

Expose B-0043 candidate preparation through an authenticated, Origin-checked,
CSRF-protected owner route and return only the safe candidate projection.
