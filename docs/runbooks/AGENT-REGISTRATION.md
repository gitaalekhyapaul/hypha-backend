# Agent Registration And SDK Authentication

Status: B-0064 implemented and tested locally, not deployed to the NUS VM.

## Available Flows

1. Sign in to the operator portal and open **Agents**.
2. Choose **Register agent**, enter a display name and the agent's raw Ed25519 public
   key as 64 hexadecimal characters. The private key stays with the agent.
3. Select the registration and create an API token. Choose a lifetime from 1 to 90 days.
4. Copy the token into the agent server's secret configuration. The portal shows it
   once. Closing the token display or leaving Agents clears that display.
5. Revisit the registration to inspect token status and expiry. Revoke a token to
   block later authentication. Rotation uses another token for the same registered key.

A token identifies a registered caller. It does not grant dataset access or create a
capability. The [access inbox](AGENT-ACCESS-INBOX.md) now supports request inspection,
rejection, and cancellation. Approval and proof dispatch remain unfinished.

## Runtime

The existing M5 Compose profile now starts Redis for the console and builds
`raziel-agent-auth` with the other owner tools. No new host package is required.
Use [the M5 startup runbook](M5-LOCAL-PORTAL.md) after the code is committed and pulled.
Do not treat these uncommitted local changes as already present on the VM.
The separate [M6 preview](AGENT-ACCESS-INBOX.md#local-m6-preview) now includes
registration, the inbox, and versioned policy publication. It does not issue proofs.

| Variable | Purpose |
| --- | --- |
| `RAZIEL_REDIS_URL` | Private Redis endpoint, supplied by Compose |
| `RAZIEL_TRUST_DOMAIN_ID` | API audience and registry namespace, default `raziel-local-dev` |
| `RAZIEL_AGENT_AUTH_EXECUTABLE` | Linux Rust helper, built inside the pinned toolchain |
| `RAZIEL_OPERATOR_SECRET_PATH` | Existing operator authentication configuration |
| `RAZIEL_OPERATOR_EXPECTED_ORIGIN` | Existing browser origin and CSRF boundary |

Keep the trust-domain ID stable. A change selects a different registration namespace.
Keep Redis private and retain its volume. Only token hashes, metadata, public keys,
and short-lived replay markers are stored. Tokens and private keys are never logged.

Use the package's `dev` or `start` commands, not direct `next dev` or `next start`.
The [Next.js custom HTTP entry point](https://nextjs.org/docs/app/guides/custom-server)
checks raw authentication headers and preserves exact request targets. This avoids
[Node's duplicate-header normalization](https://nodejs.org/api/http.html#messageheaders).
It overwrites caller-supplied internal target headers. No extra proxy or public port
is introduced. A deployed proxy must not rewrite signed paths or query strings.

## SDK Candidate

Source lives in the sibling `raziel-sdk-ts` repository. The backend consumes the
local `@gitaalekhyapaul/raziel-sdk-ts@0.1.0` tarball through a compatibility re-export.
It is not yet available from npm. See the [artifact record](../../packages/sdk-ts/vendor/README.md).

```ts
import { createRazielClient } from "@gitaalekhyapaul/raziel-sdk-ts";

const raziel = createRazielClient({
  baseUrl: process.env.RAZIEL_API_URL,
  audience: process.env.RAZIEL_TRUST_DOMAIN_ID,
  tokenProvider: () => agentToken,
  signer: persistentAgentSigner,
});
const registration = await raziel.me();
```

The candidate now includes `createAccessRequest`, `listAccessRequests`,
`getAccessRequest`, and `cancelAccessRequest` with generated public DTOs. The sibling
README contains the typed examples. These are explicit calls, with no automatic
proof submission or retry. Persist the intent ID and context in the agent application.

The application supplies its token and signer. The signer exposes a raw 32-byte
public key and `sign(message): Promise<Uint8Array>`. Each call signs a fresh nonce,
timestamp, token ID, audience, method, target, and body digest. Keep these operations
in the agent server, not its browser dashboard. Remote URLs require HTTPS.

## Validation And Limits

- Rust and SDK share exact signature vectors. Invalid and weak keys are rejected.
- The live local test uses the packaged SDK, production HTTP server, Rust helper,
  and Redis. It verifies two-agent separation, concurrent replay, token rotation,
  revocation, operator/agent route isolation, and malformed raw requests.
- The desktop registration and token workflow passed a browser check with a mocked
  transport. The final integrated browser recheck was unavailable through the in-app
  browser. HTTP validation passed against the running local server.
- No proof was generated for this authentication slice. Existing proof semantics are unchanged.
- Public release, approval, policy migration, agent dashboards, and VM E2E remain pending.
