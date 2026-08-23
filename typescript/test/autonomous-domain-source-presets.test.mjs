import test from "node:test";
import assert from "node:assert/strict";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceProviderContractRegistry,
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  buildAutonomousEvidencePlan,
  builtinAutonomousDomainHttpSourcePresets,
  builtinAutonomousDomainProfiles,
  createBuiltinAutonomousDomainEvidenceSourceCatalogue,
  registerAutonomousDomainHttpSourceMatrix,
} from "../dist/index.js";

async function evidencePlan() {
  const profiles = await builtinAutonomousDomainProfiles();
  return buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
}

function transportFor(domain, calls) {
  return {
    policy: new AutonomousHttpConnectorPolicy({
      allowedHosts: ["matrix.example"],
      requireHttps: true,
      timeoutMs: 1_000,
      maxRequestBytes: 64_000,
      maxResponseBytes: 64_000,
    }),
    endpointResolver: (_manifest, request) => new AutonomousHttpConnectorRequest({
      method: "GET",
      url: `https://matrix.example/${domain}?operation=${encodeURIComponent(String(request.operation))}`,
      headers: { accept: "application/json" },
    }),
    requestForContext: (context) => ({ operation: context.request.metadata.operation }),
    headerResolver: () => ({ Authorization: "Bearer caller-owned-session" }),
    fetch: async (input, init) => {
      calls.fetch += 1;
      assert.match(input, new RegExp(`^https://matrix\\.example/${domain}\\?operation=`));
      assert.equal(new Headers(init.headers).get("authorization"), "Bearer caller-owned-session");
      return new Response(JSON.stringify({ claim: `${domain}:claim`, transient_value: "not-retained" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  };
}

test("reviewed HTTP source presets register and execute a complete all-domain matrix", async () => {
  const presets = builtinAutonomousDomainHttpSourcePresets();
  assert.deepEqual(presets.map((preset) => preset.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(new Set(presets.map((preset) => preset.preset_id)).size, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(presets.every((preset) => preset.provider_protocol === "http_json"), true);
  assert.equal(presets.every((preset) => preset.secret_material === "never_returned"), true);

  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const adapterRegistry = new AutonomousEvidenceAdapterRegistry();
  const providerContractRegistry = new AutonomousEvidenceProviderContractRegistry(adapterRegistry);
  const calls = { fetch: 0 };
  const before = JSON.stringify(catalogue.toJSON());
  const matrix = registerAutonomousDomainHttpSourceMatrix({
    catalogue,
    adapterRegistry,
    providerContractRegistry,
    entries: presets.map((preset) => ({
      preset,
      sourceId: `matrix-${preset.domain}`,
      ...transportFor(preset.domain, calls),
      metadata: { operation: preset.operations[0], request_family: `fixture-${preset.domain}` },
    })),
  });
  assert.equal(calls.fetch, 0, "matrix registration must never dispatch HTTP");
  assert.equal(matrix.preset_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(matrix.registrations.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(matrix.coverage.every((row) => row.route_count === 1 && row.state === "partial"), true);
  assert.equal(JSON.stringify(catalogue.toJSON()).includes("caller-owned-session"), false);
  assert.equal(JSON.stringify(catalogue.toJSON()).includes("request_family"), false);
  assert.equal(before.includes("matrix-coding"), false);

  const plan = await evidencePlan();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const preset = presets.find((candidate) => candidate.domain === domain);
    const requirement = plan.requirements.find((candidate) => candidate.domain === domain);
    assert.ok(preset);
    assert.ok(requirement);
    const prepared = catalogue.prepare(plan, requirement.requirement_id, {
      profileId: preset.profile_id,
      sourceIds: [`matrix-${domain}`],
      quorum: 1,
    });
    const result = await catalogue.execute(plan, prepared, {
      approveSourceDispatch: true,
      normalizer: (value) => ({ claim: value.claim }),
    });
    assert.equal(result.json.status, "consensus", domain);
    assert.equal(result.normalizedValues[`matrix-${domain}`].claim, `${domain}:claim`);
  }
  assert.equal(calls.fetch, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(adapterRegistry.toJSON().adapters.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(providerContractRegistry.toJSON().contracts.length, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("source preset registration fails closed on stale identity, secret metadata, and incomplete matrices", () => {
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  const presets = builtinAutonomousDomainHttpSourcePresets();
  const coding = presets.find((preset) => preset.domain === "coding");
  assert.ok(coding);
  const stale = { ...coding, profile_digest: "0".repeat(64) };
  stale.preset_digest = "0".repeat(64);
  assert.throws(() => registerAutonomousDomainHttpSourceMatrix({
    catalogue,
    entries: [{ preset: stale, sourceId: "stale", ...transportFor("coding", { fetch: 0 }) }],
  }), /preset digest|stale|cover every/);

  assert.throws(() => registerAutonomousDomainHttpSourceMatrix({
    catalogue,
    requireAllDomains: false,
    entries: [{ preset: coding, sourceId: "secret-metadata", ...transportFor("coding", { fetch: 0 }), metadata: { operation: coding.operations[0], api_key: "must-not-store" } }],
  }), /credential-shaped/);

  assert.throws(() => registerAutonomousDomainHttpSourceMatrix({
    catalogue,
    entries: presets.slice(0, -1).map((preset) => ({ preset, sourceId: `incomplete-${preset.domain}`, ...transportFor(preset.domain, { fetch: 0 }) })),
  }), /cover every autonomous domain/);
});
