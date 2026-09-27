import {
  activatePolicyCandidate,
  loadPolicyActivationConfig,
  PolicyActivationError,
} from "../policy-activation/index.ts";
import { loadPolicyVersionCandidate } from "./store.ts";
import { loadPolicyVersionConfig } from "./validation.ts";

export async function publishPolicyVersion(
  candidateId: string,
  operatorId: string,
  environment: NodeJS.ProcessEnv = process.env,
) {
  const prepared = await loadPolicyVersionCandidate(
    candidateId,
    loadPolicyVersionConfig(environment),
  );
  // A separate owner socket selects composed publication. Never fall back to M5 activation.
  const config = loadPolicyActivationConfig({
    ...environment,
    RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: environment.RAZIEL_POLICY_PUBLICATION_SOCKET_PATH,
  });
  const publication = await activatePolicyCandidate(candidateId, operatorId, {
    ...config,
    publication: true,
  });
  if (
    publication.policy_name !== prepared.candidate.policy_name ||
    publication.policy_version !== prepared.candidate.policy_version ||
    publication.policy_hash !== prepared.candidate.policy_hash ||
    publication.record_id !== prepared.candidate.record_id ||
    publication.approval_epoch !== prepared.candidate.expected_approval_epoch
  ) {
    throw new PolicyActivationError("POLICY_ACTIVATION_CONFLICT");
  }
  return { identity: prepared.identity, publication };
}
