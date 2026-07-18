export const gatewayStatus = {
  service: "demo-gateway",
  milestone: "M0",
  status: "ok",
} as const;

export interface HealthPayload {
  readonly service: typeof gatewayStatus.service;
  readonly milestone: typeof gatewayStatus.milestone;
  readonly status: typeof gatewayStatus.status;
}

export function createHealthPayload(): HealthPayload {
  return { ...gatewayStatus };
}
