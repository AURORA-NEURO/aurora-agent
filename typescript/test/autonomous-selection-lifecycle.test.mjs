import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousOnlineLearner,
  AutonomousSelectionPromotionLifecycle,
  AutonomousSelectionPromotionLifecycleStore,
  LLMRuntime,
  evaluateAutonomousSelectionPolicy,
  evaluateAutonomousSelectionPromotion,
} from "../dist/index.js";

function request(domain) {
  return {
    task: `private lifecycle task for ${domain}`,
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
    provider_health: { lab: { provider: "lab", circuit: "closed", credential_required: false, credential_ready: true, structured_output_mode: "json_object" } },
    model_health: {},
  };
}

function cases(selectedArmWins) {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({
    case_id: `${domain}-lifecycle-case`,
    domain,
    request: request(domain),
    rewards: selectedArmWins
      ? { [`lab/${domain}-quality`]: 0.95, [`lab/${domain}-cheap`]: 0.25 }
      : { [`lab/${domain}-quality`]: 0.25, [`lab/${domain}-cheap`]: 0.95 },
  }));
}

test("selection lifecycle applies hold, admission, rollback, and durable restore", async () => {
  const admittedReplay = await evaluateAutonomousSelectionPolicy(cases(true), { requireAllDomains: true });
  const admitted = evaluateAutonomousSelectionPromotion(admittedReplay);
  const heldReplay = await evaluateAutonomousSelectionPolicy(cases(false), { requireAllDomains: true });
  const held = evaluateAutonomousSelectionPromotion(heldReplay);
  const lifecycle = new AutonomousSelectionPromotionLifecycle({ lifecycleId: "selection-lifecycle-test", clock: () => 100 });

  assert.equal(lifecycle.state.status, "uninitialized");
  assert.equal(lifecycle.apply(held).status, "held");
  assert.equal(lifecycle.apply(admitted).status, "admitted");
  assert.equal(lifecycle.state.generation, 1);
  assert.equal(lifecycle.state.active_promotion_digest, admitted.promotion_digest);

  const store = new AutonomousSelectionPromotionLifecycleStore();
  await store.save(lifecycle.state);
  const snapshot = await store.snapshot();
  const restoredStore = new AutonomousSelectionPromotionLifecycleStore();
  await restoredStore.restore(snapshot);
  assert.equal((await restoredStore.load()).state_digest, lifecycle.state.state_digest);

  const rolledBack = lifecycle.rollback("operator detected drift");
  assert.equal(rolledBack.status, "rolled_back");
  assert.equal(rolledBack.active_promotion_digest, null);
  assert.equal(rolledBack.rollback_count, 1);
  assert.equal(JSON.stringify(rolledBack).includes("private lifecycle task"), false);
});

test("selection lifecycle joins all-domain readiness and gates the learner until admission", async () => {
  const replay = await evaluateAutonomousSelectionPolicy(cases(true), { requireAllDomains: true });
  const promotion = evaluateAutonomousSelectionPromotion(replay);
  const lifecycle = new AutonomousSelectionPromotionLifecycle({ lifecycleId: "selection-readiness-test", clock: () => 200 });
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } });
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner(), selectionPromotion: lifecycle });
  agent.registerModel({
    provider: "offline",
    model: "offline-model",
    capabilities: ["structured_output", "reasoning", "science", "code", "web", "data", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 10,
    cost_per_million_tokens: 0,
    reliability: 0.99,
  });

  const held = await agent.readiness({ requirePromotedSelection: true, selectionPromotionReport: promotion });
  assert.equal(held.learning.selection_promotion.lifecycle_status, "uninitialized");
  assert.equal(held.learning.selection_promotion.decision, "admit");
  assert.equal(held.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(held.domains.every((row) => row.selection_promotion.domain_decision === "admit"), true);
  assert.equal(held.domains.every((row) => row.state === "partial"), true);

  agent.applySelectionPromotion(promotion);
  const ready = await agent.readiness({ requirePromotedSelection: true, selectionPromotionReport: promotion });
  assert.equal(ready.learning.selection_promotion.lifecycle_status, "admitted");
  assert.equal(ready.learning.selection_promotion.active_promotion_digest, promotion.promotion_digest);
  assert.equal(ready.domains.every((row) => row.selection_promotion.status === "admitted"), true);
  assert.equal(ready.next_actions.some((action) => action.includes("apply an admitted all-domain")), false);

  const persisted = new AutonomousSelectionPromotionLifecycleStore();
  await agent.saveSelectionPromotion(persisted);
  const restoredLifecycle = new AutonomousSelectionPromotionLifecycle({ lifecycleId: "restored-selection-readiness-test", clock: () => 201 });
  const restoredAgent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner(), selectionPromotion: restoredLifecycle });
  await restoredAgent.restoreSelectionPromotion(persisted);
  assert.equal(restoredAgent.selectionPromotionState().active_promotion_digest, promotion.promotion_digest);
});
