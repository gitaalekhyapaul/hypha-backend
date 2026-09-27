# Local SDK Release Candidate

The tarball is built from the sibling `raziel-sdk-ts` repository, package
`@gitaalekhyapaul/raziel-sdk-ts@0.1.0`. It is not an npm publication.

Current candidate: `gitaalekhyapaul-raziel-sdk-ts-0.1.0-b0065.tgz`.

SHA-256: `0ea89506eb120abd09befe9b45dd3b882374adc3926a00d43c95e2848f9f495e`

B-0065 adds signed access-request create/list/get/cancel methods and public generated
DTOs. It uses the already pinned `@bufbuild/protobuf@2.12.1` runtime. Polling, final
issuance submission, and the protected-query facade remain B-0066 work.

The earlier `raziel-sdk-ts-0.1.0.tgz` B-0064 candidate is retained but is no longer
selected by the workspace dependency.

This artifact keeps a backend clone runnable without sibling source imports or a
Git submodule. Its public files contain no private registry, witness, or credentials.
The workspace SDK is a compatibility re-export, not a second implementation.

Build replacements in the SDK repository with `pnpm build` and `pnpm pack`, then
update this artifact, its checksum, and the lockfile together. B-0066 replaces the
local release candidate dependency with the reviewed public package.
