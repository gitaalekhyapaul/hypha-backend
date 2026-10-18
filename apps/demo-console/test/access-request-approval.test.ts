import { create } from "@bufbuild/protobuf";
import { AccessRequestSchema } from "@raziel/proto-ts";
import {
  Children,
  type FunctionComponent,
  isValidElement,
  type ReactElement,
  type ReactNode,
} from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ApprovedPolicyCatalogueEntry,
  GrantInputField,
} from "../lib/policy-version/catalogue-types.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";

const mocks = vi.hoisted(() => ({
  catalogue: vi.fn(),
  roles: vi.fn(),
  approve: vi.fn(),
  get: vi.fn(),
}));

// Drive the component's hooks and event handlers without a browser or DOM dependency.
const hooks = vi.hoisted(() => {
  let slots: unknown[] = [];
  let cursor = 0;
  let dirty = false;
  let effects: (() => void)[] = [];
  return {
    reset() {
      slots = [];
      cursor = 0;
      dirty = false;
      effects = [];
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
  useId: () => "approval-test",
  useState: hooks.useState,
  useRef: hooks.useRef,
  useEffect: hooks.useEffect,
}));
vi.mock("../lib/portal-client/policy-catalogue.ts", () => ({
  getApprovedPolicyCatalogue: mocks.catalogue,
}));
vi.mock("../lib/portal-client/agent-roles.ts", () => ({
  listAgentRoles: mocks.roles,
}));
vi.mock("../lib/portal-client/access-requests.ts", () => ({
  approveAccessRequest: mocks.approve,
  getAccessRequest: mocks.get,
}));

import { AccessRequestApproval } from "../components/access-request-approval.tsx";

function policy(
  requiredGrantFields: readonly GrantInputField[],
  id = "b",
): ApprovedPolicyCatalogueEntry {
  const familyId = "a".repeat(32);
  const versionId = id.repeat(32);
  return {
    identity: { familyId, versionId, version: 1 },
    recordId: `record-${id}`,
    policyHash: id.repeat(64),
    policyName: `family:${familyId}:v1`,
    policyVersion: "1",
    resourceType: `dataset_${versionId}`,
    permission: "query",
    approvalEpoch: "1",
    validFromUnixSeconds: "1",
    validUntilUnixSeconds: "253402300799",
    requiredGrantFields,
  };
}

let props: Parameters<typeof AccessRequestApproval>[0];
let root: ReactNode;

beforeEach(() => {
  vi.clearAllMocks();
  hooks.reset();
  props = {
    request: create(AccessRequestSchema, {
      agentId: "d".repeat(32),
      requestId: "request",
      datasetId: "processor-demo-v1",
      generation: 1n,
    }),
    csrfToken: "csrf",
    disabled: false,
    onBusyChange: vi.fn(),
    onUpdated: vi.fn(),
    onSessionExpired: vi.fn(),
  };
  mocks.approve.mockResolvedValue(props.request);
  mocks.roles.mockResolvedValue({ managed: false });
});

function render() {
  for (let attempt = 0; attempt < 10; attempt++) {
    hooks.begin();
    root = AccessRequestApproval(props);
    if (!hooks.flush()) return;
  }
  throw new Error("Form did not settle");
}

type Element = ReactElement<Record<string, unknown> & { children?: ReactNode }>;
function elements(node: ReactNode = root): Element[] {
  if (!isValidElement<Record<string, unknown> & { children?: ReactNode }>(node)) return [];
  if (typeof node.type === "function")
    return elements((node.type as FunctionComponent<Element["props"]>)(node.props) as ReactNode);
  return [node, ...Children.toArray(node.props.children).flatMap((child) => elements(child))];
}
function find(type: string, idSuffix = ""): Element {
  const result = elements().find(
    (element) => element.type === type && String(element.props.id ?? "").endsWith(idSuffix),
  );
  if (!result) throw new Error(`Missing ${type} ${idSuffix}`);
  return result;
}
function change(element: Element, value: string | boolean) {
  (element.props.onChange as (event: unknown) => void)({ target: { value, checked: value } });
  render();
}
async function select(entry: ApprovedPolicyCatalogueEntry) {
  change(find("select"), `${entry.identity.versionId}:${entry.approvalEpoch}:${entry.recordId}`);
  await settle();
  render();
}
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));
async function mount(policies: readonly ApprovedPolicyCatalogueEntry[]) {
  mocks.catalogue.mockResolvedValue({ formatVersion: 1, policies });
  render();
  await settle();
  render();
}
async function submit() {
  (find("form").props.onSubmit as (event: unknown) => void)({ preventDefault() {} });
  await settle();
  render();
}
function inputs(): string[] {
  return elements()
    .filter((element) => element.type === "input")
    .map((element) => String(element.props.id).replace("approval-test-", ""));
}

describe("policy-aware private grant form", () => {
  it.each(
    [
      [],
      ["teamId"],
      ["organizationId"],
      ["approvedAmount"],
      ["isApproved", "isSuspended"],
      ["approvedAmount", "isOverride"],
      ["teamId", "organizationId", "approvedAmount", "isApproved", "isSuspended", "isOverride"],
    ].map((fields) => ({ fields: fields as GrantInputField[] })),
  )("renders only projected fields $fields", async ({ fields }) => {
    const entry = policy(fields);
    await mount([entry]);
    expect(inputs()).toEqual([]);
    await select(entry);
    expect(inputs()).toEqual(
      fields.map((field) => (field === "approvedAmount" ? "amount" : field)),
    );
    expect(elements().some((element) => element.type === "details")).toBe(fields.length > 0);
  });

  it("submits the unchanged full default payload for a policy needing no fields", async () => {
    const entry = policy([]);
    await mount([entry]);
    await select(entry);
    await submit();
    expect(mocks.approve).toHaveBeenCalledWith(
      props.request.agentId,
      props.request.requestId,
      {
        formatVersion: 1,
        expectedGeneration: "1",
        policyRecordId: entry.recordId,
        policyVersionId: entry.identity.versionId,
        approvalEpoch: "1",
        grant: {
          formatVersion: 1,
          teamId: "finance-operations",
          organizationId: "nus-fintech-lab",
          approvedAmount: "50000",
          isApproved: true,
          isSuspended: false,
          isOverride: false,
        },
      },
      "csrf",
    );
  });

  it("resets entered values and stale errors across policy changes", async () => {
    const team = policy(["teamId"]);
    const direct = policy([], "c");
    await mount([team, direct]);
    await select(team);
    change(find("input", "teamId"), "");
    mocks.approve.mockRejectedValueOnce(new PortalRequestError("INVALID_REQUEST", 400));
    await submit();
    expect(elements().some((element) => element.props.role === "alert")).toBe(true);
    await select(direct);
    expect(inputs()).toEqual([]);
    expect(elements().some((element) => element.props.role === "alert")).toBe(false);
    await select(team);
    expect(find("input", "teamId").props.value).toBe("finance-operations");
  });

  it("retains the exact uncertain retry through same-policy refresh and generation updates", async () => {
    const team = policy(["teamId"]);
    await mount([team]);
    await select(team);
    change(find("input", "teamId"), "private-team");
    mocks.approve.mockRejectedValueOnce(new Error("uncertain response"));
    await submit();
    const submitted = mocks.approve.mock.calls[0]?.[2];
    props = { ...props, request: { ...props.request, generation: 2n } };
    const refresh = elements().find(
      (element) => element.props["aria-label"] === "Refresh approved policies",
    );
    if (!refresh) throw new Error("Missing catalogue refresh");
    (refresh.props.onClick as () => void)();
    render();
    await settle();
    render();
    await settle();
    render();
    await submit();
    expect(mocks.approve.mock.calls[1]?.[2]).toBe(submitted);
    expect(submitted.expectedGeneration).toBe("1");
    expect(submitted.grant.teamId).toBe("private-team");
  });

  it("resets every private value after switching away from a policy", async () => {
    const all = policy([
      "teamId",
      "organizationId",
      "approvedAmount",
      "isApproved",
      "isSuspended",
      "isOverride",
    ]);
    const direct = policy([], "c");
    await mount([all, direct]);
    await select(all);
    change(find("input", "teamId"), "old-team");
    change(find("input", "organizationId"), "old-org");
    change(find("input", "amount"), "123");
    change(find("input", "isApproved"), false);
    change(find("input", "isSuspended"), true);
    change(find("input", "isOverride"), true);
    await select(direct);
    await select(all);
    expect(find("input", "teamId").props.value).toBe("finance-operations");
    expect(find("input", "organizationId").props.value).toBe("nus-fintech-lab");
    expect(find("input", "amount").props.value).toBe("50000");
    expect(find("input", "isApproved").props.checked).toBe(true);
    expect(find("input", "isSuspended").props.checked).toBe(false);
    expect(find("input", "isOverride").props.checked).toBe(false);
  });

  it("clears a stale conflict notice after selecting another policy", async () => {
    const team = policy(["teamId"]);
    const direct = policy([], "c");
    await mount([team, direct]);
    await select(team);
    mocks.approve.mockRejectedValueOnce(new PortalRequestError("CONFLICT", 409));
    mocks.get.mockResolvedValueOnce(props.request);
    await submit();
    expect(elements().some((element) => element.props.className === "form-notice")).toBe(true);
    await select(direct);
    expect(elements().some((element) => element.props.className === "form-notice")).toBe(false);
  });

  it("drops an uncertain retry after an explicit policy change", async () => {
    const team = policy(["teamId"]);
    const direct = policy([], "c");
    await mount([team, direct]);
    await select(team);
    change(find("input", "teamId"), "private-team");
    mocks.approve.mockRejectedValueOnce(new Error("uncertain response"));
    await submit();
    await select(direct);
    await submit();
    expect(mocks.approve.mock.calls[1]?.[2]).toMatchObject({
      policyRecordId: direct.recordId,
      grant: { teamId: "finance-operations" },
    });
  });

  it("preserves an uncertain retry through a temporary catalogue outage", async () => {
    const team = policy(["teamId"]);
    await mount([team]);
    await select(team);
    change(find("input", "teamId"), "private-team");
    mocks.approve.mockRejectedValueOnce(new Error("uncertain response"));
    await submit();
    const submitted = mocks.approve.mock.calls[0]?.[2];
    mocks.catalogue.mockRejectedValueOnce(new Error("catalogue unavailable"));
    for (let attempt = 0; attempt < 2; attempt++) {
      const refresh = elements().find(
        (element) => element.props["aria-label"] === "Refresh approved policies",
      );
      if (!refresh) throw new Error("Missing catalogue refresh");
      (refresh.props.onClick as () => void)();
      render();
      await settle();
      render();
      await settle();
      render();
    }
    expect(find("input", "teamId").props.value).toBe("private-team");
    await submit();
    expect(mocks.approve.mock.calls[1]?.[2]).toBe(submitted);
  });

  it("hides stale controls and clears messages after the selected approval disappears", async () => {
    const team = policy(["teamId"]);
    await mount([team]);
    await select(team);
    mocks.approve.mockRejectedValueOnce(new Error("uncertain response"));
    await submit();
    mocks.catalogue.mockResolvedValue({ formatVersion: 1, policies: [] });
    const refresh = elements().find(
      (element) => element.props["aria-label"] === "Refresh approved policies",
    );
    if (!refresh) throw new Error("Missing catalogue refresh");
    (refresh.props.onClick as () => void)();
    render();
    await settle();
    render();
    expect(inputs()).toEqual([]);
    expect(find("button").props.disabled).toBe(false);
    expect(elements().find((element) => element.props.type === "submit")?.props.disabled).toBe(
      true,
    );
    expect(elements().filter((element) => element.props.role === "alert")).toHaveLength(1);
  });
});
