export type StatusState = "pending" | "ready" | "unavailable";

export interface StatusItem {
  readonly id: string;
  readonly label: string;
  readonly detail: string;
  readonly state: StatusState;
}

interface ConsoleStatus {
  readonly services: readonly StatusItem[];
  readonly workflow: readonly StatusItem[];
}

export const consoleStatus: ConsoleStatus = {
  services: [
    {
      id: "demo-gateway",
      label: "Demo gateway",
      detail: "Not connected",
      state: "unavailable",
    },
    {
      id: "razield",
      label: "Prover service",
      detail: "Not configured",
      state: "pending",
    },
    {
      id: "verifierd",
      label: "Verifier service",
      detail: "Not configured",
      state: "pending",
    },
  ],
  workflow: [
    {
      id: "source",
      label: "Source manifest",
      detail: "Not selected",
      state: "pending",
    },
    {
      id: "policy",
      label: "Approved policy",
      detail: "Not registered",
      state: "pending",
    },
    {
      id: "snapshot",
      label: "Relationship snapshot",
      detail: "Not published",
      state: "pending",
    },
    {
      id: "capability",
      label: "Proof capability",
      detail: "Not requested",
      state: "pending",
    },
  ],
};

export function countReadyItems(items: readonly StatusItem[]): number {
  return items.filter((item) => item.state === "ready").length;
}
