export const OPERATOR_SECRET_FORMAT_VERSION = 1 as const;
export const OPERATOR_SESSION_FORMAT_VERSION = 1 as const;
export const OPERATOR_SESSION_LIFETIME_SECONDS = 8 * 60 * 60;
export const OPERATOR_CSRF_HEADER = "x-raziel-csrf-token";

export type OperatorAuthFailureCode =
  | "INVALID_OPERATOR_REQUEST"
  | "OPERATOR_CSRF_REJECTED"
  | "OPERATOR_ORIGIN_REJECTED"
  | "OPERATOR_UNAUTHORIZED"
  | "OPERATOR_UNAVAILABLE";

export interface OperatorAuthConfig {
  readonly secretPath: string;
  readonly expectedOrigin: string;
  readonly liveMode: boolean;
}

export interface OperatorSecret {
  readonly operatorId: string;
  readonly passwordSalt: Buffer;
  readonly passwordVerifier: Buffer;
  readonly sessionHmacKey: Buffer;
}

export interface OperatorSession {
  readonly operatorId: string;
  readonly sessionId: string;
  readonly csrfToken: string;
  readonly issuedAtUnixSeconds: number;
  readonly expiresAtUnixSeconds: number;
}

export interface OperatorSessionProjection {
  readonly operatorId: string;
  readonly csrfToken: string;
  readonly expiresAtUnixSeconds: number;
}

export interface IssuedOperatorSession {
  readonly session: OperatorSessionProjection;
  readonly setCookie: string;
}

export interface OperatorAuthDependencies {
  readonly nowUnixSeconds?: () => number;
  readonly randomBytes?: (size: number) => Buffer;
}

export class OperatorAuthError extends Error {
  public readonly code: OperatorAuthFailureCode;

  public constructor(code: OperatorAuthFailureCode) {
    super(code);
    this.name = "OperatorAuthError";
    this.code = code;
  }
}
