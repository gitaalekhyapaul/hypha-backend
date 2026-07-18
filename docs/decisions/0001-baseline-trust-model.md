# ADR-0001: Baseline Policy and Proof Trust Model

- Status: accepted
- Date: 2026-07-16
- Applies to: Raziel payment-reconciliation PoC

## Context

Raziel needs to hide private relationships and authorization paths from the AI agent.
The independent verifier still needs to know which approved policy it is checking.

A fully private policy would only prove that an unspecified hidden policy returned
`ALLOW`. Independent audit would then depend on a second policy-authority attestation.
Small private policies face guessing attacks, even with commitments. Repeated requests
can still act as an inference oracle.

## Decision

Keep the exact PolicyIR in a trusted local registry available to `verifierd` and the
auditor. Keep PolicyIR out of the capability and agent context.

The data-owner policy authority reviews and signs the policy record before registration.
`razield` cannot assign approved status. The snapshot authority signs a separate
descriptor bound to the approved `policy_hash`. One PoC administrator can operate both
roles through separate key identifiers, but the artifacts and verification steps stay
separate.

The capability carries:

- `policy_hash`
- approved public capability claims
- relationship snapshot root
- validity window
- proof package or verifier receipt

The verifier maps `policy_hash` to an approved PolicyIR record. It checks policy status,
authority signature, and proof public values.

Keep these values private to Raziel:

- relationship tuples
- satisfying paths and branch choices
- Merkle openings
- stored caveat context
- sensitive policy parameters

Publish these semantics:

- PolicyIR interpreter behavior
- supported SpiceDB and caveat subset
- canonical encoding rules
- capability and public-value formats

## Consequences

- The verifier checks cryptographic evidence against an inspectable approved policy.
- The agent cannot inspect the policy graph or satisfying relationship path.
- Policy approval remains a separate trust root from proof correctness.
- Policy hash commitments need a canonical, versioned encoding.
- A verifier needs a local policy registry and approval-key configuration.
- Fully private PolicyIR remains a separate research experiment.

## Rejected baseline

Do not ship PolicyIR to the agent for local preflight evaluation. That design reveals
policy structure and creates an inference surface.

Do not let `verifierd` fetch policy from `razield`. That design breaks verifier
independence and lets the prover select the policy after seeing the request.

Do not treat a signed JWT containing `ALLOW` as equivalent proof. A signature proves
who made a claim. The Raziel proof establishes evaluation against committed inputs.

## Deferred experiment

Test private PolicyIR only after the baseline works. Use a salted commitment such as:

```text
policy_commitment = H(
  domain_separator,
  private_policy_salt,
  canonical_policy_ir
)
```

Compare proof cost, auditability, inference exposure, and implementation complexity.
