# Agent Public SDK Consumption

- Date UTC: 2026-10-01
- Check UTC: 2026-10-01T13:26:15Z
- Outcome: local agent consumption of public npm SDK `0.1.0` verified.
- Agent source base: `54aa8090d1b2ff20d4bb545a71b7745fc9025efd`
- Authorization: maintainer requested registry consumption after SDK publication.
- Git state: migration is uncommitted and unpushed. No VM deployment ran.

## Changes

The agent manifest pins `@gitaalekhyapaul/raziel-sdk-ts` to exact version `0.1.0`.
pnpm regenerated the lockfile with the public archive integrity:

`sha512-b/iymgYncBr57U+4D8ymzGyZiKo744r8Eo/2qOvOA/6TMdwO9LmulDRGYzIxopJ1gLJmEbWEP0eJvBvVA00dzw==`

The Docker build installs the registry release without credentials. It no longer
copies the vendor directory. The old tarball remains historical evidence, not a
build dependency. Package caches and the vendor directory stay out of the image
build context. Other dependency versions and agent runtime source are unchanged.

pnpm 11 generated an age-check exception for this exact verified SDK version.
The Docker build includes that setting. No global age-check override or wildcard
exception was added.

## Validation

| Check | Result | Evidence |
| --- | --- | --- |
| Frozen install | PASS | Pinned Node/pnpm Docker toolchain installed the public SDK with no publishing credentials |
| Agent typecheck | PASS | `pnpm typecheck` |
| Agent regression | PASS | `pnpm test`: 13 passed, no failed or skipped checks |
| Formatting | PASS | `pnpm format:check`: 21 files, no fixes |
| Production build | PASS | Frozen production install fetched SDK `0.1.0`; separate local image built |
| Image import | PASS | Networkless, read-only process imports SDK ESM, observes `0.1.0` public-release metadata, and finds no vendor directory |
| Connected VM proof | SKIP | This migration does not replace the pending real proof-backed SDK acceptance gate |

Test image: `raziel-demo-agents:b0067-registry-check`. No running preview or VM
container was recreated. The backend compatibility shim retains its earlier pin.

## Release Order

Validate and commit SDK source, publish a new immutable version, then update the
agent's exact dependency and lockfile. Run the agent checks and commit that update.
VM pulls then rebuild the agent image. Do not follow `latest` at runtime or install
directly from the SDK source checkout. No new publication is needed for agent-only
changes or reuse of an unchanged SDK release.

## 2026-10-08 Checkpoint Follow-Up

The migration is now committed and pushed at
`bb66ee3c2e9ea546661fd9e1134a5c956dedec24`. All 13 agent tests, typecheck,
21-file formatting check, image build, and networkless public SDK import pass
again. The October 1 uncommitted state above remains historical evidence.
No VM deployment or real proof run occurred during this release checkpoint.

The maintainer then requested removal of the obsolete SDK archive from the agent
checkout. That cleanup is committed and pushed at
`84f9eb135532cc52b89bbfbf20d77c1f85b12b7d`. The original archive remains recoverable
in Git history. A frozen offline install passes with networking disabled and the
unchanged lockfile. All 13 agent tests and the image build still pass.
