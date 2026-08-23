import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  evaluateAutonomousSelectionPolicy,
  evaluateAutonomousSelectionPromotion,
  validateAutonomousSelectionPromotionReport,
} from "../dist/index.js";

function request(domain) {
  return {
    task: `private promotion task for ${domain}; this text must never be retained`,
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
        context_window_tokens: 8192,
        max_output_tokens: 2048,
        quality: 0.9,
        latency_ms: 20,
        cost_per_million_tokens: 4,
        reliability: 0.92,
        enabled: true,
      },
      {
        provider: "lab",
        model: `${domain}-cheap`,
        capabilities: ["structured_output"],
        context_window_tokens: 8192,
        max_output_tokens: 2048,
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

function cases({ selectedArmWins = true } = {}) {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({
    case_id: `${domain}-promotion-case`,
    domain,
    request: request(domain),
    rewards: selectedArmWins
      ? { [`lab/${domain}-quality`]: 0.95, [`lab/${domain}-cheap`]: 0.25 }
      : { [`lab/${domain}-quality`]: 0.25, [`lab/${domain}-cheap`]: 0.95 },
  }));
}

test("selection promotion admits a complete high-agreement policy and is deterministic", async () => {
  const replay = await evaluateAutonomousSelectionPolicy(cases(), { requireAllDomains: true });
  const promotion = evaluateAutonomousSelectionPromotion(replay);

  assert.equal(promotion.decision, "admit");
  assert.deepEqual(promotion.reasons, []);
  assert.equal(promotion.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(promotion.domains.every((row) => row.decision === "admit" && row.oracle_agreement_rate === 1 && row.mean_regret === 0), true);
  assert.equal(JSON.stringify(promotion).includes("private promotion task"), false);
  assert.deepEqual(validateAutonomousSelectionPromotionReport(promotion), promotion);
  assert.equal(evaluateAutonomousSelectionPromotion(replay).promotion_digest, promotion.promotion_digest);
});

test("selection promotion holds incomplete and low-quality evidence with actionable reasons", async () => {
  const incompleteReplay = await evaluateAutonomousSelectionPolicy([cases()[0]], { requireAllDomains: true });
  const incomplete = evaluateAutonomousSelectionPromotion(incompleteReplay);
  assert.equal(incomplete.decision, "hold");
  assert.equal(incomplete.reasons.includes("selection replay report is not complete"), true);
  assert.equal(incomplete.domains.filter((row) => row.decision === "hold").length, AUTONOMOUS_DOMAIN_NAMES.length - 1);

  const lowQualityReplay = await evaluateAutonomousSelectionPolicy(cases({ selectedArmWins: false }), { requireAllDomains: true });
  const lowQuality = evaluateAutonomousSelectionPromotion(lowQualityReplay);
  assert.equal(lowQuality.decision, "hold");
  assert.equal(lowQuality.domains.every((row) => row.reasons.some((reason) => reason.includes("oracle agreement") && row.mean_regret !== null)), true);
  assert.equal(lowQuality.domains[0].mean_regret, 0.7);
});

test("selection promotion validates policy bounds and digest-bound tampering", async () => {
  const replay = await evaluateAutonomousSelectionPolicy(cases(), { requireAllDomains: true });
  assert.throws(
    () => evaluateAutonomousSelectionPromotion(replay, { maxMeanRegret: 3 }),
    (error) => error instanceof ArgumentError && /maxMeanRegret/.test(error.message),
  );

  const promotion = evaluateAutonomousSelectionPromotion(replay);
  assert.throws(
    () => validateAutonomousSelectionPromotionReport({ ...promotion, decision: "hold" }),
    (error) => error instanceof ArgumentError && /decision|digest/.test(error.message),
  );
  assert.throws(
    () => validateAutonomousSelectionPromotionReport({
      ...promotion,
      domains: promotion.domains.map((row, index) => index === 0 ? { ...row, reasons: ["tampered"] } : row),
    }),
    (error) => error instanceof ArgumentError && /digest|decision/.test(error.message),
  );
});
