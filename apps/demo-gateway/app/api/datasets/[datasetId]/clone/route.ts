import { NextResponse } from "next/server";

import {
  parseCloneRequest,
  readOperatorJson,
  reconciliationErrorResponse,
} from "../../../../../src/reconciliation/http";
import { getSyntheticDatasetStore } from "../../../../../src/reconciliation/runtime";

interface DatasetRouteContext {
  readonly params: Promise<{ readonly datasetId: string }>;
}

export async function POST(request: Request, context: DatasetRouteContext): Promise<NextResponse> {
  try {
    const { datasetId } = await context.params;
    const { targetDatasetId } = parseCloneRequest(await readOperatorJson(request));
    return NextResponse.json(await getSyntheticDatasetStore().clone(datasetId, targetDatasetId), {
      status: 201,
    });
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}
