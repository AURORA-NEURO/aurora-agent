import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  LLMRuntime,
  auditAutonomousDomainContracts,
  builtinAutonomousDomainProfiles,
  validateAutonomousDomainAuditReport,
} from "../dist/index.js";

test("the high-level brain facade exposes the same provider-free domain audit", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { calls += 1; throw new Error("domain audit must not contact a provider"); } });
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(runtime) });
  const report = await brain.domainAudit();

  assert.equal(report.summary.domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.summary.static_contract_status, "valid");
  assert.equal(calls, 0);
});

test("domain audit validates every built-in profile, workflow graph, tool contract, and evidence contract", async () => {
  const report = await auditAutonomousDomainContracts();

  assert.equal(report.summary.domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.summary.valid_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.summary.invalid_domain_count, 0);
  assert.equal(report.summary.static_contract_status, "valid");
  assert.equal(report.summary.runtime_status, "unassessed");
  assert.equal(report.summary.runtime_unassessed_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.rows.every((row) => row.contract_status === "valid"), true);
  assert.equal(report.rows.every((row) => row.stage_count >= 4), true);
  assert.equal(report.rows.every((row) => row.evidence_surface.requirement_count > 0), true);
  assert.equal(report.rows.every((row) => /^[0-9a-f]{64}$/.test(row.row_digest)), true);
  assert.equal(JSON.stringify(report).includes("unit-test-only-not-a-provider-key"), false);
  assert.deepEqual(await validateAutonomousDomainAuditReport(report), report);
});

test("domain audit projects complete caller-owned tool and evidence surfaces without dispatch", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const tools = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const evidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const report = await auditAutonomousDomainContracts({ availableToolNames: tools, availableEvidence: evidence });

  assert.equal(report.summary.static_contract_status, "valid");
  assert.equal(report.summary.runtime_status, "ready_for_review");
  assert.equal(report.summary.runtime_ready_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.summary.runtime_partial_domain_count, 0);
  assert.equal(report.summary.missing_tool_count, 0);
  assert.equal(report.rows.every((row) => row.tool_surface.assessed && row.tool_surface.missing_tool_names.length === 0), true);
  assert.equal(report.rows.every((row) => row.evidence_surface.assessed && row.evidence_surface.coverage_status === "complete"), true);
  assert.equal(report.rows.every((row) => row.evidence_surface.coverage_ratio === 1), true);
});

test("domain audit reports a malformed profile contract instead of treating it as ready", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const damaged = structuredClone(profiles);
  const coding = damaged.find((profile) => profile.domain === "coding");
  coding.capabilities = coding.capabilities.filter((capability) => capability !== coding.default_capability);
  const report = await auditAutonomousDomainContracts({ profiles: damaged });
  const row = report.rows.find((candidate) => candidate.domain === "coding");

  assert.equal(row.contract_status, "invalid");
  assert.equal(row.runtime_status, "blocked");
  assert.ok(row.issues.some((issue) => issue.code === "default_capability_unlisted" && issue.severity === "blocking"));
  assert.equal(report.summary.static_contract_status, "invalid");
  assert.equal(report.summary.runtime_blocked_domain_count, 1);
});

test("domain audit is digest-bound and rejects tampered row or report metadata", async () => {
  const report = await auditAutonomousDomainContracts();
  const rowTampered = structuredClone(report);
  rowTampered.rows[0].stage_count += 1;
  await assert.rejects(() => validateAutonomousDomainAuditReport(rowTampered), /row digest/);

  const reportTampered = structuredClone(report);
  reportTampered.next_actions = [...reportTampered.next_actions, "tampered"];
  await assert.rejects(() => validateAutonomousDomainAuditReport(reportTampered), /report digest/);
});
