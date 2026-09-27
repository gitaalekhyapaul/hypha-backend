// Uses the installed Playwright module against a versioned console. No real policy is published.
// PLAYWRIGHT_MODULE=<module URL> RAZIEL_UI_URL=http://127.0.0.1:3215 node test/policy-version.ui.mjs
import assert from "node:assert/strict";
import { mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

assert.ok(process.env.PLAYWRIGHT_MODULE, "Set PLAYWRIGHT_MODULE to an installed module.");
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE);
const browser = await chromium.launch({ channel: "chrome", headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
const page = await context.newPage();
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const now = Math.floor(Date.now() / 1000);
const stamp = (seconds) => ({ seconds: String(seconds), nanos: 0 });
const familyId = "a".repeat(32);
const requests = [];
const publications = [];
let candidate;
let loseCompile = true;
let losePublish = true;
let expired = false;

await page.route("**/api/**", async (route) => {
  const request = route.request();
  const path = new URL(request.url()).pathname;
  const respond = (json, status = 200) => route.fulfill({ json, status });
  if (path === "/api/operator/session")
    return respond({
      operatorId: "ui-operator",
      csrfToken: "ui-csrf",
      expiresAtUnixSeconds: now + 3600,
    });
  if (path === "/api/datasets") return respond({ datasets: [] });
  assert.ok(path.startsWith("/api/policies/versions"), `Unexpected API: ${path}`);
  if (expired) return route.fulfill({ status: 401, body: "expired" });
  if (request.method() === "POST")
    assert.equal(request.headers()["x-raziel-csrf-token"], "ui-csrf");
  if (path === "/api/policies/versions") {
    const body = request.postDataJSON();
    requests.push(body);
    assert.equal(body.draft.policyVersion, 1);
    assert.equal(body.expectedApprovalEpoch, 1);
    assert.equal(body.draft.resourceType, "dataset");
    const version = body.familyId ? 2 : 1;
    const versionId = String(version).repeat(32);
    candidate = {
      identity: { familyId, versionId, version },
      candidate: {
        candidate_id: body.draft.candidateId,
        compiler_version: "1.54.0",
        record_id: `record-${version}`,
        policy_name: `family:${familyId}:v${version}`,
        policy_version: String(version),
        resource_type: `dataset_${versionId}`,
        permission: "query",
        rule_summary: body.draft.rule.kind,
        source_schema_digest: "b".repeat(64),
        policy_hash: "c".repeat(64),
        expected_approval_epoch: "1",
        valid_from: stamp(now),
        valid_until: stamp(now + 86400),
        created_at: stamp(now),
        expires_at: stamp(now + 900),
        status: "COMPILED",
      },
    };
    if (loseCompile) {
      loseCompile = false;
      return route.fulfill({ status: 502, body: "Response lost" });
    }
    return respond(candidate, 201);
  }
  if (path.endsWith("/publish")) {
    assert.equal(request.postData(), null);
    publications.push(path);
    assert.equal(path, `/api/policies/versions/${candidate.candidate.candidate_id}/publish`);
    if (losePublish) {
      losePublish = false;
      return route.fulfill({ status: 503, body: "Response lost" });
    }
    return respond({
      identity: candidate.identity,
      publication: {
        record_id: candidate.candidate.record_id,
        policy_name: candidate.candidate.policy_name,
        policy_version: candidate.candidate.policy_version,
        policy_hash: candidate.candidate.policy_hash,
        approval_epoch: "1",
        status: "APPROVED",
        registered_at: stamp(now),
        valid_from: stamp(now),
        valid_until: stamp(now + 86400),
        registry_generation: String(candidate.identity.version),
      },
    });
  }
  assert.equal(request.method(), "GET");
  assert.equal(path, `/api/policies/versions/${candidate.candidate.candidate_id}`);
  return respond(candidate);
});

try {
  await page.goto(process.env.RAZIEL_UI_URL ?? "http://127.0.0.1:3215");
  await page.getByRole("button", { name: "Policy builder", exact: true }).click();
  await page.getByText("Versioned policies", { exact: true }).waitFor();
  assert.equal(await page.getByLabel("Policy version", { exact: true }).count(), 0);
  assert.equal(await page.getByLabel("Approval epoch", { exact: true }).count(), 0);
  assert.equal(await page.getByLabel("Resource type", { exact: true }).count(), 0);

  for (const rule of [
    "Team membership",
    "Alternative paths",
    "Dual control",
    "Caveated access",
    "Direct assignment",
  ]) {
    await page.getByRole("tab", { name: rule, exact: true }).click();
    assert.equal(await page.getByRole("group", { name: "Policy rule preview" }).count(), 1);
  }
  const compile = page.getByRole("button", { name: "Compile version", exact: true });
  await compile.evaluate((button) => {
    button.click();
    button.click();
  });
  await page.getByText("Outcome unconfirmed. This candidate is retained for retry.").waitFor();
  assert.equal(requests.length, 1);
  assert.equal(await page.getByLabel("Family selection").isDisabled(), true);
  assert.equal(
    await page.getByRole("button", { name: "New policy family", exact: true }).isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "Retry compile", exact: true }).click();
  await page.getByRole("button", { name: "Publish version", exact: true }).waitFor();
  assert.equal(requests.length, 2);
  assert.deepEqual(requests[0], requests[1]);
  assert.equal(
    await page.getByRole("button", { name: "Version compiled", exact: true }).isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "Publish version", exact: true }).click();
  await page.getByText("Outcome unconfirmed. This candidate is retained for retry.").waitFor();
  assert.equal(
    await page.getByRole("button", { name: "Draft next version", exact: true }).isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "Refresh candidate", exact: true }).click();
  await page.getByText("Candidate status refreshed.", { exact: true }).waitFor();
  assert.equal(
    await page.getByRole("button", { name: "Draft next version", exact: true }).isDisabled(),
    true,
  );
  await page.getByRole("button", { name: "Publish version", exact: true }).click();
  await page.getByRole("button", { name: "Version published", exact: true }).waitFor();
  assert.equal(publications[0], publications[1]);

  await page.getByRole("button", { name: "Draft next version", exact: true }).click();
  assert.equal(await page.getByLabel("Family selection").inputValue(), "existing");
  assert.equal(await page.getByLabel("Policy family ID", { exact: true }).inputValue(), familyId);
  await page.getByRole("tab", { name: "Caveated access", exact: true }).click();
  await page.getByLabel("Caveat template", { exact: true }).selectOption("task_purpose_scope");
  await page.getByLabel("Task ID", { exact: true }).fill("task-two");
  await page.getByRole("button", { name: "Compile version", exact: true }).click();
  await page.getByRole("button", { name: "Publish version", exact: true }).waitFor();
  assert.equal(requests[2].familyId, familyId);
  assert.notEqual(requests[2].draft.candidateId, requests[1].draft.candidateId);
  assert.equal(requests[2].draft.rule.caveat.taskId, "task-two");
  await page.getByRole("button", { name: "Publish version", exact: true }).click();
  await page.getByRole("button", { name: "Version published", exact: true }).waitFor();
  assert.equal(await page.getByRole("heading", { name: "Access graph", exact: true }).count(), 0);
  const output = join(tmpdir(), "raziel-b0065-policy-ui");
  await mkdir(output, { recursive: true });
  await page.screenshot({ path: join(output, "version-published.png"), fullPage: true });
  await page.getByRole("button", { name: "New policy family", exact: true }).click();
  assert.equal(await page.getByLabel("Family selection").inputValue(), "new");
  await page.getByLabel("Family selection").selectOption("existing");
  await page.getByLabel("Policy family ID", { exact: true }).fill("invalid");
  await page.getByRole("button", { name: "Compile version", exact: true }).click();
  await page.getByRole("alert").waitFor();
  assert.equal(requests.length, 3);
  await page.getByLabel("Policy family ID", { exact: true }).fill(familyId);
  expired = true;
  await page.getByRole("button", { name: "Compile version", exact: true }).click();
  await page.getByLabel("Operator password", { exact: true }).waitFor();
  assert.deepEqual(errors, []);
  console.log(
    `PASS: rule controls, exact compile retry, double-click guard, uncertain publish retry, refresh, next version, family selection, safe review, and session expiry. Screenshot: ${output}/version-published.png`,
  );
} finally {
  await browser.close();
}
