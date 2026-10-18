# Agent B Revision 2 Failure Check

Status: runtime failure confirmed, exact internal cause not retained.

## Request

- VM backend commit: `92b5dd9`.
- Dataset: `bank-demo-v1-r2`, revision `2`.
- Request: `3d329d75-7c4c-4da5-b53d-c3ec9d24c659`.
- Task: `c50bf328-d7e2-490d-8f03-3169c0d55b01`.
- Policy record: `policy-8d67cdfe4df4018c526eac920b65d761`, policy version `3`, approval epoch `1`.
- Proof job: `5a7f1be13871f4fd69d972f15e6fb193706015fcccf42a3828cf61928b70f5cb`.

## Observed Evidence

Read-only SSH checks on 2026-10-18 established:

- Admission and issuance signing completed. The request retained a proof-job ID.
- BullMQ retained `SERVICE_UNAVAILABLE` and progress phase `PROVING`.
- Processing began at `1792319899222` ms and finished at `1792319910069` ms,
  an interval of 10,847 ms. This was not a completed 15-minute proof attempt.
- No cgroup OOM or maximum-memory events were recorded. Swap maximum was `0`.
- Eight CPUs were visible. Go/Rayon threads were `8`, Go memory target `24GiB`.
- The installed prover's public `--check-program-key` returned
  `approved_program_key=pass`.
- All four public Plonk artifact checksums passed.
- Policy and accepted snapshot records carried the request's exact policy hash.
  Both were within their validity periods at worker start.
- The snapshot file existed with mode `0600`, owner `0:0`. Registry and snapshot
  directories had mode `0700`, owner `0:0`.
- The current registry publication preceded worker start. No same-name policy
  approval supersession appeared in its public metadata.
- Proof launcher and supervisor logs contained no retained error detail.

These checks do not prove that every trusted-state validation or private input
assembly step passed. They rule out the earlier wrong-revision admission failure,
the known guest-key mismatch, and the observed resource-limit failure modes.

## Diagnostic Gap And Next Action

`one_shot/executor.rs` distinguishes rejected input from unavailable runtime state,
but `worker/processor.rs` maps all non-cancellation executor failures to public
`SERVICE_UNAVAILABLE`. This includes rejected witnesses or authorization inputs.
The retained reason does not establish an infrastructure fault or rule out a
membership problem. The launcher discards child stdout/stderr, and the retained job
does not identify the exact failed stage.

Add an owner-only, closed diagnostic class/stage at the supervisor boundary, with
no raw child stderr, tuples, policy expressions, caveats, or witness bytes. Keep
agent errors generic. Review this logging change separately from list ordering,
then capture one controlled fresh attempt after maintainer approval.

No VM source, configuration, service, job, role, or private record was modified.
No proof was replayed or submitted during this check.
