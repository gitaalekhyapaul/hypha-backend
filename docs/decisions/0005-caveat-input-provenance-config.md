# ADR-0005: Caveat Input Provenance Configuration

- Status: accepted
- Date: 2026-07-19
- Milestone: M1

## Context

The SpiceDB compiler records each caveat parameter name and CEL type. Its compiled
form does not state whether a runtime value comes from request context, stored
relationship context, or a capability claim. SpiceDB resolves caveats from a merged
context during an online permission check.

Raziel keeps those sources separate. A request value must not replace a committed
relationship value or a signed capability claim. PolicyIR requires a source on every
caveat parameter and input reference.

PolicyIR carries proof-boundary limits. Production values remain unset until M3 cycle
measurements, but compiler fixtures need explicit test values.

## Decision

The Go compiler adapter requires a versioned JSON configuration. It contains explicit
PolicyIR limits and a source binding for every compiled caveat parameter.

```json
{
  "config_version": 1,
  "limits": {
    "max_expression_nodes": 64,
    "max_caveat_nodes": 64,
    "max_path_depth": 4,
    "max_relationship_witnesses": 32,
    "max_merkle_depth": 32,
    "max_string_bytes": 256,
    "max_context_entries": 16,
    "max_total_private_input_bytes": 4096
  },
  "caveats": {
    "reconcile_scope": {
      "task_id": "CAPABILITY_CLAIM",
      "approved_amount": "RELATIONSHIP_CONTEXT",
      "requested_amount": "REQUEST_CONTEXT"
    }
  }
}
```

The numeric values above are fixture values, not production ceilings.

The accepted source labels are `REQUEST_CONTEXT`, `RELATIONSHIP_CONTEXT`, and
`CAPABILITY_CLAIM`. Parameter names and scalar types still come from the compiled
SpiceDB definition.

Configuration decoding rejects unknown fields, duplicate keys, trailing JSON values,
an unsupported config version, zero limits, unknown source labels, missing bindings,
extra caveats, and extra parameters. Schema compilation fails before PolicyIR output
on any mismatch.

The configuration contains no caveat values. Stored relationship context remains in
the private relationship snapshot. Request values remain in the signed request.
Capability values remain in signed capability claims.

The mapping becomes typed PolicyIR metadata. It is verifier-visible through the
approved policy registry but does not enter the agent-visible capability.

## Consequences

- The proof evaluator reads each caveat value from one declared trust source.
- Scenario configuration must change when a caveat parameter is added or removed.
- Test policies can declare bounded fixture limits before M3 selects production values.
- SpiceDB and PolicyIR caveat equivalence tests can supply the same values through
  their respective context models.

## Rejected Options

### Infer sources from parameter names

Naming conventions are not part of SpiceDB semantics and can bind a value to the wrong
trust source.

### Treat all caveat inputs as request context

This lets request data replace committed relationship context or capability claims.

### Put runtime values in compiler configuration

Static values would mix public policy metadata with private witness data and request
state.
