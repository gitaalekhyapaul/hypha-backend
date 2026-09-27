export const POLICY_DRAFT_FORMAT_VERSION = 1 as const;

export type DirectRelation = "approver" | "auditor" | "owner" | "viewer";

export type CaveatSelection =
  | {
      readonly template: "amount_limit";
    }
  | {
      readonly template: "amount_or_override";
    }
  | {
      readonly template: "approval_gate";
    }
  | {
      readonly template: "task_purpose_scope";
      readonly taskId: string;
      readonly purpose: string;
      readonly resourceScope: string;
    }
  | {
      readonly template: "reconciliation_scope";
      readonly taskId: string;
      readonly purpose: string;
      readonly resourceScope: string;
    };

export interface DirectRule {
  readonly kind: "direct";
  readonly relation: DirectRelation;
  readonly caveat?: CaveatSelection;
}

export interface SubjectSetRule {
  readonly kind: "subject_set";
  readonly relation: "team_viewer";
}

export interface ArrowRule {
  readonly kind: "arrow";
  readonly relation: "organization";
  readonly targetPermission: "audit";
}

export interface AnyRule {
  readonly kind: "any";
  readonly children: readonly PolicyRule[];
}

export interface AllRule {
  readonly kind: "all";
  readonly children: readonly PolicyRule[];
}

export type PolicyRule = DirectRule | SubjectSetRule | ArrowRule | AnyRule | AllRule;

export interface PolicyDraftV1 {
  readonly formatVersion: typeof POLICY_DRAFT_FORMAT_VERSION;
  readonly candidateId: string;
  readonly policyName: string;
  readonly policyVersion: number;
  readonly resourceType: string;
  readonly permission: "query";
  readonly rule: PolicyRule;
}

export type CaveatParameterSource = "CAPABILITY_CLAIM" | "RELATIONSHIP_CONTEXT" | "REQUEST_CONTEXT";

export type CaveatValueType = "bool" | "int" | "string";

export interface CaveatParameterDefinition {
  readonly name: string;
  readonly valueType: CaveatValueType;
  readonly source: CaveatParameterSource;
}

export interface CaveatCatalogEntry {
  readonly template: CaveatSelection["template"];
  readonly schemaName: string;
  readonly label: string;
  readonly description: string;
  readonly parameters: readonly CaveatParameterDefinition[];
}

export interface RenderedPolicySource {
  readonly draft: PolicyDraftV1;
  readonly compilerVersion: string;
  readonly schema: string;
  readonly compilerConfig: string;
  readonly ruleSummary: string;
}

export interface PolicyVersionIdentity {
  readonly familyId: string;
  readonly versionId: string;
  readonly version: number;
}

export interface PolicySourceInventory {
  // Includes the shared empty agent definition. Names are sorted lexically.
  readonly definitions: readonly string[];
  readonly caveats: readonly string[];
}

export interface RenderedPolicyVersionSource extends RenderedPolicySource {
  readonly identity: PolicyVersionIdentity;
  readonly signedPolicyName: string;
  readonly inventory: PolicySourceInventory;
}

export class PolicyDraftValidationError extends Error {
  public readonly path: string;

  public constructor(path: string, message: string) {
    super(`${path}: ${message}`);
    this.name = "PolicyDraftValidationError";
    this.path = path;
  }
}
