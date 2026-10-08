# VM Runbook And Secret-Loading Correction

Date: 2026-10-08

Parent baton: B-0067. Scope: documentation correction, not live demo acceptance.

## Request

Separate backend and agent commands, explain SSH versus ngrok, and document the
placement and consumption of the supplied credentials.

## Changes

- Split the full deployment guide into VM backend, VM agents, private credential
  files, Mac forwarding, API ingress selection, and browser registration.
- Document `hypha-backend/.env.m6` through `scripts/compose-m6` and Compose interpolation.
- Document `raziel-demo-agents/.env.agents` and the separate SoC file consumed through
  the agent services' `env_file` entries.
- Distinguish generated Ed25519 identities from operator-issued API tokens retained
  in private agent state. Neither is the supplied SoC LLM key.
- Correct the plan's stale deferred-publication statement. Agents use public SDK
  `0.1.0`; no package mutation is part of this correction.
- Label ngrok as optional. The SDK requires HTTPS outside loopback development.
  Current agent Compose does not provide a private TLS frontend to the backend.

## Source Checks

Read the backend Compose wrapper, M6 overlays, operator initializer, and policy-key
initializer. Read agent Compose, configuration, identity, storage, and connection
code. Read the SDK origin validator. No private credential file was opened.

## Validation

| Check | Result | Scope |
| --- | --- | --- |
| Relative Markdown links | PASS | Changed guides, plan, baton, and this record |
| Shell example syntax | PASS | Bash fences in both deployment guides; syntax only |
| Settings ignore rules | PASS | Backend `.env.m6` and agent `.env.agents` stay ignored |
| Diff whitespace | PASS | Documentation changes |
| Runtime and VM validation | SKIP | No services, remote source, credential file, or VM config changed |

## Next Action

Select a trusted VM-reachable HTTPS API route with the maintainer. SSH forwarding
remains the browser route. A private VM TLS frontend needs deployment work and any
missing dependency approval. The public tunnel option needs exposure approval.
Then follow the split guide for provisional VM startup and Agent A's real flow.
M6/M7 connected proof and recording gates remain pending.
