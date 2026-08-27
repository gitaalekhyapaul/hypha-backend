import {
  jsonResponse,
  operatorAuthResponse,
  operatorAuthService,
  readLoginPassword,
} from "../../../../lib/operator-auth/index.ts";

export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<Response> {
  try {
    const service = await operatorAuthService();
    service.assertExactOrigin(request);
    const issued = await service.login(await readLoginPassword(request));
    return jsonResponse(issued.session, 200, { "Set-Cookie": issued.setCookie });
  } catch (error) {
    return operatorAuthResponse(error);
  }
}
