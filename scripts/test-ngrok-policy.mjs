import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const policy = JSON.parse(
  readFileSync(new URL("../tools/ngrok/traffic-policy.json", import.meta.url), "utf8"),
);
assert.equal(policy.on_http_request.length, 1);
const rule = policy.on_http_request[0];
assert.equal(rule.expressions.length, 1);
const expression = rule.expressions[0];
assert.ok(expression.startsWith("!(") && expression.endsWith(")"));
const paths = [
  ...expression.matchAll(/req.method == '(GET|POST)' && req.url.path.matches\('([^']+)'\)/g),
].map((match) => ({ method: match[1], path: new RegExp(match[2]) }));
assert.equal(paths.length, 2);
const id = "22222222-2222-4222-8222-222222222222";
const allowed = [
  ["GET", "/api/health"],
  ["GET", "/api/agent/v1/me"],
  ["GET", "/api/agent/v1/access-requests"],
  ["GET", `/api/agent/v1/access-requests/${id}`],
  ["POST", "/api/agent/v1/access-requests"],
  ...["resume", "issuance", "cancel"].map((suffix) => [
    "POST",
    `/api/agent/v1/access-requests/${id}/${suffix}`,
  ]),
  ["POST", "/api/datasets/processor-demo-v1/protected-query"],
  ["POST", "/api/datasets/ledger%3Av1/protected-query"],
];
const denied = [
  ["POST", "/api/health"],
  ["HEAD", "/api/health"],
  ["GET", "/api/health/details"],
  ["GET", "/api/health/"],
  ["GET", "/"],
  ["POST", "/api/operator/login"],
  ["GET", "/api/operator/agents"],
  ["GET", "/api/datasets"],
  ["POST", "/api/datasets/seed-reconciliation"],
  ["GET", "/api/policies/versions"],
  ["POST", "/api/access-graphs"],
  ["GET", "/api/datasets/processor-demo-v1/protected-query"],
  ["DELETE", `/api/agent/v1/access-requests/${id}`],
  ["POST", "/api/agent/v1/../../operator/login"],
  ["POST", "/api/agent/v1/%2e%2e/%2e%2e/operator/login"],
  ["POST", "/api/datasets/a/b/protected-query"],
];
for (const [method, path] of allowed)
  assert.ok(
    paths.some((rule) => rule.method === method && rule.path.test(path)),
    path,
  );
for (const [method, path] of denied)
  assert.ok(!paths.some((rule) => rule.method === method && rule.path.test(path)), path);
assert.deepEqual(rule.actions, [
  {
    type: "custom-response",
    config: {
      status_code: 404,
      body: '{"error":"NOT_FOUND"}',
      headers: { "content-type": "application/json", "cache-control": "no-store" },
    },
  },
]);
console.log(
  `ngrok_policy_static=pass allowed=${allowed.length} denied=${denied.length} cloud_validation=not-run`,
);
