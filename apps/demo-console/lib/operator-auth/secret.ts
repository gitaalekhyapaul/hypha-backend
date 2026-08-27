import { scrypt as scryptCallback } from "node:crypto";
import { constants as fsConstants } from "node:fs";
import { type FileHandle, lstat, open } from "node:fs/promises";
import { isAbsolute, normalize } from "node:path";

import {
  OPERATOR_SECRET_FORMAT_VERSION,
  type OperatorAuthConfig,
  OperatorAuthError,
  type OperatorSecret,
} from "./types.ts";

const MAX_SECRET_BYTES = 4 * 1024;
const PASSWORD_SALT_BYTES = 16;
const PASSWORD_VERIFIER_BYTES = 32;
const SESSION_HMAC_KEY_BYTES = 32;
const OPERATOR_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;
const BASE64URL = /^[A-Za-z0-9_-]+$/;
const SCRYPT_OPTIONS = {
  N: 32_768,
  r: 8,
  p: 1,
  maxmem: 64 * 1024 * 1024,
} as const;

export function loadOperatorAuthConfig(
  environment: NodeJS.ProcessEnv = process.env,
): OperatorAuthConfig {
  const secretPath = environment.RAZIEL_OPERATOR_SECRET_PATH ?? "";
  const expectedOrigin = environment.RAZIEL_OPERATOR_EXPECTED_ORIGIN ?? "";
  const liveModeValue = environment.RAZIEL_OPERATOR_LIVE_MODE ?? "";
  if (
    !safeAbsolutePath(secretPath) ||
    !safeOrigin(expectedOrigin) ||
    (liveModeValue !== "true" && liveModeValue !== "false")
  ) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  return { secretPath, expectedOrigin, liveMode: liveModeValue === "true" };
}

export async function loadOperatorSecret(path: string): Promise<OperatorSecret> {
  let handle: FileHandle | undefined;
  try {
    const pathMetadata = await lstat(path);
    if (
      !pathMetadata.isFile() ||
      pathMetadata.isSymbolicLink() ||
      pathMetadata.nlink !== 1 ||
      !ownerOnlyMode(pathMetadata.mode)
    ) {
      throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
    }
    verifyOwner(pathMetadata.uid);
    handle = await open(path, fsConstants.O_RDONLY | fsConstants.O_NOFOLLOW);
    const handleMetadata = await handle.stat();
    if (
      !handleMetadata.isFile() ||
      handleMetadata.dev !== pathMetadata.dev ||
      handleMetadata.ino !== pathMetadata.ino ||
      handleMetadata.nlink !== 1 ||
      !ownerOnlyMode(handleMetadata.mode) ||
      handleMetadata.size <= 0 ||
      handleMetadata.size > MAX_SECRET_BYTES
    ) {
      throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
    }
    verifyOwner(handleMetadata.uid);
    const bytes = await handle.readFile();
    if (bytes.byteLength !== handleMetadata.size) {
      throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
    }
    return parseOperatorSecret(JSON.parse(bytes.toString("utf8")) as unknown);
  } catch (error) {
    if (error instanceof OperatorAuthError) {
      throw error;
    }
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  } finally {
    await handle?.close();
  }
}

export async function derivePasswordVerifierForProvisioning(
  password: string,
  salt: Buffer,
): Promise<Buffer> {
  if (salt.byteLength !== PASSWORD_SALT_BYTES) {
    throw new OperatorAuthError("INVALID_OPERATOR_REQUEST");
  }
  return new Promise<Buffer>((resolve, reject) => {
    scryptCallback(password, salt, PASSWORD_VERIFIER_BYTES, SCRYPT_OPTIONS, (error, derived) => {
      if (error) {
        reject(new OperatorAuthError("OPERATOR_UNAVAILABLE"));
      } else {
        resolve(Buffer.from(derived));
      }
    });
  });
}

function parseOperatorSecret(input: unknown): OperatorSecret {
  const keys = [
    "format_version",
    "operator_id",
    "password_salt_base64url",
    "password_scrypt_base64url",
    "session_hmac_key_base64url",
  ];
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  const record = input as Record<string, unknown>;
  if (Object.keys(record).length !== keys.length || keys.some((key) => !(key in record))) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  if (
    record.format_version !== OPERATOR_SECRET_FORMAT_VERSION ||
    typeof record.operator_id !== "string" ||
    !OPERATOR_ID.test(record.operator_id)
  ) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  return {
    operatorId: record.operator_id,
    passwordSalt: decodeBase64Url(record.password_salt_base64url, PASSWORD_SALT_BYTES),
    passwordVerifier: decodeBase64Url(record.password_scrypt_base64url, PASSWORD_VERIFIER_BYTES),
    sessionHmacKey: decodeBase64Url(record.session_hmac_key_base64url, SESSION_HMAC_KEY_BYTES),
  };
}

function decodeBase64Url(input: unknown, expectedBytes: number): Buffer {
  if (typeof input !== "string" || !BASE64URL.test(input)) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  const decoded = Buffer.from(input, "base64url");
  if (decoded.byteLength !== expectedBytes || decoded.toString("base64url") !== input) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
  return decoded;
}

function safeAbsolutePath(value: string): boolean {
  return isAbsolute(value) && value !== "/" && normalize(value) === value && !value.includes("\0");
}

function safeOrigin(value: string): boolean {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.origin === value &&
      url.username === "" &&
      url.password === "" &&
      url.pathname === "/" &&
      url.search === "" &&
      url.hash === ""
    );
  } catch {
    return false;
  }
}

function ownerOnlyMode(mode: number): boolean {
  const permissions = mode & 0o777;
  return permissions === 0o400 || permissions === 0o600;
}

function verifyOwner(owner: number): void {
  if (typeof process.getuid === "function" && owner !== process.getuid()) {
    throw new OperatorAuthError("OPERATOR_UNAVAILABLE");
  }
}
