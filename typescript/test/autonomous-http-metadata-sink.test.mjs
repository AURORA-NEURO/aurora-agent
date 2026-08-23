import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA,
  ArgumentError,
  AutonomousHttpConnectorPolicy,
  AutonomousHttpMetadataEventSink,
  TransportError,
} from "../dist/index.js";

function response(status = 204, body = null) {
  return new Response(body === null ? null : JSON.stringify(body), {
    status,
    headers: body === null ? {} : { "content-type": "application/json" },
  });
}

function event(domain, sequence = 1) {
  return {
    schema: "bioprism-typescript-autonomous-run-trace-event/0.1",
    run_id: `metadata-export-${domain}`,
    sequence,
    domains: [domain],
    phase: "completed",
    status: "completed",
    event_digest: "a".repeat(64),
    retention: "metadata_only_no_prompts_responses_or_tool_payloads",
    secret_material: "never_returned",
  };
}

function sink(fetch, options = {}) {
  return new AutonomousHttpMetadataEventSink({
    endpoint: "https://collector.test/v1/metadata",
    policy: new AutonomousHttpConnectorPolicy({ allowedHosts: ["collector.test"], allowedMethods: ["POST"] }),
    fetch,
    ...options,
  });
}

test("HTTP metadata sink exports all domains with a bounded metadata-only envelope", async () => {
  const requests = [];
  const exporter = sink(async (url, init) => {
    requests.push({
      url: String(url),
      method: init.method,
      headers: new Headers(init.headers),
      body: JSON.parse(String(init.body)),
    });
    return response();
  }, {
    sourceId: "runtime-test",
    headerResolver: () => ({ authorization: "transient-test-credential" }),
  });

  const result = await exporter.emitBatch(AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => event(domain, index + 1)));
  assert.equal(result.requested, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.exported, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.already_exported, 0);
  assert.equal(result.refused, 0);
  assert.equal(result.failed, 0);
  assert.match(result.batch_digest, /^[0-9a-f]{64}$/);
  assert.equal(requests.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(requests.every((request) => request.method === "POST"));
  assert.ok(requests.every((request) => request.url === "https://collector.test/v1/metadata"));
  assert.ok(requests.every((request) => request.headers.get("authorization") === "transient-test-credential"));
  assert.ok(requests.every((request) => request.body.schema === AUTONOMOUS_HTTP_METADATA_SINK_REQUEST_SCHEMA));
  assert.ok(requests.every((request) => request.body.event_digest === request.body.idempotency_key));
  assert.ok(requests.every((request) => request.body.event.schema.endsWith("run-trace-event/0.1")));
  assert.ok(requests.every((request) => !JSON.stringify(request.body).includes("transient-test-credential")));
  assert.equal(exporter.describe().endpoint_host, "collector.test");
  assert.equal(exporter.describe().secret_material, "never_returned");
});

test("HTTP metadata sink retries only transient delivery failures and treats collector 409 as idempotent success", async () => {
  let attempts = 0;
  const delays = [];
  const retrying = sink(async () => {
    attempts += 1;
    return attempts === 1 ? response(503) : response();
  }, { maxAttempts: 3, retryDelayMs: 7, sleep: async (milliseconds) => delays.push(milliseconds) });
  const exported = await retrying.emit(event("coding"));
  assert.equal(exported.status, "exported");
  assert.equal(exported.attempts, 2);
  assert.deepEqual(delays, [7]);

  const duplicate = sink(async () => response(409));
  const receipt = await duplicate.emit(event("browser"));
  assert.equal(receipt.status, "already_exported");
  assert.equal(receipt.status_code, 409);
  assert.equal(receipt.failure_class, "already_exists");
  assert.equal(receipt.retryable, false);
});

test("HTTP metadata sink preserves refusal and transport failure semantics without returning response bodies", async () => {
  const refused = sink(async () => response(401));
  const refusal = await refused.emit(event("data"));
  assert.equal(refusal.status, "refused");
  assert.equal(refusal.failure_class, "auth_refused");
  assert.equal(refusal.retryable, false);
  await assert.rejects(() => refused.asSink()(event("data", 2)), (error) => error instanceof TransportError);

  let calls = 0;
  const unavailable = sink(async () => {
    calls += 1;
    throw new Error("collector unavailable");
  }, { maxAttempts: 2, retryDelayMs: 0, sleep: async () => {} });
  const failed = await unavailable.emit(event("science"));
  assert.equal(failed.status, "failed");
  assert.equal(failed.failure_class, "transport_error");
  assert.equal(failed.attempts, 2);
  assert.equal(calls, 2);
  assert.equal(failed.status_code, null);
});

test("HTTP metadata sink rejects secrets, unsupported schemas, unsafe policies, and oversized batches", async () => {
  const exporter = sink(async () => response());
  await assert.rejects(() => exporter.emit({ ...event("operations"), prompt: "do not export" }), ArgumentError);
  await assert.rejects(() => exporter.emit({ schema: "unknown/0.1", status: "completed" }), ArgumentError);
  await assert.rejects(() => exporter.emit({ ...event("enterprise"), content: "raw response" }), ArgumentError);
  await assert.rejects(() => exporter.emit({ ...event("evaluation"), value: "credential-shaped" }), ArgumentError);
  await assert.rejects(() => exporter.emit({ ...event("evaluation"), metadata: "x".repeat(25_000) }), ArgumentError);
  await exporter.emit({ ...event("cross_domain"), schema: "bioprism-typescript-autonomous-workflow-portfolio-execution-trace-event/0.1" });
  await assert.rejects(() => exporter.emitBatch([]), ArgumentError);
  await assert.rejects(() => exporter.emitBatch(Array.from({ length: 257 }, () => event("coding"))), ArgumentError);
  assert.throws(() => new AutonomousHttpMetadataEventSink({
    endpoint: "https://collector.test/v1/metadata",
    policy: new AutonomousHttpConnectorPolicy({ allowedHosts: ["collector.test"], allowedMethods: ["GET"] }),
    fetch: async () => response(),
  }), ArgumentError);
});

test("HTTP metadata sink can be used as an autonomous trace event callback", async () => {
  const events = [];
  const exporter = sink(async (_url, init) => {
    events.push(JSON.parse(String(init.body)));
    return response();
  });
  const write = exporter.asSink();
  await write(event("multi_agent"));
  assert.equal(events.length, 1);
  assert.equal(events[0].event.domains[0], "multi_agent");
  assert.equal(events[0].retention, "metadata_only_event_identity_and_delivery_status");
  assert.equal(events[0].secret_material, "never_returned");
});
