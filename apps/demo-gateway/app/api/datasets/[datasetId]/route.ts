import { NextResponse } from "next/server";

import { reconciliationErrorResponse } from "../../../../src/reconciliation/http";
import { getSyntheticDatasetStore } from "../../../../src/reconciliation/runtime";

interface DatasetRouteContext {
  readonly params: Promise<{ readonly datasetId: string }>;
}

export async function GET(_request: Request, context: DatasetRouteContext): Promise<NextResponse> {
  try {
    const { datasetId } = await context.params;
    return NextResponse.json(await getSyntheticDatasetStore().get(datasetId));
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}
