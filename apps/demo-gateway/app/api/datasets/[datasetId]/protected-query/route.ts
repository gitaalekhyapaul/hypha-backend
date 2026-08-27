import { NextResponse } from "next/server";

import { AuthorizationRejectedError } from "../../../../../src/protected-request/errors";
import { getProtectedQueryService } from "../../../../../src/protected-request/runtime";
import type { ProtectedQueryService } from "../../../../../src/protected-request/service";
import { PROTECTED_QUERY_MAX_ENVELOPE_BYTES } from "../../../../../src/protected-request/types";

interface DatasetRouteContext {
  readonly params: Promise<{ readonly datasetId: string }>;
}

export async function POST(request: Request, context: DatasetRouteContext): Promise<NextResponse> {
  return handleProtectedQueryRequest(request, context);
}

export async function handleProtectedQueryRequest(
  request: Request,
  context: DatasetRouteContext,
  service?: Pick<ProtectedQueryService, "execute">,
): Promise<NextResponse> {
  try {
    const { datasetId } = await context.params;
    const input = await readEnvelope(request);
    const result = await (service ?? getProtectedQueryService()).execute(datasetId, input);
    return NextResponse.json(result);
  } catch (error) {
    if (error instanceof AuthorizationRejectedError) {
      return NextResponse.json({ error: "AUTHORIZATION_REJECTED" }, { status: 403 });
    }
    return NextResponse.json({ error: "PROTECTED_QUERY_UNAVAILABLE" }, { status: 503 });
  }
}

async function readEnvelope(request: Request): Promise<unknown> {
  const declaredLength = request.headers.get("content-length");
  if (declaredLength !== null) {
    const parsed = Number(declaredLength);
    if (
      !Number.isSafeInteger(parsed) ||
      parsed < 1 ||
      parsed > PROTECTED_QUERY_MAX_ENVELOPE_BYTES
    ) {
      throw new AuthorizationRejectedError();
    }
  }
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.length === 0 || bytes.length > PROTECTED_QUERY_MAX_ENVELOPE_BYTES) {
    throw new AuthorizationRejectedError();
  }
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return JSON.parse(text) as unknown;
  } catch {
    throw new AuthorizationRejectedError();
  }
}
