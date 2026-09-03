import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousAuthorizationContext,
  AutonomousAuthorizationError,
  AutonomousAuthorizationGate,
  AutonomousAuthorizationLedger,
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

function authorizationContext(plan, operations = ["evidence_acquisition", "evaluation"]) {
  const ledger = new AutonomousAuthorizationLedger(4, 256);
  const grant = ledger.issue({
    grant_id: "evidence-runtime-grant",
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: "a".repeat(64),
    allowed_domains: [...plan.domains],
    allowed_operations: operations,
    allowed_capabilities: ["analysis"],
    allowed_risk_classes: ["read_only"],
    issued_at: 1000,
    expires_at: 2000,
    max_uses: null,
  });
  return {
    ledger,
    context: new AutonomousAuthorizationContext(
      new AutonomousAuthorizationGate(ledger),
      grant.grant_id,
      grant.tenant_id,
      grant.actor_id,
      grant.session_id,
      grant.authorization_digest,
      [...plan.domains],
      "analysis",
      "read_only",
      "evidence",
      () => 1200,
    ),
  };
}

test("evidence runtime acquires and evaluates every built-in domain without retaining raw values in JSON", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const plan = await buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
  const calls = [];
  const runtime = new AutonomousEvidenceRuntime({ plan });
  const authorization = authorizationContext(plan);
  const result = await runtime.execute(plan.requirements.map(requestFor), { ...fixtureAdapters(calls), authorizationContext: authorization.context });

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
  assert.equal(authorization.ledger.events().filter((event) => event.event_type === "request_allowed").length, plan.requirements.length * 2);
});

test("evidence runtime authorization denies before acquisition and does not record a failure", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const plan = await buildAutonomousEvidencePlan([profile.workflow]);
  const authorization = authorizationContext(plan, ["evaluation"]);
  let calls = 0;
  await assert.rejects(
    () => new AutonomousEvidenceRuntime({ plan }).execute([requestFor(plan.requirements[0])], {
      acquirer: { acquire: () => { calls += 1; return { should_not: "run" }; } },
      authorizationContext: authorization.context,
    }),
    (error) => error instanceof AutonomousAuthorizationError && /operation authorization was refused/.test(error.message),
  );
  assert.equal(calls, 0);
  assert.deepEqual(authorization.ledger.events().map((event) => event.event_type), ["grant_issued"]);
});

test("evidence runtime authorization denies evaluation before the callback while replay skips acquisition authorization", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const plan = await buildAutonomousEvidencePlan([profile.workflow]);
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const adapters = fixtureAdapters();
  const acquisitionAuthorization = authorizationContext(plan, ["evidence_acquisition"]);
  const first = await new AutonomousEvidenceRuntime({ plan, journal }).execute([requestFor(plan.requirements[0])], {
    acquirer: adapters.acquirer,
    projector: adapters.projector,
    authorizationContext: acquisitionAuthorization.context,
  });
  assert.equal(first.json.status, "awaiting_evaluation");
  assert.equal(acquisitionAuthorization.ledger.events().length, 2);

  const evaluationAuthorization = authorizationContext(plan, ["evidence_acquisition"]);
  let evaluatorCalls = 0;
  const restored = new AutonomousEvidenceRuntime({ plan, journal });
  await restored.rehydrate();
  await assert.rejects(
    () => restored.execute([requestFor(plan.requirements[0])], {
      acquirer: { acquire: () => { throw new Error("pending reconciliation must not reacquire"); } },
      evaluator: {
        evaluator_id: "fixture-evaluator",
        evaluator_version: "2026.08",
        evaluate: () => { evaluatorCalls += 1; return { evaluator_id: "fixture-evaluator", evaluator_version: "2026.08", verdict: "accepted", score: 1 }; },
      },
      rehydrateValue: () => ({ fixture: "metadata-only-test-value", requirement: plan.requirements[0].requirement_id }),
      reevaluatePending: true,
      authorizationContext: evaluationAuthorization.context,
    }),
    (error) => error instanceof AutonomousAuthorizationError && /operation authorization was refused/.test(error.message),
  );
  assert.equal(evaluatorCalls, 0);
  assert.deepEqual(evaluationAuthorization.ledger.events().map((event) => event.event_type), ["grant_issued"]);
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

async function replaySingleRequirement(plan, request, journal, value, options = {}) {
  const snapshot = await journal.snapshot(plan.plan_digest);
  const restoredJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await restoredJournal.restore(snapshot, plan.plan_digest);
  const restored = new AutonomousEvidenceRuntime({ plan, journal: restoredJournal });
  await restored.rehydrate();
  return restored.execute([request], {
    acquirer: { acquire: () => { throw new Error("replay must not reacquire evidence"); } },
    rehydrateValue: () => value,
    ...options,
  });
}

test("evidence runtime preserves missing-output state across replay without inventing pending evaluation", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const baseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: baseline.requirements.slice(1).map((item) => item.requirement_id) });
  const request = requestFor(plan.requirements[0]);
  const value = { fixture: "missing-output" };
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const fresh = await new AutonomousEvidenceRuntime({ plan, journal }).execute([request], {
    acquirer: { acquire: () => value },
  });
  const replayed = await replaySingleRequirement(plan, request, journal, value);

  assert.equal(fresh.json.status, "partial");
  assert.equal(replayed.json.status, fresh.json.status);
  assert.deepEqual(replayed.json.pending_evaluation_requirement_ids, fresh.json.pending_evaluation_requirement_ids);
  assert.deepEqual(replayed.json.missing_requirement_ids, fresh.json.missing_requirement_ids);
  assert.deepEqual(replayed.json.pending_evaluation_requirement_ids, []);
});

test("evidence runtime preserves observed unevaluated state across replay", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const baseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: baseline.requirements.slice(1).map((item) => item.requirement_id) });
  const requirement = plan.requirements[0];
  const request = requestFor(requirement);
  const value = { fixture: "observed-unevaluated" };
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const projector = { project: () => [{ label: requirement.label, status: "observed" }] };
  const fresh = await new AutonomousEvidenceRuntime({ plan, journal }).execute([request], {
    acquirer: { acquire: () => value },
    projector,
  });
  const replayed = await replaySingleRequirement(plan, request, journal, value);

  assert.equal(fresh.json.status, "awaiting_evaluation");
  assert.equal(replayed.json.status, fresh.json.status);
  assert.deepEqual(replayed.json.pending_evaluation_requirement_ids, fresh.json.pending_evaluation_requirement_ids);
  assert.deepEqual(replayed.json.missing_requirement_ids, fresh.json.missing_requirement_ids);
});

test("evidence runtime keeps a rejected observed assessment pending across replay", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "science");
  const baseline = await buildAutonomousEvidencePlan([profile.workflow]);
  const plan = await buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: baseline.requirements.slice(1).map((item) => item.requirement_id) });
  const requirement = plan.requirements[0];
  const request = requestFor(requirement);
  const value = { fixture: "rejected" };
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const projector = { project: () => [{ label: requirement.label, status: "observed" }] };
  const evaluator = {
    evaluator_id: "rejecting-evaluator",
    evaluator_version: "1",
    evaluate: () => ({ evaluator_id: "rejecting-evaluator", evaluator_version: "1", verdict: "rejected", score: 0 }),
  };
  const fresh = await new AutonomousEvidenceRuntime({ plan, journal }).execute([request], {
    acquirer: { acquire: () => value },
    projector,
    evaluator,
  });
  const replayed = await replaySingleRequirement(plan, request, journal, value);

  assert.equal(fresh.json.status, "awaiting_evaluation");
  assert.equal(replayed.json.status, fresh.json.status);
  assert.deepEqual(replayed.json.pending_evaluation_requirement_ids, fresh.json.pending_evaluation_requirement_ids);
  assert.equal(replayed.json.receipts[0].evaluator_status, "rejected");
  assert.equal(replayed.json.assessments[0].verdict, "rejected");

  const revisionJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await revisionJournal.restore(await journal.snapshot(plan.plan_digest), plan.plan_digest);
  const revisionRuntime = new AutonomousEvidenceRuntime({ plan, journal: revisionJournal });
  await revisionRuntime.rehydrate();
  const revised = await revisionRuntime.execute([request], {
    acquirer: { acquire: () => { throw new Error("reevaluation must not reacquire evidence"); } },
    rehydrateValue: () => value,
    reevaluatePending: true,
    evaluator: {
      evaluator_id: "accepting-evaluator",
      evaluator_version: "2",
      evaluate: () => ({ evaluator_id: "accepting-evaluator", evaluator_version: "2", verdict: "accepted", score: 1 }),
    },
  });
  assert.equal(revised.json.receipts[0].evaluator_status, "accepted");
  assert.equal(revised.json.assessments[0].verdict, "accepted");
  assert.ok(revised.json.completed_requirement_ids.includes(requirement.requirement_id));
  assert.ok(!revised.json.pending_evaluation_requirement_ids.includes(requirement.requirement_id));
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
