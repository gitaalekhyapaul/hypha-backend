export const RAZIEL_PROTO_PACKAGE = "raziel.v1" as const;

export interface ProtoPackageInfo {
  readonly packageName: typeof RAZIEL_PROTO_PACKAGE;
  readonly generationStatus: "pending";
}

export function getProtoPackageInfo(): ProtoPackageInfo {
  return {
    packageName: RAZIEL_PROTO_PACKAGE,
    generationStatus: "pending",
  };
}
