export * from "./gen/raziel/v1/audit_pb.js";
export * from "./gen/raziel/v1/authority_pb.js";
export * from "./gen/raziel/v1/capability_pb.js";
export * from "./gen/raziel/v1/common_pb.js";
export * from "./gen/raziel/v1/policy_ir_pb.js";
export * from "./gen/raziel/v1/policy_pb.js";
export * from "./gen/raziel/v1/prover_service_pb.js";
export * from "./gen/raziel/v1/snapshot_pb.js";
export * from "./gen/raziel/v1/verifier_service_pb.js";

export const RAZIEL_PROTO_PACKAGE = "raziel.v1" as const;

export interface ProtoPackageInfo {
  readonly packageName: typeof RAZIEL_PROTO_PACKAGE;
  readonly generationStatus: "generated";
}

export function getProtoPackageInfo(): ProtoPackageInfo {
  return {
    packageName: RAZIEL_PROTO_PACKAGE,
    generationStatus: "generated",
  };
}
