# Public Ingress Health Route

- Parent baton: B-0067 deployment follow-up
- Checkpoint UTC: 2026-10-14T22:17:23Z
- Maintainer date: 2026-10-15, Asia/Singapore
- Approval: explicit maintainer request for an unauthenticated ngrok health route.
- Result: implemented and locally checked. Public VM acceptance remains pending.

## Scope And Review

GET `/api/health` returns exactly `{"status":"ok"}` with HTTP 200, no-store, and
nosniff headers. It sets no cookie and reads no auth configuration or downstream
state. The route exposes only ingress/console liveness, not proof-stack readiness.
No policy, identity, queue, registry, dataset, credential, or environment details
are returned. ADR-0022 records this narrow public exposure amendment.

The traffic policy permits the exact GET path. POST, HEAD, trailing slash, and
subpath checks stay outside the public allowlist. Existing signed routes and
operator authentication remain unchanged. No SDK, agent, dependency, proof, or
private-state change belongs to this checkpoint.

## Validation

| Check | Result | Scope |
| --- | --- | --- |
| Health route unit | PASS | Status 200, exact JSON, no-store/nosniff, no cookie, no credentials |
| Existing operator-auth tests | PASS | Seven tests, including origin rejection and session guards |
| Traffic policy vectors | PASS | Ten allowed and sixteen denied cases |
| Console types | PASS | Pinned Docker tsc --noEmit with no external network |
| Formatting/lint | PASS | Existing pinned Biome for route, test, and policy vectors |
| Public VM health | SKIP | Requires maintainer pull and console/tunnel recreation |
| Signed SDK/proof flows | SKIP | Separate live M6/M7 gate |

## Handoff

Push this checkpoint with the preceding auth-tmpfs repair. The maintainer pulls
in the VM backend repo, recreates only the console and tunnel, then runs the GET
probe in [Public Ingress Health](../../runbooks/M7-AGENT-DEMO.md#public-ingress-health).
Do not reset volumes, rebuild proof binaries, restart agents, or change credentials.
HTTP 200 from this route establishes public routing and console liveness only.
