# B-0031 Retained Plonk Verifier Run

## Purpose

This run creates one synthetic public Plonk proof and verifies it through the
isolated `verifierd` worker with every `razield` role stopped. It records the
verification-only peak memory under the provisional 2 GiB cap.

The run retains only:

- the public zero-knowledge proof
- the public journal
- the public verification-key ID and SP1 key hash
- checksums and public proof-run provenance

It does not retain a witness, private snapshot, relationship path, Merkle opening,
salt, caveat value, private journal input, or debug trace.

## Machine

Use the independent NUS SoC VM with Docker. Do not use the NUS Slurm path for these
commands. The VM needs at least eight CPUs and 62 GiB RAM. The preflight reports
available disk space but does not reject the run against a fixed disk floor.

Keep the SSH session open during proving. A healthy proof command can remain silent
for up to 60 minutes. The measured eight-CPU run took about 15 minutes 30 seconds.

## Pull The Reviewed Plumbing

Run from the repository root:

```bash
git pull --ff-only origin main
git status --short
git rev-parse HEAD
```

`git status --short` must print nothing. Record the commit printed by the last
command.

## Preflight And Prepare

```bash
export RAZIEL_CLUSTER_NAME=independent
export RAZIEL_CLUSTER_RUNTIME=docker

./scripts/cluster/check-sp1-plonk-host
./scripts/cluster/check-sp1-plonk-plumbing
./scripts/cluster/prepare-sp1-plonk
```

Preparation can download pinned dependencies and the pinned Plonk setup archive. It
does not create a witness or proof. The later proof phase runs without a network.

## Create The Retained Public Fixture

The following flags record the maintainer approval for this one run:

```bash
export RAZIEL_SP1_CLUSTER_GATE_APPROVED=1
export RAZIEL_SP1_RETAIN_PUBLIC_ARTIFACT_APPROVED=1

./scripts/cluster/run-sp1-plonk
```

Do not interrupt or rerun a quiet command. A second SSH session can check the proof
container without changing it:

```bash
docker ps \
  --filter name=raziel-sp1-plonk-benchmark \
  --format '{{.ID}} {{.Status}}'
```

A passing run stages six public files under:

```text
services/verifierd/tests/fixtures/retained-plonk-v1/
```

The proof process first writes into an ignored owner-only directory. The host stage
accepts exactly four public payload files, checks them against the proof log, writes
`SHA256SUMS` and `PROVENANCE`, then removes the temporary export.

## Run Independent Verification

Stop issuer roles, then run the verifier acceptance script:

```bash
./scripts/compose \
  --profile services \
  --profile proof \
  stop \
  razield \
  razield-proof-worker \
  razield-proof-sandbox \
  verifierd \
  verifierd-worker-sandbox

./scripts/check-verifier-sandbox-compose
./scripts/run-retained-sp1-verifier
```

The runner refuses to continue if a `razield` role is running. It recreates only the
networkless verifier sandbox, runs the positive fixture and eight mutation cases,
reads the verifier cgroup peak, then stops the sandbox.

A passing run creates:

```text
.sp1/benchmarks/verifier-<timestamp>.pass.log
docs/benchmarks/sp1-verifier/verifier-<timestamp>.md
```

The raw log stays ignored. The Markdown result is public.

## Return The Evidence

Run these commands and send their output back for review:

```bash
git status --short
find services/verifierd/tests/fixtures/retained-plonk-v1 \
  -maxdepth 1 \
  -type f \
  -print \
  | sort
cat services/verifierd/tests/fixtures/retained-plonk-v1/PROVENANCE
cat docs/benchmarks/sp1-verifier/verifier-*.md
```

Do not print the binary proof or public-journal files. Do not commit until the six-file
allowlist, provenance, verifier result, and Git status have been reviewed.
