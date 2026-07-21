import { readFileSync } from "node:fs";
import { isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { equals, fromJsonString } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";

import { PolicyProgramSchema } from "../src/gen/raziel/v1/policy_ir_pb.ts";
import {
  applyByteMutation,
  decodeHexStrict,
  encodeHexLower,
  parseCanonicalPolicy,
  parseCanonicalPolicyHex,
  parseI64Hex,
  parseOptionalStringHex,
  parsePolicyHashPreimage,
  parsePolicyHashPreimageHex,
  parsePolicyVectorDocument,
  parseUtf8StringHex,
} from "./support/policy-vector-parser.ts";

const REPOSITORY_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const VECTOR_PATH = "tests/vectors/policy-ir-v1.json";
const vector = parsePolicyVectorDocument(readRepositoryFile(VECTOR_PATH));

describe("canonical PolicyIR vectors", () => {
  it("parses each policy and matches the strict human Protobuf JSON", () => {
    for (const policyCase of vector.policy_cases) {
      const humanPolicy = fromJsonString(
        PolicyProgramSchema,
        readRepositoryFile(policyCase.human_input_path),
        { ignoreUnknownFields: false },
      );
      const payloadPolicy = parseCanonicalPolicyHex(policyCase.canonical_payload_hex);
      const preimage = parsePolicyHashPreimageHex(policyCase.hash_preimage_hex);

      expect(equals(PolicyProgramSchema, payloadPolicy, humanPolicy), policyCase.id).toBe(true);
      expect(equals(PolicyProgramSchema, preimage.policy, humanPolicy), policyCase.id).toBe(true);
      expect(encodeHexLower(preimage.payload), policyCase.id).toBe(
        policyCase.canonical_payload_hex,
      );
      expect(preimage.payload).toHaveLength(policyCase.canonical_payload_byte_length);
      expect(decodeHexStrict(policyCase.hash_preimage_hex)).toHaveLength(
        policyCase.hash_preimage_byte_length,
      );
    }
  });

  it("rejects every TypeScript-targeted negative vector", () => {
    const policyCase = requirePolicyCase();
    const rejectionCases = vector.negative_cases.filter(
      (negativeCase) => negativeCase.expected_typescript_parser_result === "reject",
    );

    expect(rejectionCases.length).toBeGreaterThan(0);
    for (const negativeCase of rejectionCases) {
      const sourceHex =
        negativeCase.target === "canonical_payload"
          ? policyCase.canonical_payload_hex
          : negativeCase.target === "hash_preimage"
            ? policyCase.hash_preimage_hex
            : undefined;
      if (sourceHex === undefined) {
        throw new Error(`${negativeCase.id}: unsupported TypeScript mutation target`);
      }

      const mutated = applyByteMutation(decodeHexStrict(sourceHex), negativeCase.mutation);
      const parse =
        negativeCase.target === "canonical_payload"
          ? () => parseCanonicalPolicy(mutated)
          : () => parsePolicyHashPreimage(mutated);
      expect(parse, negativeCase.id).toThrow();
    }
  });

  it("rejects truncated, trailing, and mutated preimage framing", () => {
    const preimage = decodeHexStrict(requirePolicyCase().hash_preimage_hex);
    const trailing = new Uint8Array(preimage.length + 1);
    trailing.set(preimage);
    const mutatedDomainLength = preimage.slice();
    mutatedDomainLength[3] = 15;

    expect(() => parsePolicyHashPreimage(preimage.slice(0, -1))).toThrow();
    expect(() => parsePolicyHashPreimage(trailing)).toThrow();
    expect(() => parsePolicyHashPreimage(mutatedDomainLength)).toThrow();
  });
});

describe("canonical primitives", () => {
  it("parses every signed i64 boundary vector as bigint", () => {
    for (const primitiveCase of vector.primitive_cases.i64) {
      expect(parseI64Hex(primitiveCase.canonical_hex)).toBe(BigInt(primitiveCase.value));
    }
  });

  it("preserves exact UTF-8 spellings", () => {
    const parsed = vector.primitive_cases.utf8_string.map((primitiveCase) => ({
      bytes: primitiveCase.canonical_hex,
      value: parseUtf8StringHex(primitiveCase.canonical_hex),
    }));

    for (const [index, primitiveCase] of vector.primitive_cases.utf8_string.entries()) {
      expect(parsed[index]?.value).toBe(primitiveCase.value);
    }
    expect(parsed[0]?.bytes).not.toBe(parsed[1]?.bytes);
    expect(parsed[0]?.value).not.toBe(parsed[1]?.value);
  });

  it("distinguishes absent and present-empty optional strings", () => {
    for (const primitiveCase of vector.primitive_cases.optional_string) {
      expect(parseOptionalStringHex(primitiveCase.canonical_hex)).toBe(
        primitiveCase.value === null ? undefined : primitiveCase.value,
      );
    }
    expect(() => parseOptionalStringHex("02")).toThrow();
  });

  it("rejects malformed hex and invalid UTF-8", () => {
    expect(() => decodeHexStrict(12)).toThrow();
    expect(() => decodeHexStrict("0")).toThrow();
    expect(() => decodeHexStrict("AA")).toThrow();
    expect(() => decodeHexStrict("gg")).toThrow();
    expect(() => parseUtf8StringHex("00000001ff")).toThrow();
  });
});

function requirePolicyCase() {
  const policyCase = vector.policy_cases[0];
  if (policyCase === undefined) {
    throw new Error("policy vector has no positive policy case");
  }
  return policyCase;
}

function readRepositoryFile(relativePath: string): string {
  if (isAbsolute(relativePath)) {
    throw new Error(`vector path must be repository-relative: ${relativePath}`);
  }
  const absolutePath = resolve(REPOSITORY_ROOT, relativePath);
  const repositoryRelativePath = relative(REPOSITORY_ROOT, absolutePath);
  if (repositoryRelativePath.startsWith("..") || isAbsolute(repositoryRelativePath)) {
    throw new Error(`vector path leaves the repository: ${relativePath}`);
  }
  return readFileSync(absolutePath, "utf8");
}
