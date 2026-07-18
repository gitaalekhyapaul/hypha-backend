import { describe, expect, it } from "vitest";

import { consoleStatus, countReadyItems, type StatusItem } from "../lib/console-status.ts";

describe("console status", () => {
  it("reports no M0 workflow step as ready", () => {
    expect(countReadyItems(consoleStatus.workflow)).toBe(0);
  });

  it("derives ready item counts from state", () => {
    const items: readonly StatusItem[] = [
      { id: "one", label: "One", detail: "Ready", state: "ready" },
      { id: "two", label: "Two", detail: "Pending", state: "pending" },
      { id: "three", label: "Three", detail: "Ready", state: "ready" },
    ];

    expect(countReadyItems(items)).toBe(2);
  });
});
