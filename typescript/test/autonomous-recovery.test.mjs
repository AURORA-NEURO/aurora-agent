import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  planAutonomousRecovery,
  validateAutonomousRecoveryPlan,
} from "../dist/index.js";

test("recovery planning gives every built-in domain an explicit completion handoff", () => {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = planAutonomousRecovery({ domain, capability: "bounded_review", status: "completed" });
    assert.equal(plan.status, "completed");
    assert.equal(plan.next_action, "complete");
    assert.deepEqual(plan.actions, ["complete"]);
    assert.equal(plan.domain_guardrails.length, 2);
    assert.equal(validateAutonomousRecoveryPlan(plan).plan_digest, plan.plan_digest);
    assert.doesNotMatch(JSON.stringify(plan), /private task|(?:sk|gsk)-[A-Za-z0-9_-]{16,}/i);
  }
});

test("recovery planning preserves bounded retry intent and exhaustion", () => {
  const retry = planAutonomousRecovery({
    domain: "coding",
    capability: "provider_call",
    status: "failed",
    failure_code: "http_5xx",
    retryable: true,
    retry_count: 1,
    max_retries: 3,
  });
  assert.equal(retry.status, "retryable");
  assert.equal(retry.next_action, "retry_provider");
  assert.equal(retry.reason_codes[0], "bounded_retry_budget_remains");

  const exhausted = planAutonomousRecovery({
    domain: "coding",
    capability: "provider_call",
    status: "failed",
    failure_code: "http_5xx",
    retryable: true,
    retry_count: 3,
    max_retries: 3,
  });
  assert.equal(exhausted.status, "blocked");
  assert.equal(exhausted.next_action, "stop_and_escalate");
  assert.deepEqual(exhausted.reason_codes, ["retry_budget_exhausted"]);
});

test("recovery planning gives reconciliation and approval precedence over retry", () => {
  const uncertain = planAutonomousRecovery({
    domain: "operations",
    capability: "incident_response",
    status: "failed",
    failure_code: "transport",
    retryable: true,
    reconciliation_required: true,
  });
  assert.equal(uncertain.status, "reconciliation_required");
  assert.equal(uncertain.next_action, "reconcile_external_effect");
  assert.equal(uncertain.actions.includes("retry_provider"), false);

  const approval = planAutonomousRecovery({
    domain: "enterprise",
    capability: "change_request",
    status: "approval_required",
    retryable: true,
    approval_required: true,
  });
  assert.equal(approval.status, "held");
  assert.equal(approval.next_action, "approve_provider_call");
  assert.equal(approval.actions.includes("stop_and_escalate"), true);
});

test("recovery planning separates credential, route, quality, and policy remediation", () => {
  assert.equal(planAutonomousRecovery({ domain: "science", capability: "analysis", status: "failed", failure_code: "credential" }).next_action, "collect_credential");
  assert.equal(planAutonomousRecovery({ domain: "browser", capability: "search", status: "abstained", route_reviewed: false }).next_action, "review_route");
  assert.equal(planAutonomousRecovery({ domain: "biomedical", capability: "review", status: "response_review_required", response_quality_passed: false }).next_action, "review_response_quality");
  assert.equal(planAutonomousRecovery({ domain: "evaluation", capability: "audit", status: "policy_blocked", policy_admitted: false }).next_action, "review_domain_policy");
});

test("recovery plans reject secret-shaped observations and tampering", () => {
  assert.throws(
    () => planAutonomousRecovery({ domain: "coding", capability: "review", status: "failed", prompt: "private task" }),
    /unsupported fields/,
  );
  const plan = planAutonomousRecovery({ domain: "data", capability: "audit", status: "completed" });
  assert.throws(() => validateAutonomousRecoveryPlan({ ...plan, next_action: "retry_provider" }), /next_action|digest/);
  assert.throws(() => validateAutonomousRecoveryPlan({ ...plan, domain_guardrails: ["prompt"] }), /domain|secret|identifier|digest/);
  assert.throws(() => planAutonomousRecovery({ domain: "coding", capability: "review", status: "failed", response_quality_passed: "yes" }), /boolean/);
});
