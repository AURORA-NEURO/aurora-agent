import test from "node:test";
import assert from "node:assert/strict";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceProviderContractRegistry,
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  buildAutonomousEvidencePlan,
  builtinAutonomousDomainEvidenceSourceProfiles,
  builtinAutonomousDomainProfiles,
  createBuiltinAutonomousDomainEvidenceSourceCatalogue,
  digestJsonSync,
  registerAutonomousDomainHttpEvidenceSource,
} from "../dist/index.js";

async function evidencePlan() {
  const profiles = await builtinAutonomousDomainProfiles();
  return buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
}

function digest(value) {
  return digestJsonSync(value);
}

function httpOptions({ catalogue, profile, sourceId, fetch, calls, adapterRegistry, providerContractRegistry }) {
  return {
    catalogue,
    profileId: profile.profile_id,
    sourceId,
    provider: `http-${profile.domain}`,
    adapterId: `http-adapter-${profile.domain}`,
    adapterVersion: "1",
    adapterRegistry,
    providerContractRegistry,
    providerContract: providerContractRegistry === undefined ? undefined : {
      contractId: `contract-${profile.domain}`,
      version: "1",
      protocol: "http_json",
      operations: [profile.operations[0]],
      authMode: "caller_managed_credential",
      freshness: profile.freshness,
      pagination: profile.pagination,
      requiredMetadata: profile.required_metadata,
      operationMetadataKey: "operation",
    },
    policy: new AutonomousHttpConnectorPolicy({
      allowedHosts: ["source.example"],
      requireHttps: true,
      timeoutMs: 1_000,
      maxRequestBytes: 64_000,
      maxResponseBytes: 64_000,
    }),
    endpointResolver: (_manifest, request) => new AutonomousHttpConnectorRequest({
      method: "GET",
      url: `https://source.example/evidence?operation=${encodeURIComponent(String(request.operation))}`,
      headers: { accept: "application/json" },
    }),
    headerResolver: () => {
      calls.headers += 1;
      return { Authorization: "Bearer caller-owned-session" };
    },
    requestForContext: (context) => ({ operation: context.request.metadata.operation }),
    fetch,
    metadata: { operation: profile.operations[0] },
  };
}

test("policy-gated HTTP sources invoke through the catalogue across all domains", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const adapterRegistry = new AutonomousEvidenceAdapterRegistry();
  const providerContractRegistry = new AutonomousEvidenceProviderContractRegistry(adapterRegistry);
  const profiles = builtinAutonomousDomainEvidenceSourceProfiles();
  const plan = await evidencePlan();
  const calls = { fetch: 0, headers: 0 };

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const profile = profiles.find((candidate) => candidate.domain === domain);
    assert.ok(profile);
    const beforeRegistrationFetch = calls.fetch;
    const registration = registerAutonomousDomainHttpEvidenceSource({
      ...httpOptions({
        catalogue,
        profile,
        sourceId: `${domain}-http-source`,
        adapterRegistry,
        providerContractRegistry,
        calls,
        fetch: async (input, init) => {
          calls.fetch += 1;
          assert.match(input, /^https:\/\/source\.example\/evidence\?/);
          assert.equal(new Headers(init.headers).get("authorization"), "Bearer caller-owned-session");
          return new Response(JSON.stringify({ claim: `${domain}:claim`, response_marker: "transient" }), {
            status: 200,
            headers: { "content-type": "application/json" },
          });
        },
      }),
    });
    assert.equal(registration.adapter_manifest.adapter_id, `http-adapter-${domain}`);
    assert.equal(registration.provider_contract.contract_id, `contract-${domain}`);
    assert.equal(registration.route.contract_digest, registration.provider_contract.contract_digest);
    assert.ok(registration.route.adapter_manifest_digest);
    assert.equal(calls.fetch, beforeRegistrationFetch, "registration must not dispatch HTTP");

    const requirement = plan.requirements.find((candidate) => candidate.domain === domain);
    assert.ok(requirement);
    const prepared = catalogue.prepare(plan, requirement.requirement_id, {
      profileId: profile.profile_id,
      sourceIds: [`${domain}-http-source`],
      quorum: 1,
    });
    const beforeApprovalFetch = calls.fetch;
    await assert.rejects(
      () => catalogue.execute(plan, prepared, { normalizer: (value) => ({ claim: value.claim }) }),
      /explicit approval/,
    );
    assert.equal(calls.fetch, beforeApprovalFetch, "approval refusal must not dispatch HTTP");
    const result = await catalogue.execute(plan, prepared, {
      approveSourceDispatch: true,
      normalizer: (value) => ({ claim: value.claim }),
    });
    assert.equal(result.json.status, "consensus");
    assert.equal(result.normalizedValues[`${domain}-http-source`].claim, `${domain}:claim`);
  }

  assert.equal(calls.fetch, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(calls.headers, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(adapterRegistry.toJSON().adapters.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(providerContractRegistry.toJSON().contracts.length, AUTONOMOUS_DOMAIN_NAMES.length);
  const projection = JSON.stringify(catalogue.toJSON());
  assert.equal(projection.includes("caller-owned-session"), false);
  assert.equal(projection.includes("response_marker"), false);
});

test("HTTP source failures remain typed and bounded at the catalogue boundary", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const profile = catalogue.profile("builtin.browser.evidence");
  const plan = await evidencePlan();
  const calls = { fetch: 0, headers: 0 };
  registerAutonomousDomainHttpEvidenceSource({
    ...httpOptions({
      catalogue,
      profile,
      sourceId: "browser-auth-refused",
      calls,
      fetch: async () => {
        calls.fetch += 1;
        return new Response(JSON.stringify({ error: "denied" }), { status: 401 });
      },
    }),
  });
  const requirement = plan.requirements.find((candidate) => candidate.domain === "browser");
  assert.ok(requirement);
  const prepared = catalogue.prepare(plan, requirement.requirement_id, { profileId: profile.profile_id, quorum: 1 });
  const result = await catalogue.execute(plan, prepared, {
    approveSourceDispatch: true,
    normalizer: (value) => value,
  });
  assert.equal(result.json.status, "failed");
  assert.equal(result.json.failed_count, 1);
  assert.equal(result.json.source_results[0].failure_class, "auth_refused");
  assert.equal(result.json.source_results[0].retryable, false);
  assert.equal(calls.fetch, 1);
  assert.equal(calls.headers, 1);
});

test("HTTP source bridge refuses unsafe endpoint policy before provider dispatch", async () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const profile = catalogue.profile("builtin.data.evidence");
  const plan = await evidencePlan();
  const calls = { fetch: 0, headers: 0 };
  registerAutonomousDomainHttpEvidenceSource({
    ...httpOptions({
      catalogue,
      profile,
      sourceId: "data-unsafe-source",
      calls,
      fetch: async () => {
        calls.fetch += 1;
        return new Response("{}", { status: 200 });
      },
    }),
    endpointResolver: () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://source.example/data" }),
  });
  const requirement = plan.requirements.find((candidate) => candidate.domain === "data");
  assert.ok(requirement);
  const prepared = catalogue.prepare(plan, requirement.requirement_id, { profileId: profile.profile_id, quorum: 1 });
  const result = await catalogue.execute(plan, prepared, { approveSourceDispatch: true, normalizer: (value) => value });
  assert.equal(result.json.status, "failed");
  assert.equal(result.json.source_results[0].failure_class, "unknown");
  assert.equal(calls.fetch, 0);
  assert.equal(calls.headers, 0);
});
