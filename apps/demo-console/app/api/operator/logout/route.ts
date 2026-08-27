import {
  jsonResponse,
  operatorAuthResponse,
  operatorAuthService,
} from "../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  try {
    const service = await operatorAuthService();
    await service.authorizeMutation(request);
    return jsonResponse({ status: "SIGNED_OUT" }, 200, { "Set-Cookie": service.logoutCookie() });
  } catch (error) {
    return operatorAuthResponse(error);
  }
}
