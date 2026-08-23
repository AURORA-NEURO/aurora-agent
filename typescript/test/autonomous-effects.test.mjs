import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AutonomousDomainToolRuntime,
  AutonomousEffectBoundary,
  AutonomousEffectPersistenceCoordinator,
  AutonomousEffectReconciliationRequiredError,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  InMemoryAutonomousEffectJournal,
  TransactionalJsonAutonomousEffectSnapshotPersistence,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  AutonomousDomainToolRegistry,
} from "../dist/index.js";

const digest = (character) => character.repeat(64);

test("effect boundary records an idempotent metadata-only lifecycle", async () => {
  const effectJournal = new InMemoryAutonomousEffectJournal();
  const execution = await AutonomousExecutionController.create({
    executionId: "effect-execution-1",
    domain: "operations",
    capability: "incident_response",
    riskClass: "external_effect",
    policy: { allow_side_effects: true, max_effectful_calls: 1, max_steps: 16 },
  });
  await execution.admitToolCall({ tool: "incident_write", callId: "effect-call-1", readOnly: false, approvalRequired: true });
  let dispatched = 0;
  const boundary = new AutonomousEffectBoundary({ journal: effectJournal, execution });
  const request = { execution_id: execution.state.execution_id, tool: "incident_write", call_id: "effect-call-1", risk_class: "external_effect", arguments: { note: "private-value" } };
  const first = await boundary.execute(request, async (context) => {
    dispatched += 1;
    assert.match(context.idempotency_key, /^aurora-effect-[0-9a-f]{64}$/);
    return { accepted: true, value: "private-output" };
  });
  const second = await boundary.execute(request, async () => {
    dispatched += 1;
    return { accepted: false };
  });

  assert.deepEqual(second, first);
  assert.equal(dispatched, 1);
  assert.equal(execution.state.status, "running");
  assert.equal((await effectJournal.events()).map((row) => row.event.status).join(","), "prepared,dispatching,dispatched,completed");
  const snapshot = await effectJournal.snapshot();
  assert.doesNotMatch(JSON.stringify(snapshot), /private-value|private-output/);
  const restored = new InMemoryAutonomousEffectJournal();
  await restored.restore(snapshot);
  assert.equal((await restored.get(await boundary.effectId(request))).status, "completed");
  assert.equal((await effectJournal.verifyIntegrity()).verified, true);
});

test("effect journal persistence validates the chain and fences stale workers", async () => {
  const source = new InMemoryAutonomousEffectJournal();
  const boundary = new AutonomousEffectBoundary({ journal: source });
  await boundary.execute({ tool: "external_write", call_id: "effect-persistence-1", risk_class: "side_effecting", arguments: { private_value: "not-retained" } }, async () => ({ committed: true }));

  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observed !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousEffectSnapshotPersistence(textStore);
  const coordinator = new AutonomousEffectPersistenceCoordinator(source, persistence);
  const first = await coordinator.flush();
  assert.equal(JSON.parse(encoded).snapshot_digest, first.snapshot_digest);
  assert.doesNotMatch(encoded, /not-retained/);

  const stale = new AutonomousEffectPersistenceCoordinator(new InMemoryAutonomousEffectJournal(), persistence);
  await assert.rejects(() => stale.flush(), /compare-and-swap/);
  const restored = new AutonomousEffectPersistenceCoordinator(new InMemoryAutonomousEffectJournal(), persistence);
  const recovered = await restored.restore();
  assert.equal(recovered.snapshot_digest, first.snapshot_digest);
  const canonical = encoded;
  encoded = JSON.stringify(JSON.parse(canonical), null, 2);
  await assert.rejects(() => persistence.read(), /canonical/);
  encoded = canonical;
  encoded = "{invalid";
  await assert.rejects(() => persistence.read(), /invalid/);
});

test("uncertain effects refuse replay until a resolver confirms the external outcome", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const executionJournal = new InMemoryAutonomousExecutionJournal();
  const executionPolicy = { allow_side_effects: true, max_effectful_calls: 1, max_steps: 16 };
  const firstExecution = await AutonomousExecutionController.create({
    executionId: "effect-reconcile-1",
    domain: "enterprise",
    capability: "external_change",
    riskClass: "external_effect",
    policy: executionPolicy,
    journal: executionJournal,
  });
  // Keep the effect ledger and execution journal separate; only the effect ledger is
  // rehydrated by the resolver path below.
  await firstExecution.admitToolCall({ tool: "external_change", callId: "effect-call-2", readOnly: false, approvalRequired: true });
  const request = { execution_id: firstExecution.state.execution_id, tool: "external_change", call_id: "effect-call-2", risk_class: "external_effect", arguments: { change: "bounded" } };
  const firstBoundary = new AutonomousEffectBoundary({ journal, execution: firstExecution });
  await assert.rejects(
    firstBoundary.execute(request, async () => { throw new Error("transport_lost_after_dispatch"); }),
    AutonomousEffectReconciliationRequiredError,
  );
  assert.equal(firstExecution.state.status, "reconciliation_required");
  const effectId = await firstBoundary.effectId(request);

  const resolver = { resolve: async (record) => ({ status: record.effect_id === effectId ? "completed" : "unknown", result: { confirmed: true } }) };
  const resumedExecution = await AutonomousExecutionController.create({ executionId: "effect-reconcile-1", domain: "enterprise", capability: "external_change", riskClass: "external_effect", policy: executionPolicy, journal: executionJournal, resume: true });
  assert.equal(resumedExecution.state.status, "reconciliation_required");
  const recovered = new AutonomousEffectBoundary({ journal, resolver, execution: resumedExecution });
  const result = await recovered.execute(request, async () => {
    throw new Error("must_not_duplicate_external_dispatch");
  });
  assert.deepEqual(result, { confirmed: true });
  assert.equal(resumedExecution.state.status, "running");
  assert.equal((await journal.get(effectId)).status, "reconciled");
  assert.equal((await journal.events({ effectId })).at(-1).event.status, "reconciled");
});

test("custom tool adapters receive approval before the idempotency-aware executor", async () => {
  const boundary = new AutonomousEffectBoundary({ journal: new InMemoryAutonomousEffectJournal() });
  const calls = [
    { id: "custom-refused", name: "external_write", arguments: {} },
    { id: "custom-approved", name: "external_write", arguments: { value: "bounded" } },
  ];
  let executed = 0;
  const results = await boundary.authorizeAndExecute(calls, {
    approve: (call) => call.id === "custom-approved",
    execute: async (_call, context) => { executed += 1; assert.match(context.idempotency_key, /^aurora-effect-/); return { accepted: true }; },
  });
  assert.equal(results[0].approved, false);
  assert.equal(results[1].approved, true);
  assert.equal(executed, 1);
});

test("effect ledger is applied through the same boundary for all domain profiles", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const effectBindings = profiles.map((profile) => {
    const binding = profile.tool_profile.bindings.find((candidate) => !candidate.read_only) ?? profile.tool_profile.bindings[0];
    assert.ok(binding, `${profile.domain} must expose at least one curated binding`);
    return { profile, binding, effectful: !binding.read_only };
  });
  const definitions = await ToolCatalogue.fromDefinitions([...new Map(effectBindings.map(({ binding }) => [binding.name, { name: binding.name, description: binding.name, inputSchema: { type: "object", additionalProperties: true } }])).values()]);
  let externalExecutions = 0;
  for (const { profile, binding, effectful } of effectBindings) {
    const registry = await AutonomousDomainToolRegistry.create(definitions, [profile.tool_profile]);
    const execution = await AutonomousExecutionController.create({ executionId: `effect-domain-${profile.domain}`, domain: profile.domain, capability: profile.default_capability, riskClass: profile.risk_class, policy: { allow_side_effects: effectful, max_effectful_calls: effectful ? 1 : 0, max_steps: 16 } });
    const journal = new InMemoryAutonomousEffectJournal();
    const runtime = new AutonomousDomainToolRuntime(registry, async (_tool, _arguments, effectContext) => { externalExecutions += 1; if (effectful) assert.match(effectContext.idempotency_key, /^aurora-effect-/); return { domain: profile.domain, committed: true }; }, { effectBoundary: new AutonomousEffectBoundary({ journal }) });
    const callId = `domain-${profile.domain}`;
    await execution.admitToolCall({ tool: binding.name, callId, readOnly: binding.read_only, approvalRequired: effectful });
    const result = await runtime.authorizeAndExecute([{ id: callId, name: binding.name, arguments: {} }], { domains: [profile.domain], approveEffects: true, execution, effectBoundary: runtime.effectBoundary });
    assert.equal(result[0].approved, true, profile.domain);
    assert.equal((await journal.events()).at(-1)?.event.status ?? null, effectful ? "completed" : null, profile.domain);
  }
  assert.equal(externalExecutions, profiles.length);
});
