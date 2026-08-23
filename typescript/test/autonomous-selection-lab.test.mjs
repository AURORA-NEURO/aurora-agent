import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousOnlineLearner,
  evaluateAutonomousSelectionPolicy,
  validateAutonomousSelectionLabReport,
} from "../dist/index.js";

function request(domain, { disabled = false } = {}) {
  return {
    task: `private evaluator task for ${domain}; this text must never be retained`,
    domain,
    capability: "default",
    risk_class: "low",
    required_capabilities: [],
    estimated_input_tokens: 100,
    requested_output_tokens: 100,
    candidates: [
      {
        provider: "lab",
        model: `${domain}-quality`,
        capabilities: ["structured_output"],
        context_window_tokens: 8_192,
        max_output_tokens: 2_048,
        quality: 0.9,
        latency_ms: 20,
        cost_per_million_tokens: 4,
        reliability: 0.92,
        enabled: !disabled,
      },
      {
        provider: "lab",
        model: `${domain}-cheap`,
        capabilities: ["structured_output"],
        context_window_tokens: 8_192,
        max_output_tokens: 2_048,
        quality: 0.7,
        latency_ms: 10,
        cost_per_million_tokens: 1,
        reliability: 0.85,
        enabled: true,
      },
    ],
    provider_health: {
      lab: {
        provider: "lab",
        circuit: "closed",
        consecutive_failures: 0,
        attempts: 0,
        successes: 0,
        failures: 0,
        success_rate: 0,
        mean_latency_ms: null,
        last_latency_ms: null,
        last_model: null,
        last_status_code: null,
        credential_posture: "caller_supplied_in_memory_handle",
        credential_required: false,
        credential_ready: true,
        structured_output_mode: "json_object",
      },
    },
    model_health: {},
  };
}

function labCases() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({
    case_id: `${domain}-selection-case`,
    domain,
    request: request(domain),
    // Deliberately make the lower-ranked arm the counterfactual oracle so regret is observable.
    rewards: {
      [`lab/${domain}-quality`]: 0.25,
      [`lab/${domain}-cheap`]: 0.95,
    },
  }));
}

test("selection lab evaluates every autonomous domain without retaining task text", async () => {
  const cases = labCases();
  const report = await evaluateAutonomousSelectionPolicy(cases, { requireAllDomains: true });

  assert.equal(report.status, "completed");
  assert.equal(report.case_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.evaluated_case_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(report.missing_domains, []);
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.domains.every((row) => row.case_count === 1 && row.evaluated_coverage === 1), true);
  assert.equal(report.oracle_agreement_count, 0);
  assert.equal(report.total_regret, 8.4);
  assert.equal(JSON.stringify(report).includes("private evaluator task"), false);
  assert.equal(JSON.stringify(report).includes("selection-case"), true);

  const validated = validateAutonomousSelectionLabReport(report);
  assert.deepEqual(validated, report);
  const repeated = await evaluateAutonomousSelectionPolicy(cases, { requireAllDomains: true });
  assert.equal(repeated.report_digest, report.report_digest);
});

test("selection lab makes missing coverage and safe abstention explicit", async () => {
  const oneCase = [labCases()[0]];
  const incomplete = await evaluateAutonomousSelectionPolicy(oneCase, { requireAllDomains: true });
  assert.equal(incomplete.status, "insufficient_coverage");
  assert.equal(incomplete.missing_domains.length, AUTONOMOUS_DOMAIN_NAMES.length - 1);

  const abstained = await evaluateAutonomousSelectionPolicy(oneCase, {
    selector: () => ({ selected_model: null, strategy: "caller_selector", ranking: [], abstention_reason: "caller gate" }),
  });
  assert.equal(abstained.abstained_case_count, 1);
  assert.equal(abstained.cases[0].status, "abstained");
  assert.equal(abstained.cases[0].oracle_model_id, "lab/coding-cheap");

  const missingReward = await evaluateAutonomousSelectionPolicy([{
    ...oneCase[0],
    rewards: {},
  }]);
  assert.equal(missingReward.no_counterfactual_reward_count, 1);
  assert.equal(missingReward.cases[0].status, "no_counterfactual_reward");

  const noEligible = await evaluateAutonomousSelectionPolicy([{
    ...oneCase[0],
    request: {
      ...request("coding", { disabled: true }),
      candidates: request("coding", { disabled: true }).candidates.map((candidate) => ({ ...candidate, enabled: false })),
    },
    rewards: { "lab/coding-cheap": 0.5 },
  }]);
  assert.equal(noEligible.no_eligible_model_count, 1);
  assert.equal(noEligible.cases[0].status, "no_eligible_model");
});

test("selection lab accepts the online learner and rejects selector contract violations", async () => {
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0, seed: 17 } });
  const report = await evaluateAutonomousSelectionPolicy(labCases(), { learner });
  assert.equal(report.selector_label, "autonomous_online_learner");
  assert.equal(report.evaluated_case_count, AUTONOMOUS_DOMAIN_NAMES.length);

  await assert.rejects(
    () => evaluateAutonomousSelectionPolicy([labCases()[0]], {
      selector: () => ({ selected_model: { provider: "lab", model: "coding-quality" }, strategy: "caller_selector", ranking: [] }),
      learner,
    }),
    (error) => error instanceof ArgumentError && /cannot both/.test(error.message),
  );

  await assert.rejects(
    () => evaluateAutonomousSelectionPolicy([labCases()[0]], {
      selector: () => ({ selected_model: { provider: "lab", model: "unknown" }, strategy: "caller_selector", ranking: [] }),
    }),
    (error) => error instanceof ArgumentError && /unknown model arm/.test(error.message),
  );

  await assert.rejects(
    () => evaluateAutonomousSelectionPolicy([{
      ...labCases()[0],
      request: request("coding", { disabled: true }),
    }], {
      selector: () => ({ selected_model: { provider: "lab", model: "coding-quality" }, strategy: "caller_selector", ranking: [] }),
    }),
    (error) => error instanceof ArgumentError && /ineligible/.test(error.message),
  );
});
