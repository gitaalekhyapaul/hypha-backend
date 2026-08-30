import {
  decodeHex,
  verifyProtectedRequestSignature,
} from "../../../../packages/sdk-ts/dist/index.js";

import type { ProtectedRequestEnvelope, ProtectedRequestSignatureVerifier } from "./types.ts";

export class SdkProtectedRequestSignatureVerifier implements ProtectedRequestSignatureVerifier {
  public async verify(capabilityId: string, envelope: ProtectedRequestEnvelope): Promise<boolean> {
    try {
      return await verifyProtectedRequestSignature({
        formatVersion: envelope.formatVersion,
        capabilityId,
        agentPublicKey: decodeHex(envelope.agentPublicKeyHex),
        requestMethod: envelope.requestMethod,
        canonicalRequestTarget: envelope.canonicalRequestTarget,
        requestBodyHash: decodeHex(envelope.requestBodyHashHex),
        requestNonce: decodeHex(envelope.requestNonceHex),
        requestTimestampUnixSeconds: envelope.requestTimestampUnixSeconds,
        agentRequestSignature: decodeHex(envelope.agentRequestSignatureHex),
      });
    } catch {
      return false;
    }
  }
}
