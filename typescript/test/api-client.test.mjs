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
          targets: [{ target: "local_delivery", available: true, eligible: true, blockers: [], notes: [] }],
          ready: true,
          fail_closed: false,
          no_implicit_release: true,
          available_target_count: 10,
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
  const delivery = await client.developerDeliveryAudit({ release_request: { id: "delivery-1", targets: ["local_delivery"] } });
  assert.equal(delivery.mcp.result.structuredContent.workflow, "developer_delivery_audit");
  assert.equal(delivery.mcp.result.structuredContent.readiness.local_delivery_ready, true);
  assert.equal(delivery.mcp.result.structuredContent.release_request.targets[0].target, "local_delivery");
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
