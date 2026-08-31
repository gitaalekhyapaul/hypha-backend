# ADR-0017: Durable Capability Cancellation Control

- Status: accepted
- Date: 2026-08-24
- Milestone: M4
- Related decisions: ADR-0013, ADR-0014, ADR-0015
- Related baton: B-0027
- Security review triggers: capability cancellation, capability reuse, queue lifecycle,
  private-envelope cleanup, logs containing authorization context

## Context

ADR-0013 makes BullMQ's retained Redis state authoritative for capability jobs. It
requires queued cancellation to remove the proof job, delete the private envelope,
and retain a typed `CANCELED` result. Duplicate cancellation must be deterministic,
and a terminal job must remain immutable.

The pinned `bullmq-official` `1.2.3` API cannot implement all those requirements on
one job record:

- `JobState` contains waiting, active, delayed, prioritized, completed, failed,
  waiting-children, and unknown. It has no cancelled state.
- `Queue::remove` deletes the job record. A later Get or duplicate Cancel call has no
  authoritative record to read.
- `Worker::cancel_job` signals only an active job held by that worker process. It
  cannot cancel a waiting job.
- A waiting job cannot be moved to failed through the public queue API. BullMQ's
  manual `move_to_failed` path requires an active worker lock token.

Treating a missing job as cancelled is unsafe. A missing record can mean expiry,
operator damage, a wrong identifier, or a queue consistency fault. It cannot prove
that cleanup completed.

Letting the proof worker claim a queued job and then cancel it would retain a failed
`CANCELED` record, but global proof concurrency is one. A queued cancellation could
wait behind a 29-minute Plonk proof before deleting its private envelope.

M4 needs one durable control record that exists before the proof job is removed or
its active cancellation token is signalled.

## Decision

### Queue topology

Add one internal BullMQ queue named `cancellation-commands`. The existing proof queue
remains `proof-jobs`. Both names sit under the configured Raziel queue prefix. Both
queues use the pinned BullMQ binding and owner-only Redis service.

The API publishes a cancellation command with the same 64-character lowercase job
identifier as the target. Queue names separate the two records. Exact duplicate
Cancel calls resolve to the retained command instead of creating another mutation.

The command payload is versioned and bounded. It contains only:

- the target capability job identifier
- the target submission timestamp observed before publication

It contains no private-envelope reference, issuance request, relationship, caveat
value, salt, witness, proof input, path, bearer token, or signing material.

The command uses a fixed job name and fixed options. Its retained terminal value is a
closed public outcome. Version 1 permits:

- `CANCELED`, with the original submission time and confirmed cancellation time
- `TERMINAL_UNCHANGED`, when the proof job became terminal before cancellation won

The cancellation command never stores a `ProofCapability`. Successful proof results
remain solely in the proof queue.

### Runtime ownership

The existing `razield-proof-worker` process owns a second lightweight BullMQ worker
for cancellation commands. This worker holds the existing proof worker handle and
private-envelope store. No new network listener, Unix socket, database, process, or
package is added.

The cancellation worker is the only runtime component allowed to remove a waiting
proof job. Its command is durable before the target mutation. This ownership rule
makes a missing target during command recovery meaningful: the command publisher
already observed an exact target, so a missing target records a prior queued removal
that still needs cleanup confirmation.

Cancellation commands use bounded retries. Proof jobs retain `attempts = 1`; proof
execution is never retried. A cancellation retry repeats only idempotent queue-state
inspection, worker signalling, and private-envelope deletion.

### Queued cancellation

For a waiting, delayed, or prioritized proof job, the cancellation worker:

1. validates the command and exact retained proof-job identity
2. removes the proof job through `Queue::remove`
3. deletes the private envelope and accepts an already-absent file during recovery
4. records the command outcome as `CANCELED`

It does not load or decode the private envelope. It reports no terminal cancellation
until deletion succeeds.

### Active cancellation

For an active proof job, the cancellation worker:

1. validates the command and exact retained proof-job identity
2. calls `Worker::cancel_job` on the local proof worker handle
3. waits for the one-shot executor termination and private cleanup path
4. confirms that the proof job reached failed state with exact reason `CANCELED`
5. records the command outcome as `CANCELED`

ADR-0015 keeps the ordering inside the proof processor. It terminates and waits for
the child, deletes the private envelope, and only then returns the typed cancellation
failure to BullMQ.

The cancellation command queue replaces ADR-0013's unspecified internal control
endpoint. It is the durable internal control endpoint for both waiting and active
jobs.

### Terminal races and public RPC behavior

A completed or non-cancellation failed proof job is immutable. A cancellation
command that loses a race records `TERMINAL_UNCHANGED`. Get and Cancel return the
original proof terminal state.

`CancelCapability` is asynchronous. It validates the identifier, publishes or finds
the exact cancellation command, and returns the current authoritative public proof
job state. It does not claim success before cleanup is confirmed. Clients poll
`GetCapability` through the existing job resource.

`GetCapability` reads the proof record first. It maps:

- waiting, delayed, or prioritized to `QUEUED`
- active progress `EXECUTING` to `EXECUTING`
- active progress `PROVING` to `PROVING`
- completed with a strictly decoded terminal result to `SUCCEEDED`
- failed reason `CANCELED` to `CANCELED`
- another allowed failed reason to `FAILED`

If the proof record is absent, Get reads the cancellation command. A completed,
strictly decoded `CANCELED` command maps to `CANCELED`. A pending command maps to the
`QUEUED` state. A missing proof can occur here only after the cancellation worker
removed a queued target and before it retained the command result. An absent proof and
absent cancellation record maps to not found. Corrupt, inconsistent, or unavailable
records fail with a generic service error.

The API never reads the private envelope for Get or Cancel. It returns only generated
Protobuf fields assembled from strictly validated public queue data.

### Retention and cleanup

Cancellation commands use fixed age and count retention ceilings. Their retention
window matches or exceeds the proof-job retention window. This keeps duplicate
cancellation deterministic for every retained capability job.

Startup reconciliation treats a completed cancellation command as authority to
remove an orphaned private envelope. It never treats a missing job alone as evidence
of cancellation.

## Trust-Boundary Effect

The proof supervisor gains ownership of a second owner-only BullMQ queue. It already
holds the proof worker cancellation handle, Redis access, and private-envelope store.
No private capability data crosses a new process boundary.

The API can publish public cancellation commands. It still cannot terminate a prover
directly, read a private envelope for lifecycle responses, or manufacture a terminal
state.

The agent sees no queue payload or cancellation control record. It sees only the
existing public `ProofJob` projection.

## PoC Operating Assumptions

The maintainer accepted these M4 consistency limits for the PoC:

- one Redis instance and one proof-supervisor process own the queues
- global proof concurrency remains one
- BullMQ records remain authoritative for lifecycle state
- `CancelCapability` means cancellation was accepted, not already completed
- clients poll `GetCapability` for the terminal result
- a brief generic unavailable response is valid during a bounded queue transition
- terminal proof states never change
- a missing proof record never implies cancellation without its retained command
- cancellation is idempotent but is not atomic with proof-job state
- restart recovery provides eventual cancellation and private-envelope deletion
- restart recovery does not promise that proof execution never started

### 2026-08-31 submission and capacity amendment

The maintainer accepted two further limits after the M5 live walkthrough exposed an
uncertain create response followed by a second signed request:

- one distinct nonterminal proof job can exist in the PoC proof queue
- an exact duplicate request still resolves to its retained deterministic job
- a different request is rejected without retaining a private envelope or BullMQ job
  when a job is waiting, active, delayed, prioritized, or paused
- the M5 console assigns one operator-scoped submission ID to one browser action
- an uncertain retry reuses the exact signed transport request, including its nonce,
  expiry, context, and signature
- the console checks `GetCapability` for the expected commitment-derived job ID
  before resending that exact create command

The submission ID is API orchestration metadata. It is not a capability field, proof
input, public claim, commitment input, or BullMQ job identifier. The server keeps its
signed retry record in bounded process memory and never logs or returns the private
request context.

One fixed BullMQ deduplication ID owns the proof-capacity slot. BullMQ arbitrates
concurrent publishers atomically and releases the slot at terminal completion or
queued removal. The publisher checks the slot before normal staging. A concurrent
loser can stage during the race, then deletes that envelope before returning busy.
The busy result reveals only occupied proof capacity. It reveals no existing job ID,
policy, dataset, agent, relationship, caveat value, or private-envelope reference.

## Public and Private Data Effect

The new retained data is public lifecycle metadata: job identifier, submission time,
cancellation time, command version, and closed outcome. It reveals no policy body,
relationship, caveat value, private reference, witness field, satisfying path, salt,
or proof input.

Private envelopes are deleted sooner for queued cancellation and only after child
termination for active cancellation. Logs use job-independent event names and typed
classes. They do not print job identifiers or queue payloads.

## Failure and Recovery Rules

- A command publication failure leaves the proof job unchanged.
- A worker crash before target mutation leaves a retained command for retry.
- A worker crash after queued removal retries cleanup and terminal command recording.
- A worker crash after active signalling reads the proof terminal state before any
  repeated signal.
- A failed private-envelope deletion prevents a `CANCELED` outcome.
- A lost proof-worker lock remains `FAILED`, not `CANCELED`, unless an accepted
  cancellation command caused the exact typed cancellation path.
- Queue transport errors never map to missing or cancelled.
- BullMQ job fields and set membership are separate reads. The cancellation worker
  retries a bounded inconsistent transition snapshot before treating it as unavailable.

## Validation

B-0027 must prove:

- strict Get mapping for every accepted BullMQ state and progress value
- strict terminal result decoding with malformed and oversized rejection
- queued cancellation removes the proof job and private envelope before `CANCELED`
- active cancellation reaches the existing token and waits for cleanup
- a terminal race returns the immutable proof result
- duplicate Cancel calls retain one command and one outcome
- cancellation-worker restart recovers before and after target mutation
- unknown, expired, malformed, and mismatched records fail closed
- Redis and RPC responses contain no private envelope or witness data
- all checks run without SP1 proof generation

## Rejected Options

### Treat a missing proof job as cancelled

This loses deterministic duplicate behavior and confuses cancellation with expiry or
corruption.

### Wait for the proof worker to claim the queued job

This can retain a typed failure, but cancellation can wait behind one full Plonk
proof and leaves the private envelope in place.

### Write a cancellation tombstone through raw Redis commands

This recreates lifecycle storage outside BullMQ and violates ADR-0013's queue
ownership rule.

### Keep cancellation state in memory or on the API filesystem

This loses state across restarts and makes API replicas disagree.

### Add a database only for lifecycle state

This adds another authority and deployment dependency when BullMQ already supplies
durable commands, custom identifiers, locks, retries, and retention.

## Approval Record

The maintainer approved this decision on 2026-08-24. The approval accepts the durable
BullMQ cancellation-command queue, its public data boundary, its recovery rules, and
the PoC operating assumptions in this record.

The maintainer approved the submission and capacity amendment on 2026-08-31. This
approval keeps the capability format, proof statement, witness, and 75-minute
worker-start validity rule unchanged.
