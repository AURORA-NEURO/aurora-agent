import test from "node:test";
import assert from "node:assert/strict";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousDomainEvidenceSourceCatalogue,
  AutonomousEvidenceAcquisitionError,
  AutonomousDomainEvidenceSourceProfile,
  AutonomousEvidenceNormalizerRegistration,
  AutonomousEvidenceNormalizerSpec,
  buildAutonomousEvidencePlan,
  builtinAutonomousDomainEvidenceSourceProfiles,
  builtinAutonomousDomainProfiles,
  createBuiltinAutonomousDomainEvidenceSourceCatalogue,
  createBuiltinAutonomousEvidenceNormalizerRegistry,
  digestJsonSync,
} from "../dist/index.js";

async function evidencePlan() {
  const profiles = await builtinAutonomousDomainProfiles();
  return buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
}

function digest(value) {
  return digestJsonSync(value);
}

test("domain evidence catalogue binds two reviewed source routes across all autonomous domains", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const profiles = builtinAutonomousDomainEvidenceSourceProfiles();
  const plan = await evidencePlan();

  assert.deepEqual(profiles.map((profile) => profile.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(catalogue.profiles().length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(catalogue.toJSON().covered_domain_count, 0);

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const profile = profiles.find((candidate) => candidate.domain === domain);
    assert.ok(profile);
    const requirement = plan.requirements.find((candidate) => candidate.domain === domain);
    assert.ok(requirement);
    for (const suffix of ["a", "b"]) {
      const sourceId = `${domain}-source-${suffix}`;
      catalogue.registerRoute({
        sourceId,
        profileId: profile.profile_id,
        provider: `fixture-${suffix}`,
        sourceDigest: digest({ sourceId }),
        requestId: `${sourceId}-request`,
        metadata: { operation: profile.operations[0], query_digest: digest({ domain, suffix }) },
        acquirer: {
          acquire: async (context) => ({
            claim: `${domain}:shared-claim`,
            provider_marker: context.request.source_id,
          }),
        },
      });
    }
    const prepared = catalogue.prepare(plan, requirement.requirement_id, {
      profileId: profile.profile_id,
      quorum: 2,
      maxConcurrency: 2,
    });
    assert.equal(prepared.plan.normalizer_id, profile.normalizer_id);
    const result = await catalogue.execute(plan, prepared, {
      approveSourceDispatch: true,
      normalizer: (value) => ({ claim: value.claim }),
    });
    assert.equal(result.json.status, "consensus");
    assert.equal(result.json.observed_count, 2);
    assert.equal(result.json.consensus_normalized_digest, digest({ claim: `${domain}:shared-claim` }));
    assert.equal(result.values[`${domain}-source-a`].provider_marker, `${domain}-source-a`);
    assert.equal(result.normalizedValues[`${domain}-source-b`].claim, `${domain}:shared-claim`);
    assert.equal(result.json.source_results.every((row) => !Object.hasOwn(row, "metadata")), true);
  }

  const projection = JSON.stringify(catalogue.toJSON());
  assert.equal(catalogue.toJSON().route_count, AUTONOMOUS_DOMAIN_NAMES.length * 2);
  assert.equal(catalogue.toJSON().covered_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(projection.includes("query_digest"), false);
  assert.equal(projection.includes("provider_marker"), false);
});

test("domain evidence catalogue preserves dissent, typed failure, and explicit approval", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const plan = await evidencePlan();
  const profile = catalogue.profile("builtin.science.evidence");
  const requirement = plan.requirements.find((candidate) => candidate.domain === "science");
  assert.ok(requirement);
  for (const [sourceId, claim] of [["science-literature", "claim-a"], ["science-registry", "claim-b"]]) {
    catalogue.registerRoute({
      sourceId,
      profileId: profile.profile_id,
      provider: sourceId,
      sourceDigest: digest({ sourceId }),
      requestId: `${sourceId}-request`,
      metadata: { operation: profile.operations[0] },
      acquirer: { acquire: async () => ({ claim }) },
    });
  }
  catalogue.registerRoute({
    sourceId: "science-failing-source",
    profileId: profile.profile_id,
    provider: "fixture-failure",
    sourceDigest: digest({ sourceId: "science-failing-source" }),
    requestId: "science-failing-request",
    metadata: { operation: profile.operations[0] },
    acquirer: { acquire: async () => { throw new AutonomousEvidenceAcquisitionError("transport_error", true); } },
  });

  const prepared = catalogue.prepare(plan, requirement.requirement_id, {
    profileId: profile.profile_id,
    quorum: 2,
    maxConcurrency: 3,
  });
  await assert.rejects(
    () => catalogue.execute(plan, prepared, { normalizer: (value) => value }),
    /explicit approval/,
  );
  const result = await catalogue.execute(plan, prepared, {
    approveSourceDispatch: true,
    normalizer: (value) => ({ claim: value.claim }),
  });
  assert.equal(result.json.status, "disagreement");
  assert.equal(result.json.observed_count, 2);
  assert.equal(result.json.failed_count, 1);
  assert.equal(result.json.unique_normalized_count, 2);
  assert.equal(result.json.consensus_normalized_digest, null);
  assert.ok(result.json.disagreement_digest);
  const failed = result.json.source_results.find((row) => row.source_id === "science-failing-source");
  assert.equal(failed.failure_class, "transport_error");
  assert.equal(failed.retryable, true);
});

test("domain evidence catalogue fails closed on profile, route, metadata, and restart drift", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const plan = await evidencePlan();
  const profile = catalogue.profile("builtin.coding.evidence");
  const requirement = plan.requirements.find((candidate) => candidate.domain === "coding");
  assert.ok(requirement);
  assert.throws(
    () => catalogue.registerRoute({
      sourceId: "coding-missing-metadata",
      profileId: profile.profile_id,
      provider: "fixture",
      acquirer: { acquire: async () => ({ claim: "never" }) },
    }),
    /missing required field/,
  );
  assert.throws(
    () => catalogue.registerRoute({
      sourceId: "coding-secret-metadata",
      profileId: profile.profile_id,
      provider: "fixture",
      metadata: { operation: profile.operations[0], api_key: "never-store" },
      acquirer: { acquire: async () => ({ claim: "never" }) },
    }),
    /credential-shaped/,
  );
  assert.throws(
    () => catalogue.registerRoute({
      sourceId: "coding-bad-capability",
      profileId: profile.profile_id,
      provider: "fixture",
      capabilities: ["not-a-coding-capability"],
      metadata: { operation: profile.operations[0] },
      acquirer: { acquire: async () => ({ claim: "never" }) },
    }),
    /exceeds its profile contract/,
  );

  catalogue.registerRoute({
    sourceId: "coding-drift-source",
    profileId: profile.profile_id,
    provider: "fixture",
    sourceDigest: digest({ version: 1 }),
    metadata: { operation: profile.operations[0] },
    acquirer: { acquire: async () => ({ claim: "stable" }) },
  });
  const prepared = catalogue.prepare(plan, requirement.requirement_id, { profileId: profile.profile_id, quorum: 1 });
  assert.throws(
    () => catalogue.registerProfile(new AutonomousDomainEvidenceSourceProfile({
      profileId: profile.profile_id,
      version: profile.version,
      domain: profile.domain,
      purpose: profile.purpose,
      sourceKinds: profile.source_kinds,
      capabilities: profile.capabilities,
      operations: [...profile.operations, "new_operation"],
      requiredMetadata: profile.required_metadata,
      freshness: profile.freshness,
      authMode: profile.auth_mode,
      pagination: profile.pagination,
      normalizerId: profile.normalizer_id,
      normalizerVersion: profile.normalizer_version,
      defaultQuorum: profile.default_quorum,
      defaultMaxConcurrency: profile.default_max_concurrency,
      limitations: profile.limitations,
    }), { replace: true }),
    /routes bind its previous digest/,
  );
  catalogue.registerRoute({
    sourceId: "coding-drift-source",
    profileId: profile.profile_id,
    provider: "fixture",
    sourceDigest: digest({ version: 2 }),
    metadata: { operation: profile.operations[0] },
    acquirer: { acquire: async () => ({ claim: "changed" }) },
  }, { replace: true });
  await assert.rejects(
    () => catalogue.execute(plan, prepared, { approveSourceDispatch: true, normalizer: (value) => value }),
    /changed after preparation/,
  );
});

test("custom domain profiles can be validated without allowing unbound routes", () => {
  const profile = new AutonomousDomainEvidenceSourceProfile({
    profileId: "custom.coding",
    version: "1",
    domain: "coding",
    purpose: "A caller-owned coding evidence route.",
    sourceKinds: ["repository"],
    capabilities: ["review"],
    operations: ["snapshot"],
    freshness: "caller_declared",
    authMode: "none",
    pagination: "none",
    normalizerId: "custom.coding.claim",
    normalizerVersion: "1",
    limitations: ["caller-owned fixture"],
  });
  const catalogue = new AutonomousDomainEvidenceSourceCatalogue([profile]);
  assert.equal(catalogue.coverage().find((row) => row.domain === "coding").state, "missing");
  assert.equal(catalogue.profiles()[0].profile_digest, profile.profile_digest);
});

test("built-in evidence normalizers project every domain without retaining values", async () => {
  const registry = createBuiltinAutonomousEvidenceNormalizerRegistry();
  const profiles = builtinAutonomousDomainEvidenceSourceProfiles();
  const plan = await evidencePlan();
  assert.equal(registry.registrations().length, AUTONOMOUS_DOMAIN_NAMES.length * 2);

  for (const profile of profiles) {
    const registration = registry.resolve(profile.domain, profile.normalizer_id, profile.normalizer_version);
    const requirement = plan.requirements.find((candidate) => candidate.domain === profile.domain);
    assert.ok(requirement);
    const value = { answer: `transient-${profile.domain}`, records: [{ status: "observed" }] };
    const projected = await registry.normalize(profile.domain, profile.normalizer_id, profile.normalizer_version, value, {
      plan_digest: "a".repeat(64),
      requirement,
      request: { requirement_id: requirement.requirement_id, source_id: `${profile.domain}-source`, metadata: { operation: profile.operations[0] } },
      attempt: 1,
      parent_evidence_digests: [],
      execution: "caller_owned_adapter;raw_value_transient",
    });
    assert.equal(projected.schema, "bioprism-typescript-autonomous-evidence-claim-projection/0.1");
    assert.equal(projected.domain, profile.domain);
    assert.equal(projected.operation, profile.operations[0]);
    assert.equal(projected.value_digest, digest(value));
    assert.equal(JSON.stringify(projected).includes(`transient-${profile.domain}`), false);
    assert.equal(typeof registration.spec.spec_digest, "string");
  }
});

test("catalogue executes all domains with the default registry and fences registry drift", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const profiles = builtinAutonomousDomainEvidenceSourceProfiles();
  const plan = await evidencePlan();
  for (const profile of profiles) {
    const requirement = plan.requirements.find((candidate) => candidate.domain === profile.domain);
    assert.ok(requirement);
    for (const suffix of ["a", "b"]) {
      catalogue.registerRoute({
        sourceId: `${profile.domain}-default-${suffix}`,
        profileId: profile.profile_id,
        provider: `default-${suffix}`,
        metadata: { operation: profile.operations[0] },
        acquirer: { acquire: async () => ({ claim: `${profile.domain}:stable` }) },
      });
    }
    const prepared = catalogue.prepare(plan, requirement.requirement_id, { profileId: profile.profile_id, quorum: 2, maxConcurrency: 2 });
    const result = await catalogue.execute(plan, prepared, { approveSourceDispatch: true });
    assert.equal(result.json.status, "consensus");
    assert.equal(result.normalizedValues[`${profile.domain}-default-a`].domain, profile.domain);
    assert.equal(result.normalizedValues[`${profile.domain}-default-a`].claim_posture, "projection_only;truth_and_evaluation_caller_owned");
  }

  const registry = createBuiltinAutonomousEvidenceNormalizerRegistry();
  const driftCatalogue = new AutonomousDomainEvidenceSourceCatalogue(profiles, { normalizerRegistry: registry });
  const coding = profiles.find((profile) => profile.domain === "coding");
  const codingRequirement = plan.requirements.find((candidate) => candidate.domain === "coding");
  assert.ok(coding);
  assert.ok(codingRequirement);
  driftCatalogue.registerRoute({
    sourceId: "drift-route",
    profileId: coding.profile_id,
    provider: "fixture",
    metadata: { operation: coding.operations[0] },
    acquirer: { acquire: async () => ({ claim: "stable" }) },
  });
  const prepared = driftCatalogue.prepare(plan, codingRequirement.requirement_id, { profileId: coding.profile_id, quorum: 1 });
  const extraSpec = new AutonomousEvidenceNormalizerSpec({
    domain: "coding",
    normalizerId: "caller.coding.test",
    version: "1",
    purpose: "A registry drift fixture.",
    limitations: ["test-only"],
  });
  registry.register(new AutonomousEvidenceNormalizerRegistration(extraSpec, (value) => value));
  await assert.rejects(() => driftCatalogue.execute(plan, prepared, { approveSourceDispatch: true }), /normalizer registry changed/);
});

test("normalizer registry rejects tampered specs, callback replacement, and unsafe default output", async () => {
  const registry = createBuiltinAutonomousEvidenceNormalizerRegistry();
  const registration = registry.resolve("coding", "builtin.coding.claim-projection", "1");
  const wire = registration.spec.toJSON();
  assert.deepEqual(AutonomousEvidenceNormalizerSpec.fromJSON(wire).toJSON(), wire);
  const tampered = { ...wire, purpose: "tampered" };
  assert.throws(() => AutonomousEvidenceNormalizerSpec.fromJSON(tampered), /digest|canonical/);
  assert.throws(() => registry.register(new AutonomousEvidenceNormalizerRegistration(registration.spec, (value) => value), { replace: true }), /callback changed/);

  registry.register(new AutonomousEvidenceNormalizerRegistration(
    new AutonomousEvidenceNormalizerSpec({
      domain: "coding",
      normalizerId: "builtin.coding.claim-projection",
      version: "1",
      purpose: "A test replacement that must be rejected at output validation.",
      limitations: ["test-only"],
    }),
    () => ({ authorization: "should-fail" }),
  ), { replace: true });
  const catalogue = new AutonomousDomainEvidenceSourceCatalogue(builtinAutonomousDomainEvidenceSourceProfiles(), { normalizerRegistry: registry });
  const plan = await evidencePlan();
  const profile = catalogue.profile("builtin.coding.evidence");
  const requirement = plan.requirements.find((candidate) => candidate.domain === "coding");
  assert.ok(requirement);
  catalogue.registerRoute({
    sourceId: "unsafe-normalizer-route",
    profileId: profile.profile_id,
    provider: "fixture",
    metadata: { operation: profile.operations[0] },
    acquirer: { acquire: async () => ({ claim: "stable" }) },
  });
  const prepared = catalogue.prepare(plan, requirement.requirement_id, { profileId: profile.profile_id, quorum: 1 });
  const result = await catalogue.execute(plan, prepared, { approveSourceDispatch: true });
  assert.equal(result.json.status, "failed");
  assert.equal(result.json.source_results[0].failure_class, "unknown");
});
