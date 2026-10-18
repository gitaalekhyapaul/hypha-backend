import {
  Children,
  type FunctionComponent,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccessRequest } from "../lib/portal-client/access-requests.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";

const mocks = vi.hoisted(() => ({ catalogue: vi.fn(), roles: vi.fn(), approve: vi.fn() }));
// Local hook driver follows access-request-approval.test.ts without adding a DOM dependency.
const hooks = vi.hoisted(() => {
  let slots: unknown[] = [];
  let cursor = 0;
  let dirty = false;
  let effects: (() => void)[] = [];
  return {
    reset() {
      slots = [];
      effects = [];
      cursor = 0;
      dirty = false;
    },
    begin() {
      cursor = 0;
      dirty = false;
    },
    flush() {
      for (const effect of effects.splice(0)) effect();
      return dirty;
    },
    useState(initial: unknown) {
      const index = cursor++;
      if (!(index in slots)) slots[index] = initial;
      return [
        slots[index],
        (value: unknown) => {
          const next = typeof value === "function" ? value(slots[index]) : value;
          if (!Object.is(next, slots[index])) dirty = true;
          slots[index] = next;
        },
      ];
    },
    useRef(initial: unknown) {
      const index = cursor++;
      if (!(index in slots)) slots[index] = { current: initial };
      return slots[index];
    },
    useEffect(effect: () => (() => void) | undefined, dependencies: unknown[]) {
      const index = cursor++;
      const previous = slots[index] as
        | { dependencies: unknown[]; cleanup?: () => void }
        | undefined;
      if (previous && dependencies.every((value, i) => Object.is(value, previous.dependencies[i])))
        return;
      const state = { dependencies, cleanup: undefined as (() => void) | undefined };
      slots[index] = state;
      effects.push(() => {
        previous?.cleanup?.();
        state.cleanup = effect();
      });
    },
  };
});
vi.mock("react", async (original) => ({
  ...(await original<typeof import("react")>()),
  useId: () => "mode-test",
  useState: hooks.useState,
  useRef: hooks.useRef,
  useEffect: hooks.useEffect,
}));
vi.mock("../lib/portal-client/policy-catalogue.ts", () => ({
  getApprovedPolicyCatalogue: mocks.catalogue,
}));
vi.mock("../lib/portal-client/agent-roles.ts", () => ({ listAgentRoles: mocks.roles }));
vi.mock("../lib/portal-client/access-requests.ts", () => ({
  approveAccessRequest: mocks.approve,
  getAccessRequest: vi.fn(),
}));

import { AccessRequestApproval } from "../components/access-request-approval.tsx";

const policies = ["a", "b"].map((digit) => ({
  identity: { familyId: "f".repeat(32), versionId: digit.repeat(32), version: 1 },
  recordId: `record-${digit}`,
  policyHash: digit.repeat(64),
  approvalEpoch: "1",
  validFromUnixSeconds: "1",
  validUntilUnixSeconds: "253402300799",
  requiredGrantFields: ["teamId"],
}));
let props: Parameters<typeof AccessRequestApproval>[0];
let root: ReactNode;
type Element = ReactElement<Record<string, unknown> & { children?: ReactNode }>;
function elements(node: ReactNode = root): Element[] {
  if (!isValidElement<Element["props"]>(node)) return [];
  if (typeof node.type === "function")
    return elements((node.type as FunctionComponent<Element["props"]>)(node.props) as ReactNode);
  return [node, ...Children.toArray(node.props.children).flatMap((child) => elements(child))];
}
function render() {
  for (let i = 0; i < 10; i++) {
    hooks.begin();
    root = AccessRequestApproval(props);
    if (!hooks.flush()) return;
  }
  throw new Error("Approval did not settle");
}
const settle = async () => {
  await new Promise<void>((resolve) => setImmediate(resolve));
  render();
};
function select(index = 0) {
  const policy = policies[index];
  if (!policy) throw new Error("Missing policy");
  const select = elements().find((item) => item.type === "select");
  if (!select) throw new Error("Missing selection");
  (select.props.onChange as (event: unknown) => void)({
    target: { value: `${policy.identity.versionId}:1:${policy.recordId}` },
  });
  render();
}
const hasTeam = () =>
  elements().some((item) => item.type === "input" && String(item.props.id).endsWith("-teamId"));
const managedLabel = () =>
  elements().some((item) => item.type === "p" && item.props.children === "Operator-managed roles");
const approveDisabled = () =>
  elements().find((item) => item.props.type === "submit")?.props.disabled;
async function mount() {
  render();
  await settle();
  select();
}

beforeEach(() => {
  vi.resetAllMocks();
  hooks.reset();
  props = {
    request: {
      agentId: "agent-one",
      requestId: "request-one",
      datasetId: "processor-demo-v1",
      generation: 1n,
    } as AccessRequest,
    csrfToken: "csrf",
    disabled: false,
    onBusyChange: vi.fn(),
    onUpdated: vi.fn(),
    onSessionExpired: vi.fn(),
  };
  mocks.catalogue.mockResolvedValue({ formatVersion: 1, policies });
  mocks.approve.mockResolvedValue(props.request);
});

describe("approval role management mode", () => {
  it("hides private fields for managed versions and preserves the full grant defaults", async () => {
    mocks.roles.mockResolvedValue({ managed: true });
    await mount();
    await settle();
    expect(hasTeam()).toBe(false);
    expect(managedLabel()).toBe(true);
    expect(approveDisabled()).toBe(false);
    expect(mocks.roles).toHaveBeenCalledWith(
      "agent-one",
      { policyVersionId: "a".repeat(32), approvalEpoch: "1", datasetId: "processor-demo-v1" },
      expect.any(AbortSignal),
    );
    const form = elements().find((item) => item.type === "form");
    if (!form) throw new Error("Missing approval form");
    (form.props.onSubmit as (event: unknown) => void)({ preventDefault() {} });
    await settle();
    expect(mocks.approve.mock.calls[0]?.[2].grant).toEqual({
      formatVersion: 1,
      teamId: "finance-operations",
      organizationId: "nus-fintech-lab",
      approvedAmount: "50000",
      isApproved: true,
      isSuspended: false,
      isOverride: false,
    });
  });

  it("keeps the legacy team form usable after resolving mode", async () => {
    mocks.roles.mockResolvedValue({ managed: false });
    await mount();
    await settle();
    expect(hasTeam()).toBe(true);
    expect(managedLabel()).toBe(false);
    expect(approveDisabled()).toBe(false);
  });

  it("hides old mode immediately and ignores an out-of-order policy response", async () => {
    let resolveOld: ((value: { managed: boolean }) => void) | undefined;
    mocks.roles.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOld = resolve;
        }),
    );
    mocks.roles.mockResolvedValueOnce({ managed: true });
    await mount();
    expect(hasTeam()).toBe(false);
    expect(managedLabel()).toBe(false);
    expect(approveDisabled()).toBe(true);
    const oldSignal = mocks.roles.mock.calls[0]?.[2] as AbortSignal;
    select(1);
    await settle();
    expect(oldSignal.aborted).toBe(true);
    expect(managedLabel()).toBe(true);
    resolveOld?.({ managed: false });
    await settle();
    expect(hasTeam()).toBe(false);
    expect(managedLabel()).toBe(true);
  });

  it.each(["agentId", "datasetId"] as const)(
    "invalidates the old mode on %s changes",
    async (field) => {
      mocks.roles
        .mockResolvedValueOnce({ managed: false })
        .mockImplementationOnce(() => new Promise(() => {}));
      await mount();
      await settle();
      expect(hasTeam()).toBe(true);
      props = { ...props, request: { ...props.request, [field]: "changed-id" } };
      render();
      expect(hasTeam()).toBe(false);
      expect(approveDisabled()).toBe(true);
      expect(mocks.roles).toHaveBeenCalledTimes(2);
    },
  );

  it("fails closed instead of falling back to legacy fields", async () => {
    mocks.roles.mockRejectedValue(new PortalRequestError("PORTAL_UNAVAILABLE", 503));
    await mount();
    await settle();
    expect(hasTeam()).toBe(false);
    expect(managedLabel()).toBe(false);
    expect(approveDisabled()).toBe(true);
    expect(elements().some((item) => item.props.role === "alert")).toBe(true);
    expect(mocks.approve).not.toHaveBeenCalled();
  });

  it("handles an expired operator session", async () => {
    mocks.roles.mockRejectedValue(new PortalRequestError("OPERATOR_UNAUTHORIZED", 401));
    await mount();
    await settle();
    expect(props.onSessionExpired).toHaveBeenCalledOnce();
    expect(approveDisabled()).toBe(true);
  });
});
