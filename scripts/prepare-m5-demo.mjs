#!/usr/bin/env node

import { randomBytes, scrypt as scryptCallback } from "node:crypto";
import { constants } from "node:fs";
import { chmod, lstat, mkdir, open, readFile, rename, unlink } from "node:fs/promises";
import { dirname, isAbsolute, normalize } from "node:path";
import { promisify } from "node:util";

const scrypt = promisify(scryptCallback);
const secretPath = process.env.RAZIEL_OPERATOR_SECRET_PATH ?? "";
const password = process.env.RAZIEL_M5_OPERATOR_PASSWORD ?? "";
const operatorId = process.env.RAZIEL_M5_OPERATOR_ID ?? "operator-local";
const reset = process.env.RAZIEL_M5_RESET_OPERATOR === "1";
const operatorIdPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/;

if (
  !isAbsolute(secretPath) ||
  normalize(secretPath) !== secretPath ||
  secretPath === "/" ||
  !operatorIdPattern.test(operatorId) ||
  Buffer.byteLength(password, "utf8") < 12 ||
  Buffer.byteLength(password, "utf8") > 256
) {
  throw new Error("invalid M5 operator provisioning configuration");
}

const directory = dirname(secretPath);
await mkdir(directory, { recursive: true, mode: 0o700 });
await chmod(directory, 0o700);

const existingOperatorId = reset ? null : await existingSecretOperatorId(secretPath);
if (existingOperatorId !== null) {
  if (existingOperatorId !== operatorId) {
    throw new Error("existing M5 operator ID differs; set RAZIEL_M5_RESET_OPERATOR=1");
  }
  console.log(`m5_operator_secret=present operator_id=${existingOperatorId}`);
  process.exit(0);
}

const salt = randomBytes(16);
const verifier = Buffer.from(
  await scrypt(password, salt, 32, {
    N: 32_768,
    r: 8,
    p: 1,
    maxmem: 64 * 1024 * 1024,
  }),
);
const record = {
  format_version: 1,
  operator_id: operatorId,
  password_salt_base64url: salt.toString("base64url"),
  password_scrypt_base64url: verifier.toString("base64url"),
  session_hmac_key_base64url: randomBytes(32).toString("base64url"),
};
const temporaryPath = `${secretPath}.tmp-${process.pid}`;
let handle;
try {
  handle = await open(
    temporaryPath,
    constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY,
    0o600,
  );
  await handle.writeFile(`${JSON.stringify(record)}\n`, "utf8");
  await handle.sync();
  await handle.close();
  handle = undefined;
  await rename(temporaryPath, secretPath);
  await chmod(secretPath, 0o600);
} catch (error) {
  await handle?.close();
  await unlink(temporaryPath).catch(() => undefined);
  throw error;
}

console.log(`m5_operator_secret=created operator_id=${operatorId}`);

async function existingSecretOperatorId(path) {
  try {
    const metadata = await lstat(path);
    const permissions = metadata.mode & 0o777;
    if (
      !metadata.isFile() ||
      metadata.isSymbolicLink() ||
      metadata.nlink !== 1 ||
      (permissions !== 0o400 && permissions !== 0o600) ||
      metadata.size <= 0 ||
      metadata.size > 4096
    ) {
      throw new Error("existing M5 operator secret is not owner-only");
    }
    const record = JSON.parse(await readFile(path, "utf8"));
    if (
      typeof record !== "object" ||
      record === null ||
      Array.isArray(record) ||
      typeof record.operator_id !== "string" ||
      !operatorIdPattern.test(record.operator_id)
    ) {
      throw new Error("existing M5 operator secret is invalid");
    }
    return record.operator_id;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}
