import assert from "node:assert/strict";
import test from "node:test";

import {
  JsonProviderQuotaPersistence,
  LLMRuntime,
  ProviderQuotaController,
  ProviderQuotaExceededError,
  ProviderRuntimeError,
  validateProviderQuotaSnapshot,
} from "../dist/index.js";

function request() {
  return {
    model: "offline-model",
    messages: [{ role: "user", content: "offline fixture prompt" }],
    maxOutputTokens: 256,
  };
}

test("provider/model quotas reserve, settle, fence concurrency, and roll windows", async () => {
  let now = 1_000_000;
  const quota = new ProviderQuotaController({ clock: () => now });
  quota.setPolicy({ provider: "offline", model: "offline-model", windowMs: 1_000, maxRequests: 2, maxOutputTokens: 512, maxConcurrent: 1 });

  const first = quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 12, outputTokens: 256 });
  assert.equal(quota.status("offline", "offline-model")[0].requests_reserved, 1);
  assert.throws(
    () => quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 1, outputTokens: 1 }),
    (error) => error instanceof ProviderQuotaExceededError && error.code === "quota_exceeded" && error.dimensions.includes("concurrent"),
  );
  assert.throws(() => first.settle(), (error) => error instanceof ProviderRuntimeError && error.code === "protocol");
  first.markDispatched();
  const settlement = first.settle({ inputTokens: 8, outputTokens: 32, costUnits: 1.5 });
  assert.equal(settlement.charged_output_tokens, 32);
  assert.equal(quota.status("offline", "offline-model")[0].requests_used, 1);

  const second = quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 1, outputTokens: 1 });
  second.markDispatched();
  second.settle({ inputTokens: 1, outputTokens: 1 });
  assert.throws(
    () => quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 1, outputTokens: 1 }),
    (error) => error instanceof ProviderQuotaExceededError && error.dimensions.includes("requests"),
  );
  now += 1_000;
  const nextWindow = quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 1, outputTokens: 1 });
  nextWindow.release();
  assert.equal(quota.status("offline", "offline-model")[0].window_start, 1_001_000);
});

test("invalid authoritative usage leaves a dispatched reservation recoverable", () => {
  const quota = new ProviderQuotaController({ clock: () => 1_500_000 });
  quota.setPolicy({ provider: "recoverable", model: "recoverable-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const reservation = quota.reserve({ provider: "recoverable", model: "recoverable-model", inputTokens: 4, outputTokens: 8 });
  reservation.markDispatched();

  assert.throws(
    () => reservation.settle({ inputTokens: 2_000_000_001, outputTokens: 0 }),
    /provider quota settlement inputTokens/,
  );
  assert.equal(quota.status("recoverable", "recoverable-model")[0].concurrent, 1);

  const settlement = reservation.settle({ inputTokens: 4, outputTokens: 2 });
  assert.equal(settlement.charged_input_tokens, 4);
  const [status] = quota.status("recoverable", "recoverable-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
});

test("quota snapshots are canonical, digest checked, and metadata-only", async () => {
  const quota = new ProviderQuotaController({ clock: () => 2_000_000 });
  quota.setPolicy({ provider: "offline", model: "offline-model", windowMs: 10_000, maxRequests: 5, maxOutputTokens: 512 });
  const reservation = quota.reserve({ provider: "offline", model: "offline-model", inputTokens: 2, outputTokens: 8 });
  reservation.markDispatched();
  reservation.settle({ inputTokens: 2, outputTokens: 3 });
  const snapshot = await quota.snapshot();
  assert.equal(snapshot.retention, "metadata_only;provider_model_counters_no_prompts_credentials_or_payloads");
  assert.equal(snapshot.secret_material, "never_returned");
  assert.equal(JSON.stringify(snapshot).includes("offline fixture prompt"), false);

  let encoded = null;
  const persistence = new JsonProviderQuotaPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
  });
  await persistence.write(snapshot);
  assert.deepEqual(await persistence.read(), snapshot);
  encoded = encoded.replace('"requests":1', '"requests":2');
  await assert.rejects(() => persistence.read(), (error) => error instanceof ProviderRuntimeError && error.code === "protocol");
  await assert.rejects(() => validateProviderQuotaSnapshot({ ...snapshot, snapshot_digest: "0".repeat(64) }), (error) => error instanceof ProviderRuntimeError && error.code === "protocol");
});

test("LLMRuntime enforces one shared quota across the provider transport boundary", async () => {
  let calls = 0;
  const quota = new ProviderQuotaController({ clock: () => 3_000_000 });
  quota.setPolicy({ provider: "offline", model: "offline-model", windowMs: 10_000, maxRequests: 1, maxOutputTokens: 512 });
  const runtime = new LLMRuntime({
    fetch: async () => { throw new Error("network must not be reached"); },
    providerQuota: quota,
  });
  runtime.registerInMemoryProvider("offline", () => {
    calls += 1;
    return { output_text: "deterministic fixture", usage: { input_tokens: 4, output_tokens: 6 } };
  });

  const response = await runtime.invoke("offline", request());
  assert.equal(response.text, "deterministic fixture");
  assert.equal(calls, 1);
  assert.equal(quota.status("offline", "offline-model")[0].requests_used, 1);
  await assert.rejects(() => runtime.invoke("offline", request()), (error) => error instanceof ProviderQuotaExceededError && error.code === "quota_exceeded");
  assert.equal(calls, 1);
});
