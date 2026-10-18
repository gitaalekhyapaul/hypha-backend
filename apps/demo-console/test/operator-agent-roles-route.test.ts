import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { GET, POST } from "../app/api/operator/agents/[agentId]/roles/route.ts";
import { agentAuthService } from "../lib/agent-auth/runtime.ts";
import { AgentAuthService } from "../lib/agent-auth/service.ts";
import type { AgentAuthStore, AgentRecord } from "../lib/agent-auth/types.ts";
import { executeAgentRoles } from "../lib/agent-roles/service.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
  OperatorAuthService,
} from "../lib/operator-auth/index.ts";

vi.mock("../lib/agent-auth/runtime.ts", () => ({ agentAuthService: vi.fn() }));

const ORIGIN = "http://127.0.0.1:3000";
const AGENT_ID = "a1".repeat(16);
const KEY_HEX = Buffer.from(Array.from({ length: 32 }, (_, index) => index)).toString("hex");
const SCOPE = {
  policyVersionId: "b2".repeat(16),
  approvalEpoch: "18446744073709551615",
  datasetId: "processor-001",
};
const RECORD: AgentRecord = {
  formatVersion: 1,
  agentId: AGENT_ID,
  displayName: "Role test agent",
  publicKey: Buffer.alloc(32, 7).toString("base64"),
  keyThumbprint: Buffer.from(KEY_HEX, "hex").toString("base64"),
  createdAtUnixSeconds: "1800000000",
  active: true,
};
const context = (agentId = AGENT_ID) => ({ params: Promise.resolve({ agentId }) });
let secretRoot: string;
let root: string;
let cookie: string;
let csrfToken: string;
let service: AgentAuthService;
let store: AgentAuthStore;

beforeAll(async () => {
  secretRoot = await mkdtemp(join(tmpdir(), "raziel-role-auth-"));
  await chmod(secretRoot, 0o700);
  const password = "controlled role test password";
  const salt = Buffer.alloc(16, 0x31);
  const verifier = await derivePasswordVerifierForProvisioning(password, salt);
  const secretPath = join(secretRoot, "operator.json");
  await writeFile(
    secretPath,
    JSON.stringify({
      format_version: 1,
      operator_id: "operator-role-test",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x41).toString("base64url"),
    }),
    { mode: 0o600 },
  );
  const issued = await new OperatorAuthService({
    secretPath,
    expectedOrigin: ORIGIN,
    liveMode: false,
  }).login(password);
  cookie = issued.setCookie.split(";", 1)[0] ?? "";
  csrfToken = issued.session.csrfToken;
});

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), "raziel-role-child-"));
  vi.stubEnv("RAZIEL_OPERATOR_SECRET_PATH", join(secretRoot, "operator.json"));
  vi.stubEnv("RAZIEL_OPERATOR_EXPECTED_ORIGIN", ORIGIN);
  vi.stubEnv("RAZIEL_OPERATOR_LIVE_MODE", "false");
  vi.stubEnv("RAZIEL_POLICY_ACTIVATION_EXECUTABLE", join(root, "roles.mjs"));
  vi.stubEnv("RAZIEL_POLICY_ACTIVATION_SOCKET_PATH", join(root, "legacy.sock"));
  vi.stubEnv("RAZIEL_POLICY_PUBLICATION_SOCKET_PATH", join(root, "composed.sock"));
  vi.stubEnv("RAZIEL_TEST_PRIVATE_SECRET", "must-not-reach-child");
  vi.stubEnv("RAZIEL_REDIS_URL", "redis://private-test-value");
  store = {
    register: vi.fn(async () => true),
    getAgent: vi.fn(async () => RECORD),
    listAgents: vi.fn(async () => [RECORD]),
    createToken: vi.fn(async () => true),
    getToken: vi.fn(async () => undefined),
    listTokens: vi.fn(async () => []),
    revokeToken: vi.fn(async () => undefined),
    consumeNonce: vi.fn(async () => "unauthenticated" as const),
  };
  service = new AgentAuthService("role-test", store, vi.fn());
  vi.mocked(agentAuthService).mockReturnValue(service);
  await childSource();
});

afterEach(async () => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  await rm(root, { recursive: true, force: true });
});

afterAll(async () => {
  await rm(secretRoot, { recursive: true, force: true });
});

describe("B-0068 operator agent roles HTTP adapter", () => {
  it("synthesizes list, binds the stored thumbprint, and isolates child arguments/environment", async () => {
    const response = await GET(
      getRequest(undefined, { "x-agent-public-key": "caller-key" }),
      context(),
    );
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    await expect(response.json()).resolves.toEqual(projection());
    const transport = await readTransport();
    expect(transport).toEqual({
      args: ["--agent-roles"],
      request: { formatVersion: 1, operation: "list", ...SCOPE, agentKeyThumbprint: KEY_HEX },
      environment: {
        NODE_ENV: process.env.NODE_ENV ?? "production",
        PATH: process.env.PATH,
        RAZIEL_POLICY_ACTIVATION_SOCKET_PATH: join(root, "composed.sock"),
        RAZIEL_POLICY_PUBLICATION_MODE: "composed",
      },
    });
    expect(Buffer.byteLength(JSON.stringify(transport.request))).toBeLessThanOrEqual(4096);
    expect(store.getAgent).toHaveBeenCalledExactlyOnceWith(AGENT_ID);
    expect(store.listAgents).not.toHaveBeenCalled();
    expect(store.getToken).not.toHaveBeenCalled();
  });

  it.each([
    { operation: "enable" },
    { operation: "add", role: "viewer" },
    { operation: "add", role: "approver" },
    { operation: "add", role: "auditor" },
    { operation: "add", role: "owner" },
    { operation: "remove", role: "viewer" },
    { operation: "remove", role: "owner" },
    { operation: "add", role: "team_member", teamId: "team-001" },
    { operation: "remove", role: "team_member", teamId: "team-001" },
  ])("sends mutation $operation/$role with JSON on stdin", async (operation) => {
    const response = await POST(postRequest(operation), context());
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect((await readTransport()).request).toEqual({
      formatVersion: 1,
      ...SCOPE,
      ...operation,
      agentKeyThumbprint: KEY_HEX,
    });
  });

  it.each(["GET", "POST"])(
    "rejects agent credentials on %s before registration/body access",
    async (method) => {
      const request = new Request(`${ORIGIN}/api/operator/agents/${AGENT_ID}/roles`, {
        method,
        headers: {
          authorization: `Bearer rz1.${AGENT_ID}.${"a".repeat(43)}`,
          origin: ORIGIN,
          "x-raziel-agent-version": "1",
          "x-raziel-agent-signature": "a".repeat(128),
        },
        ...(method === "POST" ? { body: "{}" } : {}),
      });
      const body = vi.spyOn(request, "body", "get");
      const response = await (method === "GET" ? GET : POST)(request, context());
      expect(response.status).toBe(401);
      expect(response.headers.get("cache-control")).toBe("no-store");
      expect(body).not.toHaveBeenCalled();
      expect(store.getAgent).not.toHaveBeenCalled();
      await expect(readTransport()).rejects.toMatchObject({ code: "ENOENT" });
    },
  );

  it.each([
    { [OPERATOR_CSRF_HEADER]: "" },
    { [OPERATOR_CSRF_HEADER]: "wrong" },
    { origin: "https://attacker.invalid" },
    { origin: "" },
  ])("requires CSRF and origin for mutations: %j", async (headers) => {
    const request = postRequest({ operation: "enable" }, headers);
    const body = vi.spyOn(request, "body", "get");
    const response = await POST(request, context());
    expect(response.status).toBe(403);
    expect(body).not.toHaveBeenCalled();
    expect(store.getAgent).not.toHaveBeenCalled();
  });

  it.each([
    "policyVersionId=bad&approvalEpoch=1&datasetId=x",
    `policyVersionId=${SCOPE.policyVersionId.toUpperCase()}&approvalEpoch=1&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=0&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=01&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=18446744073709551616&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=1e3&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}%0A&approvalEpoch=1&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=1%0A&datasetId=x`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=1&datasetId=x%0A`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=1&datasetId=../private`,
    `policyVersionId=${SCOPE.policyVersionId}&approvalEpoch=1&datasetId=${"x".repeat(129)}`,
    "approvalEpoch=1&datasetId=x",
    `${new URLSearchParams(SCOPE)}&datasetId=other`,
    `${new URLSearchParams(SCOPE)}&agentKeyThumbprint=${KEY_HEX}`,
    `${new URLSearchParams(SCOPE)}&operation=add`,
  ])("rejects malformed, duplicate, missing, or identity-bearing query %s", async (query) => {
    const response = await GET(getRequest(query), context());
    expect(response.status).toBe(400);
    expect(store.getAgent).not.toHaveBeenCalled();
  });

  it.each([
    { operation: "list" },
    { operation: "unknown" },
    { operation: { toString: "enable" } },
    { operation: "enable", role: "viewer" },
    { operation: "enable", teamId: "team" },
    { operation: "add" },
    { operation: "remove", role: "team_member" },
    { operation: "add", role: "organization_member" },
    { operation: "add", role: "viewer", teamId: "team" },
    { operation: "add", role: "team_member", teamId: "../team" },
    { operation: "add", role: "team_member", teamId: "x".repeat(129) },
    { operation: "enable", agentKeyThumbprint: KEY_HEX },
    { operation: "enable", publicKey: RECORD.publicKey },
    { operation: "enable", agentId: AGENT_ID },
    { operation: "enable", formatVersion: 2 },
    { operation: "enable", approvalEpoch: 1 },
    { operation: "enable", approvalEpoch: "-1" },
    { operation: "enable", policyVersionId: "x".repeat(32) },
    { operation: "enable", datasetId: "" },
    { operation: "enable", policyVersionId: `${SCOPE.policyVersionId}\n` },
    { operation: "enable", approvalEpoch: "1\n" },
    { operation: "enable", datasetId: "processor-001\n" },
    { operation: "add", role: "team_member", teamId: "team-001\n" },
  ])("rejects malformed mutation %j before registration lookup", async (operation) => {
    const response = await POST(postRequest(operation), context());
    expect(response.status).toBe(400);
    expect(store.getAgent).not.toHaveBeenCalled();
  });

  it.each([
    { body: "{", contentType: "application/json" },
    { body: "null", contentType: "application/json" },
    { body: "[]", contentType: "application/json" },
    { body: "{}", contentType: "text/plain" },
    { body: " ".repeat(4097), contentType: "application/json" },
    { body: Buffer.from([0xff]), contentType: "application/json" },
  ])("bounds and strictly decodes the HTTP body: $contentType", async ({ body, contentType }) => {
    const response = await POST(
      new Request(`${ORIGIN}/api/operator/agents/${AGENT_ID}/roles`, {
        method: "POST",
        headers: mutationHeaders({ "content-type": contentType }),
        body,
      }),
      context(),
    );
    expect(response.status).toBe(400);
    expect(store.getAgent).not.toHaveBeenCalled();
  });

  it("rejects encoded bodies and mutation query parameters", async () => {
    expect(
      (await POST(postRequest({ operation: "enable" }, { "content-encoding": "gzip" }), context()))
        .status,
    ).toBe(400);
    const request = postRequest({ operation: "enable" });
    expect(
      (await POST(new Request(`${request.url}?datasetId=other`, request), context())).status,
    ).toBe(400);
  });

  it("accepts the 4096-byte HTTP input and 64-KiB child-output boundaries", async () => {
    await childSource("process.stdout.write(JSON.stringify(value).padEnd(65536, ' '));");
    const body = JSON.stringify({ formatVersion: 1, ...SCOPE, operation: "enable" });
    const response = await POST(
      new Request(`${ORIGIN}/api/operator/agents/${AGENT_ID}/roles`, {
        method: "POST",
        headers: mutationHeaders(),
        body: body.padEnd(4096, " "),
      }),
      context(),
    );
    expect(response.status).toBe(200);
    await expect(response.json()).resolves.toEqual(projection());
    expect(Buffer.byteLength(JSON.stringify((await readTransport()).request))).toBeLessThan(4096);
  });

  it("validates path IDs and rejects unknown or inactive registrations without launching", async () => {
    expect((await GET(getRequest(), context("../private"))).status).toBe(400);
    expect((await GET(getRequest(), context(`${AGENT_ID}\n`))).status).toBe(400);
    expect(store.getAgent).not.toHaveBeenCalled();
    vi.mocked(store.getAgent).mockResolvedValueOnce(undefined);
    expect((await GET(getRequest(), context())).status).toBe(404);
    vi.mocked(store.getAgent).mockResolvedValueOnce({ ...RECORD, active: false });
    expect((await GET(getRequest(), context())).status).toBe(404);
    await expect(readTransport()).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("exposes registration fields only through the permitted lookup method", async () => {
    const { active: _active, ...registration } = RECORD;
    expect(await service.getAgent(AGENT_ID)).toEqual(registration);
    expect(store.listAgents).not.toHaveBeenCalled();
    expect(store.listTokens).not.toHaveBeenCalled();
  });

  it.each(["", KEY_HEX, RECORD.keyThumbprint.slice(0, -1), `${RECORD.keyThumbprint}\n`])(
    "fails closed on corrupt stored thumbprint %j",
    async (keyThumbprint) => {
      vi.mocked(store.getAgent).mockResolvedValueOnce({ ...RECORD, keyThumbprint });
      const response = await GET(getRequest(), context());
      expect(response.status).toBe(503);
      await expect(readTransport()).rejects.toMatchObject({ code: "ENOENT" });
    },
  );

  it.each([
    ["invalid policy-activation request", 400, "INVALID_POLICY_ACTIVATION_REQUEST"],
    ["policy activation conflicts with trusted state", 409, "POLICY_ACTIVATION_CONFLICT"],
    ["policy candidate was rejected", 422, "POLICY_ACTIVATION_REJECTED"],
    ["private owner details must not escape", 503, "POLICY_ACTIVATION_UNAVAILABLE"],
  ])("maps child failure %s to %s without diagnostic disclosure", async (message, status, code) => {
    await childSource(
      `console.error(${JSON.stringify(`raziel-policy-activate: ${message}`)}); process.exit(2);`,
    );
    const response = await POST(postRequest({ operation: "enable" }), context());
    expect(response.status).toBe(status);
    expect(response.headers.get("cache-control")).toBe("no-store");
    await expect(response.json()).resolves.toEqual({ error: code });
  });

  it.each([
    "value.formatVersion = 2;",
    "delete value.managed;",
    "value.policyVersionId = 'cc'.repeat(16);",
    "value.approvalEpoch = '1';",
    "value.approvalEpoch = 1;",
    "value.datasetId = 'other';",
    "value.agentKeyThumbprint = 'dd'.repeat(32);",
    "value.managed = 'true';",
    "value.canEnable = 1;",
    "value.privateWitness = 'must not escape';",
    "value.availableRoles = ['unsupported'];",
    "value.availableRoles = ['viewer', 'viewer'];",
    "value.assignments = [{role: 'viewer', teamId: 'unexpected'}];",
    "value.assignments = [{role: 'team_member'}];",
    "value.assignments = [{role: 'team_member', teamId: '../private'}];",
    "value.assignments = [{role: 'team_member', teamId: 'team-001\\n'}];",
    "value.assignments = [{role: 'viewer', witness: 'private'}];",
    "value.assignments = [{role: 'viewer'}, {role: 'viewer'}];",
    "value.assignments = null;",
  ])("rejects malformed or mismatched child projection: %s", async (mutation) => {
    await childSource(`${mutation}\nconsole.log(JSON.stringify(value));`);
    const response = await GET(getRequest(), context());
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toEqual({ error: "POLICY_ACTIVATION_UNAVAILABLE" });
  });

  it.each([
    "console.log('not-json');",
    "process.stdout.write(Buffer.from([0xff]));",
    "process.exit(0);",
    "process.kill(process.pid, 'SIGKILL');",
    "process.stdout.write('x'.repeat(65537));",
    "process.stderr.write('x'.repeat(65537));",
    "process.stdout.write('x'.repeat(40000)); process.stderr.write('y'.repeat(40000));",
  ])("contains child output/crash failure: %s", async (source) => {
    await childSource(source);
    const response = await GET(getRequest(), context());
    expect(response.status).toBe(503);
    expect(response.headers.get("cache-control")).toBe("no-store");
  });

  it("fails closed for missing executable or composed socket without legacy fallback", async () => {
    vi.stubEnv("RAZIEL_POLICY_PUBLICATION_SOCKET_PATH", undefined);
    expect((await GET(getRequest(), context())).status).toBe(503);
    vi.stubEnv("RAZIEL_POLICY_PUBLICATION_SOCKET_PATH", join(root, "composed.sock"));
    vi.stubEnv("RAZIEL_POLICY_ACTIVATION_EXECUTABLE", join(root, "missing"));
    expect((await GET(getRequest(), context())).status).toBe(503);
    await expect(readTransport()).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("kills and reaps the child at the fixed 125-second deadline", async () => {
    await childSource("setInterval(() => {}, 1000);");
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const result = executeAgentRoles(RECORD, { formatVersion: 1, operation: "list", ...SCOPE });
    const assertion = expect(result).rejects.toMatchObject({
      code: "POLICY_ACTIVATION_UNAVAILABLE",
    });
    await vi.advanceTimersByTimeAsync(124_999);
    expect(vi.getTimerCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    await assertion;
    expect(vi.getTimerCount()).toBe(0);
  });
});

function getRequest(query = new URLSearchParams(SCOPE).toString(), headers = {}): Request {
  return new Request(`${ORIGIN}/api/operator/agents/${AGENT_ID}/roles?${query}`, {
    headers: { cookie, ...headers },
  });
}

function mutationHeaders(overrides: Record<string, string> = {}): Record<string, string> {
  return {
    cookie,
    origin: ORIGIN,
    "content-type": "application/json",
    [OPERATOR_CSRF_HEADER]: csrfToken,
    ...overrides,
  };
}

function postRequest(operation: Record<string, unknown>, headers = {}): Request {
  return new Request(`${ORIGIN}/api/operator/agents/${AGENT_ID}/roles`, {
    method: "POST",
    headers: mutationHeaders(headers),
    body: JSON.stringify({ formatVersion: 1, ...SCOPE, ...operation }),
  });
}

function projection() {
  return {
    formatVersion: 1,
    ...SCOPE,
    agentKeyThumbprint: KEY_HEX,
    managed: true,
    canEnable: false,
    availableRoles: ["viewer", "approver", "auditor", "owner", "team_member"],
    assignments: [{ role: "viewer" }, { role: "team_member", teamId: "team-001" }],
  };
}

async function readTransport(): Promise<{
  args: string[];
  request: Record<string, unknown>;
  environment: Record<string, string>;
}> {
  return JSON.parse(await readFile(join(root, "transport.json"), "utf8"));
}

async function childSource(action = "console.log(JSON.stringify(value));"): Promise<void> {
  await writeFile(
    join(root, "roles.mjs"),
    `#!${process.execPath}
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const chunks = [];
for await (const chunk of process.stdin) chunks.push(chunk);
const input = Buffer.concat(chunks);
if (input.length > 4096) process.exit(2);
const request = JSON.parse(input.toString('utf8'));
writeFileSync(fileURLToPath(new URL('./transport.json', import.meta.url)), JSON.stringify({
  args: process.argv.slice(2), request, environment: process.env
}));
const value = ${JSON.stringify(projection())};
${action}
`,
    { mode: 0o700 },
  );
}
