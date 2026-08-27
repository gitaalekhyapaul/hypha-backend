import {
  datasetErrorResponse,
  datasetResponse,
  loadDatasetGatewayConfig,
  SyntheticDatasetClient,
} from "../../../../../lib/dataset-client/index.ts";
import {
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

interface DatasetRouteContext {
  readonly params: Promise<{ readonly datasetId: string }>;
}

export async function POST(request: Request, context: DatasetRouteContext): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.authorizeMutation(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const { datasetId } = await context.params;
    const client = new SyntheticDatasetClient(loadDatasetGatewayConfig());
    return datasetResponse(await client.finalize(datasetId));
  } catch (error) {
    return datasetErrorResponse(error);
  }
}
