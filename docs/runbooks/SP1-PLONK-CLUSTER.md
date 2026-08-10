# SP1 Plonk Cluster Run

## Purpose

This run tests the smallest production-shaped Raziel authorization proof. It does
not use transaction data. It does not test the 100 million instruction ceiling.
The checked-in fixture contains synthetic relationships and caveat values.

Do not use real transaction data, production relationships, stored caveat values,
Merkle openings, or snapshot salts on a shared cluster. That use needs a separate
trust and data-governance decision.

The fixed cluster profile uses:

- one Linux node
- four CPU cores
- 64 GiB scheduler memory
- a 56 GiB proof-process memory cap
- a 24 GiB Go soft heap target
- Go garbage-collector memory diagnostics
- zero swap
- no proof-process network route
- a two-hour scheduler limit
- a 60-minute proof deadline

The 30 GiB and 36 GiB gates failed at their exact cgroup limits. The maintainer
approved one 56 GiB attempt on the dedicated independent Docker VM on 2026-08-11.
That approval permits no retry, cap increase, Slurm submission, or proof-profile
change. The proof runner fixes every listed value. Environment variables cannot
raise or lower the approved memory, disk, heap, CPU-affinity, diagnostic, or worker
limits.

## Runtime Choice

NUS uses Slurm and does not provide Docker on its nodes. The NUS path requires a
pinned Apptainer or Singularity SIF.

Set `RAZIEL_CLUSTER_RUNTIME=apptainer` or `singularity` for an HPC container node.
Set `RAZIEL_SP1_TOOLCHAIN_SIF` to a readable SIF image built from the pinned
`raziel-toolchain:m0` image.

Docker stays supported for a separate Linux machine. Set:

```bash
export RAZIEL_CLUSTER_NAME=independent
export RAZIEL_CLUSTER_RUNTIME=docker
```

The independent machine needs Git, a local Docker Engine, and Docker Compose. The
repository toolchain image supplies Node.js, pnpm, Rust, Go, Buf, Protobuf, libclang,
and SP1. Do not install those tools on the host for this run. The proof preflight also
requires at least four CPUs, 62 GiB RAM, and 60 GiB free disk.

The NUS preflight rejects Docker even if a client binary appears in `PATH`. It also
rejects an invocation without `SLURM_JOB_ID`. Preparation and proving must run inside
their checked-in Slurm jobs.

The preflight rejects ambient `APPTAINER_*`, `SINGULARITY_*`, `DOCKER_*`,
`COMPOSE_*`, and `BUILDX_*` controls. These variables can add mounts, select a remote
daemon, or replace the Compose file. The independent Docker path accepts only a
daemon reached through a local Unix socket.

The Apptainer path requires these user-facing runtime controls:

- `--containall`
- `--net --network none`
- `--no-eval`
- `--no-mount hostfs,home,cwd`
- `--memory`
- `--memory-swap`
- read-only bind mounts

The preflight rejects the run if the installed runtime lacks one of these controls.
The inner proof runner checks the effective cgroup and network state again.

## Commit And Transfer

Commit the reviewed M3 implementation and cluster plumbing before using NUS. This
implementation commit does not claim that M3 is complete. It supplies the immutable
source identity required by preparation and proof checks.

Push or transfer that commit, then create a clean NUS clone at the exact commit.
Confirm both commands before submitting any job:

```bash
git rev-parse HEAD
git status --short
```

The second command must print nothing. Do not copy individual scripts, compiled
binaries, setup files, or proof artifacts into a different source tree.

## Probe Slurm

Submit this small job before preparing the SIF:

```bash
sbatch scripts/cluster/slurm-probe-sp1-plonk
```

It reports the installed Apptainer or Singularity version and whether a compute job
can make an HTTPS HEAD request to the pinned SP1 setup URL. It requests one CPU,
1 GiB memory, and ten minutes. It does not download the setup archive.

The private probe log is written under:

```text
.sp1/slurm-logs/raziel-sp1-probe-<job-id>.log
```

After preparing the SIF, export its path and SHA-256 and submit the probe again:

```bash
export RAZIEL_SP1_TOOLCHAIN_SIF="$PWD/.sp1/images/raziel-toolchain-m0-amd64.sif"
export RAZIEL_SP1_TOOLCHAIN_SIF_SHA256="<the SIF SHA-256>"
sbatch scripts/cluster/slurm-probe-sp1-plonk
```

The second run tests an exact 256 MiB child cgroup, every cgroup evidence file used
by the proof runner, zero swap, read-only input mounts, an in-memory `/tmp`, and a
no-network namespace. It mounts only synthetic probe files. A failure means the NUS
runtime cannot enforce the current proof boundary.

## Prepare A SIF

Run these commands on an x86_64 Docker host, or on Docker Desktop with x86_64
emulation:

```bash
DOCKER_DEFAULT_PLATFORM=linux/amd64 docker compose \
  --env-file tools/toolchain-versions.env \
  build toolchain
docker image inspect --format '{{.Architecture}}' raziel-toolchain:m0
docker save --output raziel-toolchain-m0-amd64.tar raziel-toolchain:m0
sha256sum raziel-toolchain-m0-amd64.tar \
  >raziel-toolchain-m0-amd64.tar.sha256
```

Transfer the archive and its SHA-256 value to the cluster. Then build the SIF:

```bash
mkdir -p .sp1/images
mv raziel-toolchain-m0-amd64.tar* .sp1/images/
cd .sp1/images
sha256sum --check raziel-toolchain-m0-amd64.tar.sha256
export SOURCE_DATE_EPOCH="$(git -C ../.. show -s --format=%ct HEAD)"
apptainer build raziel-toolchain-m0-amd64.sif \
  docker-archive:raziel-toolchain-m0-amd64.tar
sha256sum raziel-toolchain-m0-amd64.sif
cd ../..
export RAZIEL_CLUSTER_RUNTIME=apptainer
export RAZIEL_SP1_TOOLCHAIN_SIF="$PWD/.sp1/images/raziel-toolchain-m0-amd64.sif"
export RAZIEL_SP1_TOOLCHAIN_SIF_SHA256="<the SIF SHA-256>"
```

`SOURCE_DATE_EPOCH` removes build-time variation from supported Apptainer SIF
metadata. Record that value, the exact Apptainer version, x86 Docker image ID, Docker
archive SHA-256, and resulting SIF SHA-256. The maintainer must review and approve
this provenance tuple before the proof submission. A caller-supplied SIF digest
alone detects later changes but does not prove derivation from the approved Docker
image.

Use `singularity` in place of `apptainer` on a Singularity cluster. Some older
Singularity releases spell the archive source as
`docker-archive://.sp1/images/raziel-toolchain-m0-amd64.tar`.

The image archive and SIF are local build artifacts. Do not add them to Git.

## Validate Plumbing

Run this command on a development host with Docker Compose:

```bash
./scripts/cluster/check-sp1-plonk-plumbing
```

It checks shell syntax, checks JavaScript syntax through the digest-pinned Node image,
renders the 56 GiB Compose profile, verifies the four-CPU affinity and Go memory
diagnostics, exercises the public result exporter, and proves that raw host and
scheduler identifiers stay out of the generated Markdown. Host Node.js is not
required. Docker pulls the pinned Node image on the first run if the image is not
already present.

## Prepare

Preparation uses network access. It verifies the setup archive, fills dependency
caches, builds the guest, and compiles the proof test. It does not construct a
witness or create a proof.

Run it through Slurm from a clean clone with at least 60 GiB free space:

```bash
sbatch scripts/cluster/slurm-prepare-sp1-plonk
```

The command writes an ignored preparation record under `.sp1/cluster/`. It stages an
owner-only run bundle containing an explicit file allowlist. The proof job mounts
this bundle instead of the repository. Ignored environment files, private datasets,
prior proofs, and unrelated workspace files cannot enter the witness-bearing
container.

The proof job checks the source commit, source tree, runtime, image digest,
proof-test binary, guest ELF, Plonk setup manifest, and complete run-bundle manifest
against the preparation record. Preparation and proving share one atomic operation
lock.

This stage needs HTTPS access for dependency and setup downloads. If NUS compute
nodes block outbound HTTPS, use the independent Docker machine for the complete
prepare and proof gate. Do not copy a prebuilt proof-test binary into NUS and create
a new preparation record. That would not prove that the cluster binary came from the
recorded clean source and toolchain.

## Submit

The scheduled phase runs offline. It does not build an image, download setup data,
fetch dependencies, or compile source. The independent-machine approval does not
authorize a Slurm submission. Approve the SIF provenance tuple and one Slurm attempt
before setting:

```bash
export RAZIEL_SP1_CLUSTER_GATE_APPROVED=1
```

```bash
sbatch scripts/cluster/slurm-sp1-plonk
```

Pass site account, queue, partition, and scratch options through `sbatch`. The
checked-in template does not guess NUS account names.

Preparation and proof logs use owner-only files under `.sp1/slurm-logs/`. Slurm
stdout points to `/dev/null`, so the scheduler does not create a permissive log in
the repository root.

## Independent Docker Machine

Use this path on the separate Linux machine:

```bash
export RAZIEL_CLUSTER_NAME=independent
export RAZIEL_CLUSTER_RUNTIME=docker
./scripts/cluster/prepare-sp1-plonk
export RAZIEL_SP1_CLUSTER_GATE_APPROVED=1
./scripts/cluster/run-sp1-plonk
```

The same 56 GiB hard cap, four-CPU affinity, Go memory diagnostics, zero-swap rule,
network isolation, source checks, setup checks, proof deadline, verification gate,
mutation gate, and public result exporter apply. The 2026-08-11 approval permits
this command once against the recorded preparation state. Do not rerun it after
success or failure without another decision.

The wrapper writes proof output to an owner-only in-progress log and prints it only
after the container exits. A healthy terminal can remain silent for about 68 minutes.
The stage limits are two minutes for execution, 60 minutes for proving, one minute
for verification, and five minutes for mutation checks. Small wrapper steps run
outside those limits. Keep the SSH session open. Do not interrupt or rerun a quiet
process.

From a second SSH session, this read-only command confirms whether the proof
container is still running:

```bash
docker ps \
  --filter name=raziel-sp1-plonk-benchmark \
  --format '{{.ID}} {{.Status}}'
```

An empty result means the container exited. Wait for the original command to print
and rename its in-progress log before inspecting the result.

## Result

A successful job creates two local outputs:

```text
.sp1/benchmarks/<job>.pass.log
docs/benchmarks/sp1-plonk/<job>.md
```

Git ignores the raw log. The Markdown file contains a fixed set of public fields.
It records proof time, verification time, artifact size, instruction count,
resource limits, CPU-affinity count, active Go memory diagnostics, peak memory, tool
versions, setup hash, source hashes, and the allowlisted run-bundle manifest hash. It
omits hostnames, usernames, scheduler job IDs, account names, and internal paths.

Review and commit only the Markdown result. Do not commit the SIF, setup archive,
proof artifact, public-journal file, or scheduler log.

The exporter rejects duplicate required fields, unknown execution-path combinations,
profile drift, malformed digests, unsafe Markdown values, mismatched proof and
verification keys, and any timing or resource value outside ADR-0012.
