# Raziel Scenario Authoring Contract

Status: active scenario contract with ADR-0019 synthetic demo scope amendment

This contract defines the work needed to add a domain scenario to Raziel. Payment
reconciliation is the first scenario and the reference implementation.

A scenario maps domain data and actions onto the shared Raziel authorization
protocol. It does not change the proof protocol.

## Start gate

Do not start a second scenario until the payment-reconciliation workflow passes M7 in
`POC-TECHNICAL-SPEC.md`.

The two-ledger SDK demo extends the existing reconciliation scenario. Under
[ADR-0019](../decisions/0019-public-sdk-and-two-agent-reconciliation.md), seeded and
portal-created synthetic data satisfy its demo gate. The real-source steps in this
contract and the scenario skill apply to deferred real-data work. They do not block
the revised M6-M7 scope. Preserve synthetic labels and actual runtime query behavior.

The reconciliation reference must demonstrate:

- labeled synthetic transaction data with origin, or real data with provenance
- deterministic import
- an approved SpiceDB schema within the supported subset
- PolicyIR compilation and registry approval
- a signed relationship snapshot
- successful local proof creation
- independent verification with `razield` stopped
- one allowed protected query
- one denied request
- bounded capability reuse for rapid I/O
- audit receipts and benchmark output

## Scenario boundary

A normal scenario can add:

- source manifests
- ingestion and canonical domain mappings
- SpiceDB schema, relationships, and supported caveats
- resource and action mappings
- gateway handlers and domain services
- console screens
- test fixtures and real-data acceptance configuration
- audit event attributes approved for that domain

A normal scenario cannot add:

- a new SP1 guest
- a new public proof value
- a new PolicyIR node
- a new canonical hash format
- a new relationship leaf format
- a new capability format
- non-membership or negative proofs
- hidden PolicyIR semantics
- recursive or unbounded authorization

Treat any item in the second list as proof-system work. Create a decision record and a
separate technical plan.

## Required scenario package

Use this target shape:

```text
examples/<scenario-slug>/
  README.md
  scenario.yaml
  source-manifest.example.yaml
  spicedb/
    schema.zed
    relationships.example.yaml
  mappings/
    actions.yaml
    resources.yaml
    fields.yaml
  fixtures/
    README.md
    allowed/
    denied/
  tests/
    acceptance.yaml
```

Runtime adapters belong in the owning app or package. Keep scenario configuration in
`examples/<scenario-slug>/`.

## Scenario descriptor

`scenario.yaml` records stable domain configuration:

```yaml
format_version: 1
scenario_id: payment-reconciliation
display_name: Payment reconciliation
resource_types:
  - dataset
actions:
  - query
policy_entrypoints:
  query: dataset#query
capability_defaults:
  ttl_seconds: 900
  max_rows: 1000
  allowed_output_classes:
    - aggregate
    - exception_reference
```

The descriptor does not contain secrets, relationship tuples, PolicyIR, private caveat
values, or real data paths.

## Real source manifest

Each acceptance source follows the canonical schema in [the PoC technical
specification](POC-TECHNICAL-SPEC.md#real-data-source-manifest). Do not create a
scenario-specific manifest shape.

Do not commit the raw data, direct identifiers, credentials, or custody documents.
Commit `examples/<scenario-slug>/source-manifest.example.yaml` with placeholders. Store
the completed local manifest at `data/manifests/<scenario-slug>/source-manifest.yaml`
or in a controlled evidence bundle. Git ignores the local manifest path.

## Domain analysis

Define these entities before coding:

- subject or agent principal
- human or service delegator
- protected resource type
- resource owner or tenant
- allowed actions
- task and purpose values
- request-time caveat context
- stored relationship context
- output classes
- audit actors and identifiers

For each action, answer:

- Which resource does the capability cover?
- Which SpiceDB permission is authoritative?
- Which public scope fields bind the proof?
- Which private relationship facts satisfy it?
- Which caveats use request-time values?
- Which outputs remain legal inside the capability?
- Which repeated queries create inference risk?

Keep relevance and discovery outside this policy mapping. A relevance service can
produce a dataset commitment. Raziel decides whether the agent can discover, request,
query, or write against that committed resource.

## Policy work

Create a SpiceDB schema that stays inside the supported subset from
`POC-TECHNICAL-SPEC.md`.

Required checks:

- compile through the pinned SpiceDB adapter
- reject every unsupported construct
- emit stable canonical PolicyIR
- register the approved PolicyIR by `policy_hash`
- compare SpiceDB and PolicyIR decisions for fixtures
- cover each union and intersection branch
- cover caveat true, false, missing, and type-error cases

Do not hand-write PolicyIR for a production scenario. Hand-written PolicyIR can appear
only in isolated interpreter tests.

## Data adapter work

The adapter must:

- validate the source manifest before import
- read approved source records or stored synthetic revisions
- map source columns into canonical domain records
- reject missing required fields
- normalize time, currency, amounts, and identifiers deterministically
- produce stable record identifiers without exposing raw identifiers
- record input checksum, output count, rejected count, and schema fingerprint
- avoid logging raw sensitive fields
- support a dry-run validation mode

The adapter must not invent records from a claimed real source. Demo seeding is an
explicit synthetic operation. Queries must use stored rows, never fabricated results.

## Gateway work

Each protected route must:

- parse the capability and agent request signature
- call `verifierd` or use a valid verification-cache entry
- recompute and match the agent public-key thumbprint
- reconstruct and verify the signed method-target-body envelope on every request
- compare verified task, purpose, action, resource, and expiry with the request
- apply route-specific row, field, and output limits
- reject nonce replay
- emit an audit receipt
- return a generic authorization error

Do not put PolicyIR or private policy explanations in Next.js middleware, browser code,
or agent SDK responses.

## Console work

The console needs operational screens for:

- scenario and dataset selection
- task and purpose entry
- capability request status
- proof verification result
- reconciliation query execution
- cached request state
- denial and tamper demonstrations
- audit receipt inspection
- source provenance and benchmark summary

Display public claims only. Do not render PolicyIR, graph paths, Merkle openings, caveat
secrets, or exact denial explanations.

## Test matrix

Every scenario needs these tests:

| Category | Required case |
| --- | --- |
| Import | Selected source or synthetic seed imports deterministically |
| Import | Missing or malformed required field fails |
| Import | Checksum mismatch fails |
| Policy | Allowed relationship and caveat path matches SpiceDB |
| Policy | Missing relation denies |
| Policy | False or missing caveat denies |
| Proof | Valid capability verifies |
| Proof | Altered policy hash fails |
| Proof | Altered snapshot root fails |
| Scope | Wrong task, purpose, action, or resource fails |
| Key binding | Different agent key fails |
| Time | Expired capability fails |
| Replay | Reused request nonce fails |
| Independence | Verification works with `razield` stopped |
| Cache | Reuse stays within proven scope |
| Audit | Receipt contains approved public identifiers only |
| Privacy | Agent and logs contain no witness material |

## Acceptance evidence

Create a local evidence bundle containing:

- sanitized source manifest
- import summary and checksums
- policy and snapshot record identifiers
- capability public claims
- verifier receipt
- allowed and denied request receipts
- tamper-test results
- benchmark measurements
- software versions
- exact commands used

Do not put raw data, private relationships, openings, caveat secrets, or signing keys in
the bundle.

## Effort classification

### Small scenario

Use this band for an existing data shape and current PolicyIR nodes.

Expected work after source access:

- domain and policy mapping: 0.5 to 1 day
- data adapter and validation: 1 to 2 days
- gateway and console: 0.5 to 1 day
- tests and acceptance evidence: 1 day

Total planning range: 2 to 4 engineering days.

### Medium scenario

Use this band for a new source format, several resource types, or supported caveat
combinations.

Expected work:

- domain and threat analysis: 1 to 2 days
- policy and equivalence tests: 1 to 3 days
- data adapter: 2 to 4 days
- gateway and console: 1 to 3 days
- tests, review, and acceptance evidence: 2 to 3 days

Total planning range: 1 to 2 engineering weeks.

### Research change

This is not scenario authoring. Examples include exclusion, non-membership, recursive
permissions, new proof values, and private PolicyIR. Create an ADR and benchmark plan
before code work.

Data access, legal review, and source cleaning sit outside these engineering ranges.

## Exit gate

A scenario is complete only after:

- its descriptor and source manifest validate
- PolicyIR output is stable and approved
- SpiceDB equivalence tests pass
- the data adapter reproduces the recorded import summary
- proof creation and independent verification pass
- allowed, denied, tampered, expired, replayed, and out-of-scope cases behave correctly
- the verifier works with the prover stopped
- no proof guest or core commitment format changed
- demo data classification and evidence are complete, with real-data claims only
  after the separate real-data acceptance gate
- the current baton names the next scenario or closes scenario work
