import { createServer } from "node:http";
import { parseArgs } from "node:util";
import next from "next";
import { prepareAgentTransport } from "./server/agent-transport.ts";

const { values } = parseArgs({
  options: {
    dev: { type: "boolean", default: false },
    hostname: { type: "string", default: "0.0.0.0" },
    port: { type: "string", default: process.env.PORT ?? "3000" },
  },
});
const port = Number(values.port);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("invalid port");
process.env.NODE_ENV = values.dev ? "development" : "production";
const app = next({ dev: values.dev, hostname: values.hostname, port });
await app.prepare();
const handle = app.getRequestHandler();
const server = createServer(async (request, response) => {
  if (!prepareAgentTransport(request)) {
    response.writeHead(401, { "content-type": "application/json", "cache-control": "no-store" });
    response.end(JSON.stringify({ error: { code: "NOT_AUTHORIZED", retryable: false } }));
    return;
  }
  try {
    await handle(request, response);
  } catch {
    if (!response.headersSent) response.writeHead(503, { "cache-control": "no-store" });
    response.end();
  }
});
server.on("upgrade", app.getUpgradeHandler());
server.listen(port, values.hostname, () => {
  console.log(`Raziel console listening on port ${port}`);
});
let closing = false;
async function close() {
  if (closing) return;
  closing = true;
  const deadline = setTimeout(() => process.exit(1), 10000).unref();
  server.close();
  await app.close();
  server.closeAllConnections();
  clearTimeout(deadline);
  process.exit(0);
}
process.on("SIGINT", close);
process.on("SIGTERM", close);
