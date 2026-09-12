import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError, ProtocolError } from "../dist/index.js";

function jsonResponse(value, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function campaignResult(state = "not_started", overrides = {}) {
  const campaignStatus = state === "not_started" ? "planned" : state;
  const stageDispositions = {
    completed: "succeeded",
    awaiting_human_review: "awaiting_human_review",
    refused: "refused",
    needs_input: "missing_input",
    exhausted: "exhausted",
  };
  const settled = state in stageDispositions;
  const execution = state === "reconciliation_required"
    ? { state, reason: "durable state requires reconciliation" }
    : settled && state !== "completed"
      ? { state, stage_id: "measure" }
      : { state };
  const stage = settled
    ? {
        state: "settled",
        stage_id: "measure",
        kind: "synthetic_research",
        input_digest: "b".repeat(64),
        action_ordinal: 1,
        disposition: stageDispositions[state],
        artifact_digest: "c".repeat(64),
        receipt_digest: "d".repeat(64),
        artifact_locator: "artifacts/0001-research-dossier.json",
        file_sha256: "e".repeat(64),
      }
    : {
        state: "not_started",
        stage_id: "measure",
        kind: "synthetic_research",
        input_digest: "b".repeat(64),
        artifact_locator: "artifacts/0001-research-dossier.json",
      };
  const durable = settled
    ? {
        checkpoint: { locator: "campaign.checkpoint.json", schema: "bioprism-research-campaign-checkpoint/0.1", generation: 2, snapshot_digest: "f".repeat(64) },
        trusted_head: { locator: "campaign.head.json", campaign_id: "campaign-1", spec_digest: "a".repeat(64), generation: 2, snapshot_digest: "f".repeat(64) },
        manifest: { locator: "campaign.manifest.json", digest: "1".repeat(64), file_sha256: "1".repeat(64) },
        written: [
          "campaign/output/artifacts/0001-research-dossier.json",
          "campaign/output/authority/0001-authorization.json",
          "campaign/output/authority/0002-terminal.json",
          "campaign/output/campaign.checkpoint.json",
          "campaign/output/campaign.manifest.json",
          "campaign/output/campaign.head.json",
        ],
      }
    : { checkpoint: null, trusted_head: null, manifest: null, written: [] };
  return {
    schema: "bioprism-mcp/research-campaign-offline-run/0.1",
    workflow: "research_campaign_run_offline",
    campaign_id: "campaign-1",
    spec_digest: "a".repeat(64),
    execution,
    campaign_status: campaignStatus,
    actions_used: settled ? 1 : 0,
    stages: [stage],
    ...durable,
    limitations: [
      "supports only synthetic_research and brain_plan campaign stages",
      "synthetic_research measures seeded repository fixtures and does not search external literature",
      "brain_plan validates and orders a plan but never executes its steps",
      "this first slice has no resume or execution-journal reconciliation; an interrupted output directory must be inspected rather than retried",
    ],
    ...overrides,
  };
}

function twoStageCompletedResult() {
  return campaignResult("completed", {
    actions_used: 2,
    stages: [
      {
        state: "settled",
        stage_id: "measure",
        kind: "synthetic_research",
        input_digest: "b".repeat(64),
        action_ordinal: 1,
        disposition: "completed_with_negative_findings",
        artifact_digest: "3".repeat(64),
        receipt_digest: "4".repeat(64),
        artifact_locator: "artifacts/0001-research-dossier.json",
        file_sha256: "5".repeat(64),
      },
      {
        state: "settled",
        stage_id: "plan",
        kind: "brain_plan",
        input_digest: "6".repeat(64),
        action_ordinal: 2,
        disposition: "succeeded",
        artifact_digest: "7".repeat(64),
        receipt_digest: "8".repeat(64),
        artifact_locator: "artifacts/0002-brain-plan-report.json",
        file_sha256: "9".repeat(64),
      },
    ],
    checkpoint: { locator: "campaign.checkpoint.json", schema: "bioprism-research-campaign-checkpoint/0.1", generation: 3, snapshot_digest: "f".repeat(64) },
    trusted_head: { locator: "campaign.head.json", campaign_id: "campaign-1", spec_digest: "a".repeat(64), generation: 3, snapshot_digest: "f".repeat(64) },
    manifest: { locator: "campaign.manifest.json", digest: "1".repeat(64), file_sha256: "1".repeat(64) },
    written: [
      "campaign/output/artifacts/0001-research-dossier.json",
      "campaign/output/artifacts/0002-brain-plan-report.json",
      "campaign/output/authority/0001-authorization.json",
      "campaign/output/authority/0002-authorization.json",
      "campaign/output/authority/0003-terminal.json",
      "campaign/output/campaign.checkpoint.json",
      "campaign/output/campaign.manifest.json",
      "campaign/output/campaign.head.json",
    ],
  });
}

function toolEnvelope(result, overrides = {}) {
  return {
    ...rustWireEnvelope(result),
    ...overrides,
  };
}

function rustWireEnvelope(result) {
  return {
    ok: true,
    tool: "research_campaign_run_offline",
    request_id: "request-42",
    mcp: {
      jsonrpc: "2.0",
      id: "request-42",
      result: {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        isError: false,
      },
    },
    guarantee: "REST and MCP calls share the same in-process tool dispatcher",
  };
}

const validArgs = {
  spec_path: "campaign/spec.json",
  stage_input_paths: { measure: "campaign/inputs/measure.json" },
  output_dir: "campaign/output",
};

test("offline campaign sends only validated paths and makes preview confirmation explicit", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      seen.push({ input: String(input), init });
      return jsonResponse(toolEnvelope(campaignResult()));
    },
  });

  const response = await client.researchCampaignRunOffline(validArgs, { requestId: "request-42" });
  assert.equal(response.mcp.result.structuredContent.execution.state, "not_started");
  assert.equal(seen.length, 1);
  assert.equal(new URL(seen[0].input).pathname, "/v1/tools/research_campaign_run_offline");
  assert.equal(seen[0].init.headers["x-request-id"], "request-42");
  assert.deepEqual(JSON.parse(seen[0].init.body), { ...validArgs, confirm: false });
});

test("offline campaign parses and normalizes the actual Rust content-only wire shape", async () => {
  const result = campaignResult();
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(rustWireEnvelope(result)),
  });
  const response = await client.researchCampaignRunOffline(validArgs);
  assert.deepEqual(response.mcp.result.structuredContent, result);
  assert.equal(response.mcp.result.content.length, 1);

  const conflicting = rustWireEnvelope(result);
  conflicting.mcp.result.structuredContent = { ...result, campaign_id: "another-campaign" };
  const conflictClient = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(conflicting),
  });
  await assert.rejects(conflictClient.researchCampaignRunOffline(validArgs), ProtocolError);
});

test("offline campaign rejects ambiguous paths, invalid stage maps, and non-boolean confirmation before transport", async () => {
  let calls = 0;
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => {
      calls += 1;
      return jsonResponse(toolEnvelope(campaignResult()));
    },
  });

  const invalid = [
    { ...validArgs, spec_path: "/campaign/spec.json" },
    { ...validArgs, spec_path: 42 },
    { ...validArgs, spec_path: "   " },
    { ...validArgs, spec_path: "campaign/../spec.json" },
    { ...validArgs, output_dir: "C:\\campaign\\output" },
    { ...validArgs, stage_input_paths: {} },
    { ...validArgs, stage_input_paths: Object.fromEntries(Array.from({ length: 9 }, (_, index) => [`s${index}`, `inputs/${index}.json`])) },
    { ...validArgs, stage_input_paths: { measure: "../outside.json" } },
    { ...validArgs, confirm: "true" },
    { ...validArgs, verify_existing: true },
    { ...validArgs, inline_objective: "must never cross this boundary" },
  ];
  for (const args of invalid) {
    await assert.rejects(client.researchCampaignRunOffline(args), ArgumentError);
  }
  assert.equal(calls, 0);
});

test("offline campaign preserves every terminal or paused execution state without deriving success", async () => {
  const states = [
    "not_started",
    "completed",
    "awaiting_human_review",
    "refused",
    "needs_input",
    "exhausted",
    "reconciliation_required",
  ];
  for (const state of states) {
    const client = new ApiClient({
      baseUrl: "http://127.0.0.1:18788",
      fetch: async () => jsonResponse(toolEnvelope(campaignResult(state))),
    });
    const response = await client.researchCampaignRunOffline({ ...validArgs, confirm: state !== "not_started" });
    assert.equal(response.mcp.result.structuredContent.execution.state, state);
  }
});

test("offline campaign keeps MCP refusals intact", async () => {
  const refusal = {
    ok: true,
    tool: "research_campaign_run_offline",
    request_id: "r-refused",
    mcp: {
      jsonrpc: "2.0",
      id: "r-refused",
      result: {
        isError: true,
        content: [{ type: "text", text: JSON.stringify({ ok: false, error: "unsupported stage kind" }) }],
      },
    },
    guarantee: "REST and MCP calls share the same in-process tool dispatcher",
  };
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(refusal),
  });
  assert.deepEqual(await client.researchCampaignRunOffline(validArgs), refusal);
});

test("offline campaign rejects malformed outer success envelopes", async () => {
  const valid = toolEnvelope(campaignResult());
  const malformed = [
    { ...valid, tool: "some_other_tool" },
    { ...valid, mcp: {} },
    { ...valid, mcp: { jsonrpc: "2.0", id: "request-42", result: "not-an-object" } },
    { ...valid, mcp: { jsonrpc: "2.0", id: "request-42", result: { isError: "false", content: valid.mcp.result.content } } },
    { ...valid, mcp: { ...valid.mcp, id: "cross-wired-request" } },
    { ...valid, mcp: { ...valid.mcp, jsonrpc: "1.0" } },
    {
      ...valid,
      mcp: {
        ...valid.mcp,
        result: {
          ...valid.mcp.result,
          content: [{ ...valid.mcp.result.content[0], raw_objective: "must not escape" }],
        },
      },
    },
    { ...valid, guarantee: "bounded" },
    { ...valid, raw_result: campaignResult() },
    {
      ...valid,
      mcp: {
        ...valid.mcp,
        result: {
          ...valid.mcp.result,
          isError: true,
          structuredContent: campaignResult("completed"),
          content: [{ type: "text", text: JSON.stringify({ ok: false, error: "refused" }) }],
        },
      },
    },
  ];
  for (const envelope of malformed) {
    const client = new ApiClient({
      baseUrl: "http://127.0.0.1:18788",
      fetch: async () => jsonResponse(envelope),
    });
    await assert.rejects(client.researchCampaignRunOffline(validArgs), ProtocolError);
  }

  const requestBoundClient = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(valid),
  });
  await assert.rejects(
    requestBoundClient.researchCampaignRunOffline(validArgs, { requestId: "caller-bound-request" }),
    ProtocolError,
  );
});

test("offline campaign represents a durable unsettled authorization only as reconciliation-required", async () => {
  const digest = "f".repeat(64);
  const result = campaignResult("reconciliation_required", {
    actions_used: 2,
    stages: [
      {
        state: "settled",
        stage_id: "measure",
        kind: "synthetic_research",
        input_digest: "b".repeat(64),
        action_ordinal: 1,
        disposition: "completed_with_negative_findings",
        artifact_digest: "3".repeat(64),
        receipt_digest: "4".repeat(64),
        artifact_locator: "artifacts/0001-research-dossier.json",
        file_sha256: "5".repeat(64),
      },
      {
        state: "reconciliation_required",
        stage_id: "plan",
        kind: "brain_plan",
        input_digest: "6".repeat(64),
        action_ordinal: 2,
        authorization_digest: "9".repeat(64),
        artifact_locator: "artifacts/0002-brain-plan-report.json",
        reason: "authorization persisted but completion is unknown",
      },
    ],
    checkpoint: { locator: "authority/0002-authorization.json#/checkpoint", schema: "bioprism-research-campaign-checkpoint/0.1", generation: 2, snapshot_digest: digest },
    trusted_head: { locator: "authority/0002-authorization.json#/candidate_checkpoint_head", campaign_id: "campaign-1", spec_digest: "a".repeat(64), generation: 2, snapshot_digest: digest },
    manifest: null,
    written: [
      "campaign/output/artifacts/0001-research-dossier.json",
      "campaign/output/authority/0001-authorization.json",
      "campaign/output/authority/0002-authorization.json",
    ],
  });
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(toolEnvelope(result)),
  });
  const response = await client.researchCampaignRunOffline({
    ...validArgs,
    stage_input_paths: {
      measure: "campaign/inputs/measure.json",
      plan: "campaign/inputs/plan.json",
    },
    confirm: true,
  });
  assert.equal(response.mcp.result.structuredContent.execution.state, "reconciliation_required");
  assert.equal(response.mcp.result.structuredContent.stages[0].state, "settled");
  assert.equal(response.mcp.result.structuredContent.stages[1].state, "reconciliation_required");
  assert.equal(response.mcp.result.structuredContent.manifest, null);
});

test("offline campaign accepts a terminally committed unknown-completion reconciliation", async () => {
  const result = campaignResult("reconciliation_required", {
    actions_used: 1,
    stages: [{
      state: "reconciliation_required",
      stage_id: "measure",
      kind: "synthetic_research",
      input_digest: "b".repeat(64),
      action_ordinal: 1,
      authorization_digest: "9".repeat(64),
      artifact_locator: "artifacts/0001-research-dossier.json",
      reason: "native execution returned without a trustworthy completion receipt",
    }],
    checkpoint: { locator: "campaign.checkpoint.json", schema: "bioprism-research-campaign-checkpoint/0.1", generation: 2, snapshot_digest: "f".repeat(64) },
    trusted_head: { locator: "campaign.head.json", campaign_id: "campaign-1", spec_digest: "a".repeat(64), generation: 2, snapshot_digest: "f".repeat(64) },
    manifest: { locator: "campaign.manifest.json", digest: "1".repeat(64), file_sha256: "1".repeat(64) },
    written: [
      "campaign/output/authority/0001-authorization.json",
      "campaign/output/authority/0002-terminal.json",
      "campaign/output/campaign.checkpoint.json",
      "campaign/output/campaign.manifest.json",
      "campaign/output/campaign.head.json",
    ],
  });
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(toolEnvelope(result)),
  });
  const response = await client.researchCampaignRunOffline({ ...validArgs, confirm: true });
  assert.equal(response.mcp.result.structuredContent.execution.state, "reconciliation_required");
  assert.equal(response.mcp.result.structuredContent.manifest.locator, "campaign.manifest.json");
});

test("offline campaign binds each action ordinal to its exact campaign stage", async () => {
  const request = {
    ...validArgs,
    stage_input_paths: {
      measure: "campaign/inputs/measure.json",
      plan: "campaign/inputs/plan.json",
    },
    confirm: true,
  };
  const canonical = twoStageCompletedResult();
  const canonicalClient = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(toolEnvelope(canonical)),
  });
  const response = await canonicalClient.researchCampaignRunOffline(request);
  assert.deepEqual(response.mcp.result.structuredContent.stages.map((stage) => stage.action_ordinal), [1, 2]);

  const swapped = structuredClone(canonical);
  swapped.stages[0].action_ordinal = 2;
  swapped.stages[1].action_ordinal = 1;
  const swappedClient = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async () => jsonResponse(toolEnvelope(swapped)),
  });
  await assert.rejects(swappedClient.researchCampaignRunOffline(request), ProtocolError);
});

test("offline campaign rejects malformed positive metadata", async () => {
  const invalidResults = [
    campaignResult("completed", { spec_digest: "not-a-digest" }),
    campaignResult("completed", { execution: { state: "probably_completed" } }),
    campaignResult("completed", { actions_used: 9 }),
    campaignResult("completed", { campaign_status: "ready" }),
    campaignResult("completed", { manifest: null }),
    campaignResult("completed", { manifest: { locator: "campaign.manifest.json", digest: "1".repeat(64), file_sha256: "2".repeat(64) } }),
    campaignResult("completed", { limitations: ["external literature was searched"] }),
    campaignResult("completed", { written: [...campaignResult("completed").written, "campaign/output/unrecognized.json"] }),
    { ...campaignResult(), objective: "raw objective must be refused" },
    campaignResult("not_started", { stages: [] }),
    campaignResult("reconciliation_required", { actions_used: 1 }),
    campaignResult("reconciliation_required", {
      checkpoint: { locator: "authority/0001-authorization.json#/checkpoint", schema: "bioprism-research-campaign-checkpoint/0.1", generation: 1, snapshot_digest: "f".repeat(64) },
      trusted_head: { locator: "authority/0001-authorization.json#/candidate_checkpoint_head", campaign_id: "campaign-1", spec_digest: "a".repeat(64), generation: 1, snapshot_digest: "f".repeat(64) },
    }),
    campaignResult("completed", {
      actions_used: 2,
      stages: [
        { state: "settled", stage_id: "a", kind: "brain_plan", action_ordinal: 1, input_digest: "b".repeat(64), disposition: "succeeded", artifact_digest: "d".repeat(64), receipt_digest: "e".repeat(64), artifact_locator: "artifacts/0001-brain-plan.json", file_sha256: "f".repeat(64) },
        { state: "settled", stage_id: "b", kind: "brain_plan", action_ordinal: 1, input_digest: "c".repeat(64), disposition: "succeeded", artifact_digest: "3".repeat(64), receipt_digest: "4".repeat(64), artifact_locator: "artifacts/0002-brain-plan.json", file_sha256: "5".repeat(64) },
      ],
    }),
  ];
  for (const result of invalidResults) {
    const client = new ApiClient({
      baseUrl: "http://127.0.0.1:18788",
      fetch: async () => jsonResponse(toolEnvelope(result)),
    });
    await assert.rejects(
      client.researchCampaignRunOffline({ ...validArgs, confirm: result.execution?.state !== "not_started" }),
      ProtocolError,
    );
  }
});
