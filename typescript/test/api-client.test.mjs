import assert from "node:assert/strict";
import test from "node:test";
import {
  ApiClient,
  ApiError,
  ArgumentError,
  MissionWaitTimeoutError,
  assertMissionPreflight,
  MissionPreflightError,
  ResponseTooLargeError,
  ToolCatalogue,
  ToolSchemaError,
  ToolRefusalError,
  parseSse,
} from "../dist/index.js";

function jsonResponse(value, status = 200, headers = {}) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

test("client exposes typed discovery, tool calls, and refusal preservation", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    bearerToken: "0123456789abcdef",
    fetch: async (input, init) => {
      seen.push({ input: String(input), init });
      const path = new URL(String(input)).pathname;
      if (path === "/v1/tools") return jsonResponse({ tools: [{ name: "echo", description: "test", inputSchema: { type: "object", required: ["value"], properties: { value: { type: "integer" }, mode: { type: "string", enum: ["safe", "fast"] } } } }] });
      if (path === "/v1/tools/echo") return jsonResponse({ ok: true, tool: "echo", request_id: "r1", mcp: { result: { structuredContent: { value: 3 } } }, guarantee: "shared" });
      if (path === "/v1/tools/metrics_analytics_audit") return jsonResponse({ ok: true, tool: "metrics_analytics_audit", request_id: "r3", mcp: { result: { structuredContent: { workflow: "metrics_descriptive_analytics" } } } });
      if (path === "/v1/tools/biocapability_evidence_audit") return jsonResponse({ ok: true, tool: "biocapability_evidence_audit", request_id: "r16", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "biocapability_evidence_conditioned_profile",
        metrics: { ok: true, coverage: { measured: 1 } },
        metrics_ok: true,
        evidence: {
          items: [{ index: 0, ok: true, id: "evidence-1", dimension: "evidence_grounding", domain: "oncology", declared_status: "observed", effective_status: "observed", issues: [], support: { source: "ledger" }, fail_closed: false }],
          omitted_items: 0,
          item_count: 1,
          invalid_item_count: 0,
          dimensions: [{ dimension: "evidence_grounding", state: "observed", evidence_count: 1, measured_count: 1, declared_count: 0, blocked_count: 0, missing: false, measured: true }],
          domains: { oncology: 1 },
        },
        claim_requests: {
          rows: [{ index: 0, ok: true, id: "claim-1", claim: "grounded profile", requires: ["temporal_validity"], allow_declared: false, eligible: false, blockers: [{ dimension: "temporal_validity", state: "missing" }], explicit_assumptions: [], fail_closed: true }],
          omitted_rows: 0,
          requested: 1,
          eligible: 0,
          all_requested_claims_eligible: false,
        },
        subaudits: { information_value: null, reference_quality: null, temporal_validity: null, reproducibility: null },
        release_posture: { ready_for_requested_claims: false, requires_explicit_claim_request: false, numeric_scores_are_not_claims_without_evidence: true, declared_evidence_is_visible_but_not_measured_support: true },
        guarantees: ["declared evidence is not measured support"],
        limitations: ["no external dataset was inspected"],
      } } } });
      if (path === "/v1/tools/bioatlas_publication_audit") return jsonResponse({ ok: true, tool: "bioatlas_publication_audit", request_id: "r17", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioatlas-publication-audit/0.1",
        workflow: "bioatlas_publication_audit",
        atlas: { ok: true, summary: { coverage_supports_aggregation: true } },
        evidence_audit: null,
        card: null,
        leaderboard: null,
        release_request: {
          present: true,
          id: "publication-1",
          targets: [{ target: "atlas_profile", eligible: true, blockers: [], notes: [] }],
          ready: true,
          fail_closed: false,
          no_implicit_release: true,
        },
        cross_layer: {
          numeric_score_requires_evidence_audit: true,
          numeric_score_evidence_ready: false,
          atlas_aggregation_ready: true,
          leaderboard_ranked_count: 3,
          leaderboard_unranked_count: 1,
          unranked_leaderboard_entries_remain_visible: true,
          withheld_scores_are_not_zeroes: true,
        },
        guarantees: ["publication targets are explicit"],
        limitations: ["no network publisher"],
      } } } });
      if (path === "/v1/tools/developer_workbench") return jsonResponse({ ok: true, tool: "developer_workbench", request_id: "r4", mcp: { result: { structuredContent: { workflow: "developer_workbench", audit: { valid: true } } } } });
      if (path === "/v1/tools/ci_execution_evidence_audit") return jsonResponse({ ok: true, tool: "ci_execution_evidence_audit", request_id: "r27", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "ci_execution_evidence_audit",
        schema: "bioprism-devplat-ci-execution-evidence/0.1",
        valid: true,
        ci_evidence_ready: true,
        plan_digest: "p".repeat(64),
        evidence_digest: "e".repeat(64),
        audit: {
          schema: "bioprism-devplat-ci-execution-evidence/0.1",
          workflow: "contracts",
          plan_digest: "p".repeat(64),
          evidence_digest: "e".repeat(64),
          run_id: "run-42",
          provider: "github_actions",
          source: "provider_observed",
          conclusion: "success",
          expected_check_count: 1,
          observed_check_count: 1,
          passed_check_count: 1,
          failed_check_count: 0,
          skipped_check_count: 0,
          unknown_check_count: 0,
          required_missing: [],
          required_failed: [],
          optional_nonpassing: [],
          complete: true,
          structurally_valid: true,
          release_candidate: true,
          execution: "evidence_supplied_not_executed_here",
          verification: "structural_only",
          findings: [],
          guarantees: [],
          limitations: [],
        },
        guarantees: [],
        limitations: [],
      } } } });
      if (path === "/v1/tools/developer_platform_status") return jsonResponse({ ok: true, tool: "developer_platform_status", request_id: "r16", mcp: { result: { structuredContent: {
        ok: true,
        root: "workspace",
        detail_mode: "summary",
        max_items: 3,
        devplat: {
          digest: "d".repeat(64),
          verdict_counts: [1, 1, 1, 1],
          modules_classified: 4,
          implemented_count: 1,
          not_implemented_count: 3,
          foreign_subject_count: 1,
          walkthrough_count: 0,
          guarded_claims: 0,
          unguarded_claims: 0,
        },
        walkthroughs: [],
        cookbook: { recipes: 0, anti_recipes: 0, crates: [], enforcing_tests: 0, quotes: 0, verification: { clean: true, crates_checked: 0, entry_points_checked: 0, tests_checked: 0, quotes_checked: 0, defect_count: 0, defects_returned: [], omitted_defects: 0 } },
        developer_contract: { surface_count: 0, surfaces_returned: [], omitted_surfaces: 0 },
        diagnostic_catalogue: { clean: true, checked: 0, errors: 0, warnings: 0, finding_count: 0, findings_returned: [], omitted_findings: 0 },
        exit_code_audit: { clean: true, retry_decision_recoverable_from_code_alone: true, divergence_count: 0, divergences_returned: [], omitted_divergences: 0 },
        limitations: ["foreign artifacts remain explicit"],
      } } } });
      if (path === "/v1/tools/token_context_plan") return jsonResponse({ ok: true, tool: "token_context_plan", request_id: "r17", mcp: { result: { structuredContent: {
        ok: true,
        plan: {
          request_digest: "a".repeat(64),
          plan_digest: "b".repeat(64),
          candidates: ["invariant/identity"],
          mandatory: ["invariant/identity"],
          handles: [],
          mandatory_estimate: { tokens: 20, method: { method: "declared_by_caller" } },
          optional_estimate: { tokens: 0, method: { method: "declared_by_caller" } },
          envelope: { total: 100 },
        },
        comparison: null,
        guarantees: ["mandatory closure is checked before a plan is returned"],
      } } } });
      if (path === "/v1/tools/weavelang_compile") return jsonResponse({ ok: true, tool: "weavelang_compile", request_id: "r18", mcp: { result: { structuredContent: {
        ok: true,
        program: {
          program_id: "urn:weave:program:demo@sha256:" + "p".repeat(64),
          digest: "d".repeat(64),
          semantic_digest: "s".repeat(64),
          weave_ir_version: "0.1.0",
          roles: 2,
          participants: 2,
          interfaces: 1,
          policies: 1,
          state_nodes: 3,
          transitions: 2,
          monitors: 0,
          initial_state: "start",
          terminal_states: ["done"],
        },
        execution: {
          status: "not_requested",
          mode: "replay",
          state: "start",
          liveness: { messages_left_unconsumed: 0, commitments_left_open: [], states_without_exit: [], unreachable_states: [], deadlock_freedom_proven: false },
          invariant_violations: [],
        },
        ir: null,
        guarantees: ["execution is a local semantic trace; it performs no network, model, or tool call"],
      } } } });
      if (path === "/v1/tools/epistemic_voi") return jsonResponse({ ok: true, tool: "epistemic_voi", request_id: "r19", mcp: { result: { structuredContent: {
        ok: true,
        mode: "single",
        value: { gross: 4, cost: 0.1, net: 3.9, outcome_probabilities: [0.5, 0.5], action_without: 0, action_after: [0, 1] },
        actions: { without: "treat", after: ["treat", "abstain"] },
        complementarity: null,
        guarantees: ["gross risk reduction and declared acquisition cost remain separate"],
      } } } });
      if (path === "/v1/tools/benchmark_trace_analyze") return jsonResponse({ ok: true, tool: "benchmark_trace_analyze", request_id: "r20", mcp: { result: { structuredContent: {
        ok: true,
        trace_id: "failed-run",
        succeeded: false,
        event_count: 3,
        reference_trace_id: "reference-run",
        analysis: { trace_id: "failed-run", textual: { kind: "diverged", failing_step: 1, passing_step: 1, common_prefix: 1, failing_did: "choice route", passing_did: "choice safe", visibility_gap: [] }, textual_is_actionable: true, reference: "reference-run", terminal_step: 2, ancestry: [1], candidates: [], verdict: { verdict: "first_causal", step: 1, score: 0.7 } },
        episodes: [],
        boundaries: [],
        repetitions: [],
        summary: { episode_count: 0, boundary_count: 0, extractable_boundaries: 0, repetition_groups: 0 },
        guarantees: ["causal ranking remains separate"],
      } } } });
      if (path === "/v1/tools/benchmark_decision_audit") return jsonResponse({ ok: true, tool: "benchmark_decision_audit", request_id: "r20b", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-decision-audit/0.1",
        trace_id: "failed-run",
        trace_digest: "a".repeat(64),
        analysis: { trace_id: "failed-run", verdict: { verdict: "first_causal", step: 1, score: 0.8 }, ancestry: [1], candidates: [] },
        decision: {
          selected_step: 1,
          causal_step: 1,
          causal_alignment: "aligned",
          event_kind: "choice",
          coverage: { total: 3, visible_at_decision_time: 2, validation_only: 1, feasible: 2, strong: 1, plausible_wrong_alternatives: 1, adequate: true },
          action_counts: { all: 3, visible_to_agent: 2, validation_only: 1, acceptable: 2 },
          actions: [], visible_to_agent: [], validation_only: [], acceptable: [],
          omitted: { all: 3, visible_to_agent: 2, validation_only: 1, acceptable: 2 },
        },
        failure_card: { trace_id: "failed-run", terminal_step: 2, blame: { blame: "agent", at_step: 1 }, recommended_cell_steps: [1], findings: [], hypotheses: [], violated_constraints: [], alternative_explanations: [], missing_evidence: [], evidence_ratio: 1 },
        failure_card_omitted: {},
        guarantees: ["future options are validation-only"],
      } } } });
      if (path === "/v1/tools/benchmark_integrity_audit") return jsonResponse({ ok: true, tool: "benchmark_integrity_audit", request_id: "r20c", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-integrity-audit/0.1",
        instance_digest: "a".repeat(64),
        counts: { instances: 3, panel_runs: 2, bench_instances: 3, known_instances: 3, safety_vetoes: 1 },
        dedup: { examined: 3, distinct: 2, groups: [], groups_omitted: 0, removed: ["duplicate"], removed_omitted: 0, caveat: "no semantic similarity" },
        holdout: { private_share: 20, rotating_panels: 0, counts: { private: 1, public: 2 }, rows: [], omitted: 3 },
        contamination: { counts: { clean: 1, unassessed: 1, leaks_through_channel: 1 }, admissible: 1, inadmissible: 2, rows: [], omitted: 3 },
        calibration: { discriminating: 1, trivial_cue: 0, universally_passed: 0, universally_failed: 0, unmeasured: 2, safety_vetoes: 0, instances: [], omitted: 3 },
        effective_diversity: { instances: 3, parents: 2, families: 2, signatures: 2, equivalence_classes: 2, inflation_ratio: 1.5, caveat: "independent classes" },
        guarantees: ["unmeasured is not zero"],
      } } } });
      if (path === "/v1/tools/benchmark_counterfactual_check") return jsonResponse({ ok: true, tool: "benchmark_counterfactual_check", request_id: "r20d", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-counterfactual/0.1",
        pair: { differing_fields: ["query"], realism_reviewed: false },
        outcome: { outcome: "as_predicted" },
        satisfied: true,
        source_verdict: "pass",
        followup_verdict: "pass",
        cell_digests: { source: "a".repeat(64), followup: "b".repeat(64) },
        allowed_cell_fields: ["world", "query", "acceptable_verdicts", "required_witnesses", "require_protected_closure"],
        guarantees: ["one factor"],
        limitations: ["no realism validator"],
      } } } });
      if (path === "/v1/tools/benchmark_oracle_review") return jsonResponse({ ok: true, tool: "benchmark_oracle_review", request_id: "r20e", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-oracle-review/0.1",
        proposal: { oracle_id: "oracle-demo", strength: "exact_state_predicate", acceptable_verdicts: ["pass"] },
        reviewed_oracle: { inner: { oracle_id: "oracle-demo" }, reviewer: "reviewer-1", review_digest: "c".repeat(64) },
        reviewer: "reviewer-1",
        review_digest: "c".repeat(64),
        strength: "exact_state_predicate",
        deterministic: true,
        grade: { acceptance: { outcome: "passed" }, passed: true },
        cell: { cell_id: "cell-reviewed", acceptable_verdicts: ["pass"], required_witnesses: ["evidence"] },
        synthesis_order: ["exact_state_predicate", "execution_test", "property_relation", "trajectory_constraint", "statistical_tolerance", "model_judge"],
        guarantees: ["only the kernel review gate creates a ReviewedOracle"],
        limitations: ["declarative contract"],
      } } } });
      if (path === "/v1/tools/benchmark_compile") return jsonResponse({ ok: true, tool: "benchmark_compile", request_id: "r20f", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-compile/0.1",
        trace_id: "run_fail",
        compilation: { trace_id: "run_fail", class: { class: "candidate_research_cell" } },
        class: { class: "candidate_research_cell" },
        cell_step: 3,
        episodes: 1,
        boundary_count: 2,
        oracle: { oracle_id: "oracle-run-fail", strength: "exact_state_predicate" },
        minimization: { minimal: ["panel_manifest"], reduction_ratio: 0.5 },
        confidence: { boundary_detection: { state: "measured", value: 0.8 } },
        limiting_stage: ["boundary_detection", 0.8],
        unmeasured_stages: ["state_reconstruction", "oracle_adequacy", "mutation_validity"],
        probe: { provided_rows: 4, evaluations: 8, execution: "caller-supplied observation table; no world or architecture was run" },
        guarantees: ["no execution"],
        limitations: ["no mutation generation"],
      } } } });
      if (path === "/v1/tools/benchmark_compile_review") return jsonResponse({ ok: true, tool: "benchmark_compile_review", request_id: "r20g", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/benchmark-compile-review/0.1",
        compile: { trace_id: "run_fail", class: { class: "candidate_research_cell" } },
        reviewed_oracle: { inner: { oracle_id: "oracle-run-fail" }, reviewer: "reviewer-1", review_digest: "d".repeat(64) },
        reviewer: "reviewer-1",
        review_digest: "d".repeat(64),
        grade: { acceptance: { outcome: "passed" }, passed: true },
        cell: { cell_id: "dc_run_fail#step3", acceptable_verdicts: ["invalid"], required_witnesses: ["identity_leakage"] },
        guarantees: ["reviewed before packaging"],
        limitations: ["no execution"],
      } } } });
      if (path === "/v1/tools/pack_coverage_audit") return jsonResponse({ ok: true, tool: "pack_coverage_audit", request_id: "r20h", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/pack-coverage-audit/0.1",
        section: "15",
        selected_pack_count: 1,
        selected_pack_ids: ["prism.context-acquisition"],
        summary: { families: 14, covered: 10, uncovered: 4, singly_covered: 3, weakly_covered: 2, coverage_fraction: 0.714, gap_summary: "10 of 14 capability families have at least one pack" },
        rows: [{ family: "evidence_acquisition", code: "B1", packs: ["prism.context-acquisition"], grounded: true }],
        rows_omitted: 13,
        uncovered: ["tool_use"],
        uncovered_omitted: 3,
        singly_covered: ["verification"],
        singly_covered_omitted: 2,
        weakly_covered: ["planning"],
        weakly_covered_omitted: 1,
        matrix: [{ family: "evidence_acquisition", domain: "coding", packs: ["prism.context-acquisition"] }],
        matrix_omitted: 4,
        guarantees: ["gaps remain visible"],
        limitations: ["declaration level"],
      } } } });
      if (path === "/v1/tools/pack_release_audit") return jsonResponse({ ok: true, tool: "pack_release_audit", request_id: "r20i", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/pack-release-audit/0.1",
        section: "15",
        selected_pack_count: 1,
        selected_pack_ids: ["prism.context-acquisition"],
        sequenced_count: 1,
        unsequenced_count: 0,
        release_coverage_fraction: 1,
        wave_counts: { "1": 1 },
        axis_counts: { mechanism: 1 },
        release_order: [{ selected_position: 1, portfolio_position: 1, id: "prism.context-acquisition" }],
        release_order_omitted: 0,
        unsequenced: [],
        unsequenced_omitted: 0,
        guarantees: ["unsequenced packs remain explicit"],
        limitations: ["not an approval"],
      } } } });
      if (path === "/v1/tools/foundation_contract_check") return jsonResponse({ ok: true, tool: "foundation_contract_check", request_id: "r21", mcp: { result: { structuredContent: {
        ok: true,
        verdict: "refused",
        contract: { ok: true, id: "fbc:test:001", intent: "check", falsifier_count: 1, action_count: 1, evidence_obligation_count: 0, minimum_reviewers: 0, uncertainty_required: true },
        parent_relation: null,
        envelope: null,
        world: { ok: false, world_id: "observed-world", class: "observed_replay", counterfactual_strength: "low", reveal_policy: "admissible", claim: "unsupported", fail_closed: true },
        transition: { ok: false, verdict: "plane_confusion", refusal: "latent state", fail_closed: true },
        guarantees: ["contract gates remain separate"],
      } } } });
      if (path === "/v1/tools/pack_catalogue") return jsonResponse({ ok: true, tool: "pack_catalogue", request_id: "r22", mcp: { result: { structuredContent: {
        ok: true,
        section: "15",
        portfolio_count: 46,
        section_counts: { "15": 25, "29": 21 },
        returned: [{ id: "prism.context-acquisition", title: "Context", blueprint_module: "15.01", axis: "mechanism", measures: "evidence", capabilities: ["A00"], domains: ["coding"], decision_families: ["choose"], oracles: ["deterministic"], strongest_oracle: "deterministic", has_execution_grounded_oracle: true, release_wave: { wave: 1 }, capability_signature: "Mechanism|A00|coding" }],
        omitted: 24,
        duplicate_signature_groups: [{ signature: "Domain|B5|biomedical research", pack_ids: ["a", "b"] }],
        guarantees: ["catalogue rows are declarations"],
      } } } });
      if (path === "/v1/tools/pack_health_assess") return jsonResponse({ ok: true, tool: "pack_health_assess", request_id: "r23", mcp: { result: { structuredContent: {
        ok: true,
        pack: "demo.pack",
        pack_digest: "a".repeat(64),
        verdict: "unreportable",
        finding_count: 1,
        blocking_findings: 1,
        advisory_findings: 0,
        health: { pack: "demo.pack", pack_digest: "a".repeat(64), findings: [{ finding: "saturated", pooled_pass_rate: 0.99, systems: 3 }] },
        calibration: { observations: [{ system: "system-a", trials: 100, passes: 99 }] },
        score_gate: { reportable: false, refusal: "pack is saturated", fail_closed: true, score: null },
        guarantees: ["declarations, observed outcomes, oracle posture, and reportability remain separate"],
      } } } });
      if (path === "/v1/tools/security_redteam_simulate") return jsonResponse({ ok: true, tool: "security_redteam_simulate", request_id: "r24", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "section_13_redteam_incident_evidence",
        input_counts: { findings: 1, vulnerabilities: 0, deliveries: 0, incidents: 0, audit_records: 0, attestations: 0 },
        findings: [{ index: 0, ok: true, finding: { id: "F-confirmed", campaign: "sandbox", boundary: "agent_sandbox", class: "sandbox_bypass", status: "confirmed" }, regression_gate: { eligible: true, cell: { finding: "F-confirmed" }, public_summary: "F-confirmed against agent_sandbox" } }],
        findings_omitted: 0,
        regression_corpus: { sentinel_count: 1, covered_boundaries: ["agent_sandbox"], unminimised_count: 0, uncovered_boundaries: [], cells: [], omitted_cells: 0 },
        vulnerabilities: [],
        vulnerabilities_omitted: 0,
        boundary: { model: "evaluation_model", within_trial_agent_to_evaluator: [], within_trial_evaluator_to_agent: [], all_scope_agent_to_evaluator: [], feedback_loops: [], delivery_rows: [], delivery_rows_omitted: 0, allowed_delivery_count: 0, refused_delivery_count: 0 },
        incidents: [],
        incidents_omitted: 0,
        audit: { rows: [], rows_omitted: 0, chain_length: 0, head: null, verified: true, verification_refusal: null, assertion_count: 0, public_view_count: 0, records: [] },
        attestations: [],
        attestations_omitted: 0,
        guarantees: ["only confirmed findings can become regression cells"],
        limitations: ["this endpoint replays typed contracts; it does not run fuzzers"],
      } } } });
      if (path === "/v1/tools/security_program_audit") return jsonResponse({ ok: true, tool: "security_program_audit", request_id: "r26", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "security_program_audit",
        schema: "bioprism-security-program-audit/0.1",
        manifest_digest: "a".repeat(64),
        valid: true,
        security_program_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-security-program-audit/0.1",
          manifest_schema: "bioprism-security-program/0.1",
          digest: "a".repeat(64),
          valid: true,
          system_id: "aurora-security",
          counts: { scopes: 1, authorized_scopes: 1, campaigns: 1, completed_campaigns: 1, findings: 1, high_or_worse_findings: 1, actionable_findings: 0, remediations: 1, completed_remediations: 1, incidents: 1, open_incidents: 0, closed_incidents: 1, disclosures: 1, advisory_disclosures: 1, public_disclosures: 0, enabled_controls: 8 },
          scope_audits: [{ scope_id: "api-staging", authorization_valid: true, methods_valid: true, guardrails_valid: true, environments_valid: true, ready: true }],
          campaign_audits: [{ campaign_id: "campaign-1", scope_valid: true, operator_present: true, independent_review_valid: true, methodology_valid: true, evidence_valid: true, complete: true, ready: true }],
          finding_audits: [{ finding_id: "finding-1", campaign_valid: true, evidence_valid: true, reproduction_valid: true, severity_requires_action: true, remediation_valid: true, incident_required: true, incident_valid: true, regression_present: true, ready: true }],
          remediation_audits: [{ remediation_id: "remediation-1", finding_valid: true, owner_valid: true, completion_valid: true, verification_valid: true, ready: true }],
          incident_audits: [{ incident_id: "incident-1", finding_valid: true, timeline_valid: true, containment_valid: true, closure_valid: true, notification_valid: true, ready: true }],
          disclosure_audits: [{ disclosure_id: "advisory-1", finding_valid: true, stage_order_valid: true, approval_valid: true, advisory_valid: true, publication_valid: true, ready: true }],
          control_audits: [{ control: "independent_review", enabled: true, required: true, ready: true }],
          issues: [],
          guarantees: ["program layers remain separate"],
          limitations: ["declaration only"],
        },
        guarantees: ["program layers remain separate"],
        limitations: ["declaration only"],
      } } } });
      if (path === "/v1/tools/factory_lifecycle_simulate") return jsonResponse({ ok: true, tool: "factory_lifecycle_simulate", request_id: "r25", mcp: { result: { structuredContent: {
        ok: true,
        action_count: 3,
        action_failures: 0,
        trace: [
          { index: 0, kind: "lease", ok: true, result: { job_id: "job-1", worker_id: "worker-1", attempt: 1, granted_at: { nanos: 0 }, expires_at: { nanos: 30 }, last_heartbeat: { nanos: 0 } } },
          { index: 1, kind: "stage", ok: true, result: { job_id: "job-1", visible_before_commit: false } },
          { index: 2, kind: "commit", ok: true, result: { job_id: "job-1", committed: true } },
        ],
        jobs: [{ id: "job-1", job: { id: "job-1", resource_class: "compile", idempotency: "idempotent", state: "succeeded", attempts: 1 }, committed_result: { digest: "out-1" } }],
        quarantined: [],
        dead_lettered: [],
        counts_by_class: { compile: 1 },
        guarantees: ["the simulation delegates every lifecycle transition to the typed in-memory JobStore"],
      } } } });
      if (path === "/v1/tools/storage_lifecycle_simulate") return jsonResponse({ ok: true, tool: "storage_lifecycle_simulate", request_id: "r26", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/storage-lifecycle/0.1",
        max_items: 100,
        now: 20,
        tiering: {
          policy: { demote_to_warm_after: 5, demote_to_cold_after: 12, promote_after_accesses: 3, promote_within: 2 },
          plan: { now: 20, transitions: [{ object: "pinned-hot", from: "Hot", to: "Warm", reason: { HeldByPin: { epochs: 20 } }, skipped_a_tier: false }] },
          transition_count: 1,
          bytes_by_target: [{ tier: "Warm", name: "warm", bytes: 200 }],
          apply_requested: false,
          apply_report: null,
          records: [{ object: "pinned-hot", tier: "Hot", last_access: 0, recent_accesses: 0, bytes: 200, pinned: true }],
          omitted_records: 0,
          input_rows: [{ index: 0, ok: true, object: "pinned-hot" }],
          omitted_input_rows: 0,
        },
        quota: {
          limit: 1000,
          reserve: 100,
          used: 0,
          remaining: 1000,
          remaining_for_ingest: 900,
          remaining_for_evidence_finalization: 1000,
          remaining_for_cleanup: 1000,
          classes: [
            { class: "Objects", name: "objects", reconstructible: false, charged: 0 },
            { class: "Events", name: "events", reconstructible: false, charged: 0 },
            { class: "Indexes", name: "indexes", reconstructible: true, charged: 0 },
            { class: "Results", name: "results", reconstructible: false, charged: 0 },
            { class: "Cache", name: "cache", reconstructible: true, charged: 0 },
          ],
          charges: [], omitted_charges: 0, releases: [], omitted_releases: 0, delegations: [], omitted_delegations: 0,
          absorptions: [], omitted_absorptions: 0, remaining_children: [], omitted_children: 0,
        },
        guarantees: ["tiering is planned against a caller-supplied logical epoch, so the same records and policy replay to the same transitions"],
        limitations: ["this is a deterministic in-memory lifecycle projection; it does not move bytes, run a scheduler, or persist an audit event"],
      } } } });
      if (path === "/v1/tools/registry_lifecycle_simulate") return jsonResponse({ ok: true, tool: "registry_lifecycle_simulate", request_id: "r27", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/registry-lifecycle/0.1",
        policy: { minimum_tier: "unranked" },
        packs: [{ index: 0, valid: false, refusal: "invalid attested pack", fail_closed: true }],
        initial_integrity: { artifact_count: 0, log_count: 0, broken_count: 0, broken: [], operations_allowed: true },
        actions: [
          { index: 0, op: "publish", ok: false, refusal: "pack 0 is unavailable", fail_closed: true },
          { index: 1, op: "resolve", ok: true, result: { name: "missing@0.1.0", found: false, digest: null, core_digest: null } },
          { index: 2, op: "verify_all", ok: true, result: { clean: true, broken_count: 0, broken: [] } },
        ],
        final: { artifact_count: 0, log_count: 0, broken_count: 0, integrity_clean: true, verification: [], log: [] },
        registry: { artifacts: {}, core_digests: {}, tiers: {}, statuses: {}, names: {}, latest_artifact: {}, log: [] },
        guarantees: ["failed actions are typed refusals and do not abort independent later actions"],
        limitations: ["this is a local deterministic registry projection; it does not provide network transport, signatures, federation, moderation, quarantine, or authentication"],
      } } } });
      if (path === "/v1/tools/cache_invalidation_simulate") return jsonResponse({ ok: true, tool: "cache_invalidation_simulate", request_id: "r28", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/cache-invalidation/0.1",
        max_items: 100,
        key_schema: { name: "decision-cache", components: ["code", "input"], reuse: "SameBuildOnly" },
        entries: { accepted: 1, submitted: 1, rows: [{ index: 0, ok: true, digest: "d-unproven", dependencies: "Undeclared" }], omitted_rows: 0 },
        graph: { known_resources: ["input"], known_resource_count: 1, opaque_resources: ["input"], cycle: null, cycle_is_a_scheduler_defect_not_an_invalidation_hang: false },
        invalidation: {
          changed: "input",
          plan: { changed: "input", affected_resources: ["input"], invalid_entries: [], proved_unaffected: [], completeness: { Partial: { opaque_resources: [], unknown_resources: [], entries_without_declared_dependencies: ["d-unproven"], entries_depending_on_opaque_resources: [] } }, population: 1 },
          apply_requested: true,
          apply_report: { removed: [], marked_unproven: ["d-unproven"], left_proven: [], invalidation_was_complete: false },
        },
        lookups: { pre_apply: [{ index: 0, ok: true, hit: true, value: { answer: "legacy" } }], post_apply: [{ index: 0, ok: true, hit: false, miss_reason: { UnprovenAfterPartialInvalidation: { since: 2, cause: "partial" } } }], omitted_post_apply: 0 },
        reprove: [],
        cache: { entry_count: 1, unproven: ["d-unproven"], hits: 1, misses_by_reason: [{ reason: "unproven", count: 1 }], hit_rate: 0.5, entries: [], omitted_entries: 1 },
        guarantees: ["cache keys are rebuilt from every declared component and never from a bare digest", "partial invalidation marks unknown entries unproven rather than serving them optimistically", "re-proving names the digest and build that re-established currentness"],
        limitations: ["the cache and dependency graph are in-memory projections; no durable index, tenant isolation, eviction worker, or external invalidation feed is created"],
      } } } });
      if (path === "/v1/tools/hub_disclosure_review") return jsonResponse({ ok: true, tool: "hub_disclosure_review", request_id: "r29", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/hub-disclosure/0.1",
        action_count: 4,
        action_failures: 0,
        trace: [
          { index: 0, kind: "declare_held_out", ok: true, result: { pack: "pack-1", state: { disclosure: "held_out" } } },
          { index: 1, kind: "disclose", ok: true, result: { pack: "pack-1", state: { disclosure: "disclosed", since: 5 } } },
          { index: 2, kind: "headline_eligibility", ok: true, result: { pack: "pack-1", eligible: false, refusal: "disclosure is not acknowledged", fail_closed: true } },
          { index: 3, kind: "headline_eligibility", ok: true, result: { pack: "pack-1", eligible: true, label: { label: "disclosed_pack", disclosed_at: 5, caveat: "visible benchmark" } } },
        ],
        entries: [{ pack: "pack-1", state: { disclosure: "disclosed", since: 5 } }],
        ledger: { packs: { "pack-1": { disclosure: "disclosed", since: 5 } } },
        guarantees: ["disclosure is keyed by immutable pack digest rather than a mutable name", "disclosure is a ratchet and contamination cannot be walked back", "headline eligibility returns a caveat or a typed refusal instead of a bare score", "the review does not detect leaks"],
      } } } });
      if (path === "/v1/tools/hub_submission_review") return jsonResponse({ ok: true, tool: "hub_submission_review", request_id: "r32", mcp: { result: { structuredContent: {
        ok: true, schema: "bioprism-mcp/hub-submission/0.1", stage: "moderation_ledger",
        submission: { id: "sub-1", content: "digest-1" }, limitation_card: "bounded", state: "accepted", verification: "reproduced", published: ["sub-1"], event_count: 1,
        ledger: { records: {}, events: [{ submission: "sub-1", kind: "opened", actor: "hub", at: 1, reason: null, superseded_by: null }], last_epoch: 1 },
        guarantees: ["moderation is an append-only in-memory state machine with monotonic epochs", "self-review and self-asserted verification are refused"],
      } } } });
      if (path === "/v1/tools/hub_card_render") return jsonResponse({ ok: true, tool: "hub_card_render", request_id: "r30", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/hub-card/0.1",
        card: {
          resource_type: "bioatlas-card", resource_id: "digest-card", version: "bioatlas-card/0.1", submission: "sub-card",
          scope: { decision_family: ["ranking"] }, provenance: ["digest-parent"], access: "public", state: "available", verification: "self-reported",
          score: { display: "published", score: { value: 0.82, interval: null }, label: { label: "held_out" } }, non_claims: [], attributions: [], limitations: "bounded",
        },
        score: { attached: true, pack: "pack-1", computed_at: 4 },
        moderation_state: "accepted", verification: "self-reported",
        guarantees: ["a card starts with a withheld score and never uses zero or blank as a failure state", "scores require disclosure eligibility and an available publication state", "the result is a renderer-facing object; it does not render HTML, resolve links, or publish a page"],
      } } } });
      if (path === "/v1/tools/hub_leaderboard_render") return jsonResponse({ ok: true, tool: "hub_leaderboard_render", request_id: "r31", mcp: { result: { structuredContent: {
        ok: true, schema: "bioprism-mcp/hub-leaderboard/0.1", board: "board-1", ranked_count: 1, unranked_count: 1, leader_count: 1,
        headline: "Rank 1 under conditions; no clinical validity.", rendered: null,
        guarantees: ["evidence scale and disclosure eligibility are checked before an entry is rankable"],
      } } } });
      if (path === "/v1/tools/developer_delivery_audit") return jsonResponse({ ok: true, tool: "developer_delivery_audit", request_id: "r15", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "developer_delivery_audit",
        platform: {},
        repository: {},
        repository_impact: null,
        sdk: {},
        conformance: {},
        provider: {},
        governance: {},
        release: {},
        ci_evidence: { ci_evidence_ready: true },
        readiness: {
          platform_checks_clean: true,
          unguarded_claims: 0,
          developer_claims_ready: true,
          repository_scope_clean: true,
          repository_impact_clean: false,
          sdk_admission_clean: true,
          conformance_release: true,
          provider_capability_gate_cleared: true,
          governance_document_clean: true,
          release_audit_ready: true,
          ci_execution_evidence_ready: true,
          local_delivery_ready: true,
        },
        external_surface_posture: {
          foreign_subject_count: 2,
          foreign_artifacts_present: true,
          foreign_artifacts_are_not_inferred: true,
          local_integration_foundations: [{ artifact: "prism_sdk", kind: "client" }],
          unverified_surface_families: ["typescript_sdk"],
        },
        release_request: {
          present: true,
          id: "delivery-1",
          targets: [{ target: "ci_execution_evidence", available: true, eligible: true, blockers: [], notes: [] }],
          ready: true,
          fail_closed: false,
          no_implicit_release: true,
          available_target_count: 11,
        },
        guarantees: ["no implicit release"],
        limitations: ["external execution remains outside the workflow"],
      } } } });
      if (path === "/v1/tools/repository_catalog") return jsonResponse({ ok: true, tool: "repository_catalog", request_id: "r11", mcp: { result: { structuredContent: { workflow: "repository_catalog", prefix: "docs/" } } } });
      if (path === "/v1/tools/repository_bundle") return jsonResponse({ ok: true, tool: "repository_bundle", request_id: "r12", mcp: { result: { structuredContent: { workflow: "repository_bundle", policy: "exhaustive" } } } });
      if (path === "/v1/tools/repository_impact") return jsonResponse({ ok: true, tool: "repository_impact", request_id: "r13", mcp: { result: { structuredContent: { workflow: "repository_impact", changed: "docs/README" } } } });
      if (path === "/v1/tools/telemetry_project") return jsonResponse({ ok: true, tool: "telemetry_project", request_id: "r14", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/telemetry-projection/0.1",
        event_id: "evt-ts",
        event_kind: "tool.completed",
        trace: "trace-ts",
        policy_version: "telemetry-v1",
        record: { event_id: "evt-ts", kind: "tool.completed", trace: "trace-ts", attributes: { status: "ok" }, epoch: 7, policy: "telemetry-v1" },
        loss: { dropped: [], coarsened: [] },
        lossless: true,
        metric: null,
        guarantees: ["telemetry is a one-way projection of the canonical DomainEvent"],
      } } } });
      if (path === "/v1/tools/ledger_ingest") return jsonResponse({ ok: true, tool: "ledger_ingest", request_id: "r15", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/ledger-ingest/0.1",
        entries: 2,
        next_seq: 2,
        head: "entry-digest",
        admissions: { recorded: 2, duplicates: 1, quarantined: 1, released: 1, receipts: null },
        chain: { status: "intact" },
        clock_anomalies: [{ seq: 1, previous_record: "2025-01-01T00:00:00Z", record: "2024-01-01T00:00:00Z" }],
        quarantine: { count: 0, items: [], omitted: 0 },
        class_counts: { material: 2 },
        latest_by_subject: { count: 1, items: [{ subject: "patient-7/specimen-1", event: "evt-1", seq: 1, valid: "2025-01-01T00:00:00Z", payload_digest: "payload-digest" }], omitted: 0 },
        cut: { requested: { as_of_record: "2024-06-01T00:00:00Z" }, count: 1, entries: [{ seq: 0, id: "evt-0", class: "material", kind: "specimen.collected", subject: "patient-7/specimen-1", valid: "2025-01-01T00:00:00Z", record: "2025-01-01T00:00:00Z", release: "2025-01-01T00:00:00Z" }], omitted: 0 },
        guarantees: ["payload bodies are not returned by default; projections carry digests rather than copied payloads", "no durable storage, clock reading, network, or external side effect occurs"],
      } } } });
      if (path === "/v1/tools/quality_gate_run") return jsonResponse({ ok: true, tool: "quality_gate_run", request_id: "r15b", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/quality-gate/0.1",
        verdict: "failed",
        passed: false,
        dataset: "release-quality",
        rows: 2,
        check_count: 3,
        report: {
          gate: "release-gate",
          dataset: "release-quality",
          rows: 2,
          outcomes: {
            age_range: { Pass: { examined: 2 } },
            subject_unique: { Fail: { witness: { row: 1, column: "subject", found: "s-1", expected: "a value not already seen at row 0" } } },
            foreign_site: { NotRunnable: { reason: { MissingReferenceSet: { reference: "sites" } } } },
          },
          verdict: { Failed: { failing: ["subject_unique"], not_runnable: ["foreign_site"] } },
        },
        guarantees: ["pass requires every named check to run and hold", "failed checks carry a concrete row and expected value witness"],
      } } } });
      if (path === "/v1/tools/atlas_report") return jsonResponse({ ok: true, tool: "atlas_report", request_id: "r15c", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/atlas-report/0.1",
        ontology_version: "atlas-test/1",
        summary: { measured: 1, holes: 2, families: 3, inconsistencies: 0, coverage_debt_ratio: 0.667, has_holes: true, coverage_supports_aggregation: false },
        debt: { total_capabilities: 3, measured: 1, unmeasured: 2, closed_by_declaration: 0, dark_families: ["tool_use"], unclassified_failures: 0, undiagnosed_failures: 0 },
        measured: [{ capability: "measured", family: "verification", score: 1, depth: "single", evaluable: 1, excluded: 0, effective_size: 1, generated_instances: 0, permitted_claim: "unit_conformance" }],
        omitted_measured: 0,
        holes: [{ capability: "unmeasured", family: "tool_use", reason: "not_attempted", influence: "unknown", aggregate: false, blocks_claims_for: ["agent"] }, { capability: "agent", family: "domain_reasoning", reason: "not_attempted", influence: "unknown", aggregate: true, blocks_claims_for: [] }],
        omitted_holes: 0,
        family_coverage: [{ family: "domain_reasoning", total: 1, measured: 0, holes: 1 }, { family: "tool_use", total: 1, measured: 0, holes: 1 }, { family: "verification", total: 1, measured: 1, holes: 0 }],
        omitted_families: 0,
        depth_histogram: [{ depth: "single", count: 1 }],
        stage_histogram: [],
        inconsistencies: [],
        omitted_inconsistencies: 0,
        composite: { ok: false, refusal: "unmeasured capability", fail_closed: true },
        guarantees: ["unmeasured capabilities remain holes and are never rendered as zero"],
        limitations: ["the atlas indexes caller-supplied evidence; it does not run trials"],
      } } } });
      if (path === "/v1/tools/atlas_surface_audit") return jsonResponse({ ok: true, tool: "atlas_surface_audit", request_id: "r15surface", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/atlas-surface-audit/0.1",
        workflow: "atlas_surface_audit",
        coverage: {
          subject: "surface-system", total_capabilities: 3, measured: 1, unmeasured: 2,
          blocking: 2, closed_by_declaration: 0, vacuous: false,
          holes: [{ capability: "causal.interpretation", reason: "not_attempted", blocks_claim: true }],
          omitted_holes: 1, profile_coverage: { outcome: "answered", cell: { kind: "share", value: { numerator: 1, denominator: 3 } } },
        },
        debt_discharge: { any_evidence: true, measured: { rows: ["cohort.statistics"], total: 1, omitted: 0 } },
        failure_browse: {
          subject: "surface-system", facet: "mechanism", taxonomy_version: "atlasx-test/1",
          records_browsed: 2, visible: 1, withheld: 1, contested: 0, undiagnosed: 0,
          evaluator_induced: 0, distinct_families: 1, shares_sum_to_one: true,
          buckets: [{ label: "mechanism:stale_evidence_trusted", member_count: 1 }], omitted_buckets: 0,
        },
        rate_checks: { rows: [{ capability: "identity.lineage", answered: true }], total: 1 },
        surface_audits: { sound: true }, policies: { require_sound_surfaces: true },
        guarantees: ["holes are not zeroes"], limitations: ["caller-supplied records"],
      } } } });
      if (path === "/v1/tools/engineering_manifest_audit") return jsonResponse({ ok: true, tool: "engineering_manifest_audit", request_id: "r15engineering", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-engineering-audit/0.1",
        workflow: "engineering_manifest_audit",
        manifest_digest: "a".repeat(64),
        valid: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-engineering-audit/0.1",
          manifest_schema: "bioprism-engineering-manifest/0.1",
          digest: "a".repeat(64),
          valid: true,
          counts: { packages: 2, public_packages: 1, tickets: 2, completed_tickets: 1, actionable_tickets: 1, adrs: 1, accepted_adrs: 1, ownership_rows: 1 },
          package_order: ["core", "api"],
          cyclic_packages: [],
          ticket_readiness: [{ ticket_id: "T-002", status: "planned", state: "actionable", blocking_dependencies: [], dependency_ready: true }],
          adr_supersession: [], ownership_surfaces: ["api"], issues: [], guarantees: ["edges checked"], limitations: ["artifact only"],
        },
        guarantees: ["edges checked"], limitations: ["artifact only"],
      } } } });
      if (path === "/v1/tools/engineering_execution_plan") return jsonResponse({ ok: true, tool: "engineering_execution_plan", request_id: "r15engineeringplan", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-engineering-plan-audit/0.1",
        workflow: "engineering_execution_plan",
        request_digest: "b".repeat(64),
        manifest_digest: "a".repeat(64),
        plan_digest: "c".repeat(64),
        valid: true,
        engineering_plan_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-engineering-plan-audit/0.1",
          valid: true,
          planning_started: true,
          truncated: false,
          ticket_count: 3,
          planned_ticket_count: 2,
          omitted_ticket_count: 0,
          package_order: ["core", "api"],
          ticket_plans: [
            { ticket_id: "T-002", package: "api", contract: "api-contract", status: "planned", state: "ready", dependency_ids: ["T-001"], blocking_dependencies: [], dependency_ready: true, scheduled: true, wave: 0, critical_path_length: 2 },
            { ticket_id: "T-003", package: "api", contract: "docs-contract", status: "planned", state: "ready", dependency_ids: ["T-002"], blocking_dependencies: [], dependency_ready: true, scheduled: true, wave: 1, critical_path_length: 1 },
          ],
          waves: [
            { index: 0, ticket_ids: ["T-002"], package_ids: ["api"], depends_on_waves: [], parallelism: 1 },
            { index: 1, ticket_ids: ["T-003"], package_ids: ["api"], depends_on_waves: [0], parallelism: 1 },
          ],
          critical_path: ["T-001", "T-002", "T-003"],
          gates: [{ name: "manifest_admission", passed: true, required: true, detail: "valid" }],
          manifest_issues: [], issues: [], guarantees: ["deterministic ordering"], limitations: ["artifact only"],
        },
        guarantees: ["deterministic ordering"], limitations: ["artifact only"],
      } } } });
      if (path === "/v1/tools/release_pipeline_audit") return jsonResponse({ ok: true, tool: "release_pipeline_audit", request_id: "r15releasepipeline", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-release-pipeline-audit/0.1",
        workflow: "release_pipeline_audit",
        manifest_digest: "a".repeat(64),
        valid: true,
        release_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-release-pipeline-audit/0.1",
          manifest_schema: "bioprism-release-pipeline/0.1",
          digest: "a".repeat(64),
          valid: true,
          counts: { environments: 2, protected_environments: 2, stages: 2, required_stages: 2, artifacts: 1, attestations: 3, promotions: 2, production_promotions: 1 },
          stage_order: ["build", "test"],
          cyclic_stages: [],
          stage_readiness: [{ stage_id: "build", state: "ready_to_schedule", dependency_ready: true, blocking_dependencies: [] }],
          artifact_audits: [{ artifact_id: "binary", digest_valid: true, producer_valid: true, inputs_valid: true, attestations_valid: true, provenance_present: true, signature_present: true }],
          promotion_audits: [{ promotion_id: "to-production", from: "staging", to: "production", valid: true, production: true, missing_attestations: [], missing_approvals: [], rollback_present: true }],
          issues: [], guarantees: ["layers remain separate"], limitations: ["artifact only"],
        },
        guarantees: ["layers remain separate"], limitations: ["artifact only"],
      } } } });
      if (path === "/v1/tools/security_privacy_audit") return jsonResponse({ ok: true, tool: "security_privacy_audit", request_id: "r15securityprivacy", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-security-privacy-audit/0.1",
        workflow: "security_privacy_audit",
        manifest_digest: "a".repeat(64),
        valid: true,
        security_privacy_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-security-privacy-audit/0.1",
          manifest_schema: "bioprism-security-privacy/0.1",
          digest: "a".repeat(64),
          valid: true,
          system_id: "aurora-api",
          counts: { assets: 1, sensitive_assets: 1, flows: 1, allowed_flows: 1, identities: 1, hardened_identities: 1, threats: 1, high_or_worse_threats: 1, treated_threats: 1, reviews: 1, current_reviews: 1, controls: 10, enabled_controls: 10 },
          asset_audits: [{ asset_id: "patient-records", purpose_valid: true, retention_valid: true, residency_valid: true, deletion_valid: true, sensitive: true, ready: true }],
          flow_audits: [{ flow_id: "api-to-vendor", asset_valid: true, purpose_valid: true, legal_basis_present: true, authorization_present: true, allowed: true, ready: true }],
          identity_audits: [{ identity_id: "researcher", assets_valid: true, authentication_valid: true, mfa: true, least_privilege: true, sensitive_access: true, ready: true }],
          threat_audits: [{ threat_id: "exfiltration", high_or_worse: true, treated: true, control_present: true, evidence_valid: true, rationale_present: false, ready: true }],
          review_audits: [{ review_id: "pia-1", reviewer_independent: true, evidence_valid: true, current: true, complete: true, ready: true }],
          control_audits: [{ control: "encryption_at_rest", enabled: true, required: true, ready: true }],
          issues: [], guarantees: ["layers remain separate"], limitations: ["artifact only"],
        },
        guarantees: ["layers remain separate"], limitations: ["artifact only"],
      } } } });
      if (path === "/v1/tools/sandbox_admission_audit") return jsonResponse({ ok: true, tool: "sandbox_admission_audit", request_id: "r15sandbox", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-sandbox-audit/0.1",
        workflow: "sandbox_admission_audit",
        manifest_digest: "a".repeat(64),
        valid: true,
        sandbox_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-sandbox-audit/0.1",
          manifest_schema: "bioprism-sandbox/0.1",
          digest: "a".repeat(64),
          valid: true,
          system_id: "prism-sandbox",
          counts: { artifacts: 2, untrusted_artifacts: 1, profiles: 1, isolated_profiles: 1, capabilities: 1, approved_capabilities: 1, dangerous_capabilities: 1, outputs: 1, quarantined_outputs: 1, released_outputs: 0 },
          artifact_audits: [{ artifact_id: "dataset", digest_valid: true, lineage_valid: true, source_valid: true, trust: "untrusted", hardening_required: true, ready: true }],
          profile_audits: [{ profile_id: "profile", artifact_valid: true, isolation_valid: true, network_valid: true, mounts_valid: true, capabilities_valid: true, resources_valid: true, output_valid: true, ready: true }],
          capability_audits: [{ capability_id: "network", profile_valid: true, target_valid: true, approved: true, dangerous: true, evidence_valid: true, ready: true }],
          boundary_audits: [{ profile_id: "profile", default_deny: true, network_mode: "allowlist", allowlist_valid: true, host_paths_rejected: true, dangerous_capabilities: 1, ready: true }],
          resource_audits: [{ profile_id: "profile", cpu_bounded: true, memory_bounded: true, wall_time_bounded: true, processes_bounded: true, output_bounded: true, ready: true }],
          output_audits: [{ output_id: "result", profile_valid: true, artifact_valid: true, digest_valid: true, lineage_valid: true, quarantined: true, review_valid: true, release_valid: true, ready: true }],
          issues: [], guarantees: ["layers remain separate"], limitations: ["admission only"],
        },
        guarantees: ["layers remain separate"], limitations: ["admission only"],
      } } } });
      if (path === "/v1/tools/sandbox_runtime_simulate") return jsonResponse({ ok: true, tool: "sandbox_runtime_simulate", request_id: "r15runtime", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-sandbox-runtime-audit/0.1",
        workflow: "sandbox_runtime_simulate",
        manifest_digest: "a".repeat(64),
        admission_digest: "b".repeat(64),
        trace_digest: "c".repeat(64),
        valid: true,
        sandbox_runtime_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-sandbox-runtime-audit/0.1",
          manifest_schema: "bioprism-sandbox-runtime/0.1",
          admission_digest: "b".repeat(64),
          trace_digest: "c".repeat(64),
          valid: true,
          profile_id: "profile",
          admission_valid: true,
          simulation_started: true,
          completed: true,
          stopped_on_refusal: false,
          request_count: 2,
          simulated_count: 2,
          refused_count: 0,
          not_run_count: 0,
          usage: { cpu_millis: 200, memory_mb_peak: 128, wall_time_seconds: 10, processes_peak: 1, output_bytes: 2000 },
          steps: [{ request_id: "read-input", kind: "filesystem_read", target: "/inputs/data", capability_id: "read", capability_valid: true, target_valid: true, resource_valid: true, decision: "simulated", charged: true, usage_after: { cpu_millis: 100, memory_mb_peak: 128, wall_time_seconds: 5, processes_peak: 1, output_bytes: 1000 }, refusal: null }],
          admission_issues: [],
          issues: [],
          guarantees: ["decisions remain traceable"],
          limitations: ["simulation only"],
        },
        guarantees: ["decisions remain traceable"],
        limitations: ["simulation only"],
      } } } });
      if (path === "/v1/tools/operational_readiness_audit") return jsonResponse({ ok: true, tool: "operational_readiness_audit", request_id: "r15operational", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-operational-readiness-audit/0.1",
        workflow: "operational_readiness_audit",
        manifest_digest: "a".repeat(64),
        valid: true,
        operationally_ready: true,
        blocking_issue_count: 0,
        warning_count: 0,
        audit: {
          schema: "bioprism-operational-readiness-audit/0.1",
          manifest_schema: "bioprism-operational-readiness/0.1",
          digest: "a".repeat(64),
          valid: true,
          service_id: "aurora-api",
          counts: { contracts: 1, required_contracts: 1, indicators: 1, observed_indicators: 1, dependencies: 1, critical_dependencies: 1, runbooks: 1, incidents: 1, open_incidents: 0, controls: 7, enabled_controls: 7 },
          indicator_audits: [{ indicator_id: "availability", contract_valid: true, source_valid: true, observed: true, evidence_valid: true, ready: true }],
          dependency_audits: [{ dependency_id: "registry", owner_valid: true, failure_mode_valid: true, fallback_present: true, critical: true, ready: true }],
          runbook_audits: [{ runbook_id: "api-degraded", valid: true, review_current: true, step_count: 2, referenced_incidents: 1 }],
          incident_audits: [{ incident_id: "INC-1", valid: true, runbook_valid: true, timeline_present: true, postmortem_present: true, closed: true }],
          control_audits: [{ control: "on_call", enabled: true, required: true, ready: true }],
          issues: [], guarantees: ["layers remain separate"], limitations: ["artifact only"],
        },
        guarantees: ["layers remain separate"], limitations: ["artifact only"],
      } } } });
      if (path === "/v1/tools/adaptive_panel") return jsonResponse({ ok: true, tool: "adaptive_panel", request_id: "r15d", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/adaptive-panel/0.1",
        audit: { trials: 0, scored_trials: 0, abstentions: 0, total_cost: 0, capabilities: [], caveat: "clustered evidence caveat" },
        audit_summary: { trials: 0, scored_trials: 0, abstentions: 0, total_cost: 0, capabilities: 0, reported: 0, withheld: 0, effective_trials: 0, headline: "empty" },
        audit_digest: null,
        selection: { ok: true, value: { mode: "next", record: { chosen: { instance: "inst-1", capability: "capability-a", parent: "parent-1", score: 0.5, expected_variance_reduction: 0.1, independence_weight: 1, cost: 1, parent_trials_before: 0 }, eligible: 1, already_run: 0, coverage_gated_out: 0, gated_by: null, runners_up: [], icc_used: 0.5, icc_source: "assumed", caveat: "greedy" } } },
        capability: { capability: "capability-a", coverage: { capability: "capability-a", trials: 0, parents: 0, qualifying_parents: 0, abstentions: 0, shortfalls: [{ kind: "trials", have: 0, need: 30 }] }, stopping: null, stopping_refusal: "no recorded trials", estimate: null, estimate_refusal: "no recorded trials", fail_closed: true },
        comparison: null,
        finished: false,
        finished_refusal: null,
        guarantees: ["abstentions are retained and costed but never counted as failures"],
        limitations: ["selection never executes candidates"],
      } } } });
      if (path === "/v1/tools/posterior_gate") return jsonResponse({ ok: true, tool: "posterior_gate", request_id: "r15e", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/posterior-gate/0.1",
        schema_version: "07.0.1",
        observations: 4,
        unprovenanced_observations: 2,
        capabilities: {
          "capability-a": {
            capability: "capability-a",
            pass_rate: { label: "pass", mean: 0.75, naive_instance_mean: 0.75, instances: 2, clusters: 2, largest_cluster: 1, icc: { icc: "not_applicable" }, effective_sample_size: 2, unknown_instances: 0, unknown_fraction: 0 },
            credit: { label: "credit", mean: 0.75, naive_instance_mean: 0.75, instances: 2, clusters: 2, largest_cluster: 1, icc: { icc: "not_applicable" }, effective_sample_size: 2, unknown_instances: 0, unknown_fraction: 0 },
            outcome_rate: { label: "outcome", mean: 0.9, naive_instance_mean: 0.9, instances: 2, clusters: 2, largest_cluster: 1, icc: { icc: "not_applicable" }, effective_sample_size: 2, unknown_instances: 0, unknown_fraction: 0 },
            vetoes: [], disputed: 1, abstained: 0, optimistic_weak_evidence: 1, weakest_tier: "execution",
          },
        },
        gate: { ok: true, value: { gate: "release-a", value: 0.75, formula: "weighted mean", rationale: "named release decision", terms: [["capability-a", 0.75, 1]], sensitivity: [["capability-a", 0.75]], weakest_tier: "execution", min_effective_sample: 2 } },
        comparison: { ok: true, dominance: { dominance: "incomparable", better: ["capability-a"], worse: [], uncertain: ["capability-b"] }, tolerance: 0.01, min_effective: 2 },
        guarantees: ["vectors remain separate from release scalars"],
        limitations: ["point estimates are not posterior distributions"],
      } } } });
      if (path === "/v1/tools/trace_otel_ingest") return jsonResponse({ ok: true, tool: "trace_otel_ingest", request_id: "r16", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/trace-otel-ingest/0.1",
        trace_id: "otel-ts",
        event_count: 1,
        succeeded: false,
        trace_sha256: "a".repeat(64),
        valid: true,
        validation_error: null,
        mapping: { format: "otlp_json", resource_count: 1, scope_count: 1, source_span_count: 1, accepted_span_count: 1, span_event_count: 0 },
        loss: { dropped_spans: [], dropped_span_events: [], unmapped_fields: [], duplicate_attributes: [], inferred_kinds: [], missing_start_times: [], unresolved_parents: [], multiple_trace_ids: [] },
        lossless: true,
        dropped_events: 0,
        compilable: true,
        events_included: true,
        events: [{ step: 0, kind: "goal", payload: { name: "agent.goal" }, visible: [] }],
        omitted_events: 0,
        guarantees: ["source spans are retained"],
        limitations: ["this is not an OTLP exporter"],
      } } } });
      if (path === "/v1/tools/capability_discover") return jsonResponse({ ok: true, tool: "capability_discover", request_id: "r6", mcp: { result: { structuredContent: { workflow: "capability_discover", capability_schema_version: "bioprism-devplat-capability/0.1", schema_version: "bioprism-devplat-capability/0.1", catalog_digest: "c".repeat(64), total_groups: 1, query: {}, result_count: 1, matches: [{ group: { id: "testing", domains: ["verification"], crates: ["bioprism-devplat"], mcp_tools: ["echo"], cli_entrypoints: ["bioprism test"], python_artifacts: ["prism_sdk.testing"], status: "implemented" }, score: 100, matched_fields: ["domains"], matched_tools: ["echo"], tool_schemas: [] }], schema_attachment: { requested: false, returned: 0, missing: [] } } } } });
      if (path === "/v1/tools/capability_audit") return jsonResponse({ ok: true, tool: "capability_audit", request_id: "r7", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "capability_audit",
        capability_schema_version: "bioprism-devplat-capability/0.1",
        catalog_digest: "c".repeat(64),
        healthy: true,
        total_groups: 1,
        catalog_tool_memberships: 1,
        unique_catalog_tools: 1,
        advertised_tool_count: 1,
        catalog_only_tools: [],
        advertised_only_tools: [],
        duplicate_schema_names: [],
        duplicate_group_memberships: [],
        schema_quality: { checked: 1, valid: 1, total_bytes: 128, maximum_schema_bytes: 1000000, findings: [] },
        invariants: {
          every_catalog_tool_has_authoritative_schema: true,
          every_advertised_tool_is_catalogued: true,
          schema_names_are_unique: true,
          all_input_schemas_are_well_formed: true,
          multi_group_membership_is_allowed: true,
        },
        groups: [{ id: "testing", domains: ["verification"], status: "implemented", declared_tool_memberships: 1, unique_tools: 1, schemas_found: 1, missing_schemas: [] }],
      } } } });
      if (path === "/v1/tools/capability_dashboard") return jsonResponse({ ok: true, tool: "capability_dashboard", request_id: "r7dashboard", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "capability_dashboard",
        schema: "bioprism-devplat-capability-dashboard/0.1",
        catalog_digest: "c".repeat(64),
        dashboard_digest: "d".repeat(64),
        capability_dashboard_ready: true,
        duplicate_schema_names: [],
        audit: {
          schema: "bioprism-devplat-capability-dashboard/0.1",
          catalog_digest: "c".repeat(64),
          dashboard_digest: "d".repeat(64),
          query: { domain: "verification", max_groups: 128, include_tools: true, include_gaps: true },
          total_group_count: 1, selected_group_count: 1, available_group_count: 1,
          callable_group_count: 1, partial_group_count: 0, declared_only_group_count: 0,
          selected_tool_memberships: 1, selected_unique_tools: 1, schema_backed_unique_tools: 1,
          readiness_counts: { callable: 1 }, gap_counts: {},
          groups: [{ id: "testing", domains: ["verification"], status: "implemented", readiness: "callable", surfaces: { crates: 1, mcp_tools: 1, cli_entrypoints: 1, python_artifacts: 1 }, tool_count: 1, callable_tool_count: 1, schema_backed_tool_count: 1, missing_transport_schemas: [], invalid_transport_schemas: [], tools: ["echo"], gaps: [] }],
          warnings: [], guarantees: [], limitations: [], ready: true,
        },
      } } } });
      if (path === "/v1/tools/capability_route") return jsonResponse({ ok: true, tool: "capability_route", request_id: "r8", mcp: { result: { structuredContent: { workflow: "capability_route", execution: "not_started", route_coverage: { needs_total: 1, needs_resolved: 1, needs_unresolved: 0, candidate_group_count: 1, candidate_groups: ["testing"], candidate_domain_count: 1, candidate_domains: ["verification"], candidate_tool_count: 1, posture: "routing evidence only" } } } } });
      if (path === "/v1/tools/capability_route_review") return jsonResponse({ ok: true, tool: "capability_route_review", request_id: "r9", mcp: { result: { structuredContent: { workflow: "capability_route_review", review_id: "v".repeat(64), review_status: "ready", handoff_status: "mission_preflight_required", execution: "not_started", findings: [], dependency_waves: [["oncology"]], schema_review: { requested: true, checked: 1, valid: true, fully_checked: true } } } } });
      if (path === "/v1/tools/adapter_plan") return jsonResponse({ ok: true, tool: "adapter_plan", request_id: "r10", mcp: { result: { structuredContent: {
        ok: true,
        workflow: "adapter_plan",
        plan_id: "p".repeat(64),
        registry: "bioprism-adapter-registry/0.1",
        executable: true,
        selected_adapter: { id: "bioprism.tabular", execution: "native", version: "0.1.0", conformance_level: "normalize", optional_dependency: null, declared_loss_kinds: ["precision_reduced"], scope_dimensions: ["subject"] },
        plan: {
          schema: "bioprism-adapter-registry/0.1",
          request: { source_id: "scan-1", source_kind: "bytes", declared_format: "application/dicom" },
          selected_adapter: { id: "bioprism.tabular", version: "0.1.0", execution: "native", accepted_formats: ["application/dicom"], accepts_undeclared_format: true, source_kinds: ["bytes"], conformance_level: "normalize", declared_loss_kinds: ["precision_reduced"], scope_dimensions: ["subject"], optional_dependency: null, description: "bounded tabular adapter" },
          executable: true,
          candidates: [{ adapter: { id: "bioprism.tabular", version: "0.1.0", execution: "native", accepted_formats: ["application/dicom"], accepts_undeclared_format: true, source_kinds: ["bytes"], conformance_level: "normalize", declared_loss_kinds: ["precision_reduced"], scope_dimensions: ["subject"], optional_dependency: null, description: "bounded tabular adapter" }, status: "ready", reasons: ["native adapter is available in this runtime"] }],
          limitations: ["source-specific conformance remains required"],
        },
        execution: "not_started",
        guarantees: ["format matching is explicit"],
        limitations: ["does not execute adapters"],
      } } } });
      if (path === "/v1/tools/tabular_ingest") return jsonResponse({ ok: true, tool: "tabular_ingest", request_id: "r11", mcp: { result: { structuredContent: {
        ok: true,
        source_id: "cohort.csv",
        fact_count: 1,
        ingestion_sha256: "sha256:ingestion",
        manifest: { source_id: "cohort.csv", declared_format: "text/csv", source_digest: "sha256:source", byte_length: 20, adapter: "bioprism.tabular", adapter_version: "0.1.0", profile_digest: "sha256:profile", provenance: { accession: "RG-DEMO-001" } },
        semantic_loss: { audit: "lossless", mapped: [{ source_id: "cohort.csv", column: "subject" }] },
        conformance: { report: { adapter: "bioprism.tabular", adapter_version: "0.1.0", source_id: "cohort.csv", checks: [{ check: "determinism", status: "pass", detail: "stable" }] }, passed: true, verified: true, summary: "verified" },
        max_items: 100,
        facts: [{ id: "fact-1", provides: "subject", value: "S1" }],
        omitted_facts: 0,
        limitations: ["source truth remains caller-owned"],
      } } } });
      if (path === "/v1/tools/conformance_run") return jsonResponse({ ok: true, tool: "conformance_run", request_id: "r12", mcp: { result: { structuredContent: {
        ok: true,
        suite: { id: "fiber-compiler-conformance", version: "0.1.0", digest: "d".repeat(64), fixture_manifest_id: "fixture-manifest-1", fixture_count: 1, synthetic_fixture_count: 0, case_count: 1, passed: 1, failed: 0, unsupported: 0, errored: 0, fixture_drift: [], pyramid: { counts: { unit: 1 } }, fully_conformant: true },
        release_decision: { decision: "release", suite_id: "fiber-compiler-conformance", suite_version: "0.1.0", suite_digest: "d".repeat(64), implementation: "reference 0.1.0", gates: ["no_fixture_drift"] },
        summary: "fiber-compiler-conformance 0.1.0 against reference 0.1.0: 1 passed, 0 failed, 0 unsupported, 0 errored",
        results: null,
        guarantees: ["fixture digests are verified"],
      } } } });
      if (path === "/v1/tools/release_audit") return jsonResponse({ ok: true, tool: "release_audit", request_id: "r13", mcp: { result: { structuredContent: {
        ok: true,
        release_ready: true,
        required_check_count: 1,
        check_count: 1,
        invocation_failures: 0,
        blocking_count: 0,
        blockers: [],
        checks: [{ index: 0, kind: "conformance_run", required: true, advisory: false, evaluated: true, gate: true, passed: true, result_digest: "r".repeat(64) }],
        guarantees: ["required checks are conjunctive"],
        limitations: ["local evidence only"],
      } } } });
      if (path === "/v1/tools/operations_catalog") return jsonResponse({ ok: true, tool: "operations_catalog", request_id: "r14", mcp: { result: { structuredContent: {
        ok: true,
        detail_mode: "summary",
        max_items: 2,
        topologies: { local: { deployment: "local", technologies: ["sqlite"], classes: [] }, team: { deployment: "team", technologies: ["postgresql"], classes: [] }, promise_parity: { compared: 5, holds: true, differences: [] }, technology_is_not_promise_parity: true },
        data_classes: [],
        deployment_planes: [],
        tenant_patterns: [],
        slo_objectives: ["api-read-availability"],
        service_contracts: { summary: { satisfied: 0, diverges: 9, not_implemented: 0, divergences: 59, total: 9 }, entries: [], entry_count: 9, omitted_entries: 9 },
        metrics: { metrics_schema_version: "bioprism-metrics/0.1", atlasx_schema_version: "bioprism-atlasx/0.1", named_in_scope: 118, named_but_undefined: 117, defined_here: [], undefined_metrics_returned: [], omitted_undefined_metrics: 117, undefined_is_not_zero: true },
        sdk: { registration_note: "registration", execution_and_isolation_are_not_implied: true },
        limitations: ["local only"],
      } } } });
      if (path === "/v1/tools/ops_acceptance") return jsonResponse({ ok: true, tool: "ops_acceptance", request_id: "r15", mcp: { result: { structuredContent: {
        ok: true,
        summary: { met: 0, refuted: 1, unverifiable: 2, total: 3, is_release_ready: false, is_decidable: false },
        findings: [],
        omitted_findings: 3,
        guarantees: ["unverifiable is not a pass"],
        limitations: ["no external CI"],
      } } } });
      if (path === "/v1/tools/safety_release_gate") return jsonResponse({ ok: true, tool: "safety_release_gate", request_id: "r18", mcp: { result: { structuredContent: {
        ok: true,
        subject: "pack/biological-design@1",
        category: "biological_design",
        decision: { decision: "cleared", subject: "pack/biological-design@1" },
        cleared: true,
        unrated_dimensions: [],
        high_risk_dimensions: [],
        rule: "zero high non-mitigating dimensions clears",
        fail_closed: true,
        limitations: [],
      } } } });
      if (path === "/v1/tools/medical_boundary_check") return jsonResponse({ ok: true, tool: "medical_boundary_check", request_id: "r19", mcp: { result: { structuredContent: {
        ok: false,
        admitted: false,
        refusal: "clinical output is not admitted",
        research_only_label: "research use only",
        boundary_is_unconditional: true,
        clinical_output_is_never_admitted: true,
      } } } });
      if (path === "/v1/tools/safety_posture") return jsonResponse({ ok: true, tool: "safety_posture", request_id: "r20", mcp: { result: { structuredContent: {
        ok: true,
        model: "section_13",
        adversaries: 9,
        threats: 25,
        coverage: { mitigated: 6, declared_only: 15, unmitigated: 4 },
        coverage_summary: "6 enforced, 15 declared-only, 4 unmitigated (of 25)",
        residual_threat_ids: ["T-13.26-dual-use"],
        unanalysed_threat_ids: [],
        unreachable_threat_ids: [],
        audit_acceptances: true,
        perimeter_controls_are_not_claimed_as_enforced: true,
      } } } });
      if (path === "/v1/tools/measurement_compare") return jsonResponse({ ok: true, tool: "measurement_compare", request_id: "r21", mcp: { result: { structuredContent: {
        ok: true,
        comparable: true,
        policy: { require_bound_terms: false },
        report: { left: "left", right: "right", verdict: { verdict: "comparable" }, conversions: [], caveats: [] },
        report_sha256: "a".repeat(64),
        guarantees: ["unit conversion is explicit"],
        limitations: ["caller-supplied declarations"],
      } } } });
      if (path === "/v1/tools/hub_search") return jsonResponse({ ok: true, tool: "hub_search", request_id: "r22", mcp: { result: { structuredContent: {
        ok: true,
        catalog_count: 1,
        release_count: 1,
        requested_limit: null,
        effective_limit: 100,
        matches: [{ name: "bioprism/onco", version: "1.0.0", digest: "sha256:onco", summary: "oncology reference pack", tier: "reviewed", authority: { authority: "authoritative", registry: "origin" }, freshness: { freshness: "authoritative" }, why: [{ why: "keyword_matched", keyword: "onco" }] }],
        match_count: 1,
        excluded: [],
        excluded_count: 0,
        omitted_excluded: 0,
        truncated: false,
        guarantees: ["every match carries its matching facets, authority, tier, digest, and freshness"],
        limitations: ["catalog contents are caller-supplied"],
      } } } });
      if (path === "/v1/tools/hub_resolve") return jsonResponse({ ok: true, tool: "hub_resolve", request_id: "r23", mcp: { result: { structuredContent: {
        ok: true,
        resolution: { subject: { name: "bioprism/root", version: "1.0.0", digest: "sha256:root" }, provenance: { authority: { authority: "authoritative", registry: "origin" }, freshness: { freshness: "authoritative" }, accepted_under: { require_authority: false, accept_undetermined: false, accept_beyond_bound: false, max_accepted_lag: null }, notes: [] } },
        answered_by: "origin",
        authoritative: true,
        catalog_count: 1,
        guarantees: ["federation is checked"],
        limitations: ["caller-supplied catalogs"],
      } } } });
      if (path === "/v1/tools/hub_lock") return jsonResponse({ ok: true, tool: "hub_lock", request_id: "r24", mcp: { result: { structuredContent: {
        ok: true,
        entry_count: 1,
        fully_authoritative: true,
        answering_registries: ["origin"],
        remarked_entry_count: 0,
        entries: [{ name: "bioprism/root", locked: { resolution: { subject: { name: "bioprism/root", version: "1.0.0", digest: "sha256:root" }, provenance: { authority: { authority: "authoritative", registry: "origin" }, freshness: { freshness: "authoritative" }, accepted_under: { require_authority: false, accept_undetermined: false, accept_beyond_bound: false, max_accepted_lag: null }, notes: [] } }, required_by: [{ on: "bioprism/root", req: { req: "any" }, source: { source: "root" } }] } }],
        omitted_entries: 0,
        max_items: 3,
        guarantees: ["transitive dependencies are fixed by a bounded deterministic fixpoint"],
      } } } });
      if (path === "/v1/tools/world_claim_check") return jsonResponse({ ok: true, tool: "world_claim_check", request_id: "r25", mcp: { result: { structuredContent: {
        ok: false,
        supported: false,
        claim: { kind: "biology", quantity: "tumour growth rate", counterfactual: null, population: null },
        refusal: "claim exceeds the mechanistic rung",
        provenance: { top: "mechanistic", stands_on: ["mechanistic"], assumptions: ["tumour growth rate"], unsupported_counterfactuals: [], selection: { selection: "undeclared" } },
        fail_closed: true,
      } } } });
      if (path === "/v1/tools/observed_world_declare") return jsonResponse({ ok: true, tool: "observed_world_declare", request_id: "r26", mcp: { result: { structuredContent: {
        ok: true,
        world: { id: "observed-demo", sources: [{ name: "cohort", version: "v1", access: { access: "controlled", policy: "reviewer-only" }, embedded: false }], design: { cohort_size: 2, strata: [{ name: "all", count: 2 }], selection: { selection: "consecutive", criterion: "all eligible participants" }, stands_for_population: "RG-DEMO population", unsupported_counterfactuals: [] }, outcome_labels: ["negative", "positive"] },
        provenance: { top: "observed", stands_on: ["observed"], assumptions: [], unsupported_counterfactuals: [], selection: { selection: "consecutive", criterion: "all eligible participants" } },
        world_id: "observed-demo",
        source_count: 1,
        controlled_sources: ["cohort"],
        outcome_label_count: 2,
        guarantees: ["pinned sources are retained"],
      } } } });
      if (path === "/v1/tools/lineage_audit") return jsonResponse({ ok: true, tool: "lineage_audit", request_id: "r27", mcp: { result: { structuredContent: { ok: true, clean: true, identity_complete: true, finding_count: 0 } } } });
      if (path === "/v1/tools/preanalytic_apply") return jsonResponse({ ok: true, tool: "preanalytic_apply", request_id: "r28", mcp: { result: { structuredContent: { ok: false, applied: false, refusal: "biology changed", fail_closed: true } } } });
      if (path === "/v1/tools/contradiction_review") return jsonResponse({ ok: true, tool: "contradiction_review", request_id: "r29", mcp: { result: { structuredContent: { ok: false, stage: "pose", refusal: "readings agree", fail_closed: true } } } });
      if (path === "/v1/tools/lab_plan") return jsonResponse({ ok: true, tool: "lab_plan", request_id: "r30", mcp: { result: { structuredContent: { ok: true, goal: "safe assay", should_escalate: true } } } });
      if (path === "/v1/tools/onco_boundary_check") return jsonResponse({ ok: true, tool: "onco_boundary_check", request_id: "r31", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/onco-boundary-check/0.1",
        outcome_kind: "disposition",
        disposition_kind: "release_partial",
        permitted: ["cohort_analysis", "method_development"],
        disposition: { disposition: "release_partial", released: ["cohort_analysis"], refused: ["treatment_recommendation"], escalation: { trigger: "individual_clinical_request", route: "treating_clinical_team" } },
        released: ["cohort_analysis"],
        refused: ["treatment_recommendation"],
        terminal_action: "escalate",
        escalation: { trigger: "individual_clinical_request", route: "treating_clinical_team" },
        escalation_present: true,
        escalation_trigger: "individual_clinical_request",
        escalation_route: "treating_clinical_team",
        requested_use_count: 2,
        released_count: 1,
        refused_count: 1,
        identifier_fields_present: false,
      } } } });
      if (path === "/v1/tools/onco_response_assess") return jsonResponse({ ok: true, tool: "onco_response_assess", request_id: "r32", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/onco-response-assess/0.1",
        outcome_kind: "assessment",
        call_kind: "not_evaluable",
        unconfirmed_reading: "progression",
        criterion: { id: "rano-hgg", version: "2010" },
        treatment: { modality: "radiotherapy" },
        criterion_recognises_post_treatment_change: true,
        post_treatment_window_days: 84,
        pseudoresponse_possible: false,
        measurement_error_fraction: 0.1,
        evidence_present: false,
        criterion_divergence_present: true,
        sensitivity_flips: false,
        hypothesis_non_identifiable: true,
        call_label: "not evaluable",
        withheld_progression: true,
        hypothesis_count: 2,
        evidence_requests: ["histopathology", "interval_follow_up"],
      } } } });
      if (path === "/v1/tools/onco_worldline_view") return jsonResponse({ ok: true, tool: "onco_worldline_view", request_id: "r33", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/onco-worldline-view/0.1",
        subject: "S-1",
        baseline: "baseline",
        timepoint_count: 1,
        biological_order: ["baseline"],
        record_order: ["baseline"],
        record_order_differs: false,
        clock_axes: ["acquired", "recorded", "released", "visible"],
        clock_order_guaranteed: true,
        baseline_biological_index: 0,
        baseline_record_index: 0,
        visibility_cutoff: "2026-01-02T00:00:00Z",
        visibility_filter_applied: true,
        visible_timepoints: ["baseline"],
        hidden_from_agent: [],
        visibility_partition: { cutoff: "2026-01-02T00:00:00Z", filter_applied: true, visible: ["baseline"], hidden: [], visible_count: 1, hidden_count: 0 },
        visible_count: 1,
        hidden_count: 0,
        timepoints: [{ label: "baseline", biological_index: 0, record_index: 0, clocks: { acquired: "2026-01-01T00:00:00Z", recorded: "2026-01-01T00:00:00Z", released: "2026-01-01T00:00:00Z", visible: "2026-01-01T00:00:00Z" }, days_from_baseline: 0, observation: {}, visibility_state: "visible", visible_at_cutoff: true }],
        guarantees: [],
        limitations: [],
      } } } });
      if (path === "/v1/tools/onco_classification_check") return jsonResponse({ ok: true, tool: "onco_classification_check", request_id: "r34", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/onco-classification-check/0.1",
        histology: "diffuse_glioma",
        resolution: { resolution: "unresolved", candidates: ["astrocytoma_idh_mutant", "oligodendroglioma_idh_mutant1p19q_codeleted"], obligations: [{ marker: "idh_mutation", role: "required", state: { unobserved: "not_collected" }, discriminates: 2 }] },
        resolution_kind: "unresolved",
        is_integrated: false,
        entity: null,
        obligations: [{ marker: "idh_mutation", role: "required", state: { unobserved: "not_collected" }, discriminates: 2 }],
        obligation_count: 1,
        panel_states: [{ marker: "idh_mutation", state: { unobserved: "not_collected" } }],
        panel_state_count: 1,
        observed_panel_state_count: 0,
        unobserved_panel_state_count: 1,
        guarantees: [],
        limitations: [],
      } } } });
      if (path === "/v1/tools/oncoworlds_identity_join") return jsonResponse({ ok: true, tool: "oncoworlds_identity_join", request_id: "r35", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/oncoworlds-identity-join/0.1",
        joinable: false,
        report: { left: "left", right: "right", unit: "specimen", verdict: { verdict: "declined", reason: { refusal: "no_identity_evidence" } } },
        verdict_kind: "declined",
        refusal_kind: "no_identity_evidence",
        bridge_declared: false,
        epoch_bridge: null,
        identity_evidence_present: false,
        identity_link_count: 0,
        bridge_warrant_present: false,
        checked_dimensions: ["participant_identity", "identity_evidence"],
        guarantees: [],
        limitations: [],
      } } } });
      if (path === "/v1/tools/onco_outcome_analyze") return jsonResponse({ ok: true, tool: "onco_outcome_analyze", request_id: "r36", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/onco-outcome-analyze/0.1",
        analysis: {
          subject: "P-1",
          estimand: { endpoint: "time_to_progression", population: "intention_to_treat", variable: "time from entry to progression", summary_measure: "median_time_to_event", intercurrent_event_strategies: [["death", "hypothetical"]], censoring_assumption: "noninformative_assumed" },
          at_risk_days: 10,
          immortal_time_days: 4,
          outcome: { outcome: "censored", lost_to_follow_up: null },
          bias_flags: ["left_truncation", "informative_loss_to_follow_up"],
        },
        outcome: { outcome: "censored", lost_to_follow_up: null },
        bias_flags: ["left_truncation", "informative_loss_to_follow_up"],
        bias_count: 2,
        informative_bias_count: 1,
        at_risk_days: 10,
        immortal_time_days: 4,
        left_truncated: true,
        event: false,
        censoring_reason: "lost_to_follow_up",
        censoring_informative: true,
        informative_bias_flags: ["informative_loss_to_follow_up"],
        guarantees: [],
        limitations: [],
      } } } });
      if (path === "/v1/tools/oracle_combine") return jsonResponse({ ok: true, tool: "oracle_combine", request_id: "r37", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/oracle-combine/0.1",
        subject: "s",
        at: "2026-01-01T00:00:00Z",
        status: "underdetermined",
        underdetermined: true,
        deciding_tier: "deterministic",
        judge_only: false,
        suppressed_override: true,
        acceptable: false,
        basis: { basis: "decided", tier: "deterministic" },
        confidence: { low: 1, high: 1 },
        establishes: ["artifact"],
        does_not_establish: ["biology"],
        contributing: [{ oracle: { id: "checksum:sha", version: { major: 1, minor: 0, patch: 0 } }, tier: "deterministic", declared_tier: "deterministic", position: "supported", confidence: 1, belief: null, establishes: ["artifact"], cannot_establish: ["biology"], findings: [], admissibility: { state: "admissible" }, rationale: "" }],
        omitted_contributing: 0,
        withheld: [], omitted_withheld: 0, inadmissible: [], omitted_inadmissible: 0,
        suppressed: [{ oracle: { id: "judge:review", version: { major: 1, minor: 0, patch: 0 } }, attempted_position: "contradicted", attempted_tier: "judge", attempted_confidence: 0.99, deciding_tier: "deterministic", deciding_positions: ["supported"], rule: "nondeterministic_over_grounded" }],
        omitted_suppressed: 0,
        disagreements: [{ tier: "deterministic", positions: { supported: [{ id: "checksum:sha", version: { major: 1, minor: 0, patch: 0 } }] }, source: { source: "genuine_ambiguity" }, would_be_settled_by: [{ settlement: "higher_tier_oracle", at_least: "deterministic" }], resolution: { resolution: "open" } }],
        omitted_disagreements: 0,
        guarantees: ["same-tier disagreement remains set-valued"], limitations: ["caller supplied"],
      } } } });
      if (path === "/v1/tools/oracle_reference_panel") return jsonResponse({ ok: true, tool: "oracle_reference_panel", request_id: "r38", mcp: { result: { structuredContent: { ok: true, rule_label: "majority", readers: 2, omitted_reads: 0 } } } });
      if (path === "/v1/tools/oracle_missingness") return jsonResponse({ ok: true, tool: "oracle_missingness", request_id: "r39", mcp: { result: { structuredContent: { ok: true, small_cell_floor: 5 } } } });
      if (path === "/v1/tools/bioeval_reference_audit") return jsonResponse({ ok: true, tool: "bioeval_reference_audit", request_id: "r40", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-reference-audit/0.1",
        reference: { standard: "distribution", mass: { progression: 0.6, stable: 0.4 }, dispersion: { kind: "mixed", aleatoric_fraction: 0.5 } },
        reference_kind: "distribution",
        can_certify_clean_pass: false,
        resolution: { resolution: "distributed", modal_mass: 0.6 },
        modal_state: "progression", modal_mass: 0.6, modal_confidence: 0.6, entropy_bits: 0.97,
        dispersion: "mixed", queried_state: "progression", queried_state_mass: 0.6,
        guarantees: ["mass normalized"], limitations: ["not a scorer"],
      } } } });
      if (path === "/v1/tools/evaluation_worldline_audit") return jsonResponse({ ok: true, tool: "evaluation_worldline_audit", request_id: "r41", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/evaluation-worldline-audit/0.1",
        decisions: 2,
        leak_count: 1,
        leaks: [{ decision: "decision-1", observation: "future", clock: "accessible", decision_at: "2026-01-08T00:00:00Z", available_at: "2026-01-10T00:00:00Z" }],
        dangling_count: 1,
        dangling_references: [["decision-1", "missing"]],
        admissible_at: ["early"],
        guarantees: ["accessibility clock"], limitations: ["no denominator"],
      } } } });
      if (path === "/v1/tools/evaluation_reproduction_check") return jsonResponse({ ok: true, tool: "evaluation_reproduction_check", request_id: "r42", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/evaluation-reproduction-check/0.1",
        certificate: { workflow: "w1", environment_pinned: true, verdicts: [["score", { verdict: "diverged", detail: "delta exceeds tolerance" }]] },
        verdicts: [{ output: "score", verdict: "diverged", detail: "delta exceeds tolerance" }],
        verdict_count: 1, matched_count: 0, diverged_count: 1, missing_count: 0,
        reproduced: false,
        first_divergence: { output: "score", verdict: { verdict: "diverged", detail: "delta exceeds tolerance" } },
        missing_outputs: [], portability_demonstrated: false,
        validity_claim: { ok: false, refusal: "not biological validity", fail_closed: true },
        guarantees: ["first divergence"], limitations: ["no execution"],
      } } } });
      if (path === "/v1/tools/evaluation_trajectory_check") return jsonResponse({ ok: true, tool: "evaluation_trajectory_check", request_id: "r43", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/evaluation-trajectory-check/0.1",
        steps: 1, acts: ["verify"],
        step_records: [{ act: "verify", irreversible: false, succeeded: true, progress: 1 }],
        properties: [{ shape: "followed_by", trigger: "verify", follow_up: "report" }],
        property_records: [{ name: "report-after-verify", property: { shape: "followed_by", trigger: "verify", follow_up: "report" } }],
        property_outcomes: [{ property: "report-after-verify", violations: [], vacuous: false, held: true }],
        property_count: 1, held_count: 1, violated_count: 0, vacuous_count: 0,
        recovery: [], recovery_records: [], recovery_count: 0,
        bounded_suffix: null,
        guarantees: ["nonvacuous"], limitations: ["declared path"],
      } } } });
      if (path === "/v1/tools/agent_mission") return jsonResponse({ ok: true, tool: "agent_mission", request_id: "r5", mcp: { result: { structuredContent: {
        workflow: "agent_mission",
        execution: "planned",
        mission_status: "planned",
        returned_bytes: 0,
        execution_trace_schema_version: "bioprism-devplat-mission-trace/0.1",
        execution_trace: [
          { sequence: 0, event: "mission.started", wave: null, step_id: null, tool: null, status: "planned", arguments_digest: null, bytes: 0, detail: null },
          { sequence: 1, event: "mission.completed", wave: null, step_id: null, tool: null, status: "planned", arguments_digest: null, bytes: 0, detail: "planning did not dispatch any nested tool" },
        ],
        plan: {},
        results: [],
      } } } });
      if (path.startsWith("/v1/route-reviews/")) return jsonResponse({ ok: true, workflow: "capability_route_review_evidence", review_id: "a".repeat(64), found: true, page: { events: [{ id: 1, event_type: "tool.completed", subject: "capability_route_review", request_id: "req-1", payload: {} }], after: 0, next_after: 1, oldest: 1, newest: 1, gap: false, dropped_events: 0 } });
      if (path === "/v1/tools/refuse") return jsonResponse({ ok: true, tool: "refuse", request_id: "r2", mcp: { result: { isError: true, structuredContent: { reason: "blocked" } } }, guarantee: "shared" });
      return jsonResponse({ ok: true });
    },
  });

  assert.equal((await client.tools())[0].name, "echo");
  const catalogue = await client.toolCatalogue();
  assert.equal(catalogue.definitions.length, 1);
  assert.equal(catalogue.digest.length, 64);
  const assembly = client.missionFromRoute({
    workflow: "capability_route",
    route_id: "route-ts",
    catalog_digest: "d".repeat(64),
    goal: "check routed work",
    needs: [{ id: "echo-need", resolution: "explicit", candidate_tools: ["echo"] }],
    unresolved_needs: [],
  }, "mission-from-route", [{
    need_id: "echo-need",
    tool: "echo",
    domain: "workspace",
    capability: "discovery",
    objective: "check routed work",
    arguments: { value: 3 },
  }]);
  assert.equal(assembly.route_id, "route-ts");
  assert.deepEqual(assembly.selected_tools, ["echo"]);
  const assembledPreflight = await client.missionPreflight(assembly.mission, catalogue);
  assert.equal(assembledPreflight.ok, true);
  assert.throws(() => client.missionFromRoute({
    workflow: "capability_route",
    route_id: "route-ts",
    catalog_digest: "d".repeat(64),
    goal: "reject unselected tool",
    needs: [{ id: "echo-need", candidate_tools: ["echo"] }],
    unresolved_needs: [],
  }, "mission-bad-route", [{
    need_id: "echo-need",
    tool: "missing",
    domain: "workspace",
    capability: "discovery",
    objective: "reject",
    arguments: {},
  }]), ArgumentError);
  const plan = await client.planTool("echo", { value: 3, mode: "safe" }, catalogue);
  assert.equal(plan.tool, "echo");
  assert.equal(plan.report.fullyChecked, true);
  await assert.rejects(client.planTool("echo", { value: "not-an-integer" }, catalogue), ToolSchemaError);
  const callsBeforeMissionPreflight = seen.length;
  const preflight = await client.missionPreflight({
    mission_id: "mission-preflight",
    goal: "prepare and consume",
    steps: [
      { id: "prepare", domain: "workspace", capability: "discovery", objective: "prepare", tool: "echo", arguments: { value: 3 } },
      {
        id: "consume",
        domain: "workspace",
        capability: "discovery",
        objective: "consume",
        tool: "echo",
        arguments: { value: 3 },
        depends_on: ["prepare"],
        bindings: [{ from_step: "prepare", source_pointer: "/value", target_pointer: "/value" }],
      },
    ],
  }, catalogue);
  assert.equal(preflight.ok, true);
  assert.equal(preflight.fully_checked, true);
  assert.deepEqual(preflight.waves, [["prepare"], ["consume"]]);
  assert.equal(preflight.steps[1].status, "ready");
  assert.equal(seen.length, callsBeforeMissionPreflight);
  const parallel = await client.missionPreflight({
    mission_id: "mission-parallel",
    goal: "prepare independent checks",
    steps: [
      { id: "first", domain: "workspace", capability: "discovery", objective: "first", tool: "echo", arguments: { value: 1 } },
      { id: "second", domain: "workspace", capability: "discovery", objective: "second", tool: "echo", arguments: { value: 2 } },
    ],
    policy: {
      execute: true,
      execution_mode: "parallel_waves",
      max_parallelism: 2,
      allowed_tools: ["echo"],
      max_step_output_bytes: 2_000_000,
      max_total_output_bytes: 4_000_000,
    },
  }, catalogue);
  assert.equal(parallel.ok, true);
  assert.equal(parallel.execution_mode, "parallel_waves");
  assert.equal(parallel.max_parallelism, 2);
  assert.deepEqual(parallel.waves, [["first", "second"]]);
  const invalidMode = await client.missionPreflight({
    mission_id: "mission-invalid-mode",
    goal: "reject an unknown execution mode",
    steps: [{ id: "only", domain: "workspace", capability: "discovery", objective: "only", tool: "echo", arguments: { value: 3 } }],
    policy: { execution_mode: "distributed" },
  }, catalogue);
  assert.equal(invalidMode.ok, false);
  assert.equal(invalidMode.issues.some((issue) => issue.includes("execution_mode")), true);
  const invalidParallelism = await client.missionPreflight({
    mission_id: "mission-invalid-parallelism",
    goal: "reject an unsafe concurrency ceiling",
    steps: [{ id: "only", domain: "workspace", capability: "discovery", objective: "only", tool: "echo", arguments: { value: 3 } }],
    policy: { execution_mode: "parallel_waves", max_parallelism: 17 },
  }, catalogue);
  assert.equal(invalidParallelism.ok, false);
  assert.equal(invalidParallelism.issues.some((issue) => issue.includes("max_parallelism")), true);
  const cycle = await client.missionPreflight({
    mission_id: "mission-cycle",
    goal: "reject a cycle",
    steps: [
      { id: "a", domain: "workspace", capability: "discovery", objective: "a", tool: "echo", arguments: { value: 3 }, depends_on: ["b"] },
      { id: "b", domain: "workspace", capability: "discovery", objective: "b", tool: "echo", arguments: { value: 3 }, depends_on: ["a"] },
    ],
  }, catalogue);
  assert.equal(cycle.ok, false);
  assert.equal(cycle.issues.some((issue) => issue.includes("dependency cycle")), true);
  assert.equal(cycle.steps.every((step) => step.status === "blocked"), true);
  assert.throws(() => assertMissionPreflight(cycle), MissionPreflightError);
  const unauthorized = await client.missionPreflight({
    mission_id: "mission-unauthorized",
    goal: "reject implicit execution",
    steps: [{ id: "only", domain: "workspace", capability: "discovery", objective: "only", tool: "echo", arguments: { value: 3 } }],
    policy: { execute: true },
  }, catalogue);
  assert.equal(unauthorized.ok, false);
  assert.equal(unauthorized.execution, "planned");
  assert.equal(unauthorized.issues.some((issue) => issue.includes("allowed_tools")), true);
  const checked = await client.toolChecked("echo", { value: 4 }, undefined, catalogue);
  assert.equal(checked.mcp.result.structuredContent.value, 3);
  const response = await client.callTool("echo", { value: 3 }, { requestId: "request-1" });
  assert.equal(response.mcp.result.structuredContent.value, 3);
  assert.equal(seen.at(-1).init.headers.Authorization, "Bearer 0123456789abcdef");
  assert.equal(seen.at(-1).init.headers["x-request-id"], "request-1");
  const analytics = await client.metricsAnalyticsAudit({ observations: [{ id: "one" }] });
  assert.equal(analytics.mcp.result.structuredContent.workflow, "metrics_descriptive_analytics");
  const catalog = await client.repositoryCatalog({ prefix: "docs/", limit: 5, include_briefs: true });
  const bundle = await client.repositoryBundle({ route: { id: "route-ts" }, policy: "exhaustive", max_depth: 2 });
  const impact = await client.repositoryImpact({ changed: "docs/README", route: { id: "route-ts" } });
  const telemetry = await client.telemetryProject({ event: { kind: "tool.completed" }, policy: { treatments: {} }, trace: "trace-ts" });
  const ledger = await client.ledgerIngest({ events: [{ class: "material" }], include_receipts: false, max_items: 5 });
  const quality = await client.qualityGateRun({
    dataset: { name: "release-quality", columns: { age: [41, 42], subject: ["s-1", "s-1"] }, rows: 2 },
    gate: { name: "release-gate", checks: { age_range: { InRange: { column: "age", min: 0, max: 120 } } } },
  });
  const atlas = await client.atlasReport({ atlas: { ontology: {}, cells: {}, failures: [] }, max_items: 10 });
  const atlasSurface = await client.atlasSurfaceAudit({ grid: { label: "surface-system", conditions: {}, cells: {} }, facet: "mechanism", max_items: 10 });
  const adaptive = await client.adaptivePanel({ panel: { config: {}, ledger: {} }, candidates: [{ instance: "inst-1", capability: "capability-a", parent: "parent-1", cost: 1 }], capability: "capability-a" });
  const posterior = await client.posteriorGate({ observations: [{ capability: "capability-a", parent: "parent-1", result: { conclusion: "pass" } }], other_observations: [], tolerance: 0.01, min_effective: 2 });
  const otel = await client.traceOtelIngest({ trace_id: "otel-ts", otlp_json: '{"resourceSpans":[]}', include_events: true });
  assert.equal(catalog.mcp.result.structuredContent.workflow, "repository_catalog");
  assert.equal(bundle.mcp.result.structuredContent.policy, "exhaustive");
  assert.equal(impact.mcp.result.structuredContent.changed, "docs/README");
  assert.equal(telemetry.mcp.result.structuredContent.trace, "trace-ts");
  assert.equal(telemetry.mcp.result.structuredContent.record.event_id, "evt-ts");
  assert.equal(telemetry.mcp.result.structuredContent.lossless, true);
  assert.equal(ledger.mcp.result.structuredContent.schema, "bioprism-mcp/ledger-ingest/0.1");
  assert.equal(ledger.mcp.result.structuredContent.chain.status, "intact");
  assert.equal(ledger.mcp.result.structuredContent.latest_by_subject.items[0].payload_digest, "payload-digest");
  assert.equal(quality.mcp.result.structuredContent.schema, "bioprism-mcp/quality-gate/0.1");
  assert.equal(quality.mcp.result.structuredContent.report.outcomes.subject_unique.Fail.witness.row, 1);
  assert.equal(quality.mcp.result.structuredContent.report.outcomes.foreign_site.NotRunnable.reason.MissingReferenceSet.reference, "sites");
  assert.equal(atlas.mcp.result.structuredContent.schema, "bioprism-mcp/atlas-report/0.1");
  assert.equal(atlas.mcp.result.structuredContent.holes[0].reason, "not_attempted");
  assert.equal(atlas.mcp.result.structuredContent.composite.fail_closed, true);
  assert.equal(atlasSurface.mcp.result.structuredContent.schema, "bioprism-mcp/atlas-surface-audit/0.1");
  assert.equal(atlasSurface.mcp.result.structuredContent.failure_browse.withheld, 1);
  assert.equal(atlasSurface.mcp.result.structuredContent.surface_audits.sound, true);
  assert.equal(adaptive.mcp.result.structuredContent.schema, "bioprism-mcp/adaptive-panel/0.1");
  assert.equal(adaptive.mcp.result.structuredContent.selection.value.record.chosen.instance, "inst-1");
  assert.equal(adaptive.mcp.result.structuredContent.capability.estimate, null);
  assert.equal(posterior.mcp.result.structuredContent.schema, "bioprism-mcp/posterior-gate/0.1");
  assert.equal(posterior.mcp.result.structuredContent.capabilities["capability-a"].pass_rate.effective_sample_size, 2);
  assert.equal(posterior.mcp.result.structuredContent.gate.value.sensitivity[0][0], "capability-a");
  assert.equal(posterior.mcp.result.structuredContent.comparison.dominance.dominance, "incomparable");
  assert.equal(otel.mcp.result.structuredContent.schema, "bioprism-mcp/trace-otel-ingest/0.1");
  assert.equal(otel.mcp.result.structuredContent.mapping.accepted_span_count, 1);
  assert.equal(otel.mcp.result.structuredContent.events[0].kind, "goal");
  const workbench = await client.developerWorkbench({ session: { session_id: "studio-1" }, dashboard: { include_holes: true } });
  assert.equal(workbench.mcp.result.structuredContent.workflow, "developer_workbench");
  const ciEvidence = await client.ciExecutionEvidenceAudit({ ci: { workflow: "contracts" }, evidence: { run_id: "run-42" } });
  assert.equal(ciEvidence.mcp.result.structuredContent.workflow, "ci_execution_evidence_audit");
  assert.equal(ciEvidence.mcp.result.structuredContent.ci_evidence_ready, true);
  assert.equal(ciEvidence.mcp.result.structuredContent.audit.verification, "structural_only");
  const platform = await client.developerPlatformStatus({ include_details: false, max_items: 3 });
  assert.equal(platform.mcp.result.structuredContent.detail_mode, "summary");
  assert.equal(platform.mcp.result.structuredContent.devplat.modules_classified, 4);
  const tokenPlan = await client.tokenContextPlan({
    request: { world_ref: "world", decision_ref: "decision", role: "researcher", policy_id: "policy", envelope: { total: 100 }, depth: "l1", compiler_version: "compiler/1" },
    candidates: [{ node_id: "invariant/identity", kind: "invariant", mandatory: true, estimate: { tokens: 20, method: { method: "declared_by_caller" } } }],
  });
  assert.equal(tokenPlan.mcp.result.structuredContent.plan.mandatory_estimate.tokens, 20);
  assert.equal(tokenPlan.mcp.result.structuredContent.comparison, null);
  const weave = await client.weavelangCompile({ source: "package demo", execute: false, mode: "replay" });
  assert.equal(weave.mcp.result.structuredContent.execution.mode, "replay");
  assert.equal(weave.mcp.result.structuredContent.program.semantic_digest.length, 64);
  const voi = await client.epistemicVoi({
    problem: { actions: ["treat", "abstain"], models: ["responsive", "resistant"], loss: [0, 10, 10, 0] },
    belief: { mass: [0.5, 0.5] },
    acquisition: {
      id: "assay",
      cost: 0.1,
      outcomes: [
        { label: "positive", likelihood: [0.9, 0.1] },
        { label: "negative", likelihood: [0.1, 0.9] },
      ],
    },
  });
  assert.equal(voi.mcp.result.structuredContent.value.gross, 4);
  assert.equal(voi.mcp.result.structuredContent.value.net, 3.9);
  assert.deepEqual(voi.mcp.result.structuredContent.actions.after, ["treat", "abstain"]);
  const traceAnalysis = await client.benchmarkTraceAnalyze({
    failing: {
      trace_id: "failed-run",
      succeeded: false,
      events: [{ step: 0, kind: "goal", payload: { summary: "solve" } }],
    },
    reference: {
      trace_id: "reference-run",
      succeeded: true,
      events: [{ step: 0, kind: "goal", payload: { summary: "solve" } }],
    },
  });
  assert.equal(traceAnalysis.mcp.result.structuredContent.analysis.verdict.verdict, "first_causal");
  const decisionAudit = await client.benchmarkDecisionAudit({
    trace: { trace_id: "failed-run", succeeded: false, events: [{ step: 1, kind: "choice", payload: { action: "unsafe" } }] },
    max_items: 10,
  });
  assert.equal(decisionAudit.mcp.result.structuredContent.schema, "bioprism-mcp/benchmark-decision-audit/0.1");
  assert.equal(decisionAudit.mcp.result.structuredContent.decision.coverage.validation_only, 1);
  assert.equal(decisionAudit.mcp.result.structuredContent.failure_card.blame.blame, "agent");
  const integrityAudit = await client.benchmarkIntegrityAudit({
    instances: [{ instance_id: "a", content: {}, acceptable_verdicts: [], required_witnesses: [] }],
    private_share: 20,
    max_items: 10,
  });
  assert.equal(integrityAudit.mcp.result.structuredContent.schema, "bioprism-mcp/benchmark-integrity-audit/0.1");
  assert.equal(integrityAudit.mcp.result.structuredContent.contamination.admissible, 1);
  assert.equal(integrityAudit.mcp.result.structuredContent.effective_diversity.equivalence_classes, 2);
  const counterfactual = await client.benchmarkCounterfactualCheck({
    source: { cell_id: "source" },
    followup: { cell_id: "followup" },
    intervention: { factor: "fresh evidence", target: "evidence_availability", from: false, to: true, changes: ["query"] },
    expected: { expect: "invariant", rationale: "same verdict" },
    source_verdict: "pass",
    followup_verdict: "pass",
  });
  assert.equal(counterfactual.mcp.result.structuredContent.outcome.outcome, "as_predicted");
  assert.equal(counterfactual.mcp.result.structuredContent.pair.realism_reviewed, false);
  const oracleReview = await client.benchmarkOracleReview({
    proposal: { oracle_id: "oracle-demo", decision_point: "choose evidence", strength: "exact_state_predicate", acceptable_verdicts: ["pass"], required_witnesses: ["evidence"], can_see: ["world"], blind_spots: ["hidden grader state"], exploits: [] },
    reviewer: "reviewer-1",
    grade: { verdict: "pass", witnesses: ["evidence"], closure_complete: true },
    cell: { cell_id: "cell-reviewed", world: { locator: "world", sha256: "a".repeat(64) }, query: { locator: "query", sha256: "b".repeat(64) } },
  });
  assert.equal(oracleReview.mcp.result.structuredContent.schema, "bioprism-mcp/benchmark-oracle-review/0.1");
  assert.equal(oracleReview.mcp.result.structuredContent.grade.acceptance.outcome, "passed");
  assert.equal(oracleReview.mcp.result.structuredContent.cell.cell_id, "cell-reviewed");
  const benchmarkCompile = await client.benchmarkCompile({
    trace: { trace_id: "run_fail", succeeded: false, events: [{ step: 0, kind: "goal", payload: { summary: "rank" } }] },
    context: [{ id: "panel_manifest", tier: "artifact", guard: "removable" }],
    probe_observations: [{ kept: ["panel_manifest"], signature: { verdict: "invalid", witnesses: ["identity_leakage"], divergence_step: 3 } }],
  });
  assert.equal(benchmarkCompile.mcp.result.structuredContent.schema, "bioprism-mcp/benchmark-compile/0.1");
  assert.equal(benchmarkCompile.mcp.result.structuredContent.class.class, "candidate_research_cell");
  assert.equal(benchmarkCompile.mcp.result.structuredContent.minimization.reduction_ratio, 0.5);
  const benchmarkCompileReview = await client.benchmarkCompileReview({
    trace: { trace_id: "run_fail", succeeded: false, events: [{ step: 0, kind: "goal", payload: { summary: "rank" } }] },
    context: [{ id: "panel_manifest", tier: "artifact", guard: "removable" }],
    probe_observations: [{ kept: ["panel_manifest"], signature: { verdict: "invalid", witnesses: ["identity_leakage"], divergence_step: 3 } }],
    reviewer: "reviewer-1",
    world: { locator: "world", sha256: "a".repeat(64) },
    query: { locator: "query", sha256: "b".repeat(64) },
    grade: { verdict: "invalid", witnesses: ["identity_leakage"], closure_complete: true },
  });
  assert.equal(benchmarkCompileReview.mcp.result.structuredContent.schema, "bioprism-mcp/benchmark-compile-review/0.1");
  assert.equal(benchmarkCompileReview.mcp.result.structuredContent.cell.cell_id, "dc_run_fail#step3");
  assert.equal(benchmarkCompileReview.mcp.result.structuredContent.grade.acceptance.outcome, "passed");
  const coverage = await client.packCoverageAudit({ section: "15", max_items: 3 });
  assert.equal(coverage.mcp.result.structuredContent.schema, "bioprism-mcp/pack-coverage-audit/0.1");
  assert.equal(coverage.mcp.result.structuredContent.summary.uncovered, 4);
  assert.equal(coverage.mcp.result.structuredContent.rows_omitted, 13);
  const packRelease = await client.packReleaseAudit({ section: "15", max_items: 3 });
  assert.equal(packRelease.mcp.result.structuredContent.schema, "bioprism-mcp/pack-release-audit/0.1");
  assert.equal(packRelease.mcp.result.structuredContent.sequenced_count, 1);
  assert.equal(packRelease.mcp.result.structuredContent.unsequenced_omitted, 0);
  const foundation = await client.foundationContractCheck({
    contract: { id: "fbc:test:001", intent: "check", falsifiers: ["disagree"], actions: ["inspect"], claim_schema: "typed", reference_standard: "fixture", terminations: ["success"] },
    claim: "real_treatment_effect",
  });
  assert.equal(foundation.mcp.result.structuredContent.verdict, "refused");
  assert.equal(foundation.mcp.result.structuredContent.transition.verdict, "plane_confusion");
  const packCatalogue = await client.packCatalogue({ section: "15", max_items: 1 });
  assert.equal(packCatalogue.mcp.result.structuredContent.returned[0].blueprint_module, "15.01");
  assert.equal(packCatalogue.mcp.result.structuredContent.returned[0].release_wave.wave, 1);
  const packHealth = await client.packHealthAssess({ pack: { manifest: {}, content: {} }, observations: { calibration: { observations: [] }, trivial_baselines: [], contamination: [] } });
  assert.equal(packHealth.mcp.result.structuredContent.verdict, "unreportable");
  assert.equal(packHealth.mcp.result.structuredContent.score_gate.reportable, false);
  assert.equal(packHealth.mcp.result.structuredContent.health.pack_digest.length, 64);
  const redteam = await client.securityRedteamSimulate({ findings: [{ id: "F-confirmed" }], include_details: true, max_items: 10 });
  assert.equal(redteam.mcp.result.structuredContent.workflow, "section_13_redteam_incident_evidence");
  assert.equal(redteam.mcp.result.structuredContent.regression_corpus.sentinel_count, 1);
  assert.equal(redteam.mcp.result.structuredContent.findings[0].regression_gate.eligible, true);
  const factory = await client.factoryLifecycleSimulate({
    jobs: [{ id: "job-1", resource_class: "compile", idempotency: "idempotent", priority: 5, max_attempts: 3, spec: { kind: "pure-build" }, state: "queued", attempts: 0 }],
    workers: [{ worker_id: "worker-1", classes: ["compile"], lease_duration_nanos: 30 }],
    actions: [{ kind: "lease", worker_id: "worker-1", now_nanos: 0 }],
  });
  assert.equal(factory.mcp.result.structuredContent.trace[0].kind, "lease");
  assert.equal(factory.mcp.result.structuredContent.jobs[0].job.state, "succeeded");
  assert.equal(factory.mcp.result.structuredContent.trace.every((row) => row.fail_closed !== true), true);
  const storage = await client.storageLifecycleSimulate({
    now: 20,
    tiering_policy: { demote_to_warm_after: 5, demote_to_cold_after: 12, promote_after_accesses: 3, promote_within: 2 },
    records: [{ object: "pinned-hot", tier: "hot", last_access: 0, pinned: true }],
    quota: { limit: 1000, reserve: 100 },
  });
  assert.equal(storage.mcp.result.structuredContent.schema, "bioprism-mcp/storage-lifecycle/0.1");
  assert.equal(storage.mcp.result.structuredContent.tiering.plan.transitions[0].reason.HeldByPin.epochs, 20);
  assert.equal(storage.mcp.result.structuredContent.quota.remaining_for_ingest, 900);
  const registry = await client.registryLifecycleSimulate({
    packs: [{ not: "an attested pack" }],
    actions: [{ op: "publish", pack_index: 0, tier: "exploratory" }, { op: "verify_all" }],
    include_index: true,
  });
  assert.equal(registry.mcp.result.structuredContent.schema, "bioprism-mcp/registry-lifecycle/0.1");
  assert.equal(registry.mcp.result.structuredContent.actions[0].fail_closed, true);
  assert.equal(registry.mcp.result.structuredContent.actions[2].result.clean, true);
  assert.equal(Object.hasOwn(registry.mcp.result.structuredContent, "registry"), true);
  const cache = await client.cacheInvalidationSimulate({
    schema: { name: "decision-cache", components: ["input", "code"], reuse: "same_build_only" },
    entries: [{ components: { input: "world@2", code: "build-a" }, produced_by: "build-a", written_at: 1, dependencies: "undeclared" }],
    graph: { opaque: ["input"] },
    changed: "input",
    apply: true,
    apply_at: 2,
  });
  assert.equal(cache.mcp.result.structuredContent.invalidation.plan.completeness.Partial.entries_without_declared_dependencies[0], "d-unproven");
  assert.equal(cache.mcp.result.structuredContent.invalidation.apply_report.invalidation_was_complete, false);
  assert.equal(cache.mcp.result.structuredContent.lookups.post_apply[0].miss_reason.UnprovenAfterPartialInvalidation.since, 2);
  const disclosure = await client.hubDisclosureReview({
    actions: [
      { kind: "declare_held_out", pack: "pack-1" },
      { kind: "disclose", pack: "pack-1", at: 5 },
      { kind: "headline_eligibility", pack: "pack-1", computed_at: 6 },
    ],
  });
  assert.equal(disclosure.mcp.result.structuredContent.schema, "bioprism-mcp/hub-disclosure/0.1");
  assert.equal(disclosure.mcp.result.structuredContent.trace[2].result.eligible, false);
  assert.equal(disclosure.mcp.result.structuredContent.trace[2].result.fail_closed, true);
  assert.equal(disclosure.mcp.result.structuredContent.trace[3].result.label.label, "disclosed_pack");
  const submission = await client.hubSubmissionReview({
    draft: { id: "sub-1", content: "digest-1" },
    submitter: { id: "lab-a", conflicts_declared: true },
    moderation: { transitions: [], attestations: [], revocations: [] },
  });
  assert.equal(submission.mcp.result.structuredContent.schema, "bioprism-mcp/hub-submission/0.1");
  assert.equal(submission.mcp.result.structuredContent.stage, "moderation_ledger");
  assert.equal(submission.mcp.result.structuredContent.verification, "reproduced");
  const card = await client.hubCardRender({
    moderation: { records: {} },
    submission: "sub-card",
    score: { value: 0.82, interval: null },
    pack: "pack-1",
    computed_at: 4,
    disclosure: { packs: {} },
  });
  assert.equal(card.mcp.result.structuredContent.schema, "bioprism-mcp/hub-card/0.1");
  assert.equal(card.mcp.result.structuredContent.card.score.display, "published");
  assert.equal(card.mcp.result.structuredContent.score.attached, true);
  const leaderboard = await client.hubLeaderboardRender({ board: {}, entries: [], moderation: {}, disclosure: {}, include_details: false });
  assert.equal(leaderboard.mcp.result.structuredContent.schema, "bioprism-mcp/hub-leaderboard/0.1");
  assert.equal(leaderboard.mcp.result.structuredContent.unranked_count, 1);
  const delivery = await client.developerDeliveryAudit({
    ci_evidence: { ci: { workflow: "contracts" }, evidence: { run_id: "run-42" } },
    release_request: { id: "delivery-1", targets: ["ci_execution_evidence"] },
  });
  assert.equal(delivery.mcp.result.structuredContent.workflow, "developer_delivery_audit");
  assert.equal(delivery.mcp.result.structuredContent.readiness.local_delivery_ready, true);
  assert.equal(delivery.mcp.result.structuredContent.ci_evidence.ci_evidence_ready, true);
  assert.equal(delivery.mcp.result.structuredContent.readiness.ci_execution_evidence_ready, true);
  assert.equal(delivery.mcp.result.structuredContent.release_request.targets[0].target, "ci_execution_evidence");
  const deliveryRequest = JSON.parse(seen.at(-1).init.body);
  assert.deepEqual(deliveryRequest.ci_evidence, { ci: { workflow: "contracts" }, evidence: { run_id: "run-42" } });
  const engineering = await client.engineeringManifestAudit({
    project: { id: "aurora-agent", version: "0.1.0", repository: "github.com/AURORA-NEURO/aurora-agent" },
    baseline: { language: "Rust 2021", runtime: "cargo", api: "MCP JSON-RPC", storage: "in-memory", observability: "structured", deployment: "local" },
    packages: [{ id: "core", path: "crates/core", language: "rust", kind: "library", owner: "platform" }],
    tickets: [{ id: "T-001", title: "ship core", package: "core", contract: "core-contract", status: "done", acceptance: ["tests pass"] }],
  });
  assert.equal(engineering.mcp.result.structuredContent.schema, "bioprism-engineering-audit/0.1");
  assert.equal(engineering.mcp.result.structuredContent.audit.package_order[1], "api");
  assert.equal(engineering.mcp.result.structuredContent.audit.counts.actionable_tickets, 1);
  const engineeringPlan = await client.engineeringExecutionPlan({
    manifest: {
      project: { id: "aurora-agent", version: "0.1.0", repository: "github.com/AURORA-NEURO/aurora-agent" },
      baseline: { language: "Rust 2021", runtime: "cargo", api: "MCP JSON-RPC", storage: "in-memory", observability: "structured", deployment: "local" },
      packages: [{ id: "core", path: "crates/core", language: "rust", kind: "library", owner: "platform" }],
      tickets: [{ id: "T-001", title: "ship core", package: "core", contract: "core-contract", status: "done", acceptance: ["tests pass"] }],
    },
    policies: { max_tickets: 10, max_parallelism: 2 },
  });
  assert.equal(engineeringPlan.mcp.result.structuredContent.workflow, "engineering_execution_plan");
  assert.equal(engineeringPlan.mcp.result.structuredContent.engineering_plan_ready, true);
  assert.deepEqual(engineeringPlan.mcp.result.structuredContent.audit.critical_path, ["T-001", "T-002", "T-003"]);
  assert.equal(engineeringPlan.mcp.result.structuredContent.audit.waves[1].depends_on_waves[0], 0);
  const releasePipeline = await client.releasePipelineAudit({
    project: { id: "aurora-agent", version: "0.1.0", repository: "github.com/AURORA-NEURO/aurora-agent" },
    source: { ref_name: "main", commit_digest: "a".repeat(64), workflow: "release.yml" },
    environments: [{ id: "staging", class: "staging" }, { id: "production", class: "production", protected: true, required_approvals: 1 }],
    stages: [{ id: "build", kind: "build", environment: "staging" }, { id: "test", kind: "test", environment: "staging", depends_on: ["build"] }],
    artifacts: [{ id: "binary", kind: "binary", digest: "a".repeat(64), produced_by: "build", attestations: ["prov", "sig"] }],
    attestations: [
      { id: "prov", kind: "provenance", artifact: "binary", digest: "a".repeat(64), issuer: "ci", statement: "built" },
      { id: "sig", kind: "signature", artifact: "binary", digest: "a".repeat(64), issuer: "key", statement: "signed" },
    ],
    promotions: [{ id: "to-production", kind: "advance", from: "staging", to: "production", artifacts: ["binary"], required_attestations: ["prov", "sig"], rollback_target: "rollback" }],
  });
  assert.equal(releasePipeline.mcp.result.structuredContent.schema, "bioprism-release-pipeline-audit/0.1");
  assert.equal(releasePipeline.mcp.result.structuredContent.release_ready, true);
  assert.equal(releasePipeline.mcp.result.structuredContent.audit.promotion_audits[0].rollback_present, true);
  const securityPrivacy = await client.securityPrivacyAudit({
    system: { id: "aurora-api", version: "0.1.0", owner: "platform" },
    assets: [{ id: "patient-records", name: "records", classification: "regulated", owner: "privacy", purpose: "care research", retention_days: 365, residency: "us", deletion_process: "erase" }],
    flows: [{ id: "api-to-vendor", asset: "patient-records", source: "api", destination: "approved-vendor", purpose: "care research", legal_basis: "consent", decision: "allow", authorization_evidence: "a".repeat(64) }],
    identities: [{ id: "researcher", principal: "team", role: "research", authentication: "oidc", mfa: true, least_privilege: true, assets: ["patient-records"] }],
    threats: [{ id: "exfiltration", category: "data-exfiltration", severity: "high", status: "mitigated", control: "dlp", evidence_digest: "a".repeat(64) }],
    reviews: [{ id: "pia-1", kind: "privacy_impact", scope: "patient-records", reviewer: "independent", status: "complete", evidence_digest: "a".repeat(64) }],
    controls: { access_control: true, encryption_at_rest: true, encryption_in_transit: true, key_rotation: true, audit_logging: true, vulnerability_management: true, backup_restore: true, incident_response: true, vendor_review: true, data_subject_rights: true },
  });
  assert.equal(securityPrivacy.mcp.result.structuredContent.schema, "bioprism-security-privacy-audit/0.1");
  assert.equal(securityPrivacy.mcp.result.structuredContent.security_privacy_ready, true);
  assert.equal(securityPrivacy.mcp.result.structuredContent.audit.flow_audits[0].authorization_present, true);
  assert.equal(securityPrivacy.mcp.result.structuredContent.audit.threat_audits[0].treated, true);
  const sandboxAdmission = await client.sandboxAdmissionAudit({
    system: { id: "prism-sandbox", version: "0.1.0", owner: "platform" },
    artifacts: [
      { id: "source", kind: "source_code", digest: "a".repeat(64), source: "repo/source.py", producer: "ci", trust: "reviewed" },
      { id: "dataset", kind: "dataset", digest: "b".repeat(64), source: "registry/dataset", producer: "registry", trust: "untrusted", inputs: ["source"] },
    ],
    profiles: [{ id: "profile", artifact: "dataset", runtime: "oci", image_digest: "c".repeat(64), environment_digest: "d".repeat(64), user: "runner", rootless: true, read_only_root: true, no_privilege_escalation: true, network: "allowlist", network_allowlist: ["packages.example"], mounts: [{ id: "input", source_artifact: "dataset", target: "/inputs/data", mode: "read_only" }], capabilities: ["network"], resources: { cpu_millis: 1000, memory_mb: 1024, wall_time_seconds: 60, processes: 8, output_bytes: 1000000 }, output_quarantine: true, release_requires_review: true }],
    capabilities: [{ id: "network", profile: "profile", kind: "network_egress", target: "packages.example", decision: "allow", evidence_digest: "e".repeat(64) }],
    outputs: [{ id: "result", profile: "profile", artifact: "dataset", digest: "f".repeat(64), destination: "quarantine", quarantined: true, released: false, reviewed: false, parents: ["dataset"] }],
  });
  assert.equal(sandboxAdmission.mcp.result.structuredContent.schema, "bioprism-sandbox-audit/0.1");
  assert.equal(sandboxAdmission.mcp.result.structuredContent.sandbox_ready, true);
  assert.equal(sandboxAdmission.mcp.result.structuredContent.audit.profile_audits[0].isolation_valid, true);
  assert.equal(sandboxAdmission.mcp.result.structuredContent.audit.output_audits[0].quarantined, true);
  const sandboxRuntime = await client.sandboxRuntimeSimulate({
    schema: "bioprism-sandbox-runtime/0.1",
    admission: {
      schema: "bioprism-sandbox/0.1",
      system: { id: "prism-sandbox", version: "0.1.0", owner: "platform" },
      artifacts: [], profiles: [], capabilities: [], outputs: [],
    },
    profile: "profile",
    requests: [{ id: "read-input", kind: "filesystem_read", target: "/inputs/data", cpu_millis: 100, memory_mb: 128, wall_time_seconds: 5, processes: 1, output_bytes: 1000 }],
  });
  assert.equal(sandboxRuntime.mcp.result.structuredContent.schema, "bioprism-sandbox-runtime-audit/0.1");
  assert.equal(sandboxRuntime.mcp.result.structuredContent.sandbox_runtime_ready, true);
  assert.equal(sandboxRuntime.mcp.result.structuredContent.audit.steps[0].decision, "simulated");
  assert.equal(sandboxRuntime.mcp.result.structuredContent.audit.usage.cpu_millis, 200);
  const securityProgram = await client.securityProgramAudit({
    system: { id: "aurora-security", version: "0.1.0", owner: "security-owner", mission: "bounded adversarial assurance" },
    scopes: [{ id: "api-staging", name: "staging API", kind: "api", target: "api-staging.internal", owner: "service-owner", authorization_digest: "a".repeat(64), allowed_methods: ["authenticated-read"], forbidden_actions: ["production-write"], environments: ["isolated-staging"], data_handling: "synthetic fixtures only" }],
    campaigns: [{ id: "campaign-1", scope: "api-staging", operator: "red-team", independent_reviewer: "independent-reviewer", methodology: "bounded mutation", hypothesis: "boundary crossing", status: "completed", started_at: "2026-01-01", completed_at: "2026-01-02", evidence_digest: "a".repeat(64), stop_conditions: ["stop on production boundary"], finding_ids: ["finding-1"] }],
    findings: [{ id: "finding-1", campaign: "campaign-1", title: "boundary mismatch", severity: "high", status: "closed", evidence_digest: "a".repeat(64), reproduction_digest: "a".repeat(64), regression_digest: "a".repeat(64), discovered_at: "2026-01-02", affected_targets: ["api-staging"], remediation_ids: ["remediation-1"], incident_id: "incident-1", public_safe: true }],
    remediations: [{ id: "remediation-1", finding: "finding-1", owner: "service-owner", action: "validate boundary", status: "complete", due_at: "2026-01-10", verification_digest: "a".repeat(64) }],
    incidents: [{ id: "incident-1", finding: "finding-1", severity: "high", owner: "incident-owner", status: "closed", opened_at: "2026-01-02", contained_at: "2026-01-02", closed_at: "2026-01-03", containment_evidence: "a".repeat(64), closure_evidence: "a".repeat(64), notification_required: true, timeline: [{ epoch: 1, actor: "incident-owner", event: "opened", evidence_digest: "a".repeat(64) }] }],
    disclosures: [{ id: "advisory-1", finding: "finding-1", stage: "advisory", audience: "affected operators", requested_at: "2026-01-04", approver: "independent-reviewer", approval_digest: "a".repeat(64), advisory_digest: "a".repeat(64), published_at: "2026-01-04" }],
    controls: { scope_authorization: true, operator_separation: true, independent_review: true, evidence_retention: true, remediation_tracking: true, incident_response: true, disclosure_review: true, regression_testing: true },
  });
  assert.equal(securityProgram.mcp.result.structuredContent.schema, "bioprism-security-program-audit/0.1");
  assert.equal(securityProgram.mcp.result.structuredContent.security_program_ready, true);
  assert.equal(securityProgram.mcp.result.structuredContent.audit.finding_audits[0].incident_valid, true);
  assert.equal(securityProgram.mcp.result.structuredContent.audit.disclosure_audits[0].approval_valid, true);
  const operational = await client.operationalReadinessAudit({
    service: { id: "aurora-api", version: "0.1.0", owner: "platform", criticality: "critical" },
    contracts: [{ id: "availability", kind: "availability", objective: "serve requests", target: "99.9%", required: true }],
    indicators: [{ id: "availability", contract: "availability", metric: "availability_ratio", source: "metrics", status: "observed", measurement: "0.999", evidence_digest: "a".repeat(64) }],
    dependencies: [{ id: "registry", name: "registry", owner: "platform", criticality: "critical", failure_mode: "unavailable", fallback: "cached" }],
    runbooks: [{ id: "api-degraded", trigger: "availability alert", owner: "oncall", steps: ["triage", "fail over"], review_status: "reviewed", incident_classes: ["availability"] }],
    incidents: [{ id: "INC-1", severity: "sev2", state: "closed", runbook: "api-degraded", owner: "oncall", timeline: ["detected", "resolved"], postmortem: "learned" }],
    controls: { on_call: true, alerting: true, tracing: true, audit_logging: true, backup: true, restore_test: true, access_review: true },
  });
  assert.equal(operational.mcp.result.structuredContent.schema, "bioprism-operational-readiness-audit/0.1");
  assert.equal(operational.mcp.result.structuredContent.operationally_ready, true);
  assert.equal(operational.mcp.result.structuredContent.audit.indicator_audits[0].observed, true);
  assert.equal(operational.mcp.result.structuredContent.audit.dependency_audits[0].fallback_present, true);
  assert.equal(operational.mcp.result.structuredContent.audit.incident_audits[0].postmortem_present, true);
  const capabilities = await client.capabilityDiscover({ query: "oncology evidence", include_tools: true });
  const evidenceAudit = await client.bioCapabilityEvidenceAudit({ evidence: [], claim_requests: [], metrics: {} });
  const publicationAudit = await client.bioAtlasPublicationAudit({ atlas: { atlas_id: "atlas-1" }, release_request: { id: "publication-1", targets: ["atlas_profile"] } });
  const capabilityAudit = await client.capabilityAudit({ include_groups: false });
  const route = await client.capabilityRoute({ goal: "compose evidence", needs: [{ id: "oncology", query: "oncology" }] });
  const routeReview = await client.capabilityRouteReview({
    route: route.mcp.result.structuredContent,
    selections: [{ need_id: "oncology", tool: "echo", domain: "testing", capability: "verification", objective: "review", arguments: {} }],
    validate_schemas: true,
  });
  const adapter = await client.adapterPlan({ source_id: "scan-1", source_kind: "bytes", declared_format: "application/dicom", available_dependencies: ["pydicom"] });
  const tabular = await client.tabularIngest({ source_id: "cohort.csv", profile: { profile_id: "RG-DEMO-001" }, csv: "subject\nS1\n", format: "text/csv", include_facts: true });
  const conformance = await client.conformanceRun({ include_details: false, max_items: 100 });
  const release = await client.releaseAudit({ checks: [{ kind: "conformance_run", arguments: {} }] });
  const operations = await client.operationsCatalog({ include_details: false, max_items: 2 });
  const acceptance = await client.opsAcceptance({ max_items: 3 });
  const safety = await client.safetyReleaseGate({ assessment: { subject: "pack/biological-design@1", category: "biological_design", ratings: { capability_uplift: "low" } } });
  const medical = await client.medicalBoundaryCheck({ output: { side: "clinical", category: "treatment_selection", label: "choose a treatment" } });
  const posture = await client.safetyPosture({ include_threats: false });
  const measurement = await client.measurementCompare({ left: { label: "left" }, right: { label: "right" }, require_bound_terms: false });
  const hub = await client.hubSearch({ federation: { members: {} }, catalogs: [], query: { facets: [] }, max_items: 3 });
  const resolvedHub = await client.hubResolve({ federation: { members: {} }, catalogs: [], request: { name: "bioprism/root" } });
  const lockedHub = await client.hubLock({ federation: { members: {} }, catalogs: [], request: { name: "bioprism/root" }, max_items: 3 });
  const worldClaim = await client.worldClaimCheck({ provenance: { top: "mechanistic" }, claim: { kind: "biology", quantity: "tumour growth rate" } });
  const observedWorld = await client.observedWorldDeclare({ id: "observed-demo", sources: [], design: { cohort_size: 0 }, outcome_labels: [] });
  const lineage = await client.lineageAudit({ registry: { nodes: {}, artifacts: {} }, max_items: 3 });
  const preanalytic = await client.preanalyticApply({ specimen: { id: "sp-1" }, mutation: { id: "m-1" } });
  const contradiction = await client.contradictionReview({ left: {}, right: {}, intent: "resolvable", hypotheses: [{ id: "h-1", account: {} }] });
  const lab = await client.labPlan({ graph: {}, actions: [], budget: {}, max_items: 3 });
  const onco = await client.oncoBoundaryCheck({ request: { requested_uses: ["cohort_analysis"] } });
  const responseAssessment = await client.oncoResponseAssess({ criterion: {}, baseline: {}, current: {}, current_acquired: "2026-01-01T00:00:00Z", baseline_clinical: {}, current_clinical: {}, treatment: {} });
  const worldline = await client.oncoWorldlineView({ worldline: {}, visible_at: "2026-01-02T00:00:00Z" });
  const classification = await client.oncoClassificationCheck({ histology: "diffuse_glioma", panel: {} });
  const identity = await client.oncoworldsIdentityJoin({ left: {}, right: {}, unit: "specimen" });
  const outcome = await client.oncoOutcomeAnalyze({ follow_up: {}, estimand: {} });
  const oracleCombine = await client.oracleCombine({ subject: "s", at: "2026-01-01T00:00:00Z", judgements: [{}] });
  const oraclePanel = await client.oracleReferencePanel({ panel: {} });
  const oracleMissingness = await client.oracleMissingness({ pattern: {}, field: {}, boundary: {}, small_cell_floor: 5 });
  const referenceAudit = await client.bioevalReferenceAudit({ reference: {} });
  const evaluationWorldline = await client.evaluationWorldlineAudit({ worldline: {} });
  const reproduction = await client.evaluationReproductionCheck({ reexecution: {} });
  const trajectory = await client.evaluationTrajectoryCheck({ trajectory: {} });
  assert.equal(capabilities.mcp.result.structuredContent.workflow, "capability_discover");
  assert.equal(capabilities.mcp.result.structuredContent.catalog_digest.length, 64);
  assert.equal(capabilities.mcp.result.structuredContent.matches[0].group.domains[0], "verification");
  assert.equal(evidenceAudit.mcp.result.structuredContent.workflow, "biocapability_evidence_conditioned_profile");
  assert.equal(evidenceAudit.mcp.result.structuredContent.release_posture.ready_for_requested_claims, false);
  assert.equal(evidenceAudit.mcp.result.structuredContent.claim_requests.rows[0].fail_closed, true);
  assert.equal(publicationAudit.mcp.result.structuredContent.workflow, "bioatlas_publication_audit");
  assert.equal(publicationAudit.mcp.result.structuredContent.cross_layer.atlas_aggregation_ready, true);
  assert.equal(publicationAudit.mcp.result.structuredContent.release_request.targets[0].target, "atlas_profile");
  assert.equal(capabilityAudit.mcp.result.structuredContent.workflow, "capability_audit");
  assert.equal(capabilityAudit.mcp.result.structuredContent.healthy, true);
  assert.equal(capabilityAudit.mcp.result.structuredContent.catalog_digest.length, 64);
  assert.equal(capabilityAudit.mcp.result.structuredContent.schema_quality.valid, 1);
  const capabilityDashboard = await client.capabilityDashboard({ domain: "verification", include_tools: true });
  assert.equal(capabilityDashboard.mcp.result.structuredContent.workflow, "capability_dashboard");
  assert.equal(capabilityDashboard.mcp.result.structuredContent.capability_dashboard_ready, true);
  assert.equal(capabilityDashboard.mcp.result.structuredContent.audit.groups[0].readiness, "callable");
  assert.equal(capabilityDashboard.mcp.result.structuredContent.audit.groups[0].tools[0], "echo");
  assert.equal(route.mcp.result.structuredContent.workflow, "capability_route");
  assert.equal(route.mcp.result.structuredContent.execution, "not_started");
  assert.equal(route.mcp.result.structuredContent.route_coverage.needs_resolved, 1);
  assert.equal(routeReview.mcp.result.structuredContent.review_status, "ready");
  assert.equal(routeReview.mcp.result.structuredContent.review_id.length, 64);
  assert.deepEqual(routeReview.mcp.result.structuredContent.dependency_waves, [["oncology"]]);
  assert.equal(routeReview.mcp.result.structuredContent.schema_review.valid, true);
  assert.equal(adapter.mcp.result.structuredContent.workflow, "adapter_plan");
  assert.equal(adapter.mcp.result.structuredContent.plan.candidates[0].status, "ready");
  assert.equal(adapter.mcp.result.structuredContent.selected_adapter.id, "bioprism.tabular");
  assert.equal(tabular.mcp.result.structuredContent.conformance.verified, true);
  assert.equal(tabular.mcp.result.structuredContent.facts[0].value, "S1");
  assert.equal(conformance.mcp.result.structuredContent.release_decision.decision, "release");
  assert.equal(conformance.mcp.result.structuredContent.suite.pyramid.counts.unit, 1);
  assert.equal(release.mcp.result.structuredContent.release_ready, true);
  assert.equal(release.mcp.result.structuredContent.checks[0].advisory, false);
  assert.equal(operations.mcp.result.structuredContent.topologies.promise_parity.holds, true);
  assert.equal(operations.mcp.result.structuredContent.metrics.named_but_undefined, 117);
  assert.equal(acceptance.mcp.result.structuredContent.summary.is_decidable, false);
  assert.equal(safety.mcp.result.structuredContent.decision.decision, "cleared");
  assert.equal(medical.mcp.result.structuredContent.admitted, false);
  assert.equal(posture.mcp.result.structuredContent.coverage.unmitigated, 4);
  assert.equal(measurement.mcp.result.structuredContent.report.verdict.verdict, "comparable");
  assert.equal(hub.mcp.result.structuredContent.matches[0].authority.authority, "authoritative");
  assert.equal(resolvedHub.mcp.result.structuredContent.resolution.subject.digest, "sha256:root");
  assert.equal(lockedHub.mcp.result.structuredContent.entries[0].locked.required_by[0].source.source, "root");
  assert.equal(worldClaim.mcp.result.structuredContent.fail_closed, true);
  assert.equal(observedWorld.mcp.result.structuredContent.provenance.top, "observed");
  assert.equal(reproduction.mcp.result.structuredContent.schema, "bioprism-mcp/evaluation-reproduction-check/0.1");
  assert.equal(reproduction.mcp.result.structuredContent.diverged_count, 1);
  assert.equal(reproduction.mcp.result.structuredContent.first_divergence.output, "score");
  assert.equal(trajectory.mcp.result.structuredContent.schema, "bioprism-mcp/evaluation-trajectory-check/0.1");
  assert.equal(trajectory.mcp.result.structuredContent.property_outcomes[0].held, true);
  assert.equal(lineage.mcp.result.structuredContent.identity_complete, true);
  assert.equal(preanalytic.mcp.result.structuredContent.fail_closed, true);
  assert.equal(contradiction.mcp.result.structuredContent.stage, "pose");
  assert.equal(lab.mcp.result.structuredContent.should_escalate, true);
  assert.equal(onco.mcp.result.structuredContent.terminal_action, "escalate");
  assert.equal(onco.mcp.result.structuredContent.schema, "bioprism-mcp/onco-boundary-check/0.1");
  assert.equal(onco.mcp.result.structuredContent.disposition_kind, "release_partial");
  assert.equal(onco.mcp.result.structuredContent.escalation_present, true);
  assert.equal(onco.mcp.result.structuredContent.refused_count, 1);
  assert.equal(responseAssessment.mcp.result.structuredContent.withheld_progression, true);
  assert.equal(responseAssessment.mcp.result.structuredContent.call_label, "not evaluable");
  assert.equal(responseAssessment.mcp.result.structuredContent.schema, "bioprism-mcp/onco-response-assess/0.1");
  assert.equal(responseAssessment.mcp.result.structuredContent.call_kind, "not_evaluable");
  assert.equal(responseAssessment.mcp.result.structuredContent.criterion_divergence_present, true);
  assert.equal(worldline.mcp.result.structuredContent.record_order_differs, false);
  assert.equal(worldline.mcp.result.structuredContent.schema, "bioprism-mcp/onco-worldline-view/0.1");
  assert.equal(worldline.mcp.result.structuredContent.timepoints[0].clocks.visible, "2026-01-01T00:00:00Z");
  assert.equal(worldline.mcp.result.structuredContent.visibility_partition.visible_count, 1);
  assert.equal(classification.mcp.result.structuredContent.is_integrated, false);
  assert.equal(classification.mcp.result.structuredContent.schema, "bioprism-mcp/onco-classification-check/0.1");
  assert.equal(classification.mcp.result.structuredContent.resolution_kind, "unresolved");
  assert.equal(classification.mcp.result.structuredContent.unobserved_panel_state_count, 1);
  assert.equal(identity.mcp.result.structuredContent.joinable, false);
  assert.equal(identity.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-identity-join/0.1");
  assert.equal(identity.mcp.result.structuredContent.refusal_kind, "no_identity_evidence");
  assert.equal(identity.mcp.result.structuredContent.identity_link_count, 0);
  assert.equal(outcome.mcp.result.structuredContent.censoring_reason, "lost_to_follow_up");
  assert.equal(outcome.mcp.result.structuredContent.schema, "bioprism-mcp/onco-outcome-analyze/0.1");
  assert.equal(outcome.mcp.result.structuredContent.analysis.estimand.endpoint, "time_to_progression");
  assert.equal(outcome.mcp.result.structuredContent.bias_count, 2);
  assert.equal(oracleCombine.mcp.result.structuredContent.status, "underdetermined");
  assert.equal(oracleCombine.mcp.result.structuredContent.schema, "bioprism-mcp/oracle-combine/0.1");
  assert.equal(oracleCombine.mcp.result.structuredContent.contributing[0].oracle.version.major, 1);
  assert.equal(oracleCombine.mcp.result.structuredContent.suppressed[0].rule, "nondeterministic_over_grounded");
  assert.equal(oracleCombine.mcp.result.structuredContent.disagreements[0].resolution.resolution, "open");
  assert.equal(oraclePanel.mcp.result.structuredContent.rule_label, "majority");
  assert.equal(oracleMissingness.mcp.result.structuredContent.small_cell_floor, 5);
  assert.equal(referenceAudit.mcp.result.structuredContent.reference_kind, "distribution");
  assert.equal(referenceAudit.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-reference-audit/0.1");
  assert.equal(referenceAudit.mcp.result.structuredContent.reference.mass.progression, 0.6);
  assert.equal(referenceAudit.mcp.result.structuredContent.resolution.resolution, "distributed");
  assert.equal(evaluationWorldline.mcp.result.structuredContent.leak_count, 1);
  assert.equal(evaluationWorldline.mcp.result.structuredContent.schema, "bioprism-mcp/evaluation-worldline-audit/0.1");
  assert.equal(evaluationWorldline.mcp.result.structuredContent.leaks[0].clock, "accessible");
  assert.equal(evaluationWorldline.mcp.result.structuredContent.dangling_references[0][1], "missing");
  assert.equal(reproduction.mcp.result.structuredContent.reproduced, false);
  assert.equal(trajectory.mcp.result.structuredContent.steps, 1);
  const mission = await client.agentMission({ mission_id: "mission-1", goal: "discover", steps: [{ id: "catalog", domain: "workspace", capability: "discovery", objective: "discover", tool: "workspace_capabilities" }] });
  assert.equal(mission.mcp.result.structuredContent.workflow, "agent_mission");
  assert.equal(mission.mcp.result.structuredContent.execution_trace[0].event, "mission.started");
  assert.equal(mission.mcp.result.structuredContent.execution_trace.at(-1).event, "mission.completed");
  await assert.rejects(client.callTool("unsafe/name"), ArgumentError);
  await assert.rejects(async () => client.requireToolSuccess(await client.callTool("refuse")), ToolRefusalError);
});

test("client exposes typed literature binding and citation refusal evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse({
      ok: true,
      tool: "literature_bind_check",
      request_id: "literature-1",
      mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/literature-bind-check/0.1",
        outcome_kind: "cite_refused",
        bound: true,
        citable: false,
        evidence: {
          outcome_kind: "cite_refused",
          bound: true,
          citable: false,
          refusal: null,
          refusal_kind: null,
          citation_refusal: { unsupported: "biological_measurement" },
          citation_refusal_kind: "unsupported",
        },
        guarantees: ["source binding is separate from citation support"],
        limitations: ["no external literature retrieval"],
      } } },
    }),
  });
  const result = await client.literatureBindCheck({
    claim: { text: "a bounded source claim" },
    target: { disease: "diffuse_glioma" },
    at_tier: "primary",
    horizon: { kind: "open" },
    claim_kind: "published_claim_support",
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/literature-bind-check/0.1");
  assert.equal(result.mcp.result.structuredContent.outcome_kind, "cite_refused");
  assert.equal(result.mcp.result.structuredContent.evidence.citation_refusal_kind, "unsupported");
});

test("client exposes typed modality support and pseudoreplication evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse({
      ok: true,
      tool: "modality_support_check",
      request_id: "modality-1",
      mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/modality-support-check/0.1",
        outcome_kind: "refused",
        modality: "bulk_transcriptomics",
        claim: "cell_intrinsic_change",
        supported: false,
        claim_requirements: { axes: ["cell"] },
        support: {
          supported: false,
          refusal: { unsupported: "missing_resolution" },
          refusal_kind: "named_failure_mode",
          root_refusal_kind: "missing_resolution",
        },
        analysis_unit: {
          requested: true,
          counted: "population",
          independent: "subject",
          admissible: false,
          refusal: { unsupported: "named_failure_mode" },
          refusal_kind: "named_failure_mode",
        },
        descriptor: { complete: true, supported_catalogue_claims: ["population_average"] },
      } } },
    }),
  });
  const result = await client.modalitySupportCheck({
    modality: "bulk_transcriptomics",
    claim: "cell_intrinsic_change",
    counted_unit: "population",
  });
  assert.equal(result.mcp.result.structuredContent.outcome_kind, "refused");
  assert.equal(result.mcp.result.structuredContent.support.root_refusal_kind, "missing_resolution");
  assert.equal(result.mcp.result.structuredContent.analysis_unit.admissible, false);
});

test("client exposes typed modality transport loss and inverse evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse({
      ok: true,
      tool: "modality_transport_check",
      request_id: "transport-1",
      mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/modality-transport-check/0.1",
        outcome_kind: "constructed",
        constructed: true,
        from: "single_cell",
        to: "bulk_transcriptomics",
        axis: "cell",
        transport: { kind: "aggregation", operator: "mean" },
        fidelity: { fidelity: "exact" },
        loss: { discarded: ["cell distribution"] },
        scope_mapping: {},
        scope_mapping_check: "sound",
        inverse: { invertible: false, refusal_kind: "not_invertible" },
        application: { applied: true },
        applied_descriptor: {},
        claims: [{ claim: "cell_intrinsic_change", support_lost: true }],
      } } },
    }),
  });
  const result = await client.modalityTransportCheck({
    from: "single_cell",
    to: "bulk_transcriptomics",
    axis: "cell",
    transport: { kind: "aggregation", operator: "mean" },
    claims: ["cell_intrinsic_change"],
  });
  assert.equal(result.mcp.result.structuredContent.fidelity.fidelity, "exact");
  assert.equal(result.mcp.result.structuredContent.inverse.invertible, false);
  assert.equal(result.mcp.result.structuredContent.claims[0].support_lost, true);
});

test("client exposes modality-aware comparability refusals and report digests", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse({
      ok: true,
      tool: "modality_comparability_check",
      request_id: "comparability-1",
      mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/modality-comparability-check/0.1",
        outcome_kind: "blocked",
        comparable: false,
        policy: {},
        check_order: ["measurand", "reported resolution axis", "status of that axis"],
        left: { modality: "bulk_transcriptomics", measurand: "RNA abundance" },
        right: { modality: "proteomics", measurand: "protein abundance" },
        report: { verdict: { comparable: false } },
        verdict: { reason: { blocked_by: "measurand_mismatch" } },
        report_sha256: "a".repeat(64),
      } } },
    }),
  });
  const result = await client.modalityComparabilityCheck({
    left: { descriptor: {}, reported_at: "population", measurement: {} },
    right: { descriptor: {}, reported_at: "population", measurement: {} },
  });
  assert.equal(result.mcp.result.structuredContent.outcome_kind, "blocked");
  assert.equal(result.mcp.result.structuredContent.verdict.reason.blocked_by, "measurand_mismatch");
  assert.equal(result.mcp.result.structuredContent.report_sha256.length, 64);
});

test("client exposes fail-closed obligation gate decisions and graph projections", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse({
      ok: true,
      tool: "obligation_gate_check",
      request_id: "gate-1",
      mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/obligation-gate-check/0.1",
        outcome_kind: "blocked",
        allowed: false,
        goal: "publish a validation report",
        action: { id: "publish", regret: "irreversible" },
        gate: { gate: "blocked", reason: { reason: "mandatory_obligation_outstanding", obligation: "consent" } },
        refusal: { reason: "mandatory_obligation_outstanding", obligation: "consent" },
        graph: { valid: true, sha256: "a".repeat(64), obligation_count: 3, frontier: [{ obligation: "consent" }] },
      } } },
    }),
  });
  const result = await client.obligationGateCheck({
    graph: { goal: "publish a validation report", obligations: {} },
    action: { id: "publish", regret: "irreversible", prerequisites: [] },
    max_items: 10,
  });
  assert.equal(result.mcp.result.structuredContent.outcome_kind, "blocked");
  assert.equal(result.mcp.result.structuredContent.refusal.obligation, "consent");
  assert.equal(result.mcp.result.structuredContent.graph.valid, true);
});

test("client parses cursor SSE and validates webhook mutations", async () => {
  const client = new ApiClient({
    baseUrl: "https://example.test",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      if (path.endsWith("/deliveries")) {
        return jsonResponse({ ok: true, page: { deliveries: [{ delivery_id: 1, subscription_id: "sub", attempt: 1, state: "failed", last_error: "blocked", last_error_retryable: false, event_id: 2, event_type: "tool.completed", signature: "sha256=x", envelope: { delivery_id: 1, subscription_id: "sub", attempt: 1, event: { id: 2, event_type: "tool.completed", subject: "tool", request_id: "req", payload: {} }, signature: "sha256=x" } }], after: 0, next_after: 1, pending_count: 1, dropped_deliveries: 0 } });
      }
      if (path.endsWith("/replay") && init.method === "POST") {
        return jsonResponse({ ok: true, replayed: [{ delivery_id: 1, subscription_id: "sub", attempt: 1, state: "pending", last_error: null, last_error_retryable: null, event_id: 2, event_type: "tool.completed", signature: "sha256=x", envelope: {} }] });
      }
      if (path.startsWith("/v1/route-reviews/")) return jsonResponse({ ok: true, workflow: "capability_route_review_evidence", review_id: "a".repeat(64), found: true, page: { events: [{ id: 1, event_type: "tool.completed", subject: "capability_route_review", request_id: "req-1", payload: {} }], after: 0, next_after: 1, oldest: 1, newest: 1, gap: false, dropped_events: 0 } });
      return new Response("id: 4\nevent: tool.completed\ndata: {\"ok\":true}\n\nevent: cursor_gap\ndata: {\"after\":0}\n\n", {
      headers: { "content-type": "text/event-stream", "x-next-after": "4" },
      });
    },
  });
  const snapshot = await client.eventStream(0, 10);
  assert.equal(snapshot.nextAfter, 4);
  assert.equal(snapshot.events[0].event, "tool.completed");
  assert.deepEqual(JSON.parse(snapshot.events[1].data), { after: 0 });
  const evidence = await client.routeReviewEvidence("a".repeat(64));
  assert.equal(evidence.workflow, "capability_route_review_evidence");
  assert.equal(evidence.found, true);
  assert.equal(evidence.page.events[0].subject, "capability_route_review");
  await assert.rejects(client.routeReviewEvidence("invalid"), ArgumentError);
  assert.deepEqual(parseSse("data: a\ndata: b\n\n"), [{ data: "a\nb" }]);
  assert.throws(() => parseSse("retry: nope\n\n"), /retry/);
  const deliveries = await client.deliveries("sub");
  assert.equal(deliveries.page.deliveries[0].state, "failed");
  const replayed = await client.replay("sub", [1]);
  assert.equal(replayed.replayed[0].state, "pending");
  await assert.rejects(client.acknowledge("sub", [0]), ArgumentError);
});

test("client exposes asynchronous mission submission, status, and cancellation", async () => {
  const client = new ApiClient({
    baseUrl: "https://example.test",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      if (path === "/v1/missions" && init.method === "POST") {
        return jsonResponse({ ok: true, mission_id: "async-1", status: "queued", cancel_requested: false });
      }
      if (path === "/v1/missions/preflight" && init.method === "POST") {
        return jsonResponse({ ok: true, workflow: "agent_mission", execution: "planned", mission_status: "planned", preflight: true, dispatch: "not_started", results: [] });
      }
      if (path === "/v1/missions/async-1" && init.method === "GET") {
        return jsonResponse({ ok: true, mission_id: "async-1", status: "succeeded", cancel_requested: false, progress: { phase: "succeeded", current_wave: 0, total_steps: 1, completed_steps: 1, active_steps: 0, succeeded: 1, refused: 0, blocked: 0, cancelled: 0, required_failures: 0, returned_bytes: 14, trace_sequence: 4, last_event: "mission.completed" }, result: { mission_status: "succeeded" } });
      }
      if (path === "/v1/missions/slow" && init.method === "GET") {
        return jsonResponse({ ok: true, mission_id: "slow", status: "running", cancel_requested: false, progress: { phase: "running", current_wave: 0, total_steps: 1, completed_steps: 0, active_steps: 1, succeeded: 0, refused: 0, blocked: 0, cancelled: 0, required_failures: 0, returned_bytes: 0, trace_sequence: 1, last_event: "step.started" } });
      }
      if (path === "/v1/missions/async-1/trace" && init.method === "GET") {
        return jsonResponse({ ok: true, mission_id: "async-1", trace_schema_version: "bioprism-devplat-mission-trace/0.1", events: [{ sequence: 0, event: "mission.started", wave: null, step_id: null, tool: null, status: "running", arguments_digest: null, bytes: 0, detail: null }, { sequence: 1, event: "mission.completed", wave: null, step_id: null, tool: null, status: "succeeded", arguments_digest: null, bytes: 14, detail: null }], after: 0, next_after: 2, oldest: 0, newest: 1, gap: false, dropped_events: 0, terminal: true, limit: 100, truncated: false });
      }
      if (path === "/v1/missions/async-1/cancel" && init.method === "POST") {
        return jsonResponse({ ok: true, mission_id: "async-1", status: "running", cancel_requested: true, cancel_reason: "operator stop" }, 202);
      }
      if (path === "/v1/missions" && init.method === "GET") {
        return jsonResponse({ ok: true, missions: [{ mission_id: "async-1", status: "succeeded", cancel_requested: false, progress: { phase: "succeeded", current_wave: 0, total_steps: 1, completed_steps: 1, active_steps: 0, succeeded: 1, refused: 0, blocked: 0, cancelled: 0, required_failures: 0, returned_bytes: 14, trace_sequence: 4, last_event: "mission.completed" }, summary: { total_steps: 1, completed_steps: 1, succeeded: 1, refused: 0, blocked: 0, cancelled: 0, required_failures: 0, returned_bytes: 14, result_available: true }, poll: "/v1/missions/async-1", cancel: "/v1/missions/async-1/cancel" }], returned: 1, total_matching: 1, limit: 5, truncated: false, status_filter: "succeeded" });
      }
      return jsonResponse({ ok: false, error: { code: "not_found", message: path } }, 404);
    },
  });
  const preflight = await client.preflightMission({ mission_id: "preflight-1", goal: "plan", steps: [] });
  assert.equal(preflight.preflight, true);
  assert.equal(preflight.dispatch, "not_started");
  const submitted = await client.submitMission({ mission_id: "async-1", goal: "run", steps: [] });
  assert.equal(submitted.status, "queued");
  const status = await client.missionStatus("async-1");
  assert.equal(status.status, "succeeded");
  assert.equal(status.progress.phase, "succeeded");
  assert.equal(status.progress.completed_steps, 1);
  assert.equal(status.progress.last_event, "mission.completed");
  const waited = await client.waitMission("async-1", { timeoutMs: 1_000, pollIntervalMs: 10 });
  assert.equal(waited.status, "succeeded");
  const trace = await client.missionTrace("async-1");
  assert.equal(trace.events[0].event, "mission.started");
  assert.equal(trace.events[1].event, "mission.completed");
  assert.equal(trace.next_after, 2);
  assert.equal(status.result.mission_status, "succeeded");
  const inventory = await client.missions("succeeded", 5);
  assert.equal(inventory.missions[0].mission_id, "async-1");
  assert.equal(inventory.missions[0].progress.completed_steps, 1);
  const cancelled = await client.cancelMission("async-1", "operator stop");
  assert.equal(cancelled.cancel_requested, true);
  assert.equal(cancelled.cancel_reason, "operator stop");
  await assert.rejects(client.waitMission("async-1", { timeoutMs: 0 }), ArgumentError);
  await assert.rejects(
    client.waitMission("slow", { timeoutMs: 1, pollIntervalMs: 1 }),
    (error) => error instanceof MissionWaitTimeoutError && error.lastJob.status === "running",
  );
});

test("tool catalogue keeps unsupported schema features visible", async () => {
  const catalogue = await ToolCatalogue.fromDefinitions([
    {
      name: "union",
      description: "test",
      inputSchema: {
        type: "object",
        properties: { value: { anyOf: [{ type: "string" }, { type: "integer" }] } },
        dependentSchemas: { value: { required: ["other"] } },
      },
    },
  ]);
  const accepted = catalogue.validate("union", { value: 3 });
  assert.equal(accepted.ok, true);
  assert.equal(accepted.fullyChecked, false);
  assert.equal(accepted.warnings.some((issue) => issue.code === "unsupported_schema_keyword"), true);
  const rejected = catalogue.validate("union", { value: [] });
  assert.equal(rejected.ok, false);
  assert.equal(rejected.issues.some((issue) => issue.code === "anyOf_no_match"), true);
});

test("structured HTTP errors and response ceilings stay typed", async () => {
  const failing = new ApiClient({
    baseUrl: "http://example.test",
    fetch: async () => jsonResponse({ ok: false, request_id: "r9", error: { code: "refused", message: "no" } }, 422, { "x-request-id": "r9" }),
  });
  await assert.rejects(failing.health(), (error) => error instanceof ApiError && error.status === 422 && error.requestId === "r9");

  const bounded = new ApiClient({
    baseUrl: "http://example.test",
    maxResponseBytes: 8,
    fetch: async () => new Response("0123456789"),
  });
  await assert.rejects(bounded.health(), ResponseTooLargeError);
});

test("client exposes runtime and bioethics safety workflows with their exact tool names", async () => {
  const calls = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      const tool = path.split("/").pop();
      calls.push({ tool, body: JSON.parse(init.body) });
      const projections = {
        runtime_effect_check: { ok: true, request: { kind: "clock_now" }, kind: "clock_now", class: "pure", class_label: "pure", authorization: "perform", simulated_outcome: null, guarantees: [], limitations: [] },
        runtime_tape_verify: { ok: true, schema: "bioprism-mcp/runtime-tape-verify/0.1", run: "run-1", lineage: null, entries: 0, head: "", chain_verified: true, checkpoint_results: [], checkpoint_count: 0, checkpoint_pass_count: 0, checkpoint_failure_count: 0, artifacts: { consumed: [], created: {} }, artifact_consumed_count: 0, artifact_created_count: 0, simulated_steps: [], simulated_step_count: 0, first_divergence: null, comparison_supplied: false, guarantees: [], limitations: [] },
        runtime_execution_simulate: { ok: true, schema: "bioprism-mcp/runtime-execution-simulate/0.1", run: "run-1", request_count: 1, recorded_requests: 1, recording_complete: true, partial_recording: false, live_outcomes: [], live_outcome_count: 0, execution_error: null, tape: {}, world: { calls: 0, task_millis: 0, state_manifest: {}, file_changes: [] }, policy_journal: [], policy_journal_count: 0, budget: null, replay: { verified: true, matched: true, outcomes: [], outcome_count: 0, complete: true, error: null }, replay_outcome_count: 0, replay_complete: true, fork: null, fork_requested: false, guarantees: [], limitations: [] },
        bioethics_action_review: { ok: true, subject: "study", declared_use: "cohort_analysis", permitted_uses: ["cohort_analysis"], disposition: {}, physical_step_count: 0, in_silico_step_count: 0, requires_external_authorisation: false, referral: { executes_physical_action: false }, guarantees: [] },
        bioethics_human_subject_screen: { ok: true, subject: "study", determination: { determination: "undetermined" }, requires_institutional_review: false, triggers: [], consent: { status: "not_run" }, return_of_results: { status: "admitted" }, clearance_issued: false, guarantees: [] },
        bioethics_dual_use_review: { ok: true, subject: "capability", surfaces: [], assessor: "reviewer", sensitive_category: "biological_design", decision: { decision: "cleared" }, referral: {}, withholding: { status: "not_requested" }, guarantees: [] },
        bioethics_validation_check: { ok: true, subject: "module", author: "author", maturity: "experimental", missing: [], missing_count: 0, verification: { status: "refused", fail_closed: true }, guarantees: [] },
        bioethics_representation_audit: { ok: true, summary: { measured: [], unmeasured: [], suppressed: [] }, measured_count: 0, unmeasured_count: 0, suppressed_count: 0, complete: true, incomplete_axes: [], attribution: { status: "not_requested" }, guarantees: [] },
      }[tool];
      return jsonResponse({ ok: true, tool, request_id: "safety-1", mcp: { result: { structuredContent: projections } }, guarantee: "bounded" });
    },
  });
  const options = { policy: {}, request: { kind: "clock_now" } };
  const tape = await client.runtimeTapeVerify({ tape: {} });
  const effect = await client.runtimeEffectCheck(options);
  const simulation = await client.runtimeExecutionSimulate({ policy: {}, requests: [] });
  const action = await client.bioethicsActionReview({ plan: {} });
  const human = await client.humanSubjectScreen({ study: {} });
  const dual = await client.bioethicsDualUseReview({ release: {}, risk: {} });
  const validation = await client.bioethicsValidationCheck({ dossier: {} });
  const representation = await client.bioethicsRepresentationAudit({ subject: "study", observations: [] });
  assert.equal(effect.mcp.result.structuredContent.authorization, "perform");
  assert.equal(tape.mcp.result.structuredContent.schema, "bioprism-mcp/runtime-tape-verify/0.1");
  assert.equal(tape.mcp.result.structuredContent.chain_verified, true);
  assert.equal(simulation.mcp.result.structuredContent.replay.verified, true);
  assert.equal(simulation.mcp.result.structuredContent.schema, "bioprism-mcp/runtime-execution-simulate/0.1");
  assert.equal(simulation.mcp.result.structuredContent.recording_complete, true);
  assert.equal(action.mcp.result.structuredContent.referral.executes_physical_action, false);
  assert.equal(human.mcp.result.structuredContent.clearance_issued, false);
  assert.equal(dual.mcp.result.structuredContent.decision.decision, "cleared");
  assert.equal(validation.mcp.result.structuredContent.verification.fail_closed, true);
  assert.equal(representation.mcp.result.structuredContent.summary.unmeasured.length, 0);
  assert.deepEqual(calls.map((call) => call.tool), [
    "runtime_tape_verify",
    "runtime_effect_check",
    "runtime_execution_simulate",
    "bioethics_action_review",
    "bioethics_human_subject_screen",
    "bioethics_dual_use_review",
    "bioethics_validation_check",
    "bioethics_representation_audit",
  ]);
});

test("client exposes the complete typed OncoWorlds transport family", async () => {
  const calls = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      const tool = path.split("/").pop();
      calls.push({ tool, body: JSON.parse(init.body) });
      const projections = {
        oncoworlds_model_transport: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-model-transport/0.1",
          supported: true,
          outcome_kind: "supported",
          model_statement: "effect",
          effect: "the compound reduced viability",
          model_identity: { model: "ORG-1", system: "organoid", source_specimen: "S-1", passage: 3, verified_against_source: true },
          rests_on: ["genomic"],
          fidelity_axes: [{ axis: "genomic", passage: 3, measured: true }],
          establishment: { attempted: 3, established: 3, selected: false, selection_modelled: false },
          replicates: { technical_wells: 6, biological_replicates: 3, effective_biological_n: 3, claimed_n: 3 },
          transport_assumption_names: [],
          required_assumptions: [],
          effective_biological_n: 3,
          patient_relevant_claim: { result: {}, cohort: {}, transport: {}, claimed_n: 3 },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_methylation_classify: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-methylation-classify/0.1",
          outcome_kind: "unclassifiable",
          classified: false,
          class: null,
          classifier: { name: "methylation-demo", version: "v1", reference_version: "ref-1", reporting_threshold: 7000 },
          classifier_threshold: 7000,
          threshold_declared: true,
          qc: { qc: "passed" },
          tumour_content: { unobserved: "not_collected" },
          score_count: 1,
          score_classes: ["class-a"],
          caveat_count: 1,
          nearest_present: true,
          report: { outcome: { outcome: "unclassifiable", reason: { reason: "no_class_above_threshold" }, nearest: { label_only: "class-a" }, }, caveats: ["tumour content is not measured"] },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_methylation_compare: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-methylation-compare/0.1",
          divergence_kind: "version_conditioned",
          classifier_changed: true,
          left_outcome_kind: "classified",
          right_outcome_kind: "unclassifiable",
          stable_evidence_count: 0,
          comparison: { divergence: { divergence: "version_conditioned", under_left: "class-a", under_right: null }, stable_evidence: [] },
          left_classifier: { version: "v1" },
          right_classifier: { version: "v2" },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_radiogenomic_check: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-radiogenomic-check/0.1",
          supported: true,
          outcome_kind: "supported",
          claim_target: "association",
          claim_statement: "supported",
          design: {
            split_unit: "participant",
            feature_provenance: "fitted_on_training_split_only",
            feature_version: "features-v1",
            external_cohort: null,
            strata: [],
            mechanism_strata_present: false,
          },
          transport_assumption_names: [],
          required_assumptions: [],
          supported_claim: {
            claim: { target: "association", statement: "supported" },
            label: { marker: "idh_mutation" },
            strata: [],
            transport: {},
          },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_clonal_history_check: { ok: true, schema: "bioprism-mcp/oncoworlds-clonal-history-check/0.1", compatible_count: 1, rejected_count: 0, candidate_count: 1, compatible: [{ edges: [] }], rejected: [], rejected_records: [], unique_history: { ok: true, history: { edges: [] } }, unique_status: "unique", guarantees: [], limitations: [] },
        oncoworlds_clonal_evidence_check: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1",
          outcome_kind: "report",
          all_admissible: false,
          check_count: 2,
          refusal_count: 1,
          checks: {
            promotion: { allowed: true, outcome_kind: "present_in_sampled_regions", refusal: null, refusal_kind: null },
            attribution: { allowed: false, outcome_kind: "refused", refusal: { refusal: "unsupported_directionality" }, refusal_kind: "unsupported_directionality" },
          },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_era_shift_check: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-era-shift-check/0.1",
          outcome_kind: "comparable",
          comparable: true,
          evidence: {
            left: { name: "historical", site: "site-a", classification_version: "criteria-a", entities: ["entity-1"] },
            right: { name: "current", site: "site-b", classification_version: "criteria-b", entities: ["entity-1a"] },
            mapping: { from: "criteria-a", to: "criteria-b", fates: { "entity-1": { fate: "renamed", to: "entity-1a" } } },
            mapping_declared: true,
            mapping_fate_count: 1,
            mapping_versions_match: true,
            same_classification_version: false,
            left_entity_count: 1,
            right_entity_count: 1,
            assay_contexts: [{ site: "site-b", assay: "methylation", availability: { availability: "unavailable_at_site" }, observation: {}, negative_call_supported: false, negative_call_refusal: { refusal: "resource_absence_read_as_biology" }, negative_call_refusal_kind: "resource_absence_read_as_biology" }],
            assay_context_count: 1,
            descriptor_checks: [{ descriptor: "self_reported_race_or_ethnicity", descriptor_label: "self-reported race or ethnicity", use: "stratification", use_label: "a stratification variable", administrative: true, allowed: true }],
            descriptor_check_count: 1,
          },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_equity_check: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-equity-check/0.1",
          outcome_kind: "equity_report",
          equity_supported: true,
          pooled_value: 0.91,
          subgroups: [{ subgroup: "group-a", n: 900, estimate: 0.93, interval: { low: 0.90, high: 0.95 } }],
          subgroup_count: 1,
          interval_count: 1,
          all_intervals_present: true,
          report: { pooled: 0.91 },
          guarantees: [],
          limitations: [],
        },
        oncoworlds_entity_world_check: {
          ok: true,
          schema: "bioprism-mcp/oncoworlds-entity-world-check/0.1",
          outcome_kind: "report",
          all_admissible: false,
          check_count: 2,
          refusal_count: 1,
          checks: {
            provenance: { allowed: false, refusal: { refusal: "unmodelled_provenance_selection" }, refusal_kind: "unmodelled_provenance_selection" },
            benchmark: { allowed: true, feasibility: { feasibility: "feasible" }, feasibility_kind: "feasible", refusal: null, refusal_kind: null },
          },
          guarantees: [],
          limitations: [],
        },
      }[tool];
      return jsonResponse({ ok: true, tool, request_id: "oncoworlds-1", mcp: { result: { structuredContent: projections } }, guarantee: "bounded" });
    },
  });
  const model = await client.oncoworldsModelTransport({ result: {}, establishment: {}, claimed_n: 3, transport: {} });
  const classify = await client.oncoworldsMethylationClassify({ classifier: {}, scores: {}, context: {} });
  const compare = await client.oncoworldsMethylationCompare({ left: {}, right: {} });
  const radiogenomic = await client.oncoworldsRadiogenomicCheck({ claim: {}, design: {}, observation: {}, transport: {} });
  const clonal = await client.oncoworldsClonalHistoryCheck({ population: {}, candidates: [] });
  const clonalEvidence = await client.oncoworldsClonalEvidenceCheck({ promotion: {} });
  const eraShift = await client.oncoworldsEraShiftCheck({ left: {}, right: {} });
  const equity = await client.oncoworldsEquityCheck({ pooled: {} });
  const entityWorld = await client.oncoworldsEntityWorldCheck({ provenance: {} });
  assert.equal(model.mcp.result.structuredContent.patient_relevant_claim.claimed_n, 3);
  assert.equal(model.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-model-transport/0.1");
  assert.equal(model.mcp.result.structuredContent.model_identity.verified_against_source, true);
  assert.equal(model.mcp.result.structuredContent.replicates.effective_biological_n, 3);
  assert.equal(classify.mcp.result.structuredContent.classified, false);
  assert.equal(classify.mcp.result.structuredContent.outcome_kind, "unclassifiable");
  assert.equal(classify.mcp.result.structuredContent.score_count, 1);
  assert.equal(compare.mcp.result.structuredContent.comparison.divergence.divergence, "version_conditioned");
  assert.equal(compare.mcp.result.structuredContent.divergence_kind, "version_conditioned");
  assert.equal(compare.mcp.result.structuredContent.classifier_changed, true);
  assert.equal(radiogenomic.mcp.result.structuredContent.supported_claim.claim.statement, "supported");
  assert.equal(radiogenomic.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-radiogenomic-check/0.1");
  assert.equal(radiogenomic.mcp.result.structuredContent.outcome_kind, "supported");
  assert.equal(radiogenomic.mcp.result.structuredContent.design.split_unit, "participant");
  assert.equal(clonal.mcp.result.structuredContent.unique_history.ok, true);
  assert.equal(clonal.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-clonal-history-check/0.1");
  assert.equal(clonal.mcp.result.structuredContent.unique_status, "unique");
  assert.equal(clonalEvidence.mcp.result.structuredContent.checks.attribution.refusal_kind, "unsupported_directionality");
  assert.equal(clonalEvidence.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1");
  assert.equal(eraShift.mcp.result.structuredContent.evidence.mapping_fate_count, 1);
  assert.equal(eraShift.mcp.result.structuredContent.evidence.assay_contexts[0].negative_call_supported, false);
  assert.equal(eraShift.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-era-shift-check/0.1");
  assert.equal(equity.mcp.result.structuredContent.all_intervals_present, true);
  assert.equal(equity.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-equity-check/0.1");
  assert.equal(entityWorld.mcp.result.structuredContent.checks.provenance.refusal_kind, "unmodelled_provenance_selection");
  assert.equal(entityWorld.mcp.result.structuredContent.checks.benchmark.feasibility_kind, "feasible");
  assert.equal(entityWorld.mcp.result.structuredContent.schema, "bioprism-mcp/oncoworlds-entity-world-check/0.1");
  assert.deepEqual(calls.map((call) => call.tool), [
    "oncoworlds_model_transport",
    "oncoworlds_methylation_classify",
    "oncoworlds_methylation_compare",
    "oncoworlds_radiogenomic_check",
    "oncoworlds_clonal_history_check",
    "oncoworlds_clonal_evidence_check",
    "oncoworlds_era_shift_check",
    "oncoworlds_equity_check",
    "oncoworlds_entity_world_check",
  ]);
});

test("client exposes typed biological stress profile and report workflows", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    bearerToken: "0123456789abcdef",
    fetch: async (input) => {
      const path = new URL(String(input)).pathname;
      if (path === "/v1/tools/stress_profile") {
        return jsonResponse({ ok: true, tool: "stress_profile", request_id: "stress-1", mcp: { result: { structuredContent: {
          ok: true,
          headline: "batch effect profile",
          profile: {
            family: "batch_effect",
            blueprint_module: "32.06",
            stress_id: "site-offset",
            cohort_id: "cohort-1",
            parent_digest: "sha256:parent",
            identifiability: { identifiability: "separable", batch: "site-a", overlap: 0.5 },
            sweep: [{ magnitude: 125, effective_n: 4, nominal_n: 4, unresolved: 0, analysable_prevalence: 0.5, abandoned: false }],
            findings: [],
            generator_defects: [],
            caveat: "finite ladder",
          },
          guarantees: [],
          limitations: [],
        } } } });
      }
      if (path === "/v1/tools/stress_report") {
        return jsonResponse({ ok: true, tool: "stress_report", request_id: "stress-2", mcp: { result: { structuredContent: {
          ok: true,
          headline: "stress report",
          report: { cohort_id: "cohort-1", profiles: [] },
          worst_family: null,
          guarantees: [],
          limitations: [],
        } } } });
      }
      throw new Error(`unexpected path ${path}`);
    },
  });
  const profile = await client.stressProfile({ cohort: {}, stress: {} });
  assert.equal(profile.mcp.result.structuredContent.profile.family, "batch_effect");
  const report = await client.stressReport({ cohort: {}, stresses: [] });
  assert.equal(report.mcp.result.structuredContent.report.cohort_id, "cohort-1");
});

test("client exposes typed influence bounds and explicit unknown estimates", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      const path = new URL(String(input)).pathname;
      assert.equal(path, "/v1/tools/influence_analyze");
      return jsonResponse({ ok: true, tool: "influence_analyze", request_id: "influence-1", mcp: { result: { structuredContent: {
        ok: true,
        region: {
          label: "small-region",
          variables: { a: 2 },
          free: ["a"],
          bound: [],
          factors: [{ id: "f.a", scope: ["a"], arity: 1, has_table: false }],
          has_tables: false,
          joint_entries: 2,
          free_entries: 2,
          assumed_cardinality_fraction: 0,
        },
        execute: false,
        analysis: {
          subject: ["f.a"],
          perturbation: { class: "removal" },
          estimate: { kind: "unknown", reason: { reason: "no_factor_table", factor: "f.a" } },
          attempted: [{ method: "dynamic_range", declined: { reason: "no_factor_table", factor: "f.a" } }],
        },
        looseness: null,
        guarantees: ["unknown remains unknown"],
      } } } });
    },
  });
  const result = await client.influenceAnalyze({
    label: "small-region",
    variables: { a: 2 },
    factors: [{ id: "f.a", scope: ["a"] }],
    free: ["a"],
    factor: "f.a",
    perturbation: { class: "removal" },
  });
  assert.equal(result.mcp.result.structuredContent.analysis.estimate.kind, "unknown");
  assert.equal(result.mcp.result.structuredContent.execute, false);
});

test("client exposes typed routing abstention and holdout posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/routing_decide");
      return jsonResponse({ ok: true, tool: "routing_decide", request_id: "routing-1", mcp: { result: { structuredContent: {
        ok: true,
        decision: {
          architecture: { kind: "full_context" },
          confidence: 0,
          abstained: true,
          reason: { reason: "insufficient_coverage", eligible_architectures: 0, neighbouring_observations: 0 },
          considered: [],
        },
        task_id: null,
        holdout_check: "caller_must_supply_unseen_identity",
        evidence: { observations: 0, distinct_tasks: 0, neighbourhood_observations: 0, neighbourhood_radius: 3 },
        guarantees: ["safe default"],
      } } } });
    },
  });
  const result = await client.routingDecide({ fingerprint: {}, evidence: [], policy: {} });
  assert.equal(result.mcp.result.structuredContent.decision.abstained, true);
  assert.equal(result.mcp.result.structuredContent.holdout_check, "caller_must_supply_unseen_identity");
});

test("client exposes offline routing lab regret and holdout evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/routing_lab_run");
      return jsonResponse({ ok: true, tool: "routing_lab_run", request_id: "routing-lab-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/routing-lab-run/0.1",
        tasks: 2,
        holdout: "task",
        holdout_label: "leave-one-task-out",
        approved_architectures: ["full-context", "fiber"],
        fixed_default: { kind: "full_context" },
        include_rows: true,
        report: {
          account: { router: {}, oracle: {} },
          calibration: { bins: [] },
          verdict: "router_loses_to_fixed_default",
          abstention_rate: 0.5,
          oracle_agreement_rate: 0,
          tasks_won: 0,
          tasks_lost: 1,
          tasks_tied: 1,
          caveats: ["leave-one-task-out"],
          task_rows: [{ task_id: "reference-task" }],
          task_rows_omitted: 1,
        },
        guarantees: ["route_unseen holdout is enforced"],
        limitations: ["offline lab"],
      } } } });
    },
  });
  const result = await client.routingLabRun({ tasks: [], settings: {}, include_rows: true, max_rows: 1 });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/routing-lab-run/0.1");
  assert.equal(result.mcp.result.structuredContent.report.verdict, "router_loses_to_fixed_default");
  assert.equal(result.mcp.result.structuredContent.report.task_rows_omitted, 1);
});

test("client exposes typed inference-lab Pareto trade-offs and holes", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/lab_pareto_audit");
      return jsonResponse({ ok: true, tool: "lab_pareto_audit", request_id: "pareto-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/lab-pareto-audit/0.1",
        objective_count: 2,
        profile_count: 2,
        objectives: [],
        admissions: [],
        admissions_omitted: 2,
        front: {
          count: 2,
          members: [],
          unresolved_count: 1,
          unresolved: [{ candidate: "hole", axes: ["cost_units"] }],
          selection: { selection: "ambiguous", front: ["cheap", "accurate"], unresolved: [] },
        },
        archived_count: 0,
        archived: [],
        archived_omitted: 0,
        relations: [{ left: "cheap", right: "accurate", relation: { relation: "incomparable" } }],
        relations_omitted: 0,
        max_rows: 100,
        guarantees: ["trade-offs remain incomparable"],
        limitations: ["point measurements"],
      } } } });
    },
  });
  const result = await client.labParetoAudit({
    objectives: [{ axis: "admissible_rate", direction: "higher_is_better" }],
    profiles: [{ candidate: "cheap", values: {} }],
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/lab-pareto-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.front.selection.selection, "ambiguous");
  assert.equal(result.mcp.result.structuredContent.relations[0].relation.relation, "incomparable");
});

test("client exposes risk-triggered branch cost and escaped harm accounting", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/lab_branch_audit");
      return jsonResponse({ ok: true, tool: "lab_branch_audit", request_id: "branch-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/lab-branch-audit/0.1",
        policy: {},
        decision_count: 1,
        yield: {
          decisions: 1,
          escalations: 1,
          escalations_on_undetermined: 1,
          spent: { branches: 1, verifier_calls: 1 },
          catches: 0,
          wasted_escalations: 1,
          escaped_after_escalation: 1,
          escaped_without_escalation: 0,
          branches_per_catch: null,
        },
        verdict: { verdict: "paid_and_caught_nothing", spent: { branches: 1, verifier_calls: 1 }, escalations: 1 },
        rows: [{ index: 0, decision: "uncertain" }],
        rows_omitted: 0,
        max_rows: 100,
        guarantees: ["undetermined risk escalates"],
        limitations: ["planning only"],
      } } } });
    },
  });
  const result = await client.labBranchAudit({
    policy: {},
    decisions: [{ decision: "uncertain", features: { historical_failure_rate: null } }],
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/lab-branch-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.verdict.verdict, "paid_and_caught_nothing");
  assert.equal(result.mcp.result.structuredContent.yield.escalations_on_undetermined, 1);
});

test("client exposes holdout contamination and rollback retention", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/lab_holdout_audit");
      return jsonResponse({ ok: true, tool: "lab_holdout_audit", request_id: "holdout-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/lab-holdout-audit/0.1",
        current: "v1",
        space: { candidate_count: 2, registered_ids: ["v1", "v2"] },
        holdouts: [{ id: "private-a", measurement_refusal: "selected" }],
        remaining_certification_budget: [],
        checkpoints: [{ label: "before-v2" }],
        checkpoint_count: 1,
        history: [{ event: "rolled_back" }],
        operations: [{ index: 0, kind: "measure", result: "measurement_refused" }],
        operations_omitted: 0,
        operation_count: 1,
        measurement_count: 0,
        measurement_refusal_count: 1,
        rollback_count: 1,
        permanently_burned: [{ holdout: "private-a", configuration: "v2" }],
        max_rows: 100,
        guarantees: ["rollback never rewinds exposure"],
        limitations: ["offline audit"],
      } } } });
    },
  });
  const result = await client.labHoldoutAudit({
    cost_ceiling: 100,
    candidates: [{ id: "v1" }],
    holdouts: [{ id: "private-a", partition: "rotating_private_certification", query_budget: 4 }],
    current: "v1",
    operations: [{ kind: "measure" }],
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/lab-holdout-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.measurement_refusal_count, 1);
  assert.equal(result.mcp.result.structuredContent.permanently_burned[0].configuration, "v2");
});

test("client exposes evolution claim gating and retained measurement rows", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/lab_evolution_audit");
      return jsonResponse({ ok: true, tool: "lab_evolution_audit", request_id: "evolution-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/lab-evolution-audit/0.1",
        status: "improvement_claimed",
        claimable: true,
        card: { id: "card-v2", surface: { surface: "rotating_private_certification" } },
        claim: { card_id: "card-v2", delta: 0.13 },
        sentence: "v2 improves admissible_rate over v1",
        measurement_count: 2,
        measurement_rows: [{ index: 0, result: "clean_measurement" }],
        measurement_rows_omitted: 1,
        max_rows: 1,
        guarantees: ["clean measurements only"],
        limitations: ["point delta"],
      } } } });
    },
  });
  const result = await client.labEvolutionAudit({
    cost_ceiling: 100,
    candidates: [{ id: "v1" }, { id: "v2", derived_from: "v1" }],
    baseline: "v1",
    candidate: "v2",
    holdout: { id: "private-a", partition: "rotating_private_certification", query_budget: 4 },
    measurements: [
      { configuration: "v1", metric: "admissible_rate", value: 0.7 },
      { configuration: "v2", metric: "admissible_rate", value: 0.83 },
    ],
    card_id: "card-v2",
    proposal: { id: "proposal-v2" },
    rollback_handle: "v1",
    direction: "higher_is_better",
    would_have_to_be_true: ["the gain survives another private set"],
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.status, "improvement_claimed");
  assert.equal(result.mcp.result.structuredContent.claimable, true);
  assert.equal(result.mcp.result.structuredContent.claim.delta, 0.13);
  assert.equal(result.mcp.result.structuredContent.measurement_rows_omitted, 1);
});

test("client exposes immutable architecture lineage and component diffs", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/lab_space_audit");
      return jsonResponse({ ok: true, tool: "lab_space_audit", request_id: "space-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/lab-space-audit/0.1",
        cost_ceiling: 10,
        candidate_count: 2,
        registered_count: 2,
        space_committed: true,
        space: { registered_ids: ["v1", "v2"], root_ids: ["v1"], lineage_depth_max: 2 },
        candidate_rows: [{ index: 0, registration: "registered" }],
        candidate_rows_omitted: 1,
        inspection_count: 1,
        inspection_rows: [{ configuration: "v2", lineage: ["v2", "v1"] }],
        inspection_rows_omitted: 0,
        comparison_count: 1,
        comparison_rows: [{ before: "v1", after: "v2", changes: ["cost_units 0 -> 2"] }],
        comparison_rows_omitted: 0,
        max_rows: 1,
        guarantees: ["immutable registry"],
        limitations: ["no execution"],
      } } } });
    },
  });
  const result = await client.labSpaceAudit({
    cost_ceiling: 10,
    candidates: [{ id: "v1" }, { id: "v2", derived_from: "v1" }],
    inspect: ["v2"],
    comparisons: [{ before: "v1", after: "v2" }],
    include_components: true,
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/lab-space-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.space_committed, true);
  assert.deepEqual(result.mcp.result.structuredContent.inspection_rows[0].lineage, ["v2", "v1"]);
  assert.equal(result.mcp.result.structuredContent.comparison_rows[0].after, "v2");
});

test("client exposes provider gate states and differential indeterminacy", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/provider_capability_gate");
      return jsonResponse({ ok: true, tool: "provider_capability_gate", request_id: "provider-1", mcp: { result: { structuredContent: {
        ok: true,
        provider: "runtime-a",
        required: ["host_escape"],
        required_states: { HostEscape: { state: "untested" } },
        gate: { outcome: "blocked", unproven: ["HostEscape=untested"] },
        claims: [],
        measurement_count: 0,
        differential: { HostEscape: { drift: "indeterminate", untested: ["runtime-a", "runtime-b"] } },
        guarantees: ["untested blocks"],
      } } } });
    },
  });
  const result = await client.providerCapabilityGate({ card: { provider: "runtime-a", states: {}, measurements: [] }, required: ["host_escape"] });
  assert.equal(result.mcp.result.structuredContent.gate.outcome, "blocked");
  assert.equal(result.mcp.result.structuredContent.differential.HostEscape.drift, "indeterminate");
});

test("client exposes SDK registry fail-closed admission results", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/sdk_registry_check");
      return jsonResponse({ ok: true, tool: "sdk_registry_check", request_id: "registry-1", mcp: { result: { structuredContent: {
        ok: false,
        stage: "manifest_validation",
        manifests: [{ index: 0, valid: false, refusal: "invalid plugin manifest" }],
        registry: null,
        fail_closed: true,
        guarantees: ["no partial registry"],
      } } } });
    },
  });
  const result = await client.sdkRegistryCheck({ manifests: [{ id: "plugin" }] });
  assert.equal(result.mcp.result.structuredContent.stage, "manifest_validation");
  assert.equal(result.mcp.result.structuredContent.registry, null);
  assert.equal(result.mcp.result.structuredContent.fail_closed, true);
});

test("client exposes epistemic context frontier and subset refusal accounting", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/epistemic_context_audit");
      return jsonResponse({ ok: true, tool: "epistemic_context_audit", request_id: "context-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/epistemic-context-audit/0.1",
        criterion: "bayes_regret",
        tolerance: 1,
        compatibility_floor: 0,
        problem: { actions: ["treat", "abstain"], models: ["responsive", "resistant"] },
        evidence_pool: { item_count: 2, full_rate: 3 },
        identification: { status: "non_identified", minimax_regret: 10 },
        sufficiency: { outcome: "sufficient", retained: [0, 1], rate: 3, distortion: 0 },
        frontier: { criterion: "bayes_regret", evaluated: 4, points: [] },
        include_frontier: true,
        subset_rows: [{ index: 0, result: "evaluated" }],
        subset_count: 2,
        subset_refusal_count: 0,
        subset_rows_omitted: 1,
        max_rows: 1,
        guarantees: ["decision regret"],
        limitations: ["caller-declared prior"],
      } } } });
    },
  });
  const result = await client.epistemicContextAudit({
    problem: { actions: ["treat", "abstain"], models: ["responsive", "resistant"], loss: [0, 10, 10, 0] },
    belief: { mass: [0.5, 0.5] },
    evidence_pool: { items: [
      { id: "scan", cost: 2, likelihood: [0.9, 0.1] },
      { id: "marker", cost: 1, likelihood: [0.1, 0.9] },
    ] },
    criterion: "bayes_regret",
    tolerance: 1,
    compatibility_floor: 0,
    subsets: [[0], [0, 1]],
    include_frontier: true,
    max_rows: 1,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/epistemic-context-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.frontier.evaluated, 4);
  assert.equal(result.mcp.result.structuredContent.subset_rows_omitted, 1);
});

test("client exposes epistemic selection guarantee and exactness posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/epistemic_selection_audit");
      return jsonResponse({ ok: true, tool: "epistemic_selection_audit", request_id: "selection-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/epistemic-selection-audit/0.1",
        objective: "regret_reduction",
        problem: { action_count: 2, model_count: 2 },
        evidence_pool: { count: 3, total_cost: 4 },
        constraint: { cardinality: 2, budget: null },
        baseline: { full_context_regret: 9.5, empty_context_value: 0 },
        submodularity: { status: "evaluated" },
        greedy: { chosen: [{ index: 0, id: "scan" }], guarantee: { applicability: "applies" } },
        lazy: { chosen: [{ index: 0, id: "scan" }] },
        comparisons: { greedy_lazy_agree: true, exact_optimum: { status: "evaluated", ratio: 1 } },
        guarantees: ["factor gated"],
        limitations: ["scalarized cost"],
      } } } });
    },
  });
  const result = await client.epistemicSelectionAudit({
    problem: { actions: ["treat", "defer"], models: ["responsive", "resistant"], loss: [0, 10, 10, 0] },
    belief: { mass: [0.4, 0.6] },
    evidence_pool: { items: [
      { id: "scan", cost: 2, likelihood: [0.9, 0.1] },
      { id: "marker", cost: 1, likelihood: [0.8, 0.2] },
      { id: "uninformative", cost: 1, likelihood: [1, 1] },
    ] },
    constraint: { cardinality: 2 },
    protected: [],
    check_submodularity: true,
    include_lazy: true,
    compare_optimum: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/epistemic-selection-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.greedy.guarantee.applicability, "applies");
  assert.equal(result.mcp.result.structuredContent.comparisons.exact_optimum.status, "evaluated");
});

test("client exposes bioeval acquisition stopping and named regret evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_acquisition_audit");
      return jsonResponse({ ok: true, tool: "bioeval_acquisition_audit", request_id: "acquisition-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-acquisition-audit/0.1",
        workflow: "bioeval_acquisition_audit",
        status: "admissible",
        stopped_after: true,
        admissible: true,
        obligations: [{ id: "subtype", required: true, closed: true }],
        open_obligations: [],
        actions: [{ id: "panel", kind: "assay", cost: 40 }],
        cost: 42,
        findings: { deferred_decisive_cost: 2, redundant_action_ids: [], unnecessary_action_ids: [] },
        reference_policy: { name: "random", cost: 30, admissible: true },
        regret: { cost_difference: 12, like_for_like: true },
        guarantees: ["required obligations gate admissibility"],
        limitations: ["no acquisition executed"],
      } } } });
    },
  });
  const result = await client.bioevalAcquisitionAudit({
    obligations: [{ id: "subtype", required: true }],
    actions: [{ id: "panel", kind: "assay", cost: 40, closes: ["subtype"] }],
    stopped_after: true,
    reference_policy: { name: "random", cost: 30, admissible: true },
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-acquisition-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.status, "admissible");
  assert.equal(result.mcp.result.structuredContent.regret.like_for_like, true);
});

test("client exposes bioeval grounding state, locator, and staleness evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_grounding_audit");
      return jsonResponse({ ok: true, tool: "bioeval_grounding_audit", request_id: "grounding-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-grounding-audit/0.1",
        workflow: "bioeval_grounding_audit",
        claims: { rows: [], returned: 0, total: 2, omitted: 0 },
        evidence: { rows: [], returned: 0, total: 2, omitted: 0 },
        edges: { rows: [], returned: 0, total: 3, omitted: 0 },
        census: { claims: 2, supported: 1, contested: 1, fully_grounded: false },
        graph: { claim_count: 2, evidence_count: 2, edge_count: 3 },
        locator_census: { resolved: 1, not_checked: 1, unresolvable: 0 },
        staleness: { requested: true, stale_count: 0 },
        findings: { contested_claims: { ids: ["contested"], total: 1, omitted: 0 } },
        guarantees: ["states remain distinct"],
        limitations: ["no dereference"],
      } } } });
    },
  });
  const result = await client.bioevalGroundingAudit({
    claims: [{ id: "supported" }, { id: "contested" }],
    evidence: [
      { id: "source", last_modified: "2026-01-01T00:00:00Z", lineage: ["specimen-1"], locator_status: { locator: "resolved", digest: "sha256:source" } },
      { id: "asserted", last_modified: "2026-01-01T00:00:00Z", locator_status: { locator: "not_checked" } },
    ],
    edges: [
      { claim: "supported", evidence: "source", kind: "supports" },
      { claim: "contested", evidence: "source", kind: "supports" },
      { claim: "contested", evidence: "asserted", kind: "contradicts" },
    ],
    stale_against: "2026-03-01T00:00:00Z",
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-grounding-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.census.contested, 1);
  assert.equal(result.mcp.result.structuredContent.staleness.requested, true);
});

test("client exposes bioeval estimand identification and transport evidence", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_estimand_audit");
      return jsonResponse({ ok: true, tool: "bioeval_estimand_audit", request_id: "estimand-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-estimand-audit/0.1",
        workflow: "bioeval_estimand_audit",
        estimand: { five_elements_complete: true, scope: "pdac-twin" },
        claim: { kind: "intervention", still_model_conditional: false, identification_summary: { status: "probed" } },
        policies: { require_identification: true },
        transport: { status: "partially_declared", accepted: 1, refused: 1 },
        guarantees: ["qualifier retained"],
        limitations: ["no causal engine"],
      } } } });
    },
  });
  const result = await client.bioevalEstimandAudit({
    estimand: {
      intervention: "knockdown",
      comparator: "control",
      unit: "cell line",
      outcome: "viability",
      horizon: "72h",
      scope: "pdac-twin",
    },
    kind: "intervention",
    basis: { evidentiary: "model_conditional", model: "pdac-twin-v2" },
    identification: {
      identification: "probed",
      strategy: "backdoor",
      assumptions: ["no unmeasured confounding"],
      checks: [{ name: "negative-control", passed: false, detail: "signal remained" }],
    },
    corroborations: [{ source: "GSE-14520", kind: "intervention", detail: "external replication" }],
    transport_requests: [{ target: "pdac-twin", declared_scopes: ["pdac-twin"] }],
    require_identification: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-estimand-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.claim.identification_summary.status, "probed");
  assert.equal(result.mcp.result.structuredContent.transport.status, "partially_declared");
});

test("client exposes evaluator health separately from task outcomes and hidden data", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_evaluator_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.runs[0].health.health, "healthy");
      assert.equal(body.runs[1].health.health, "timed_out");
      return jsonResponse({ ok: true, tool: "bioeval_evaluator_audit", request_id: "evaluator-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-evaluator-audit/0.1",
        workflow: "bioeval_evaluator_audit",
        runs: { rows: [], returned: 0, total: 2, omitted: 2 },
        panel: { run_count: 2, healthy_count: 1, unhealthy_count: 1, task_evidence_count: 1, posture: "review_required_hidden_data" },
        findings: { hidden_data_evaluators: { ids: ["grader"], total: 1, omitted: 0 } },
        guarantees: ["harness failures remain unscored"],
        limitations: ["no harness execution"],
      } } } });
    },
  });
  const result = await client.bioevalEvaluatorAudit({
    runs: [
      { evaluator: "grader", health: { health: "healthy" }, reached: "met", diagnostic: { command: "pytest", exit_state: "0", diff: "", hidden_data_access: ["read expected_outputs/"] } },
      { evaluator: "timeout", health: { health: "timed_out", after: "120s" }, reached: null, diagnostic: { command: "", exit_state: "", diff: "" } },
    ],
    fail_on_hidden_data: false,
    max_items: 2,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-evaluator-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.panel.posture, "review_required_hidden_data");
  assert.equal(result.mcp.result.structuredContent.findings.hidden_data_evaluators.ids[0], "grader");
});

test("client exposes scoring-plane cells and fold refusal posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_plane_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.plane.cells.accuracy.state, "scored");
      assert.equal(body.plane.cells.assay_selection.state, "inapplicable");
      return jsonResponse({ ok: true, tool: "bioeval_plane_audit", request_id: "plane-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-plane-audit/0.1",
        workflow: "bioeval_plane_audit",
        plane: { system: "pipeline", tier: "workflow_pipeline", dimension_count: 2, scored_count: 2, unscored_count: 0, inapplicable_count: 0 },
        dimensions: { rows: [], returned: 0, total: 2, omitted: 2 },
        findings: { unscored_dimensions: { ids: [], total: 0, omitted: 0 }, fold_blocked: false },
        fold: { folded: true, value: 0.8, included: ["accuracy", "workflow"], excluded: [], refusal: null },
        guarantees: ["missing remains distinct"],
        limitations: ["no ranking"],
      } } } });
    },
  });
  const result = await client.bioevalPlaneAudit({
    plane: {
      system: "fixed-model",
      tier: "fixed_input_model",
      dimensions: [
        { id: "accuracy", required: "fixed_input_model", weight: 1 },
        { id: "assay_selection", required: "tool_using_agent", weight: 1 },
      ],
      cells: {
        accuracy: { state: "scored", score: 0.8 },
        assay_selection: { state: "inapplicable", required: "tool_using_agent", declared: "fixed_input_model" },
      },
    },
    require_fold: false,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-plane-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.fold.folded, true);
  assert.equal(result.mcp.result.structuredContent.findings.fold_blocked, false);
});

test("client exposes metamorphic response directions and undetermined posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_metamorphic_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.families[0].trials[0].response.response, "unchanged");
      assert.equal(body.families[1].relation.directional_change.expected, "increase");
      return jsonResponse({ ok: true, tool: "bioeval_metamorphic_audit", request_id: "metamorphic-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-metamorphic-audit/0.1",
        workflow: "bioeval_metamorphic_audit",
        suite: { family_count: 2, trial_count: 5, relation_coverage: { invariant: true, directional_change: true, complete: true }, undetermined_trial_count: 1, has_suite_wide_consistency: false },
        families: { rows: [], returned: 0, total: 2, omitted: 2 },
        findings: {
          false_sensitivity_trials: { ids: ["shortcut"], total: 1, omitted: 0 },
          false_invariance_trials: { ids: ["blind-spot"], total: 1, omitted: 0 },
          wrong_direction_trials: { ids: [], total: 0, omitted: 0 },
        },
        guarantees: ["incomparable responses remain undetermined"],
        limitations: ["no suite-wide consistency percentage"],
      } } } });
    },
  });
  const result = await client.bioevalMetamorphicAudit({
    families: [
      {
        id: "invariance",
        relation: "invariant",
        trials: [
          { id: "same", relation: "invariant", response: { response: "unchanged" } },
          { id: "shortcut", relation: "invariant", response: { response: "moved", direction: "increase" } },
          { id: "undetermined", relation: "invariant", response: { response: "incomparable" } },
        ],
      },
      {
        id: "direction",
        relation: { directional_change: { expected: "increase" } },
        trials: [
          { id: "expected", relation: { directional_change: { expected: "increase" } }, response: { response: "moved", direction: "increase" } },
          { id: "blind-spot", relation: { directional_change: { expected: "increase" } }, response: { response: "unchanged" } },
        ],
      },
    ],
    require_both_relations: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-metamorphic-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.suite.relation_coverage.complete, true);
  assert.equal(result.mcp.result.structuredContent.suite.has_suite_wide_consistency, false);
  assert.equal(result.mcp.result.structuredContent.findings.false_sensitivity_trials.ids[0], "shortcut");
  assert.equal(result.mcp.result.structuredContent.findings.false_invariance_trials.ids[0], "blind-spot");
});

test("client exposes release-gate waiver evidence without rewriting verdicts", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_waiver_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.gates[0].verdict.verdict, "violated");
      assert.equal(body.waivers[0].affected_versions[0], "release-2026.08");
      return jsonResponse({ ok: true, tool: "bioeval_waiver_audit", request_id: "waiver-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-waiver-audit/0.1",
        workflow: "bioeval_waiver_audit",
        release: { version: "release-2026.08", blocking_before: 2, blocking_after: 1, waived_count: 1, unevaluable_count: 1, releasable: false },
        gates: { rows: [], returned: 0, total: 2, omitted: 2 },
        waivers: { rows: [], returned: 0, total: 1, omitted: 1 },
        findings: {
          still_blocking: { ids: ["unknown-rate"], total: 1, omitted: 0 },
          waived_gates: { ids: ["health"], total: 1, omitted: 0 },
          unevaluable_gates: { ids: ["unknown-rate"], total: 1, omitted: 0 },
          safety_vetoes: { ids: [], total: 0, omitted: 0 },
        },
        guarantees: ["underlying verdict remains visible"],
        limitations: ["no identity provider"],
      } } } });
    },
  });
  const result = await client.bioevalWaiverAudit({
    version: "release-2026.08",
    at: "2026-08-16T12:00:00Z",
    gates: [
      { id: "health", kind: "benchmark_health", verdict: { verdict: "violated", detail: "calibration below floor" } },
      { id: "unknown-rate", kind: "maximum_unknown_rate", verdict: { verdict: "unevaluable", missing: "reference panel" } },
    ],
    waivers: [{
      gate: "health",
      authoriser: "release-board",
      rationale: "documented exception",
      expiry: "2026-09-01T00:00:00Z",
      affected_versions: ["release-2026.08"],
      follow_up: "recalibrate before next release",
    }],
    require_releasable: false,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-waiver-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.release.releasable, false);
  assert.equal(result.mcp.result.structuredContent.findings.waived_gates.ids[0], "health");
  assert.equal(result.mcp.result.structuredContent.findings.unevaluable_gates.ids[0], "unknown-rate");
});

test("client exposes factorial contrasts and interaction coverage", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_design_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.baseline, "base");
      assert.deepEqual(body.arms[0].levels, { planner: "react", verifier: "off" });
      return jsonResponse({ ok: true, tool: "bioeval_design_audit", request_id: "design-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-design-audit/0.1",
        workflow: "bioeval_design_audit",
        design: { cell_id: "cell-7", factors: ["planner", "verifier"], baseline: "base", arm_count: 4, contrast_count: 4, unattributable_arm_count: 1, controlled: true, valid: true },
        arms: { rows: [], returned: 0, total: 4, omitted: 4 },
        contrasts: { rows: [], returned: 0, total: 4, omitted: 4 },
        interactions: { rows: [], returned: 0, total: 1, omitted: 1, estimable_count: 1, missing_count: 0 },
        attributions: { rows: [], returned: 0, total: 4, omitted: 4, refused_count: 0, causal_count: 4 },
        findings: { unattributable_arms: { ids: ["both"], total: 1, omitted: 0 }, missing_interactions: { ids: [], total: 0, omitted: 0 }, no_single_factor_contrasts: false, attribution_refusal_count: 0 },
        guarantees: ["single-factor contrasts remain distinct"],
        limitations: ["no arm execution"],
      } } } });
    },
  });
  const result = await client.bioevalDesignAudit({
    cell_id: "cell-7",
    factors: ["planner", "verifier"],
    baseline: "base",
    arms: [
      { id: "base", levels: { planner: "react", verifier: "off" }, conclusion: "fail", tier: "execution" },
      { id: "p1", levels: { planner: "tree", verifier: "off" }, conclusion: "pass", tier: "execution" },
      { id: "v1", levels: { planner: "react", verifier: "on" }, conclusion: "pass", tier: "execution" },
      { id: "both", levels: { planner: "tree", verifier: "on" }, conclusion: "pass", tier: "execution" },
    ],
    controlled: true,
    require_complete_interactions: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-design-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.design.contrast_count, 4);
  assert.equal(result.mcp.result.structuredContent.interactions.missing_count, 0);
  assert.equal(result.mcp.result.structuredContent.findings.unattributable_arms.ids[0], "both");
});

test("client exposes evaluator mesh independence classes and disagreement posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_mesh_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.evaluators[0].inputs[0], "report-77");
      assert.equal(body.verdicts[0].position, "progression");
      return jsonResponse({ ok: true, tool: "bioeval_mesh_audit", request_id: "mesh-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-mesh-audit/0.1",
        workflow: "bioeval_mesh_audit",
        mesh: { evaluator_count: 3, independent_class_count: 2, non_model_class_count: 2, independence_verified: true, kinds_present: ["expert_review", "executable_analysis"], inputs_undeclared: [] },
        evaluators: { rows: [], returned: 0, total: 3, omitted: 3 },
        classes: { rows: [], returned: 0, total: 2, omitted: 2 },
        verdicts: { rows: [], returned: 0, total: 3, omitted: 3 },
        disagreements: { rows: [], returned: 0, total: 1, omitted: 1, within_class_count: 0, across_class_count: 1 },
        independent_ratings: { status: "accepted", rows: [], refusal: null },
        contributions: { status: "accepted", expected: "progression", rows: [], refusal: null },
        findings: { inputs_undeclared: { ids: [], total: 0, omitted: 0 }, unreported_evaluators: { ids: [], total: 0, omitted: 0 }, abstaining_evaluators: { ids: [], total: 0, omitted: 0 }, within_class_disagreement_count: 0, across_class_disagreement_count: 1, rating_projection_refused: false },
        guarantees: ["shared inputs collapse into classes"],
        limitations: ["no adjudication"],
      } } } });
    },
  });
  const result = await client.bioevalMeshAudit({
    system_artifacts: ["system-weights"],
    evaluators: [
      { id: "reader-a", kind: "expert_review", inputs: ["report-77"] },
      { id: "reader-b", kind: "expert_review", inputs: ["report-77"] },
      { id: "molecular", kind: "executable_analysis", inputs: ["panel-9"] },
    ],
    verdicts: [
      { evaluator: "reader-a", position: "progression" },
      { evaluator: "reader-b", position: "progression" },
      { evaluator: "molecular", position: "pseudoprogression" },
    ],
    expected: "progression",
    require_independence: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-mesh-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.mesh.independent_class_count, 2);
  assert.equal(result.mcp.result.structuredContent.disagreements.across_class_count, 1);
  assert.equal(result.mcp.result.structuredContent.findings.rating_projection_refused, false);
});

test("client exposes burden residuals, failed draws, and fork feasibility", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_burden_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.resources[0].class, "tissue_aliquot");
      assert.equal(body.draws[0].outcome, "wasted");
      return jsonResponse({ ok: true, tool: "bioeval_burden_audit", request_id: "burden-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-burden-audit/0.1",
        workflow: "bioeval_burden_audit",
        burden: { root: "root", resource_count: 1, branch_count: 3, draw_count: 2, nonrenewable_resource_count: 1 },
        resources: { rows: [], returned: 0, total: 1, omitted: 1 },
        branches: { rows: [], returned: 0, total: 3, omitted: 3 },
        draws: { rows: [], returned: 0, total: 2, omitted: 2 },
        joint_feasibility: { status: "refused", branches: ["a", "b"], refusal: "fork double spend" },
        wasted_nonrenewable: { rows: [], returned: 0, total: 1, omitted: 1 },
        findings: { wasted_nonrenewable_actions: { ids: ["extract"], total: 1, omitted: 0 }, joint_feasibility_refused: true },
        guarantees: ["failed draws remain visible"],
        limitations: ["no pricing"],
      } } } });
    },
  });
  const result = await client.bioevalBurdenAudit({
    root: "root",
    resources: [{ id: "biopsy", class: "tissue_aliquot", initial: 100, unit: "uL" }],
    branches: [{ id: "a" }, { id: "b" }],
    draws: [
      { branch: "a", action: "extract", resource: "biopsy", amount: 60, unit: "uL", outcome: "wasted", destructive: true },
      { branch: "b", action: "extract-b", resource: "biopsy", amount: 60, unit: "uL", outcome: "productive", destructive: true },
    ],
    joint_branches: ["a", "b"],
    require_joint_feasible: false,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-burden-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.joint_feasibility.status, "refused");
  assert.equal(result.mcp.result.structuredContent.findings.joint_feasibility_refused, true);
});

test("client exposes prospective reveal locks and rubric digest policy", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_reveal_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.commitments[0].analysis_plan, "plan-v1");
      assert.equal(body.rubric.version, 1);
      return jsonResponse({ ok: true, tool: "bioeval_reveal_audit", request_id: "reveal-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-reveal-audit/0.1",
        workflow: "bioeval_reveal_audit",
        study: "prospective-2026",
        sealed_at: "2026-08-16T12:00:00Z",
        digests: { rubric: "rubric-digest", commitments: "commitment-digest" },
        commitments: { rows: [], returned: 0, total: 2, omitted: 2 },
        outcomes: { rows: [], returned: 0, total: 1, omitted: 1 },
        seal_lock: { status: "refused", refusal: "already sealed" },
        reveal_lock: { status: "refused", refusal: "already revealed" },
        scoring: { status: "accepted", value: {}, refusal: null, complete: false },
        findings: { unrevealed_commitments: { ids: ["case-b"], total: 1, omitted: 0 }, selective_publication: true, rubric_match_refused: false },
        guarantees: ["commitment digest frozen"],
        limitations: ["no timestamp attestation"],
      } } } });
    },
  });
  const result = await client.bioevalRevealAudit({
    study: "prospective-2026",
    commitments: [
      { target: "case-a", prediction: { class: "stable" }, analysis_plan: "plan-v1" },
      { target: "case-b", prediction: { class: "progression" }, analysis_plan: "plan-v1" },
    ],
    rubric: { version: 1, rules: ["predeclared"] },
    sealed_at: "2026-08-16T12:00:00Z",
    outcomes: [{ target: "case-a", observed: { class: "stable" } }],
    score_rubric: { version: 1, rules: ["predeclared"] },
    require_rubric_match: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-reveal-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.scoring.complete, false);
  assert.equal(result.mcp.result.structuredContent.findings.selective_publication, true);
});

test("client exposes contextual-integrity verdicts and Pareto safety posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/bioeval_boundary_audit");
      const body = JSON.parse(init.body);
      assert.equal(body.flows[0].channel, "inter_agent_messages");
      assert.equal(body.policies[0].transmission_principle, "consent");
      return jsonResponse({ ok: true, tool: "bioeval_boundary_audit", request_id: "boundary-1", mcp: { result: { structuredContent: {
        ok: true,
        schema: "bioprism-mcp/bioeval-boundary-audit/0.1",
        workflow: "bioeval_boundary_audit",
        boundary: { policy_count: 1, flow_count: 5, authorised_count: 1, compliant_count: 1, violation_count: 3, veto_count: 2 },
        policies: { rows: [], returned: 0, total: 1, omitted: 1 },
        flows: { rows: [], returned: 0, total: 5, omitted: 5 },
        violations_by_channel: { final_output: 1, external_queries: 1, logs: 1 },
        pareto: { utility: 0.8, violations: 3 },
        composite: { status: "refused", value: null, refusal: "composite refused" },
        findings: { violating_flows: { ids: ["violation"], total: 3, omitted: 0 }, veto_flows: { ids: ["veto", "bypass"], total: 2, omitted: 0 }, composite_refused: true },
        guarantees: ["utility and safety remain separate"],
        limitations: ["no payload detector"],
      } } } });
    },
  });
  const result = await client.bioevalBoundaryAudit({
    policies: [{
      id: "consent-study",
      recipient: "evaluator",
      information_type: "deidentified",
      purpose: "study",
      transmission_principle: "consent",
      channels: ["inter_agent_messages"],
    }],
    flows: [{
      id: "authorized",
      sender: "agent",
      subject: "participant-1",
      recipient: "evaluator",
      information_type: "deidentified",
      purpose: "study",
      transmission_principle: "consent",
      channel: "inter_agent_messages",
      effect: { effect: "materialized" },
    }],
    utility: 0.8,
    require_no_vetoes: true,
  });
  assert.equal(result.mcp.result.structuredContent.schema, "bioprism-mcp/bioeval-boundary-audit/0.1");
  assert.equal(result.mcp.result.structuredContent.boundary.violation_count, 3);
  assert.equal(result.mcp.result.structuredContent.composite.status, "refused");
});
