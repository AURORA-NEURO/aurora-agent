import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousOnlineLearner,
  AutonomousOnlineLearnerPersistenceCoordinator,
  AutonomousHttpSnapshotTextStore,
  TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence,
  ArgumentError,
  ResponseTooLargeError,
  TransportError,
} from "../dist/index.js";

function response(body, status = 200, headers = {}) {
  return new Response(body, { status, headers: { "content-type": "application/json", ...headers } });
}

function snapshot(domain, version) {
  return JSON.stringify({ schema: "test-snapshot/0.1", domain, version, metadata_only: true });
}

test("HTTP snapshot text store supports all domains, protected transient headers, and CAS", async () => {
  const values = new Map();
  const requests = [];
  const fetch = async (url, init) => {
    const headers = new Headers(init?.headers);
    const resource = headers.get("x-aurora-snapshot-resource");
    requests.push({ url: String(url), method: init?.method, resource, authorization: headers.get("authorization"), ifMatch: headers.get("if-match"), ifNoneMatch: headers.get("if-none-match") });
    if (init.method === "GET") return values.has(resource) ? response(values.get(resource)) : response(null, 404);
    const current = values.get(resource);
    if (headers.get("if-none-match") === "*" && current !== undefined) return response(null, 412);
    if (headers.get("if-match") !== null) {
      const expected = headers.get("if-match").replaceAll('"', "");
      const observed = current === undefined ? null : JSON.parse(current).version_digest;
      if (expected !== observed) return response(null, 412);
    }
    const body = String(init.body);
    const parsed = JSON.parse(body);
    parsed.version_digest = parsed.version_digest ?? "a".repeat(64);
    values.set(resource, JSON.stringify(parsed));
    return response(null, 204);
  };
  const seenContexts = [];
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const store = new AutonomousHttpSnapshotTextStore({
      endpoint: "https://snapshots.test/v1/state",
      allowedHosts: ["snapshots.test"],
      resource: `${domain}/learning-state`,
      fetch,
      headerResolver: (context) => {
        seenContexts.push(context);
        return { authorization: "unit-test-transient-auth" };
      },
    });
    assert.equal(await store.read(), null);
    assert.equal(await store.writeIfUnchanged(null, snapshot(domain, "one")), true);
    assert.equal(await store.writeIfUnchanged(null, snapshot(domain, "two")), false);
    const current = JSON.parse(await store.read());
    assert.equal(current.domain, domain);
    assert.equal(await store.writeIfUnchanged(current.version_digest, snapshot(domain, "three")), true);
    assert.equal(store.describe().credentials, "transient_header_resolver;never_returned");
    assert.equal(store.describe().resource, `${domain}/learning-state`);
  }
  assert.equal(seenContexts.length, AUTONOMOUS_DOMAIN_NAMES.length * 5);
  assert.ok(seenContexts.every((context) => context.expected_snapshot_digest === null || typeof context.expected_snapshot_digest === "string"));
  assert.ok(requests.every((request) => request.authorization === "unit-test-transient-auth"));
  assert.ok(requests.every((request) => !JSON.stringify(request).includes("snapshot-secret")));
  assert.ok(requests.some((request) => request.ifNoneMatch === "*"));
  assert.ok(requests.some((request) => request.ifMatch !== null));
});

test("HTTP snapshot store enforces endpoint, request, response, and protocol bounds", async () => {
  assert.throws(() => new AutonomousHttpSnapshotTextStore({ endpoint: "http://snapshots.test/state", allowedHosts: ["snapshots.test"], resource: "state", fetch: async () => response(null, 404) }), ArgumentError);
  assert.throws(() => new AutonomousHttpSnapshotTextStore({ endpoint: "https://snapshots.test/state", allowedHosts: ["other.test"], resource: "state", fetch: async () => response(null, 404) }), ArgumentError);
  const oversized = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    maxResponseBytes: 32,
    fetch: async () => response("x".repeat(100), 200),
  });
  await assert.rejects(() => oversized.read(), ResponseTooLargeError);
  const malformed = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    fetch: async () => response("[]", 200),
  });
  await assert.rejects(() => malformed.read(), ArgumentError);
  const oversizedRequest = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    maxRequestBytes: 16,
    fetch: async () => response(null, 204),
  });
  await assert.rejects(() => oversizedRequest.write(JSON.stringify({ large: "x".repeat(100) })), ArgumentError);
  const rejectedHeaders = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    headerResolver: () => ({ "x-invalid": "line\nbreak" }),
    fetch: async () => response(null, 404),
  });
  await assert.rejects(() => rejectedHeaders.read(), ArgumentError);
});

test("HTTP snapshot store separates CAS conflicts from transport failures and respects timeout/abort", async () => {
  const conflict = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    fetch: async () => response(null, 409),
  });
  assert.equal(await conflict.writeIfUnchanged("a".repeat(64), JSON.stringify({ state: "next" })), false);

  const timedOut = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    timeoutMs: 100,
    fetch: async (_url, init) => await new Promise((_, reject) => {
      if (init.signal.aborted) reject(new Error("aborted"));
      else init.signal.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
    }),
  });
  await assert.rejects(() => timedOut.read(), (error) => error instanceof TransportError && /timed out/.test(error.message));

  const controller = new AbortController();
  controller.abort();
  const aborted = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/state",
    allowedHosts: ["snapshots.test"],
    resource: "state",
    signal: controller.signal,
    fetch: async (_url, init) => await new Promise((_, reject) => {
      if (init.signal.aborted) reject(new Error("aborted"));
      else init.signal.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
    }),
  });
  await assert.rejects(() => aborted.read(), (error) => error instanceof TransportError && /aborted/.test(error.message));
});

test("HTTP snapshot text store plugs into an existing transactional learner persistence contract", async () => {
  let remote = null;
  const fetch = async (_url, init) => {
    const headers = new Headers(init.headers);
    if (init.method === "GET") return remote === null ? response(null, 404) : response(remote);
    if (headers.get("if-none-match") === "*" && remote !== null) return response(null, 412);
    if (headers.get("if-match") !== null) {
      const expected = headers.get("if-match").replaceAll('"', "");
      if (remote === null || JSON.parse(remote).snapshot_digest !== expected) return response(null, 412);
    }
    remote = String(init.body);
    return response(null, 204);
  };
  const store = new AutonomousHttpSnapshotTextStore({
    endpoint: "https://snapshots.test/learner",
    allowedHosts: ["snapshots.test"],
    resource: "all-domains/online-learner",
    fetch,
  });
  const learner = new AutonomousOnlineLearner();
  const persistence = new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(store);
  const coordinator = new AutonomousOnlineLearnerPersistenceCoordinator(learner, persistence);
  assert.equal(await coordinator.restore(), null);
  const first = await coordinator.flush();
  assert.equal(JSON.parse(remote).snapshot_digest, first.snapshot_digest);
  const restarted = new AutonomousOnlineLearner();
  const restored = new AutonomousOnlineLearnerPersistenceCoordinator(restarted, new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(store));
  assert.equal((await restored.restore()).snapshot_digest, first.snapshot_digest);
  const second = await restored.flush();
  assert.match(second.snapshot_digest, /^[0-9a-f]{64}$/);
});
