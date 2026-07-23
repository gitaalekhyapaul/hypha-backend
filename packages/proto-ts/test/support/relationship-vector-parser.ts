import {
  applyByteMutation,
  decodeHexStrict,
  encodeHexLower,
  type VectorMutation,
} from "./policy-vector-parser.ts";

export const RELATIONSHIP_TREE_VERSION = 1;
export const CANONICAL_RELATIONSHIP_VERSION = 1;
export const RELATIONSHIP_DOMAINS = {
  empty: "raziel/relationship-empty/v1",
  leaf: "raziel/relationship-leaf/v1",
  node: "raziel/relationship-node/v1",
  root: "raziel/relationship-root/v1",
} as const;

const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const MAX_VECTOR_ITEMS = 65_536;
const I64_MIN = -(1n << 63n);
const I64_MAX = (1n << 63n) - 1n;
const U64_MODULUS = 1n << 64n;

export type HumanContextValue =
  | { readonly type: "bool"; readonly value: boolean }
  | { readonly type: "int"; readonly value: string }
  | { readonly type: "string"; readonly value: string };

export interface HumanContextEntry {
  readonly name: string;
  readonly value: HumanContextValue;
}

export interface HumanCaveat {
  readonly context: readonly HumanContextEntry[];
  readonly name: string;
}

export interface HumanRelationship {
  readonly relation: string;
  readonly resource_id: string;
  readonly resource_type: string;
  readonly stored_caveat: HumanCaveat | null;
  readonly subject_id: string;
  readonly subject_relation: string | null;
  readonly subject_type: string;
}

export interface RelationshipVectorCase {
  readonly canonical_byte_length: number;
  readonly canonical_hex: string;
  readonly expected_rust_result: "match";
  readonly expected_typescript_parser_result: "match_human_input";
  readonly human_input: HumanRelationship;
  readonly id: string;
  readonly leaf_digest_hex: string;
  readonly leaf_index: number;
  readonly leaf_preimage_hex: string;
  readonly private_test_only: true;
}

export interface TreeOpeningCase {
  readonly leaf_index: number;
  readonly relationship_id: string;
  readonly sibling_hashes_hex: readonly string[];
}

export interface TreeNodeCase {
  readonly digest_hex: string;
  readonly height: number;
  readonly leaf_count: number;
  readonly leaf_start: number;
  readonly left_digest_hex: string;
  readonly preimage_hex: string;
  readonly right_digest_hex: string;
}

export interface TreeLevelCase {
  readonly digests_hex: readonly string[];
  readonly height: number;
}

export interface RelationshipTreeCase {
  readonly empty_preimage_hex: string | null;
  readonly expected_root_hex: string;
  readonly expected_rust_result: "match";
  readonly expected_typescript_parser_result: "match_structure";
  readonly id: string;
  readonly leaf_count: number;
  readonly leaf_digests_hex: readonly string[];
  readonly node_steps: readonly TreeNodeCase[];
  readonly openings: readonly TreeOpeningCase[];
  readonly relationship_ids: readonly string[];
  readonly root_preimage_hex: string;
  readonly tree_digest_hex: string;
  readonly tree_levels: readonly TreeLevelCase[];
}

export interface RelationshipNegativeCase {
  readonly expected_rust_result: string;
  readonly expected_typescript_parser_result: "not_checked" | "reject";
  readonly id: string;
  readonly mutation: VectorMutation | null;
  readonly target: string;
}

export interface RelationshipVectorDocument {
  readonly canonical_relationship_version: 1;
  readonly domains: typeof RELATIONSHIP_DOMAINS;
  readonly fixed_private_test_salt_hex: string;
  readonly hash_algorithm: "sha256";
  readonly negative_cases: readonly RelationshipNegativeCase[];
  readonly private_test_only: true;
  readonly relationship_cases: readonly RelationshipVectorCase[];
  readonly relationship_tree_version: 1;
  readonly test_ceilings: {
    readonly max_relationship_bytes: number;
    readonly max_relationships: number;
    readonly max_total_canonical_bytes: number;
  };
  readonly tree_cases: readonly RelationshipTreeCase[];
  readonly vector_format_version: 1;
  readonly warning: string;
}

interface ParsedLeafPreimage {
  readonly canonicalRelationship: Uint8Array;
  readonly leafIndex: number;
  readonly salt: Uint8Array;
}

class CanonicalReader {
  private offset = 0;

  public constructor(private readonly bytes: Uint8Array) {}

  public get remaining(): number {
    return this.bytes.length - this.offset;
  }

  public readU8(label: string): number {
    const value = this.readRaw(1, label)[0];
    if (value === undefined) {
      throw new Error(`${label}: missing byte`);
    }
    return value;
  }

  public readU32(label: string): number {
    const bytes = this.readRaw(4, label);
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(0, false);
  }

  public readI64(label: string): bigint {
    const bytes = this.readRaw(8, label);
    let value = 0n;
    for (const byte of bytes) {
      value = (value << 8n) | BigInt(byte);
    }
    return bytes[0] !== undefined && (bytes[0] & 0x80) !== 0 ? value - U64_MODULUS : value;
  }

  public readBool(label: string): boolean {
    const value = this.readU8(label);
    if (value === 0) {
      return false;
    }
    if (value === 1) {
      return true;
    }
    throw new Error(`${label}: unsupported Boolean byte ${value}`);
  }

  public readLengthDelimited(label: string): Uint8Array {
    return this.readRaw(this.readU32(`${label}.byte_length`), label);
  }

  public readString(label: string): string {
    try {
      return UTF8_DECODER.decode(this.readLengthDelimited(label));
    } catch {
      throw new Error(`${label}: invalid UTF-8`);
    }
  }

  public readOptionalString(label: string): string | null {
    const tag = this.readU8(`${label}.presence`);
    if (tag === 0) {
      return null;
    }
    if (tag === 1) {
      return this.readString(label);
    }
    throw new Error(`${label}: unsupported presence tag ${tag}`);
  }

  public readCollection<T>(label: string, parseItem: (reader: CanonicalReader) => T): T[] {
    const count = this.readU32(`${label}.item_count`);
    if (count > MAX_VECTOR_ITEMS || count > this.remaining) {
      throw new Error(`${label}: item count cannot fit in the remaining input`);
    }
    const values: T[] = [];
    for (let index = 0; index < count; index += 1) {
      const before = this.offset;
      values.push(parseItem(this));
      if (before === this.offset) {
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

  private readRaw(length: number, label: string): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0 || length > this.remaining) {
      throw new Error(`${label}: truncated or invalid byte length`);
    }
    const start = this.offset;
    this.offset += length;
    return this.bytes.slice(start, this.offset);
  }
}

export function parseCanonicalRelationshipHex(value: unknown): HumanRelationship {
  return parseCanonicalRelationship(decodeHexStrict(value, "canonical_relationship_hex"));
}

export function parseCanonicalRelationship(bytes: Uint8Array): HumanRelationship {
  const reader = new CanonicalReader(bytes);
  expectVersion(reader.readU32("relationship.version"), "relationship.version");
  const relationship: HumanRelationship = {
    resource_type: reader.readString("relationship.resource_type"),
    resource_id: reader.readString("relationship.resource_id"),
    relation: reader.readString("relationship.relation"),
    subject_type: reader.readString("relationship.subject_type"),
    subject_id: reader.readString("relationship.subject_id"),
    subject_relation: reader.readOptionalString("relationship.subject_relation"),
    stored_caveat: parseOptionalCaveat(reader),
  };
  reader.expectEnd("relationship");
  return relationship;
}

export function parseRelationshipVectorDocument(json: string): RelationshipVectorDocument {
  let value: unknown;
  try {
    value = JSON.parse(json);
  } catch {
    throw new Error("relationship vector: invalid JSON");
  }
  const root = expectRecord(value, "relationship vector");
  expectExactKeys(
    root,
    [
      "canonical_relationship_version",
      "domains",
      "fixed_private_test_salt_hex",
      "hash_algorithm",
      "negative_cases",
      "private_test_only",
      "relationship_cases",
      "relationship_tree_version",
      "test_ceilings",
      "tree_cases",
      "vector_format_version",
      "warning",
    ],
    "relationship vector",
  );
  expectLiteral(root.vector_format_version, 1, "relationship vector.vector_format_version");
  expectLiteral(
    root.canonical_relationship_version,
    CANONICAL_RELATIONSHIP_VERSION,
    "relationship vector.canonical_relationship_version",
  );
  expectLiteral(
    root.relationship_tree_version,
    RELATIONSHIP_TREE_VERSION,
    "relationship vector.relationship_tree_version",
  );
  expectLiteral(root.private_test_only, true, "relationship vector.private_test_only");
  expectLiteral(root.hash_algorithm, "sha256", "relationship vector.hash_algorithm");

  const domains = parseDomains(root.domains);
  const saltHex = expectDigestHex(
    root.fixed_private_test_salt_hex,
    "relationship vector.fixed_private_test_salt_hex",
  );
  const salt = decodeHexStrict(saltHex);
  const relationshipCases = expectArray(
    root.relationship_cases,
    "relationship vector.relationship_cases",
  ).map((item, index) => parseRelationshipCase(item, index, salt));
  const relationships = new Map(
    relationshipCases.map((relationshipCase) => [relationshipCase.id, relationshipCase]),
  );
  if (relationships.size !== relationshipCases.length) {
    throw new Error("relationship vector.relationship_cases: duplicate id");
  }

  const treeCases = expectArray(root.tree_cases, "relationship vector.tree_cases").map(
    (item, index) => parseTreeCase(item, index, salt, relationships),
  );
  const negativeCases = expectArray(root.negative_cases, "relationship vector.negative_cases").map(
    parseNegativeCase,
  );

  return {
    canonical_relationship_version: 1,
    domains,
    fixed_private_test_salt_hex: saltHex,
    hash_algorithm: "sha256",
    negative_cases: negativeCases,
    private_test_only: true,
    relationship_cases: relationshipCases,
    relationship_tree_version: 1,
    test_ceilings: parseTestCeilings(root.test_ceilings),
    tree_cases: treeCases,
    vector_format_version: 1,
    warning: expectNonemptyString(root.warning, "relationship vector.warning"),
  };
}

function parseRelationshipCase(
  value: unknown,
  index: number,
  fixedSalt: Uint8Array,
): RelationshipVectorCase {
  const label = `relationship vector.relationship_cases[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    [
      "canonical_byte_length",
      "canonical_hex",
      "expected_rust_result",
      "expected_typescript_parser_result",
      "human_input",
      "id",
      "leaf_digest_hex",
      "leaf_index",
      "leaf_preimage_hex",
      "private_test_only",
    ],
    label,
  );
  expectLiteral(record.private_test_only, true, `${label}.private_test_only`);
  expectLiteral(record.expected_rust_result, "match", `${label}.expected_rust_result`);
  expectLiteral(
    record.expected_typescript_parser_result,
    "match_human_input",
    `${label}.expected_typescript_parser_result`,
  );
  const humanInput = parseHumanRelationship(record.human_input, `${label}.human_input`);
  const canonicalHex = expectString(record.canonical_hex, `${label}.canonical_hex`);
  const canonical = decodeHexStrict(canonicalHex, `${label}.canonical_hex`);
  const canonicalByteLength = expectSafeInteger(
    record.canonical_byte_length,
    `${label}.canonical_byte_length`,
  );
  if (canonical.length !== canonicalByteLength) {
    throw new Error(`${label}: canonical byte length mismatch`);
  }
  if (!deepEqual(parseCanonicalRelationship(canonical), humanInput)) {
    throw new Error(`${label}: canonical relationship does not match human input`);
  }

  const leafIndex = expectSafeInteger(record.leaf_index, `${label}.leaf_index`);
  const leafPreimageHex = expectString(record.leaf_preimage_hex, `${label}.leaf_preimage_hex`);
  const parsedLeaf = parseLeafPreimage(leafPreimageHex);
  if (
    parsedLeaf.leafIndex !== leafIndex ||
    !bytesEqual(parsedLeaf.salt, fixedSalt) ||
    !bytesEqual(parsedLeaf.canonicalRelationship, canonical)
  ) {
    throw new Error(`${label}: leaf preimage fields do not match the vector case`);
  }

  return {
    canonical_byte_length: canonicalByteLength,
    canonical_hex: canonicalHex,
    expected_rust_result: "match",
    expected_typescript_parser_result: "match_human_input",
    human_input: humanInput,
    id: expectNonemptyString(record.id, `${label}.id`),
    leaf_digest_hex: expectDigestHex(record.leaf_digest_hex, `${label}.leaf_digest_hex`),
    leaf_index: leafIndex,
    leaf_preimage_hex: leafPreimageHex,
    private_test_only: true,
  };
}

function parseTreeCase(
  value: unknown,
  index: number,
  fixedSalt: Uint8Array,
  relationships: ReadonlyMap<string, RelationshipVectorCase>,
): RelationshipTreeCase {
  const label = `relationship vector.tree_cases[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    [
      "empty_preimage_hex",
      "expected_root_hex",
      "expected_rust_result",
      "expected_typescript_parser_result",
      "id",
      "leaf_count",
      "leaf_digests_hex",
      "node_steps",
      "openings",
      "relationship_ids",
      "root_preimage_hex",
      "tree_digest_hex",
      "tree_levels",
    ],
    label,
  );
  expectLiteral(record.expected_rust_result, "match", `${label}.expected_rust_result`);
  expectLiteral(
    record.expected_typescript_parser_result,
    "match_structure",
    `${label}.expected_typescript_parser_result`,
  );
  const leafCount = expectSafeInteger(record.leaf_count, `${label}.leaf_count`);
  const relationshipIds = expectArray(record.relationship_ids, `${label}.relationship_ids`).map(
    (item, itemIndex) => {
      const id = expectNonemptyString(item, `${label}.relationship_ids[${itemIndex}]`);
      if (!relationships.has(id)) {
        throw new Error(`${label}: unknown relationship id`);
      }
      return id;
    },
  );
  const leafDigests = expectArray(record.leaf_digests_hex, `${label}.leaf_digests_hex`).map(
    (item, itemIndex) => expectDigestHex(item, `${label}.leaf_digests_hex[${itemIndex}]`),
  );
  if (relationshipIds.length !== leafCount || leafDigests.length !== leafCount) {
    throw new Error(`${label}: leaf collections do not match leaf_count`);
  }
  for (let leafIndex = 0; leafIndex < leafCount; leafIndex += 1) {
    const relationship = relationships.get(requireItem(relationshipIds, leafIndex, label));
    if (
      relationship === undefined ||
      relationship.leaf_index !== leafIndex ||
      relationship.leaf_digest_hex !== leafDigests[leafIndex]
    ) {
      throw new Error(`${label}: leaf digest does not match its relationship case`);
    }
  }

  const emptyPreimageHex =
    record.empty_preimage_hex === null
      ? null
      : expectString(record.empty_preimage_hex, `${label}.empty_preimage_hex`);
  if ((leafCount === 0) !== (emptyPreimageHex !== null)) {
    throw new Error(`${label}: empty preimage presence does not match leaf_count`);
  }
  if (emptyPreimageHex !== null) {
    parseEmptyPreimage(emptyPreimageHex);
  }

  const nodeSteps = expectArray(record.node_steps, `${label}.node_steps`).map((item, itemIndex) =>
    parseNodeCase(item, `${label}.node_steps[${itemIndex}]`, leafCount),
  );
  const treeLevels = expectArray(record.tree_levels, `${label}.tree_levels`).map(
    (item, itemIndex) => parseTreeLevel(item, `${label}.tree_levels[${itemIndex}]`),
  );
  validateTreeLevels(treeLevels, nodeSteps, label);
  const treeDigestHex = expectDigestHex(record.tree_digest_hex, `${label}.tree_digest_hex`);
  const rootPreimageHex = expectString(record.root_preimage_hex, `${label}.root_preimage_hex`);
  parseRootPreimage(rootPreimageHex, leafCount, fixedSalt, decodeHexStrict(treeDigestHex));

  const openings = expectArray(record.openings, `${label}.openings`).map((item, itemIndex) =>
    parseOpening(item, `${label}.openings[${itemIndex}]`, leafCount, relationshipIds),
  );
  if (openings.length !== leafCount) {
    throw new Error(`${label}: expected one opening per leaf`);
  }

  return {
    empty_preimage_hex: emptyPreimageHex,
    expected_root_hex: expectDigestHex(record.expected_root_hex, `${label}.expected_root_hex`),
    expected_rust_result: "match",
    expected_typescript_parser_result: "match_structure",
    id: expectNonemptyString(record.id, `${label}.id`),
    leaf_count: leafCount,
    leaf_digests_hex: leafDigests,
    node_steps: nodeSteps,
    openings,
    relationship_ids: relationshipIds,
    root_preimage_hex: rootPreimageHex,
    tree_digest_hex: treeDigestHex,
    tree_levels: treeLevels,
  };
}

function parseNodeCase(value: unknown, label: string, totalLeaves: number): TreeNodeCase {
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    [
      "digest_hex",
      "height",
      "leaf_count",
      "leaf_start",
      "left_digest_hex",
      "preimage_hex",
      "right_digest_hex",
    ],
    label,
  );
  const leafStart = expectSafeInteger(record.leaf_start, `${label}.leaf_start`);
  const leafCount = expectSafeInteger(record.leaf_count, `${label}.leaf_count`);
  const height = expectSafeInteger(record.height, `${label}.height`);
  if (leafCount < 2 || height < 1 || leafStart + leafCount > totalLeaves) {
    throw new Error(`${label}: invalid node range or height`);
  }
  const left = expectDigestHex(record.left_digest_hex, `${label}.left_digest_hex`);
  const right = expectDigestHex(record.right_digest_hex, `${label}.right_digest_hex`);
  parseNodePreimage(
    expectString(record.preimage_hex, `${label}.preimage_hex`),
    decodeHexStrict(left),
    decodeHexStrict(right),
  );
  return {
    digest_hex: expectDigestHex(record.digest_hex, `${label}.digest_hex`),
    height,
    leaf_count: leafCount,
    leaf_start: leafStart,
    left_digest_hex: left,
    preimage_hex: expectString(record.preimage_hex, `${label}.preimage_hex`),
    right_digest_hex: right,
  };
}

function parseTreeLevel(value: unknown, label: string): TreeLevelCase {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["digests_hex", "height"], label);
  const height = expectSafeInteger(record.height, `${label}.height`);
  if (height < 1) {
    throw new Error(`${label}.height: expected a positive value`);
  }
  return {
    digests_hex: expectArray(record.digests_hex, `${label}.digests_hex`).map((item, index) =>
      expectDigestHex(item, `${label}.digests_hex[${index}]`),
    ),
    height,
  };
}

function parseOpening(
  value: unknown,
  label: string,
  leafCount: number,
  relationshipIds: readonly string[],
): TreeOpeningCase {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["leaf_index", "relationship_id", "sibling_hashes_hex"], label);
  const leafIndex = expectSafeInteger(record.leaf_index, `${label}.leaf_index`);
  if (leafIndex >= leafCount) {
    throw new Error(`${label}.leaf_index: outside the tree`);
  }
  const relationshipId = expectNonemptyString(record.relationship_id, `${label}.relationship_id`);
  if (relationshipId !== relationshipIds[leafIndex]) {
    throw new Error(`${label}: relationship id does not match the leaf index`);
  }
  const siblings = expectArray(record.sibling_hashes_hex, `${label}.sibling_hashes_hex`).map(
    (item, index) => expectDigestHex(item, `${label}.sibling_hashes_hex[${index}]`),
  );
  if (siblings.length !== derivedPathLength(leafIndex, leafCount)) {
    throw new Error(`${label}: sibling count does not match the derived path`);
  }
  return {
    leaf_index: leafIndex,
    relationship_id: relationshipId,
    sibling_hashes_hex: siblings,
  };
}

function parseNegativeCase(value: unknown, index: number): RelationshipNegativeCase {
  const label = `relationship vector.negative_cases[${index}]`;
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    ["expected_rust_result", "expected_typescript_parser_result", "id", "mutation", "target"],
    label,
  );
  const typescriptResult = expectOneOf(
    record.expected_typescript_parser_result,
    ["not_checked", "reject"] as const,
    `${label}.expected_typescript_parser_result`,
  );
  const mutation =
    record.mutation === null ? null : parseMutation(record.mutation, `${label}.mutation`);
  if ((typescriptResult === "reject") !== (mutation !== null)) {
    throw new Error(`${label}: TypeScript rejection cases require one byte mutation`);
  }
  return {
    expected_rust_result: expectNonemptyString(
      record.expected_rust_result,
      `${label}.expected_rust_result`,
    ),
    expected_typescript_parser_result: typescriptResult,
    id: expectNonemptyString(record.id, `${label}.id`),
    mutation,
    target: expectNonemptyString(record.target, `${label}.target`),
  };
}

function parseMutation(value: unknown, label: string): VectorMutation {
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

function parseHumanRelationship(value: unknown, label: string): HumanRelationship {
  const record = expectRecord(value, label);
  expectExactKeys(
    record,
    [
      "relation",
      "resource_id",
      "resource_type",
      "stored_caveat",
      "subject_id",
      "subject_relation",
      "subject_type",
    ],
    label,
  );
  return {
    relation: expectString(record.relation, `${label}.relation`),
    resource_id: expectString(record.resource_id, `${label}.resource_id`),
    resource_type: expectString(record.resource_type, `${label}.resource_type`),
    stored_caveat:
      record.stored_caveat === null
        ? null
        : parseHumanCaveat(record.stored_caveat, `${label}.stored_caveat`),
    subject_id: expectString(record.subject_id, `${label}.subject_id`),
    subject_relation:
      record.subject_relation === null
        ? null
        : expectString(record.subject_relation, `${label}.subject_relation`),
    subject_type: expectString(record.subject_type, `${label}.subject_type`),
  };
}

function parseHumanCaveat(value: unknown, label: string): HumanCaveat {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["context", "name"], label);
  return {
    context: expectArray(record.context, `${label}.context`).map((item, index) =>
      parseHumanContextEntry(item, `${label}.context[${index}]`),
    ),
    name: expectString(record.name, `${label}.name`),
  };
}

function parseHumanContextEntry(value: unknown, label: string): HumanContextEntry {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["name", "value"], label);
  return {
    name: expectString(record.name, `${label}.name`),
    value: parseHumanContextValue(record.value, `${label}.value`),
  };
}

function parseHumanContextValue(value: unknown, label: string): HumanContextValue {
  const record = expectRecord(value, label);
  expectExactKeys(record, ["type", "value"], label);
  const type = expectOneOf(record.type, ["bool", "int", "string"] as const, `${label}.type`);
  switch (type) {
    case "bool":
      return { type, value: expectBoolean(record.value, `${label}.value`) };
    case "int": {
      const decimal = expectString(record.value, `${label}.value`);
      if (!/^-?(?:0|[1-9][0-9]*)$/.test(decimal)) {
        throw new Error(`${label}.value: expected a canonical decimal integer`);
      }
      const parsed = BigInt(decimal);
      if (parsed < I64_MIN || parsed > I64_MAX) {
        throw new Error(`${label}.value: outside the signed i64 range`);
      }
      return { type, value: decimal };
    }
    case "string":
      return { type, value: expectString(record.value, `${label}.value`) };
  }
}

function parseOptionalCaveat(reader: CanonicalReader): HumanCaveat | null {
  const tag = reader.readU8("relationship.stored_caveat.presence");
  if (tag === 0) {
    return null;
  }
  if (tag !== 1) {
    throw new Error(`relationship.stored_caveat: unsupported presence tag ${tag}`);
  }
  return {
    name: reader.readString("relationship.stored_caveat.name"),
    context: reader.readCollection("relationship.stored_caveat.context", parseContextEntry),
  };
}

function parseContextEntry(reader: CanonicalReader): HumanContextEntry {
  const name = reader.readString("relationship.stored_caveat.context.name");
  const tag = reader.readU8("relationship.stored_caveat.context.value.tag");
  switch (tag) {
    case 1:
      return {
        name,
        value: {
          type: "bool",
          value: reader.readBool("relationship.stored_caveat.context.value.bool"),
        },
      };
    case 2:
      return {
        name,
        value: {
          type: "int",
          value: reader.readI64("relationship.stored_caveat.context.value.int").toString(),
        },
      };
    case 3:
      return {
        name,
        value: {
          type: "string",
          value: reader.readString("relationship.stored_caveat.context.value.string"),
        },
      };
    default:
      throw new Error(`relationship.stored_caveat.context.value: unsupported tag ${tag}`);
  }
}

function parseLeafPreimage(value: string): ParsedLeafPreimage {
  const payload = parseFramedPreimage(value, RELATIONSHIP_DOMAINS.leaf, "leaf preimage");
  const reader = new CanonicalReader(payload);
  expectVersion(reader.readU32("leaf.version"), "leaf.version");
  const leafIndex = reader.readU32("leaf.index");
  const salt = expectLength(reader.readLengthDelimited("leaf.salt"), 32, "leaf.salt");
  const canonicalRelationship = reader.readLengthDelimited("leaf.relationship");
  parseCanonicalRelationship(canonicalRelationship);
  reader.expectEnd("leaf payload");
  return { canonicalRelationship, leafIndex, salt };
}

function parseNodePreimage(
  value: string,
  expectedLeft: Uint8Array,
  expectedRight: Uint8Array,
): void {
  const payload = parseFramedPreimage(value, RELATIONSHIP_DOMAINS.node, "node preimage");
  const reader = new CanonicalReader(payload);
  expectVersion(reader.readU32("node.version"), "node.version");
  const left = expectLength(reader.readLengthDelimited("node.left"), 32, "node.left");
  const right = expectLength(reader.readLengthDelimited("node.right"), 32, "node.right");
  reader.expectEnd("node payload");
  if (!bytesEqual(left, expectedLeft) || !bytesEqual(right, expectedRight)) {
    throw new Error("node preimage: child digests do not match");
  }
}

function parseEmptyPreimage(value: string): void {
  const payload = parseFramedPreimage(value, RELATIONSHIP_DOMAINS.empty, "empty preimage");
  const reader = new CanonicalReader(payload);
  expectVersion(reader.readU32("empty.version"), "empty.version");
  reader.expectEnd("empty payload");
}

function parseRootPreimage(
  value: string,
  expectedCount: number,
  expectedSalt: Uint8Array,
  expectedTreeDigest: Uint8Array,
): void {
  const payload = parseFramedPreimage(value, RELATIONSHIP_DOMAINS.root, "root preimage");
  const reader = new CanonicalReader(payload);
  expectVersion(reader.readU32("root.version"), "root.version");
  const leafCount = reader.readU32("root.leaf_count");
  const salt = expectLength(reader.readLengthDelimited("root.salt"), 32, "root.salt");
  const treeDigest = expectLength(
    reader.readLengthDelimited("root.tree_digest"),
    32,
    "root.tree_digest",
  );
  reader.expectEnd("root payload");
  if (
    leafCount !== expectedCount ||
    !bytesEqual(salt, expectedSalt) ||
    !bytesEqual(treeDigest, expectedTreeDigest)
  ) {
    throw new Error("root preimage: fields do not match the tree case");
  }
}

function parseFramedPreimage(value: string, domain: string, label: string): Uint8Array {
  const reader = new CanonicalReader(decodeHexStrict(value, label));
  const observedDomain = reader.readLengthDelimited(`${label}.domain`);
  if (!bytesEqual(observedDomain, new TextEncoder().encode(domain))) {
    throw new Error(`${label}: domain mismatch`);
  }
  const payload = reader.readLengthDelimited(`${label}.payload`);
  reader.expectEnd(label);
  return payload;
}

function validateTreeLevels(
  levels: readonly TreeLevelCase[],
  nodes: readonly TreeNodeCase[],
  label: string,
): void {
  const expected = new Map<number, string[]>();
  for (const node of nodes) {
    const digests = expected.get(node.height) ?? [];
    digests.push(node.digest_hex);
    expected.set(node.height, digests);
  }
  if (levels.length !== expected.size) {
    throw new Error(`${label}: tree levels do not cover every node height`);
  }
  for (const level of levels) {
    const digests = expected.get(level.height);
    if (digests === undefined || !deepEqual(digests, level.digests_hex)) {
      throw new Error(`${label}: tree level does not match node steps`);
    }
  }
}

function derivedPathLength(mutIndex: number, mutCount: number): number {
  let index = mutIndex;
  let count = mutCount;
  let depth = 0;
  while (count > 1) {
    const split = largestPowerOfTwoLessThan(count);
    if (index < split) {
      count = split;
    } else {
      index -= split;
      count -= split;
    }
    depth += 1;
  }
  return depth;
}

function largestPowerOfTwoLessThan(value: number): number {
  if (!Number.isSafeInteger(value) || value <= 1) {
    throw new Error("tree count must exceed one");
  }
  let power = 1;
  while (power * 2 < value) {
    power *= 2;
  }
  return power;
}

function parseDomains(value: unknown): typeof RELATIONSHIP_DOMAINS {
  const record = expectRecord(value, "relationship vector.domains");
  expectExactKeys(record, ["empty", "leaf", "node", "root"], "relationship vector.domains");
  for (const [name, expected] of Object.entries(RELATIONSHIP_DOMAINS)) {
    expectLiteral(record[name], expected, `relationship vector.domains.${name}`);
  }
  return RELATIONSHIP_DOMAINS;
}

function parseTestCeilings(value: unknown): RelationshipVectorDocument["test_ceilings"] {
  const record = expectRecord(value, "relationship vector.test_ceilings");
  expectExactKeys(
    record,
    ["max_relationship_bytes", "max_relationships", "max_total_canonical_bytes"],
    "relationship vector.test_ceilings",
  );
  return {
    max_relationship_bytes: expectSafeInteger(
      record.max_relationship_bytes,
      "relationship vector.test_ceilings.max_relationship_bytes",
    ),
    max_relationships: expectSafeInteger(
      record.max_relationships,
      "relationship vector.test_ceilings.max_relationships",
    ),
    max_total_canonical_bytes: expectSafeInteger(
      record.max_total_canonical_bytes,
      "relationship vector.test_ceilings.max_total_canonical_bytes",
    ),
  };
}

function expectRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label}: expected an object`);
  }
  return value as Record<string, unknown>;
}

function expectArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value) || value.length > MAX_VECTOR_ITEMS) {
    throw new Error(`${label}: expected a bounded array`);
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

function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    throw new Error(`${label}: expected a Boolean`);
  }
  return value;
}

function expectSafeInteger(value: unknown, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label}: expected a nonnegative safe integer`);
  }
  return value;
}

function expectLiteral<T extends string | number | boolean>(
  value: unknown,
  expected: T,
  label: string,
): T {
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

function expectDigestHex(value: unknown, label: string): string {
  const parsed = expectString(value, label);
  expectLength(decodeHexStrict(parsed, label), 32, label);
  return parsed;
}

function expectLength(bytes: Uint8Array, length: number, label: string): Uint8Array {
  if (bytes.length !== length) {
    throw new Error(`${label}: expected ${length} bytes`);
  }
  return bytes;
}

function expectVersion(value: number, label: string): void {
  if (value !== RELATIONSHIP_TREE_VERSION) {
    throw new Error(`${label}: unsupported version ${value}`);
  }
}

function requireItem<T>(values: readonly T[], index: number, label: string): T {
  const value = values[index];
  if (value === undefined) {
    throw new Error(`${label}: missing item ${index}`);
  }
  return value;
}

function deepEqual(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) {
    return true;
  }
  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => deepEqual(value, right[index]))
    );
  }
  if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) {
    return false;
  }
  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const leftKeys = Object.keys(leftRecord).sort();
  const rightKeys = Object.keys(rightRecord).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every(
      (key, index) => key === rightKeys[index] && deepEqual(leftRecord[key], rightRecord[key]),
    )
  );
}

function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
}

export { applyByteMutation, decodeHexStrict, encodeHexLower };
