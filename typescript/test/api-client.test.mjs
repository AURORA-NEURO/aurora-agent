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
      if (path === "/v1/tools/telemetry_project") return jsonResponse({ ok: true, tool: "telemetry_project", request_id: "r14", mcp: { result: { structuredContent: { workflow: "telemetry_project", trace: "trace-ts" } } } });
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
  assert.equal(catalog.mcp.result.structuredContent.workflow, "repository_catalog");
  assert.equal(bundle.mcp.result.structuredContent.policy, "exhaustive");
  assert.equal(impact.mcp.result.structuredContent.changed, "docs/README");
  assert.equal(telemetry.mcp.result.structuredContent.trace, "trace-ts");
  const workbench = await client.developerWorkbench({ session: { session_id: "studio-1" }, dashboard: { include_holes: true } });
  assert.equal(workbench.mcp.result.structuredContent.workflow, "developer_workbench");
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
  const mission = await client.agentMission({ mission_id: "mission-1", goal: "discover", steps: [{ id: "catalog", domain: "workspace", capability: "discovery", objective: "discover", tool: "workspace_capabilities" }] });
  assert.equal(mission.mcp.result.structuredContent.workflow, "agent_mission");
  assert.equal(mission.mcp.result.structuredContent.execution_trace[0].event, "mission.started");
  assert.equal(mission.mcp.result.structuredContent.execution_trace.at(-1).event, "mission.completed");
  await assert.rejects(client.callTool("unsafe/name"), ArgumentError);
  await assert.rejects(async () => client.requireToolSuccess(await client.callTool("refuse")), ToolRefusalError);
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
