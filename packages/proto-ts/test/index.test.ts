import { describe, expect, it } from "vitest";

import { getProtoPackageInfo, RAZIEL_PROTO_PACKAGE } from "../src/index.ts";

describe("proto package info", () => {
  it("identifies the future generated namespace", () => {
    expect(getProtoPackageInfo()).toEqual({
      packageName: RAZIEL_PROTO_PACKAGE,
      generationStatus: "pending",
    });
  });
});
