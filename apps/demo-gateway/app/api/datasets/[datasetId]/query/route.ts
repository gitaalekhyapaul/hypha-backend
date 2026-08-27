import { NextResponse } from "next/server";

import {
  parseReconciliationQuery,
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
    const query = parseReconciliationQuery(await readOperatorJson(request));
    const rows = await getSyntheticDatasetStore().query(datasetId, query);
    return NextResponse.json({ classification: "SYNTHETIC", rowCount: rows.length, rows });
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}
