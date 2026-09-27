import type { PolicyVersionIdentity } from "../policy-draft/types.ts";

export interface ApprovedPolicyCatalogueEntry {
  readonly identity: PolicyVersionIdentity;
  readonly recordId: string;
  readonly policyHash: string;
  readonly policyName: string;
  readonly policyVersion: string;
  readonly resourceType: string;
  readonly permission: "query";
  readonly approvalEpoch: string;
  readonly validFromUnixSeconds: string;
  readonly validUntilUnixSeconds: string;
}

export interface ApprovedPolicyCatalogue {
  readonly formatVersion: 1;
  readonly policies: readonly ApprovedPolicyCatalogueEntry[];
}

export class InvalidPolicyCatalogueError extends Error {
  constructor() {
    super("Invalid approved policy catalogue");
    this.name = "InvalidPolicyCatalogueError";
  }
}

export function parseApprovedPolicyCatalogue(input: unknown): ApprovedPolicyCatalogue {
  const value = object(input, ["formatVersion", "policies"]);
  if (value.formatVersion !== 1 || !Array.isArray(value.policies) || value.policies.length > 32)
    throw invalid();
  const policies = value.policies.map(policy);
  if (new Set(policies.map((entry) => entry.identity.versionId)).size !== policies.length)
    throw invalid();
  return { formatVersion: 1, policies };
}

function policy(input: unknown): ApprovedPolicyCatalogueEntry {
  const value = object(input, [
    "identity",
    "recordId",
    "policyHash",
    "policyName",
    "policyVersion",
    "resourceType",
    "permission",
    "approvalEpoch",
    "validFromUnixSeconds",
    "validUntilUnixSeconds",
  ]);
  const identity = object(value.identity, ["familyId", "versionId", "version"]);
  const familyId = hexadecimal(identity.familyId, 32);
  const versionId = hexadecimal(identity.versionId, 32);
  const version = identity.version;
  if (typeof version !== "number" || !Number.isSafeInteger(version) || version < 1) throw invalid();
  const policyName = `family:${familyId}:v${version}`;
  const policyVersion = String(version);
  const resourceType = `dataset_${versionId}`;
  if (
    value.policyName !== policyName ||
    value.policyVersion !== policyVersion ||
    value.resourceType !== resourceType ||
    value.permission !== "query"
  )
    throw invalid();
  const validFromUnixSeconds = timestamp(value.validFromUnixSeconds);
  const validUntilUnixSeconds = timestamp(value.validUntilUnixSeconds);
  if (BigInt(validUntilUnixSeconds) <= BigInt(validFromUnixSeconds)) throw invalid();
  return {
    identity: { familyId, versionId, version },
    recordId: recordId(value.recordId),
    policyHash: hexadecimal(value.policyHash, 64),
    policyName,
    policyVersion,
    resourceType,
    permission: "query",
    approvalEpoch: decimal(value.approvalEpoch, 18_446_744_073_709_551_615n),
    validFromUnixSeconds,
    validUntilUnixSeconds,
  };
}

function object(input: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) throw invalid();
  const prototype = Object.getPrototypeOf(input);
  if (prototype !== Object.prototype && prototype !== null) throw invalid();
  const actual = Reflect.ownKeys(input);
  if (
    actual.length !== keys.length ||
    actual.some((key) => typeof key !== "string" || !keys.includes(key))
  )
    throw invalid();
  return input as Record<string, unknown>;
}

function recordId(input: unknown): string {
  if (typeof input !== "string" || input.length === 0 || input.length > 256) throw invalid();
  return input;
}

function hexadecimal(input: unknown, length: number): string {
  if (typeof input !== "string" || input.length !== length || !/^[0-9a-f]+$/.test(input))
    throw invalid();
  return input;
}

function decimal(input: unknown, maximum: bigint): string {
  if (typeof input !== "string" || !/^[1-9][0-9]{0,19}$/.test(input) || BigInt(input) > maximum)
    throw invalid();
  return input;
}

function timestamp(input: unknown): string {
  const value = decimal(input, 253_402_300_799n);
  if (value === "0") throw invalid();
  return value;
}

function invalid(): InvalidPolicyCatalogueError {
  return new InvalidPolicyCatalogueError();
}
