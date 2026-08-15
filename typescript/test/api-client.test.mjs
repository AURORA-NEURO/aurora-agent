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
      if (path === "/v1/tools/developer_workbench") return jsonResponse({ ok: true, tool: "developer_workbench", request_id: "r4", mcp: { result: { structuredContent: { workflow: "developer_workbench", audit: { valid: true } } } } });
      if (path === "/v1/tools/repository_catalog") return jsonResponse({ ok: true, tool: "repository_catalog", request_id: "r11", mcp: { result: { structuredContent: { workflow: "repository_catalog", prefix: "docs/" } } } });
      if (path === "/v1/tools/repository_bundle") return jsonResponse({ ok: true, tool: "repository_bundle", request_id: "r12", mcp: { result: { structuredContent: { workflow: "repository_bundle", policy: "exhaustive" } } } });
      if (path === "/v1/tools/repository_impact") return jsonResponse({ ok: true, tool: "repository_impact", request_id: "r13", mcp: { result: { structuredContent: { workflow: "repository_impact", changed: "docs/README" } } } });
      if (path === "/v1/tools/telemetry_project") return jsonResponse({ ok: true, tool: "telemetry_project", request_id: "r14", mcp: { result: { structuredContent: { workflow: "telemetry_project", trace: "trace-ts" } } } });
      if (path === "/v1/tools/capability_discover") return jsonResponse({ ok: true, tool: "capability_discover", request_id: "r6", mcp: { result: { structuredContent: { workflow: "capability_discover", result_count: 1 } } } });
      if (path === "/v1/tools/capability_audit") return jsonResponse({ ok: true, tool: "capability_audit", request_id: "r7", mcp: { result: { structuredContent: { workflow: "capability_audit", healthy: true } } } });
      if (path === "/v1/tools/capability_route") return jsonResponse({ ok: true, tool: "capability_route", request_id: "r8", mcp: { result: { structuredContent: { workflow: "capability_route", execution: "not_started" } } } });
      if (path === "/v1/tools/adapter_plan") return jsonResponse({ ok: true, tool: "adapter_plan", request_id: "r10", mcp: { result: { structuredContent: { workflow: "adapter_plan", executable: true } } } });
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
  const capabilities = await client.capabilityDiscover({ query: "oncology evidence", include_tools: true });
  const capabilityAudit = await client.capabilityAudit({ include_groups: false });
  const route = await client.capabilityRoute({ goal: "compose evidence", needs: [{ id: "oncology", query: "oncology" }] });
  const adapter = await client.adapterPlan({ source_id: "scan-1", source_kind: "bytes", declared_format: "application/dicom", available_dependencies: ["pydicom"] });
  assert.equal(capabilities.mcp.result.structuredContent.workflow, "capability_discover");
  assert.equal(capabilityAudit.mcp.result.structuredContent.workflow, "capability_audit");
  assert.equal(capabilityAudit.mcp.result.structuredContent.healthy, true);
  assert.equal(route.mcp.result.structuredContent.workflow, "capability_route");
  assert.equal(route.mcp.result.structuredContent.execution, "not_started");
  assert.equal(adapter.mcp.result.structuredContent.workflow, "adapter_plan");
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
      return new Response("id: 4\nevent: tool.completed\ndata: {\"ok\":true}\n\nevent: cursor_gap\ndata: {\"after\":0}\n\n", {
      headers: { "content-type": "text/event-stream", "x-next-after": "4" },
      });
    },
  });
  const snapshot = await client.eventStream(0, 10);
  assert.equal(snapshot.nextAfter, 4);
  assert.equal(snapshot.events[0].event, "tool.completed");
  assert.deepEqual(JSON.parse(snapshot.events[1].data), { after: 0 });
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
