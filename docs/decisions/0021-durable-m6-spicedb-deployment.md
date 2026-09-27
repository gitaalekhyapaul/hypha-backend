# ADR-0021: Durable M6 SpiceDB Deployment

- Status: accepted
- Date: 2026-09-27
- Baton: B-0066

## Decision

Use PostgreSQL 17.10 for the live M6 SpiceDB datastore. The maintainer approved
`postgres@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f`.
SpiceDB stays at its existing pinned version. Run its supported migrations before
starting the server. Use a dedicated project-local database volume and internal
database network, with no host database port.

The live stack is a separate Compose project from M5 and the partial M6 preview.
It starts with empty policy, registration, dataset, request, and relationship state.
It does not reconstruct or import the memory-backed preview. No existing database,
volume, authority key, or schema is replaced.

## Reason

The preview retains policy installation records but loses its memory-backed SpiceDB
state on restart. The composition drift check then correctly blocks publication.
The live demo needs both state stores to survive ordinary container replacement.

## Boundaries

PostgreSQL holds SpiceDB schema, relationships, and revision history, not snapshot
salts or proof witnesses. Existing owner-only snapshot and request volumes remain.
The guest, canonical commitments, approvals, signatures, and proof mode do not change.
No raw relationship data or database credential enters agent responses.

Retained volumes are required for restart recovery. `down --volumes` destroys them
and is not a restart command. This is a single-VM demonstration deployment, not a
high-availability or backup guarantee. Backup and restore must cover the database,
Redis, registry, owner files, and authority keys together.

## Validation

Check the rendered role/network/volume boundaries, run the pinned migration, and
confirm schema and relationships survive a database and SpiceDB restart in an
isolated test project. Full live agent proof validation remains a VM gate.
