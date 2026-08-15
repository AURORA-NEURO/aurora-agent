import assert from "node:assert/strict";
import test from "node:test";
import {
  ApiClient,
  ApiError,
  ArgumentError,
  ResponseTooLargeError,
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
      if (path === "/v1/tools") return jsonResponse({ tools: [{ name: "echo", description: "test", inputSchema: { type: "object" } }] });
      if (path === "/v1/tools/echo") return jsonResponse({ ok: true, tool: "echo", request_id: "r1", mcp: { result: { structuredContent: { value: 3 } } }, guarantee: "shared" });
      if (path === "/v1/tools/metrics_analytics_audit") return jsonResponse({ ok: true, tool: "metrics_analytics_audit", request_id: "r3", mcp: { result: { structuredContent: { workflow: "metrics_descriptive_analytics" } } } });
      if (path === "/v1/tools/developer_workbench") return jsonResponse({ ok: true, tool: "developer_workbench", request_id: "r4", mcp: { result: { structuredContent: { workflow: "developer_workbench", audit: { valid: true } } } } });
      if (path === "/v1/tools/capability_discover") return jsonResponse({ ok: true, tool: "capability_discover", request_id: "r6", mcp: { result: { structuredContent: { workflow: "capability_discover", result_count: 1 } } } });
      if (path === "/v1/tools/capability_audit") return jsonResponse({ ok: true, tool: "capability_audit", request_id: "r7", mcp: { result: { structuredContent: { workflow: "capability_audit", healthy: true } } } });
      if (path === "/v1/tools/capability_route") return jsonResponse({ ok: true, tool: "capability_route", request_id: "r8", mcp: { result: { structuredContent: { workflow: "capability_route", execution: "not_started" } } } });
      if (path === "/v1/tools/adapter_plan") return jsonResponse({ ok: true, tool: "adapter_plan", request_id: "r10", mcp: { result: { structuredContent: { workflow: "adapter_plan", executable: true } } } });
      if (path === "/v1/tools/agent_mission") return jsonResponse({ ok: true, tool: "agent_mission", request_id: "r5", mcp: { result: { structuredContent: { workflow: "agent_mission", execution: "planned" } } } });
      if (path === "/v1/tools/refuse") return jsonResponse({ ok: true, tool: "refuse", request_id: "r2", mcp: { result: { isError: true, structuredContent: { reason: "blocked" } } }, guarantee: "shared" });
      return jsonResponse({ ok: true });
    },
  });

  assert.equal((await client.tools())[0].name, "echo");
  const response = await client.callTool("echo", { value: 3 }, { requestId: "request-1" });
  assert.equal(response.mcp.result.structuredContent.value, 3);
  assert.equal(seen.at(-1).init.headers.Authorization, "Bearer 0123456789abcdef");
  assert.equal(seen.at(-1).init.headers["x-request-id"], "request-1");
  const analytics = await client.metricsAnalyticsAudit({ observations: [{ id: "one" }] });
  assert.equal(analytics.mcp.result.structuredContent.workflow, "metrics_descriptive_analytics");
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
  await assert.rejects(client.callTool("unsafe/name"), ArgumentError);
  await assert.rejects(async () => client.requireToolSuccess(await client.callTool("refuse")), ToolRefusalError);
});

test("client parses cursor SSE and validates webhook mutations", async () => {
  const client = new ApiClient({
    baseUrl: "https://example.test",
    fetch: async () => new Response("id: 4\nevent: tool.completed\ndata: {\"ok\":true}\n\nevent: cursor_gap\ndata: {\"after\":0}\n\n", {
      headers: { "content-type": "text/event-stream", "x-next-after": "4" },
    }),
  });
  const snapshot = await client.eventStream(0, 10);
  assert.equal(snapshot.nextAfter, 4);
  assert.equal(snapshot.events[0].event, "tool.completed");
  assert.deepEqual(JSON.parse(snapshot.events[1].data), { after: 0 });
  assert.deepEqual(parseSse("data: a\ndata: b\n\n"), [{ data: "a\nb" }]);
  assert.throws(() => parseSse("retry: nope\n\n"), /retry/);
  await assert.rejects(client.acknowledge("sub", [0]), ArgumentError);
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
