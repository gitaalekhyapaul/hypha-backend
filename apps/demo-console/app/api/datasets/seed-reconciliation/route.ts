import {
  datasetErrorResponse,
  datasetResponse,
  loadDatasetGatewayConfig,
  SyntheticDatasetClient,
} from "../../../../lib/dataset-client/index.ts";
import { seedReconciliationLedgers } from "../../../../lib/dataset-client/reconciliation-seed.ts";
import { operatorAuthResponse, operatorAuthService } from "../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.authorizeMutation(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const client = new SyntheticDatasetClient(loadDatasetGatewayConfig());
    return datasetResponse(await seedReconciliationLedgers(client));
  } catch (error) {
    return datasetErrorResponse(error);
  }
}
