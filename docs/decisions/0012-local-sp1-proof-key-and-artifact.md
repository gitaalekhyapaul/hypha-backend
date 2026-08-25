# ADR-0012: Local SP1 Proof, Key, and Artifact Contract

- Status: accepted, amended, and validated
- Date: 2026-07-24
- Milestone: M3
- Approved by maintainer: 2026-07-27
- Plonk amendment approved by maintainer: 2026-07-27
- One 36 GiB proof attempt approved by maintainer: 2026-08-10
- 36 GiB validation failed: 2026-08-10
- One 56 GiB proof attempt approved by maintainer: 2026-08-11
- 56 GiB validation passed: 2026-08-11
- Eight-CPU validation passed: 2026-08-13
- Eight-CPU profile approved by maintainer: 2026-08-24
- Fixed disk floor removed by maintainer: 2026-08-26
- 30 GiB diagnostic approved by maintainer: 2026-07-27
- Resolution: O-003 is closed. D-019 selects local Plonk for final demo proof
  artifacts.

## Context

ADR-0011 fixes the private guest input and exact public journal. B-0010 proves that
the host and guest produce the same journal in execute mode. M3 still needs a real
proof, a stable verification-key identifier, and strict bytes for
`ProofCapability.zk_proof`.

SP1 `6.3.1` exposes four proof modes:

- Core has the lowest proving overhead, but proof size grows with shard count.
- Compressed has constant proof size, but recursive aggregation costs more time and
  memory.
- Plonk and Groth16 add a zero-knowledge outer proof. Their on-chain verification
  support is not needed by this PoC.

The local Docker VM exposes 11 CPUs and 18,832,257,024 bytes of memory. SP1's default
executor memory limit is 24 GiB. Its default proving trace ring uses about 1.43 GiB.
Those defaults do not match this workstation. The first proof run needs an isolated
resource cap so a failed experiment cannot freeze the development session.

SP1's native proof save format uses bincode `1.3.3` with fixed-width integer encoding.
Its default decoder permits trailing bytes. Raziel needs one bounded encoding and must
reject trailing data.

SP1 can write the full private stdin to `stdin.bin` when `SP1_DUMP` is active. This
debug path is not safe for Raziel witness data.

### Post-approval security review hold

A source review after approval found no witness-masking or trace-blinding path in
the pinned SP1 `6.3.1` Core prover. The Core prover commits generated main traces
directly. Its proof serializes main-trace multilinear openings at verifier challenge
points.

Those openings are not the raw witness, but they are witness-derived linear
information. The pinned Hypercube audit focuses on soundness and implementation
security. It does not establish a zero-knowledge simulator or witness-hiding claim
for Core proof artifacts.

This finding suspends Core artifacts for `ProofCapability.zk_proof`. The benchmark
can measure Core performance, but it cannot close O-003 or permit proof exposure to
an agent or independent verifier. Removing this hold needs one of:

- a pinned upstream zero-knowledge assurance that covers SP1 `6.3.1` Core artifacts
- a privacy-preserving outer proof such as local Groth16 or Plonk, with a revised
  resource decision
- a new proof construction with reviewed witness-masking semantics

### Approved Plonk amendment

The maintainer approved local Plonk on 2026-07-27 after reviewing the Core privacy
finding, the pinned Gnark path, the native dependency graph, and the setup artifact
trust boundary.

The pinned CPU BN254 Plonk prover creates fresh blinding polynomials for its witness
polynomials before commitment and opening. Raziel claims computational zero knowledge
for this mode. SP1 does not request Gnark's optional statistical-zero-knowledge mode,
so Raziel does not claim statistical zero knowledge.

The Plonk proof exposes five SP1 outer public inputs: program-key hash,
committed-values digest, exit code, recursion-key root, and proof nonce. The
`SP1ProofWithPublicValues` bundle also carries the exact public ADR-0011 journal.
The recursive proof and authorization witness remain private prover inputs.

## Decision

### Capped local Plonk proof

The fixed direct-relationship fixture has this structural size:

```text
fixture = direct relationship ALLOW
policy object definitions = 4
policy relation definitions = 8
policy permission definitions = 7
policy caveat definitions = 1
snapshot relationship count = 8
proof witness relationship count = 1
selected authorization path depth = 1
```

Use SP1 Plonk for the M3 proof artifact only after one run passes all of these gates:

```text
measured instructions = 422138
proof wall-clock deadline = 60 minutes
verification wall-clock deadline = 60 seconds
maximum encoded artifact = 1048576 bytes
maximum container memory = 7516192768 bytes
parallel proof jobs = 1
```

The benchmark runs in one disposable container. The container receives a 7 GiB
memory cap. The host test binary uses Cargo's release profile and is built before the
measurement container starts. The command records wall-clock time, peak container
memory, proof bytes, verification time, Docker resources, CPU architecture, tool
versions, the Cargo lock hash, and the guest ELF hash.

An out-of-memory kill, deadline, proof-size breach, verification failure, or journal
mismatch fails the Plonk gate. Do not retry a larger fixture. Do not switch to
Compressed, Groth16, CUDA, or network proving without a revised decision and
maintainer approval.

### 7 GiB validation result

The approved gate failed three times on 2026-07-27. Every run ended with exit 137
at an exact cgroup peak of 7,516,192,768 bytes. No proof artifact was produced.

The second run used one trace slot, one gas slot, a `2^20` shard preallocation,
four Rust workers, four Go workers, and a 5 GiB Go soft heap target. The third run
lowered the Linux native executor child-process limit to 2 GiB. Both runs reached
the same outer cgroup boundary.

The pinned proving key declares a `2^25` Plonk domain. The pinned Gnark setup data
expands to a source-derived live-memory floor near 15.75 GiB before the 633.8 MiB
circuit, witness, FFT, MSM, runtime, and proof buffers. `SP1CoreOpts` cannot bound
the native Gnark allocations.

The maintainer allocated 33,600,331,776 bytes to the Docker VM and approved one
bounded diagnostic on 2026-07-27. The diagnostic uses a 30 GiB container cap and a
24 GiB Go soft heap target. This leaves about 1.3 GiB of VM memory outside the
container. A passing run amends the accepted maximum to its measured peak. A failed
run does not approve another increase.

A 36 GiB cap on a project-controlled machine with at least 48 GiB RAM was the next
gate after the 30 GiB memory failure. Its result appears below.

### 30 GiB validation result

The approved diagnostic failed on 2026-07-27. The no-swap run ended with exit 137
at an exact cgroup peak of 32,212,254,720 bytes. The cgroup recorded one OOM kill
and zero swap use. The direct execute check passed at 422,138 instructions in 306
milliseconds. No proof artifact was produced.

An earlier 30 GiB attempt exposed Docker's default extra 30 GiB swap allowance. That
attempt was stopped before peak memory. Compose now sets equal memory and
memory-plus-swap limits, and the runner requires zero cgroup swap.

### Approved 36 GiB independent-machine attempt

The maintainer approved one proof-creation attempt on 2026-08-10 with this fixed
profile:

```text
fixture = direct relationship ALLOW
measured instructions = 422138
proof wall-clock deadline = 60 minutes
verification wall-clock deadline = 60 seconds
maximum encoded artifact = 1048576 bytes
maximum container memory = 38654705664 bytes
minimum host memory = 51539607552 bytes
Go soft heap target = 24 GiB
Rust and Go worker count = 4
container swap = 0 bytes
container network = none
parallel proof jobs = 1
runtime = independent local Docker daemon
```

The run uses only the checked-in synthetic M3 fixture. It uses the prepared
allowlisted run bundle and read-only Plonk setup. The approval permits no automatic
retry, cap increase, hosted prover, SP1 network prover, CUDA path, proof-mode change,
or larger fixture.

This amendment changes the resource gate only. It does not change proof semantics,
public values, artifact encoding, verification-key identity, or privacy boundaries.
A passing run records the measured peak and closes the M3 proof path. A failed run
does not approve another resource increase.

### 36 GiB validation result

The approved independent-machine attempt failed on 2026-08-10. The direct execute
gate passed at 422,138 instructions in 434 milliseconds. The native constraint-system
solver then reported 27,576,375 constraints before the kernel killed the proof
process. No proof artifact was produced, so verification and mutation gates did not
run.

The container reached its exact 38,654,705,664-byte memory limit. Its cgroup recorded
37 maximum-memory events, one OOM event, and one OOM kill. Container swap use and its
swap limit were both zero. The run used four Rust workers, four Go workers, and the
approved 24 GiB Go soft heap target on a host with 67,419,770,880 bytes of RAM.

The source manifest digest was
`3bb80bf3864581090bc784bca39b360b9da8097ea3a8b106b210ed8f1e9132d3`.
The failure report remains outside Git at
`.sp1/benchmarks/cluster-20260810T085319Z-70121.fail.log`. This result establishes
that the current direct-fixture Plonk path needs more than 36 GiB under the fixed
profile. It does not authorize a retry, a larger cap, swap, or a profile change.

### Approved 56 GiB dedicated-machine attempt

The maintainer approved one further proof-creation attempt on 2026-08-11. The Linux
VM is dedicated to this computation. The fixed profile is:

```text
fixture = direct relationship ALLOW
measured instructions = 422138
proof wall-clock deadline = 60 minutes
verification wall-clock deadline = 60 seconds
maximum encoded artifact = 1048576 bytes
maximum container memory = 60129542144 bytes
minimum host memory = 66571993088 bytes
container CPU affinity = CPUs 0-3
visible CPU count = 4
Go soft heap target = 24 GiB
Go maximum processes = 4
Rust worker count = 4
Go memory diagnostics = gctrace=1
container swap = 0 bytes
container network = none
parallel proof jobs = 1
runtime = independent local Docker daemon
```

The recorded VM has 67,419,770,880 bytes of RAM. The 56 GiB cap leaves
7,290,228,736 bytes outside the proof cgroup for Linux, Docker, SSH, and filesystem
activity. Swap remains disabled so a passing measurement reflects physical memory
rather than disk-backed paging.

CPU affinity is part of the memory boundary. `GOMAXPROCS=4` limits simultaneous Go
execution, but it does not change `runtime.NumCPU()`. The pinned KZG path uses
`runtime.NumCPU() * 2` as its default multi-exponentiation task count. Restricting
the process to CPUs `0-3` makes the Go scheduler, Gnark task sizing, and Rust worker
limit agree on four visible CPUs.

`GODEBUG=gctrace=1` records Go garbage-collector timings and aggregate heap sizes in
the ignored owner-only raw log. It does not print witness values, proof bytes, public
journal bytes, PolicyIR, relationships, salts, caveat values, or Merkle openings.
The public benchmark exporter records only that the diagnostic was active.

The approval permits one attempt from a newly prepared immutable run bundle. It
permits no automatic retry, larger cap, swap, hosted prover, SP1 network prover,
CUDA path, proof-mode change, or larger fixture. A failure requires another decision.

### 56 GiB validation result

The approved independent-machine attempt passed on 2026-08-11. The synthetic direct
fixture executed 422,138 instructions in 424 milliseconds. Local Plonk proof
generation took 1,742,106 milliseconds and produced a 2,537-byte encoded artifact.
The independent `LightProver` path verified the exact proof, trusted program key, and
public journal in 479 milliseconds. Proof, key, and public-journal mutation checks
failed closed.

The container peaked at 46,301,016,064 bytes under the 60,129,542,144-byte hard
limit. It exposed four affinity-bound CPUs, used no swap, and recorded no OOM event
or OOM kill. The proof process had no network route. Temporary proof and journal
files were deleted inside the isolated process after all gates completed.

The commit-safe public record is
[`cluster-20260810T174732Z-76324.md`](../benchmarks/sp1-plonk/cluster-20260810T174732Z-76324.md).
The raw owner-only log remains ignored. Its exporter used the documented narrow
recovery for duplicate profile output and an exact Rust test-harness prefix. The
recovery changed no proof, public value, metric, or resource result.

### Eight-CPU profile amendment

A later dedicated-VM run kept the direct fixture, source manifest, verification key,
56 GiB cap, zero swap, Go heap target, proof deadline, and proof code unchanged. It
restricted the process to CPUs `0-7` and set `GOMAXPROCS=8` and
`RAYON_NUM_THREADS=8`.

The run passed on 2026-08-13. Proof generation took 929,660 milliseconds,
verification took 438 milliseconds, and the encoded artifact remained 2,537 bytes.
The container peaked at 45,592,842,240 bytes with no swap, OOM event, or OOM kill.
Mutation checks passed. Against the four-CPU validation, proving time fell by 46.6
percent and peak memory fell by 1.5 percent.

The maintainer approved this eight-CPU profile on 2026-08-24. The current fixed
profile is `cluster-56g-8cpu`. The amendment changes CPU affinity and Go and Rust
worker counts only. It does not change the proof statement, guest, source fixture,
keys, public journal, artifact format, memory cap, swap policy, network policy,
deadline, or 75-minute capability window.

The public record is
[`cluster-20260813T063649Z-8cpu.md`](../benchmarks/sp1-plonk/cluster-20260813T063649Z-8cpu.md).
Its source manifest matches the four-CPU validation. The manual transcript did not
emit a source commit, source tree, run-bundle manifest digest, or toolchain image ID,
so the record leaves those fields unknown.

### B-0031 signable-fixture measurement

B-0031 binds the direct synthetic fixture to the public RFC 8032 Ed25519 test key so
the independent verifier can exercise the protected-request signature gate. M4 also
added shared capability-scope validation helpers to the authorization guest after the
M3 benchmark. These approved changes produce a new guest ELF and fixture commitment.

The execute-only pre-proof gate ran on 2026-08-25 from source commit
`e159538721b7040326f5cf84fb04d6d12a052b6e`. It measured 424,312 instructions in
430 milliseconds. The guest ELF SHA-256 was
`817b8689719d2079756160e56fe779b96f8e8194b77f68b5cca1c82fcb664a41`, and the source
manifest SHA-256 was
`e8423186bfb1947fe163e32baad1298ed532d5a5524342f41ff332e6c8223faf`. The old
422,138-instruction assertion rejected the run before Plonk proof generation. The
temporary input was deleted, and no proof artifact or retained witness was written.
Peak memory was 48,537,600 bytes, with zero swap and no OOM event.

The current B-0031 direct-fixture pin is 424,312 instructions. Historical M3 records
remain unchanged at 422,138 instructions. The new measurement stays below the
approved 520,000-instruction direct-path bound and does not change a proof ceiling,
resource profile, proof statement, or public encoding.

### Fixed disk-floor removal

The maintainer approved removal of the fixed 60 GiB free-disk preflight on
2026-08-26. The original cluster script introduced that value as a conservative
host-capacity guard. No benchmark derived it from proof storage.

The validated Plonk artifact is 2,537 bytes. The witness-bearing container uses a
read-only root, read-only source and setup mounts, and a memory-backed `/tmp`. Its
large transient proving state falls under the 56 GiB memory cgroup, not the worktree
filesystem. Preparation still stores the toolchain image, setup files, dependency
caches, and build output. Their size does not justify rejecting every prepared proof
run below 60 GiB free space.

The host preflight continues to report exact available worktree bytes for operational
diagnostics. It no longer accepts a minimum-disk override or rejects a run against a
fixed disk threshold. Normal filesystem and container-runtime failures still fail the
operation.

Plonk is selected over Groth16 for three reasons:

- the baseline authorization paths execute in fewer than 520,000 instructions
- its release setup archive is 2,305,113,029 bytes rather than 6,211,807,514 bytes
- Groth16's smaller proof and lower on-chain gas do not benefit the local PoC

Core remains available for internal performance measurements with non-sensitive
fixtures. A Core artifact never enters `ProofCapability.zk_proof`. The benchmark
does not claim that the 94,608,186-instruction boundary fixture fits the Plonk
resource limits. A proof job that exceeds a service limit fails closed and emits no
capability.

### Local prover and verifier

Proof generation constructs this concrete client:

```text
ProverClient::builder().cpu().core_opts(profile).build()
```

The code cannot construct `EnvProver`, `NetworkProver`, `MockProver`, or an
experimental prover. The SP1 `network`, `cuda`, and `experimental` Cargo features
remain disabled.

The local profile starts with:

```text
minimal_trace_chunk_threshold = 16777216
gas_trace_chunk_threshold = 134217728
memory_limit = 2147483648
trace_chunk_slots = 1
gas_trace_chunk_slots = 1
shard_size = 1048576
element_threshold = 402653184
height_threshold = 4194304
```

These values overwrite every `SP1CoreOpts` field that its default constructor can
read from the environment. Other fields retain the pinned `6.3.1` defaults. A
profile change needs new benchmark evidence. The outer container cap remains
authoritative for total memory.

On Linux, the 2 GiB `memory_limit` caps the native executor child process RSS. The
outer Gnark process is outside this setting. The 7, 30, and 36 GiB failures confirm
that the outer cgroup limit remains the operative total-memory boundary.

The approved container sets `RAYON_NUM_THREADS=8`, `GOMAXPROCS=8`,
`GOMEMLIMIT=24GiB`, and `GODEBUG=gctrace=1`. Docker restricts the process to CPUs
`0-7`. The Compose memory and memory-plus-swap limits both equal 56 GiB. The runner
requires eight affinity-visible CPUs, the exact Go controls, and
`memory.swap.max=0` before witness construction. The 56 GiB cgroup limit is the hard
failure boundary for the approved profile.

The proof request uses:

```text
mode = Plonk
cycle_limit = 100000000
expected_exit_code = SUCCESS
deferred_proof_verification = true
proof_nonce = first 16 issuance-nullifier bytes as four big-endian u32 values,
              reduced modulo the KoalaBear field modulus 0x7f000001
```

The proof nonce differentiates most capability issuances. Truncation and field
reduction permit collisions, so it is not a uniqueness guarantee. It adds no private
value and does not replace the complete public issuance nullifier.

Verification constructs `LightProver`. `LightProver` performs full SP1
cryptographic verification but cannot create proofs. This keeps the verifier path
separate from the memory-heavy proving node.

The verifier is constructed from the trusted authorization guest ELF. It owns the
derived key and key identifier for its lifetime. The verification call cannot accept
an ELF or verification key from a request. This prevents a caller from selecting a
different program and presenting the matching identifier.

### Verification-key identifier

Use SP1's program verification-key digest, not bincode bytes or a display string.

```text
domain = ASCII("raziel/sp1-verification-key-id/v1")

payload =
  string(SP1_CIRCUIT_VERSION) ||
  bytes(vkey.hash_bytes())

verification_key_id =
  "sp1-vk-v1:" ||
  lowercase_hex(SHA-256(framed(domain, payload)))
```

`SP1_CIRCUIT_VERSION` is `v6.1.0` in SDK `6.3.1`. `hash_bytes()` is the 32-byte
big-endian form of SP1's eight-word program-key digest.

The SDK version stays in the proof-artifact compatibility frame. It does not enter
the verification-key identifier. An SDK update that retains the same circuit and
program key does not rotate the key identifier.

The verifier derives the identifier from its local guest ELF and key setup. It
rejects a caller-provided identifier that differs by one byte or character. The
identifier does not contain the proof mode. One program key can verify any SP1 mode,
and the artifact fixes the permitted mode separately.

### Proof artifact bytes

`ProofCapability.zk_proof` carries one `RazielSp1ProofArtifactV1`:

```text
8 bytes  magic = ASCII("RZSP1PF1")
u8       proof_mode = 2 for Plonk
string   sdk_version = "6.3.1"
string   circuit_version = "v6.1.0"
bytes    SP1ProofWithPublicValues payload
```

`string` and `bytes` use a four-byte unsigned big-endian byte-length prefix. The
artifact has one canonical encoding and a 1 MiB total ceiling. Proof mode `1` remains
reserved for the suspended Core experiment and is rejected by the capability codec.

The SP1 payload uses bincode `1.3.3`:

```text
DefaultOptions::new()
  .with_fixint_encoding()
  .with_little_endian()
```

The decoder applies the same options with:

```text
.with_limit(payload_length)
.reject_trailing_bytes()
```

The outer decoder rejects unknown magic, modes, versions, lengths, truncation, and
trailing bytes. The inner decoder rejects trailing bytes. It re-encodes the decoded
value and requires an exact payload-byte match.

The 1 MiB ceiling bounds bytes consumed from the wire. Bincode's byte limit does
not bound heap allocation during decode. The independent verifier service must run
artifact decoding and SP1 verification in a child process with a hard memory limit
and deadline before it accepts remote proof bytes.

The decoded value must meet every rule:

- `proof` is the Plonk variant
- `sp1_version` equals `SP1_CIRCUIT_VERSION`
- `tee_proof` is absent
- `plonk_vkey_hash` equals the pinned wrapper verification-key SHA-256
- `raw_proof` is absent from the canonical Raziel payload
- Plonk public inputs and encoded proof use one canonical text encoding
- `public_values` equals the complete ADR-0011 journal byte for byte
- the exact local verification key validates the proof with `SUCCESS`

The artifact does not carry the guest ELF, proving key, PolicyIR, relationships,
openings, caveat values, salts, witness traces, or another public-journal field.

### Unsafe SP1 environment guard

The proving API rejects every pinned SP1 setting that writes private execution
material, changes worker topology, or weakens verification:

```text
SP1_DUMP
SP1_RECORD_WRITE_DIR
TRACE_FILE
DUMP_ELF_OUTPUT
SP1_RECORD_MAX_ARITY_INPUT
SP1_RECORD_SHRINK_INPUT
WITHOUT_VK_VERIFICATION
SP1_CIRCUIT_MODE
SP1_WORKER_*
```

Any presence returns a typed configuration error before key setup or `SP1Stdin`
construction. Production services must apply the same check at process startup and
must not mutate these variables later.

Proof logs and benchmark output contain only public identifiers, aggregate timings,
aggregate sizes, instruction count, and resource measurements. They never format the
guest input or SP1 stdin.

### Plonk setup artifacts

The release setup archive is:

```text
url = https://sp1-circuits.s3-us-east-2.amazonaws.com/v6.1.0-plonk.tar.gz
expected_size = 2305113029
```

The first download is an approved trust-on-first-use setup step over HTTPS. It runs
without guest input or witness data. The setup command checks the byte count, rejects
unsafe paths, links, devices, duplicate entries, more than 1,024 entries, and more
than 8 GiB of expanded regular files. It computes SHA-256, records the digest in the
pinned toolchain metadata, and extracts through a staging directory with GNU tar in
the pinned Linux image.

Later setup runs require the pinned SHA-256 before extraction. The proof container
mounts the extracted `v6.1.0` directory read-only at the configured
`SP1_PLONK_CIRCUIT_PATH`. It mounts an allowlisted, owner-only run bundle instead of
the repository. The witness-bearing proof container uses `network_mode: none`, a
read-only root and workspace, and a private `/tmp` tmpfs.
Per-file SHA-256 values cover the constraints, circuit, proving key, and verification
key before each proof process starts. Missing or changed setup files fail before
`SP1Stdin` construction, so the SDK cannot download artifacts during proving.

### Verification order and failures

The verifier performs these steps:

1. Check the artifact size.
2. Decode the outer frame strictly.
3. Check the SDK version, circuit version, and Plonk mode.
4. Decode the SP1 payload strictly.
5. Require an absent TEE proof.
6. Compare the request identifier with the verifier-owned trusted-key identifier.
7. Recompute the expected ADR-0011 journal from capability claims.
8. Compare the complete journal byte sequence.
9. Verify the SP1 proof with `LightProver`, the verifier-owned key, and `SUCCESS`.

Each failure maps to one typed internal error. A service later maps these failures to
the coarse public capability-invalid response. The in-process M3 adapter catches a
normal Rust unwind from SP1 parsing or verification and fails closed. M4 still needs
process isolation for aborts, out-of-memory termination, and hard deadlines.

## Approved Dependencies And Toolchain

Implementation adds two direct, exact Cargo dependencies already present in
`Cargo.lock` through SP1:

```toml
bincode = "=1.3.3"
sp1-core-executor = { version = "=6.3.1", default-features = false }
```

`bincode` provides the in-memory SP1 payload encoder and strict bounded decoder.
`sp1-core-executor` exposes `SP1CoreOpts` so Raziel can set a 2 GiB executor memory
limit, one trace slot, and a 1 Mi-cycle shard.

The approved amendment changes the exact SDK feature set:

```toml
sp1-sdk = {
  version = "=6.3.1",
  default-features = false,
  features = ["blocking", "native-gnark"]
}
```

`native-gnark` activates SP1's pinned Go/Gnark bridge and locked `bindgen 0.70.1`
dependency. The pinned Debian Bookworm toolchain image installs:

```text
libclang-14-dev = 1:14.0.6-12
```

Go remains pinned at `1.26.5`. Network, CUDA, and experimental SP1 features remain
disabled.

Without `bincode`, the SDK only offers file-based proof save and load helpers with a
trailing-byte-permitting decoder. Without `sp1-core-executor`, Raziel must accept
SP1's 24 GiB default child-process limit or control proof memory through global
environment variables.

## Test Requirements

M3 tests must cover:

- stable verification-key ID bytes and text
- proof artifact encode and decode equality
- strict artifact and inner-payload size ceilings
- wrong magic, version, mode, circuit version, and trailing bytes
- changed proof bytes
- another verification key and key identifier
- changed, truncated, and extended public journal
- a present TEE proof
- every forbidden SP1 environment setting
- verifier-owned trusted guest key selection
- panic containment around malformed proof verification
- a successful local Plonk proof and independent `LightProver` verification
- the benchmark gate and recorded workstation evidence
- feature-tree exclusion of SP1 network, CUDA, and experimental clients
- required `native-gnark` and `blocking` feature-tree checks
- setup archive size, SHA-256, path, and link checks
- a read-only artifact mount and network-isolated witness-bearing proof run

## Consequences

- The successful Plonk prove and verify gate closes M3 and O-003.
- Core remains suspended for agent-facing and verifier-facing capability artifacts.
- M4 must configure service message-size and job limits from the measured proof size.
- M4 must decode and verify remote artifacts in a memory-capped, time-capped child
  process.
- A guest ELF change that alters the program key, a circuit-version change, or a key
  digest change creates a new verification-key record.
- An SDK release, proof-mode change, or artifact-encoding change needs a compatible
  artifact version and a new decision. It rotates the verification-key record only
  when the circuit or program key changes.
- Compressed proof research stays open for a later deployment profile with more
  memory or hardware acceleration.

## Options Rejected

### Default environment-selected prover

An environment-selected client can route work to another prover implementation. The
private witness boundary requires a concrete local `CpuProver`.

### SP1 native save and load as the wire contract

The SDK file helper permits trailing bytes and has no Raziel mode or version frame.
It remains useful for local debugging with public fixtures, not capability parsing.

### Hash the serialized verification key

Serde and bincode representations can change without changing the SP1 program key.
SP1's program-key digest is the stable identity input.

### Start with Compressed

Compressed mode does not provide the approved zero-knowledge outer proof. It adds
recursive aggregation and does not resolve the Core privacy hold.

### Use Groth16 for the first outer proof

Groth16 needs a 6,211,807,514-byte circuit-specific setup archive. Its smaller proof
and lower on-chain gas do not benefit this local PoC. Plonk uses the smaller approved
release archive and provides the required computational zero-knowledge property.
