import type {
  PublicAuditReceipt,
  PublicReceiptLedger,
  ReceiptFilterField,
} from "../receipt-ledger/types.ts";

export type { PublicAuditReceipt, PublicReceiptLedger, ReceiptFilterField };

export interface ReceiptFilterSelection {
  readonly field: ReceiptFilterField;
  readonly value: string;
}

export function receiptQueryPath(filter?: ReceiptFilterSelection, format = "json"): string {
  const parameters = new URLSearchParams();
  parameters.set("format", format);
  if (filter) {
    parameters.set("field", filter.field);
    parameters.set("value", filter.value);
  }
  return `/api/receipts?${parameters.toString()}`;
}
