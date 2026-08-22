import assert from "node:assert/strict";
import test from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  InMemoryAutonomousBrainJobScheduler,
  InMemoryAutonomousBrainJobSchedulerPersistence,
  AutonomousBrainJobSchedulerPersistenceCoordinator,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

function submission(index, overrides = {}) {
  return {
    jobId: `job-${index}`,
    idempotencyKey: `idempotency-${index}-secret-task-never-retained`,
    specDigest: digest("abcdef"[index % 6]),
    domain: AUTONOMOUS_DOMAIN_NAMES[index % AUTONOMOUS_DOMAIN_NAMES.length],
    capability: "bounded_task",
    riskClass: "review",
    priority: 10,
    maxAttempts: 3,
    ...overrides,
  };
}

test("scheduler admits every autonomous domain and retains only metadata digests", () => {
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 32, clock: () => 1_000 });
  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) scheduler.submit(submission(index));
  const rows = scheduler.inventory({ limit: 32 });
  assert.equal(rows.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(new Set(rows.map((row) => row.domain)), new Set(AUTONOMOUS_DOMAIN_NAMES));
  const serialized = JSON.stringify(rows);
  assert.equal(serialized.includes("secret-task-never-retained"), false);
  assert.equal(serialized.includes("idempotency-0"), false);
  assert.equal(rows.every((row) => row.retention.startsWith("metadata_only")), true);
  assert.equal(scheduler.verifyIntegrity().verified, true);
});

test("idempotency is digest-bound and conflicting reuse is rejected", () => {
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => 10 });
  const first = scheduler.submit(submission(1));
  const repeated = scheduler.submit(submission(1));
  assert.equal(first.created, true);
  assert.equal(repeated.idempotent, true);
  assert.equal(repeated.job.job_digest, first.job.job_digest);
  assert.throws(() => scheduler.submit(submission(1, { specDigest: digest("e") })), /different specDigest/);
});

test("claimNext uses deterministic priority and bounded aging", () => {
  let time = 0;
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => time });
  scheduler.submit(submission(1, { priority: 1 }), time);
  time = 120_000;
  scheduler.submit(submission(2, { priority: 0 }), time);
  const claimed = scheduler.claimNext("worker-a", 10_000, time);
  assert.equal(claimed?.job_id, "job-1");
  assert.equal(claimed?.state, "leased");
  assert.equal(claimed?.attempts, 1);
  assert.throws(() => scheduler.renew("job-1", "worker-b", 10_000, time), /does not own/);
});

test("leases are fenced, expired preflight work is reclaimed, and retry limits are explicit", () => {
  let time = 0;
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => time });
  scheduler.submit(submission(3, { maxAttempts: 2 }), time);
  const claimed = scheduler.claim("job-3", "worker-a", 100, time);
  scheduler.checkpoint("job-3", "worker-a", { phase: "preflight", checkpointDigest: digest("c"), sideEffectBoundary: "preflight", now: 50 });
  assert.throws(() => scheduler.complete("job-3", "worker-b", digest("r"), 60), /does not own/);
  time = 101;
  const reclaimed = scheduler.claim("job-3", "worker-b", 100, time);
  assert.equal(reclaimed.recovered_after_restart, true);
  assert.equal(reclaimed.attempts, 2);
  const failed = scheduler.fail("job-3", "worker-b", { reason: "transient provider timeout", retryable: true, now: 110 });
  assert.equal(failed.state, "dead_lettered");
  assert.equal(scheduler.claim("job-3", "worker-c", 100, 120).state, "dead_lettered");
  assert.equal(claimed.lease_owner, "worker-a");
});

test("external-boundary failures quarantine until explicit reconciliation", () => {
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => 0 });
  scheduler.submit(submission(4));
  scheduler.claim("job-4", "worker-a", 10_000, 0);
  scheduler.checkpoint("job-4", "worker-a", { phase: "dispatch_started", sideEffectBoundary: "dispatched", now: 1 });
  const uncertain = scheduler.fail("job-4", "worker-a", { reason: "worker lost after dispatch", retryable: true, now: 2 });
  assert.equal(uncertain.state, "reconciliation_required");
  const deferred = scheduler.reconcile("job-4", { outcome: "unknown", evidenceDigest: digest("e"), reason: "provider status unavailable", now: 3 });
  assert.equal(deferred.state, "reconciliation_required");
  const queued = scheduler.reconcile("job-4", { outcome: "not_executed", evidenceDigest: digest("f"), evidenceKind: "idempotency_probe", now: 4 });
  assert.equal(queued.state, "queued");
  assert.equal(queued.side_effect_boundary, "not_started");
});

test("approval pauses and cooperative stage handoff remain explicit", () => {
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => 0 });
  scheduler.submit(submission(8));
  scheduler.claim("job-8", "worker-a", 10_000, 0);
  const waiting = scheduler.checkpoint("job-8", "worker-a", { phase: "approval", waitingForApproval: true, now: 1 });
  assert.equal(waiting.state, "waiting_approval");
  assert.equal(scheduler.resumeApproval("job-8", "operator-1", "approved reviewed scope", 2).state, "queued");
  scheduler.claim("job-8", "worker-b", 10_000, 3);
  assert.equal(scheduler.release("job-8", "worker-b", "stage checkpoint persisted", 4).state, "queued");
  scheduler.claim("job-8", "worker-c", 10_000, 5);
  scheduler.checkpoint("job-8", "worker-c", { phase: "dispatch", sideEffectBoundary: "dispatched", now: 6 });
  assert.throws(() => scheduler.release("job-8", "worker-c", "unsafe", 7), /external dispatch/);
});

test("restart snapshots recover active leases and reject tampering", async () => {
  let time = 0;
  const persistence = new InMemoryAutonomousBrainJobSchedulerPersistence();
  const first = new InMemoryAutonomousBrainJobScheduler({ clock: () => time });
  const firstController = new AutonomousBrainJobSchedulerPersistenceCoordinator(first, persistence);
  first.submit(submission(5), time);
  first.claim("job-5", "worker-a", 50, time);
  await firstController.flush();
  const tampered = persistence.read();
  tampered.jobs[0].capability = "tampered";
  const restarted = new InMemoryAutonomousBrainJobScheduler({ clock: () => time });
  assert.throws(() => restarted.restore(tampered), /snapshot digest/);
  const persisted = await persistence.read();
  time = 51;
  const restartedController = new AutonomousBrainJobSchedulerPersistenceCoordinator(restarted, persistence);
  await restartedController.restore();
  const reclaimed = restarted.claimNext("worker-b", 100, time);
  assert.equal(reclaimed?.job_id, "job-5");
  assert.equal(reclaimed?.recovered_after_restart, true);
  assert.equal(persisted.jobs[0].lease_owner, "worker-a");
  assert.equal(JSON.stringify(restarted.snapshot()).includes("secret-task-never-retained"), false);
});

test("shared snapshot persistence rejects a stale scheduler writer", async () => {
  const persistence = new InMemoryAutonomousBrainJobSchedulerPersistence();
  const seed = new InMemoryAutonomousBrainJobScheduler({ clock: () => 2_000 });
  const seedController = new AutonomousBrainJobSchedulerPersistenceCoordinator(seed, persistence);
  seed.submit(submission(9), 2_000);
  await seedController.flush();

  const left = new InMemoryAutonomousBrainJobScheduler({ clock: () => 2_000 });
  const right = new InMemoryAutonomousBrainJobScheduler({ clock: () => 2_000 });
  const leftController = new AutonomousBrainJobSchedulerPersistenceCoordinator(left, persistence);
  const rightController = new AutonomousBrainJobSchedulerPersistenceCoordinator(right, persistence);
  await leftController.restore();
  await rightController.restore();
  left.claim("job-9", "worker-left", 10_000, 2_001);
  await leftController.flush();
  right.claim("job-9", "worker-right", 10_000, 2_001);
  await assert.rejects(rightController.flush(), /compare-and-swap conflict/);
  assert.equal(persistence.read().jobs[0].lease_owner, "worker-left");
  assert.equal(persistence.read().jobs[0].state, "leased");
});

test("one coordinator serializes overlapping snapshot flushes", async () => {
  const persistence = new InMemoryAutonomousBrainJobSchedulerPersistence();
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ clock: () => 2_500 });
  const controller = new AutonomousBrainJobSchedulerPersistenceCoordinator(scheduler, persistence);
  scheduler.submit(submission(10), 2_500);
  const snapshots = await Promise.all([controller.flush(), controller.flush(), controller.flush()]);
  assert.equal(snapshots.length, 3);
  assert.equal(new Set(snapshots.map((snapshot) => snapshot.snapshot_digest)).size, 1);
  assert.equal(persistence.read().snapshot_digest, snapshots.at(-1).snapshot_digest);
});

test("capacity, cancellation, and checkpoint bounds fail closed", () => {
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 1, clock: () => 0 });
  scheduler.submit(submission(6));
  assert.throws(() => scheduler.submit(submission(7)), /full/);
  const cancelled = scheduler.cancel("job-6", "operator requested stop");
  assert.equal(cancelled.state, "cancelled");
  assert.throws(() => scheduler.checkpoint("job-6", "worker-a", { phase: "x" }), /unknown|does not own/);
});
