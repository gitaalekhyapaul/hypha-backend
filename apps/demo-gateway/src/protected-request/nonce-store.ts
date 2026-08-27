import { createHash } from "node:crypto";

import { createClient } from "redis";

import { ProtectedQueryInfrastructureError } from "./errors";
import type { NonceStore } from "./types";

type RedisClient = ReturnType<typeof configuredRedisClient>;

export class RedisNonceStore implements NonceStore {
  readonly #url: string;
  #connection: Promise<RedisClient> | undefined;

  public constructor(url: string) {
    if (url.length === 0 || url.length > 4_096) {
      throw new ProtectedQueryInfrastructureError();
    }
    this.#url = url;
  }

  public async claim(
    capabilityId: string,
    requestNonceHex: string,
    ttlSeconds: number,
  ): Promise<boolean> {
    if (!Number.isSafeInteger(ttlSeconds) || ttlSeconds < 1) {
      throw new ProtectedQueryInfrastructureError();
    }
    const keyDigest = createHash("sha256")
      .update("raziel/protected-request-nonce/v1\0", "utf8")
      .update(capabilityId, "utf8")
      .update("\0", "utf8")
      .update(requestNonceHex, "ascii")
      .digest("hex");
    try {
      const client = await this.#client();
      const result = await client.set(`raziel:m5:nonce:${keyDigest}`, "1", {
        expiration: { type: "EX", value: ttlSeconds },
        condition: "NX",
      });
      return result === "OK";
    } catch {
      this.#connection = undefined;
      throw new ProtectedQueryInfrastructureError();
    }
  }

  async #client(): Promise<RedisClient> {
    this.#connection ??= this.#connect();
    return this.#connection;
  }

  async #connect(): Promise<RedisClient> {
    const client = configuredRedisClient(this.#url);
    client.on("error", () => undefined);
    await client.connect();
    return client;
  }
}

function configuredRedisClient(url: string) {
  return createClient({ url });
}
