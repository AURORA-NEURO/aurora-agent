import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousEvaluatorCalibrationHarness,
  AutonomousEvaluatorCalibrationRegistry,
  AutonomousValueEvaluatorRegistry,
  InMemoryAutonomousEvaluatorCalibrationStore,
  JsonAutonomousEvaluatorCalibrationStore,
  TransactionalJsonAutonomousEvaluatorCalibrationStore,
  builtinAutonomousValueEvaluatorProfiles,
  canonicalJson,
} from "../dist/index.js";

function allDomainCases() {
  return builtinAutonomousValueEvaluatorProfiles().flatMap((profile) => {
    const evidence = (value) => ({
      schema: "bioprism-brain-domain-evaluator/0.1",
      domain: profile.domain,
      capability: "calibration-store-fixture",
      risk_class: "read_only",
      signals: Object.fromEntries(profile.required_signals.map((signal) => [signal, value])),
      references: [],
      limitations: [],
      retention: "value_only_digests_and_signal_scores",
    });
    return [
      { case_id: `${profile.domain}-store-positive`, domain: profile.domain, evidence: evidence(1), label: 1, split: "calibration" },
      { case_id: `${profile.domain}-store-negative`, domain: profile.domain, evidence: evidence(0), label: 0, split: "calibration" },
      { case_id: `${profile.domain}-store-holdout-positive`, domain: profile.domain, evidence: evidence(1), label: 1, split: "holdout" },
      { case_id: `${profile.domain}-store-holdout-negative`, domain: profile.domain, evidence: evidence(0), label: 0, split: "holdout" },
    ];
  });
}

function report() {
  return new AutonomousEvaluatorCalibrationHarness(AutonomousValueEvaluatorRegistry.withBuiltinProfiles()).run({
    cases: allDomainCases(),
    bins: 5,
    minCalibrationCasesPerDomain: 2,
    minHoldoutCasesPerDomain: 2,
    maxExpectedCalibrationError: 0.01,
    maxBrierScore: 0.01,
  });
}

test("calibration registry imports, queries, and restores one all-domain report without raw cases", async () => {
  const calibration = report();
  const registry = new AutonomousEvaluatorCalibrationRegistry();
  const created = registry.import(calibration);
  assert.equal(created.created, true);
  assert.equal(created.registry_generation, 1);
  const duplicate = registry.import(calibration);
  assert.equal(duplicate.created, false);
  assert.equal(registry.size, 1);
  assert.equal(registry.query({ domain: "science" }).length, 1);
  assert.equal(registry.query({ status: "ready", decision: "admit_learning", limit: 1 }).length, 1);
  assert.equal(JSON.stringify(registry.snapshot()).includes("case_id"), false);
  assert.equal(JSON.stringify(registry.snapshot()).includes("calibration-store-fixture"), false);

  const restored = new AutonomousEvaluatorCalibrationRegistry();
  restored.restore(registry.snapshot());
  assert.deepEqual(restored.get(calibration.report_digest), calibration);
  assert.equal(restored.query({ domain: "coding" })[0].report_digest, calibration.report_digest);
  assert.match(created.registry_digest, /^[0-9a-f]{64}$/);
  assert.match(created.import_digest, /^[0-9a-f]{64}$/);
  assert.equal(new Set(AUTONOMOUS_DOMAIN_NAMES).size, 12);
});

test("calibration registry JSON persistence validates tampering and compare-and-swap fencing", async () => {
  const calibration = report();
  const registry = new AutonomousEvaluatorCalibrationRegistry([calibration]);
  const memory = new InMemoryAutonomousEvaluatorCalibrationStore();
  const snapshot = await registry.flush(memory);
  assert.equal((await memory.read()).snapshot_digest, snapshot.snapshot_digest);
  assert.equal(memory.writeIfUnchanged("0".repeat(64), snapshot), false);
  assert.equal(memory.writeIfUnchanged(snapshot.snapshot_digest, snapshot), true);

  let text = canonicalJson(snapshot);
  const textStore = {
    read: () => text,
    write: (value) => { text = value; },
  };
  const json = new JsonAutonomousEvaluatorCalibrationStore(textStore);
  const resumed = new AutonomousEvaluatorCalibrationRegistry();
  await resumed.restoreFrom(json);
  assert.equal(resumed.get(calibration.report_digest).status, "ready");

  let transactionalText = canonicalJson(snapshot);
  const transactional = new TransactionalJsonAutonomousEvaluatorCalibrationStore({
    read: () => transactionalText,
    write: (value) => { transactionalText = value; },
    writeIfUnchanged: (expected, value) => {
      const current = JSON.parse(transactionalText).snapshot_digest;
      if (current !== expected) return false;
      transactionalText = value;
      return true;
    },
  });
  assert.equal(await transactional.writeIfUnchanged("0".repeat(64), snapshot), false);
  assert.equal(await transactional.writeIfUnchanged(snapshot.snapshot_digest, snapshot), true);

  const tampered = JSON.parse(text);
  tampered.reports[0].status = "miscalibrated";
  text = JSON.stringify(tampered);
  await assert.rejects(() => json.read(), /digest|inconsistent/);
});
