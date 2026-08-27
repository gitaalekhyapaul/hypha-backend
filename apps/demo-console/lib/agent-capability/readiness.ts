import type { CapabilityJobProjection, PublishedAccessSelection } from "./types.ts";

export function capabilityAuthorizesPublishedDataset(
  job: CapabilityJobProjection | null,
  access: PublishedAccessSelection | null,
): boolean {
  const capability = job?.capability ?? null;
  return (
    access !== null &&
    job?.mode === "live" &&
    job.state === "PROOF_JOB_STATE_SUCCEEDED" &&
    job.authorizes_requests &&
    capability !== null &&
    capability.capability_id === job.job_id &&
    capability.action === "query" &&
    capability.resource_type === "dataset" &&
    capability.resource_scope === access.datasetId &&
    capability.snapshot_id === access.snapshotId &&
    capability.policy_hash === access.policyHash &&
    capability.agent_key_thumbprint === access.agent.key_thumbprint
  );
}
