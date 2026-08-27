export * from "./agent.js";
export * from "./canonical.js";
export * from "./types.js";

export const RAZIEL_SDK_PACKAGE = "@raziel/sdk-ts" as const;

export interface SdkPackageInfo {
  readonly packageName: typeof RAZIEL_SDK_PACKAGE;
  readonly readiness: "m5-signing";
  readonly authorizationBehavior: "proof-capability-client";
}

export function getSdkPackageInfo(): SdkPackageInfo {
  return {
    packageName: RAZIEL_SDK_PACKAGE,
    readiness: "m5-signing",
    authorizationBehavior: "proof-capability-client",
  };
}
