import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousBrainControlPlaneMonitor,
  ArgumentError,
  ProviderRuntimeError,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

function job(domain, index, state = "queued") {
  return {
    schema: "bioprism-brain-job/0.1",
    job_id: `control-job-${index}`,
    idempotency_key_digest: digest("a"),
    spec_digest: digest("b"),
    domain,
    capability: "bounded_task",
    risk_class: "review",
    priority: 0,
    max_attempts: 3,
    state,
    attempts: 0,
    lease_owner: null,
    lease_expires_ns: null,
    checkpoint_digest: null,
    side_effect_boundary: "not_started",
    recovered_after_restart: false,
    reason_digest: null,
    created_sequence: index + 1,
    updated_sequence: index + 1,
    record_digest: digest("c"),
    spec: "not_returned; caller resolver owns rehydration",
    retention: "metadata_only_hash_chained",
  };
}

function response(tool, structuredContent) {
  return {
    ok: true,
    tool,
    request_id: `request-${tool}`,
    mcp: { result: { structuredContent } },
    guarantee: "metadata_only",
  };
}

function statusResult(record) {
  return response("brain_job_status", { schema: "bioprism-brain-control-plane/0.1", ok: true, job: record, head_digest: digest("d"), durability: { scope: "test", restart: "caller_must_rehydrate_from_durable_job_store", secrets: "never_retained" } });
}

function eventsResult(jobId, after = 0, events = []) {
  return response("brain_job_events", { schema: "bioprism-brain-control-plane/0.1", ok: true, events, after, next_after: events.at(-1)?.sequence ?? after, head_digest: digest("d"), chain: "sha256_prev_digest", retention: "metadata_only_hash_chained", durability: { scope: "test", restart: "caller_must_rehydrate_from_durable_job_store", secrets: "never_retained" }, job_id: jobId });
}

test("control-plane monitor fans out status across every autonomous domain", async () => {
  const records = new Map(AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => [`control-job-${index}`, job(domain, index)]));
  let inFlight = 0;
  let maxInFlight = 0;
  const client = {
    brainJobStatus: async ({ job_id }) => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await Promise.resolve();
      inFlight -= 1;
      return statusResult(records.get(job_id));
    },
    brainJobEvents: async ({ job_id, after = 0 }) => eventsResult(job_id, after),
    brainJobApproval: async ({ job_id }) => response("brain_job_approval", { schema: "bioprism-brain-control-plane/0.1", ok: true, job: records.get(job_id), event: { sequence: 1 }, authorization: { posture: "caller_authenticated_out_of_band", verified_by_server: false, execution: "not_started" }, durability: { scope: "test", restart: "caller_must_rehydrate_from_durable_job_store", secrets: "never_retained" } }),
  };
  const monitor = new AutonomousBrainControlPlaneMonitor({ client });
  const result = await monitor.statusAll([...records.keys()], { maxParallel: 3 });
  assert.equal(result.status, "completed");
  assert.equal(result.jobs.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(new Set(result.domains), new Set(AUTONOMOUS_DOMAIN_NAMES));
  assert.equal(result.max_parallel, 3);
  assert.equal(maxInFlight <= 3, true);
});

test("control-plane monitor validates approval authorization and collects bounded event cursors", async () => {
  const record = job("coding", 90);
  let approvalArgs;
  const event = { schema: "bioprism-brain-job-event/0.1", sequence: 1, event_type: "job_checkpointed", job_id: record.job_id, payload: { state: "waiting_approval" }, previous_digest: "", event_digest: digest("e"), head_digest: digest("e"), created_ns: 1, retention: "metadata_only_hash_chained" };
  const client = {
    brainJobStatus: async () => statusResult(record),
    brainJobEvents: async ({ job_id, after = 0 }) => eventsResult(job_id, after, after === 0 ? [event] : []),
    brainJobApproval: async (args) => {
      approvalArgs = args;
      return response("brain_job_approval", { schema: "bioprism-brain-control-plane/0.1", ok: true, job: { ...record, state: "queued" }, event, authorization: { posture: "caller_authenticated_out_of_band", verified_by_server: false, execution: "not_started" }, durability: { scope: "test", restart: "caller_must_rehydrate_from_durable_job_store", secrets: "never_retained" } });
    },
  };
  const monitor = new AutonomousBrainControlPlaneMonitor({ client });
  await assert.rejects(monitor.approval(record.job_id, "approve"), ArgumentError);
  await monitor.approval(record.job_id, "approve", { reason: "reviewed scope", authorizationDigest: digest("f") });
  assert.deepEqual(approvalArgs, { job_id: record.job_id, action: "approve", reason: "reviewed scope", authorization_digest: digest("f") });
  const events = await monitor.events(record.job_id, 0, 4);
  assert.equal(events.events.events.length, 1);
  assert.equal(events.events.next_after, 1);

  const cursorClient = { ...client, brainJobEvents: async ({ job_id }) => eventsResult(job_id, 1, []) };
  await assert.rejects(new AutonomousBrainControlPlaneMonitor({ client: cursorClient }).events(record.job_id, 0, 4), ProviderRuntimeError);

  const first = { ...event, event_digest: digest("1") };
  const second = { ...event, sequence: 2, previous_digest: digest("2"), event_digest: digest("3") };
  const brokenChainClient = { ...client, brainJobEvents: async ({ job_id, after = 0 }) => eventsResult(job_id, after, after === 0 ? [first, second] : []) };
  await assert.rejects(new AutonomousBrainControlPlaneMonitor({ client: brokenChainClient }).events(record.job_id, 0, 4), ProviderRuntimeError);
});

test("control-plane monitor reaches terminal state, times out explicitly, and refuses unsafe projections", async () => {
  const record = job("science", 91);
  let state = "queued";
  let now = 0;
  let transitionOnSleep = true;
  const event = { schema: "bioprism-brain-job-event/0.1", sequence: 1, event_type: "job_completed", job_id: record.job_id, payload: { state: "succeeded" }, previous_digest: "", event_digest: digest("e"), head_digest: digest("e"), created_ns: 1, retention: "metadata_only_hash_chained" };
  const client = {
    brainJobStatus: async () => statusResult({ ...record, state }),
    brainJobEvents: async ({ job_id, after = 0 }) => eventsResult(job_id, after, after === 0 && state === "succeeded" ? [event] : []),
    brainJobApproval: async () => response("brain_job_approval", { schema: "bioprism-brain-control-plane/0.1", ok: true, job: { ...record, state: "queued" }, event, authorization: { posture: "caller_authenticated_out_of_band", verified_by_server: false, execution: "not_started" }, durability: { scope: "test", restart: "caller_must_rehydrate_from_durable_job_store", secrets: "never_retained" } }),
  };
  const monitor = new AutonomousBrainControlPlaneMonitor({ client, clock: () => now, sleep: async (milliseconds) => { now += milliseconds; if (transitionOnSleep) state = "succeeded"; } });
  const reached = await monitor.wait(record.job_id, { timeoutMs: 10, pollMs: 1, maxPolls: 4 });
  assert.equal(reached.status, "reached");
  assert.equal(reached.terminal_state, "succeeded");
  assert.equal(reached.events.length, 1);

  state = "waiting_approval";
  now = 0;
  transitionOnSleep = false;
  const timedOut = await monitor.wait(record.job_id, { timeoutMs: 2, pollMs: 1, maxPolls: 2 });
  assert.equal(timedOut.status, "timed_out");
  assert.equal(timedOut.terminal_state, "waiting_approval");

  const unsafeClient = { ...client, brainJobStatus: async () => response("brain_job_status", { job: { ...record, prompt: "must not cross boundary" }, head_digest: digest("d") }) };
  await assert.rejects(new AutonomousBrainControlPlaneMonitor({ client: unsafeClient }).status(record.job_id), ProviderRuntimeError);
});
