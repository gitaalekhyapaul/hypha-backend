# ADR-0014: Agent Key Thumbprint and Issuance Identity

- Status: accepted
- Date: 2026-08-12
- Milestone: M4
- Related decisions: ADR-0011, ADR-0013
- Security review trigger: agent key binding and capability issuance identity

## Context

ADR-0011 fixes `agent_key_thumbprint` at 32 bytes and uses its lowercase hexadecimal
form as the PolicyIR agent subject. ADR-0013 requires `razield` to derive that
thumbprint from the supplied Ed25519 public key before verifying an issuance
signature.

Neither decision defines the exact derivation bytes. Rust, TypeScript, and future
verifiers can produce different identities from the same key by hashing raw Ed25519
bytes, SubjectPublicKeyInfo DER, or a JWK representation.

Two related statements need one precise interpretation:

- ADR-0011 calls `issuance_nullifier` the duplicate or conflict key.
- ADR-0013 assigns the exact request commitment to BullMQ's custom job ID.

ADR-0013 also requires changed request fields to fail before queue publication.
`agent_id` is intentionally absent from the ADR-0011 commitment and remains
presentation metadata.

## Decision

### Version 1 Ed25519 key input

`CreateCapabilityRequest.agent_public_key` contains exactly the 32-byte RFC 8032
compressed Ed25519 public key. Admission parses it with strict Ed25519 validation and
rejects malformed or weak keys.

The canonical bytes are the validated key's 32-byte encoding:

```text
canonical_agent_public_key = Ed25519VerifyingKey(agent_public_key).to_bytes()
```

### Agent key thumbprint

Version 1 derives the thumbprint as:

```text
agent_key_thumbprint = SHA-256(canonical_agent_public_key)
```

No DER, JWK JSON, Base64 text, hexadecimal text, key-ID prefix, or Protobuf bytes enter
the hash. The version 1 request accepts only Ed25519, so the algorithm is fixed by the
protocol version. Support for another key algorithm requires a new versioned rule.

This matches the 32-byte digest already used after the `ed25519:` prefix in Raziel
authority key IDs. The implementation will share one helper to prevent drift.

Deterministic vector from RFC 8032 test key 1:

```text
agent_public_key = d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
agent_key_thumbprint = 21fe31dfa154a261626bf854046fd2271b7bed4b6abe45aa58877ef47f9721b9
PolicyIR subject = agent:21fe31dfa154a261626bf854046fd2271b7bed4b6abe45aa58877ef47f9721b9
```

### Issuance signature

The 64-byte Ed25519 signature covers exactly the 32-byte ADR-0011
`capability_request_commitment`:

```text
request_signature = Ed25519.Sign(agent_private_key, capability_request_commitment)
```

Admission uses strict Ed25519 verification. It adds no second hash, domain frame,
text encoding, or Protobuf envelope.

### Queue job identity and issuance nullifier

ADR-0013's later queue decision controls BullMQ identity:

```text
job_id = lowercase_hex(capability_request_commitment)
```

The ID is exactly 64 lowercase hexadecimal characters. It contains no prefix or
private request data. Repeating the same signed authorization-bound request resolves
the retained BullMQ job.

`issuance_nullifier` keeps its ADR-0011 formula and remains a proof-bound public value,
conflict identifier, and SP1 proof nonce input. It is not the BullMQ custom job ID.
This clarifies ADR-0011 without changing its hash or any M3 vector.

### Presentation metadata

`agent_id` remains outside the request commitment. Admission bounds it as untrusted
display metadata. It cannot select a subject, job, registry record, audit identity, or
authorization value.

Changing only `agent_id` does not change the commitment or job ID. Tests that mutate
every signed request field exclude `agent_id` and separately confirm this metadata
rule. Product surfaces must display the key-derived subject as the authoritative
identity.

### Timestamp transport

`requested_expires_at` must contain whole UTC seconds. Admission rejects a missing
timestamp, negative seconds, or nonzero nanoseconds. This prevents two Protobuf
timestamps from mapping to one signed ADR-0011 value.

At API admission:

```text
requested_expires_at_unix_seconds >= trusted_now_unix_seconds + 75 minutes
```

The proof worker repeats the check at its trusted start time. Final expiry remains no
later than worker start plus 75 minutes and no later than the request, policy,
snapshot, delegation, or key bounds.

## Security and Privacy Effects

- One key produces one cross-language version 1 agent subject.
- Strict key parsing and weak-key rejection happen before queue publication.
- Private request context remains covered by the existing nonce-hardened commitment.
- BullMQ sees only a public commitment digest as its custom ID.
- `agent_id` cannot impersonate another proof subject or become an audit identity.
- Logs and public errors cannot contain key bytes, signatures, commitments, nonces, or
  private context values.

## Test Requirements

- Match the fixed RFC 8032 thumbprint vector in Rust and TypeScript.
- Reject public keys with the wrong length, malformed encodings, and weak points.
- Verify a valid signature over the exact 32-byte commitment.
- Reject signature length changes, byte mutations, another public key, and another
  commitment.
- Prove that every authorization-bound request field changes the commitment or fails
  validation.
- Prove that changing only `agent_id` leaves the commitment and job ID unchanged.
- Reject missing timestamps, nonzero nanoseconds, negative seconds, overflow, expired
  requests, and less than the 75-minute admission window.
- Confirm that error and debug strings contain no synthetic secret test values.

## Migration and Compatibility Cost

The approved raw-key hash does not change ADR-0011 request commitment bytes, M3 public
values, the guest ELF, or existing proof vectors. It adds a new key-derivation vector
and service admission tests. Snapshot relationships used by the product must name the
derived key subject before capability issuance.

The current synthetic M3 fixtures use fixed thumbprint bytes without an agent key.
They remain valid proof-semantic fixtures. End-to-end service fixtures must use a real
synthetic Ed25519 key and its derived thumbprint.

Selecting DER, JWK, or a new domain-separated hash changes the key subject and needs
matching Rust and TypeScript vectors. No accepted real-data snapshot exists yet, so
there is no production relationship migration.

Adding `agent_id` to the commitment has a larger cost. It changes canonical request
bytes, checked-in vectors, the guest program, the verification-key ID, and proof
artifacts. That option requires an ADR-0011 amendment and a new M3 proof gate.

## Options Not Selected

### SHA-256 of SubjectPublicKeyInfo DER

This binds the algorithm identifier but requires DER construction in each language.
Version 1 already fixes Ed25519 and carries raw key bytes.

### RFC 7638 JWK thumbprint

This adds canonical JSON and Base64url rules to a binary Protobuf protocol. No current
component uses JWK as its canonical key form.

### A new Raziel domain-separated key hash

This gives protocol-level algorithm separation but would differ from the digest in
existing Raziel Ed25519 key IDs. A future multi-algorithm version can define an
algorithm-tagged identity.

### Use `issuance_nullifier` as the BullMQ job ID

This conflicts with ADR-0013's later custom-ID rule and adds another hash without
changing deduplication behavior.

### Add `agent_id` to the existing commitment

This changes ADR-0011 canonical bytes, checked-in vectors, and proof public values.
Version 1 does not trust this display label.

## Approval

The maintainer approved this decision on 2026-08-12. The approval accepts:

1. SHA-256 over validated raw 32-byte Ed25519 public-key bytes as the version 1
   thumbprint.
2. Strict Ed25519 verification over the exact 32-byte request commitment.
3. Lowercase hexadecimal request commitment as the BullMQ job ID, with the issuance
   nullifier retained as a separate proof-bound value.
4. `agent_id` as unsigned, non-authoritative display metadata.
5. Whole-second timestamp transport and the 75-minute API admission check.
