import { constants } from "node:fs";
import { chmod, mkdir, open } from "node:fs/promises";
import path from "node:path";

import { ProtectedQueryInfrastructureError } from "./errors";
import type { PublicReceipt, ReceiptStore } from "./types";

const RECEIPT_DIRECTORY_MODE = 0o700;
const RECEIPT_FILE_MODE = 0o600;
const MAX_RECEIPT_BYTES = 4_096;

export class JsonlReceiptStore implements ReceiptStore {
  readonly #filePath: string;
  #tail: Promise<void> = Promise.resolve();

  public constructor(filePath: string) {
    if (filePath.length === 0 || filePath.length > 4_096) {
      throw new ProtectedQueryInfrastructureError();
    }
    this.#filePath = path.resolve(filePath);
  }

  public append(receipt: PublicReceipt): Promise<void> {
    const result = this.#tail.then(
      () => this.#append(receipt),
      () => this.#append(receipt),
    );
    this.#tail = result.catch(() => undefined);
    return result;
  }

  async #append(receipt: PublicReceipt): Promise<void> {
    const line = Buffer.from(`${JSON.stringify(receipt)}\n`, "utf8");
    if (line.length > MAX_RECEIPT_BYTES) {
      throw new ProtectedQueryInfrastructureError();
    }
    const directory = path.dirname(this.#filePath);
    try {
      await mkdir(directory, { recursive: true, mode: RECEIPT_DIRECTORY_MODE });
      await chmod(directory, RECEIPT_DIRECTORY_MODE);
      const handle = await open(
        this.#filePath,
        constants.O_APPEND | constants.O_CREAT | constants.O_WRONLY,
        RECEIPT_FILE_MODE,
      );
      try {
        await handle.writeFile(line);
        await handle.sync();
      } finally {
        await handle.close();
      }
      await chmod(this.#filePath, RECEIPT_FILE_MODE);
    } catch {
      throw new ProtectedQueryInfrastructureError();
    }
  }
}
