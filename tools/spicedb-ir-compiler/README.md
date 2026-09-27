# SpiceDB PolicyIR Compiler

This build-time Go CLI runs the pinned SpiceDB schema compiler and translates its
compiled Protobuf records into Raziel's bounded PolicyIR subset.

## Compile

Run the command through the repository toolchain:

```bash
./scripts/dev go run ./tools/spicedb-ir-compiler/cmd/spicedb-ir-compiler compile \
  --schema tools/spicedb-ir-compiler/internal/adapter/testdata/supported.zed \
  --config tools/spicedb-ir-compiler/internal/adapter/testdata/supported.json \
  --compiler-version 1.54.0 \
  --out /tmp/raziel-policy.pb \
  --diagnostic-out /tmp/raziel-policy.json
```

The command requires an explicit compiler version. It rejects any value other than the
repository pin. Unsupported SpiceDB or CEL constructs return a typed, source-aware
error and produce no new output.

`--out` receives deterministic PolicyIR Protobuf bytes. `--diagnostic-out` receives
the same message as stable, indented Protobuf JSON for human review. Output paths must
differ from each other and from both input paths. Both files use mode `0600`.

The JSON config assigns each caveat parameter to a trust source and declares explicit
PolicyIR limits. ADR-0005 defines that contract. The files under `internal/adapter/testdata`
use fixture limits and are not production policy configuration.

`source_schema_digest` is SHA-256 over the `.zed` file's exact bytes. ADR-0006 defines
the byte rule. The compiler output contains no relationship tuples or runtime caveat
values.

## Compose Retained Versions

The offline `compose` command checks multiple owner-supplied policy packages. It
recompiles each exact source/config pair against its expected binary PolicyIR,
combines independent namespaces, and checks the compiled aggregate against their
union. It does not approve a policy or install a schema in SpiceDB.

Example private manifest:

```json
{
  "format_version": 1,
  "packages": [
    {
      "version_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "schema_path": "schema.zed",
      "config_path": "compiler.json",
      "expected_policy_ir_path": "policy.pb"
    }
  ]
}
```

Paths resolve relative to the manifest. Use the actual owner-allocated version ID,
not the illustrative ID above. One optional legacy entry replaces `version_id`
with `"legacy": true` and supplies its exact retained inputs.

```bash
./scripts/dev go run ./tools/spicedb-ir-compiler/cmd/spicedb-ir-compiler compose \
  --manifest /tmp/raziel-packages/manifest.json \
  --out /tmp/raziel-packages/composed.zed \
  --reflection-out /tmp/raziel-packages/reflection.json \
  --compiler-version 1.54.0
```

The command accepts at most 32 namespaced versions plus one legacy package. Only
the empty `agent` definition can be shared. Schema and reflection outputs are
each bounded to 1 MiB, with a combined 2 MiB ceiling. Both files use mode `0600`.
Reflection contains private compiled structures and source digests. It must never
be returned to an agent or committed as a public artifact.

Composition uses the pinned SpiceDB parser's definition spans and compiler
structures. Semantic comparison excludes source locations and documentation but
retains checked caveat expressions. Owner publication still needs signed registry
approval checks, retained-package validation, and trusted schema installation.

The owner compares a SpiceDB `ReadSchema` response with the expected composition
using compiled structures, not source-byte equality:

```bash
spicedb-ir-compiler compare-schema \
  --expected composed.zed \
  --actual reflected.zed \
  --compiler-version 1.54.0
```

Success prints `schema=equivalent`. Definition ordering, documentation, and source
positions can differ. Changed relations, expressions, namespaces, or extra definitions
fail. Both inputs remain private and are bounded to 1 MiB each. This command does
not alter either file or change the exact-source digest in a retained policy package.

## Version

```bash
./scripts/dev go run ./tools/spicedb-ir-compiler/cmd/spicedb-ir-compiler version
```
