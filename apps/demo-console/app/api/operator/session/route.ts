import {
  jsonResponse,
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function GET(request: Request): Promise<Response> {
  try {
    const service = await operatorAuthService();
    return jsonResponse(await service.verify(request));
  } catch (error) {
    return operatorAuthResponse(error);
  }
}
