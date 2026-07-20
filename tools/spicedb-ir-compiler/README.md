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

## Version

```bash
./scripts/dev go run ./tools/spicedb-ir-compiler/cmd/spicedb-ir-compiler version
```
