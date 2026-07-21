# ADR-0004: PolicyIR Wire and Evaluator Boundary

- Status: accepted
- Date: 2026-07-19
- Milestone: M1
- Superseded in part: ADR-0007 replaces the Boolean missing-input propagation rule.

## Context

The technical specification fixes the supported permission nodes, caveat nodes, and
proof-boundary limits. It does not define the declaration records needed to resolve
object, relation, permission, and caveat references. It leaves relationship data
access outside the PolicyIR evaluator contract.

The wire contract must support the Go compiler adapter, the Rust host evaluator, and
the SP1 guest without placing private graph data in PolicyIR.

## Decision

Use a dedicated `raziel.v1.PolicyProgram` Protobuf message. A program declares object
types and relations, typed permissions, typed caveats, and explicit limits.

Identify permissions by `(object_type, name)`. Each relation declares accepted object
types plus optional subject-relation and caveat constraints. Identify caveat inputs by
`(source, name)` and declare their scalar types.

Keep canonical commitment encoding outside Protobuf. `PolicyRecord` contains the typed
program, but capabilities and public proof values still contain only `policy_hash`.
Replacing the M0 `bytes policy_ir` placeholder is an intentional pre-release generated
API break. No released consumer uses the placeholder.

The Rust evaluator reads relationship facts through a read-only resolver. It returns
only an authorization Boolean or a typed error. The resolver and evaluator do not emit
satisfying paths, Merkle openings, stored caveat values, or witness traces.

`RelationRef` matches direct uncaveated tuples. `SubjectRelationRef` makes exactly one
declared userset hop. `Arrow` requires one uncaveated direct target type and resolves a
permission on that type. `CaveatedRelation` matches direct tuples carrying the exact
named caveat. Caveated usersets and caveated arrow sources fail validation.

Relationship caveat context stays attached to its tuple. Mistyped context returns a
typed evaluation error. Boolean composites evaluate every child. Missing input follows
the order-independent CEL absorption rules in ADR-0007. Every other error propagates.

Set-like declarations and commutative expression children use strict structural order.
M2 will document the matching cross-language byte comparator with the canonical
encoding vectors.

Generate Prost messages and Tonic service stubs into separate directories. The shared
protocol crate compiles message contracts without linking network-service dependencies
into the proof-critical PolicyIR crate.

The program carries declared limits. Rust validation compares every declared limit
with implementation ceilings. Final production ceilings remain unset until M3 cycle
measurement.

The relationship resolver reports witness count, Merkle depth, and private-input byte
usage. The witness boundary must derive these metrics from validated witness material.
PolicyIR does not calculate Merkle encodings.

## Consequences

- The compiler can fail on unresolved or unsupported references before evaluation.
- Host and guest code can share one typed PolicyIR model and pure evaluator.
- Relationship storage, witness construction, and Merkle verification remain owned by
  their dedicated crates.
- Raw Protobuf bytes remain unsuitable as commitment inputs.
- Strict unknown-field checks remain part of the canonical conversion boundary in M2.

## Rejected Options

### Store PolicyIR as opaque bytes

Opaque bytes prevent generated clients and Rust conversion code from checking the
approved node set.

### Put relationship tuples in PolicyIR

This merges public policy with private snapshot state and breaks the repository trust
boundary.

### Return evaluation paths

Paths expose branch choices and relationship structure. The baseline evaluator returns
no explanation artifact.
