import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  createAutonomousHttpConnectorExecutor,
} from "../dist/index.js";

function manifest() {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "http-connector",
    version: "1.0.0",
    provider: "local-loopback-test",
    connector_kind: "provider_api",
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
    capabilities: ["evidence_read"],
    transport: "caller_managed",
    auth_posture: { status: "delegated", secret_refs: ["opaque-session-ref"], does_not_claim: ["credential validity"] },
  };
}

function response(payload, status = 200, contentType = "application/json") {
  const bytes = new TextEncoder().encode(payload);
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: new Headers({ "content-type": contentType }),
    body: null,
    arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
  };
}

function policy(overrides = {}) {
  return new AutonomousHttpConnectorPolicy({ allowedHosts: ["example.test"], requireHttps: false, ...overrides });
}

test("HTTP executor is domain-neutral and keeps resolver credentials transient", async () => {
  const calls = [];
  const executor = createAutonomousHttpConnectorExecutor(
    async (_manifest, request) => new AutonomousHttpConnectorRequest({ method: "GET", url: `http://example.test/evidence/${request.domain}` }),
    {
      policy: policy(),
      headerResolver: async () => ({ Authorization: "Bearer transient-test-only" }),
      fetch: async (url, init) => {
        calls.push({ url, init });
        const domain = url.split("/").at(-1);
        return response(JSON.stringify({ domain, records: 1 }));
      },
    },
  );

  const results = await Promise.all(AUTONOMOUS_DOMAIN_NAMES.map((domain) => executor(manifest(), { domain })));

  assert.deepEqual(results.map((result) => result.status), AUTONOMOUS_DOMAIN_NAMES.map(() => "observed"));
  assert.deepEqual(results.map((result) => result.value.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(calls.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(calls.every(({ init }) => init.headers.Authorization === "Bearer transient-test-only"));
  assert.ok(results.every((result) => !JSON.stringify(result).includes("transient-test-only")));
});

test("HTTP failures are projected without reading or retaining provider bodies", async () => {
  const cases = [[401, "refused", "auth_refused"], [403, "refused", "auth_refused"], [404, "refused", "not_found"], [429, "error", "rate_limited"], [500, "error", "http_5xx"]];
  for (const [status, expectedStatus, expectedFailure] of cases) {
    const executor = createAutonomousHttpConnectorExecutor(
      () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/status" }),
      { policy: policy(), fetch: async () => response("provider-secret-body", status) },
    );
    const result = await executor(manifest(), {});
    assert.equal(result.status, expectedStatus);
    assert.equal(result.failure_class, expectedFailure);
    assert.deepEqual(result.value, { status_code: status });
  }
});

test("HTTP response bounds, parsing, and timeout failures are explicit", async () => {
  const invalid = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/plain" }),
    { policy: policy(), fetch: async () => response("not-json", 200, "text/plain") },
  );
  const oversized = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/large" }),
    { policy: policy({ maxResponseBytes: 4 }), fetch: async () => response("12345") },
  );
  const timedOut = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/timeout" }),
    {
      policy: policy({ timeoutMs: 100 }),
      fetch: async (_url, init) => new Promise((_resolve, reject) => init.signal.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")))),
    },
  );
  const transportError = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/transport" }),
    { policy: policy(), fetch: async () => { throw new Error("offline"); } },
  );

  const invalidResult = await invalid(manifest(), {});
  const oversizedResult = await oversized(manifest(), {});
  const timeoutResult = await timedOut(manifest(), {});
  const transportResult = await transportError(manifest(), {});

  assert.equal(invalidResult.status, "partial");
  assert.equal(invalidResult.failure_class, "invalid_json");
  assert.equal(invalidResult.value.content_type, "text/plain");
  assert.equal(invalidResult.value.body_digest.length, 64);
  assert.equal(oversizedResult.status, "error");
  assert.equal(oversizedResult.failure_class, "response_too_large");
  assert.equal(oversizedResult.value.body_digest.length, 64);
  assert.equal(timeoutResult.failure_class, "timeout");
  assert.equal(transportResult.failure_class, "transport_error");
});

test("HTTP admission rejects credentials, unsafe headers, and unapproved hosts", async () => {
  assert.throws(() => new AutonomousHttpConnectorRequest({ method: "POST", url: "http://example.test", body: { api_key: "never" } }), /credential-shaped/);
  assert.throws(() => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test", headers: { "X-Test": "ok\r\nInjected: yes" } }), /header value/);
  const secretQuery = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test?access_token=never" }),
    { policy: policy(), fetch: async () => response("{}") },
  );
  await assert.rejects(() => secretQuery(manifest(), {}), /query/);
  const otherHost = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://other.test" }),
    { policy: policy(), fetch: async () => response("{}") },
  );
  await assert.rejects(() => otherHost(manifest(), {}), /allowlist/);
  const httpsOnly = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test" }),
    { policy: new AutonomousHttpConnectorPolicy({ allowedHosts: ["example.test"] }), fetch: async () => response("{}") },
  );
  await assert.rejects(() => httpsOnly(manifest(), {}), /HTTPS/);
});

test("credential-shaped JSON responses fail closed instead of becoming evidence", async () => {
  const executor = createAutonomousHttpConnectorExecutor(
    () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://example.test/secret" }),
    { policy: policy(), fetch: async () => response(JSON.stringify({ access_token: "never" })) },
  );

  await assert.rejects(() => executor(manifest(), {}), ArgumentError);
});
