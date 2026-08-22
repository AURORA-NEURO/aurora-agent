import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousEvidenceRuntime,
  InMemoryAutonomousEvidenceRuntimeJournal,
  builtinAutonomousDomainProfiles,
  buildAutonomousEvidencePlan,
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
