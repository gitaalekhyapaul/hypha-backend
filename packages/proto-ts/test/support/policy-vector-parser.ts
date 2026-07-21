import { create } from "@bufbuild/protobuf";

import {
  AllowedSubjectTypeSchema,
  type CaveatBinaryExpr,
  CaveatBinaryExprSchema,
  CaveatChildrenSchema,
  CaveatDefinitionSchema,
  type CaveatExpr,
  CaveatExprSchema,
  CaveatInputRefSchema,
  CaveatInputSource,
  CaveatParameterSchema,
  CaveatUnaryExprSchema,
  CaveatValueType,
  ObjectDefinitionSchema,
  PermissionArrowSchema,
  PermissionCaveatedRelationSchema,
  PermissionChildrenSchema,
  PermissionDefinitionSchema,
  type PermissionExpr,
  PermissionExprSchema,
  PermissionRelationRefSchema,
  PermissionSubjectRelationRefSchema,
  PolicyLimitsSchema,
  type PolicyProgram,
  PolicyProgramSchema,
  RelationDefinitionSchema,
} from "../../src/gen/raziel/v1/policy_ir_pb.ts";

export const POLICY_HASH_DOMAIN = "raziel/policy/v1";
export const CANONICAL_POLICY_ENCODING_VERSION = 1;

const POLICY_IR_VERSION = 1;
const SOURCE_SCHEMA_DIGEST_BYTES = 32;
const MAX_VECTOR_ITEMS = 65_536;
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const DOMAIN_BYTES = new TextEncoder().encode(POLICY_HASH_DOMAIN);
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const U64_MODULUS = 1n << 64n;

export interface ParsedPolicyHashPreimage {
  readonly domain: typeof POLICY_HASH_DOMAIN;
  readonly payload: Uint8Array;
  readonly policy: PolicyProgram;
}

export interface I64PrimitiveCase {
  readonly canonical_hex: string;
  readonly value: string;
}

export interface Utf8PrimitiveCase {
  readonly canonical_hex: string;
  readonly id: string;
  readonly value: string;
}

export interface OptionalStringPrimitiveCase {
  readonly canonical_hex: string;
  readonly id: string;
  readonly value: string | null;
}

export interface PolicyVectorCase {
  readonly canonical_encoding_version: 1;
  readonly canonical_payload_byte_length: number;
  readonly canonical_payload_hex: string;
  readonly domain_separator: typeof POLICY_HASH_DOMAIN;
  readonly expected_digest_hex: string;
  readonly expected_rust_result: "match";
  readonly expected_typescript_parser_result: "match_human_input";
  readonly hash_algorithm: "sha256";
  readonly hash_preimage_byte_length: number;
  readonly hash_preimage_hex: string;
  readonly human_input_path: string;
  readonly id: string;
  readonly wire_input_path: string;
}

export type MutationOperation = "append" | "replace" | "truncate";
export type MutationTarget = "canonical_payload" | "digest" | "hash_preimage" | "protobuf";

export interface VectorMutation {
  readonly offset: number;
  readonly operation: MutationOperation;
  readonly remove_bytes: number;
  readonly value_hex: string;
}

export interface NegativeVectorCase {
  readonly expected_rust_result:
    | "mismatch"
    | "noncanonical_protobuf"
    | "not_applicable"
    | "not_strictly_sorted_definitions"
    | "unknown_input_source"
    | "unknown_field"
    | "unknown_value_type"
    | "unsupported_policy_ir_version";
  readonly expected_typescript_parser_result: "not_applicable" | "not_checked" | "reject";
  readonly id: string;
  readonly mutation: VectorMutation;
  readonly target: MutationTarget;
}

export interface PolicyVectorDocument {
  readonly negative_cases: readonly NegativeVectorCase[];
  readonly policy_cases: readonly PolicyVectorCase[];
  readonly primitive_cases: {
    readonly i64: readonly I64PrimitiveCase[];
    readonly optional_string: readonly OptionalStringPrimitiveCase[];
    readonly utf8_string: readonly Utf8PrimitiveCase[];
  };
  readonly vector_format_version: 1;
}

class CanonicalReader {
  private offset = 0;

  public constructor(private readonly bytes: Uint8Array) {}

  public get remaining(): number {
    return this.bytes.length - this.offset;
  }

  public readU8(label: string): number {
    const bytes = this.readBytes(1, label);
    const value = bytes[0];
    if (value === undefined) {
      throw new Error(`${label}: missing byte`);
    }
    return value;
  }

  public readU32(label: string): number {
    const bytes = this.readBytes(4, label);
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, false);
  }

  public readI64(label: string): bigint {
    const bytes = this.readBytes(8, label);
    let value = 0n;
    for (const byte of bytes) {
      value = (value << 8n) | BigInt(byte);
    }
    return bytes[0] !== undefined && (bytes[0] & 0x80) !== 0 ? value - U64_MODULUS : value;
  }

  public readBool(label: string): boolean {
    const tag = this.readU8(label);
    if (tag === 0) {
      return false;
    }
    if (tag === 1) {
      return true;
    }
    throw new Error(`${label}: unsupported Boolean tag ${tag}`);
  }

  public readLengthDelimitedBytes(label: string): Uint8Array {
    const length = this.readU32(`${label}.byte_length`);
    return this.readBytes(length, label);
  }

  public readString(label: string): string {
    const bytes = this.readLengthDelimitedBytes(label);
    try {
      return UTF8_DECODER.decode(bytes);
    } catch {
      throw new Error(`${label}: invalid UTF-8`);
    }
  }

  public readOptionalString(label: string): string | undefined {
    const tag = this.readU8(`${label}.presence`);
    if (tag === 0) {
      return undefined;
    }
    if (tag === 1) {
      return this.readString(label);
    }
    throw new Error(`${label}: unsupported presence tag ${tag}`);
  }

  public readCollection<T>(label: string, parseItem: (reader: CanonicalReader) => T): T[] {
    const count = this.readU32(`${label}.item_count`);
    if (count > MAX_VECTOR_ITEMS) {
      throw new Error(`${label}: item count ${count} exceeds the test parser ceiling`);
    }
    if (count > this.remaining) {
      throw new Error(`${label}: item count ${count} cannot fit in ${this.remaining} bytes`);
    }

    const values: T[] = [];
    for (let index = 0; index < count; index += 1) {
      const offsetBeforeItem = this.offset;
      values.push(parseItem(this));
      if (this.offset === offsetBeforeItem) {
        throw new Error(`${label}[${index}]: parser consumed no bytes`);
      }
    }
    return values;
  }

  public expectEnd(label: string): void {
    if (this.remaining !== 0) {
      throw new Error(`${label}: ${this.remaining} trailing byte(s)`);
    }
  }

  private readBytes(length: number, label: string): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0) {
      throw new Error(`${label}: invalid byte length ${length}`);
    }
    if (length > this.remaining) {
      throw new Error(`${label}: truncated value, need ${length} bytes, have ${this.remaining}`);
    }
    const start = this.offset;
    this.offset += length;
    return this.bytes.slice(start, this.offset);
  }
}

export function decodeHexStrict(value: unknown, label = "hex"): Uint8Array {
  if (typeof value !== "string") {
    throw new Error(`${label}: expected a string`);
  }
  if (value.length % 2 !== 0) {
    throw new Error(`${label}: expected an even number of characters`);
  }
  if (!/^(?:[0-9a-f]{2})*$/.test(value)) {
    throw new Error(`${label}: expected lowercase hexadecimal bytes`);
  }

  const bytes = new Uint8Array(value.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

export function encodeHexLower(bytes: Uint8Array): string {
  let value = "";
  for (const byte of bytes) {
    value += byte.toString(16).padStart(2, "0");
  }
  return value;
}

export function parsePolicyHashPreimageHex(value: unknown): ParsedPolicyHashPreimage {
  return parsePolicyHashPreimage(decodeHexStrict(value, "hash_preimage_hex"));
}

export function parsePolicyHashPreimage(bytes: Uint8Array): ParsedPolicyHashPreimage {
  const reader = new CanonicalReader(bytes);
  const domain = reader.readLengthDelimitedBytes("hash_preimage.domain");
  if (!bytesEqual(domain, DOMAIN_BYTES)) {
    throw new Error(`hash_preimage.domain: expected exact ASCII ${POLICY_HASH_DOMAIN}`);
  }

  const payload = reader.readLengthDelimitedBytes("hash_preimage.payload");
  reader.expectEnd("hash_preimage");
  return {
    domain: POLICY_HASH_DOMAIN,
    payload,
    policy: parseCanonicalPolicy(payload),
  };
}

export function parseCanonicalPolicyHex(value: unknown): PolicyProgram {
  return parseCanonicalPolicy(decodeHexStrict(value, "canonical_payload_hex"));
}

export function parseCanonicalPolicy(bytes: Uint8Array): PolicyProgram {
  const reader = new CanonicalReader(bytes);
  const canonicalVersion = reader.readU32("policy.canonical_encoding_version");
  if (canonicalVersion !== CANONICAL_POLICY_ENCODING_VERSION) {
    throw new Error(`policy: unsupported canonical encoding version ${canonicalVersion}`);
  }

  const irVersion = reader.readU32("policy.ir_version");
  if (irVersion !== POLICY_IR_VERSION) {
    throw new Error(`policy: unsupported PolicyIR version ${irVersion}`);
  }

  const compilerName = reader.readString("policy.compiler_name");
  const compilerVersion = reader.readString("policy.compiler_version");
  const sourceSchemaDigest = reader.readLengthDelimitedBytes("policy.source_schema_digest");
  if (sourceSchemaDigest.length !== SOURCE_SCHEMA_DIGEST_BYTES) {
    throw new Error(
      `policy.source_schema_digest: expected ${SOURCE_SCHEMA_DIGEST_BYTES} bytes, got ${sourceSchemaDigest.length}`,
    );
  }

  const definitions = reader.readCollection("policy.definitions", parseObjectDefinition);
  const permissions = reader.readCollection("policy.permissions", parsePermissionDefinition);
  const caveats = reader.readCollection("policy.caveats", parseCaveatDefinition);
  const limits = parsePolicyLimits(reader);
  reader.expectEnd("canonical policy payload");

  return create(PolicyProgramSchema, {
    irVersion,
    compilerName,
    compilerVersion,
    sourceSchemaDigest,
    definitions,
    permissions,
    caveats,
    limits,
  });
}

export function parseI64Hex(value: unknown): bigint {
  const reader = new CanonicalReader(decodeHexStrict(value, "i64.canonical_hex"));
  const parsed = reader.readI64("i64");
  reader.expectEnd("i64");
  return parsed;
}

export function parseUtf8StringHex(value: unknown): string {
  const reader = new CanonicalReader(decodeHexStrict(value, "utf8_string.canonical_hex"));
  const parsed = reader.readString("utf8_string");
  reader.expectEnd("utf8_string");
  return parsed;
}

export function parseOptionalStringHex(value: unknown): string | undefined {
  const reader = new CanonicalReader(decodeHexStrict(value, "optional_string.canonical_hex"));
  const parsed = reader.readOptionalString("optional_string");
  reader.expectEnd("optional_string");
  return parsed;
}

function parseObjectDefinition(reader: CanonicalReader) {
  return create(ObjectDefinitionSchema, {
    name: reader.readString("object_definition.name"),
    relations: reader.readCollection("object_definition.relations", parseRelationDefinition),
  });
}

function parseRelationDefinition(reader: CanonicalReader) {
  return create(RelationDefinitionSchema, {
    name: reader.readString("relation_definition.name"),
    allowedSubjectTypes: reader.readCollection(
      "relation_definition.allowed_subject_types",
      parseAllowedSubjectType,
    ),
  });
}

function parseAllowedSubjectType(reader: CanonicalReader) {
  const objectType = reader.readString("allowed_subject_type.object_type");
  const subjectRelation = reader.readOptionalString("allowed_subject_type.subject_relation");
  const caveatName = reader.readOptionalString("allowed_subject_type.caveat_name");

  return create(AllowedSubjectTypeSchema, {
    objectType,
    ...(subjectRelation === undefined ? {} : { subjectRelation }),
    ...(caveatName === undefined ? {} : { caveatName }),
  });
}

function parsePermissionDefinition(reader: CanonicalReader) {
  return create(PermissionDefinitionSchema, {
    objectType: reader.readString("permission_definition.object_type"),
    name: reader.readString("permission_definition.name"),
    expression: parsePermissionExpr(reader),
  });
}

function parsePermissionExpr(reader: CanonicalReader): PermissionExpr {
  const tag = reader.readU8("permission_expr.tag");
  switch (tag) {
    case 1:
      return create(PermissionExprSchema, {
        node: {
          case: "relationRef",
          value: create(PermissionRelationRefSchema, {
            name: reader.readString("permission_expr.relation_ref.name"),
          }),
        },
      });
    case 2:
      return create(PermissionExprSchema, {
        node: {
          case: "subjectRelationRef",
          value: create(PermissionSubjectRelationRefSchema, {
            relation: reader.readString("permission_expr.subject_relation_ref.relation"),
            subjectRelation: reader.readString(
              "permission_expr.subject_relation_ref.subject_relation",
            ),
          }),
        },
      });
    case 3:
      return create(PermissionExprSchema, {
        node: {
          case: "anyOf",
          value: create(PermissionChildrenSchema, {
            children: reader.readCollection("permission_expr.any_of", parsePermissionExpr),
          }),
        },
      });
    case 4:
      return create(PermissionExprSchema, {
        node: {
          case: "allOf",
          value: create(PermissionChildrenSchema, {
            children: reader.readCollection("permission_expr.all_of", parsePermissionExpr),
          }),
        },
      });
    case 5:
      return create(PermissionExprSchema, {
        node: {
          case: "arrow",
          value: create(PermissionArrowSchema, {
            sourceRelation: reader.readString("permission_expr.arrow.source_relation"),
            targetPermission: reader.readString("permission_expr.arrow.target_permission"),
          }),
        },
      });
    case 6:
      return create(PermissionExprSchema, {
        node: {
          case: "caveatedRelation",
          value: create(PermissionCaveatedRelationSchema, {
            relation: reader.readString("permission_expr.caveated_relation.relation"),
            caveatName: reader.readString("permission_expr.caveated_relation.caveat_name"),
          }),
        },
      });
    default:
      throw new Error(`permission_expr: unsupported node tag ${tag}`);
  }
}

function parseCaveatDefinition(reader: CanonicalReader) {
  return create(CaveatDefinitionSchema, {
    name: reader.readString("caveat_definition.name"),
    parameters: reader.readCollection("caveat_definition.parameters", parseCaveatParameter),
    expression: parseCaveatExpr(reader),
  });
}

function parseCaveatParameter(reader: CanonicalReader) {
  return create(CaveatParameterSchema, {
    source: parseInputSource(reader, "caveat_parameter.source"),
    name: reader.readString("caveat_parameter.name"),
    valueType: parseValueType(reader, "caveat_parameter.value_type"),
  });
}

function parseCaveatExpr(reader: CanonicalReader): CaveatExpr {
  const tag = reader.readU8("caveat_expr.tag");
  switch (tag) {
    case 1:
      return create(CaveatExprSchema, {
        node: { case: "constBool", value: reader.readBool("caveat_expr.const_bool") },
      });
    case 2:
      return create(CaveatExprSchema, {
        node: { case: "constInt", value: reader.readI64("caveat_expr.const_int") },
      });
    case 3:
      return create(CaveatExprSchema, {
        node: { case: "constString", value: reader.readString("caveat_expr.const_string") },
      });
    case 4:
      return create(CaveatExprSchema, {
        node: { case: "inputRef", value: parseCaveatInputRef(reader) },
      });
    case 5:
      return create(CaveatExprSchema, {
        node: { case: "equal", value: parseCaveatBinaryExpr(reader) },
      });
    case 6:
      return create(CaveatExprSchema, {
        node: { case: "notEqual", value: parseCaveatBinaryExpr(reader) },
      });
    case 7:
      return create(CaveatExprSchema, {
        node: { case: "lessThan", value: parseCaveatBinaryExpr(reader) },
      });
    case 8:
      return create(CaveatExprSchema, {
        node: { case: "lessThanOrEqual", value: parseCaveatBinaryExpr(reader) },
      });
    case 9:
      return create(CaveatExprSchema, {
        node: { case: "greaterThan", value: parseCaveatBinaryExpr(reader) },
      });
    case 10:
      return create(CaveatExprSchema, {
        node: { case: "greaterThanOrEqual", value: parseCaveatBinaryExpr(reader) },
      });
    case 11:
      return create(CaveatExprSchema, {
        node: {
          case: "and",
          value: create(CaveatChildrenSchema, {
            children: reader.readCollection("caveat_expr.and", parseCaveatExpr),
          }),
        },
      });
    case 12:
      return create(CaveatExprSchema, {
        node: {
          case: "or",
          value: create(CaveatChildrenSchema, {
            children: reader.readCollection("caveat_expr.or", parseCaveatExpr),
          }),
        },
      });
    case 13:
      return create(CaveatExprSchema, {
        node: {
          case: "not",
          value: create(CaveatUnaryExprSchema, { child: parseCaveatExpr(reader) }),
        },
      });
    default:
      throw new Error(`caveat_expr: unsupported node tag ${tag}`);
  }
}

function parseCaveatInputRef(reader: CanonicalReader) {
  return create(CaveatInputRefSchema, {
    source: parseInputSource(reader, "caveat_input_ref.source"),
    name: reader.readString("caveat_input_ref.name"),
    valueType: parseValueType(reader, "caveat_input_ref.value_type"),
  });
}

function parseCaveatBinaryExpr(reader: CanonicalReader): CaveatBinaryExpr {
  return create(CaveatBinaryExprSchema, {
    left: parseCaveatExpr(reader),
    right: parseCaveatExpr(reader),
  });
}

function parseInputSource(reader: CanonicalReader, label: string): CaveatInputSource {
  const tag = reader.readU8(label);
  switch (tag) {
    case 1:
      return CaveatInputSource.REQUEST_CONTEXT;
    case 2:
      return CaveatInputSource.RELATIONSHIP_CONTEXT;
    case 3:
      return CaveatInputSource.CAPABILITY_CLAIM;
    default:
      throw new Error(`${label}: unsupported input source tag ${tag}`);
  }
}

function parseValueType(reader: CanonicalReader, label: string): CaveatValueType {
  const tag = reader.readU8(label);
  switch (tag) {
    case 1:
      return CaveatValueType.BOOL;
    case 2:
      return CaveatValueType.INT;
    case 3:
      return CaveatValueType.STRING;
    default:
      throw new Error(`${label}: unsupported value type tag ${tag}`);
  }
}

function parsePolicyLimits(reader: CanonicalReader) {
  return create(PolicyLimitsSchema, {
    maxExpressionNodes: reader.readU32("policy_limits.max_expression_nodes"),
    maxCaveatNodes: reader.readU32("policy_limits.max_caveat_nodes"),
    maxPathDepth: reader.readU32("policy_limits.max_path_depth"),
    maxRelationshipWitnesses: reader.readU32("policy_limits.max_relationship_witnesses"),
    maxMerkleDepth: reader.readU32("policy_limits.max_merkle_depth"),
    maxStringBytes: reader.readU32("policy_limits.max_string_bytes"),
    maxContextEntries: reader.readU32("policy_limits.max_context_entries"),
    maxTotalPrivateInputBytes: reader.readU32("policy_limits.max_total_private_input_bytes"),
  });
}

export function applyByteMutation(input: Uint8Array, mutation: VectorMutation): Uint8Array {
  const replacement = decodeHexStrict(mutation.value_hex, "mutation.value_hex");
  const { offset, remove_bytes: removeBytes, operation } = mutation;
  requireSafeInteger(offset, "mutation.offset");
  requireSafeInteger(removeBytes, "mutation.remove_bytes");

  if (offset > input.length || removeBytes > input.length - offset) {
    throw new Error("mutation: removal range is outside the input");
  }

  switch (operation) {
    case "append":
      if (offset !== input.length || removeBytes !== 0) {
        throw new Error("mutation.append: expected the input end and zero removed bytes");
      }
      break;
    case "replace":
      break;
    case "truncate":
      if (replacement.length !== 0 || offset + removeBytes !== input.length) {
        throw new Error("mutation.truncate: expected an empty replacement of the input suffix");
      }
      break;
    default:
      throw new Error(`mutation: unsupported operation ${String(operation)}`);
  }

  const suffixOffset = offset + removeBytes;
  const output = new Uint8Array(offset + replacement.length + input.length - suffixOffset);
  output.set(input.subarray(0, offset));
  output.set(replacement, offset);
  output.set(input.subarray(suffixOffset), offset + replacement.length);
  return output;
}

export function parsePolicyVectorDocument(json: string): PolicyVectorDocument {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    throw new Error("policy vector: invalid JSON");
  }

  const root = expectRecord(value, "policy vector");
  expectExactKeys(
    root,
    ["negative_cases", "policy_cases", "primitive_cases", "vector_format_version"],
    "policy vector",
  );
  expectLiteral(root.vector_format_version, 1, "policy vector.vector_format_version");

  const primitiveCases = expectRecord(root.primitive_cases, "policy vector.primitive_cases");
  expectExactKeys(
    primitiveCases,
    ["i64", "optional_string", "utf8_string"],
    "policy vector.primitive_cases",
  );

  return {
    negative_cases: expectArray(root.negative_cases, "policy vector.negative_cases").map(
      parseNegativeVectorCase,
    ),
    policy_cases: expectArray(root.policy_cases, "policy vector.policy_cases").map(
      parsePolicyVectorCase,
    ),
    primitive_cases: {
      i64: expectArray(primitiveCases.i64, "policy vector.primitive_cases.i64").map(
        parseI64PrimitiveCase,
      ),
      optional_string: expectArray(
        primitiveCases.optional_string,
        "policy vector.primitive_cases.optional_string",
      ).map(parseOptionalStringPrimitiveCase),
      utf8_string: expectArray(
        primitiveCases.utf8_string,
        "policy vector.primitive_cases.utf8_string",
      ).map(parseUtf8PrimitiveCase),
    },
    vector_format_version: 1,
  };
}

function parsePolicyVectorCase(value: unknown, index: number): PolicyVectorCase {
  const label = `policy vector.policy_cases[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    [
      "canonical_encoding_version",
      "canonical_payload_byte_length",
      "canonical_payload_hex",
      "domain_separator",
      "expected_digest_hex",
      "expected_rust_result",
      "expected_typescript_parser_result",
      "hash_algorithm",
      "hash_preimage_byte_length",
      "hash_preimage_hex",
      "human_input_path",
      "id",
      "wire_input_path",
    ],
    label,
  );

  expectLiteral(record.canonical_encoding_version, 1, `${label}.canonical_encoding_version`);
  expectLiteral(record.domain_separator, POLICY_HASH_DOMAIN, `${label}.domain_separator`);
  expectLiteral(record.expected_rust_result, "match", `${label}.expected_rust_result`);
  expectLiteral(
    record.expected_typescript_parser_result,
    "match_human_input",
    `${label}.expected_typescript_parser_result`,
  );
  expectLiteral(record.hash_algorithm, "sha256", `${label}.hash_algorithm`);

  const canonicalPayloadHex = expectString(
    record.canonical_payload_hex,
    `${label}.canonical_payload_hex`,
  );
  const canonicalPayload = decodeHexStrict(canonicalPayloadHex, `${label}.canonical_payload_hex`);
  const canonicalPayloadByteLength = expectSafeInteger(
    record.canonical_payload_byte_length,
    `${label}.canonical_payload_byte_length`,
  );
  if (canonicalPayload.length !== canonicalPayloadByteLength) {
    throw new Error(`${label}: canonical payload byte length does not match its hex field`);
  }

  const hashPreimageHex = expectString(record.hash_preimage_hex, `${label}.hash_preimage_hex`);
  const hashPreimage = decodeHexStrict(hashPreimageHex, `${label}.hash_preimage_hex`);
  const hashPreimageByteLength = expectSafeInteger(
    record.hash_preimage_byte_length,
    `${label}.hash_preimage_byte_length`,
  );
  if (hashPreimage.length !== hashPreimageByteLength) {
    throw new Error(`${label}: hash preimage byte length does not match its hex field`);
  }

  const expectedDigestHex = expectString(
    record.expected_digest_hex,
    `${label}.expected_digest_hex`,
  );
  if (decodeHexStrict(expectedDigestHex, `${label}.expected_digest_hex`).length !== 32) {
    throw new Error(`${label}.expected_digest_hex: expected 32 bytes`);
  }

  return {
    canonical_encoding_version: 1,
    canonical_payload_byte_length: canonicalPayloadByteLength,
    canonical_payload_hex: canonicalPayloadHex,
    domain_separator: POLICY_HASH_DOMAIN,
    expected_digest_hex: expectedDigestHex,
    expected_rust_result: "match",
    expected_typescript_parser_result: "match_human_input",
    hash_algorithm: "sha256",
    hash_preimage_byte_length: hashPreimageByteLength,
    hash_preimage_hex: hashPreimageHex,
    human_input_path: expectNonemptyString(record.human_input_path, `${label}.human_input_path`),
    id: expectNonemptyString(record.id, `${label}.id`),
    wire_input_path: expectNonemptyString(record.wire_input_path, `${label}.wire_input_path`),
  };
}

function parseNegativeVectorCase(value: unknown, index: number): NegativeVectorCase {
  const label = `policy vector.negative_cases[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    ["expected_rust_result", "expected_typescript_parser_result", "id", "mutation", "target"],
    label,
  );

  return {
    expected_rust_result: expectOneOf(
      record.expected_rust_result,
      [
        "mismatch",
        "noncanonical_protobuf",
        "not_applicable",
        "not_strictly_sorted_definitions",
        "unknown_field",
        "unknown_input_source",
        "unknown_value_type",
        "unsupported_policy_ir_version",
      ] as const,
      `${label}.expected_rust_result`,
    ),
    expected_typescript_parser_result: expectOneOf(
      record.expected_typescript_parser_result,
      ["not_applicable", "not_checked", "reject"] as const,
      `${label}.expected_typescript_parser_result`,
    ),
    id: expectNonemptyString(record.id, `${label}.id`),
    mutation: parseVectorMutation(record.mutation, `${label}.mutation`),
    target: expectOneOf(
      record.target,
      ["canonical_payload", "digest", "hash_preimage", "protobuf"] as const,
      `${label}.target`,
    ),
  };
}

function parseVectorMutation(value: unknown, label: string): VectorMutation {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["offset", "operation", "remove_bytes", "value_hex"], label);
  const valueHex = expectString(record.value_hex, `${label}.value_hex`);
  decodeHexStrict(valueHex, `${label}.value_hex`);
  return {
    offset: expectSafeInteger(record.offset, `${label}.offset`),
    operation: expectOneOf(
      record.operation,
      ["append", "replace", "truncate"] as const,
      `${label}.operation`,
    ),
    remove_bytes: expectSafeInteger(record.remove_bytes, `${label}.remove_bytes`),
    value_hex: valueHex,
  };
}

function parseI64PrimitiveCase(value: unknown, index: number): I64PrimitiveCase {
  const label = `policy vector.primitive_cases.i64[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(record, ["canonical_hex", "value"], label);
  const canonicalHex = expectString(record.canonical_hex, `${label}.canonical_hex`);
  const decimal = expectString(record.value, `${label}.value`);
  if (!/^-?(?:0|[1-9][0-9]*)$/.test(decimal)) {
    throw new Error(`${label}.value: expected a canonical decimal integer`);
  }
  const expected = BigInt(decimal);
  if (expected < I64_MIN || expected > I64_MAX) {
    throw new Error(`${label}.value: outside the signed i64 range`);
  }
  if (parseI64Hex(canonicalHex) !== expected) {
    throw new Error(`${label}: canonical bytes do not match the declared integer`);
  }
  return { canonical_hex: canonicalHex, value: decimal };
}

function parseUtf8PrimitiveCase(value: unknown, index: number): Utf8PrimitiveCase {
  const label = `policy vector.primitive_cases.utf8_string[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(record, ["canonical_hex", "id", "value"], label);
  const canonicalHex = expectString(record.canonical_hex, `${label}.canonical_hex`);
  const declaredValue = expectString(record.value, `${label}.value`);
  if (parseUtf8StringHex(canonicalHex) !== declaredValue) {
    throw new Error(`${label}: canonical bytes do not match the declared string`);
  }
  return {
    canonical_hex: canonicalHex,
    id: expectNonemptyString(record.id, `${label}.id`),
    value: declaredValue,
  };
}

function parseOptionalStringPrimitiveCase(
  value: unknown,
  index: number,
): OptionalStringPrimitiveCase {
  const label = `policy vector.primitive_cases.optional_string[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(record, ["canonical_hex", "id", "value"], label);
  const canonicalHex = expectString(record.canonical_hex, `${label}.canonical_hex`);
  const declaredValue = record.value === null ? null : expectString(record.value, `${label}.value`);
  const parsed = parseOptionalStringHex(canonicalHex);
  if (parsed !== (declaredValue === null ? undefined : declaredValue)) {
    throw new Error(`${label}: canonical bytes do not match the declared optional string`);
  }
  return {
    canonical_hex: canonicalHex,
    id: expectNonemptyString(record.id, `${label}.id`),
    value: declaredValue,
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label}: expected an array`);
  }
  if (value.length > MAX_VECTOR_ITEMS) {
    throw new Error(`${label}: item count exceeds the test parser ceiling`);
  }
  return value;
}

function expectExactKeys(
  record: Record<string, unknown>,
  expected: readonly string[],
  label: string,
): void {
  const actual = Object.keys(record).sort();
  const sortedExpected = [...expected].sort();
  if (
    actual.length !== sortedExpected.length ||
    actual.some((key, index) => key !== sortedExpected[index])
  ) {
    throw new Error(`${label}: unexpected or missing field`);
  }
}

function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    throw new Error(`${label}: expected a string`);
  }
  return value;
}

function expectNonemptyString(value: unknown, label: string): string {
  const parsed = expectString(value, label);
  if (parsed.length === 0) {
    throw new Error(`${label}: expected a nonempty string`);
  }
  return parsed;
}

function expectSafeInteger(value: unknown, label: string): number {
  if (typeof value !== "number") {
    throw new Error(`${label}: expected a number`);
  }
  requireSafeInteger(value, label);
  return value;
}

function requireSafeInteger(value: number, label: string): void {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label}: expected a nonnegative safe integer`);
  }
}

function expectLiteral<T extends string | number>(value: unknown, expected: T, label: string): T {
  if (value !== expected) {
    throw new Error(`${label}: expected ${String(expected)}`);
  }
  return expected;
}

function expectOneOf<const T extends readonly string[]>(
  value: unknown,
  expected: T,
  label: string,
): T[number] {
  if (typeof value !== "string" || !expected.includes(value)) {
    throw new Error(`${label}: unsupported value ${String(value)}`);
  }
  return value as T[number];
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}
