# M5 Local Portal

Status: synthetic recorded workflow and UF-08 live VM deployment prepared

UF-08 is implemented and passes local build, type, lint, unit, Rust bridge, and
Compose configuration checks. A live VM walkthrough is still required after the
B-0051 worktree is committed, pushed, and pulled onto the VM.

## Start

Docker is the only host prerequisite. The app profile uses the pinned repository
toolchain, creates an owner-only local operator verifier, and builds the policy
compiler, candidate inspector, policy activation service, Access Graph publisher, and
snapshot service from the pinned Go and Rust lockfiles.

```bash
export RAZIEL_M5_OPERATOR_PASSWORD='choose-a-local-password'
./scripts/start-m5-demo --recorded
```

Open `http://127.0.0.1:3000` and sign in with the exported password.

The default `recorded` capability mode plays the checked-in public M4 timeline. It
cannot issue a capability or unlock protected transaction data.

The mutation routes accept the request's actual same origin. Both
`http://127.0.0.1:3000` and `http://localhost:3000` work through an SSH tunnel.

For an ngrok development URL, add its hostname to the Next.js development-origin
list before startup. Set the expected origin only when a reverse proxy rewrites the
request host:

```bash
export RAZIEL_OPERATOR_EXPECTED_ORIGIN='https://demo.example.ngrok.app'
export RAZIEL_CONSOLE_ALLOWED_DEV_ORIGINS='demo.example.ngrok.app'
./scripts/start-m5-demo --recorded
```

The gateway stays on the private Compose network at `http://demo-gateway:3001`.
Browser requests remain same-origin and need no wildcard CORS policy.

The profile stores ignored runtime state under `.raziel/m5/`:

- `operator-secret.json` contains a salted scrypt verifier and session HMAC key.
- `transactions.json` contains portal-created synthetic transaction revisions.
- `policy-tools/` contains the local compiler and inspector executables.
- `policy-build/` caches their locked build outputs and Go modules.
- `policy-candidates/` contains owner-only candidate source and compiled artifacts.

Docker volumes hold the policy authority key, snapshot authority key, trusted
registry, private relationships, snapshots, retry state, and service sockets. The
console container mounts only the activation and Access Graph client sockets. It
cannot read either authority key, the trusted PolicyIR registry, SpiceDB token,
private relationships, or private snapshots.

The first startup downloads locked build dependencies inside the toolchain container.
Later starts reuse the local cache.

The initializer preserves an existing verifier. Set
`RAZIEL_M5_RESET_OPERATOR=1` for one startup to replace it with the current password.

## Current Workflows

The Dataset workspace creates, edits, finalizes, and clones synthetic reconciliation
revisions. Finalized revisions show their checksum and schema fingerprint.

The Policy workspace supports five visual rule families:

- direct agent assignment
- team membership
- alternative paths through direct, team, or organization delegation
- dual control through two required direct relations
- one direct relation with a supported caveat template

`Compile candidate` runs the real SpiceDB compiler adapter and Rust candidate
inspector. The browser receives only the safe public candidate projection.

`Approve and activate` sends the symbolic candidate ID and authenticated operator ID
to an isolated Rust activation service. That service asks the policy authority to
sign the exact private candidate, then asks the registry writer to publish the next
trusted generation. The browser receives only the active record ID, policy identity,
policy hash, epoch, status, timestamps, and registry generation. Repeating the action
returns the same active record without another signature or generation.

The Access Graph workspace unlocks after policy activation. It derives the bounded
scenario relationships supported by the selected visual rule. The operator can
create one server-owned ephemeral Ed25519 agent identity, then review the key-bound
agent subject, dataset, team or organization, task, purpose, scope, snapshot lifetime,
and applicable caveat inputs before publishing. The default lifetime is 7,200 seconds.

`Publish access graph` sends the relationship batch through an authenticated console
route to the owner-only Rust publisher. The publisher writes the approved candidate
schema and relationships to SpiceDB, then asks the snapshot-only service to publish a
signed snapshot at that private revision. The browser receives only the policy record
ID, policy hash, snapshot ID, relationship root, epoch, validity, accepted count,
registry generation, and status. It never receives the relationships, caveat values,
ZedToken, salts, Merkle openings, PolicyIR, or authority signatures. An exact retry
returns the same snapshot.

The Agent and Capability workspace unlocks after an accepted snapshot. It shows the
public agent thumbprint and the exact task, purpose, action, resource scope, request
context, policy hash, snapshot ID, and requested validity. The server signs the exact
request with the existing TypeScript SDK. The private key remains in console process
memory.

`Request capability` returns one stable job ID. The screen polls queued, executing,
proving, succeeded, failed, and canceled states. Recorded success stays visibly
non-authorizing and returns no capability.

In live mode, a succeeded capability unlocks Reconciliation only for its exact
finalized dataset. `Run protected query` signs a bounded time-window query with a
fresh nonce. The private gateway claims the nonce, asks the independent verifier to
verify the capability and request signature, checks the exact dataset scope, reads
the bounded synthetic rows, and writes a public JSONL receipt before returning the
result. The browser receives no raw capability proof, private key, PolicyIR,
relationship data, caveat values, or witness material.

The active policy selection currently lasts for one browser session. A page refresh
requires the operator to open Policy and activate the candidate again. This does not
create another policy record for the same activation request.

Policy version and approval epoch remain explicit operator inputs. A fresh trusted
registry starts at version 1 and epoch 1. A persistent registry requires the next
epoch for a new record under the same policy name. The current PoC does not expose a
registry-history read API to the browser, so it cannot derive that value after a
server restart.

## Deploy To The VM

The VM must first pull a commit that contains B-0051. Do not run the live profile
from the current remote branch until that commit has been pushed.

```bash
cd ~/hypha-backend
git pull --ff-only origin main
git rev-parse --short HEAD
```

Docker remains the only host runtime. The repository toolchain installs and builds
the pinned Node, Rust, Go, and SP1 dependencies inside containers.

## Live VM Proof And Protected Query

The live profile needs the prepared Plonk setup already used by the M4 VM run. From
the repository root on the VM:

```bash
export RAZIEL_M5_OPERATOR_PASSWORD='choose-a-local-password'
./scripts/start-m5-demo
```

`start-m5-demo` defaults to the live server profile. It derives the host user,
host group, retained verification-key ID, and retained SP1 verification-key hash
from the current machine and checked-in fixture. It fails before Compose startup
when the Plonk setup or either pinned verifier value is missing. Raw Compose and
`start-m5-demo --recorded` retain the lightweight mode for local and programmatic
testing.

The profile starts the UI, owner services, Redis queue, capability-only API, proof
worker, 60 GiB no-swap proof sandbox, independent verifier, and two-GiB networkless
verifier sandbox. The proof sandbox uses eight CPUs and has no network. The measured
M4 service proof took 955,516 ms, about 15 minutes and 56 seconds. Allow about 20
minutes for the live capability step.

Confirm the services started:

```bash
./scripts/compose \
  --profile apps \
  --profile m5-live \
  ps

curl --fail --silent --show-error http://127.0.0.1:3000 >/dev/null
```

On the Mac, forward only the console port:

```bash
ssh -N -L 3000:127.0.0.1:3000 sadm@gapaul-vm-i
```

Open `http://127.0.0.1:3000`. The console calls the gateway, capability API, and
verifier inside Compose. Do not forward ports `3001`, `50051`, `50052`, `50053`, or
`6379`.

For ngrok on the VM, keep the console bound to loopback and point ngrok at
`http://127.0.0.1:3000`. Set the exact public origin variables shown in the Start
section before starting Compose.

## Demo Walkthrough Available Now

Use the live VM profile for this walkthrough:

1. Sign in with `RAZIEL_M5_OPERATOR_PASSWORD`.
2. Open Dataset. Create or select the synthetic reconciliation revision, then save
   and finalize it. Show its row count, checksum, and schema fingerprint.
3. Open Policy. Pick one supported visual rule shape, review the visual blocks, then
   select `Compile candidate`.
4. Review the compiled public summary, then select `Approve and activate`. Show the
   active policy record and `policy_hash`.
5. Open Access Graph. Create the ephemeral agent, review the policy-bound access
   inputs, then select `Publish access graph`. Show the snapshot ID and relationship
   root. Private relationships remain hidden.
6. Open Agent and capability. Review the agent, task, purpose, action, dataset scope,
   policy hash, and snapshot. Select `Request capability` once.
7. Keep the page open during queued and proving states. The current VM profile should
   finish in about 16 minutes based on the accepted M4 run. Wait for `SUCCEEDED` and
   the public capability claims.
8. Open Reconciliation. Keep the default UTC window or choose a bounded window, set a
   row limit from 1 to 100, then select `Run protected query`.
9. Show the returned `SYNTHETIC` transaction rows and the public receipt. Point out
   the `VERIFIER` result, capability, proof job, policy, snapshot, request, proof
   digest, and accepted outcome fields.
10. Select `Run protected query` again without requesting another capability. The
    console signs a fresh nonce. Show `CACHE HIT`, the same capability and proof
    digest, and a different request and receipt ID. The gateway still checks the
    agent signature, freshness, replay nonce, key binding, target, and scope.

After UF-08 succeeds, inspect the owner-side receipt and bounded service logs on the
VM:

```bash
tail -n 1 .raziel/m5/receipts.jsonl

./scripts/compose \
  --profile apps \
  --profile m5-live \
  logs --tail 100 demo-gateway demo-verifierd demo-proof-worker
```

The current demo stops after verified capability reuse. User-controlled scope denial,
replay controls, and receipt filtering or export are scheduled in UF-10 through
UF-12. Recorded mode can demonstrate the portal and public proof timeline, but it
cannot unlock Reconciliation.

## Stop

```bash
./scripts/compose --profile apps --profile m5-live down --remove-orphans
```

This workflow is labeled `SYNTHETIC`. It does not satisfy the M6 real-data gate.
