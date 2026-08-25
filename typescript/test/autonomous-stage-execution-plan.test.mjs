import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousDomainToolRegistry,
  AutonomousDomainToolRuntime,
  AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA,
  AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  autonomousWorkflowStageContractDigest,
  builtinAutonomousDomainProfiles,
  compileAutonomousWorkflowStageExecutionPlan,
  digestJson,
} from "../dist/index.js";

test("stage execution packets compile for every built-in domain and remain payload-free", async () => {
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
  const profiles = await builtinAutonomousDomainProfiles();
  assert.equal(profiles.length, 12);

  for (const profile of profiles) {
    const blueprint = (await agent.blueprint(`Prepare a bounded ${profile.domain} workflow`, { domain: profile.domain, tools: [] })).blueprint;
    assert.ok(blueprint, profile.domain);
    assert.equal(blueprint.stage_execution_plans.length, profile.workflow.stages.length, profile.domain);
    for (const stage of profile.workflow.stages) {
      const plan = blueprint.stage_execution_plans.find((candidate) => candidate.stage_id === stage.id);
      assert.ok(plan, `${profile.domain}/${stage.id}`);
      assert.equal(plan.schema, AUTONOMOUS_WORKFLOW_STAGE_PLAN_SCHEMA);
      assert.equal(plan.domain, profile.domain);
      assert.equal(plan.workflow_digest, profile.workflow.workflow_digest);
      assert.equal(plan.stage_plan_digest.length, 64);
      assert.ok(plan.required_capabilities.length > 0);
      assert.ok(plan.capability_contracts.length > 0);
      assert.ok(plan.capability_contracts.every((contract) => contract.schema === AUTONOMOUS_CAPABILITY_CONTRACT_SCHEMA));
      assert.deepEqual(plan.capability_contract_digests, plan.capability_contracts.map((contract) => contract.contract_digest));
      const { stage_plan_digest: digest, capability_contract_digests: _contractDigests, credential_posture: _credentials, authority_posture: _authority, ...descriptor } = plan;
      assert.equal(await digestJson(descriptor), digest, `${profile.domain}/${stage.id} digest`);
      assert.equal(await autonomousWorkflowStageContractDigest(profile.workflow, stage.id), await autonomousWorkflowStageContractDigest(blueprint.workflow, stage.id));
      assert.doesNotMatch(JSON.stringify(plan), /Prepare a bounded/);
      assert.doesNotMatch(JSON.stringify(plan), /api[_-]?key|bearer|private[_-]?key|refresh[_-]?token/i);
    }
  }
});

test("stage dispatch rejects stale stage contracts and unselected tools before adapter execution", async () => {
  let executions = 0;
  const catalogue = await ToolCatalogue.fromDefinitions([{
    name: "repository_catalog",
    description: "Inspect bounded repository metadata.",
    inputSchema: { type: "object", additionalProperties: false },
  }]);
  const registry = await AutonomousDomainToolRegistry.create(catalogue);
  const runtime = new AutonomousDomainToolRuntime(registry, async () => {
    executions += 1;
    return { ok: true };
  });
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: catalogue,
    toolExecutor: async () => ({ ok: true }),
  });
  const blueprint = (await agent.blueprint("Inspect a bounded coding repository", { domain: "coding", tools: ["repository_catalog"] })).blueprint;
  assert.ok(blueprint);
  const stage = blueprint.workflow.stages.find((candidate) => candidate.id === "inspect");
  assert.ok(stage);
  const plan = blueprint.stage_execution_plans.find((candidate) => candidate.stage_id === stage.id);
  assert.ok(plan);
  assert.ok(plan.selected_tool_names.includes("repository_catalog"));
  const baseContext = {
    domain: "coding",
    workflow_id: blueprint.workflow.workflow_id,
    workflow_digest: blueprint.workflow.workflow_digest,
    stage_id: stage.id,
    stage_plan_digest: plan.stage_plan_digest,
    stage_contract_digest: await autonomousWorkflowStageContractDigest(blueprint.workflow, stage.id),
    selected_tool_names: [...plan.selected_tool_names],
  };
  const call = { id: "stage-contract-call", name: "repository_catalog", arguments: {} };
  const refused = await runtime.authorizeAndExecute([call], { domains: ["coding"], workflowContext: { ...baseContext, stage_contract_digest: "a".repeat(64) } });
  assert.equal(refused[0].content.status, "execution_failed");
  assert.equal(executions, 0);

  const allowed = await runtime.authorizeAndExecute([call], { domains: ["coding"], workflowContext: baseContext });
  assert.equal(allowed[0].approved, true);
  assert.equal(executions, 1);
  const excluded = await runtime.authorizeAndExecute([call], { domains: ["coding"], workflowContext: { ...baseContext, selected_tool_names: [] } });
  assert.equal(excluded[0].content.status, "execution_failed");
  assert.equal(executions, 1);
});
