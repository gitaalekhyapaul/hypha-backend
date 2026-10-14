# Approved Guest Checkout Mode

- Baton: B-0067, bounded preparation follow-up.
- Status: repair validated, maintainer VM pull/retry pending.
- Source baseline: `89123774e055708bfc8bf39e9154e85c92e4d09c`.

## Cause

The maintainer's owner-tools build passed. The proof build accepted the approved
ELF checksum and compiled both host binaries, then the public-only key check
returned `Configuration`. No proof ran.

Read-only VM inspection found mode `0664`, owner `1000:1000`, link count 1, and
386088 bytes on the checkout ELF. Its SHA-256 matched the approved artifact.
Public key files were readable, and the build's working directory was correct.
The key check reused the runtime reader, which rejects group-writable files.
Git does not preserve a file's exact permission mode across checkouts.
The prior isolated copy had mode `0644`, so that gate missed this deployment case.

## Repair

Preparation now makes a private temporary directory and copies the public ELF
with mode `0444`. It checks that staged copy's checksum and approved key pair,
then installs the same bytes. Cleanup removes staging on success and failure.
The checkout stays unchanged. Source symlinks reject. Runtime permission checks,
trusted keys, guest semantics, and private data handling remain unchanged.

## Validation

| Check | Result |
| --- | --- |
| Portable preparation regression | PASS, 0644/0664/0666 source modes accepted without source changes, installed mode 0444 |
| Corrupt and symlink source | PASS, rejection before runtime creation |
| Staging cleanup | PASS, empty after success and failure |
| Real isolated VM preparation | PASS, mode 0664 source, real SP1 approved pair accepted, installed mode 0444 |
| Real checksum negative | PASS, changed artifact rejects before installation |
| Formatting, shell syntax, whitespace | PASS |
| Production stack and new proof | SKIP, maintainer pull/retry required |

The isolated VM container mounted public caches and a temporary public source copy.
It had no private runtime volumes or network access. Its runtime target was tmpfs.
No VM checkout, existing volume, production service, or credential changed.

## Handoff

Pull the follow-up. Stop the partially started backend roles, run only the proof
build, then recreate the backend and run the public-only key check. Reuse the
already rebuilt owner tools. Chained commands in the runbook stop at a build error.
Preserve all volumes and identities. No full proof benchmark or dataset reset is
needed to validate this preparation fix.
