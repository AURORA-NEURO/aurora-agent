import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousRunAnalyticsLedger,
  AutonomousRunAnalyticsLedgerPersistenceCoordinator,
  JsonAutonomousRunAnalyticsLedgerPersistence,
  TransactionalJsonAutonomousRunAnalyticsLedgerPersistence,
  AutonomousRunTraceSession,
  InMemoryAutonomousRunTraceStore,
  analyzeAutonomousRunTrace,
  digestJsonSync,
  validateAutonomousRunAnalyticsLedgerSnapshot,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

async function report(marker, domain) {
  const store = new InMemoryAutonomousRunTraceStore({ clock: () => 100 });
  const session = new AutonomousRunTraceSession(store, { run_id: `run-${marker}`, task_digest: digest(marker), domains: [domain] });
  await session.started();
  await session.record({ phase: "provider_invocation_finished", status: "running", provider: `provider-${marker}`, model: `model-${marker}`, latency_ms: marker === "b" ? 11 : 10, input_tokens: 20, output_tokens: 8, tool_count: 1 });
  await session.complete({ status: "completed" });
  return analyzeAutonomousRunTrace(await store.snapshot());
}

class TransactionalTextStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expected, value) {
    const current = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (current !== expected) return false;
    this.value = value;
    return true;
  }
}

test("ledger aggregates all domains and returns explicit dedupe/conflict states", async () => {
  const first = await report("a", "coding");
  const second = await report("b", "science");
  const ledger = new AutonomousRunAnalyticsLedger({ clock: () => 123.456 });
  assert.equal(ledger.ingest(first, { ingestedAt: 1000 }).status, "accepted");
  assert.equal(ledger.ingest(first, { ingestedAt: 9999 }).status, "duplicate");

  const conflict = structuredClone(first);
  conflict.policy_digest = digest("f");
  const { report_digest: _ignored, ...conflictBody } = conflict;
  conflict.report_digest = digestJsonSync(conflictBody);
  assert.equal(ledger.ingest(conflict).status, "conflict");

  ledger.ingest(second, { ingestedAt: 2000 });
  const summary = ledger.summary();
  assert.equal(summary.report_count, 2);
  assert.equal(summary.source_snapshot_count, 2);
  assert.equal(summary.accepted_report_count, 2);
  assert.equal(summary.event_count, 6);
  assert.equal(summary.run_count, 2);
  assert.equal(summary.provider_invocations, 2);
  assert.equal(summary.latency_mean_ms, 10.5);
  assert.equal(summary.latency_p50_ms, null);
  assert.equal(summary.latency_p95_ms, null);
  assert.equal(summary.latency_quantile_posture, "not_aggregated_from_report_quantiles");
  assert.equal(summary.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(new Set(summary.domains.filter((row) => row.observed).map((row) => row.identity)), new Set(["coding", "science"]));
  assert.deepEqual(new Set(summary.providers.map((row) => row.identity)), new Set(["provider-a", "provider-b"]));
  assert.deepEqual(new Set(summary.models.map((row) => row.identity)), new Set(["provider-a/model-a", "provider-b/model-b"]));
  assert.equal(summary.domains.find((row) => row.identity === "evaluation").measurement_state, "unmeasured");
  assert.equal(Object.keys(summary).includes("prompt"), false);
  assert.equal(summary.secret_material, "never_returned");
});

test("ledger snapshot is bounded, digest verified, and restart-safe", async () => {
  const ledger = new AutonomousRunAnalyticsLedger({ policy: { max_reports: 1 } });
  const first = await report("a", "coding");
  const second = await report("b", "science");
  ledger.ingest(first, { ingestedAt: 10 });
  ledger.ingest(second, { ingestedAt: 20 });
  assert.equal(ledger.entries().length, 1);
  assert.equal(ledger.summary().evicted_report_count, 1);
  assert.equal(ledger.history({ limit: 1 })[0].report.source_snapshot_digest, second.source_snapshot_digest);

  const snapshot = ledger.snapshot();
  assert.equal(validateAutonomousRunAnalyticsLedgerSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);
  const restored = new AutonomousRunAnalyticsLedger({ policy: { max_reports: 1 } });
  restored.restore(snapshot);
  assert.deepEqual(restored.summary(), ledger.summary());

  const tampered = structuredClone(snapshot);
  tampered.entries[0].report.event_count += 1;
  assert.throws(() => restored.restore(tampered), /digest|invalid|reconcile|malformed|phase/);
  assert.throws(() => new AutonomousRunAnalyticsLedger({ policy: { max_reports: 0 } }), /positive|bounded/);
});

test("ledger persistence uses canonical JSON and compare-and-swap fencing", async () => {
  const store = new TransactionalTextStore();
  const persistence = new TransactionalJsonAutonomousRunAnalyticsLedgerPersistence(store);
  const ledger = new AutonomousRunAnalyticsLedger();
  ledger.ingest(await report("a", "coding"), { ingestedAt: 10 });
  const coordinator = new AutonomousRunAnalyticsLedgerPersistenceCoordinator(ledger, persistence);
  const saved = await coordinator.flush();
  assert.equal(saved.snapshot_digest, JSON.parse(store.value).snapshot_digest);

  const restarted = new AutonomousRunAnalyticsLedger();
  const restartedCoordinator = new AutonomousRunAnalyticsLedgerPersistenceCoordinator(restarted, persistence);
  assert.ok(await restartedCoordinator.restore());
  assert.equal(restarted.summary().report_count, 1);

  const stale = new AutonomousRunAnalyticsLedgerPersistenceCoordinator(new AutonomousRunAnalyticsLedger(), persistence);
  assert.ok(await stale.restore());
  restarted.ingest(await report("b", "science"), { ingestedAt: 20 });
  await restartedCoordinator.flush();
  await assert.rejects(() => stale.flush(), /compare-and-swap/);

  const plain = new JsonAutonomousRunAnalyticsLedgerPersistence(store);
  assert.ok(await plain.read());
  const agent = Object.create(AutonomousAgent.prototype);
  assert.equal(agent.createRunAnalyticsLedger({ policy: { max_reports: 1 } }).policy.max_reports, 1);
});
