# ADR-0007: CEL Boolean Error Absorption

- Status: accepted
- Date: 2026-07-21
- Milestone: M1
- Supersedes: the Boolean all-error-propagation clause in ADR-0004

## Context

ADR-0004 requires caveat Boolean composites to evaluate every child and propagate
every error. SpiceDB follows CEL semantics. CEL can return a definite Boolean when
one operand determines the result despite another operand error.

The conflicting cases are `false && error`, which CEL evaluates as `false`, and
`true || error`, which CEL evaluates as `true`. Raziel currently returns a typed error
for both. This breaks the M1 SpiceDB equivalence gate for missing caveat inputs.

## Decision

Keep full, order-independent child evaluation. Combine Boolean child results with
CEL-compatible missing-input absorption:

- `And` returns `false` when any child is false.
- `And` returns `MissingCaveatInput` when no child is false and at least one child is
  missing.
- `And` returns `true` when every child is true.
- `Or` returns `true` when any child is true.
- `Or` returns `MissingCaveatInput` when no child is true and at least one child is
  missing.
- `Or` returns `false` when every child is false.

Only `MissingCaveatInput` participates in absorption. Type mismatches, malformed
relationship data, resolver failures, and evaluation-budget failures still propagate.
These errors represent invalid proof inputs or failed proof-boundary checks, not CEL
partial evaluation.

This decision supersedes only ADR-0004's Boolean all-error-propagation clause. All
other ADR-0004 decisions remain accepted.

## Trust-Boundary Effect

The evaluator still reads each caveat parameter from its declared provenance source.
Missing request context cannot replace stored relationship context or capability
claims. A missing value becomes irrelevant only when another Boolean result already
determines the expression.

## Public and Private Data Effect

No wire field, public proof value, capability field, PolicyIR node, or context source
changes. No private caveat value enters an error or public result. The interpreter
semantics are public proof-system material and must match in the host, guest, and
verifier implementation.

## Migration Cost

M1 has no published policy registry, proof, or external consumer. The change needs no
data migration. Later guest and verifier code must use this decision before M3 proof
vectors become stable.

## Test Changes

- Add Rust unit tests for `false && missing`, `true && missing`, `true || missing`, and
  `false || missing`.
- Add live SpiceDB-versus-Rust cases for both absorbed and unresolved missing inputs.
- Keep typed failures for unresolved missing inputs and every non-missing error.

## Rejected Options

### Keep all-error propagation

This preserves ADR-0004 but disagrees with SpiceDB for supported CEL expressions.

### Short-circuit child evaluation

This makes budget use and error discovery depend on child order. PolicyIR Boolean
children are commutative and structurally ordered.

### Absorb every evaluator error

This can hide malformed witness data, type violations, or proof-boundary limit
failures. Only missing CEL input has partial-evaluation meaning.
