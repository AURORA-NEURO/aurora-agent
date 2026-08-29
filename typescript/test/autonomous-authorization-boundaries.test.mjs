import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousAuthorizationError,
  AutonomousAuthorizationContext,
  AutonomousAuthorizationGate,
  AutonomousAuthorizationLedger,
  CredentialStore,
  InMemoryAutonomousEpisodicMemory,
  LLMRuntime,
  AutonomousRunAnalyticsLedger,
  AutonomousRunTraceSession,
  InMemoryAutonomousRunTraceStore,
  analyzeAutonomousRunTrace,
  openaiCompatibleProvider,
  runAutonomousDecisionCycle,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

function contextFor(operations) {
  const ledger = new AutonomousAuthorizationLedger(4, 512);
  const grant = ledger.issue({
    grant_id: "boundary-grant",
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: digest("a"),
    allowed_domains: [...AUTONOMOUS_DOMAIN_NAMES],
    allowed_operations: operations,
    allowed_capabilities: [],
    allowed_risk_classes: [],
    issued_at: 1000,
    expires_at: 100000,
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
      [...AUTONOMOUS_DOMAIN_NAMES],
      null,
      null,
      "boundary",
      () => 2000,
    ),
  };
}

test("trace writes are authorized across every built-in domain", async () => {
  const { ledger, context } = contextFor(["trace_write"]);
  const store = new InMemoryAutonomousRunTraceStore({ clock: () => 2000 });
  const session = new AutonomousRunTraceSession(store, {
    run_id: "boundary-trace",
    task_digest: digest("b"),
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
    authorizationContext: context,
  });
  await session.started();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) await session.record({ phase: "plan_compiled", status: "running", domains: [domain] });
  await session.complete({ status: "completed" });
  assert.equal(ledger.events().length, AUTONOMOUS_DOMAIN_NAMES.length + 3);
});

test("analytics ingestion authorizes the verified report before mutating the ledger", async () => {
  const traceStore = new InMemoryAutonomousRunTraceStore({ clock: () => 3000 });
  const trace = new AutonomousRunTraceSession(traceStore, {
    run_id: "analytics-trace",
    task_digest: digest("c"),
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
  });
  await trace.started();
  await trace.complete({ status: "completed" });
  const report = analyzeAutonomousRunTrace(await traceStore.snapshot());
  const { ledger, context } = contextFor(["analytics_write"]);
  const analytics = new AutonomousRunAnalyticsLedger({ authorizationContext: context });
  assert.equal(analytics.ingest(report).status, "accepted");
  assert.equal(ledger.events().length, 2);
  assert.equal(analytics.summary().report_count, 1);
});

test("decision-cycle memory retrieval cannot bypass the caller authorization context", async () => {
  const { context } = contextFor(["provider_invocation"]);
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("provider must not be reached"); } });
  llm.registerProvider(openaiCompatibleProvider("authorization-cycle-provider", "https://authorization-cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({
    provider: "authorization-cycle-provider",
    model: "authorization-cycle-model",
    capabilities: ["reasoning", "code"],
    context_window_tokens: 32_000,
    max_output_tokens: 1_000,
    quality: 0.9,
    latency_ms: 10,
    cost_per_million_tokens: 1,
    reliability: 0.99,
  });
  const memory = new InMemoryAutonomousEpisodicMemory();
  await assert.rejects(
    () => runAutonomousDecisionCycle(agent, "review this coding change", {
      domain: "coding",
      approveProviderCall: true,
      memory: { store: memory },
      authorizationContext: context,
    }),
    AutonomousAuthorizationError,
  );
  assert.deepEqual(await memory.retrieve({}), []);
});
