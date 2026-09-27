import {
  jsonResponse,
  operatorAuthResponse,
  operatorAuthService,
} from "../../../lib/operator-auth/index.ts";
import {
  ReceiptLedgerError,
  readPublicReceiptLedger,
  readReceiptLedgerQuery,
  receiptStorePath,
  serializePublicReceiptJsonl,
} from "../../../lib/receipt-ledger/index.ts";

export const dynamic = "force-dynamic";

export async function GET(request: Request): Promise<Response> {
  try {
    const auth = await operatorAuthService();
    await auth.verify(request);
  } catch (error) {
    return operatorAuthResponse(error);
  }

  try {
    const query = readReceiptLedgerQuery(request.url);
    const ledger = await readPublicReceiptLedger(receiptStorePath(), query);
    if (query.format === "jsonl") {
      return new Response(serializePublicReceiptJsonl(ledger.receipts), {
        headers: {
          "Cache-Control": "no-store",
          "Content-Disposition": 'attachment; filename="raziel-public-receipts.jsonl"',
          "Content-Type": "application/x-ndjson; charset=utf-8",
          "X-Content-Type-Options": "nosniff",
        },
      });
    }
    return jsonResponse(ledger);
  } catch (error) {
    const ledgerError =
      error instanceof ReceiptLedgerError
        ? error
        : new ReceiptLedgerError("RECEIPT_LEDGER_UNAVAILABLE", 503);
    return jsonResponse({ error: ledgerError.code }, ledgerError.status);
  }
}
