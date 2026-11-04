# B-0079 Console Brand Release

- Completed: 4 November 2026, 20:45 SGT.
- Authorization: User requested commit, push, VM pull, rebuild, and restart.
- Scope: Operator console, Agent A, Agent B, and the separate Beamer source.
- Delegation: GPT-6.1 Sol, xHigh, remote deployment. Parent checked all three
  loopback pages and their served asset hashes independently.

## Published Changes

| Repository | Commit | Result |
| --- | --- | --- |
| hypha-backend | `0044aeb1c7f65f4ad3d6d5e71bf953f9f6f7832e` | Signed commit, pushed; VM fast-forwarded |
| raziel-demo-agents | `44d5b0af940b755b1cdb6c9209a52757f2f59b92` | Signed commit, pushed; VM fast-forwarded |
| raziel-final-report | `132e50bc688a0240ef5537e49b62b748268cca6e` | Signed Beamer source commit, pushed; not a deployed VM checkout |
| raziel-sdk-ts | Unchanged | No SDK change or release |

Staged whitespace checks passed. Upstream font licence notices retain exact
bytes under scoped Git attributes. A staged text scan found no private-key
headers or recognized credential token patterns.

## Deployment Evidence

| Service | Update | Verified |
| --- | --- | --- |
| Operator | Recreated only `demo-console`; compiled updated bind-mounted source using existing `raziel-toolchain:m0` | Healthy, HTTP 200 on 3000; lantern and Public Sans hashes matched |
| Agent A | Recreated after the shared image build with `--pull=false` | Healthy, HTTP 200 on 3001; lantern and Public Sans hashes matched |
| Agent B | Recreated from the same rebuilt image | Healthy, HTTP 200 on 3002; lantern and Public Sans hashes matched |

Both agents use image
`sha256:fd2e9d8c46195d8dfacb208ebd12d4b3eb1155d6b0c38ebd81e41df7aaaf73be`.
The operator kept its original toolchain image. It was stopped before the
release after a JavaScript heap-limit exit. Normal recreation restored service
without changing runtime configuration. The generated VM `next-env.d.ts`
difference remained byte-identical through pull and recreation.

The two Compose projects' 27 non-UI containers retained their IDs, images,
start times, states, and mounts. The proof and cancellation queues had no active
or pending work at the restart checks. No new proof, protected query, state
reset, volume change, migration, tunnel, or credential change ran.

## Local Validation

- B-0078 recorded 212 operator tests, 18 agent tests, operator build, and desktop
  visual checks. The release reran three agent brand asset tests successfully.
- A native Mac operator test attempt could not start without the optional
  Rolldown native binding. No install was attempted. Existing pinned-container
  evidence and successful VM compilation and HTTP checks remain separate.
- Beamer: `make slides` passed with LuaLaTeX. Rendered all 16 widescreen
  pages. No overfull boxes or missing characters were reported. The original
  report source and exported PDF were not rebuilt or edited.
- The recording placeholder remains the last slide. No historical footage
  was included.

## Next Action

Review the three branded consoles and record the new synthetic walkthrough.
See the current baton and the report repository's speaker notes.
