# SDK 0.1.0 Publication Attempt

- Date UTC: 2026-10-01
- Package: `@gitaalekhyapaul/raziel-sdk-ts@0.1.0`
- Outcome: public publication verified. The `latest` tag points to `0.1.0`.
- Registry publication UTC: `2026-10-01T13:05:51.356Z`
- Final registry check UTC: `2026-10-01T13:13:16Z`
- Authorization: maintainer requested one public publication attempt and completed
  the interactive npm account approval.
- SDK source base: `e7f1cd6b354f236f0323d9d19aa4a431c5dfa470`
- Protocol baseline: B-0066. No runtime code, version, or dependency changed.

## Package Evidence

The release reused the validated B-0066 compiled candidate. Only README and release
metadata changed. The README now describes implemented polling/protected queries
and the six one-shot access-request methods. All compiled files, package.json, and
LICENSE stayed byte-identical. No install or runtime rebuild ran.

- Original candidate SHA-256:
  `556abe87978429a1542bfbbe8d81edd48a8be92ceeb07b1731eb18e8233ce572`
- Submitted archive SHA-256:
  `a4bfd73963796ad7f74993e6f6223f7d1b2d2dd7c228117c0e025814782980c3`
- Submitted archive SHA-1: `4ff6453155b6a522e8bbf96f3d491564fdff4b04`
- Submitted integrity:
  `sha512-b/iymgYncBr57U+4D8ymzGyZiKo744r8Eo/2qOvOA/6TMdwO9LmulDRGYzIxopJ1gLJmEbWEP0eJvBvVA00dzw==`
- Public files: 40. No credentials, tests, private policy, relationship data, or witness.
- Required runtime: Node.js `>=24.18.0`
- Runtime dependency: `@bufbuild/protobuf@2.12.1`

## Validation

| Check | Result | Evidence |
| --- | --- | --- |
| Release archive | PASS | `npm pack --ignore-scripts`; reviewed 40-file inventory |
| Runtime preservation | PASS | Recursive comparison found only README and compatibility metadata changes |
| One public publish | PASS | `npm publish ... --access public --ignore-scripts` exited 0 after account approval |
| Exact version endpoint | PASS | Unauthenticated registry request returns `0.1.0` with matching integrity/SHA-1 |
| Public archive | PASS | Unauthenticated download matches submitted archive byte-for-byte |
| Maintainer staging queue | PASS | `npm stage list @gitaalekhyapaul/raziel-sdk-ts --json` returns `[]` |
| Ordinary package lookup | PASS | Early 404 resolved. `npm view` and anonymous package metadata serve `0.1.0` with matching integrity |
| Default distribution tag | PASS | `npm dist-tag ls` and anonymous metadata report `latest: 0.1.0`; no tag mutation ran |
| Package website | NOT AVAILABLE | Last browser refresh still displayed the temporary `0.0.0-stage` holding page |
| Full live M6/M7 proof flow | SKIP | Publication does not establish the connected VM demo gate |

The CLI reported processing after upload. The package website's holding version is
not the submitted SDK version. The registry later resolved normally without another
upload or tag repair. No second publication, tag change, approval bypass, or consumer
dependency migration ran. Registry metadata was verified without npm credentials.

## Next Action

Third-party consumers can install `@gitaalekhyapaul/raziel-sdk-ts@0.1.0` with Node.js
24.18.0 or newer. Existing backend and agent candidate pins stay unchanged until a
separate dependency migration. Live VM proof validation remains separate and pending.
Do not republish or change tags to repair a stale website display.

[npm package page](https://www.npmjs.com/package/@gitaalekhyapaul/raziel-sdk-ts)

## 2026-10-08 Checkpoint Follow-Up

Anonymous registry metadata still resolves `latest` to `0.1.0`. The public archive's
integrity, SHA-1, and recorded SHA-256 pass. SDK README and compatibility metadata
match the published archive byte-for-byte. These source changes are committed and
pushed at `6239fad9d02b1047806b3c449eb593ea7cd9f119`.
No new SDK runtime change, publication, version bump, or tag mutation was required.
The agents consume public `0.1.0` at their pushed checkpoint `bb66ee3`.
