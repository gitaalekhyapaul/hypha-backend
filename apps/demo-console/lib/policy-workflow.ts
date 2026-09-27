export type PolicyWorkflow = "legacy" | "versioned";

export function policyWorkflow(environment: NodeJS.ProcessEnv = process.env): PolicyWorkflow {
  const mode = environment.RAZIEL_POLICY_WORKFLOW ?? "legacy";
  if (mode !== "legacy" && mode !== "versioned") {
    throw new Error("Invalid policy workflow configuration");
  }
  return mode;
}

// Call after operator authentication and before parsing input or contacting an owner.
export function policyWorkflowResponse(
  required: PolicyWorkflow,
  environment: NodeJS.ProcessEnv = process.env,
): Response | undefined {
  try {
    if (policyWorkflow(environment) === required) return undefined;
    return Response.json(
      { error: "POLICY_WORKFLOW_UNAVAILABLE" },
      { status: 409, headers: { "Cache-Control": "no-store" } },
    );
  } catch {
    return Response.json(
      { error: "POLICY_WORKFLOW_UNAVAILABLE" },
      { status: 503, headers: { "Cache-Control": "no-store" } },
    );
  }
}
