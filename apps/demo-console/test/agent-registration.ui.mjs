// Run against an existing dev server with an already installed Playwright module:
// PLAYWRIGHT_MODULE=<module URL> RAZIEL_UI_URL=http://127.0.0.1:3214 node test/agent-registration.ui.mjs
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

assert.ok(
  process.env.PLAYWRIGHT_MODULE,
  "Set PLAYWRIGHT_MODULE to an installed Playwright module.",
);
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
const page = await context.newPage();
const publicKeyHex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const publicKey = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
const now = Math.floor(Date.now() / 1_000);
const registration = (agentId, displayName) => ({
  formatVersion: 1,
  agentId,
  displayName,
  publicKey,
  keyThumbprint: publicKey,
  createdAtUnixSeconds: String(now - 3600),
});
const agents = [
  registration("agent-one", "Processor agent"),
  registration("agent-two", "Bank agent"),
];
const tokens = new Map(agents.map((agent) => [agent.agentId, []]));
const writes = [];
const pageErrors = [];
const syntheticSecrets = [];
let tokenSequence = 0;
let expired = false;
let unavailable = false;
let tokenUnavailable = false;
let loseCreateResponse = false;
let duplicateRegistration = true;
const output = join(tmpdir(), "raziel-b0064-ui");

page.on("pageerror", (error) => pageErrors.push(error.message));
await context.addInitScript(() => {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: {
      writeText: async () => {
        throw new Error("Clipboard denied");
      },
    },
  });
});
await page.route("**/api/**", async (route) => {
  const request = route.request();
  const path = new URL(request.url()).pathname;
  const respond = (json, status = 200) => route.fulfill({ status, json });
  if (path === "/api/operator/session")
    return respond({
      operatorId: "ui-test-operator",
      csrfToken: "ui-test-csrf",
      expiresAtUnixSeconds: now + 3600,
    });
  if (path === "/api/datasets") return respond({ datasets: [] });
  if (!path.startsWith("/api/operator/agents"))
    return respond({ error: "PORTAL_UNAVAILABLE" }, 503);
  if (expired) return route.fulfill({ status: 401, body: "Session expired" });
  if (unavailable) return respond({ error: { code: "SERVICE_UNAVAILABLE", retryable: true } }, 503);
  if (request.method() === "POST") {
    assert.equal(request.headers()["x-raziel-csrf-token"], "ui-test-csrf");
    writes.push({ path, body: request.postDataJSON() });
  }
  if (path === "/api/operator/agents") {
    if (request.method() === "GET") return respond({ agents });
    if (duplicateRegistration) {
      duplicateRegistration = false;
      return respond({ error: "AGENT_ALREADY_REGISTERED" }, 409);
    }
    const body = request.postDataJSON();
    assert.deepEqual(body, { formatVersion: 1, displayName: "Ledger agent", publicKey });
    const agent = registration("agent-new", body.displayName);
    agents.push(agent);
    tokens.set(agent.agentId, []);
    return respond(agent, 201);
  }
  const match = /^\/api\/operator\/agents\/([^/]+)\/tokens(?:\/([^/]+)\/revoke)?$/.exec(path);
  assert.ok(match, `Unexpected route: ${path}`);
  const [, agentId, revokeId] = match;
  const metadata = tokens.get(agentId);
  assert.ok(metadata);
  if (request.method() === "GET") {
    if (tokenUnavailable) return respond({ error: "PORTAL_UNAVAILABLE" }, 503);
    return respond({ tokens: metadata });
  }
  if (revokeId) {
    assert.deepEqual(request.postDataJSON(), { formatVersion: 1 });
    const token = metadata.find((item) => item.tokenId === revokeId);
    assert.ok(token);
    token.revokedAtUnixSeconds = String(now);
    return respond(token);
  }
  const body = request.postDataJSON();
  assert.equal(body.formatVersion, 1);
  assert.equal(typeof body.expiresAtUnixSeconds, "string");
  const seconds = Number(body.expiresAtUnixSeconds) - Math.floor(Date.now() / 1000);
  assert.ok(seconds >= 30 * 86_400 - 3 && seconds <= 30 * 86_400);
  const tokenId = (++tokenSequence).toString(16).padStart(32, "0");
  const item = {
    formatVersion: 1,
    tokenId,
    agentId,
    createdAtUnixSeconds: String(now),
    expiresAtUnixSeconds: body.expiresAtUnixSeconds,
  };
  metadata.push(item);
  // These are synthetic transport fixtures, never credentials from a live service.
  const token = `rz1.${tokenId}.${"a".repeat(43)}`;
  syntheticSecrets.push(token);
  if (loseCreateResponse) return route.fulfill({ status: 502, body: "Response lost" });
  return respond({ token, metadata: item }, 201);
});

async function assertNoSecret() {
  const text = await page.locator("body").innerText();
  const storage = await page.evaluate(() =>
    JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }),
  );
  for (const token of syntheticSecrets) {
    assert.equal(text.includes(token), false, "Dismissed secret remained visible");
    assert.equal(storage.includes(token), false, "Secret entered browser storage");
  }
}

try {
  await page.goto(process.env.RAZIEL_UI_URL ?? "http://127.0.0.1:3214");
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  await page.getByRole("heading", { name: "Processor agent", exact: true }).waitFor();
  await page.getByText("No tokens issued.", { exact: true }).waitFor();
  await page.getByRole("button", { name: "Register agent", exact: true }).click();
  await page.getByLabel("Display name", { exact: true }).fill("Ledger agent");
  await page.getByLabel("Ed25519 public key (64 hexadecimal characters)").fill("bad-key");
  await page.getByRole("button", { name: "Register public key", exact: true }).click();
  await page
    .getByText("Enter a raw 32-byte Ed25519 public key as exactly 64 hexadecimal characters.")
    .waitFor();
  assert.equal(writes.length, 0);
  await page
    .getByLabel("Ed25519 public key (64 hexadecimal characters)")
    .fill(publicKeyHex.toUpperCase());
  await page.getByRole("button", { name: "Register public key", exact: true }).click();
  await page.getByText("This public key is already registered. Refresh the agent list.").waitFor();
  await page.getByRole("button", { name: "Register public key", exact: true }).click();
  await page.getByRole("heading", { name: "Ledger agent", exact: true }).waitFor();
  await page.getByText("No tokens issued.", { exact: true }).waitFor();
  const lifetime = page.getByLabel("Lifetime (days, 1-90)");
  const create = page.getByRole("button", { name: "Create token", exact: true });
  assert.equal(await lifetime.inputValue(), "30");
  for (const invalid of ["0", "91", "1.5"]) {
    await lifetime.fill(invalid);
    assert.equal(await create.isDisabled(), true);
  }
  await lifetime.fill("90");
  assert.equal(await create.isEnabled(), true);
  await lifetime.fill("30");
  await create.click();
  await page.getByRole("heading", { name: "New token", exact: true }).waitFor();
  assert.equal(await create.isDisabled(), true);
  assert.equal(await page.getByText(syntheticSecrets.at(-1), { exact: true }).count(), 1);
  await page.getByRole("button", { name: "Copy token", exact: true }).click();
  await page.getByText("Clipboard unavailable. Select the token to copy it manually.").waitFor();
  await page.evaluate(() =>
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async () => {} },
    }),
  );
  await page.getByRole("button", { name: "Copy token", exact: true }).click();
  await page.getByText("Token copied.", { exact: true }).waitFor();
  await page.getByRole("button", { name: "Dismiss token", exact: true }).click();
  await assertNoSecret();
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).click();
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).waitFor();
  await assertNoSecret();
  const firstToken = tokens.get("agent-new")[0].tokenId;
  await page.getByRole("button", { name: `Revoke token ${firstToken}`, exact: true }).click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  assert.equal(writes.filter((item) => item.path.endsWith("/revoke")).length, 0);
  await page.getByRole("button", { name: `Revoke token ${firstToken}`, exact: true }).click();
  await page.getByRole("button", { name: "Confirm revoke", exact: true }).click();
  await page.getByText("Token revoked.", { exact: true }).waitFor();
  assert.equal(
    await page.getByRole("button", { name: `Revoke token ${firstToken}`, exact: true }).count(),
    0,
  );

  await create.click();
  await page.getByRole("heading", { name: "New token", exact: true }).waitFor();
  await page.getByRole("button", { name: "Bank agent agent-two", exact: true }).click();
  await page.getByRole("heading", { name: "Bank agent", exact: true }).waitFor();
  await assertNoSecret();
  await page.getByRole("button", { name: "Ledger agent agent-new", exact: true }).click();
  await page.getByRole("heading", { name: "Ledger agent", exact: true }).waitFor();
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).waitFor();
  await assertNoSecret();

  tokenUnavailable = true;
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).click();
  await page.getByText("Token metadata unavailable.", { exact: true }).waitFor();
  assert.equal(await create.isDisabled(), true);
  tokenUnavailable = false;
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).click();
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).waitFor();
  loseCreateResponse = true;
  await create.click();
  await page
    .getByText(
      "Token creation not confirmed. Refresh metadata and revoke any token you cannot recover before creating another.",
    )
    .waitFor();
  assert.equal(await create.isDisabled(), true);
  await assertNoSecret();
  loseCreateResponse = false;
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).click();
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).waitFor();
  const lostToken = tokens.get("agent-new").at(-1).tokenId;
  await page.getByRole("button", { name: `Revoke token ${lostToken}`, exact: true }).click();
  await page.getByRole("button", { name: "Confirm revoke", exact: true }).click();
  await page.getByText("Token revoked.", { exact: true }).waitFor();
  await mkdir(output, { recursive: true });
  await page.screenshot({ path: join(output, "agents-desktop.png"), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(
    await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    true,
  );
  await page.screenshot({ path: join(output, "agents-mobile.png"), fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });

  await create.click();
  await page.getByRole("heading", { name: "New token", exact: true }).waitFor();
  await page.getByRole("button", { name: "Dataset", exact: true }).click();
  await assertNoSecret();
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  await page.getByRole("heading", { name: "Processor agent", exact: true }).waitFor();
  await assertNoSecret();
  unavailable = true;
  await page.getByRole("button", { name: "Refresh agents", exact: true }).click();
  await page.getByText("Agent registrations unavailable.", { exact: true }).waitFor();
  unavailable = false;
  expired = true;
  await page.getByRole("button", { name: "Refresh tokens", exact: true }).click();
  await page.getByLabel("Operator password", { exact: true }).waitFor();
  await assertNoSecret();
  assert.deepEqual(pageErrors, []);
  process.stdout.write(
    `PASS: registration, validation, token creation, copy failure/success, dismissal, metadata refresh, revocation, unknown outcome, selection/navigation cleanup, unavailable/session expiry, and responsive layout. Screenshots: ${output}\n`,
  );
} finally {
  await browser.close();
}
