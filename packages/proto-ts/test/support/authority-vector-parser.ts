import { decodeHexStrict } from "./policy-vector-parser.ts";

export const POLICY_SIGNATURE_DOMAIN = "raziel/policy-approval-signature/v1";
export const SNAPSHOT_SIGNATURE_DOMAIN = "raziel/snapshot-descriptor-signature/v1";

export interface AuthorityVectorCase {
  readonly canonical_payload_hex: string;
  readonly claims: Readonly<Record<string, string | number>>;
  readonly domain: string;
  readonly expected_rust_result: "match";
  readonly expected_typescript_parser_result: "match_structure";
  readonly key_id: string;
  readonly public_key_hex: string;
  readonly signature_hex: string;
  readonly signature_message_hex: string;
}

export interface AuthorityNegativeCase {
  readonly expected_rust_result: "claim_rejected" | "invalid_key_id" | "invalid_signature";
  readonly expected_typescript_parser_result: "match_structure";
  readonly target: string;
}

export interface AuthorityVectorDocument {
  readonly hash_algorithm: "sha256";
  readonly negative_cases: readonly AuthorityNegativeCase[];
  readonly policy_approval: AuthorityVectorCase;
  readonly private_test_only: true;
  readonly signature_algorithm: "Ed25519";
  readonly snapshot_descriptor: AuthorityVectorCase;
  readonly u64_cases: readonly {
    readonly canonical_hex: string;
    readonly value: string;
  }[];
  readonly vector_format_version: 1;
  readonly warning: string;
}

class Reader {
  private offset = 0;

  public constructor(private readonly bytes: Uint8Array) {}

  public readU32(label: string): number {
    const bytes = this.readRaw(4, label);
    return new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, false);
  }

  public readU64(label: string): bigint {
    return this.readUnsigned64(label);
  }

  public readI64(label: string): bigint {
    const value = this.readUnsigned64(label);
    return value >= 1n << 63n ? value - (1n << 64n) : value;
  }

  public readBytes(label: string): Uint8Array {
    return this.readRaw(this.readU32(`${label}.length`), label);
  }

  public readString(label: string): string {
    return new TextDecoder("utf-8", { fatal: true }).decode(this.readBytes(label));
  }

  public expectEnd(label: string): void {
    if (this.offset !== this.bytes.length) {
      throw new Error(`${label}: trailing bytes`);
    }
  }

  private readUnsigned64(label: string): bigint {
    let value = 0n;
    for (const byte of this.readRaw(8, label)) {
      value = (value << 8n) | BigInt(byte);
    }
    return value;
  }

  private readRaw(length: number, label: string): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0 || this.offset + length > this.bytes.length) {
      throw new Error(`${label}: truncated`);
    }
    const start = this.offset;
    this.offset += length;
    return this.bytes.slice(start, this.offset);
  }
}

export function parseAuthorityVectorDocument(json: string): AuthorityVectorDocument {
  const root = record(JSON.parse(json), "authority vector");
  exactKeys(
    root,
    [
      "hash_algorithm",
      "negative_cases",
      "policy_approval",
      "private_test_only",
      "signature_algorithm",
      "snapshot_descriptor",
      "u64_cases",
      "vector_format_version",
      "warning",
    ],
    "authority vector",
  );
  literal(root.vector_format_version, 1, "vector_format_version");
  literal(root.private_test_only, true, "private_test_only");
  literal(root.hash_algorithm, "sha256", "hash_algorithm");
  literal(root.signature_algorithm, "Ed25519", "signature_algorithm");

  const policyApproval = parseAuthorityCase(
    root.policy_approval,
    "policy_approval",
    POLICY_SIGNATURE_DOMAIN,
    [
      "approval_epoch",
      "approved_by",
      "format_version",
      "policy_hash_hex",
      "policy_name",
      "policy_version",
      "record_id",
      "signing_key_id",
      "trust_domain_id",
      "valid_from_unix_seconds",
      "valid_until_unix_seconds",
    ],
    parsePolicyPayload,
  );
  const snapshotDescriptor = parseAuthorityCase(
    root.snapshot_descriptor,
    "snapshot_descriptor",
    SNAPSHOT_SIGNATURE_DOMAIN,
    [
      "canonicalization_version",
      "epoch",
      "expires_at_unix_seconds",
      "format_version",
      "issued_at_unix_seconds",
      "policy_hash_hex",
      "policy_record_id",
      "relationship_snapshot_root_hex",
      "signing_key_id",
      "snapshot_id",
      "spicedb_revision",
      "trust_domain_id",
    ],
    parseSnapshotPayload,
  );
  const u64Cases = array(root.u64_cases, "u64_cases").map((item, index) => {
    const value = record(item, `u64_cases[${index}]`);
    exactKeys(value, ["canonical_hex", "value"], `u64_cases[${index}]`);
    const decimal = nonemptyString(value.value, `u64_cases[${index}].value`);
    if (!/^(0|[1-9][0-9]*)$/.test(decimal) || BigInt(decimal) > (1n << 64n) - 1n) {
      throw new Error(`u64_cases[${index}]: invalid unsigned decimal`);
    }
    const canonicalHex = hex(value.canonical_hex, 8, `u64_cases[${index}].canonical_hex`);
    const expected = BigInt(decimal).toString(16).padStart(16, "0");
    if (canonicalHex !== expected) {
      throw new Error(`u64_cases[${index}]: canonical mismatch`);
    }
    return { canonical_hex: canonicalHex, value: decimal };
  });
  const negativeCases = array(root.negative_cases, "negative_cases").map((item, index) => {
    const value = record(item, `negative_cases[${index}]`);
    exactKeys(
      value,
      ["expected_rust_result", "expected_typescript_parser_result", "target"],
      `negative_cases[${index}]`,
    );
    const expected = nonemptyString(value.expected_rust_result, "expected_rust_result");
    if (!["claim_rejected", "invalid_key_id", "invalid_signature"].includes(expected)) {
      throw new Error(`negative_cases[${index}]: invalid Rust result`);
    }
    literal(value.expected_typescript_parser_result, "match_structure", "parser result");
    return {
      expected_rust_result: expected as AuthorityNegativeCase["expected_rust_result"],
      expected_typescript_parser_result: "match_structure" as const,
      target: nonemptyString(value.target, "target"),
    };
  });

  return {
    hash_algorithm: "sha256",
    negative_cases: negativeCases,
    policy_approval: policyApproval,
    private_test_only: true,
    signature_algorithm: "Ed25519",
    snapshot_descriptor: snapshotDescriptor,
    u64_cases: u64Cases,
    vector_format_version: 1,
    warning: nonemptyString(root.warning, "warning"),
  };
}

function parseAuthorityCase(
  input: unknown,
  label: string,
  expectedDomain: string,
  claimKeys: readonly string[],
  parsePayload: (bytes: Uint8Array) => Record<string, string | number>,
): AuthorityVectorCase {
  const value = record(input, label);
  exactKeys(
    value,
    [
      "canonical_payload_hex",
      "claims",
      "domain",
      "expected_rust_result",
      "expected_typescript_parser_result",
      "key_id",
      "public_key_hex",
      "signature_hex",
      "signature_message_hex",
    ],
    label,
  );
  literal(value.domain, expectedDomain, `${label}.domain`);
  literal(value.expected_rust_result, "match", `${label}.expected_rust_result`);
  literal(
    value.expected_typescript_parser_result,
    "match_structure",
    `${label}.expected_typescript_parser_result`,
  );
  const claims = record(value.claims, `${label}.claims`);
  exactKeys(claims, claimKeys, `${label}.claims`);
  const canonicalPayloadHex = variableHex(value.canonical_payload_hex, `${label}.payload`);
  const payload = decodeHexStrict(canonicalPayloadHex);
  const parsedClaims = parsePayload(payload);
  if (stableRecordJson(parsedClaims) !== stableRecordJson(claims)) {
    throw new Error(`${label}: canonical payload does not match claims`);
  }
  const message = decodeHexStrict(variableHex(value.signature_message_hex, `${label}.message`));
  validateFrame(message, expectedDomain, payload, label);
  const keyId = nonemptyString(value.key_id, `${label}.key_id`);
  if (!/^ed25519:[0-9a-f]{64}$/.test(keyId) || claims.signing_key_id !== keyId) {
    throw new Error(`${label}: invalid key id`);
  }

  return {
    canonical_payload_hex: canonicalPayloadHex,
    claims: parsedClaims,
    domain: expectedDomain,
    expected_rust_result: "match",
    expected_typescript_parser_result: "match_structure",
    key_id: keyId,
    public_key_hex: hex(value.public_key_hex, 32, `${label}.public_key_hex`),
    signature_hex: hex(value.signature_hex, 64, `${label}.signature_hex`),
    signature_message_hex: variableHex(value.signature_message_hex, `${label}.message`),
  };
}

function parsePolicyPayload(bytes: Uint8Array): Record<string, string | number> {
  const reader = new Reader(bytes);
  const claims = {
    format_version: reader.readU32("format_version"),
    trust_domain_id: reader.readString("trust_domain_id"),
    record_id: reader.readString("record_id"),
    policy_name: reader.readString("policy_name"),
    policy_version: reader.readString("policy_version"),
    approval_epoch: reader.readU64("approval_epoch").toString(),
    policy_hash_hex: encodeHex(reader.readBytes("policy_hash")),
    valid_from_unix_seconds: reader.readI64("valid_from").toString(),
    valid_until_unix_seconds: reader.readI64("valid_until").toString(),
    approved_by: reader.readString("approved_by"),
    signing_key_id: reader.readString("signing_key_id"),
  };
  reader.expectEnd("policy payload");
  return claims;
}

function parseSnapshotPayload(bytes: Uint8Array): Record<string, string | number> {
  const reader = new Reader(bytes);
  const claims = {
    format_version: reader.readU32("format_version"),
    trust_domain_id: reader.readString("trust_domain_id"),
    snapshot_id: reader.readString("snapshot_id"),
    relationship_snapshot_root_hex: encodeHex(reader.readBytes("relationship_root")),
    policy_record_id: reader.readString("policy_record_id"),
    policy_hash_hex: encodeHex(reader.readBytes("policy_hash")),
    spicedb_revision: reader.readString("spicedb_revision"),
    epoch: reader.readU64("epoch").toString(),
    issued_at_unix_seconds: reader.readI64("issued_at").toString(),
    expires_at_unix_seconds: reader.readI64("expires_at").toString(),
    canonicalization_version: reader.readU32("canonicalization_version"),
    signing_key_id: reader.readString("signing_key_id"),
  };
  reader.expectEnd("snapshot payload");
  return claims;
}

function validateFrame(
  message: Uint8Array,
  expectedDomain: string,
  payload: Uint8Array,
  label: string,
): void {
  const reader = new Reader(message);
  const domain = reader.readString(`${label}.frame.domain`);
  const framedPayload = reader.readBytes(`${label}.frame.payload`);
  reader.expectEnd(`${label}.frame`);
  if (domain !== expectedDomain || encodeHex(framedPayload) !== encodeHex(payload)) {
    throw new Error(`${label}: signature frame mismatch`);
  }
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected object`);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label}: expected array`);
  }
  return value;
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  label: string,
): void {
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(sortedExpected)) {
    throw new Error(`${label}: unexpected fields`);
  }
}

function stableRecordJson(value: Readonly<Record<string, unknown>>): string {
  return JSON.stringify(
    Object.fromEntries(
      Object.entries(value).sort(([left], [right]) => {
        if (left < right) return -1;
        if (left > right) return 1;
        return 0;
      }),
    ),
  );
}

function literal(value: unknown, expected: unknown, label: string): void {
  if (value !== expected) {
    throw new Error(`${label}: unexpected value`);
  }
}

function nonemptyString(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label}: expected nonempty string`);
  }
  return value;
}

function variableHex(value: unknown, label: string): string {
  const decoded = decodeHexStrict(value, label);
  return encodeHex(decoded);
}

function hex(value: unknown, bytes: number, label: string): string {
  const result = variableHex(value, label);
  if (result.length !== bytes * 2) {
    throw new Error(`${label}: wrong byte length`);
  }
  return result;
}

function encodeHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}
