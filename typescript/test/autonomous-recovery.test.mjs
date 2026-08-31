import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousBrainFacade,
  AutonomousRecoveryHandoffLedger,
  AutonomousRecoveryHandoffPersistenceCoordinator,
  TransactionalJsonAutonomousRecoveryHandoffPersistence,
  planAutonomousRecovery,
  validateAutonomousRecoveryHandoff,
  validateAutonomousRecoveryHandoffSnapshot,
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

test("recovery handoffs are idempotent, review-gated, and covered for every domain", () => {
  const ledger = new AutonomousRecoveryHandoffLedger();
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const plan = planAutonomousRecovery({ domain, capability: "provider_call", status: "failed", failure_code: "provider_error" });
    const result = ledger.submit({ plan, run_id_digest: `${String(index + 1).padStart(64, "0")}`, attempt: 0 });
    assert.equal(result.status, "accepted");
    assert.equal(result.handoff.status, "queued");
    assert.equal(result.handoff.domain, domain);
    assert.equal(validateAutonomousRecoveryHandoff(result.handoff).handoff_digest, result.handoff.handoff_digest);
    assert.doesNotMatch(JSON.stringify(result.handoff), /private task|(?:sk|gsk)-[A-Za-z0-9_-]{16,}/i);
  }
  const retryPlan = planAutonomousRecovery({ domain: "coding", capability: "provider_call", status: "failed", retryable: true, retry_count: 0, max_retries: 2 });
  const accepted = ledger.submit({ plan: retryPlan, run_id_digest: "a".repeat(64), attempt: 0 });
  const duplicate = ledger.submit({ plan: retryPlan, run_id_digest: "a".repeat(64), attempt: 0 });
  assert.equal(duplicate.status, "duplicate");
  assert.throws(() => ledger.review({ handoff_id: accepted.handoff.handoff_id, decision: "approve_retry", expected_revision: 99, reviewer_digest: "b".repeat(64) }), /stale/);
  const reviewed = ledger.review({ handoff_id: accepted.handoff.handoff_id, decision: "approve_retry", expected_revision: 1, reviewer_digest: "b".repeat(64) });
  assert.equal(reviewed.handoff.status, "retry_approved");
  assert.equal(reviewed.handoff.selected_action, "retry_provider");
  assert.throws(() => ledger.review({ handoff_id: accepted.handoff.handoff_id, decision: "close", expected_revision: 2, reviewer_digest: "b".repeat(64) }), /already reviewed/);
  const snapshot = ledger.snapshot();
  assert.equal(validateAutonomousRecoveryHandoffSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);
  const restored = new AutonomousRecoveryHandoffLedger();
  restored.restore(snapshot);
  assert.equal(restored.get(accepted.handoff.handoff_id)?.handoff_digest, reviewed.handoff.handoff_digest);
  assert.equal(restored.entries({ status: "retry_approved", domain: "coding" }).length, 1);
});

test("recovery handoff decisions fail closed for credentials, reconcile uncertain effects, and fence persistence", async () => {
  const ledger = new AutonomousRecoveryHandoffLedger();
  const credential = ledger.submit({
    plan: planAutonomousRecovery({ domain: "science", capability: "provider_call", status: "failed", failure_code: "credential" }),
    run_id_digest: "c".repeat(64),
    attempt: 0,
  });
  assert.throws(() => ledger.review({ handoff_id: credential.handoff.handoff_id, decision: "approve_retry", expected_revision: 1, reviewer_digest: "d".repeat(64) }), /does not authorize/);
  const uncertain = ledger.submit({
    plan: planAutonomousRecovery({ domain: "operations", capability: "incident_response", status: "failed", reconciliation_required: true }),
    run_id_digest: "e".repeat(64),
    attempt: 0,
  });
  const reconciled = ledger.review({ handoff_id: uncertain.handoff.handoff_id, decision: "approve_reconciliation", expected_revision: 1, reviewer_digest: "f".repeat(64) });
  assert.equal(reconciled.handoff.status, "reconciliation_required");
  assert.equal(reconciled.handoff.selected_action, "reconcile_external_effect");

  class Store {
    value = null;
    async read() { return this.value; }
    async write(value) { this.value = value; }
    async writeIfUnchanged(expected, value) {
      const current = this.value === null ? null : JSON.parse(this.value);
      const currentDigest = current?.snapshot_digest ?? null;
      if (currentDigest !== expected) return false;
      this.value = value;
      return true;
    }
  }
  const store = new Store();
  const persistence = new TransactionalJsonAutonomousRecoveryHandoffPersistence(store);
  const first = new AutonomousRecoveryHandoffLedger();
  const firstCoordinator = new AutonomousRecoveryHandoffPersistenceCoordinator(first, persistence);
  await firstCoordinator.restore();
  await firstCoordinator.flush();
  const second = new AutonomousRecoveryHandoffLedger();
  const secondCoordinator = new AutonomousRecoveryHandoffPersistenceCoordinator(second, persistence);
  await secondCoordinator.restore();
  first.submit({ plan: planAutonomousRecovery({ domain: "data", capability: "audit", status: "failed" }), run_id_digest: "1".repeat(64), attempt: 0 });
  await firstCoordinator.flush();
  second.submit({ plan: planAutonomousRecovery({ domain: "browser", capability: "search", status: "failed" }), run_id_digest: "2".repeat(64), attempt: 0 });
  await assert.rejects(() => secondCoordinator.flush(), /compare-and-swap/);
  const tampered = { ...first.snapshot(), entries: [...first.snapshot().entries] };
  tampered.entries[0] = { ...tampered.entries[0], status: "escalated" };
  assert.throws(() => validateAutonomousRecoveryHandoffSnapshot(tampered), /digest|inconsistent/);
});

test("the high-level brain facade exposes recovery planning without widening execution authority", () => {
  const facade = new AutonomousBrainFacade({
    agent: {
      route: () => undefined,
      blueprint: () => undefined,
      run: () => undefined,
      runCrossDomain: () => undefined,
      readiness: () => undefined,
      refreshActivation: () => undefined,
    },
  });
  const ledger = new AutonomousRecoveryHandoffLedger();
  const result = facade.submitRecoveryHandoff(ledger, {
    observation: { domain: "multimodal", capability: "alignment", status: "failed", failure_code: "provider_error" },
    run_id_digest: "9".repeat(64),
    attempt: 1,
  });
  assert.equal(result.handoff.domain, "multimodal");
  assert.equal(result.handoff.status, "queued");
  assert.equal(facade.planRecovery({ domain: "multimodal", capability: "alignment", status: "completed" }).next_action, "complete");
});
