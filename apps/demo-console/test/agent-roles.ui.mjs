// Uses the existing browser test pattern and an already installed Playwright module.
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

assert.ok(process.env.PLAYWRIGHT_MODULE, "Set PLAYWRIGHT_MODULE to an installed module.");
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const browser = await chromium.launch({ channel: "chrome", headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
const now = Math.floor(Date.now() / 1000);
const publicKey = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
const thumbprint = Buffer.from(publicKey, "base64").toString("hex");
const policies = ["a", "b", "c"].map((digit, index) => {
  const familyId = "f".repeat(32);
  const versionId = digit.repeat(32);
  return {
    identity: { familyId, versionId, version: index + 1 },
    recordId: `record-${digit}`,
    policyName: `family:${familyId}:v${index + 1}`,
    policyVersion: String(index + 1),
    policyHash: digit.repeat(64),
    resourceType: `dataset_${versionId}`,
    permission: "query",
    approvalEpoch: String(index + 1),
    validFromUnixSeconds: String(now - 3600),
    validUntilUnixSeconds: String(now + 3600),
    requiredGrantFields: digit === "a" ? ["teamId"] : [],
  };
});
const agents = ["one", "two"].map((id) => ({
  formatVersion: 1,
  agentId: `agent-${id}`,
  displayName: id === "one" ? "Processor agent" : "Bank agent",
  publicKey,
  keyThumbprint: publicKey,
  createdAtUnixSeconds: String(now - 3600),
}));
const managed = new Set();
const assignments = new Map();
const events = [];
const pageErrors = [];
let expired = false;
let failNextMutation = false;
let loseNextMutation = false;
let failRead = false;
let delayMutation = false;
let releaseMutation;
const output = join(tmpdir(), "raziel-b0068-ui");
page.on("pageerror", (error) => pageErrors.push(error.message));

function state(agentId, scope) {
  const { policyVersionId, approvalEpoch, datasetId } = scope;
  const unsupported = policyVersionId === policies[2].identity.versionId;
  const canEnable =
    policyVersionId === policies[0].identity.versionId && !managed.has(policyVersionId);
  const key = `${agentId}:${policyVersionId}:${datasetId}`;
  return {
    formatVersion: 1,
    policyVersionId,
    approvalEpoch,
    datasetId,
    agentKeyThumbprint: thumbprint,
    managed: managed.has(policyVersionId),
    canEnable,
    availableRoles: unsupported ? [] : ["viewer", "team_member"],
    assignments: assignments.get(key) ?? [],
  };
}

await page.route("**/api/**", async (route) => {
  const request = route.request();
  const url = new URL(request.url());
  const path = url.pathname;
  const respond = (json, status = 200) => route.fulfill({ status, json });
  if (path === "/api/operator/session")
    return respond({
      operatorId: "test-operator",
      csrfToken: "current-csrf",
      expiresAtUnixSeconds: now + 3600,
    });
  if (path === "/api/datasets") return respond({ datasets: [] });
  if (path === "/api/policies/versions/approved") return respond({ formatVersion: 1, policies });
  if (path === "/api/operator/agents") return respond({ agents });
  if (path.endsWith("/tokens")) return respond({ tokens: [] });
  const match = /^\/api\/operator\/agents\/([^/]+)\/roles$/.exec(path);
  if (!match) return respond({ error: "PORTAL_UNAVAILABLE" }, 503);
  const agentId = match[1];
  if (expired) return route.fulfill({ status: 401, body: "Session expired" });
  if (request.method() === "GET") {
    const scope = Object.fromEntries(url.searchParams);
    events.push({ operation: "list", agentId, ...scope });
    if (failRead) return respond({ error: "UNAVAILABLE" }, 503);
    return respond(state(agentId, scope));
  }
  assert.equal(request.headers()["x-raziel-csrf-token"], "current-csrf");
  const body = request.postDataJSON();
  const { formatVersion, operation, role, teamId, ...scope } = body;
  assert.equal(formatVersion, 1);
  assert.deepEqual(Object.keys(scope).sort(), ["approvalEpoch", "datasetId", "policyVersionId"]);
  events.push({
    operation,
    agentId,
    ...scope,
    ...(role ? { role } : {}),
    ...(teamId ? { teamId } : {}),
  });
  if (delayMutation)
    await new Promise((resolve) => {
      releaseMutation = resolve;
    });
  if (failNextMutation) {
    failNextMutation = false;
    return respond({ error: "ROLE_CONFLICT" }, 409);
  }
  const key = `${agentId}:${scope.policyVersionId}:${scope.datasetId}`;
  if (operation === "enable") {
    managed.add(scope.policyVersionId);
    assert.equal(role, undefined);
    assert.equal(teamId, undefined);
  } else {
    assert.equal(managed.has(scope.policyVersionId), true);
    const current = assignments.get(key) ?? [];
    const assignment = { role, ...(teamId ? { teamId } : {}) };
    assignments.set(
      key,
      operation === "add"
        ? [...current, assignment]
        : current.filter((item) => JSON.stringify(item) !== JSON.stringify(assignment)),
    );
  }
  if (loseNextMutation) {
    loseNextMutation = false;
    return route.fulfill({ status: 502, body: "Response lost" });
  }
  return respond(state(agentId, scope));
});

try {
  await page.goto(process.env.RAZIEL_UI_URL ?? "http://127.0.0.1:3218");
  await page.getByRole("button", { name: "Agents", exact: true }).click();
  const editor = page.getByRole("region", { name: "Roles and teams", exact: true });
  await editor.getByRole("button", { name: "Use operator-managed roles", exact: true }).waitFor();
  assert.equal(await page.getByText("No tokens issued.", { exact: true }).count(), 1);
  assert.equal(
    await editor.getByLabel("Dataset ID", { exact: true }).inputValue(),
    "processor-demo-v1",
  );
  assert.equal(
    await editor.getByRole("button", { name: "Add role", exact: true }).isDisabled(),
    true,
  );
  await editor.getByLabel("Role", { exact: true }).waitFor();
  assert.deepEqual(
    await editor.getByLabel("Role", { exact: true }).locator("option").allTextContents(),
    ["Viewer", "Team member"],
  );
  await editor.getByRole("button", { name: "Use operator-managed roles", exact: true }).click();
  await editor.getByRole("button", { name: "Confirm operator-managed roles", exact: true }).click();
  await editor.getByText("Operator-managed roles activated.", { exact: true }).waitFor();
  assert.equal(
    await editor.getByRole("button", { name: "Use operator-managed roles", exact: true }).count(),
    0,
  );

  delayMutation = true;
  await editor.getByRole("button", { name: "Add role", exact: true }).click();
  await editor.getByRole("button", { name: "Adding...", exact: true }).waitFor();
  assert.equal(await editor.getByLabel("Installed policy version").isDisabled(), true);
  assert.equal(await editor.getByLabel("Dataset ID", { exact: true }).isDisabled(), true);
  delayMutation = false;
  releaseMutation();
  await editor.getByText("Role assignments updated.", { exact: true }).waitFor();
  assert.equal(events.at(-1).operation, "list");
  assert.equal(
    await editor.getByRole("button", { name: "Add role", exact: true }).isDisabled(),
    true,
  );

  await editor.getByRole("button", { name: "Remove Viewer", exact: true }).click();
  failNextMutation = true;
  await editor.getByRole("button", { name: "Confirm removal", exact: true }).click();
  await editor
    .getByText(
      "The policy or role state changed. Review the refreshed assignments before retrying.",
      { exact: true },
    )
    .waitFor();
  assert.equal(await editor.getByRole("button", { name: "Remove Viewer", exact: true }).count(), 1);
  assert.equal(events.at(-1).operation, "list");

  await editor.getByLabel("Role", { exact: true }).selectOption("team_member");
  await editor.getByLabel("Team ID", { exact: true }).fill("finance-team");
  await editor.getByRole("button", { name: "Add role", exact: true }).click();
  await editor
    .getByRole("button", { name: "Remove Team member finance-team", exact: true })
    .waitFor();
  await editor
    .getByRole("button", { name: "Remove Team member finance-team", exact: true })
    .click();
  await editor
    .getByText("Remove this agent from this team across all datasets in this policy version?", {
      exact: true,
    })
    .waitFor();
  loseNextMutation = true;
  await editor.getByRole("button", { name: "Confirm removal", exact: true }).click();
  await editor
    .getByText("Role change not confirmed. Review the refreshed assignments before retrying.", {
      exact: true,
    })
    .waitFor();
  assert.equal(
    await editor
      .getByRole("button", { name: "Remove Team member finance-team", exact: true })
      .count(),
    0,
  );
  assert.equal(events.at(-1).operation, "list");

  failRead = true;
  await editor.getByRole("button", { name: "Refresh roles", exact: true }).click();
  await editor.getByText("Roles unavailable. Refresh roles to retry.", { exact: true }).waitFor();
  assert.equal(
    await editor.getByRole("button", { name: "Add role", exact: true }).isDisabled(),
    true,
  );
  failRead = false;
  await editor.getByRole("button", { name: "Refresh roles", exact: true }).click();
  await editor.getByRole("button", { name: "Remove Viewer", exact: true }).waitFor();

  await editor.getByLabel("Dataset ID", { exact: true }).fill("bank-demo-v1");
  assert.equal(await editor.getByRole("button", { name: "Remove Viewer", exact: true }).count(), 0);
  await editor.getByRole("button", { name: "View roles", exact: true }).click();
  await editor.getByText("No role assignments for this dataset.", { exact: true }).waitFor();
  assert.equal(events.at(-1).datasetId, "bank-demo-v1");
  await editor.getByRole("button", { name: "Add role", exact: true }).click();
  await editor.getByRole("button", { name: "Remove Viewer", exact: true }).waitFor();
  await mkdir(output, { recursive: true });
  await page.screenshot({ path: join(output, "agent-roles-desktop.png"), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(
    await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    true,
  );
  await page.screenshot({ path: join(output, "agent-roles-mobile.png"), fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });

  await editor
    .getByLabel("Installed policy version")
    .selectOption(`${policies[1].identity.versionId}:2`);
  await editor
    .getByText(
      "This version cannot switch to operator-managed roles. Existing relationships retain legacy automatic provisioning. Publish a fresh approved version with no relationships to switch.",
      { exact: true },
    )
    .waitFor();
  assert.equal(
    await editor.getByRole("button", { name: "Add role", exact: true }).isDisabled(),
    true,
  );
  await editor
    .getByLabel("Installed policy version")
    .selectOption(`${policies[2].identity.versionId}:3`);
  await editor
    .getByText(
      "Role editing is unavailable for this policy family. Caveat and organization policies are not supported.",
      { exact: true },
    )
    .waitFor();
  assert.equal(await editor.getByLabel("Role", { exact: true }).count(), 0);
  assert.equal(
    await editor.getByRole("button", { name: "Use operator-managed roles", exact: true }).count(),
    0,
  );

  await page.getByRole("button", { name: "Bank agent agent-two", exact: true }).click();
  await editor.getByRole("button", { name: "Refresh roles", exact: true }).waitFor();
  await editor.getByText("No role assignments for this dataset.", { exact: true }).waitFor();
  assert.equal(events.at(-1).agentId, "agent-two");
  expired = true;
  await editor.getByRole("button", { name: "Refresh roles", exact: true }).click();
  await page.getByLabel("Operator password", { exact: true }).waitFor();
  assert.deepEqual(pageErrors, []);
  process.stdout.write(
    `PASS: exact catalogue selection, managed activation, response-derived roles, add/remove, row errors and busy state, uncertain write refresh, dataset/agent switching, legacy/unsupported modes, session expiry, desktop/mobile layout. Screenshots: ${output}\n`,
  );
} finally {
  await browser.close();
}
