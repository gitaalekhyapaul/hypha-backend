---
name: raziel-scenario
description: Use when planning, adding, or reviewing a Raziel authorization scenario, including its real-data source, SpiceDB policy mapping, domain adapter, Next.js routes, tests, evidence, and effort classification.
---

# Raziel Scenario

Treat a scenario as domain configuration and adapters around the shared proof system.
Do not fork or edit the SP1 guest during normal scenario work.

## Read first

1. `AGENTS.md`
2. `docs/agent/CURRENT-BATON.md`
3. `ARCHITECTURE.md`
4. `docs/spec/SCENARIO-AUTHORING.md`
5. `examples/reconciliation/` after it exists

## Workflow

1. Confirm that payment reconciliation passed M6 before starting a second scenario.
2. Acquire a baton with an exclusive scenario file scope.
3. Copy `references/scenario-template.md` into a planning note.
4. Name the real source and record provenance, usage basis, checksum, schema, and data
   handling limits.
5. Map principals, resources, actions, tasks, purposes, caveats, and outputs.
6. Keep discovery relevance separate from authorization policy.
7. Write SpiceDB policy only inside the supported PolicyIR subset.
8. Compile PolicyIR through the Go adapter. Do not hand-write scenario PolicyIR.
9. Add deterministic ingestion, gateway, console, audit, and test work.
10. Run the scenario test matrix and create a sanitized evidence bundle.
11. Confirm that no guest, public proof value, hash format, or capability format changed.
12. Close the baton with effort, evidence, risks, and one next action.

## Stop conditions

Stop scenario work and draft a decision record for:

- a new PolicyIR expression node
- exclusion or non-membership
- recursive or unbounded graph traversal
- a new public proof value
- a new relationship leaf or canonical hash format
- private PolicyIR
- an agent-visible policy evaluator

These requests change proof semantics.

## Output

Return or record:

- scenario slug and effort band
- source status
- domain mapping
- supported policy mapping
- files to add or edit
- test and evidence plan
- proof-system changes, which must be `None` for normal scenario work
- blockers and exact next action
