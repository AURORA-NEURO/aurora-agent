import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  autonomousDomainPolicy,
  autonomousDomainTaskLens,
  digestJsonSync,
  inferAutonomousTaskDecision,
  inferAutonomousTaskIntent,
} from "../dist/index.js";

function decision(task, domain) {
  const intent = inferAutonomousTaskIntent({
    task,
    taskDigest: digestJsonSync({ task }),
    domain,
    capability: "review",
    riskClass: "read_only",
    workflowId: `${domain}_workflow`,
    lens: autonomousDomainTaskLens(domain),
    desiredOutputs: ["review decision"],
  });
  return inferAutonomousTaskDecision({
    intent,
    lens: autonomousDomainTaskLens(domain),
    policy: autonomousDomainPolicy(domain),
    requiredModelCapabilities: ["reasoning", "structured_output"],
  });
}

test("task decision is digest-bound and blocks forbidden biomedical effects", () => {
  const task = "deploy the biomedical report and verify safety";
  const intent = inferAutonomousTaskIntent({
    task,
    taskDigest: digestJsonSync({ task }),
    domain: "biomedical",
    capability: "biomedical_analysis",
    riskClass: "clinical_review",
    workflowId: "biomedical_review",
    lens: autonomousDomainTaskLens("biomedical"),
    desiredOutputs: ["safety boundary"],
  });
  const result = inferAutonomousTaskDecision({
    intent,
    lens: autonomousDomainTaskLens("biomedical"),
    policy: autonomousDomainPolicy("biomedical"),
    requiredModelCapabilities: ["reasoning", "biomedical", "structured_output"],
  });
  assert.equal(result.decision_digest, "29a60c4c19879b835edb25c6b20ce6e0c9a12b9cfa479a24d1420714f039e848");
  assert.equal(result.posture, "blocked");
  assert.equal(result.recommended_path, "evidence_first");
  assert.ok(result.blocking_reasons.includes("requested_effect_forbidden_by_domain_policy"));
  assert.ok(result.approval_requirements.includes("evidence_dispatch"));
  assert.equal(JSON.stringify(result).includes(task), false);
  assert.equal(result.authorization, "guidance_only;provider_source_tool_and_effect_authority_remain_separate");
});

test("task decision covers all domains and rejects invalid inputs", () => {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = decision(`analyze the ${domain} workflow`, domain);
    assert.equal(result.domain, domain);
    assert.equal(result.decision_digest.length, 64);
    assert.ok(result.approval_requirements.length > 0);
    assert.ok(result.next_actions.length > 0);
  }
  const task = "analyze the data workflow";
  const intent = inferAutonomousTaskIntent({ task, taskDigest: digestJsonSync({ task }), domain: "data", capability: "review", riskClass: "read_only", workflowId: "data_workflow", lens: autonomousDomainTaskLens("data") });
  assert.throws(() => inferAutonomousTaskDecision({ intent, lens: autonomousDomainTaskLens("data"), policy: autonomousDomainPolicy("data"), requiredModelCapabilities: [] }));
});
