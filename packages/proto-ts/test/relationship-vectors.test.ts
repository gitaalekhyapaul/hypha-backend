import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  applyByteMutation,
  decodeHexStrict,
  parseCanonicalRelationship,
  parseRelationshipVectorDocument,
} from "./support/relationship-vector-parser.ts";

const REPOSITORY_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const VECTOR_PATH = new URL(
  "tests/vectors/relationship-tree-v1.json",
  `file://${REPOSITORY_ROOT}/`,
);
const vectorText = readFileSync(VECTOR_PATH, "utf8");
const vector = parseRelationshipVectorDocument(vectorText);

describe("blinded relationship tree vectors", () => {
  it("strictly parses every relationship, tree, node, root, and opening frame", () => {
    expect(vector.private_test_only).toBe(true);
    expect(vector.relationship_cases).toHaveLength(5);
    expect(vector.tree_cases.map((treeCase) => treeCase.leaf_count)).toEqual([0, 1, 2, 3, 5]);
    expect(
      vector.tree_cases.every((treeCase) => treeCase.openings.length === treeCase.leaf_count),
    ).toBe(true);
  });

  it("rejects every TypeScript-targeted canonical mutation", () => {
    const canonical = decodeHexStrict(vector.relationship_cases[0]?.canonical_hex);
    const rejectionCases = vector.negative_cases.filter(
      (negativeCase) => negativeCase.expected_typescript_parser_result === "reject",
    );

    expect(rejectionCases.length).toBeGreaterThan(0);
    for (const negativeCase of rejectionCases) {
      if (negativeCase.mutation === null) {
        throw new Error(`${negativeCase.id}: missing byte mutation`);
      }
      const mutated = applyByteMutation(canonical, negativeCase.mutation);
      expect(() => parseCanonicalRelationship(mutated), negativeCase.id).toThrow();
    }
  });

  it("rejects unknown vector fields and malformed digest widths", () => {
    const unknownField = JSON.parse(vectorText) as Record<string, unknown>;
    unknownField.unexpected = true;
    expect(() => parseRelationshipVectorDocument(JSON.stringify(unknownField))).toThrow();

    const malformedDigest = JSON.parse(vectorText) as {
      relationship_cases: Array<{ leaf_digest_hex: string }>;
    };
    const firstCase = malformedDigest.relationship_cases[0];
    if (firstCase === undefined) {
      throw new Error("relationship vector has no positive relationship case");
    }
    firstCase.leaf_digest_hex = firstCase.leaf_digest_hex.slice(2);
    expect(() => parseRelationshipVectorDocument(JSON.stringify(malformedDigest))).toThrow();
  });
});
