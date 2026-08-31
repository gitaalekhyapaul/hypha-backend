import type { PolicyBuilderState } from "../policy-builder/index.ts";
import { policyBoundResourceScope } from "../access-graph/model.ts";
import type {
  CapabilityContextEntry,
  CreateCapabilityInput,
  PublishedAccessSelection,
} from "./types.ts";

export function buildCapabilityRequestInput(
  builder: PolicyBuilderState,
  access: PublishedAccessSelection,
  requestedValidity: number,
  requestContext: readonly CapabilityContextEntry[],
): CreateCapabilityInput {
  const boundResourceScope = policyBoundResourceScope(builder);
  if (boundResourceScope !== null && boundResourceScope !== access.datasetId) {
    throw new Error("Published dataset does not match the policy-bound resource scope");
  }
  return {
    agent_id: access.agentId,
    task_id: builder.taskId,
    purpose: builder.purpose,
    action: "query",
    resource_type: "dataset",
    resource_scope: access.datasetId,
    requested_valid_for_seconds: requestedValidity,
    snapshot_id: access.snapshotId,
    snapshot_expires_at_unix_seconds: access.snapshotExpiresAtUnixSeconds,
    request_context: requestContext,
  };
}
