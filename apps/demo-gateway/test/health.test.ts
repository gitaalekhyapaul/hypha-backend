import { describe, expect, it } from "vitest";

import { GET } from "../app/api/health/route";
import { createHealthPayload, gatewayStatus } from "../src/service-status.ts";

describe("gateway health", () => {
  it("contains shell status only", () => {
    expect(createHealthPayload()).toEqual({
      service: "demo-gateway",
      milestone: "M0",
      status: "ok",
    });
  });

  it("returns a fresh payload for each caller", () => {
    expect(createHealthPayload()).not.toBe(gatewayStatus);
  });

  it("serves the public health payload", async () => {
    const response = GET();

    expect(response.status).toBe(200);
    await expect(response.json()).resolves.toEqual(gatewayStatus);
  });
});
