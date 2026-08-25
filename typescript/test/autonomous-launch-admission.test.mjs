import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  CredentialStore,
  LLMRuntime,
  ProviderSetup,
  builtinAutonomousDomainProfiles,
  validateAutonomousLaunchAdmission,
} from "../dist/index.js";

const capabilities = {
  persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
};

async function readyBrain() {
  const runtime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("launch admission must not dispatch"); } });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("openai", { baseUrl: "https://launch-admission.invalid" });
  const session = setup.startSession({ ttlMs: 60_000, sessionId: "launch-admission-test" });
  setup.collectUserCredential(session, "openai", "unit-test-only-not-a-provider-key");
  const profiles = await builtinAutonomousDomainProfiles();
  const modelCapabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(runtime);
  agent.registerModel({ provider: "openai", model: "admission-model", capabilities: modelCapabilities, context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 10, reliability: 0.95 });
  const brain = new AutonomousBrainFacade({ agent });
  const tools = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const evidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const preflight = await brain.launchPreflight({ availableToolNames: tools, availableEvidence: evidence, deploymentCapabilities: capabilities });
  return { brain, preflight, session };
}

test("launch admission holds a blocked preflight across all domains", async () => {
  const runtime = new LLMRuntime();
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(runtime) });
  const preflight = await brain.launchPreflight();
  const admission = brain.admitLaunchPreflight(preflight, { decision: "approve", authorizationDigest: "a".repeat(64) });

  assert.equal(admission.status, "held");
  assert.equal(admission.summary.domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(admission.summary.blocked_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(admission.domains.every((row) => row.admission_state === "blocked"), true);
  assert.deepEqual(validateAutonomousLaunchAdmission(admission), admission);
});

test("launch admission approves every ready domain against one review digest", async () => {
  const fixture = await readyBrain();
  const admission = fixture.brain.admitLaunchPreflight(fixture.preflight, {
    decision: "approve",
    authorizationDigest: "b".repeat(64),
    reason: "reviewed launch gates",
  });

  assert.equal(fixture.preflight.summary.state, "ready_for_review");
  assert.equal(admission.status, "approved");
  assert.equal(admission.summary.approved_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(admission.domains.every((row) => row.admission_state === "approved"), true);
  assert.doesNotMatch(JSON.stringify(admission), /reviewed launch gates|unit-test-only-not-a-provider-key/);
  assert.deepEqual(validateAutonomousLaunchAdmission(admission), admission);
  fixture.session.close();
});

test("launch admission supports reviewed subsets and explicit holds", async () => {
  const fixture = await readyBrain();
  const subset = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "c".repeat(64) });
  assert.equal(subset.status, "approved");
  assert.equal(subset.summary.approved_domain_count, 1);
  assert.equal(subset.summary.not_selected_domain_count, 11);
  assert.equal(subset.domains[0].admission_state, "approved");
  assert.equal(subset.domains.slice(1).every((row) => row.admission_state === "not_selected"), true);

  const held = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "hold", reason: "wait for operator" });
  assert.equal(held.status, "held");
  assert.equal(held.summary.held_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(held.authorization_digest, null);
  assert.doesNotMatch(JSON.stringify(held), /wait for operator/);
  fixture.session.close();
});

test("launch admission refuses missing authority, tampering, and secret-shaped fields", async () => {
  const fixture = await readyBrain();
  assert.throws(() => fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve" }), /authorizationDigest/);
  const admission = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "hold" });
  const tampered = structuredClone(admission);
  tampered.domains[0].admission_state = "approved";
  assert.throws(() => validateAutonomousLaunchAdmission(tampered), /admission_digest/);
  tampered.api_key = "must-not-cross";
  assert.throws(() => validateAutonomousLaunchAdmission(tampered), /secret-shaped/);
  fixture.session.close();
});

test("launch admission gates facade execution before dispatch and checks route coverage", async () => {
  const fixture = await readyBrain();
  try {
    const coding = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "d".repeat(64) });
    const review = await fixture.brain.executeWithLaunchAdmission({ task: "write a small function", domain: "coding" }, coding, { approveProviderCall: false });
    assert.equal(review.status, "approval_required");
    await assert.rejects(
      () => fixture.brain.executeWithLaunchAdmission({ task: "write a small function", domain: "biomedical" }, coding, { approveProviderCall: false }),
      /does not approve requested domains/,
    );
    const held = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "hold" });
    await assert.rejects(
      () => fixture.brain.executeWithLaunchAdmission({ task: "write a small function", domain: "coding" }, held, { approveProviderCall: false }),
      /not approved/,
    );
  } finally {
    fixture.session.close();
  }
});
