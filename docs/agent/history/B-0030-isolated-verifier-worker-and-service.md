# Engineering Baton Archive

- Baton ID: B-0030
- Final state: completed and archived
- Milestone: M4, isolated verifier worker and service activation
- Owner: Codex
- Started UTC: 2026-08-24T12:48:46Z
- Completed UTC: 2026-08-25T07:22:35Z
- Branch: `main`
- Base commit: `e27e3b0`

## Objective

Connect strict verifier admission to a resource-isolated local proof-verification
worker and expose the bounded `VerificationService` without any `razield` dependency.

## Acceptance Criteria

- [x] Live verifier ingress cannot bypass the frozen-descriptor 2 MiB strict decoder.
- [x] `verifierd` loads its fixed local registry and paired program-key configuration
      with no `razield` endpoint.
- [x] A versioned, bounded owner-only protocol sends only proof bytes, expected public
      values, and trusted program-key identity to the verifier worker.
- [x] The worker runs direct SP1 Plonk verification under the provisional 2 GiB,
      zero-swap, no-network, read-only-root, bounded-tmpfs, 60-second profile.
- [x] Worker rejection, crash, timeout, malformed output, and resource-profile failure
      stay typed without crashing `verifierd` or releasing candidate claims.
- [x] Only a matching worker success promotes admitted candidates to verified claims.
- [x] Focused protocol, process, live-ingress, feature-tree, privacy, formatting,
      lint, Compose, container, and default workspace gates pass without a new proof.

## Architecture Constraints

- `verifierd` and its worker never call `razield` or read private graph data.
- Request data cannot select a socket, executable, proof mode, key, registry path, or
  resource profile.
- The worker receives no PolicyIR, relationship, caveat, salt, opening, path, witness,
  private envelope, or agent signing material.
- ADR-0011 public values and ADR-0002 request-signature semantics did not change.
- Direct `sp1-verifier 6.3.1` remains the engine. Proving and network packages stay
  excluded from the verifier build.
- No real SP1 proof ran or entered Git.

## Decisions

- Used a crate-owned Tonic service wrapper with a custom strict decoder. Generated
  clients and message contracts remain unchanged.
- Mirrored the fixed-launcher model with one persistent sandbox launcher and one
  one-shot verifier child per request.
- Kept the API-side registry monotonic across reloads and loaded `current.pb` before
  each admission.
- Mapped proof rejection to `CAPABILITY_INVALID`. Process and trust failures map to
  `SERVICE_UNAVAILABLE`. Neither response includes candidate claims.

## Work Completed

- Added strict bounded worker request and result codecs with redacted debug output.
- Added owner and mode checks for the `0700` socket directory and `0600` socket.
- Added fixed child execution, cancellation, a 60-second deadline, and confirmed child
  termination after crash, malformed output, and timeout.
- Added a five-second initial-frame deadline so an incomplete owner connection cannot
  block the single verifier worker indefinitely.
- Added direct `Sp1Verifier` execution after exact 2 GiB and zero-swap cgroup checks.
- Activated `verifierd` as a Tonic service with strict raw ingress, local registry
  reloads, trusted key pairing, and candidate-claim promotion after proof success.
- Added a verification-only release runtime and Compose worker sandbox.
- Updated README, architecture, specification, and baton state.

## Validation Evidence

| Command or check | Result | Scope |
| --- | --- | --- |
| `cargo test -p verifierd --all-targets` | PASS, 23 tests with one controlled-child entry ignored | Admission, service, worker, launcher, and cgroup logic |
| Live malformed gRPC test | PASS | Strict decoder rejects raw unknown fields before handler execution |
| `cargo clippy -p verifierd --all-targets -- -D warnings` | PASS | Verifier code and tests |
| `./scripts/check-sp1-features` | PASS | Direct verifier present and proving or network packages absent |
| `./scripts/check-verifier-sandbox-compose` | PASS | Exact rendered sandbox and API controls |
| Verifier release container build | PASS, 28.81 seconds | Verification-only worker and child binaries |
| Live sandbox inspection | PASS | Healthy, 2 GiB memory, zero swap, no network, read-only root, PID limit 64 |
| Runtime SHA-256 manifest | PASS | Both installed verifier binaries match |
| `./scripts/dev cargo test --workspace` | PASS | Default workspace tests and docs |
| `./scripts/dev pnpm format:check` | PASS | Repository formatting |
| `./scripts/dev pnpm lint` | PASS | Biome, Clippy, Go vet, and Buf |
| `./scripts/dev pnpm typecheck` | PASS | TypeScript, Rust, and Go checks |
| `./scripts/dev pnpm test` | PASS | Default TypeScript, Rust, Go, and docs tests |
| Host `cargo test --workspace` | NOT AVAILABLE | Host lacks Go required by the SP1 test-only Gnark build script |
| `cargo fmt --all -- --check` | PASS | Rust formatting |
| `sh -n` on new scripts | PASS | Shell syntax |
| `git diff --check` | PASS | Patch hygiene |
| Real SP1 proof | NOT RUN | Separate approval remains required |

The default workspace suite ignored explicit SP1 proof, Redis, SpiceDB, and live
service tests as designed. The existing future-compatibility warning for
`proc-macro-error2 2.0.1` remains unchanged.

## Privacy Checks

- PASS: no agent response contains PolicyIR, relationship tuples, paths, or Merkle
  openings.
- PASS: no standard log contains caveat values, salts, witness data, or proof bytes.
- PASS: the worker protocol carries only approved public proof material.
- PASS: rejected and unavailable responses contain no candidate claims or proof
  metadata.
- PASS: `verifierd` reads trusted policy and snapshot state from its own registry.
- PASS: the verifier graph contains no prover or `razield` client.
- PASS: real transaction data remains outside Git and generated output.

## Remaining Risks And Follow-Up

- The 2 GiB verifier cap remains provisional until a retained real proof measures
  verification-only peak memory.
- A positive isolated-verifier run needs one approved synthetic Plonk rerun and
  retention of its public proof artifact.
- The gateway nonce store, TypeScript request-signature vector, cache, and verifier
  client remain M5 work.

## Exact Next Action

Request explicit maintainer approval for B-0031 to rerun one synthetic Plonk proof on
the NUS VM, retain only its public proof artifact, and verify it through the isolated
worker with `razield` stopped.
