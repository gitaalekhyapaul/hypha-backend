# M7 Approved Guest And Grant Controls

- Date: 2026-10-15.
- Baton: B-0067, bounded repair checkpoint.
- Status: implementation validated, production deployment and real-proof gate pending.
- Authorization: maintainer requested both fixes after the long failed proof.

## Findings

The retained job ran 966,922 ms and peaked at 48,544,919,552 bytes. Swap, OOM,
and restart counters were zero. Its discarded internal error was not recoverable.
A public-only SP1 probe established a definite guest/key mismatch:

- Installed guest SHA-256: `658fc08c9b55db6595138b3beb9ab6086a8426ab5a806e1ea36dd665d0e76cec`.
- Installed key ID: `sp1-vk-v1:925b5f42a341e6a5ba066164f86eef446674a4c6d1b2fa2921eb1d6db06e18d5`.
- Approved key ID: `sp1-vk-v1:c80994ad25358482755752c3068dc51c61f52d136e4eaf4cc9da8277239008a1`.

Preparation rebuilt the guest independently from the trusted key configuration.
The child compared the result's key only after proving. The exact compiler or
build-input difference responsible for the new guest identity was not isolated.

The approval form displayed every private grant field for every policy. The owner
already used only the values required by the selected supported policy shape.

## Changes

The existing approved public guest was recovered from the VM's earlier build
output and independently matched against both trusted key identifiers. Its bytes
and provenance are retained under `tools/authorization-guest/`.
SHA-256 is `817b8689719d2079756160e56fe779b96f8e8194b77f68b5cca1c82fcb664a41`.
This preserves existing trust. It does not approve a new program or claim source
build reproducibility. Future guest repinning needs explicit approval.

Runtime preparation checks the checksum, derives the key pair, and installs this
public artifact. The child checks both identifiers before private IPC. Existing
post-proof checks stay in place. `razield-prover --check-program-key` needs only
the public ELF path and expected pair, with no registry, snapshot, or IPC mount.

Rust derives a bounded `requiredGrantFields` list from the exact approved package.
Direct and dual-direct leaves need no fields. Team membership needs `teamId`.
Organization arrows need `organizationId`. Referenced caveats contribute only used
stored inputs. ANY/ALL collect the union. Grant materialization is unchanged.

The owner socket carries this typed metadata, and the browser validates it.
Unknown enums, duplicates, and lists over six entries reject. The form hides
irrelevant fields and resets edits on a changed policy. Exact uncertain retries
survive generation changes, same-policy refreshes, and temporary catalogue loss.

Independent review found that an old daemon's absent repeated field looked like
an empty requirement list. The internal catalogue payload is now version 2.
New clients reject version 1. A framed old-response regression covers this case.
The JSON catalogue remains version 1. Existing operator authentication stays fixed.

## Validation

| Check | Result |
| --- | --- |
| Isolated x86_64 one-shot tests | PASS, 16 passed, one controlled-child helper ignored by design |
| Isolated owner-service library suite | PASS, 125 passed, four existing live external-service tests ignored |
| Actual public-only CLI and runtime preparation | PASS, approved pair accepted, artifact installed in temporary container storage |
| Actual installed mismatched guest | PASS, rejected by new CLI with no private mount, IPC, or proving |
| Corrupted public artifact | PASS, checksum rejection before runtime installation |
| Focused console tests | PASS, 220 tests in four files |
| Console typecheck and scoped formatting | PASS, pinned networkless toolchain |
| Desktop Playwright | PASS, five policy shapes and stale-value reset at 1440x1000, synthetic API fixtures only |
| Preparation and Compose | PASS, public artifact checksum, helper checks, base proof, M6 preview/live, integrated ngrok |
| Runbooks and whitespace | PASS, 28 shell blocks parse, diff whitespace clean |
| Repaired-stack real proof | SKIP, maintainer pull/rebuild and new agent request required |

The Mac's offline cache lacked locked `anstream` 1.0.0. No dependency was added
or downloaded. The VM's existing complete public caches supported isolated builds.
The first archive extraction needed portable ownership handling. An unrelated
owner-mutation test needed CHOWN in the disposable test container. Neither change
alters production sandbox capabilities. All disposable test containers exited.

## Boundaries And Handoff

The VM checkout and production stack were not edited or restarted. Isolated source
copies lived only in container tmpfs. Only public dependency/build caches were
mounted. Runtime preparation targeted temporary storage, not the production volume.
No secret, private witness, grant values, or new proof entered this evidence.
SDK and agent repositories remain unchanged and clean.

Follow the repair section in `docs/runbooks/M6-LIVE-VM.md`. Finish or terminally
cancel active proofs first. Pull, stop the backend roles, rebuild from existing
caches, recreate the roles, and run the public-only key check. Preserve all volumes
and identities. Start a fresh agent access request after the check passes.
Two real agent capabilities, protected-query reuse, and full reconciliation remain
the pending M7 acceptance gate.
