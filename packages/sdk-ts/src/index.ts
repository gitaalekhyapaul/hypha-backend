export const RAZIEL_SDK_PACKAGE = "@raziel/sdk-ts" as const;

export interface SdkPackageInfo {
  readonly packageName: typeof RAZIEL_SDK_PACKAGE;
  readonly readiness: "scaffold";
  readonly authorizationBehavior: "not-implemented";
}

export function getSdkPackageInfo(): SdkPackageInfo {
  return {
    packageName: RAZIEL_SDK_PACKAGE,
    readiness: "scaffold",
    authorizationBehavior: "not-implemented",
  };
}
