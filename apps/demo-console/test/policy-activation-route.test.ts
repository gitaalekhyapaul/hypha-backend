import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import { POST as activationRoute } from "../app/api/policies/candidates/[candidateId]/activate/route.ts";
import {
  derivePasswordVerifierForProvisioning,
  OPERATOR_CSRF_HEADER,
} from "../lib/operator-auth/index.ts";

const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "activation route password";
const roots: string[] = [];
const environmentNames = [
  "RAZIEL_OPERATOR_SECRET_PATH",
  "RAZIEL_OPERATOR_EXPECTED_ORIGIN",
  "RAZIEL_OPERATOR_LIVE_MODE",
  "RAZIEL_POLICY_ACTIVATION_EXECUTABLE",
  "RAZIEL_POLICY_ACTIVATION_SOCKET_PATH",
] as const;
const originalEnvironment = Object.fromEntries(
  environmentNames.map((name) => [name, process.env[name]]),
) as Record<(typeof environmentNames)[number], string | undefined>;

afterEach(async () => {
  for (const name of environmentNames) {
    const value = originalEnvironment[name];
    if (value === undefined) {
      delete process.env[name];
    } else {
      process.env[name] = value;
    }
  }
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("M5 explicit policy activation route", () => {
  it("publishes one safe active-policy projection", async () => {
    await configureFixture();
    const session = await login();
    const response = await activationRoute(activationRequest(session), {
      params: Promise.resolve({ candidateId: "candidate-activation-001" }),
    });
    expect(response.status).toBe(200);
    const projection = (await response.json()) as Record<string, unknown>;
    expect(projection).toMatchObject({
      record_id: "policy-activation-001",
      policy_name: "reconciliation-query",
      policy_version: "1",
      status: "APPROVED",
      approval_epoch: "1",
      registry_generation: "2",
    });
    for (const privateField of [
      "policy_ir",
      "canonical_policy_ir",
      "authority_signature",
      "signing_key_id",
      "approval_nonce",
      "trust_domain_id",
      "registry_path",
    ]) {
      expect(projection).not.toHaveProperty(privateField);
    }
  });

  it("authenticates before work and rejects invalid candidate references", async () => {
    await configureFixture();
    const unauthorized = new Request(
      `${ORIGIN}/api/policies/candidates/candidate-activation-001/activate`,
      { method: "POST" },
    );
    Object.defineProperty(unauthorized, "text", {
      value: async () => {
        throw new Error("body should not be read");
      },
    });
    const rejected = await activationRoute(unauthorized, {
      params: Promise.resolve({ candidateId: "candidate-activation-001" }),
    });
    expect(rejected.status).toBe(403);

    const session = await login();
    const invalid = await activationRoute(activationRequest(session), {
      params: Promise.resolve({ candidateId: "../private" }),
    });
    expect(invalid.status).toBe(400);
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
  return {
    cookie: requiredHeader(response, "set-cookie").split(";", 1)[0] ?? "",
    csrfToken: body.csrfToken,
  };
}

function activationRequest(session: LoggedInSession): Request {
  return new Request(`${ORIGIN}/api/policies/candidates/candidate-activation-001/activate`, {
    method: "POST",
    headers: {
      cookie: session.cookie,
      origin: ORIGIN,
      [OPERATOR_CSRF_HEADER]: session.csrfToken,
    },
  });
}

async function configureFixture(): Promise<void> {
  const root = await mkdtemp(join(tmpdir(), "raziel-policy-activation-route-"));
  roots.push(root);
  await chmod(root, 0o700);
  const secretPath = join(root, "operator-secret.json");
  const salt = Buffer.alloc(16, 0x31);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: "operator-route",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x41).toString("base64url"),
    })}\n`,
    { mode: 0o600 },
  );
  const executable = join(root, "activate.mjs");
  await writeFile(executable, activationSource(), { mode: 0o700 });
  process.env.RAZIEL_OPERATOR_SECRET_PATH = secretPath;
  process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
  process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";
  process.env.RAZIEL_POLICY_ACTIVATION_EXECUTABLE = executable;
  process.env.RAZIEL_POLICY_ACTIVATION_SOCKET_PATH = join(root, "activation.sock");
}

function activationSource(): string {
  return `#!/usr/bin/env node
const value = (name) => process.argv[process.argv.indexOf(name) + 1];
if (value("--candidate-id") !== "candidate-activation-001" || value("--approved-by") !== "operator-route") process.exit(2);
console.log(JSON.stringify({
  record_id: "policy-activation-001",
  policy_name: "reconciliation-query",
  policy_version: "1",
  policy_hash: "ab".repeat(32),
  approval_epoch: "1",
  status: "APPROVED",
  registered_at: { seconds: "1787760000", nanos: 0 },
  valid_from: { seconds: "1787760000", nanos: 0 },
  valid_until: { seconds: "1787846400", nanos: 0 },
  registry_generation: "2"
}));
`;
}

function requiredHeader(response: Response, name: string): string {
  const value = response.headers.get(name);
  if (!value) {
    throw new Error(`missing ${name}`);
  }
  return value;
}
