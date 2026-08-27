import {
  datasetErrorResponse,
  datasetResponse,
  loadDatasetGatewayConfig,
  parseDatasetDraft,
  readDatasetJson,
  SyntheticDatasetClient,
} from "../../../lib/dataset-client/index.ts";
import { operatorAuthResponse, operatorAuthService } from "../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function GET(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.verify(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const client = new SyntheticDatasetClient(loadDatasetGatewayConfig());
    return datasetResponse(await client.list());
  } catch (error) {
    return datasetErrorResponse(error);
  }
}

export async function PUT(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.authorizeMutation(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }
  try {
    const draft = parseDatasetDraft(await readDatasetJson(request));
    const client = new SyntheticDatasetClient(loadDatasetGatewayConfig());
    return datasetResponse(await client.putDraft(draft));
  } catch (error) {
    return datasetErrorResponse(error);
  }
}
