import { chmod, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { POST as loginRoute } from "../app/api/operator/login/route.ts";
import { POST as logoutRoute } from "../app/api/operator/logout/route.ts";
import { GET as sessionRoute } from "../app/api/operator/session/route.ts";
import {
  derivePasswordVerifierForProvisioning,
  loadOperatorAuthConfig,
  loadOperatorSecret,
  OPERATOR_CSRF_HEADER,
  OPERATOR_SESSION_LIFETIME_SECONDS,
  type OperatorAuthConfig,
  OperatorAuthError,
  OperatorAuthService,
  readLoginPassword,
} from "../lib/operator-auth/index.ts";

const NOW = 1_800_000_000;
const ORIGIN = "http://127.0.0.1:3000";
const PASSWORD = "correct horse battery staple";
const roots: string[] = [];
const originalEnvironment = {
  path: process.env.RAZIEL_OPERATOR_SECRET_PATH,
  origin: process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN,
  live: process.env.RAZIEL_OPERATOR_LIVE_MODE,
};

afterEach(async () => {
  restoreEnvironment();
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

describe("M5 operator authentication", () => {
  it("issues and verifies a strict local session with session-bound CSRF", async () => {
    const fixture = await createFixture();
    const service = serviceFor(fixture.config, NOW);
    const issued = await service.login(PASSWORD);

    expect(issued.session).toMatchObject({
      operatorId: "operator-m5",
      expiresAtUnixSeconds: NOW + OPERATOR_SESSION_LIFETIME_SECONDS,
    });
    expect(issued.session.csrfToken).toMatch(/^[A-Za-z0-9_-]+$/);
    expect(issued.setCookie).toContain("raziel_operator=");
    expect(issued.setCookie).toContain("HttpOnly");
    expect(issued.setCookie).toContain("SameSite=Strict");
    expect(issued.setCookie).toContain("Path=/");
    expect(issued.setCookie).not.toContain("Secure");

    const cookie = cookiePair(issued.setCookie);
    const verified = await service.verify(request("/api/operator/session", { cookie }));
    expect(verified).toEqual(issued.session);
    await expect(
      serviceFor(fixture.config, NOW + OPERATOR_SESSION_LIFETIME_SECONDS).verify(
        request("/api/operator/session", { cookie }),
      ),
    ).rejects.toMatchObject({ code: "OPERATOR_UNAUTHORIZED" });

    const mutation = request("/api/owner/action", {
      cookie,
      origin: ORIGIN,
      csrf: issued.session.csrfToken,
      method: "POST",
    });
    expect(await service.authorizeMutation(mutation)).toEqual(issued.session);
  });

  it("rejects wrong credentials, altered cookies, Origin changes, and CSRF changes", async () => {
    const fixture = await createFixture();
    const service = serviceFor(fixture.config, NOW);
    await expect(service.login("wrong password")).rejects.toMatchObject({
      code: "OPERATOR_UNAUTHORIZED",
    });
    const issued = await service.login(PASSWORD);
    const cookie = cookiePair(issued.setCookie);
    const altered = `${cookie.slice(0, -1)}${cookie.endsWith("A") ? "B" : "A"}`;
    await expect(
      service.verify(request("/api/operator/session", { cookie: altered })),
    ).rejects.toMatchObject({ code: "OPERATOR_UNAUTHORIZED" });
    await expect(
      service.authorizeMutation(
        request("/api/owner/action", {
          cookie,
          origin: "http://127.0.0.1:3999",
          csrf: issued.session.csrfToken,
          method: "POST",
        }),
      ),
    ).rejects.toMatchObject({ code: "OPERATOR_ORIGIN_REJECTED" });
    await expect(
      service.authorizeMutation(
        request("/api/owner/action", {
          cookie,
          origin: ORIGIN,
          csrf: `${issued.session.csrfToken}x`,
          method: "POST",
        }),
      ),
    ).rejects.toMatchObject({ code: "OPERATOR_CSRF_REJECTED" });
  });

  it("uses a secure host cookie in live mode", async () => {
    const fixture = await createFixture();
    const service = serviceFor({ ...fixture.config, liveMode: true }, NOW);
    const issued = await service.login(PASSWORD);
    expect(issued.setCookie).toContain("__Host-raziel_operator=");
    expect(issued.setCookie).toContain("; Secure");
  });

  it("rejects non-private, linked, malformed, and path-selected secret records", async () => {
    const fixture = await createFixture();
    await chmod(fixture.config.secretPath, 0o644);
    await expect(loadOperatorSecret(fixture.config.secretPath)).rejects.toMatchObject({
      code: "OPERATOR_UNAVAILABLE",
    });

    await chmod(fixture.config.secretPath, 0o600);
    const linked = join(fixture.root, "linked-secret.json");
    await symlink(fixture.config.secretPath, linked);
    await expect(loadOperatorSecret(linked)).rejects.toMatchObject({
      code: "OPERATOR_UNAVAILABLE",
    });

    expect(() =>
      loadOperatorAuthConfig({
        ...process.env,
        RAZIEL_OPERATOR_SECRET_PATH: "relative/secret.json",
        RAZIEL_OPERATOR_EXPECTED_ORIGIN: ORIGIN,
        RAZIEL_OPERATOR_LIVE_MODE: "false",
      }),
    ).toThrow(OperatorAuthError);
  });

  it("accepts only one bounded login password field", async () => {
    await expect(
      readLoginPassword(
        new Request(`${ORIGIN}/api/operator/login`, {
          method: "POST",
          headers: { "content-length": "2048" },
          body: JSON.stringify({ password: PASSWORD }),
        }),
      ),
    ).rejects.toMatchObject({ code: "INVALID_OPERATOR_REQUEST" });
    await expect(
      readLoginPassword(
        new Request(`${ORIGIN}/api/operator/login`, {
          method: "POST",
          body: JSON.stringify({ password: PASSWORD, operatorId: "operator-m5" }),
        }),
      ),
    ).rejects.toMatchObject({ code: "INVALID_OPERATOR_REQUEST" });
  });

  it("runs login, session, and CSRF-protected logout through the Next routes", async () => {
    const fixture = await createFixture();
    process.env.RAZIEL_OPERATOR_SECRET_PATH = fixture.config.secretPath;
    process.env.RAZIEL_OPERATOR_EXPECTED_ORIGIN = ORIGIN;
    process.env.RAZIEL_OPERATOR_LIVE_MODE = "false";

    const login = await loginRoute(
      new Request(`${ORIGIN}/api/operator/login`, {
        method: "POST",
        headers: { origin: ORIGIN },
        body: JSON.stringify({ password: PASSWORD }),
      }),
    );
    expect(login.status).toBe(200);
    const loginBody = (await login.json()) as {
      readonly operatorId: string;
      readonly csrfToken: string;
    };
    expect(loginBody.operatorId).toBe("operator-m5");
    const cookie = cookiePair(requiredHeader(login, "set-cookie"));

    const session = await sessionRoute(request("/api/operator/session", { cookie }));
    expect(session.status).toBe(200);
    expect(await session.json()).toMatchObject({ operatorId: "operator-m5" });

    const logout = await logoutRoute(
      request("/api/operator/logout", {
        cookie,
        origin: ORIGIN,
        csrf: loginBody.csrfToken,
        method: "POST",
      }),
    );
    expect(logout.status).toBe(200);
    expect(requiredHeader(logout, "set-cookie")).toContain("Max-Age=0");

    const rejectedOrigin = await loginRoute(
      new Request(`${ORIGIN}/api/operator/login`, {
        method: "POST",
        headers: { origin: "https://example.invalid" },
        body: JSON.stringify({ password: PASSWORD }),
      }),
    );
    expect(rejectedOrigin.status).toBe(403);
    expect(await rejectedOrigin.json()).toEqual({ error: "OPERATOR_ORIGIN_REJECTED" });
  });

  it("accepts a mutation from the request's actual same origin", async () => {
    const fixture = await createFixture();
    const service = serviceFor(fixture.config, NOW);
    const issued = await service.login(PASSWORD);
    const localhostOrigin = "http://localhost:3000";
    const headers = new Headers({
      cookie: cookiePair(issued.setCookie),
      origin: localhostOrigin,
      [OPERATOR_CSRF_HEADER]: issued.session.csrfToken,
    });

    await expect(
      service.authorizeMutation(
        new Request(`${localhostOrigin}/api/owner/action`, { method: "POST", headers }),
      ),
    ).resolves.toEqual(issued.session);
  });
});

async function createFixture(): Promise<{
  readonly root: string;
  readonly config: OperatorAuthConfig;
}> {
  const root = await mkdtemp(join(tmpdir(), "raziel-operator-auth-test-"));
  roots.push(root);
  await chmod(root, 0o700);
  const secretPath = join(root, "operator-secret.json");
  const salt = Buffer.alloc(16, 0x31);
  const verifier = await derivePasswordVerifierForProvisioning(PASSWORD, salt);
  await writeFile(
    secretPath,
    `${JSON.stringify({
      format_version: 1,
      operator_id: "operator-m5",
      password_salt_base64url: salt.toString("base64url"),
      password_scrypt_base64url: verifier.toString("base64url"),
      session_hmac_key_base64url: Buffer.alloc(32, 0x42).toString("base64url"),
    })}\n`,
    { mode: 0o600 },
  );
  return {
    root,
    config: { secretPath, expectedOrigin: ORIGIN, liveMode: false },
  };
}

function serviceFor(config: OperatorAuthConfig, nowUnixSeconds: number): OperatorAuthService {
  let randomCall = 0;
  return new OperatorAuthService(config, {
    nowUnixSeconds: () => nowUnixSeconds,
    randomBytes: (size) => {
      randomCall += 1;
      return Buffer.alloc(size, randomCall);
    },
  });
}

function request(
  path: string,
  options: {
    readonly cookie?: string;
    readonly origin?: string;
    readonly csrf?: string;
    readonly method?: string;
  },
): Request {
  const headers = new Headers();
  if (options.cookie) {
    headers.set("cookie", options.cookie);
  }
  if (options.origin) {
    headers.set("origin", options.origin);
  }
  if (options.csrf) {
    headers.set(OPERATOR_CSRF_HEADER, options.csrf);
  }
  return new Request(`${ORIGIN}${path}`, { method: options.method ?? "GET", headers });
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
  restore("RAZIEL_OPERATOR_SECRET_PATH", originalEnvironment.path);
  restore("RAZIEL_OPERATOR_EXPECTED_ORIGIN", originalEnvironment.origin);
  restore("RAZIEL_OPERATOR_LIVE_MODE", originalEnvironment.live);
}

function restore(name: string, value: string | undefined): void {
  if (value === undefined) {
    delete process.env[name];
  } else {
    process.env[name] = value;
  }
}
