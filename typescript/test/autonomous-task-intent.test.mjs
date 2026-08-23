import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES,
  autonomousDomainTaskLens,
  autonomousTaskIntentPromptContract,
  digestJsonSync,
  inferAutonomousTaskIntent,
} from "../dist/index.js";

test("task intent is bounded, domain-aware, and digest-parity compatible", () => {
  const task = "deploy the biomedical report and verify safety";
  const intent = inferAutonomousTaskIntent({
    task,
    taskDigest: digestJsonSync({ task }),
    domain: "biomedical",
    capability: "biomedical_analysis",
    riskClass: "clinical_review",
    workflowId: "biomedical_review",
    lens: autonomousDomainTaskLens("biomedical"),
    constraints: ["qualified review"],
    desiredOutputs: ["safety boundary"],
  });
  assert.equal(intent.intent_digest, "e984ee4cabfaa0a2463ead4c6d4042a85ba5da9d5ad28283f0b6257366f6508d");
  assert.equal(intent.action_mode, "evaluate");
  assert.equal(intent.requested_effect, "external_effect");
  assert.equal(intent.evidence_mode, "grounding_and_safety_evidence");
  assert.ok(intent.ambiguity_flags.includes("effect_requires_explicit_approval"));
  assert.ok(intent.risk_signals.includes("human_review_boundary"));
  assert.equal(intent.authorization, "classification_only;no_provider_tool_or_effect_authority");
  assert.equal(JSON.stringify(intent).includes(task), false);
  assert.equal(intent.secret_material, "never_returned");

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const domainTask = `review the ${domain} workflow and report verification gaps`;
    const domainIntent = inferAutonomousTaskIntent({
      task: domainTask,
      taskDigest: digestJsonSync({ task: domainTask }),
      domain,
      capability: "reasoning",
      riskClass: "read_only",
      workflowId: `${domain}_workflow`,
      lens: autonomousDomainTaskLens(domain),
    });
    assert.ok(AUTONOMOUS_TASK_INTENT_EVIDENCE_MODES.includes(domainIntent.evidence_mode));
    assert.equal(domainIntent.intent_digest.length, 64);
    assert.ok(domainIntent.planning_signals.length > 0);
    assert.ok(domainIntent.success_signals.length > 0);
  }
});

test("task intent prompt projection remains classification-only", () => {
  const task = "analyze the dataset lineage";
  const intent = inferAutonomousTaskIntent({
    task,
    taskDigest: digestJsonSync({ task }),
    domain: "data",
    capability: "data_analysis",
    riskClass: "read_only",
    workflowId: "data_analysis",
    lens: autonomousDomainTaskLens("data"),
  });
  const contract = autonomousTaskIntentPromptContract(intent);
  assert.equal(contract.intent_digest, intent.intent_digest);
  assert.equal(contract.authority, "classification_only;no_provider_tool_or_effect_authority");
  assert.equal(contract.secret_material, "never_returned");
});

test("task intent rejects malformed or duplicate input items", () => {
  const task = "analyze the dataset";
  const lens = autonomousDomainTaskLens("data");
  const base = {
    task,
    taskDigest: digestJsonSync({ task }),
    domain: "data",
    capability: "data_analysis",
    riskClass: "read_only",
    workflowId: "data_analysis",
    lens,
  };
  assert.throws(() => inferAutonomousTaskIntent({ ...base, constraints: ["schema", "schema"] }));
  assert.throws(() => inferAutonomousTaskIntent({ ...base, desiredOutputs: [""] }));
});
