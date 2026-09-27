# Agent Access Inbox

Status: B-0065 complete locally. Work paused before B-0066. No VM deployment is claimed.

## Available Flow

1. A registered agent signs an access intent for one finalized dataset revision.
2. Raziel stores the request under that registration and request ID.
3. The operator opens **Access requests** to inspect the request.
4. **Pending review** shows pending records on the current page. **All requests**
   includes retained terminal records. The agent ID filter and page arrows narrow the list.
5. Selecting a request shows its task, purpose, dataset, state, generation, and times.
6. Select an installed approval in **Approved policy version**. Review its hash and
   validity, set the private grant options, then select **Approve request**.
   The result is **Approved, waiting**, bound to that version and finalized dataset.
7. **Reject** ends a pending review. **Cancel** ends a request before dispatch.
   A stale generation refreshes the details rather than overwriting newer state.
8. The agent reads its own result. Another registration cannot read or cancel it.

Approval is connected. The private request-preparation API has passed a real
SpiceDB and snapshot-authority check. The dashboard stops at **Approved, waiting**.
B-0066 will reserve proof capacity, invoke preparation, and request the agent's
final signature. No inbox status grants data access. Existing M5 proof flows remain
separate. The agent dashboards are later B-0067 work.

## Request Contract

The current agent endpoints are `POST` and `GET /api/agent/v1/access-requests`,
`GET /api/agent/v1/access-requests/{requestId}`, and
`POST /api/agent/v1/access-requests/{requestId}/cancel`.
Every call requires the registered token and Ed25519 transport signature.
See [Agent API v1](../spec/AGENT-API-V1.md) for the exact headers and ProtoJSON types.
The local SDK candidate exposes typed create/list/get/cancel methods. Its public
README includes examples with `AccessIntentSchema` and `AccessRequestMutationSchema`.

Persist the intent's UUID before the first send. An exact retry uses that UUID and
a fresh transport nonce. A changed intent returns `CONFLICT`. Two agents can choose
the same UUID without sharing a record. Exact retained retries still work during a
dataset-service outage. New requests require a finalized dataset and its checksum.

The owner sets a 24-hour review deadline. Initial limits are 100 active requests per
installation and 16 per agent. Terminal records remain for seven days. Private
request context stays in owner-only storage for exact retry comparison, not in
Redis or the public response.

## Runtime

### Policy Version Preparation

The owner API now prepares immutable version identities through
`POST /api/policies/versions`. It accepts the existing supported `draft`,
`expectedApprovalEpoch: 1`, and `validForSeconds`. Omit `familyId` for a new family,
or pass an existing family ID for its next version. The owner allocates both opaque
IDs and the positive version number. Caller draft names and numbers do not select
the signed identity or resource namespace.

The response contains `identity` and the safe `candidate` review projection.
`GET /api/policies/versions/{candidateId}` reads that projection. Both routes require
the operator session, and mutations require the existing CSRF checks. These are
operator routes, not agent SDK endpoints.

An exact candidate retry reuses its allocation. A changed request conflicts.
Failed drafts consume a version number, so later versions never reuse that identity.
Allocation metadata survives temporary candidate cleanup. The private directory is
`RAZIEL_POLICY_VERSION_ROOT`, prepared as `.raziel/m5/policy-versions` by the existing
Compose tool-preparation step.

The visual builder selects this flow in the opt-in M6 runtime. M5 remains the default,
and its legacy activation service rejects the reserved `family:` namespace. New
versions use a separate publication endpoint and owner service mode. No snapshot or
proof starts from version compilation.

The offline schema composer is implemented and tested. It checks up to 32
namespaced versions plus one retained legacy package against their exact PolicyIR.
Its [private manifest contract](../../tools/spicedb-ir-compiler/README.md#compose-retained-versions)
is an owner-tool interface, not an agent API. The compiler itself does not install
the schema or verify registry approval.

### Version Publication

`POST /api/policies/versions/{candidateId}/publish` requires the operator session and
CSRF checks. Its result contains the allocated `identity` and safe `publication`
projection. It checks the returned record ID, version, hash, and approval epoch
against the prepared candidate. No retained source, compiler config, or grant recipe
is returned.

The endpoint requires `RAZIEL_POLICY_PUBLICATION_SOCKET_PATH`. It never falls back
to the legacy activation socket. The owner process uses the existing
`raziel-policy-activation` binary with `RAZIEL_POLICY_PUBLICATION_MODE=composed`.
That mode rejects legacy candidate publication and performs these steps:

1. Take the short owner publication lock and reconcile a pending schema write.
2. Recompile the installed packages and proposed version. Check aggregate bounds.
3. Compare the live schema with the recorded composition.
4. Ask the separate policy authority to approve the exact candidate.
5. Append the signed record through the networkless registry writer.
6. Retain the approved source, config, PolicyIR, and approval record privately.
7. Record the pending composition, write it to SpiceDB, then check the read-back.
8. Save the installed generation and return the safe projection.

An uncertain schema write leaves a private pending record. Retry accepts only the
recorded previous schema or the exact proposed composition. Unexpected drift returns
a conflict without overwriting the live schema. An expired or revoked package stays
installed for schema consistency but cannot authorize a new snapshot.

Private roots must be separate `0700` directories outside candidate cleanup:

| Setting | Purpose |
| --- | --- |
| `RAZIEL_POLICY_PUBLICATION_ROOT` | Installed and pending composition state, short writer lock |
| `RAZIEL_POLICY_PACKAGE_ROOT` | Retained immutable packages |
| `RAZIEL_POLICY_VERSION_ROOT` | Existing durable identity allocations |
| `RAZIEL_POLICY_COMPILER_EXECUTABLE` | Pinned compiler with `compose` and `compare-schema` |
| `RAZIEL_POLICY_LEGACY_PACKAGE_ROOT` | Optional exact historical M5 source/config/PolicyIR/manifest directory |
| `RAZIEL_SPICEDB_ENDPOINT`, `RAZIEL_SPICEDB_TOKEN_PATH` | Owner-only schema connection and credential |

The existing authority, registry-writer, candidate, registry, and activation-socket
settings still apply. The composed role needs network access to SpiceDB. The policy
authority and registry writer do not need network access.

The opt-in M6 deployment selects this mode and removes legacy writer services.
Do not deploy it beside an active M5 access-graph publisher. Existing nonempty schema
requires a validated exact legacy package, not a reflected-source reconstruction.
Missing legacy source fails closed. Verification of retained proofs is unchanged.

### Local M6 Preview

This is a partial local feature preview, not the full agent demonstration or a VM
deployment instruction. It requires the existing pinned toolchain image, approved
SpiceDB and Redis images, installed workspace dependencies, generated Protobuf code,
and built `proto-ts` and `sdk-ts` packages. It never pulls images automatically.
The render-only configuration check needs the repository Node.js toolchain on the host.

Current Linux owner binaries must exist in `.raziel/m5/policy-tools`. The existing
preparation command builds them through the pinned image:

```bash
./scripts/compose --profile apps run --rm --no-deps demo-console-policy-tools
```

That preparation step uses the existing M5 build directory and prepares missing M5
keys. It does not start M5 services or publish a policy. Do not use it to replace
tools during an active M5 demonstration. The M6 initializer copies only executables
into its own volume and creates separate authority keys. It never imports M5 policy,
approval, relationship, or snapshot state.

Start the partial preview on a free port:

```bash
export RAZIEL_M6_CONSOLE_PORT=3011
./scripts/check-m6-policy-compose
./scripts/compose -f docker-compose.yml -f docker-compose.m6.yml \
  --profile apps up --detach --wait --wait-timeout 120
```

Open `http://127.0.0.1:3011/`. Operator provisioning uses the existing
`RAZIEL_M5_OPERATOR_ID` and `RAZIEL_M5_OPERATOR_PASSWORD` settings, but stores the
M6 configuration separately at `.raziel/m6/operator-secret.json`. Do not override
the project name `raziel-m6` or point it at M5 volumes. Only the console has a host
port. Redis, SpiceDB, and owner RPC services remain private.

In **Policy**:

1. Select **New policy family**, then one of the five supported rule families.
2. Select **Compile version**. Review the server-assigned family, version, and hash.
3. Select **Publish version**. The result shows **Approved** and its registry generation.
4. Select **Draft next version** to retain that family, change the rule, then compile
   and publish again. Earlier packages remain installed.
5. **Installed approvals** lists every installed version that is still approved.
   Select **Use family** to prepare another version without copying its ID.
6. Use the refresh icon to reload the catalogue. A page reload retrieves the same
   installed versions from the owner service. Expired or revoked approvals disappear.

The catalogue is read-only. Selecting a family does not grant agent access. Recovery
of unfinished drafts across page reloads is not implemented. Compilation retains its
exact candidate for retries within the current page. An
uncertain publication must be retried against that same candidate.

Publishing creates no grant, snapshot, or capability. M6 hides the legacy access
graph and capability panels, and server guards reject their mutation routes. The
request inbox supports inspection, approval, rejection, and cancellation. Dataset
creation and finalization remain available. The approved request waits for dispatch.

The owner-only dataset-binding store and gateway enforcement are implemented. M6
requires exact policy/type/scope mapping and finalized revision/checksum checks on
each cold or cached request. This preview initializes the private binding volume and
mounts it read-only at the gateway. Approval writes the exact binding. No
verifier or proof service runs here. This is not a connected M6 protected-query demo.

This preview uses an in-memory SpiceDB datastore. Restarting SpiceDB loses its live
schema but retains owner installation records. Later publication fails closed on
that drift. Restart recovery and durable deployment are not claimed. Do not delete
volumes to work around that check or use this partial runtime for a recorded demo.

The 2026-09-27 local gate used current debug-profile Rust binaries built in the pinned
Docker toolchain plus the pinned Go compiler. Release preparation is the command
above, not a claim that a fresh release build ran during this gate.

### Access Owner

The console calls `raziel-access-client`, which sends generated Protobuf messages to
the private `demo-access-api` Compose service. Its `razield-access` binary runs
without the SP1 proving feature. The `demo-access-private-init` service prepares the
owner-only `m6-access-intents` volume. No new public port is exposed.

| Setting | Purpose |
| --- | --- |
| `RAZIEL_ACCESS_ENDPOINT` | Private owner RPC, Compose default `http://demo-access-api:50054` |
| `RAZIEL_ACCESS_CLIENT_EXECUTABLE` | Pinned-toolchain owner RPC helper |
| `RAZIEL_ACCESS_PRIVATE_ROOT` | Owner-only private intent directory |
| `RAZIEL_ACCESS_PREFIX` | Redis namespace, stable across restarts |
| `RAZIEL_REDIS_URL` | Existing private Redis connection |
| `RAZIEL_ACCESS_APPROVAL_MODE` | `installed` in M6, absent disables approval |
| `RAZIEL_POLICY_PUBLICATION_ROOT`, `RAZIEL_POLICY_PACKAGE_ROOT` | Read-only installed-policy selection |
| `RAZIEL_REGISTRY_PATH`, `RAZIEL_TRUST_DOMAIN_ID` | Current signed approval validation |
| `RAZIEL_DATASET_BINDING_ROOT` | Owner-writable immutable dataset mapping |

The approval route accepts only the selected record/version/epoch, the displayed
generation, and bounded private grant fields. Dataset checksums, resource scope,
and agent identity come from trusted retained input, not editable browser fields.
Private grant options cover the supported direct, membership, traversal, combined,
and caveated rule families. They are not proof of an ALLOW decision.

An ambiguous approval response keeps the exact selection for retry. Changing it
requires a fresh request, not retargeting an existing approval. A confirmed conflict
refreshes the current request. Approved records show their exact policy binding and
remove the approval controls. No private grant fields appear in agent responses.

### Private Snapshot Preparation

`razield::request_preparation::RequestPreparation` is the owner-side preparation API.
It is callable without SP1 and is not a new public agent endpoint. B-0066 will call
it only after reserving the serial workflow slot.

1. Read the exact approved request and generation, including its private grant recipe.
2. Revalidate its installed policy package and immutable finalized-dataset binding.
3. Take the short publication lock and provision the namespaced grant in SpiceDB.
4. Retain the accepted composition and exact SpiceDB revision in owner-only storage.
5. Release the publication lock, then validate that composition at the exact revision.
6. Read the selected package's relationships, build the salted snapshot, and publish
   its signed descriptor through the existing authority and registry writer.
7. Recheck cancellation and generation, then return the pinned snapshot proposal.

An exact retry reuses the retained revision, private tree, and signed descriptor.
A private receipt precedes the registry append, so retry can recover an interrupted
append without making a new tree. Changed inputs or a conflicting registry state
fail closed. No relationship tuples, recipes, salts, or openings reach the agent.

The snapshot role selects request mode through `RAZIEL_ZED_TOKEN_REQUEST_ROOT`.
That owner-only directory holds recovery receipts and cannot be combined with fixed
or candidate token modes. The ordinary snapshot RPC rejects this mode without the
trusted preparation source. Existing snapshot key, private-tree, registry, and writer
settings remain required. The connected integration test is
`services/razield/tests/request_preparation.rs`. It requires an explicit synthetic
approved request and isolated owner-service configuration. It never submits a proof.

This B-0065 checkpoint is committed and pushed to GitHub. The preview remains useful for registration,
datasets, version publication, and request review, but is not the complete agent demo.
Pulling this checkpoint does not provide the full M6 runtime. The VM demo checkpoint follows
B-0066 dispatch and protected-query integration, focused checks, commits, and push
approval. Preserve the preview's SpiceDB process until durable deployment is wired.

## Evidence

- Rust access service: 11 focused tests, including 10 live Redis/Tonic checks.
- HTTP facade and operator client: 81 focused tests.
- Integrated request gate: packaged SDK, real Rust signature helper, Redis, owner CLI
  and RPC. HTTP transport runs in-process. Dataset lookup and operator-session
  admission are test doubles in this gate.
- SDK package: 220 tests, standalone build/types, and public export checks.
- Production console build passed. Browser sign-in, connected inbox, terminal
  records, and request details passed at `http://127.0.0.1:3010/` on 2026-09-26 UTC.
- No proof, npm publication, or VM deployment is claimed by this baton.
- Version allocation and pinned compiler integration created two distinct versions,
  preserved the first source, and passed exact-retry checks on 2026-09-27 UTC.
- Snapshot mutation confirmation accepts newer unrelated registry generations and
  rejects changed roots or revoked policy bindings. Focused worker tests passed.
- Offline composition and CLI tests passed in the pinned Docker toolchain, with
  network dependency access disabled. The 19 new top-level tests cover independent
  namespaces, exact expected PolicyIR, limits, and private output handling.
- Composed publication passed against the pinned SpiceDB 1.54.0 image on 2026-09-27
  UTC. The real compiler, policy authority, and registry writer published two versions
  without invalidating the first. An exact retry did not add an installation generation.
- Three controlled publication tests passed with the real compiler and authority.
  They cover coexistence, unexpected live schema drift, and a lost write response.
  The last two use a controlled schema-service substitute for fault injection.
- The admin library passed 71 tests. Its three explicit integration gates were
  ignored in the default run and passed in the separate invocation above.
- The connected M6 browser published direct-assignment version 1 and alternative-paths
  version 2 in the same family on 2026-09-27 UTC. Owner state retained two packages
  and no pending write. The flow used real SpiceDB, compiler, authority, and registry.
- This browser check found a read-only allocation-store initialization bug. The
  publisher now opens allocations without writing an ownership probe. The mount stays
  read-only. The Rust suite passed 72 tests with three explicit integration gates skipped.
- The console suite passed 341 tests with 18 opt-in integration cases skipped in
  that invocation. The real browser publication is separate evidence above.
- The Compose gate passed for `apps` and all profiles. Legacy schema, snapshot,
  proof, and verifier services are absent from the M6 override.
- Connected operator approval passed against a finalized dataset and installed v1
  with v2 still installed. The real packaged SDK over HTTP observed `APPROVED_WAITING`
  generation 2 and an exact create retry returned the same request.
- The real preparation gate passed on 2026-09-27 UTC. It returned an accepted signed
  snapshot, retained the selected policy, and reused that snapshot on retry. Public
  request state stayed `APPROVED_WAITING`, with no capability or proof job.
- Final focused validation includes 138 console HTTP/client tests, 117 admin tests
  with four opt-in gates ignored in that invocation, 28 service tests with one live
  gate ignored, 11 access approval/preparation tests against Redis, and scoped Clippy.
  The live preparation result above is separate evidence, not an ignored-test pass.
