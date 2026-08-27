import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import { GET as candidateGetRoute } from "../app/api/policies/candidates/[candidateId]/route.ts";
import { POST as candidatePostRoute } from "../app/api/policies/candidates/route.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
} from "../lib/operator-auth/index.ts";

const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "candidate route password";
const roots: string[] = [];
const environmentNames = [
  "RAZIEL_OPERATOR_SECRET_PATH",
  "RAZIEL_OPERATOR_EXPECTED_ORIGIN",
  "RAZIEL_OPERATOR_LIVE_MODE",
  "RAZIEL_POLICY_CANDIDATE_ROOT",
  "RAZIEL_POLICY_COMPILER_EXECUTABLE",
  "RAZIEL_ADMIN_EXECUTABLE",
] as const;
const originalEnvironment = Object.fromEntries(
  environmentNames.map((name) => [name, process.env[name]]),
) as Record<(typeof environmentNames)[number], string | undefined>;

afterEach(async () => {
  restoreEnvironment();
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("M5 authenticated policy candidate routes", () => {
  it("creates and reads one safe candidate projection", async () => {
    await configureRouteFixture();
    const session = await login();
    const created = await candidatePostRoute(candidateRequest(session));
    expect(created.status).toBe(201);
    const projection = (await created.json()) as Record<string, unknown>;
    expect(projection).toMatchObject({
      candidate_id: "candidate-route-001",
      policy_name: "reconciliation-query",
      resource_type: "dataset",
      permission: "query",
      rule_summary: "viewer",
      status: "COMPILED",
    });
    for (const privateField of [
      "schema",
      "compiler_config",
      "policy_ir",
      "diagnostic",
      "manifest",
      "secret_path",
      "authority_key",
      "relationships",
      "zed_token",
    ]) {
      expect(projection).not.toHaveProperty(privateField);
    }

    const read = await candidateGetRoute(
      new Request(`${ORIGIN}/api/policies/candidates/candidate-route-001`, {
        headers: { cookie: session.cookie },
      }),
      { params: Promise.resolve({ candidateId: "candidate-route-001" }) },
    );
    expect(read.status).toBe(200);
    expect(await read.json()).toEqual(projection);
  });

  it("authorizes before body parsing and rejects oversized bodies", async () => {
    await configureRouteFixture();
    const unread = new Request(`${ORIGIN}/api/policies/candidates`, { method: "POST" });
    Object.defineProperty(unread, "text", {
      value: async () => {
        throw new Error("body should not be read");
      },
    });
    const unauthorized = await candidatePostRoute(unread);
    expect(unauthorized.status).toBe(403);
    expect(await unauthorized.json()).toEqual({ error: "OPERATOR_ORIGIN_REJECTED" });

    const session = await login();
    const oversized = candidateRequest(session, { "content-length": "40000" });
    const rejected = await candidatePostRoute(oversized);
    expect(rejected.status).toBe(400);
    expect(await rejected.json()).toEqual({ error: "INVALID_CANDIDATE_REQUEST" });
  });

  it("rejects invalid and missing symbolic candidate identifiers", async () => {
    await configureRouteFixture();
    const session = await login();
    const invalid = await candidateGetRoute(
      new Request(`${ORIGIN}/api/policies/candidates/private`, {
        headers: { cookie: session.cookie },
      }),
      { params: Promise.resolve({ candidateId: "../private" }) },
    );
    expect(invalid.status).toBe(400);

    const missing = await candidateGetRoute(
      new Request(`${ORIGIN}/api/policies/candidates/missing`, {
        headers: { cookie: session.cookie },
      }),
      { params: Promise.resolve({ candidateId: "missing" }) },
    );
    expect(missing.status).toBe(404);
  });
});

interface LoggedInSession {
  readonly cookie: string;
  readonly csrfToken: string;
}

async function login(): Promise<LoggedInSession> {
  const response = await loginRoute(
    new Request(`${ORIGIN}/api/operator/login`, {
      method: "POST",
      headers: { origin: ORIGIN },
      body: JSON.stringify({ password: PASSWORD }),
    }),
  );
  expect(response.status).toBe(200);
  const body = (await response.json()) as { readonly csrfToken: string };
  return { cookie: cookiePair(requiredHeader(response, "set-cookie")), csrfToken: body.csrfToken };
}

function candidateRequest(
  session: LoggedInSession,
  additionalHeaders: Record<string, string> = {},
): Request {
  return new Request(`${ORIGIN}/api/policies/candidates`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      cookie: session.cookie,
      origin: ORIGIN,
      [OPERATOR_CSRF_HEADER]: session.csrfToken,
      ...additionalHeaders,
    },
    body: JSON.stringify({
      draft: {
        formatVersion: 1,
        candidateId: "candidate-route-001",
        policyName: "reconciliation-query",
        policyVersion: 1,
        resourceType: "dataset",
        permission: "query",
        rule: { kind: "direct", relation: "viewer" },
      },
      expectedApprovalEpoch: 1,
      validForSeconds: 86_400,
    }),
  });
}

async function configureRouteFixture(): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-candidate-route-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const candidateRoot = join(root, "candidates");
  await mkdir(candidateRoot, { mode: 0o700 });
  const secretPath = join(root, "operator-secret.json");
  const salt = Buffer.alloc(16, 0x51);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: "operator-route",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x61).toString("base64url"),
    })}\n`,
    { mode: 0o600 },
  );

  const compiler = join(root, "compiler.mjs");
  const admin = join(root, "admin.mjs");
  await writeFile(compiler, compilerSource(), { mode: 0o700 });
  await writeFile(admin, adminSource(), { mode: 0o700 });

  process.env.RAZIEL_OPERATOR_SECRET_PATH = secretPath;
  process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
  process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";
  process.env.RAZIEL_POLICY_CANDIDATE_ROOT = candidateRoot;
  process.env.RAZIEL_POLICY_COMPILER_EXECUTABLE = compiler;
  process.env.RAZIEL_ADMIN_EXECUTABLE = admin;
}

function compilerSource(): string {
  return `#!/usr/bin/env node
import { writeFile } from "node:fs/promises";
const value = (name) => process.argv[process.argv.indexOf(name) + 1];
await writeFile(value("--out"), Buffer.from([1]), { mode: 0o600 });
await writeFile(value("--diagnostic-out"), "{}\\n", { mode: 0o600 });
`;
}

function adminSource(): string {
  return `#!/usr/bin/env node
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
const value = (name) => process.argv[process.argv.indexOf(name) + 1];
const timestamp = (name) => ({ nanos: 0, seconds: value(name) });
const schema = await readFile(join(dirname(value("--policy")), "schema.zed"));
const projection = {
  candidate_id: value("--candidate-id"),
  compiler_version: "1.54.0",
  created_at: timestamp("--created-at"),
  expected_approval_epoch: value("--expected-approval-epoch"),
  expires_at: timestamp("--expires-at"),
  permission: value("--permission"),
  policy_hash: "ab".repeat(32),
  policy_name: value("--policy-name"),
  policy_version: value("--policy-version"),
  record_id: value("--record-id"),
  resource_type: value("--resource-type"),
  rule_summary: value("--rule-summary"),
  source_schema_digest: createHash("sha256").update(schema).digest("hex"),
  status: "COMPILED",
  valid_from: timestamp("--valid-from"),
  valid_until: timestamp("--valid-until")
};
await writeFile(value("--output"), JSON.stringify(projection) + "\\n", { mode: 0o600 });
await writeFile(value("--state-output"), Buffer.from([2]), { mode: 0o600 });
`;
}

function cookiePair(setCookie: string): string {
  const pair = setCookie.split(";", 1)[0];
  if (!pair) {
    throw new Error("missing cookie pair");
  }
  return pair;
}

function requiredHeader(response: Response, name: string): string {
  const value = response.headers.get(name);
  if (!value) {
    throw new Error(`missing ${name}`);
  }
  return value;
}

function restoreEnvironment(): void {
  for (const name of environmentNames) {
    const value = originalEnvironment[name];
    if (value === undefined) {
      delete process.env[name];
    } else {
      process.env[name] = value;
    }
  }
}
