import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousEvidenceRuntime,
  InMemoryAutonomousEvidenceRuntimeJournal,
  builtinAutonomousDomainProfiles,
  buildAutonomousEvidencePlan,
  digestJson,
} from "../dist/index.js";

function requestFor(requirement, index = 0) {
  return {
    requirement_id: requirement.requirement_id,
    source_id: `fixture-source-${index}`,
    request_id: `fixture-request-${index}`,
    metadata: { fixture: true, domain: requirement.domain },
  };
}

function fixtureAdapters(calls = []) {
  return {
    acquirer: {
      async acquire(context) {
        calls.push(context.requirement.requirement_id);
        return { fixture: "metadata-only-test-value", requirement: context.requirement.requirement_id };
      },
    },
    projector: {
      project(_value, context) {
        return [{ label: context.requirement.label, kind: "fact", status: "observed", confidence: 0.95 }];
      },
    },
    evaluator: {
      evaluator_id: "fixture-evaluator",
      evaluator_version: "2026.08",
      evaluate({ requirement }) {
        return { evaluator_id: "fixture-evaluator", evaluator_version: "2026.08", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest };
      },
    },
  };
}

test("evidence runtime acquires and evaluates every built-in domain without retaining raw values in JSON", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const plan = await buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
  const calls = [];
  const runtime = new AutonomousEvidenceRuntime({ plan });
  const result = await runtime.execute(plan.requirements.map(requestFor), fixtureAdapters(calls));

  assert.equal(new Set(plan.domains).size, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(plan.requirements.length > 40);
  assert.equal(calls.length, plan.requirements.length);
  assert.equal(result.json.status, "completed");
  assert.equal(result.json.missing_requirement_ids.length, 0);
  assert.equal(result.json.pending_evaluation_requirement_ids.length, 0);
  assert.equal(result.json.receipts.length, plan.requirements.length);
  assert.equal(result.json.assessments.length, plan.requirements.length);
  assert.equal(Object.hasOwn(result.toJSON(), "values"), false);
  assert.equal(result.json.retention, "metadata_only;raw_values_caller_owned");
  assert.ok(result.values[Object.keys(result.values)[0]]);
});

test("evidence runtime makes evaluation and acquisition failures explicit", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const completeBaseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: completeBaseline.requirements.slice(1).map((item) => item.requirement_id) });
  const request = requestFor(plan.requirements[0]);
  const runtime = new AutonomousEvidenceRuntime({ plan });
  const adapters = fixtureAdapters();
  const noEvaluator = await runtime.execute([request], { acquirer: adapters.acquirer, projector: adapters.projector });
  assert.equal(noEvaluator.json.status, "awaiting_evaluation");
  assert.equal(noEvaluator.json.receipts[0].evaluator_status, "not_evaluated");

  const failed = await new AutonomousEvidenceRuntime({ plan }).execute([request], {
    acquirer: { async acquire() { throw new Error("fixture acquisition failure"); } },
  });
  assert.equal(failed.json.status, "failed");
  assert.equal(failed.json.receipts[0].status, "failed");

  await assert.rejects(
    () => new AutonomousEvidenceRuntime({ plan }).execute([{ ...request, metadata: { api_key: "must never enter evidence metadata" } }], fixtureAdapters()),
    ArgumentError,
  );
});

test("evidence runtime replay requires value reconciliation after journal rehydration", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const completeBaseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: completeBaseline.requirements.slice(1).map((item) => item.requirement_id) });
  const request = requestFor(plan.requirements[0]);
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const calls = [];
  const adapters = fixtureAdapters(calls);
  const first = await new AutonomousEvidenceRuntime({ plan, journal }).execute([request], adapters);
  assert.equal(first.json.receipts[0].replay, "fresh");
  const snapshot = await journal.snapshot(plan.plan_digest);
  assert.equal(snapshot.snapshot_generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  assert.deepEqual(await journal.snapshot(plan.plan_digest), snapshot);
  const forged = structuredClone(snapshot);
  forged.snapshot_generation = 2;
  forged.previous_snapshot_digest = null;
  const { snapshot_digest: _forgedDigest, ...forgedBody } = forged;
  forged.snapshot_digest = await digestJson(forgedBody);
  const forgedJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await assert.rejects(() => forgedJournal.restore(forged, plan.plan_digest), /generation and previous_snapshot_digest/);

  const legacy = structuredClone(snapshot);
  delete legacy.snapshot_generation;
  delete legacy.previous_snapshot_digest;
  legacy.schema = "bioprism-typescript-autonomous-evidence-runtime-snapshot/0.1";
  const { snapshot_digest: _legacyDigest, ...legacyBody } = legacy;
  legacy.snapshot_digest = await digestJson(legacyBody);
  const legacyJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await legacyJournal.restore(legacy, plan.plan_digest);
  const upgraded = await legacyJournal.snapshot(plan.plan_digest);
  assert.equal(upgraded.snapshot_generation, 1);
  assert.equal(upgraded.previous_snapshot_digest, null);
  assert.notEqual(upgraded.snapshot_digest, legacy.snapshot_digest);
  const restoredJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await restoredJournal.restore(snapshot, plan.plan_digest);
  const restored = new AutonomousEvidenceRuntime({ plan, journal: restoredJournal });
  await restored.rehydrate();

  const missingValue = await restored.execute([request], { acquirer: { async acquire() { throw new Error("must not reacquire"); } } });
  assert.equal(missingValue.json.status, "reconciliation_required");
  assert.equal(missingValue.json.receipts[0].status, "reconciliation_required");
  assert.equal(calls.length, 1);

  const replayed = await restored.execute([request], {
    ...adapters,
    rehydrateValue: () => ({ fixture: "metadata-only-test-value", requirement: plan.requirements[0].requirement_id }),
  });
  assert.equal(replayed.json.receipts[0].replay, "replayed");
  assert.notEqual(replayed.json.receipts[0].status, "reconciliation_required");
  assert.equal(calls.length, 1);
});

test("evidence runtime persists a pending evaluator revision and accepts it after restart", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const baseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: baseline.requirements.slice(1).map((item) => item.requirement_id) });
  const request = requestFor(plan.requirements[0]);
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const calls = [];
  const value = { fixture: "reconcile-me", requirement: plan.requirements[0].requirement_id };
  const projector = { project: (_value, context) => [{ label: context.requirement.label, status: "observed" }] };
  const pendingEvaluator = {
    evaluator_id: "reconciliation-evaluator",
    evaluator_version: "1",
    evaluate: () => ({ evaluator_id: "reconciliation-evaluator", evaluator_version: "1", verdict: "indeterminate", score: 0.5 }),
  };
  const first = await new AutonomousEvidenceRuntime({ plan, journal }).execute([request], {
    acquirer: { acquire: () => { calls.push("acquire"); return value; } },
    projector,
    evaluator: pendingEvaluator,
  });
  assert.equal(first.json.status, "awaiting_evaluation");
  assert.equal(journal.records().length, 1);
  const snapshot = await journal.snapshot(plan.plan_digest);
  const restoredJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await restoredJournal.restore(snapshot, plan.plan_digest);
  const restored = new AutonomousEvidenceRuntime({ plan, journal: restoredJournal });
  await restored.rehydrate();
  const accepted = await restored.execute([request], {
    acquirer: { acquire: () => { throw new Error("pending reconciliation must not reacquire"); } },
    projector,
    evaluator: {
      evaluator_id: "reconciliation-evaluator",
      evaluator_version: "2",
      evaluate: () => ({ evaluator_id: "reconciliation-evaluator", evaluator_version: "2", verdict: "accepted", score: 1 }),
    },
    rehydrateValue: () => value,
    reevaluatePending: true,
  });
  assert.equal(accepted.json.status, "awaiting_evaluation", "the revised requirement is accepted while the remaining plan requirements still need evaluator decisions");
  assert.equal(accepted.json.receipts[0].replay, "replayed");
  assert.equal(accepted.json.receipts[0].evaluator_status, "accepted");
  assert.equal(accepted.json.assessments.length, 1);
  assert.ok(accepted.json.completed_requirement_ids.includes(plan.requirements[0].requirement_id));
  assert.ok(!accepted.json.pending_evaluation_requirement_ids.includes(plan.requirements[0].requirement_id));
  assert.equal(accepted.json.missing_requirement_ids.length, 0);
  assert.equal(restoredJournal.records().length, 2, "assessment reconciliation is an append-only revision");
  const secondSnapshot = await restoredJournal.snapshot(plan.plan_digest);
  assert.equal(secondSnapshot.snapshot_generation, 2);
  assert.equal(secondSnapshot.previous_snapshot_digest, snapshot.snapshot_digest);
  assert.deepEqual(calls, ["acquire"]);
});
