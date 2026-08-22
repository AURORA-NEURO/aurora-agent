import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousConnectorOperationFacade,
  AutonomousConnectorOperationRegistry,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  InMemoryAutonomousConnectorReceiptJournal,
  createBuiltinAutonomousConnectorRuntime,
} from "../dist/index.js";

function input(domain, operationRegistry, overrides = {}) {
  const operation = operationRegistry.forDomain(domain)[0];
  return {
    domain,
    capability: operation.capabilities[0],
    operation_id: operation.operation_id,
    subject_digest: "a".repeat(64),
    request: { fixture_label: `transient-${domain}`, source_digest: "b".repeat(64) },
    ...overrides,
  };
}

test("connector operation facade selects and invokes every domain through one reviewed path", async () => {
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const fixture = createBuiltinAutonomousConnectorRuntime({
    domainScoped: true,
    approvalRequired: false,
    receiptStore: journal,
  });
  const facade = new AutonomousConnectorOperationFacade({
    registry: fixture.registry,
    runtime: fixture.runtime,
    operationRegistry: fixture.operationRegistry,
  });
  assert.equal(fixture.operationFacade.constructor, AutonomousConnectorOperationFacade);
  const result = await facade.executeBatch(
    AUTONOMOUS_DOMAIN_NAMES.map((domain) => input(domain, fixture.operationRegistry)),
    { maxParallelism: 4 },
  );

  assert.equal(result.status, "completed");
  assert.equal(result.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.failed_count, 0);
  assert.equal(result.omitted_count, 0);
  assert.deepEqual(result.items.map((item) => item.index), [...Array(AUTONOMOUS_DOMAIN_NAMES.length).keys()]);
  assert.equal((await journal.verifyIntegrity()).entries, AUTONOMOUS_DOMAIN_NAMES.length);
  for (const item of result.items) {
    assert.equal(item.status, "succeeded");
    assert.ok(item.execution);
    assert.equal(item.execution.operation_plan.status, "ready");
    assert.equal(item.execution.dispatch.receipt.status, "partial");
    assert.equal(JSON.stringify(item.execution.operation_plan).includes("transient-"), false);
  }
});

test("facade plans are request-free and replay identities retain approval transitions", async () => {
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const fixture = createBuiltinAutonomousConnectorRuntime({
    domainScoped: true,
    approvalRequired: true,
    receiptStore: journal,
  });
  const facade = new AutonomousConnectorOperationFacade({
    registry: fixture.registry,
    runtime: fixture.runtime,
    operationRegistry: fixture.operationRegistry,
  });
  const base = input("science", fixture.operationRegistry, { request: { hypothesis: "transient hypothesis" } });
  const refused = await facade.execute({ ...base, approved: false });
  assert.equal(refused.status, "refused");
  assert.equal(refused.dispatch.receipt.failure_class, "approval_required");
  const approvedPlan = facade.plan({ ...base, approved: true });
  assert.equal(approvedPlan.status, "ready");
  assert.equal(JSON.stringify(approvedPlan).includes("transient hypothesis"), false);
  const approved = await facade.execute({ ...base, approved: true });
  assert.equal(approved.status, "partial");
  assert.equal(approved.replay, "fresh");
  const replay = await facade.execute({ ...base, approved: true });
  assert.equal(replay.replay, "replayed");
  assert.equal(replay.dispatch.value, null);
  assert.notEqual(refused.operation_plan.plan_digest, approved.operation_plan.plan_digest);
  assert.equal((await journal.verifyIntegrity()).entries, 2);
});

test("facade fails closed on operation scope, credential-shaped metadata, and missing connectors", async () => {
  const fixture = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const facade = new AutonomousConnectorOperationFacade({
    registry: fixture.registry,
    runtime: fixture.runtime,
    operationRegistry: fixture.operationRegistry,
  });
  const science = input("science", fixture.operationRegistry);
  assert.throws(() => facade.plan({ ...science, request: { api_key: "never-accepted" } }), ArgumentError);
  assert.throws(() => facade.plan({ ...science, domain: "browser" }), /domain does not match/);
  assert.throws(() => facade.plan({ ...science, operation_id: "browser.web_evidence_retrieval" }), /domain does not match/);

  const emptyRegistry = new AutonomousConnectorRegistry();
  const missing = new AutonomousConnectorOperationFacade({
    registry: emptyRegistry,
    runtime: new AutonomousConnectorRuntime(emptyRegistry),
    operationRegistry: new AutonomousConnectorOperationRegistry(),
  });
  const batch = await missing.executeBatch([
    input("coding", missing.operationRegistry),
    input("science", missing.operationRegistry),
  ], { maxParallelism: 1, stopOnError: true });
  assert.equal(batch.status, "failed");
  assert.equal(batch.failed_count, 1);
  assert.equal(batch.omitted_count, 1);
  assert.equal(batch.items[0].status, "failed");
  assert.equal(batch.items[1].status, "omitted");
});

test("planned connector execution rejects changed transient metadata before dispatch", async () => {
  const fixture = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const facade = new AutonomousConnectorOperationFacade({
    registry: fixture.registry,
    runtime: fixture.runtime,
    operationRegistry: fixture.operationRegistry,
  });
  const original = input("data", fixture.operationRegistry, { approved: true });
  const plan = facade.plan(original);
  await assert.rejects(
    facade.executePlanned(plan, { ...original, request: { fixture_label: "changed", source_digest: "b".repeat(64) } }),
    /does not match the supplied transient request/,
  );
});
