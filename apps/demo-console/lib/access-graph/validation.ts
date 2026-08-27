import type {
  AccessGraphPublicationProjection,
  AccessGraphRelationship,
  PublishAccessGraphRequest,
} from "./types.ts";
import { AccessGraphError } from "./types.ts";

export function validatePublicationRequest(input: unknown): PublishAccessGraphRequest {
  const object = exactObject(input, ["candidate_id", "valid_for_seconds", "relationships"]);
  const candidateId = identifierValue(object, "candidate_id");
  const validForSeconds = object.valid_for_seconds;
  if (!integerBetween(validForSeconds, 900, 86_400)) {
    invalid();
  }
  if (
    !Array.isArray(object.relationships) ||
    object.relationships.length < 1 ||
    object.relationships.length > 32
  ) {
    invalid();
  }
  return {
    candidate_id: candidateId,
    valid_for_seconds: validForSeconds,
    relationships: object.relationships.map(relationship),
  };
}

export function validatePublicationProjection(input: unknown): AccessGraphPublicationProjection {
  const object = exactObject(input, [
    "policy_record_id",
    "policy_hash",
    "snapshot_id",
    "relationship_root",
    "epoch",
    "issued_at",
    "expires_at",
    "accepted_relationship_count",
    "registry_generation",
    "status",
  ]);
  const policyRecordId = identifierValue(object, "policy_record_id", unavailable);
  const policyHash = object.policy_hash;
  const snapshotId = identifierValue(object, "snapshot_id", unavailable);
  const relationshipRoot = object.relationship_root;
  const epoch = object.epoch;
  const registryGeneration = object.registry_generation;
  const count = object.accepted_relationship_count;
  if (
    !hexDigest(policyHash) ||
    !hexDigest(relationshipRoot) ||
    !positiveDecimal(epoch) ||
    !positiveDecimal(registryGeneration) ||
    !integerBetween(count, 1, 32) ||
    object.status !== "ACCEPTED"
  ) {
    unavailable();
  }
  return {
    policy_record_id: policyRecordId,
    policy_hash: policyHash,
    snapshot_id: snapshotId,
    relationship_root: relationshipRoot,
    epoch,
    issued_at: timestamp(object.issued_at),
    expires_at: timestamp(object.expires_at),
    accepted_relationship_count: count,
    registry_generation: registryGeneration,
    status: "ACCEPTED",
  };
}

function relationship(input: unknown): AccessGraphRelationship {
  const object = exactObject(
    input,
    [
      "resource_type",
      "resource_id",
      "relation",
      "subject_type",
      "subject_id",
      "subject_relation",
      "caveat",
    ],
    true,
  );
  const resourceType = identifierValue(object, "resource_type");
  const resourceId = identifierValue(object, "resource_id");
  const relation = identifierValue(object, "relation");
  const subjectType = identifierValue(object, "subject_type");
  const subjectId = identifierValue(object, "subject_id");
  if (object.subject_relation !== undefined && !identifier(object.subject_relation)) {
    invalid();
  }
  let caveat: AccessGraphRelationship["caveat"];
  if (object.caveat !== undefined) {
    const caveatObject = exactObject(object.caveat, ["name", "context"]);
    const caveatName = identifierValue(caveatObject, "name");
    if (!Array.isArray(caveatObject.context) || caveatObject.context.length > 16) {
      invalid();
    }
    caveat = {
      name: caveatName,
      context: caveatObject.context.map((entry) => {
        const context = exactObject(entry, ["name", "value"]);
        const contextName = identifierValue(context, "name");
        const value = exactObject(context.value, ["type", "value"]);
        if (value.type === "bool" && typeof value.value === "boolean") {
          return { name: contextName, value: { type: "bool", value: value.value } };
        }
        if (value.type === "int" && Number.isSafeInteger(value.value)) {
          return { name: contextName, value: { type: "int", value: value.value as number } };
        }
        if (
          value.type === "string" &&
          typeof value.value === "string" &&
          value.value.length <= 256
        ) {
          return { name: contextName, value: { type: "string", value: value.value } };
        }
        return invalid();
      }),
    };
  }
  return {
    resource_type: resourceType,
    resource_id: resourceId,
    relation,
    subject_type: subjectType,
    subject_id: subjectId,
    ...(object.subject_relation === undefined
      ? {}
      : { subject_relation: object.subject_relation as string }),
    ...(caveat === undefined ? {} : { caveat }),
  };
}

function timestamp(input: unknown): { readonly seconds: string; readonly nanos: 0 } {
  const object = exactObject(input, ["seconds", "nanos"]);
  if (!positiveDecimal(object.seconds) || object.nanos !== 0) {
    unavailable();
  }
  return { seconds: object.seconds, nanos: 0 };
}

function exactObject(
  input: unknown,
  keys: readonly string[],
  optional = false,
): Record<string, unknown> {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    invalid();
  }
  const object = input as Record<string, unknown>;
  const allowed = new Set(keys);
  if (Object.keys(object).some((key) => !allowed.has(key))) {
    invalid();
  }
  if (!optional && keys.some((key) => !(key in object))) {
    invalid();
  }
  return object;
}

function identifierValue(
  object: Record<string, unknown>,
  field: string,
  fail: () => never = invalid,
): string {
  const value = object[field];
  if (!identifier(value)) {
    fail();
  }
  return value;
}

function identifier(input: unknown): input is string {
  return typeof input === "string" && /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/.test(input);
}

function hexDigest(input: unknown): input is string {
  return typeof input === "string" && /^[0-9a-f]{64}$/.test(input);
}

function positiveDecimal(input: unknown): input is string {
  return typeof input === "string" && /^[1-9][0-9]*$/.test(input);
}

function integerBetween(input: unknown, minimum: number, maximum: number): input is number {
  return (
    Number.isSafeInteger(input) && (input as number) >= minimum && (input as number) <= maximum
  );
}

function invalid(): never {
  throw new AccessGraphError("INVALID_ACCESS_GRAPH_REQUEST");
}

function unavailable(): never {
  throw new AccessGraphError("ACCESS_GRAPH_UNAVAILABLE");
}
