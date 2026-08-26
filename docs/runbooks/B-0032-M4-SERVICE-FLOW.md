# B-0032 M4 Complete Service Flow

## Purpose

This run sends one signed synthetic authorization request through the production M4
backend path:

```text
agent client -> razield API -> BullMQ -> proof worker -> isolated Plonk prover
             -> terminal capability -> independent verifier
```

The request first returns `QUEUED` with the proof worker stopped. The script then
starts the worker, waits for one real local Plonk proof, and records the terminal
capability. It stops the issuer and prover roles before verification. The independent
verifier must accept the original capability and reject an altered capability and a
different agent key.

This is the final synthetic M4 backend acceptance run. It does not satisfy the M5
product walkthrough or the M6 real transaction acceptance gate.

## Machine

Run this on the independent NUS VM used for B-0031. The machine needs Docker, at
least eight CPUs, at least 62 GiB RAM, and the prepared SP1 Plonk setup. Host Node,
Rust, Go, Buf, and OpenSSL installations are not required. The pinned toolchain image
provides them.

The approved proof sandbox receives eight CPUs, 56 GiB RAM, no swap, and no network.
The verifier receives 2 GiB RAM, no swap, no network, and a read-only root.

The runner uses a private, host-user-owned Cargo target for its acceptance client.
It does not depend on the ownership of the repository's shared `target` directory.
The private target is reused during one run and deleted during cleanup. Before the
first client build, the runner restores read and directory-traverse permissions on
the shared cache of public Cargo dependencies. It does not grant cache write access
or change file ownership.

## Prepare

Run from the repository root:

```bash
git pull --ff-only origin main
git status --short

export RAZIEL_CLUSTER_NAME=independent
export RAZIEL_CLUSTER_RUNTIME=docker

./scripts/cluster/check-sp1-plonk-host
./scripts/cluster/check-sp1-plonk-plumbing
./scripts/cluster/prepare-sp1-plonk
```

`git status --short` must print nothing. Preparation downloads or builds only pinned
tooling and public proving setup files. It does not create a witness or proof.

## Run One Proof

Set the explicit one-run approval flag, then start the complete flow:

```bash
export RAZIEL_M4_SERVICE_FLOW_APPROVED=1
./scripts/run-m4-service-flow
```

The proof phase can remain quiet. The client prints a heartbeat every 30 seconds.
Do not interrupt or rerun a quiet proof. The prior eight-CPU proof took about 15
minutes 30 seconds.

A passing run prints two paths:

```text
m4_service_benchmark_report=.sp1/benchmarks/m4-service-<timestamp>.pass.log
public_m4_service_result=docs/benchmarks/m4-service-flow/m4-service-<timestamp>.md
```

The raw log stays ignored. The Markdown report is public and should be reviewed,
committed, and pushed from the VM.

## Privacy And Retention

The runner scans service logs and the run's Redis keys for a private marker. It checks
that the private queue envelope was deleted before verification. Temporary authority
keys, access tokens, job identifiers, and capability bytes are deleted after the run.

The approved private snapshot remains in the owner-only Docker volume. It is needed
as durable owner-side state and is never copied into the public report. The report
contains only public capability fields, proof timing, resource measurements, and
pass or reject outcomes.

## Failure

A failed run moves its raw log to:

```text
.sp1/benchmarks/m4-service-<timestamp>.fail.log
```

The runner stops the stack first. It then removes the failed run's exact private
envelope and snapshot when their validated identifiers were recorded.

Do not start another proof. Return the last 80 lines for review:

```bash
tail -n 80 "$(ls -t .sp1/benchmarks/m4-service-*.fail.log | head -n 1)"
```
