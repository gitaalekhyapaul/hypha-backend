import { NextResponse } from "next/server";

import {
  parseDatasetDraft,
  readOperatorJson,
  reconciliationErrorResponse,
} from "../../../src/reconciliation/http";
import { getSyntheticDatasetStore } from "../../../src/reconciliation/runtime";

export async function GET(): Promise<NextResponse> {
  try {
    const datasets = await getSyntheticDatasetStore().list();
    return NextResponse.json({ classification: "SYNTHETIC", datasets });
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}

export async function POST(request: Request): Promise<NextResponse> {
  try {
    const draft = parseDatasetDraft(await readOperatorJson(request));
    const dataset = await getSyntheticDatasetStore().createDraft(draft);
    return NextResponse.json(dataset, { status: 201 });
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}

export async function PUT(request: Request): Promise<NextResponse> {
  try {
    const draft = parseDatasetDraft(await readOperatorJson(request));
    const dataset = await getSyntheticDatasetStore().putDraft(draft);
    return NextResponse.json(dataset, { status: 200 });
  } catch (error) {
    return reconciliationErrorResponse(error);
  }
}
