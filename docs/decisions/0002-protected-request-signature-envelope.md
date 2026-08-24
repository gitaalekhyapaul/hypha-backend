# ADR-0002: Protected Request Signature Envelope

- Status: accepted
- Date: 2026-07-18
- Revised: 2026-08-24
- Accepted: 2026-08-24
- Milestones: M4 verifier admission and M5 gateway enforcement
- Related baton: B-0029
- Security review triggers: key binding, request signatures, capability reuse,
  anti-replay state, and client-visible canonical values

## Context

The capability binds an agent key and public authorization scope. Each gateway request
still needs its own key proof and replay controls. Signing only the capability ID, body
hash, nonce, and timestamp leaves the HTTP method and target outside the signature. A
valid signed body can then be replayed against another route with compatible parsing.

Cached capability claims skip repeated proof verification. They must not skip agent-key
or request-signature verification.

## Proposed Decision

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

### Exact Signed Bytes

The Ed25519 signature covers the output of the repository's existing
`framed_message` construction. The domain is the ASCII byte string
`raziel/protected-request/v1`. The payload writes these fields in order:

```text
u32 big-endian format_version
u32 big-endian length + UTF-8 capability_id
u32 big-endian length + ASCII request_method
u32 big-endian length + ASCII canonical_request_target
u32 big-endian length + 32 request_body_hash bytes
u32 big-endian length + 32 request_nonce bytes
i64 big-endian request_timestamp Unix seconds
```

`framed_message` writes a four-byte domain length, the domain bytes, a four-byte
payload length, and the payload bytes. The signature covers those bytes directly. It
does not cover raw Protobuf, JSON, delimiter-concatenated text, or a second hash of
the canonical message.

The fixed field rules are:

- `format_version` equals `1`.
- `capability_id` is exactly 64 lowercase hexadecimal ASCII characters and equals the
  capability claim.
- `request_method` is one of `GET`, `POST`, `PUT`, `PATCH`, or `DELETE`.
- `canonical_request_target` is non-empty ASCII and at most 2,048 bytes.
- `request_body_hash` is exactly 32 bytes and equals SHA-256 over the exact received
  request-body bytes. An empty body uses SHA-256 of zero bytes.
- `request_nonce` is exactly 32 opaque random bytes.
- `request_timestamp` uses whole UTC Unix seconds. Protobuf nanoseconds must equal
  zero.
- `agent_public_key` is one raw 32-byte Ed25519 public key.
- `agent_request_signature` is one canonical 64-byte Ed25519 signature.

### Strict Protobuf Ingress

The verifier API decodes raw `VerifyCapabilityRequest` bytes against the frozen
version 1 descriptor before typed conversion. It rejects unknown fields, extensions,
duplicate singular fields, explicit defaults, alternate encodings, malformed bytes,
and requests larger than 2 MiB. Protobuf remains transport data and never becomes a
signature or commitment input.

Version 1 embeds
`services/verifierd/descriptors/raziel-verification-v1.binpb`. Its SHA-256 checksum is
`80774013649e64d7cfbe615de9ecee4180c16a214a93c464f14a532a6f64957d`.
The reviewed regeneration command is:

```bash
./scripts/dev buf build \
  --path proto/raziel/v1/verifier_service.proto \
  --as-file-descriptor-set \
  --exclude-source-info \
  -o services/verifierd/descriptors/raziel-verification-v1.binpb
```

Normal generation does not replace this descriptor. A request-contract change needs
protocol review, a checksum update, and strict-ingress mutation tests.

### Canonical Method And Target

The agent SDK uppercases its configured method and rejects a value outside the fixed
method list. The gateway and `verifierd` require the signed method to already use that
uppercase form.

The agent SDK canonicalizes the target before signing. The gateway reconstructs the
same target from the framework's routed HTTP request. It never accepts a
caller-supplied canonical target header. `verifierd` requires that the RPC target is
already canonical and rejects another spelling of the same value.

The target uses HTTP origin form:

```text
/path/segments?key=value&next=value
```

It has no scheme, authority, user information, or fragment. The path rules are:

- The path starts with `/`. Only `/` can end with `/`.
- Empty path segments, repeated `/`, backslash, ASCII controls, DEL, and spaces are
  rejected.
- Inside each segment, only literal ASCII unreserved characters
  `A-Z a-z 0-9 - . _ ~` or percent escapes are accepted. `/` remains the segment
  separator.
- Every percent escape contains two hexadecimal digits. Hexadecimal letters become
  uppercase.
- Escapes for unreserved bytes decode to their literal character.
- Escapes for `/`, `\\`, `?`, or `#` are rejected to avoid router ambiguity.
- Other escaped bytes retain their percent form with uppercase hexadecimal digits.
- A segment equal to `.` or `..` after unreserved decoding is rejected.
- Raw non-ASCII bytes are rejected. Non-ASCII path data uses percent escapes and
  retains its exact octet sequence.

The query rules are:

- A target with no query omits `?`. An empty query and empty query entries are
  rejected.
- Split entries only on `&`. Each entry is `key=value`. A missing `=` normalizes to
  an empty value and emits the trailing `=`.
- Keys are non-empty. Keys and values contain only literal unreserved ASCII, valid
  percent escapes, or literal `+`. Unreserved escapes decode, other escapes retain
  uppercase hexadecimal digits, and literal `+` normalizes to `%2B`. It never means
  a space.
- Reject duplicate keys after normalization. This rejects `a=1&%61=2`.
- Sort entries by normalized key bytes and join them with `&`.
- Query values remain bound exactly after percent normalization. They are not sorted,
  parsed as numbers, or Unicode-normalized.

These examples are normative:

| Input before canonicalization | Canonical signed value |
| --- | --- |
| method `post` | `POST` |
| `/payments/%72econcile?status=pending&account=%61-1` | `/payments/reconcile?account=a-1&status=pending` |
| `/payments?flag&q=a+b` | `/payments?flag=&q=a%2Bb` |
| `/r%C3%A9sum%C3%A9` | `/r%C3%A9sum%C3%A9` |

These targets are invalid:

| Target | Rejection reason |
| --- | --- |
| `//payments` | empty path segment |
| `/payments/` | trailing slash outside root |
| `/payments/../admin` | dot segment |
| `/payments/%2fadmin` | escaped path delimiter |
| `/payments?a=1&%61=2` | duplicate key after normalization |
| `/payments?` | empty query |

The gateway compares the reconstructed method and target with capability scope before
resource access. Direct callers of `verifierd` receive verified claims but cannot use
that service as a policy enforcement point.

### Time And Replay

`verifierd` and the gateway use trusted whole-second time. They reject a request when:

- its timestamp is more than 300 seconds old
- its timestamp is more than 30 seconds in the future
- the capability is not active at both the request timestamp and trusted current time

For trusted time `1,800,000,000`, timestamps `1,799,999,700` and `1,800,000,030`
pass the freshness boundary. Timestamps `1,799,999,699` and `1,800,000,031` fail.
Capability validity can still reject either passing timestamp.

The gateway owns one atomic nonce store for both cold and cached requests. It consumes
`(capability_id, request_nonce)` before calling `verifierd` or using cached claims and
retains the entry until capability expiry. A service failure does not release the
nonce. The agent retries with a fresh nonce and signature.

`verifierd` validates nonce length, signature binding, freshness, and capability time.
It does not own a second replay store. This keeps one replay authority at the policy
enforcement point.

### Key And Cache Rules

The verifier and gateway derive the raw-key thumbprint and compare it with the
capability claim before signature verification. `verifierd` performs these checks on
a cold request. The gateway performs them on every cache hit. A cache hit skips only
proof verification. It never skips key, signature, time, nonce, target, or scope
checks.

## Trust-Boundary Effect

The agent public key and signature envelope are public request material. PolicyIR,
relationship tuples, paths, openings, stored caveat values, and witness traces remain
private under the existing boundaries.

## Required Tests

- Freeze matching Rust and TypeScript canonical byte vectors.
- Freeze the verifier request descriptor and reject unknown or noncanonical wire
  spellings before typed admission.
- Change each signed field and confirm signature failure.
- Change the method, path, query key, query value, or body after signing.
- Reorder query entries and confirm canonical bytes stay stable.
- Test percent-escape case, unreserved decoding, literal plus, encoded delimiters, dot
  segments, repeated slash, empty query entries, and post-normalization duplicate keys.
- Change the agent public key and confirm the thumbprint check fails.
- Test exactly 300 seconds old, 301 seconds old, 30 seconds ahead, and 31 seconds
  ahead.
- Reuse the nonce concurrently on cold and cached paths. Exactly one request can
  consume it.
- Confirm a service failure leaves its nonce consumed.
- Confirm cached requests still run key, signature, timestamp, nonce, target, and
  scope checks.

## Rejected Options

- Signing the body hash without the method and target permits cross-route replay.
- Trusting a gateway-supplied target string without reconstruction permits mismatch
  between routing and signature checks.
- Skipping signature verification on cache hits turns a copied capability into a bearer
  token.
- Keeping one replay store in `verifierd` and another in the gateway creates two
  authorities with race and recovery disagreement.
- Releasing a nonce after a verifier timeout permits ambiguous replay.

## Approval Record

The maintainer approved all three parts on 2026-08-24:

1. The exact domain-separated signed bytes and fixed field lengths.
2. The method and origin-form target canonicalization rules.
3. The five-minute age limit, 30-second future-skew limit, and gateway-owned atomic
   nonce store.
