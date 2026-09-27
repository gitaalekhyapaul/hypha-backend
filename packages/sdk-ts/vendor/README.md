# Local SDK Release Candidate

The tarball is built from the sibling `raziel-sdk-ts` repository, package
`@gitaalekhyapaul/raziel-sdk-ts@0.1.0`. It is not an npm publication.

Current candidate: `gitaalekhyapaul-raziel-sdk-ts-0.1.0-b0066.tgz`.

SHA-256: `556abe87978429a1542bfbbe8d81edd48a8be92ceeb07b1731eb18e8233ce572`

B-0066 adds explicit resume, exact proposal signing and retained submission, bounded
read-only polling, and dual-signature protected queries. It uses the already pinned
`@bufbuild/protobuf@2.12.1` runtime. The package contains public generated DTOs only.

The earlier `raziel-sdk-ts-0.1.0.tgz` B-0064 candidate is retained but is no longer
selected by the workspace dependency.

This artifact keeps a backend clone runnable without sibling source imports or a
Git submodule. Its public files contain no private registry, witness, or credentials.
The workspace SDK is a compatibility re-export, not a second implementation.

Build replacements in the SDK repository with `pnpm build` and `pnpm pack`, then
update this artifact, its checksum, and the lockfile together. Public npm publication
is deferred by the maintainer. This candidate supports provisional VM testing.
