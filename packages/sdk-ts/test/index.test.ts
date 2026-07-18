import { describe, expect, it } from "vitest";

import { getSdkPackageInfo, RAZIEL_SDK_PACKAGE } from "../src/index.ts";

describe("SDK package info", () => {
  it("does not claim authorization behavior", () => {
    expect(getSdkPackageInfo()).toEqual({
      packageName: RAZIEL_SDK_PACKAGE,
      readiness: "scaffold",
      authorizationBehavior: "not-implemented",
    });
  });
});
