# Execution provenance audit

execution_provenance_audit is the bounded handoff between an executed agent_mission report and
the delegated checks an operator wants to carry with it. It is deliberately an audit projection,
not a second executor.

The request contains:

- mission: the exact returned mission report, including its embedded plan, step results, and
  clock-free execution_trace;
- delegated_checks: optional named evidence rows with kind, requiredness, outcome, result_digest,
  source provenance, and an optional trace sequence reference.

The Rust kernel verifies:

1. the plan has a valid content digest and the report is internally identified;
2. each planned step has at most one result, the result tool matches the plan, and result statuses
   are from the bounded mission vocabulary;
3. trace sequence numbers are contiguous and unique, lifecycle boundaries are present, every step
   event names a planned step, and trace tool identity agrees with the plan;
4. delegated checks have unique names, valid content digests, bounded outcomes, and trace
   references that point into the supplied trace;
5. the returned trace and combined provenance input receive recomputed digests.

valid and structurally_valid describe structural reconciliation only. provenance_ready and
release_candidate require a complete trace, an executed/succeeded mission, no required step
failure, and passing required delegated checks. They do not prove that the mission was actually
run by this route, that a provider authenticated its result, or that deployment, security,
scientific, clinical, or release authority exists.

The projection is available through MCP as execution_provenance_audit, Python as
ExecutionProvenanceRequest / ExecutionProvenanceReport on Workspace, AsyncWorkspace, ApiClient,
and AsyncApiClient, and TypeScript as ApiClient.executionProvenanceAudit. All surfaces preserve
missingness, identity findings, digests, and the structural-only execution/verification labels.

This boundary is useful when a mission combines discovery, evidence, analysis, safety, or
delivery tools: one downstream consumer can inspect the mission trace and delegated checks without
mistaking a green nested result for an unqualified system-level approval.
