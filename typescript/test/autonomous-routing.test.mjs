import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousExecutionController,
  CredentialStore,
  InMemoryAutonomousExecutionJournal,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
  semanticRouteAutonomousTask,
} from "../dist/index.js";

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

function model() {
  return {
    provider: "router-provider",
    model: "router-model",
    capabilities: ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function routerAgent(payloads) {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      const payload = payloads[Math.min(calls, payloads.length - 1)];
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(payload) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("router-provider", "https://router.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  return { agent, bodies, calls: () => calls };
}

test("semantic routing resolves an ambiguous task into a reviewed domain route", async () => {
  const { agent, bodies, calls } = routerAgent([{ selected_domains: [{ domain: "coding", score: 0.91, rationale: "implementation and verification" }], confidence: 0.92, abstain: false, abstain_reason: null }]);
  const task = "Please help me with an unfamiliar technical migration.";
  const result = await semanticRouteAutonomousTask(agent, task, { approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.route.source, "provider_semantic_hybrid");
  assert.equal(result.deterministic_route.abstained, true);
  assert.equal(result.semantic_confidence, 0.92);
  assert.equal(result.route.route_digest.length, 64);
  assert.equal(result.outcome_digest.length, 64);
  assert.equal(calls(), 1);
  assert.equal(JSON.stringify(result).includes(task), false);
  assert.equal(Object.prototype.hasOwnProperty.call(bodies[0], "authorization"), false);
});

test("semantic routing applies caller model-selection gates before classifier dispatch", async () => {
  const { agent, calls } = routerAgent([{ selected_domains: [{ domain: "coding", score: 0.91, rationale: "implementation" }], confidence: 0.92, abstain: false, abstain_reason: null }]);
  await assert.rejects(
    () => semanticRouteAutonomousTask(agent, "Route this ambiguous technical task", { approveProviderCall: true, maxCostPerMillionTokens: 1, maxLatencyMs: 50, minQuality: 0.95 }),
    /abstain|eligible|cost|latency|quality/,
  );
  assert.equal(calls(), 0, "semantic selection gates must run before classifier transport");
});

test("semantic routing fails the supplied execution when provider dispatch throws", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("semantic provider offline"); } });
  llm.registerProvider(openaiCompatibleProvider("router-provider", "https://router.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const execution = await AutonomousExecutionController.create({ executionId: "semantic-routing-failure-1", domain: "cross_domain", capability: "routing", riskClass: "route_review", journal: new InMemoryAutonomousExecutionJournal() });
  await assert.rejects(
    semanticRouteAutonomousTask(agent, "Route this ambiguous technical task", { approveProviderCall: true, execution }),
    /provider/,
  );
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("semantic routing refuses provider/deterministic disagreement instead of overriding the baseline", async () => {
  const { agent } = routerAgent([{ selected_domains: [{ domain: "biomedical", score: 0.95, rationale: "medical terminology" }], confidence: 0.95, abstain: false, abstain_reason: null }]);
  const result = await semanticRouteAutonomousTask(agent, "Debug this Rust repository and fix the failing tests.", { approveProviderCall: true });
  assert.equal(result.status, "provider_disagreement");
  assert.equal(result.deterministic_route.primary_domain, "coding");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.route.source, "deterministic_vocabulary");
});

test("semantic routing preserves approval and provider-abstention gates", async () => {
  const approved = routerAgent([{ selected_domains: [{ domain: "coding", score: 0.9, rationale: "code" }], confidence: 0.9, abstain: false, abstain_reason: null }]);
  const gated = await semanticRouteAutonomousTask(approved.agent, "an ambiguous task", { approveProviderCall: false });
  assert.equal(gated.status, "approval_required");
  assert.equal(approved.calls(), 0);

  const abstaining = routerAgent([{ selected_domains: [], confidence: 0.12, abstain: true, abstain_reason: "insufficient context" }]);
  const result = await semanticRouteAutonomousTask(abstaining.agent, "an ambiguous task", { approveProviderCall: true });
  assert.equal(result.status, "provider_abstained");
  assert.equal(result.route.abstained, true);
  assert.equal(result.semantic_selected_domains.length, 0);
});

test("semantic routing converts malformed provider output into a typed refusal", async () => {
  const { agent } = routerAgent([{ selected_domains: [{ domain: "coding", score: 0.9, rationale: "x".repeat(513) }], confidence: 0.9, abstain: false, abstain_reason: null }]);
  const result = await semanticRouteAutonomousTask(agent, "route this implementation task", { approveProviderCall: true });
  assert.equal(result.status, "provider_invalid");
  assert.equal(result.route.source, "deterministic_vocabulary");
  assert.equal(result.semantic_candidates.length, 0);
});

test("semantic routing covers every built-in domain with catalogue-authoritative capabilities", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const payloads = profiles.map((profile) => ({ selected_domains: [{ domain: profile.domain, score: 0.9, rationale: `catalogue route for ${profile.domain}` }], confidence: 0.9, abstain: false, abstain_reason: null }));
  const { agent } = routerAgent(payloads);
  for (const profile of profiles) {
    const result = await semanticRouteAutonomousTask(agent, `route this ${profile.domain} task`, { approveProviderCall: true });
    assert.equal(result.status, "completed", profile.domain);
    assert.equal(result.route.primary_domain, profile.domain);
    assert.equal(result.route.candidates[0].capability, profile.default_capability);
  }
});
