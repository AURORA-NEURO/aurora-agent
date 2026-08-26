import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  LLMRuntime,
  buildAutonomousDomainOperatingKit,
  buildAutonomousDomainOperatingKits,
  validateAutonomousDomainOperatingKit,
} from "../dist/index.js";

test("builds a complete operating kit for every built-in domain", async () => {
  const kits = await buildAutonomousDomainOperatingKits();

  assert.deepEqual(kits.map((kit) => kit.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(kits.length, 12);
  for (const kit of kits) {
    assert.equal(kit.status, "complete");
    assert.deepEqual(Object.values(kit.coverage), [true, true, true, true, true, true, true, true, true]);
    assert.ok(kit.stages.length >= 4);
    assert.ok(kit.capability_graph.length > 0);
    assert.match(kit.kit_digest, /^[0-9a-f]{64}$/);
    assert.ok(kit.stages.every((stage) => stage.prompt_candidate_ids.length > 0 && stage.selected_prompt_id));
    assert.ok(kit.stages.every((stage) => stage.tool_names.length > 0));
    assert.ok(kit.stages.every((stage) => /^[0-9a-f]{64}$/.test(stage.stage_digest)));
    assert.equal(JSON.stringify(kit).includes("api_key"), false);
    assert.equal(JSON.stringify(kit).includes("credential_value"), false);
  }
});

test("operating-kit facade is keyless and provider-free", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { providerCalls += 1; throw new Error("operating kit must not dispatch"); } });
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(runtime) });

  const kit = await brain.domainOperatingKit("operations");
  const all = await brain.domainOperatingKits(["coding", "evaluation"]);
  assert.equal(kit.domain, "operations");
  assert.deepEqual(all.map((item) => item.domain), ["coding", "evaluation"]);
  assert.equal(providerCalls, 0);
});

test("operating kits reject stale stage and kit digests", async () => {
  const kit = await buildAutonomousDomainOperatingKit("coding");
  const stageTampered = structuredClone(kit);
  stageTampered.stages[0].objective = "unreviewed objective";
  await assert.rejects(() => validateAutonomousDomainOperatingKit(stageTampered), /stage .*digest|stale or tampered/);

  const kitTampered = structuredClone(kit);
  kitTampered.next_actions = [...kitTampered.next_actions, "tampered handoff"];
  await assert.rejects(() => validateAutonomousDomainOperatingKit(kitTampered), /stale or tampered|digest/);
  assert.deepEqual(await validateAutonomousDomainOperatingKit(kit), kit);
});
