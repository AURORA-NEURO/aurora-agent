import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousRunTraceSession,
  InMemoryAutonomousRunTraceStore,
  analyzeAutonomousRunTrace,
  validateAutonomousRunTraceAnalyticsReport,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

async function traceSnapshot({ includeFailure = true } = {}) {
  const store = new InMemoryAutonomousRunTraceStore({ clock: () => 100 });

  const healthy = new AutonomousRunTraceSession(store, { run_id: "healthy", task_digest: digest("a"), domains: ["coding"] });
  await healthy.started();
  await healthy.record({ phase: "provider_invocation_finished", status: "running", provider: "offline", model: "reasoner-v1", latency_ms: 12, input_tokens: 20, output_tokens: 8, tool_count: 2 });
  await healthy.complete({ status: "completed" });

  if (includeFailure) {
    const failed = new AutonomousRunTraceSession(store, { run_id: "failed", task_digest: digest("b"), domains: ["science"] });
    await failed.started();
    await failed.record({ phase: "provider_invocation_finished", status: "running", provider: "offline", model: "reasoner-v1", latency_ms: null, input_tokens: 11, output_tokens: null, tool_count: 0, failure_class: "ProviderRuntimeError", failure_code: "provider_timeout" });
    await failed.complete({ status: "failed", failure_code: "execution_error" });
  }

  const incomplete = new AutonomousRunTraceSession(store, { run_id: "incomplete", task_digest: digest("c"), domains: ["browser"] });
  await incomplete.started();
  await incomplete.record({ phase: "plan_compiled", status: "running" });
  return store.snapshot();
}

test("trace analytics preserves unmeasured state and aggregates dimensions", async () => {
  const report = analyzeAutonomousRunTrace(await traceSnapshot());
  assert.equal(report.status, "attention_required");
  assert.equal(report.run_count, 3);
  assert.equal(report.terminal_run_count, 2);
  assert.equal(report.incomplete_run_count, 1);
  assert.ok(Math.abs(report.terminal_coverage - 2 / 3) < 1e-12);
  assert.equal(report.provider_invocations, 2);
  assert.equal(report.provider_failures, 1);
  assert.equal(report.provider_failure_rate, 0.5);
  assert.equal(report.input_tokens, 31);
  assert.equal(report.output_tokens, 8);
  assert.equal(report.tool_calls, 2);
  assert.equal(report.latency_observation_count, 1);
  assert.equal(report.latency_p95_ms, 12);
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);

  const byDomain = Object.fromEntries(report.domains.map((row) => [row.identity, row]));
  assert.equal(byDomain.coding.measurement_state, "measured");
  assert.equal(byDomain.coding.failure_rate, 0);
  assert.equal(byDomain.browser.measurement_state, "measured");
  assert.equal(byDomain.browser.failure_rate, null);
  assert.equal(byDomain.browser.latency_p95_ms, null);
  assert.equal(byDomain.evaluation.measurement_state, "unmeasured");
  assert.equal(byDomain.evaluation.failure_rate, null);
  assert.equal(byDomain.evaluation.latency_p95_ms, null);

  assert.equal(report.providers[0].identity, "offline");
  assert.equal(report.providers[0].failure_codes[0], "provider_timeout");
  assert.equal(report.models[0].identity, "offline/reasoner-v1");
  assert.equal(report.models[0].input_token_observation_count, 2);
  assert.equal(report.models[0].output_token_observation_count, 1);
  assert.ok(report.alerts.some((alert) => alert.code === "provider_failure_rate" && alert.severity === "critical"));
  assert.ok(report.alerts.some((alert) => alert.code === "run_not_terminal"));
});

test("trace analytics policy controls alerts and report tampering is rejected", async () => {
  const snapshot = await traceSnapshot({ includeFailure: false });
  const quiet = {
    failure_rate_warning: 1,
    failure_rate_critical: 1,
    p95_latency_warning_ms: null,
    p95_latency_critical_ms: null,
    warn_on_incomplete_runs: false,
    warn_on_unmeasured_domains: true,
  };
  const report = analyzeAutonomousRunTrace(snapshot, { policy: quiet });
  assert.equal(report.status, "observed");
  assert.ok(report.alerts.some((alert) => alert.code === "domain_unmeasured" && alert.severity === "info"));
  assert.equal(report.alerts.some((alert) => alert.code === "run_not_terminal"), false);
  assert.deepEqual(validateAutonomousRunTraceAnalyticsReport(report), report);

  const tampered = structuredClone(report);
  tampered.status = "degraded";
  assert.throws(() => validateAutonomousRunTraceAnalyticsReport(tampered), /digest|reconcile/);

  const tamperedSnapshot = structuredClone(snapshot);
  tamperedSnapshot.events[0].status = "failed";
  assert.throws(() => analyzeAutonomousRunTrace(tamperedSnapshot), /digest|hash chain|invalid/);

  assert.throws(() => analyzeAutonomousRunTrace(snapshot, { policy: { failure_rate_warning: 0.9, failure_rate_critical: 0.1 } }), /cannot exceed/);
});

test("trace analytics is exposed by the agent facade and contains no value payloads", async () => {
  const agent = Object.create(AutonomousAgent.prototype);
  const report = agent.analyzeRunTrace(await traceSnapshot());
  const wire = JSON.stringify(report);
  assert.equal(wire.includes("healthy"), false);
  assert.equal(wire.includes("reasoner-v1"), true);
  assert.equal(Object.keys(report).some((key) => ["task", "prompt", "response", "messages", "credentials", "arguments", "payload"].includes(key)), false);
  assert.equal(report.cost_posture, "not_measured_by_trace");
  assert.equal(report.secret_material, "never_returned");
});
