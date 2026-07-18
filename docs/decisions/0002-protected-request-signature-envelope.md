# ADR-0002: Protected Request Signature Envelope

- Status: proposed for maintainer review
- Date: 2026-07-18
- Applies to: Raziel protected gateway requests

## Context

The capability binds an agent key and public authorization scope. Each gateway request
still needs its own key proof and replay controls. Signing only the capability ID, body
hash, nonce, and timestamp leaves the HTTP method and target outside the signature. A
valid signed body can then be replayed against another route with compatible parsing.

Cached capability claims skip repeated proof verification. They must not skip agent-key
or request-signature verification.

## Proposed decision

Every protected request carries the agent public key and a signature over this logical
record:

```text
ProtectedRequestSignatureInput {
  format_version = 1
  capability_id
  request_method
  canonical_request_target
  request_body_hash
  request_nonce
  request_timestamp
}
```

The signature input uses a versioned, length-prefixed encoding with the ASCII domain
separator `raziel/protected-request/v1`. It does not use delimiter concatenation or raw
Protobuf bytes.

Canonicalization rules:

- Convert the method to uppercase ASCII and reject unknown method tokens.
- Use the HTTP origin-form path beginning with `/`.
- Reject invalid percent escapes and dot segments.
- Decode percent-encoded unreserved path characters.
- Encode remaining escaped bytes with uppercase hexadecimal digits.
- Parse the query as key-value entries, normalize percent encoding as above, reject
  duplicate encoded keys, sort by encoded key, and join entries with `&`.
- Hash the exact received request-body bytes with SHA-256.
- Encode the timestamp as UTC Unix seconds.
- Treat the nonce as opaque bounded bytes and reject reuse for the capability lifetime.

The verifier or gateway recomputes the public-key thumbprint and compares it with the
capability claim before checking the signature. `verifierd` performs these checks on a
cold request. The gateway performs them on every cache hit. A cache hit skips proof
verification only.

## Trust-boundary effect

The agent public key and signature envelope are public request material. PolicyIR,
relationship tuples, paths, openings, stored caveat values, and witness traces remain
private under the existing boundaries.

## Required tests

- Change the method, path, one query key, one query value, or body after signing.
- Reorder query entries and confirm the canonical form stays stable.
- Repeat a query key and confirm the request is rejected before signature verification.
- Change the agent public key and confirm the thumbprint check fails.
- Reuse the nonce on cold and cached paths.
- Confirm cached requests still run signature, timestamp, nonce, and scope checks.

## Rejected options

- Signing the body hash without the method and target permits cross-route replay.
- Trusting a gateway-supplied target string without reconstruction permits mismatch
  between routing and signature checks.
- Skipping signature verification on cache hits turns a copied capability into a bearer
  token.
