# ADR-0023: Operator-Managed Agent Roles

- Status: accepted for the bounded M7 demo
- Date: 2026-10-18
- Baton: B-0068
- Approval: maintainer approved the two-turn role-management plan with "Go ahead then"
  and "Go ahead please". Existing capability revocation is explicitly unchanged.

## Decision

Separate relationship administration from capability issuance for opt-in policy
versions. The operator uses Agents to view, add, and remove supported direct roles
and team memberships. The existing owner publication service performs each operation
under its exclusive writer lock. No agent token can invoke this interface.

A currently approved, installed version with no relationships can enter sticky
operator-managed mode. The owner persists that selection before any role write.
Existing versions keep their legacy automatic provisioning behavior. There is no
fallback to automatic provisioning after a version enters managed mode. A corrupt
mode record is an infrastructure failure, not permission to recreate grants.

The first editor supports direct viewer, approver, auditor, owner, and team membership
within the existing direct, team, alternative, and dual-control families. Caveated and
organization-delegated families remain on the existing path. Unsupported selections
fail closed. A role is valid only if the selected approved policy uses it.

Direct grants bind an exact dataset, version namespace, and registered agent key.
Team grants link that dataset to a version-local team and add the registered key as
a member. Membership removal deletes only that agent's membership, not the shared
dataset/team link. It affects all datasets using that team in that policy version.

Managed request preparation reads the current graph at one exact SpiceDB revision.
It never creates missing memberships. A retained attempt reuses its original
revision. A new request is needed to observe later role changes. Missing authorization
cannot produce a capability and is rejected before expensive Plonk work. This is
not a cryptographic DENY proof.

## Trust And Privacy

Only the operator sees role metadata. The HTTP facade resolves the registered key
from the path's agent ID. Caller-supplied key identities are rejected. Strict bounded
requests pass through existing private owner IPC. Responses include supported role
names and the selected agent's assignments, never a complete graph or PolicyIR.

The owner authenticates the selected approved package and live schema composition.
Existing relationship and transport bounds apply. No new package, signature rule,
public proof value, guest, SDK endpoint, or verifier dependency is introduced.

## Snapshot And Revocation Semantics

Role edits govern fresh snapshots. Removing membership after snapshot creation does
not invalidate that snapshot's proof or an issued capability. Current expiry, API
token revocation, registry checks, and verification-cache behavior stay unchanged.
Immediate role-change revocation requires a separate decision and implementation.

## Compatibility And Rollback

Tag `v1` preserves backend commit `1a297cb4fea1f1a1a736126606b1a1409c323c12`.
Use a fresh policy version for this demo. No migration or deletion touches legacy
state. A code rollback does not undo persistent role edits or managed-mode records.
Do not use legacy binaries to issue new capabilities against the managed demo version.
Existing approved guest material, keys, and retained capabilities remain unchanged.

## Acceptance

- An empty fresh version can enter managed mode. Existing nonempty versions cannot.
- Operator add/remove is idempotent and unsupported inputs fail without graph writes.
- Missing role, add role, and remove role produce reject/allow/reject on fresh inputs.
- Managed issuance never repairs a missing tuple. Exact retries retain their revision.
- Legacy provisioning and operator authentication/CSRF checks continue to pass.
- One real VM success remains necessary. Local fixtures do not establish that gate.
