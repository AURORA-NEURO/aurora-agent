import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousEvaluatorCalibrationHarness,
  AutonomousValueEvaluatorRegistry,
  assertAutonomousEvaluatorCalibrationReady,
  autonomousEvaluatorCalibrationAdmission,
  builtinAutonomousValueEvaluatorProfiles,
  validateAutonomousEvaluatorCalibrationReport,
} from "../dist/index.js";

function casesForDomains(domains = AUTONOMOUS_DOMAIN_NAMES) {
  const profiles = new Map(builtinAutonomousValueEvaluatorProfiles().map((profile) => [profile.domain, profile]));
  return domains.flatMap((domain) => {
    const profile = profiles.get(domain);
    const makeEvidence = (value) => ({
      schema: "bioprism-brain-domain-evaluator/0.1",
      domain,
      capability: "calibration-fixture",
      risk_class: "read_only",
      signals: Object.fromEntries(profile.required_signals.map((signal) => [signal, value])),
      references: [],
      limitations: [],
      retention: "value_only_digests_and_signal_scores",
    });
    return [
      { case_id: `${domain}-calibration-positive`, domain, evidence: makeEvidence(1), label: 1, split: "calibration" },
      { case_id: `${domain}-calibration-negative`, domain, evidence: makeEvidence(0), label: 0, split: "calibration" },
      { case_id: `${domain}-calibration-reference-abstained`, domain, evidence: makeEvidence(1), label: null, split: "calibration" },
      { case_id: `${domain}-holdout-positive`, domain, evidence: makeEvidence(1), label: 1, split: "holdout" },
      { case_id: `${domain}-holdout-negative`, domain, evidence: makeEvidence(0), label: 0, split: "holdout" },
    ];
  });
}

test("evaluator calibration measures deterministic calibration and holdout reliability across every domain", () => {
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const harness = new AutonomousEvaluatorCalibrationHarness(registry);
  const cases = casesForDomains();
  const report = harness.run({
    cases,
    bins: 5,
    minCalibrationCasesPerDomain: 2,
    minHoldoutCasesPerDomain: 2,
    maxExpectedCalibrationError: 0.01,
    maxBrierScore: 0.01,
  });
  assert.equal(report.status, "ready");
  assert.equal(report.gate.decision, "admit_learning");
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.domains.every((row) => row.status === "ready"), true);
  assert.equal(report.domains.every((row) => row.holdout.brier_score === 0), true);
  assert.equal(report.domains.every((row) => row.holdout.expected_calibration_error === 0), true);
  assert.equal(report.domains.every((row) => row.calibration.unscored_count === 1), true);
  assert.equal(assertAutonomousEvaluatorCalibrationReady(report, "science").decision, "admit_learning");
  assert.equal(autonomousEvaluatorCalibrationAdmission(report, "coding").decision, "admit_learning");
  assert.deepEqual(validateAutonomousEvaluatorCalibrationReport(report), report);

  const replay = harness.replay(report, { cases });
  assert.equal(replay.matches, true);
  assert.equal(replay.evaluator_catalogue_match, true);
  assert.equal(replay.case_set_match, true);

  const automaticSplitCases = casesForDomains(["coding"]).map(({ split: _split, ...value }) => value);
  const automaticSplitReport = harness.run({ cases: automaticSplitCases, domains: ["coding"], seed: "stable-calibration-seed", holdoutFraction: 0.5, minCalibrationCasesPerDomain: 1, minHoldoutCasesPerDomain: 1 });
  assert.equal(automaticSplitReport.seed, "stable-calibration-seed");
  assert.equal(harness.replay(automaticSplitReport, { cases: automaticSplitCases }).matches, true);
});

test("calibration holds learning for missing coverage, weak holdouts, and changed labels", () => {
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const harness = new AutonomousEvaluatorCalibrationHarness(registry);
  const sparse = casesForDomains(["coding"]);
  const missing = harness.run({ cases: sparse, domains: AUTONOMOUS_DOMAIN_NAMES, minCalibrationCasesPerDomain: 2, minHoldoutCasesPerDomain: 2 });
  assert.equal(missing.status, "insufficient_coverage");
  assert.equal(autonomousEvaluatorCalibrationAdmission(missing, "science").decision, "hold_learning");
  assert.throws(() => assertAutonomousEvaluatorCalibrationReady(missing, "science"), /calibration holds learning/);

  const weak = sparse.map((item) => item.case_id === "coding-holdout-negative" ? { ...item, evidence: { ...item.evidence, signals: { schema_valid: 1, tests_passed: 1, evidence_complete: 1 } }, label: 0 } : item);
  const weakReport = harness.run({ cases: weak, domains: ["coding"], minCalibrationCasesPerDomain: 2, minHoldoutCasesPerDomain: 2, maxExpectedCalibrationError: 0.01, maxBrierScore: 0.01 });
  assert.equal(weakReport.status, "miscalibrated");
  assert.equal(autonomousEvaluatorCalibrationAdmission(weakReport, "coding").decision, "hold_learning");
  const changed = [...sparse];
  changed[0] = { ...changed[0], label: 0 };
  const replay = harness.replay(harness.run({ cases: sparse, domains: ["coding"] }), { cases: changed });
  assert.equal(replay.matches, false);
  assert.equal(replay.case_set_match, false);
});

test("calibration rejects secret-shaped evidence and duplicate case identities", () => {
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const harness = new AutonomousEvaluatorCalibrationHarness(registry);
  const cases = casesForDomains(["coding"]);
  assert.throws(() => harness.run({ cases: [{ ...cases[0], case_id: "duplicate" }, { ...cases[1], case_id: "duplicate" }] }), /case_id values must be unique/);
  assert.throws(() => harness.run({ cases: [{ ...cases[0], evidence: { ...cases[0].evidence, api_key: "never" } }] }), /secret-shaped/);
});
