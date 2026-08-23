import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MAX_PROVIDER_CONFORMANCE_CHECKS,
  PROVIDER_PROTOCOL_CONFORMANCE_MODE,
  ProviderRuntimeError,
  assertProviderProtocolConformance,
  runProviderProtocolConformance,
} from "../dist/index.js";

test("keyless conformance covers every built-in provider protocol without network or secret retention", async () => {
  const report = await runProviderProtocolConformance();

  assert.equal(report.mode, PROVIDER_PROTOCOL_CONFORMANCE_MODE);
  assert.equal(report.status, "passed");
  assert.equal(report.provider_count, 7);
  assert.equal(report.passed_provider_count, 7);
  assert.equal(report.failed_provider_count, 0);
  assert.equal(report.check_count, MAX_PROVIDER_CONFORMANCE_CHECKS);
  assert.equal(report.failed_check_count, 0);
  assert.equal(report.transport, "intercepted_fetch_never_networked");
  assert.equal(report.retention, "metadata_only;request_response_and_credentials_not_retained");
  assert.equal(report.secret_material, "never_returned");
  assert.match(report.report_digest, /^[a-f0-9]{64}$/);
  assert.equal(JSON.stringify(report).includes("offline-fixture-token"), false);
  assertProviderProtocolConformance(report);

  for (const provider of report.providers) {
    assert.equal(provider.status, "passed", provider.provider);
    assert.equal(provider.check_count, 8, provider.provider);
    assert.equal(provider.passed_check_count, 8, provider.provider);
    assert.equal(provider.fixture_call_count, 3, provider.provider);
  }
});

test("conformance can gate one provider and rejects duplicate or unsupported selections", async () => {
  const report = await runProviderProtocolConformance({ providers: ["anthropic"], model: "fixture-custom-model" });
  assert.equal(report.status, "passed");
  assert.equal(report.provider_count, 1);
  assert.equal(report.providers[0].provider, "anthropic");

  await assert.rejects(
    runProviderProtocolConformance({ providers: ["openai", "openai"] }),
    (error) => error instanceof ProviderRuntimeError,
  );
  await assert.rejects(
    runProviderProtocolConformance({ providers: ["not-a-provider"] }),
    (error) => error instanceof ProviderRuntimeError,
  );
});
