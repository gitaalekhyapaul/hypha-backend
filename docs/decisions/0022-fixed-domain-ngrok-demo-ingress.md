# ADR-0022: Fixed-Domain Ngrok Demo Ingress

- Status: accepted
- Date: 2026-10-08
- Baton: B-0067 deployment follow-up
- Approval: maintainer selected Docker-managed ngrok and supplied the fixed domain.
  The pinned image download received separate approval.

## Decision

Run an opt-in ngrok container on the active backend host. Use the account domain
`https://either-anymore-immunize.ngrok-free.dev` through explicit configuration.
The domain is a deployment address, not an agent identity or protocol audience.
Both agent dashboards prefill it from `RAZIEL_API_BASE_URL`. Retained connections
take precedence. Each agent still needs its own registered key, token, and audience.

Pin official image `ngrok/ngrok:3.39.11-alpine` to manifest digest
`sha256:187e588f6c4efe3b29cd3eea9fcd768a1afa7342319e3cee9aeb5af6a9cf64fd`.
The manifest contains native arm64 and amd64 images. Startup never pulls an image.
The same container configuration serves the VM stack or a running Mac backend.
The Mac option starts ingress only. It does not make local proof generation viable.

Pass the fixed HTTPS origin with `--url`. Disable endpoint pooling. Stop the old
tunnel before another backend uses the domain. Do not distribute requests across
independent installations. Switching hosts transfers no registrations or private state.
These settings follow the [official CLI reference](https://ngrok.com/docs/gateway/agent/cli).

## Exposure Boundary

The traffic policy accepts the existing signed agent API methods and dataset
protected-query POST route. Other paths return a generic 404. It forwards permitted
requests unchanged. The backend still checks tokens, signatures, scope, and proofs.
The policy is not a substitute for authorization. The operator and agent dashboards
remain local or SSH-forwarded. No owner RPC, database, Redis, or proof port is public.

### Public Liveness Amendment, 2026-10-15

The maintainer explicitly requested an unauthenticated deployment health route.
Allow only GET `/api/health` through ingress. Return HTTP 200 with exactly
`{"status":"ok"}` and `Cache-Control: no-store`. Set no cookie. Read no private
configuration or downstream service state. Other methods and health subpaths remain
outside the public allowlist.

This exposes console-process liveness and public routing, not policy decisions,
agent metadata, queue depth, datasets, credentials, or proof readiness. It does not
relax any token, signature, scope, replay, or proof check on protected routes.

In the integrated VM stack, ngrok joins its own egress network and an internal
upstream network shared only with the console. It has no owner-network membership,
Docker socket, registry mount, key mount, or witness mount. The container runs
non-root with a read-only filesystem and dropped capabilities.

Disable local traffic inspection and its inspection database. Bind the health API
to container loopback without a published port. Turn off remote management and
update checks. Configuration follows the [v3 reference](https://ngrok.com/docs/gateway/agent/config/v3).
The deny response uses the documented
[custom-response action](https://ngrok.com/docs/gateway/traffic-policy/actions/custom-response).

## Secrets And Provider Trust

Keep `NGROK_AUTHTOKEN` in ignored `.env.m6` on the VM or `.env.ngrok` for standalone
Mac ingress. Inject it into the tunnel role only. Do not put it in Git, chat,
command arguments, agent settings, or diagnostic output.

Ngrok terminates public HTTPS in its cloud service. That provider can see permitted
API traffic, including bearer tokens and authorized synthetic rows. Turning off
local inspection does not establish a provider retention or confidentiality guarantee.
Use this route for the approved synthetic demo, not private production deployment.
Proof witnesses and owner state never travel through the public facade.

The free account's assigned domain supports the selected route. Account quotas and
cloud traffic-policy acceptance remain live deployment checks. Browser interstitials
do not require SDK changes for programmatic API requests, per the
[free-plan documentation](https://ngrok.com/docs/pricing-limits/free-plan-limits).

## Alternatives

A private TLS frontend avoids this cloud trust boundary but needs certificate and
network provisioning. The maintainer selected ngrok for the portable demo instead.
A manually started host tunnel remains unnecessary. Disabling SDK TLS checks or
joining agents to owner networks is rejected.

## Validation

Check both rendered deployments, exact image digest, networks, mounts, fixed URL,
credential scope, and disabled pooling/inspection. Exercise the route allowlist with
positive and negative vectors. Validate the real pinned CLI and config without an
account token. Test agent defaults and retained connections without changing the SDK.

After private token configuration, check tunnel health, public operator-path rejection,
signed agent registration lookup, and protected-query delivery. Static route tests
and CLI parsing do not establish cloud acceptance or a completed M6/M7 proof demo.
