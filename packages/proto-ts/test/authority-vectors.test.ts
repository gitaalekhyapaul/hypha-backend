import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { parseAuthorityVectorDocument } from "./support/authority-vector-parser.ts";

const REPOSITORY_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const VECTOR_PATH = new URL(
  "tests/vectors/authority-signatures-v1.json",
  `file://${REPOSITORY_ROOT}/`,
);
const vectorText = readFileSync(VECTOR_PATH, "utf8");
const vector = parseAuthorityVectorDocument(vectorText);

describe("authority signature vectors", () => {
  it("strictly parses exact policy and snapshot claims and frames", () => {
    expect(vector.private_test_only).toBe(true);
    expect(vector.policy_approval.domain).toBe("raziel/policy-approval-signature/v1");
    expect(vector.snapshot_descriptor.domain).toBe("raziel/snapshot-descriptor-signature/v1");
    expect(vector.negative_cases).toHaveLength(23);
  });

  it("matches each key identifier to its public-key digest", () => {
    for (const authorityCase of [vector.policy_approval, vector.snapshot_descriptor]) {
      const publicKey = Buffer.from(authorityCase.public_key_hex, "hex");
      const digest = createHash("sha256").update(publicKey).digest("hex");
      expect(authorityCase.key_id).toBe(`ed25519:${digest}`);
    }
  });

  it("rejects unknown fields, malformed signatures, and broken frames", () => {
    const unknown = JSON.parse(vectorText) as Record<string, unknown>;
    unknown.unexpected = true;
    expect(() => parseAuthorityVectorDocument(JSON.stringify(unknown))).toThrow();

    const shortSignature = JSON.parse(vectorText) as {
      policy_approval: { signature_hex: string };
    };
    shortSignature.policy_approval.signature_hex =
      shortSignature.policy_approval.signature_hex.slice(2);
    expect(() => parseAuthorityVectorDocument(JSON.stringify(shortSignature))).toThrow();

    const brokenFrame = JSON.parse(vectorText) as {
      snapshot_descriptor: { signature_message_hex: string };
    };
    brokenFrame.snapshot_descriptor.signature_message_hex =
      brokenFrame.snapshot_descriptor.signature_message_hex.slice(0, -2);
    expect(() => parseAuthorityVectorDocument(JSON.stringify(brokenFrame))).toThrow();

    const noncanonicalU64 = JSON.parse(vectorText) as {
      u64_cases: { value: string }[];
    };
    expect(noncanonicalU64.u64_cases).not.toHaveLength(0);
    noncanonicalU64.u64_cases = [{ value: "00" }, ...noncanonicalU64.u64_cases.slice(1)];
    expect(() => parseAuthorityVectorDocument(JSON.stringify(noncanonicalU64))).toThrow();
  });
});
