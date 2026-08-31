import type { PolicyBuilderState } from "../policy-builder/index.ts";
import { policyBoundResourceScope } from "../access-graph/model.ts";
import type {
  CapabilityRequestInput,
  CapabilityContextEntry,
  CreateCapabilityInput,
  PublishedAccessSelection,
} from "./types.ts";

export interface CapabilitySubmissionState {
  readonly fingerprint: string;
  readonly submissionId: string;
}

export function buildCapabilityRequestInput(
  builder: PolicyBuilderState,
  access: PublishedAccessSelection,
  requestedValidity: number,
  requestContext: readonly CapabilityContextEntry[],
): CapabilityRequestInput {
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

export function prepareCapabilitySubmission(
  request: CapabilityRequestInput,
  current: CapabilitySubmissionState | null,
  createSubmissionId = () => crypto.randomUUID(),
): { readonly input: CreateCapabilityInput; readonly state: CapabilitySubmissionState } {
  const fingerprint = JSON.stringify(request);
  const state =
    current?.fingerprint === fingerprint
      ? current
      : { fingerprint, submissionId: createSubmissionId() };
  return {
    input: { ...request, submission_id: state.submissionId },
    state,
  };
}
