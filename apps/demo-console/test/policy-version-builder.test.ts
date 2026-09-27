import { describe, expect, it } from "vitest";
import { defaultPolicyBuilderState, POLICY_MODE_OPTIONS } from "../lib/policy-builder/model.ts";
import {
  buildPolicyVersionRequest,
  isUncertainPolicyVersionFailure,
  policyVersionFailureMessage,
} from "../lib/policy-builder/versioned.ts";
import { CAVEAT_CATALOG } from "../lib/policy-draft/catalog.ts";
import { validateVersionRequest } from "../lib/policy-version/validation.ts";
import { PortalRequestError } from "../lib/portal-client/browser.ts";

describe("versioned policy builder model", () => {
  it("uses valid compatibility defaults instead of user identity/version/epoch fields", () => {
    const request = buildPolicyVersionRequest(
      {
        ...defaultPolicyBuilderState(),
        policyName: "ignored",
        resourceType: "ignored",
        policyVersion: "999",
        expectedApprovalEpoch: "999",
      },
      "candidate-test",
    );
    expect(request.draft).toMatchObject({
      candidateId: "candidate-test",
      policyName: "reconciliation-query",
      resourceType: "dataset",
      policyVersion: 1,
    });
    expect(request.expectedApprovalEpoch).toBe(1);
    expect(request).not.toHaveProperty("familyId");
    expect(validateVersionRequest(request)).toEqual(request);
  });

  it("selects a family without choosing the next version", () => {
    const request = buildPolicyVersionRequest(
      defaultPolicyBuilderState(),
      "candidate-test",
      ` ${"a".repeat(32)} `,
    );
    expect(request.familyId).toBe("a".repeat(32));
    expect(request).not.toHaveProperty("versionId");
    expect(validateVersionRequest(request)).toEqual(request);
  });

  it.each(["", "old-policy", "A".repeat(32), "a".repeat(31)])(
    "rejects invalid family selection %j",
    (value) => {
      expect(() =>
        buildPolicyVersionRequest(defaultPolicyBuilderState(), "candidate-test", value),
      ).toThrow("policy family ID");
    },
  );

  it.each(POLICY_MODE_OPTIONS)("retains the $value rule family", ({ value }) => {
    const request = buildPolicyVersionRequest(
      { ...defaultPolicyBuilderState(), mode: value },
      "candidate-test",
    );
    expect(validateVersionRequest(request)).toEqual(request);
  });

  it.each(CAVEAT_CATALOG)("retains the $template caveat", ({ template }) => {
    const request = buildPolicyVersionRequest(
      { ...defaultPolicyBuilderState(), mode: "caveat", caveatTemplate: template },
      "candidate-test",
    );
    expect(request.draft.rule).toMatchObject({ kind: "direct", caveat: { template } });
    expect(validateVersionRequest(request)).toEqual(request);
  });

  it("does not let version defaults bypass rule or lifetime validation", () => {
    expect(() =>
      buildPolicyVersionRequest(
        {
          ...defaultPolicyBuilderState(),
          mode: "dual_control",
          dualLeft: "viewer",
          dualRight: "viewer",
        },
        "candidate-test",
      ),
    ).toThrow("duplicate rule leaf");
    expect(() =>
      buildPolicyVersionRequest(
        { ...defaultPolicyBuilderState(), validForSeconds: "2" },
        "candidate-test",
      ),
    ).toThrow("Candidate lifetime");
  });

  it("retains unknown outcomes and never suggests editing an approval epoch", () => {
    expect(isUncertainPolicyVersionFailure(new PortalRequestError("PORTAL_UNAVAILABLE", 503))).toBe(
      true,
    );
    expect(
      isUncertainPolicyVersionFailure(new PortalRequestError("CANDIDATE_NOT_READY", 409)),
    ).toBe(true);
    expect(isUncertainPolicyVersionFailure(new PortalRequestError("COMPILER_FAILED", 422))).toBe(
      false,
    );
    expect(
      policyVersionFailureMessage(new PortalRequestError("POLICY_ACTIVATION_CONFLICT", 409)),
    ).not.toContain("epoch");
  });
});
