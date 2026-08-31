import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousMemoryConsolidationScheduler,
  AutonomousMemoryConsolidationSchedulerError,
  AutonomousMemoryConsolidationSchedulerPersistenceCoordinator,
  AutonomousMemoryConsolidator,
  JsonAutonomousMemoryConsolidationSchedulerPersistence,
  TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence,
  validateAutonomousMemoryConsolidationSchedulerSnapshot,
} from "../dist/index.js";

const digest = (value) => {
  let hash = 0x811c9dc5;
  for (const character of value) hash = Math.imul(hash ^ character.charCodeAt(0), 0x010001f3);
  return `${Math.abs(hash).toString(16).padStart(8, "0")}${"0".repeat(56)}`;
};

function observation(episodeId, domain, reward = 1) {
  return {
    episode_id: episodeId, lesson_id: "lesson-scheduler", concept_id: "scheduler-lesson", variant_id: "v1", domain,
    capability: "evidence_review", risk_class: "read_only", evaluator_id: `evaluator-${episodeId}`, evaluator_version: "v1",
    reward, passed: reward > 0, evidence_digest: digest(`evidence-${episodeId}`), lesson_digest: digest("lesson-scheduler-v1"),
    decision_digest: digest(`decision-${episodeId}`), observed_at: 100, transferable: true,
  };
}

class CasStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expected, value) {
    const observed = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (observed !== expected) return false;
    this.value = value;
    return true;
  }
}

test("priority worker loop and all-domain coverage are deterministic", () => {
  const scheduler = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 }), { defaultMaxAttempts: 2, leaseSeconds: 10 });
  const observations = AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => observation(`domain-${index}`, domain));
  const first = scheduler.submit("all-domains", observations, { priority: 0.4, submittedAt: 10 });
  const replay = scheduler.submit("all-domains", [...observations], { priority: 0.4, submittedAt: 10 });
  assert.equal(first.job_digest, replay.job_digest);
  scheduler.submit("high-priority", [observation("high", "coding")], { priority: 0.9, submittedAt: 99 });

  const claim = scheduler.claimNext("worker-a", { now: 100 });
  assert.equal(claim.job_id, "high-priority");
  scheduler.complete(claim.job_id, claim.worker_id, claim.lease_digest, digest("report-high"), { now: 100 });
  const result = scheduler.runNext("worker-a", { now: 100 });
  assert.equal(result.status, "completed");
  assert.equal(result.observation_count, observations.length);

  const snapshot = scheduler.snapshot();
  assert.deepEqual(snapshot.coverage.map((row) => row.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.deepEqual(snapshot.coverage.map((row) => row.observation_count), AUTONOMOUS_DOMAIN_NAMES.map((domain) => domain === "coding" ? 2 : 1));
  assert.equal(JSON.stringify(snapshot).toLowerCase().includes("provider_output"), false);
  assert.equal(JSON.stringify(snapshot).toLowerCase().includes("api_key"), false);
});

test("expired leases fence old workers and failures quarantine after bounded retries", () => {
  const scheduler = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { defaultMaxAttempts: 2, leaseSeconds: 5 });
  scheduler.submit("lease", [observation("lease", "operations")], { submittedAt: 1 });
  const first = scheduler.claimNext("worker-a", { now: 10 });
  const second = scheduler.claimNext("worker-b", { now: 16 });
  assert.equal(second.attempt, 2);
  assert.throws(() => scheduler.complete(first.job_id, first.worker_id, first.lease_digest, digest("stale"), { now: 16 }), AutonomousMemoryConsolidationSchedulerError);
  scheduler.submit("contradiction", [observation("same", "evaluation"), observation("same", "evaluation", 0)], { maxAttempts: 2, submittedAt: 20 });
  const firstFailure = scheduler.runNext("worker-c", { now: 20 });
  assert.equal(firstFailure.status, "queued");
  const failure = scheduler.runNext("worker-c", { now: 20 });
  assert.equal(failure.status, "quarantined");
  assert.equal(failure.error_class, "memory_consolidation_failure");
  assert.equal(JSON.stringify(scheduler.snapshot()).toLowerCase().includes("contradictory"), false);
});

test("snapshot rehydration, tamper fencing, and CAS persistence are enforced", () => {
  const source = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { leaseSeconds: 10 });
  source.submit("persist", [observation("persist", "science")], { submittedAt: 100 });
  const store = new CasStore();
  const coordinator = new AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(source, new TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence(store));
  const snapshot = coordinator.flush();
  assert.equal(validateAutonomousMemoryConsolidationSchedulerSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);

  const restored = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { leaseSeconds: 10 });
  const restoredCoordinator = new AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(restored, new JsonAutonomousMemoryConsolidationSchedulerPersistence(store));
  assert.equal(restoredCoordinator.restore().snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restored.get("persist").job_digest, snapshot.jobs[0].job_digest);

  const tampered = structuredClone(snapshot);
  tampered.jobs[0].observations[0].reward = 0.25;
  assert.throws(() => validateAutonomousMemoryConsolidationSchedulerSnapshot(tampered), AutonomousMemoryConsolidationSchedulerError);
  const extra = structuredClone(snapshot);
  extra.unexpected = true;
  assert.throws(() => validateAutonomousMemoryConsolidationSchedulerSnapshot(extra), AutonomousMemoryConsolidationSchedulerError);
  source.submit("second", [observation("second", "science")], { submittedAt: 101 });
  const competing = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { leaseSeconds: 10 });
  const competingCoordinator = new AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(competing, new JsonAutonomousMemoryConsolidationSchedulerPersistence(store));
  competingCoordinator.restore();
  competing.submit("competing", [observation("competing", "science")], { submittedAt: 102 });
  competingCoordinator.flush();
  assert.throws(() => coordinator.flush(), AutonomousMemoryConsolidationSchedulerError);
});
