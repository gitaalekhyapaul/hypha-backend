import { randomBytes, timingSafeEqual } from "node:crypto";

import { derivePasswordVerifierForProvisioning, loadOperatorSecret } from "./secret.ts";
import { issueSessionToken, verifySessionToken } from "./session.ts";
import {
  type IssuedOperatorSession,
  OPERATOR_CSRF_HEADER,
  OPERATOR_SESSION_LIFETIME_SECONDS,
  type OperatorAuthConfig,
  type OperatorAuthDependencies,
  OperatorAuthError,
  type OperatorSession,
  type OperatorSessionProjection,
} from "./types.ts";

const MAX_PASSWORD_BYTES = 256;
const COOKIE_NAME_LOCAL = "raziel_operator";
const COOKIE_NAME_LIVE = "__Host-raziel_operator";

export class OperatorAuthService {
  readonly #config: OperatorAuthConfig;
  readonly #nowUnixSeconds: () => number;
  readonly #randomBytes: (size: number) => Buffer;

  public constructor(config: OperatorAuthConfig, dependencies: OperatorAuthDependencies = {}) {
    this.#config = config;
    this.#nowUnixSeconds = dependencies.nowUnixSeconds ?? systemNowUnixSeconds;
    this.#randomBytes = dependencies.randomBytes ?? randomBytes;
  }

  public assertExactOrigin(request: Request): void {
    if (request.headers.get("origin") !== this.#config.expectedOrigin) {
      throw new OperatorAuthError("OPERATOR_ORIGIN_REJECTED");
    }
  }

  public async login(password: unknown): Promise<IssuedOperatorSession> {
    if (
      typeof password !== "string" ||
      password.length === 0 ||
      Buffer.byteLength(password, "utf8") > MAX_PASSWORD_BYTES
    ) {
      throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
    }
    const secret = await loadOperatorSecret(this.#config.secretPath);
    const suppliedVerifier = await derivePasswordVerifierForProvisioning(
      password,
      secret.passwordSalt,
    ).catch(() => {
      throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
    });
    if (
      suppliedVerifier.byteLength !== secret.passwordVerifier.byteLength ||
      !timingSafeEqual(suppliedVerifier, secret.passwordVerifier)
    ) {
      throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
    }
    const issued = issueSessionToken(secret, this.#nowUnixSeconds(), this.#randomBytes);
    return {
      session: projectSession(issued.session),
      setCookie: sessionCookie(
        cookieName(this.#config.liveMode),
        issued.token,
        OPERATOR_SESSION_LIFETIME_SECONDS,
        this.#config.liveMode,
      ),
    };
  }

  public async verify(request: Request): Promise<OperatorSessionProjection> {
    return projectSession(await this.#verifiedSession(request));
  }

  public async authorizeMutation(request: Request): Promise<OperatorSessionProjection> {
    this.assertExactOrigin(request);
    const session = await this.#verifiedSession(request);
    const suppliedCsrf = request.headers.get(OPERATOR_CSRF_HEADER);
    if (!suppliedCsrf || !constantTimeTextEqual(suppliedCsrf, session.csrfToken)) {
      throw new OperatorAuthError("OPERATOR_CSRF_REJECTED");
    }
    return projectSession(session);
  }

  public logoutCookie(): string {
    return sessionCookie(cookieName(this.#config.liveMode), "", 0, this.#config.liveMode);
  }

  async #verifiedSession(request: Request): Promise<OperatorSession> {
    const secret = await loadOperatorSecret(this.#config.secretPath);
    const token = cookieValue(request.headers.get("cookie"), cookieName(this.#config.liveMode));
    return verifySessionToken(token, secret, this.#nowUnixSeconds());
  }
}

function projectSession(session: OperatorSession): OperatorSessionProjection {
  return {
    operatorId: session.operatorId,
    csrfToken: session.csrfToken,
    expiresAtUnixSeconds: session.expiresAtUnixSeconds,
  };
}

function cookieName(liveMode: boolean): string {
  return liveMode ? COOKIE_NAME_LIVE : COOKIE_NAME_LOCAL;
}

function sessionCookie(name: string, value: string, maxAge: number, secure: boolean): string {
  return [
    `${name}=${value}`,
    "Path=/",
    "HttpOnly",
    "SameSite=Strict",
    `Max-Age=${maxAge}`,
    ...(secure ? ["Secure"] : []),
  ].join("; ");
}

function cookieValue(header: string | null, name: string): string {
  if (!header || Buffer.byteLength(header) > 4 * 1024) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  const values = header
    .split(";")
    .map((part) => part.trim())
    .filter((part) => part.startsWith(`${name}=`))
    .map((part) => part.slice(name.length + 1));
  if (values.length !== 1 || !values[0]) {
    throw new OperatorAuthError("OPERATOR_UNAUTHORIZED");
  }
  return values[0];
}

function constantTimeTextEqual(left: string, right: string): boolean {
  const leftBytes = Buffer.from(left, "utf8");
  const rightBytes = Buffer.from(right, "utf8");
  return leftBytes.byteLength === rightBytes.byteLength && timingSafeEqual(leftBytes, rightBytes);
}

function systemNowUnixSeconds(): number {
  return Math.floor(Date.now() / 1000);
}
