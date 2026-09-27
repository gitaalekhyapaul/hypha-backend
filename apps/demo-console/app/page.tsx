import { OperatorPortal } from "../components/operator-portal";
import { policyWorkflow } from "../lib/policy-workflow.ts";

export const dynamic = "force-dynamic";

export default function ConsolePage() {
  return <OperatorPortal policyWorkflow={policyWorkflow()} />;
}
