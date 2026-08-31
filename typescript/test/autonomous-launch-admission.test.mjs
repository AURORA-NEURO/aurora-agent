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
  return { agent, brain, preflight, session };
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
    const preview = await fixture.brain.modelSelectionPreview({ task: "write a small function", domain: "coding" });
    await assert.rejects(
      () => fixture.brain.executeApprovedSelectionWithLaunchAdmission({ task: "write a small function", domain: "coding" }, preview, held),
      /not approved/,
    );
    await assert.rejects(
      () => fixture.brain.executeWithLaunchAdmission({ task: "write a small function", domain: "coding" }, coding, { semanticRouting: { enabled: true, approveProviderCall: true }, approveProviderCall: false }),
      /requires provider-free routing/,
    );
  } finally {
    fixture.session.close();
  }
});

test("launch admission gates automatic decision cycles across every domain before provider dispatch", async () => {
  const fixture = await readyBrain();
  try {
    const admission = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", authorizationDigest: "f".repeat(64) });
    const tasks = {
      coding: "debug this Rust repository implementation",
      browser: "compare browser research sources",
      data: "validate this dataset schema",
      science: "design a scientific experiment hypothesis",
      biomedical: "review biomedical clinical evidence",
      neuroscience: "analyze neuroscience neural signals",
      operations: "plan an operations incident rollback",
      enterprise: "review enterprise governance compliance",
      multi_agent: "delegate a multi agent specialist subtask",
      multimodal: "align multimodal image audio evidence",
      evaluation: "run an evaluation benchmark holdout",
    };
    for (const [domain, task] of Object.entries(tasks)) {
      const result = await fixture.agent.runAutoCycleWithLaunchAdmission(task, admission, { domain, approveProviderCall: false });
      assert.equal(result.route.selected_domains.length, 1, domain);
      assert.equal(result.route.selected_domains[0], domain, domain);
      assert.equal(result.status, "approval_required", domain);
    }

    const codingOnly = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "1".repeat(64) });
    await assert.rejects(
      () => fixture.agent.runAutoCycleWithLaunchAdmission("review the biomedical evidence", codingOnly, { domain: "biomedical", approveProviderCall: false }),
      /does not approve requested domains/,
    );
    const held = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "hold" });
    await assert.rejects(
      () => fixture.agent.runAutoCycleWithLaunchAdmission("debug this repository", held, { domain: "coding", approveProviderCall: false }),
      /not approved/,
    );
    await assert.rejects(
      () => fixture.agent.runAutoCycleWithLaunchAdmission("debug this repository", admission, { domain: "coding", semanticRouting: { enabled: true, approveProviderCall: true } }),
      /requires provider-free routing/,
    );
    const replan = await fixture.agent.runAutoReplanCycleWithLaunchAdmission("debug this repository", admission, {
      domain: "coding",
      approveProviderCall: false,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "unused-launch-reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: false }),
    });
    assert.equal(replan.route.primary_domain, "coding");
    assert.equal(replan.status, "approval_required");
    const route = await fixture.agent.route("debug this repository", { domain: "coding" });
    await assert.rejects(
      () => fixture.agent.runAutoCycleWithLaunchAdmission("debug this repository", admission, { routeOverride: route, approveProviderCall: false }),
      /owns routeOverride/,
    );
  } finally {
    fixture.session.close();
  }
});

test("launch admission gates high-level direct and automatic runs across every domain", async () => {
  const fixture = await readyBrain();
  try {
    const admission = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", authorizationDigest: "3".repeat(64) });
    const tasks = {
      coding: "write a small function",
      browser: "compare browser research sources",
      data: "validate this dataset schema",
      science: "design a scientific experiment hypothesis",
      biomedical: "review biomedical clinical evidence",
      neuroscience: "analyze neuroscience neural signals",
      operations: "plan an operations incident rollback",
      enterprise: "review enterprise governance compliance",
      multi_agent: "delegate a multi agent specialist subtask",
      multimodal: "align multimodal image audio evidence",
      evaluation: "run an evaluation benchmark holdout",
    };
    for (const [domain, task] of Object.entries(tasks)) {
      const direct = await fixture.agent.runWithLaunchAdmission(task, admission, { domain, approveProviderCall: false });
      assert.equal(direct.status, "approval_required", `direct ${domain}`);
      const automatic = await fixture.agent.runAutoWithLaunchAdmission(task, admission, { domain, approveProviderCall: false });
      assert.equal(automatic.status, "approval_required", `automatic ${domain}`);
      assert.equal(automatic.route.selected_domains[0], domain, `route ${domain}`);
    }

    const preview = await fixture.agent.authorizeAutoLaunchAdmission("write a small function", admission, { domain: "coding" });
    assert.equal(preview.admission_digest, admission.admission_digest);
    const boundedCrossDomain = await fixture.agent.runAutoWithLaunchAdmission("coding data", admission, {
      minConfidence: 0,
      minMargin: 1,
      maxDomains: 2,
      allowCrossDomain: true,
      approveProviderCall: false,
    });
    assert.equal(boundedCrossDomain.route.cross_domain, true);
    assert.deepEqual(boundedCrossDomain.route.selected_domains, ["coding", "data"]);

    const codingOnly = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "2".repeat(64) });
    await assert.rejects(
      () => fixture.agent.runAutoWithLaunchAdmission("review biomedical clinical evidence", codingOnly, { domain: "biomedical", credential: { malformed: true }, approveProviderCall: false }),
      /does not approve requested domains/,
    );
    await assert.rejects(
      () => fixture.agent.runWithLaunchAdmission("write a small function", admission, { domain: "coding", semanticRouting: true }),
      /requires provider-free routing/,
    );
    const route = await fixture.agent.route("write a small function", { domain: "coding" });
    await assert.rejects(
      () => fixture.agent.runAutoWithLaunchAdmission("write a small function", admission, { routeOverride: route, approveProviderCall: false }),
      /owns routeOverride/,
    );
  } finally {
    fixture.session.close();
  }
});

test("launch admission gates ordinary, resumable, and cycle batches before dispatch", async () => {
  const fixture = await readyBrain();
  try {
    const coding = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "e".repeat(64) });
    await assert.rejects(
      () => fixture.brain.executeBatchWithLaunchAdmission([{ task: "review the biomedical evidence", domain: "biomedical" }], coding, { execution: { approveProviderCall: false } }),
      /does not approve requested domains/,
    );
    await assert.rejects(
      () => fixture.brain.executeBatchWithLaunchAdmission([{ task: "route this multidisciplinary review" }], coding, { execution: { semanticRouting: { enabled: true, approveProviderCall: true }, approveProviderCall: false } }),
      /requires provider-free routing/,
    );
    const held = fixture.brain.admitLaunchPreflight(fixture.preflight, { decision: "hold" });
    await assert.rejects(
      () => fixture.brain.executeBatchResumableWithLaunchAdmission([{ task: "review the implementation", domain: "coding" }], held, { jobId: "held-batch" }),
      /not approved/,
    );
    await assert.rejects(
      () => fixture.brain.executeCycleBatchWithLaunchAdmission([{ task: "review the biomedical evidence", domain: "biomedical" }], coding),
      /does not approve requested domains/,
    );
  } finally {
    fixture.session.close();
  }
});
