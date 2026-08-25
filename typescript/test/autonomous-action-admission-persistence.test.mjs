import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  InMemoryAutonomousActionAdmissionLedger,
  JsonAutonomousActionAdmissionSnapshotPersistence,
  LLMRuntime,
  TransactionalJsonAutonomousActionAdmissionSnapshotPersistence,
  AutonomousActionAdmissionPersistenceCoordinator,
  admitAutonomousActionPlan,
  createAutonomousActionAdmissionRecord,
  validateAutonomousActionAdmissionRecord,
} from "../dist/index.js";

const tasks = {
  coding: "debug a bounded repository change",
  browser: "compare web sources and citation gaps",
  data: "profile a dataset schema and missingness",
  science: "design a reproducible experiment and uncertainty report",
  biomedical: "review biomedical evidence with safety boundaries",
  neuroscience: "analyze neural signal preprocessing and limitations",
  operations: "prepare a reversible incident rollback runbook",
  enterprise: "map governance ownership and approvals",
  multi_agent: "delegate specialists and reconcile evidence",
  multimodal: "align document image and audio observations",
  cross_domain: "synthesize evidence across several disciplines",
  evaluation: "replay a benchmark and analyze evaluator failures",
};

function makeBrain() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  const agent = new AutonomousAgent(runtime);
  return new AutonomousBrainFacade({ agent });
}

function allApprovals(plan) {
  return Object.fromEntries(plan.required_approvals.map((approval) => [approval, true]));
}

class MemoryTextStore {
  constructor() { this.value = null; }
  read() { return this.value; }
  write(value) { this.value = value; }
}

class TransactionalMemoryTextStore extends MemoryTextStore {
  writeIfUnchanged(expected, value) {
    if (expected === null && this.value !== null) return false;
    if (expected !== null) {
      if (this.value === null) return false;
      const current = JSON.parse(this.value);
      if (current.snapshot_digest !== expected) return false;
    }
    this.value = value;
    return true;
  }
}

test("action admission ledger covers every domain and preserves a metadata-only review process", async () => {
  const brain = makeBrain();
  const ledger = new InMemoryAutonomousActionAdmissionLedger({ maxRecords: 32 });
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const request = { task: tasks[domain], domain, capability: "bounded_task", allow_cross_domain: false };
    const plan = await brain.actionPlan(request);
    const admission = admitAutonomousActionPlan(plan);
    const record = ledger.submit(plan, admission, { actionId: `domain-${domain}` });
    assert.equal(record.status === "pending_review" || record.status === "blocked", true, domain);
    assert.equal(JSON.stringify(record).includes(tasks[domain]), false, domain);
    assert.equal(record.plan.plan_digest, record.admission.plan_digest, domain);
  }
  const crossPlan = await brain.actionPlan({ task: "coordinate coding and biomedical evidence", hints: ["coding", "biomedical"], allow_cross_domain: true });
  const crossAdmission = admitAutonomousActionPlan(crossPlan);
  const cross = ledger.submit(crossPlan, crossAdmission, { actionId: "cross-domain-review" });
  assert.equal(cross.plan.cross_domain, true);
  assert.ok(cross.plan.selected_domains.length >= 2);
  assert.equal(ledger.list().length, AUTONOMOUS_DOMAIN_NAMES.length + 1);
});

test("action admission ledger revisions require reviewer identity, predecessor digest, and exact plan gates", async () => {
  const brain = makeBrain();
  const request = { task: tasks.coding, domain: "coding", capability: "bounded_task", allow_cross_domain: false };
  const plan = await brain.actionPlan(request);
  const pending = createAutonomousActionAdmissionRecord(plan, admitAutonomousActionPlan(plan), { actionId: "review-transition" });
  const ledger = new InMemoryAutonomousActionAdmissionLedger();
  ledger.put(pending);
  const reviewed = ledger.review("review-transition", {
    approvals: allApprovals(plan),
    reviewed: true,
    reviewerDigest: "a".repeat(64),
    reason: "operator reviewed every explicit gate",
    expectedRecordDigest: pending.record_digest,
  });
  assert.equal(reviewed.revision, 2);
  assert.equal(reviewed.status, "admitted");
  assert.equal(reviewed.decision, "reviewed");
  assert.equal(reviewed.previous_record_digest, pending.record_digest);
  assert.throws(() => ledger.review("review-transition", {
    approvals: allApprovals(plan),
    reviewed: true,
    reviewerDigest: "b".repeat(64),
    expectedRecordDigest: pending.record_digest,
  }), /expectedRecordDigest/);
  const tampered = { ...reviewed, status: "pending_review" };
  assert.throws(() => validateAutonomousActionAdmissionRecord(tampered), /status|digest/);
  assert.equal(JSON.stringify(reviewed).includes(tasks.coding), false);
});

test("action admission ledger snapshot persistence is canonical, restart-safe, CAS-fenced, and tamper-evident", async () => {
  const brain = makeBrain();
  const plan = await brain.actionPlan({ task: tasks.science, domain: "science", capability: "bounded_task", allow_cross_domain: false });
  const admission = admitAutonomousActionPlan(plan);
  const ledger = new InMemoryAutonomousActionAdmissionLedger();
  ledger.submit(plan, admission, { actionId: "persisted-science" });
  const textStore = new TransactionalMemoryTextStore();
  const persistence = new TransactionalJsonAutonomousActionAdmissionSnapshotPersistence(textStore);
  const coordinator = new AutonomousActionAdmissionPersistenceCoordinator(ledger, persistence);
  assert.equal(await coordinator.restore(), null);
  const snapshot = await coordinator.flush();
  assert.equal(snapshot.generation, 1);
  assert.equal(snapshot.records.length, 1);
  assert.equal(JSON.stringify(snapshot).includes(tasks.science), false);

  const restoredLedger = new InMemoryAutonomousActionAdmissionLedger();
  const restoredCoordinator = new AutonomousActionAdmissionPersistenceCoordinator(restoredLedger, persistence);
  const restored = await restoredCoordinator.restore();
  assert.equal(restored.snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restoredLedger.get("persisted-science").record_digest, snapshot.records[0].record_digest);
  const staleLedger = new InMemoryAutonomousActionAdmissionLedger();
  const staleCoordinator = new AutonomousActionAdmissionPersistenceCoordinator(staleLedger, persistence);
  await staleCoordinator.restore();
  await coordinator.flush();
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);

  const raw = JSON.parse(textStore.value);
  raw.records[0].revision = 999;
  const tamperedStore = new MemoryTextStore();
  tamperedStore.value = JSON.stringify(raw);
  const tamperedPersistence = new JsonAutonomousActionAdmissionSnapshotPersistence(tamperedStore);
  await assert.rejects(() => tamperedPersistence.read(), /digest|metadata|record/);
});
