import { describe, expect, it } from "vitest";

import { getProtoPackageInfo, RAZIEL_PROTO_PACKAGE } from "../src/index.ts";

describe("proto package info", () => {
  it("identifies the generated namespace", () => {
    expect(getProtoPackageInfo()).toEqual({
      packageName: RAZIEL_PROTO_PACKAGE,
      generationStatus: "generated",
    });
  });
});
