import { describe, expect, it } from "vitest";

import { dynamic, GET } from "../app/api/health/route.ts";

describe("public ingress liveness", () => {
  it("returns only status without authentication or private configuration", async () => {
    const response = GET();
    expect(dynamic).toBe("force-dynamic");
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ status: "ok" });
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("x-content-type-options")).toBe("nosniff");
    expect(response.headers.get("set-cookie")).toBeNull();
  });
});
