import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousOnlineLearner,
  AutonomousProviderOutcomeEvaluator,
  CredentialStore,
  LLMRuntime,
  autonomousProviderOutcomeEvaluationInput,
  autonomousProviderReceiptIdentity,
  digestCanonicalJsonText,
  digestJson,
} from "../dist/index.js";

async function learningContextDigest(context) {
  return digestCanonicalJsonText(JSON.stringify(context));
}

async function receipt(domain, index, outcome = "success") {
  const selectionDigest = await digestJson({ selection: "reviewed", domain });
  const outcomeDigest = await digestJson({ provider: "provider-fixture", model: `model-${index}`, domain, outcome });
  return {
    schema: "bioprism-typescript-autonomous-provider-invocation/0.1",
    execution_id: `provider-evaluation-${index}`,
    provider: "provider-fixture",
    model: `model-${index}`,
    kind: "answer",
    attempt: 0,
    turn: 0,
    status: outcome === "success" ? "completed" : "provider_refused",
    outcome,
    input_tokens: 128,
    output_tokens: outcome === "success" ? 64 : 0,
    estimated_cost_units: 0.25,
    actual_cost_units: outcome === "success" ? 0.2 : 0,
    latency_ms: 40 + index,
    selection_digest: selectionDigest,
    outcome_digest: outcomeDigest,
    request_id_digest: null,
    failure_class: outcome === "success" ? null : "rate_limited",
    status_code: outcome === "success" ? null : 429,
    retention: "metadata_only_no_provider_payloads_or_credentials",
    secret_material: "never_returned",
  };
}

test("provider receipts drive explicit contextual model learning across every domain", async () => {
  const receipts = await Promise.all(AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => receipt(domain, index)));
  const contexts = Object.fromEntries(await Promise.all(receipts.map(async (item, index) => {
    const context = { domain: AUTONOMOUS_DOMAIN_NAMES[index], capability: "answer", risk_class: "read_only", task_family: "provider-evaluation" };
    return [autonomousProviderReceiptIdentity(item), { ...context, context_digest: await learningContextDigest(context), contract_digest: item.selection_digest }];
  })));
  const evidence = Object.fromEntries(receipts.map((item) => [autonomousProviderReceiptIdentity(item), { evaluator_signal: "reviewed" }]));
  const callbackInputs = [];
  const evaluator = new AutonomousProviderOutcomeEvaluator({
    evaluator_id: "provider-quality",
    evaluator_version: "2026-08-26",
    evaluate(input) {
      callbackInputs.push(input);
      assert.equal(input.evidence.evaluator_signal, "reviewed");
      assert.equal(input.prompt, undefined);
      assert.equal(input.response, undefined);
      assert.equal(input.messages, undefined);
      assert.equal(input.credentials, undefined);
      assert.match(input.selection_digest, /^[0-9a-f]{64}$/);
      assert.match(input.outcome_digest, /^[0-9a-f]{64}$/);
      return { reward: 0.8, passed: true };
    },
  });
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0, seed: 11 } });
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), { learner });
  const settled = await agent.evaluateProviderReceipts({ evaluator, receipts, contexts, evidence });
  assert.equal(settled.status, "completed");
  assert.equal(settled.receipts, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(callbackInputs.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(Object.keys(settled.by_domain).length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(Object.keys(settled.by_model).length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(settled.next_learning_state.generation, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(settled.next_learning_state.arms.length, 0);
  assert.equal(settled.next_learning_state.contextual_states.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(settled.next_learning_state.contextual_states.every((state) => state.arms.length === 1));
  assert.equal(settled.next_learning_state.credited_outcomes.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(settled.evaluations.every((item) => item.learning_update === "applied"));
  assert.ok(settled.evaluations.every((item) => item.context_digest));
  assert.doesNotMatch(JSON.stringify(settled), /prompt|response|credentials/i);

  const replayed = await agent.evaluateProviderReceipts({ evaluator, receipts, contexts, evidence, learningState: settled.next_learning_state });
  assert.equal(replayed.evaluations.every((item) => item.idempotent_replay), true);
  assert.deepEqual(replayed.next_learning_state, settled.next_learning_state);
  assert.equal(replayed.learning_digest, settled.learning_digest);
});

test("provider evaluation rejects duplicate identities, unsafe evidence, tampered context, and invalid rewards", async () => {
  const [first] = await Promise.all([receipt("coding", 1, "failure")]);
  const identity = autonomousProviderReceiptIdentity(first);
  const evaluator = new AutonomousProviderOutcomeEvaluator({
    evaluator_id: "provider-quality-safe",
    evaluator_version: "1",
    evaluate: () => ({ reward: 0, passed: false, failed: true }),
  });
  await assert.rejects(() => evaluator.evaluateReceipts([first, first]), /duplicate identities/);
  await assert.rejects(() => evaluator.evaluateReceipts([first], { evidence: { [identity]: { response: "forbidden" } } }), /transient or secret-shaped/);
  const context = { domain: "coding", capability: "answer", risk_class: "read_only", context_digest: await learningContextDigest({ domain: "coding", capability: "different", risk_class: "read_only", task_family: null }) };
  await assert.rejects(() => evaluator.evaluateReceipts([first], { contexts: { [identity]: context } }), /does not match its context/);
  const bad = new AutonomousProviderOutcomeEvaluator({ evaluator_id: "provider-quality-bad", evaluator_version: "1", evaluate: () => ({ reward: 2, passed: true }) });
  await assert.rejects(() => bad.evaluateReceipts([first]), /within \[-1, 1\]/);
  const input = await autonomousProviderOutcomeEvaluationInput(first, { context: { domain: "coding", capability: "answer", risk_class: "read_only" } });
  assert.equal(input.prompt, undefined);
  assert.equal(input.response, undefined);
  assert.equal(input.status, "provider_refused");
  assert.equal(input.outcome, "failure");
});
