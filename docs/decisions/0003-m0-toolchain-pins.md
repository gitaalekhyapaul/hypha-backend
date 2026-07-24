# ADR-0003: M0 Toolchain Pins

- Status: accepted for M0, amended for M3
- Date: 2026-07-18
- Amended: 2026-07-24
- Applies to: repository scaffold, local development, and CI

## Context

M0 needs repeatable TypeScript, Rust, Go, Protobuf, SpiceDB, and SP1 builds. The current
Apple Silicon host has Docker but lacks several language tools. Its Node and pnpm
versions do not match the selected baseline.

Global host installation would alter an existing Node setup and create more
machine-specific state. The architecture already requires Linux containers with native
Apple Silicon support.

## Decision

Use a container-first toolchain. Docker is the only required host runtime for the M0
validation path. Keep human-readable version files for developers who choose matching
host tools.

| Component | Pin | Source |
| --- | --- | --- |
| Node.js | `24.18.0` | [Node.js LTS release](https://nodejs.org/en/blog/release/v24.18.0) |
| pnpm | `11.14.0` | [pnpm release](https://github.com/pnpm/pnpm/releases/tag/v11.14.0) |
| Next.js | `16.2.10` | [Next.js package release](https://www.npmjs.com/package/next/v/16.2.10) |
| React | `19.2.7` | [React package release](https://www.npmjs.com/package/react/v/19.2.7) |
| TypeScript | `6.0.3` | [TypeScript package release](https://www.npmjs.com/package/typescript/v/6.0.3) |
| Biome | `2.5.4` | [Biome release](https://github.com/biomejs/biome/releases/tag/%40biomejs%2Fbiome%402.5.4) |
| Vitest | `4.1.10` | [Vitest release](https://github.com/vitest-dev/vitest/releases/tag/v4.1.10) |
| Rust | `1.97.1` | [Rust release notes](https://doc.rust-lang.org/stable/releases.html#version-1971-2026-07-16) |
| Go | `1.26.5` | [Go release history](https://go.dev/doc/devel/release#go1.26.5) |
| Buf | `1.72.0` | [Buf release](https://github.com/bufbuild/buf/releases/tag/v1.72.0) |
| Protobuf compiler | `3.21.12`, Debian `3.21.12-3+deb12u1` | [Debian Bookworm package](https://packages.debian.org/bookworm/protobuf-compiler) |
| Protobuf development files | Debian `3.21.12-3+deb12u1` | [Debian Bookworm package](https://packages.debian.org/bookworm/libprotobuf-dev) |
| Protobuf-ES generator and runtime | `2.12.1`, revision `1` | [Protobuf-ES release](https://github.com/bufbuild/protobuf-es/releases/tag/v2.12.1) |
| Prost generator | `0.5.0`, revision `1` | [Prost plugin](https://buf.build/community/neoeinstein-prost) |
| Tonic generator | `0.5.0`, revision `1` | [Tonic plugin](https://buf.build/community/neoeinstein-tonic) |
| SpiceDB | `1.54.0` | [SpiceDB release](https://github.com/authzed/spicedb/releases/tag/v1.54.0) |
| SP1 | `6.3.1` | [SP1 release](https://github.com/succinctlabs/sp1/releases/tag/v6.3.1) |

TypeScript `7.0.2` is current but not selected for M0. The workspace keeps `6.0.3`
until the pinned Next.js build and type checks establish compatibility. A later upgrade
needs its own tested dependency change.

Pin locations:

- `.node-version` records the Node version.
- The root `package.json` records `engines.node` and `packageManager`.
- JavaScript package manifests use exact package versions and commit `pnpm-lock.yaml`.
- `rust-toolchain.toml` records Rust, rustfmt, and Clippy.
- Cargo manifests use exact SP1 crate versions and commit `Cargo.lock`.
- Go manifests record Go `1.26.5`; the shared version file records SpiceDB `v1.54.0`.
- `tools/toolchain-versions.env` records shared versions and image digests.
- The same file pins the Debian Protobuf compiler and development packages. The image
  and bootstrap checks require the matching `libprotoc` version and
  `google/protobuf/empty.proto`.
- `buf.gen.yaml` records exact remote generator versions, revisions, and output paths.
  This workspace has no Buf module or plugin dependencies that produce `buf.lock`.
- Dockerfiles and Compose use versioned tags plus OCI manifest digests.
- The SP1 `cargo-prove` release assets use exact versions and published SHA-256
  digests for Linux AMD64 and ARM64. The image verifies the digest before extraction.
  The bootstrap check matches build commit `8252c29`, which is the identifier printed
  by the `6.3.1` binary.

The selected Node, Rust, Go, Buf, and SpiceDB image indexes contain native
`linux/arm64` and `linux/amd64` manifests. Their index digests were checked with
`docker buildx imagetools inspect` on 2026-07-18.

Official release channels and package registries were checked again on 2026-07-18
before any dependency download.

The M3 amendment adds Debian Bookworm `protobuf-compiler=3.21.12-3+deb12u1`.
`sp1-prover-types 6.3.1` invokes `protoc` from its build script and imports a
well-known Protobuf type. Debian ships that type in
`libprotobuf-dev=3.21.12-3+deb12u1`. The maintainer approved both packages and
toolchain rebuilds on 2026-07-24.

## Bootstrap and approval boundary

No global host tool installation is part of the baseline. After the M0 container files
exist, these commands form the approved download boundary:

```bash
./scripts/compose build toolchain
./scripts/compose run --rm toolchain ./scripts/update-locks
```

The build downloads pinned base images, pnpm, and SP1. The lock update downloads
workspace packages and creates generated source through checked-in configuration. It
creates `pnpm-lock.yaml` and `Cargo.lock`. After those files are reviewed and
committed, clean checkouts use:

```bash
./scripts/compose run --rm toolchain ./scripts/bootstrap
```

User approval is required before either command runs. Without approval, agents can
write and review manifests but cannot claim build, generation, or test gates.

## Compatibility gates

- Build host Rust shells with Rust `1.97.1`.
- Declare Rust `1.91.1` as the guest minimum. SP1 `6.3.1` builds the guest with its
  bundled `rustc 1.94.0-dev`, independent from the host toolchain.
- Compile one minimal schema through the SpiceDB `v1.54.0` compiler package.
- Run Buf generation twice and compare generated output.
- Generate TypeScript under `packages/proto-ts/src/gen` and Rust under `gen/rust`.
- Keep SP1 proving local. Do not configure a hosted prover or network key.

## Consequences

- A developer with Docker can run the pinned toolchain without replacing host Node,
  Rust, or Go versions.
- First bootstrap needs network access and several large image downloads.
- CPU proof work on Apple Silicon has no CUDA acceleration.
- IDEs can use host tools, but container validation remains authoritative for M0.
