import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  LLMRuntime,
  autonomousCapabilityVocabulary,
  routeAutonomousCapability,
  validateAutonomousCapabilityRoute,
} from "../dist/index.js";

const examples = {
  coding: ["debug a failing stack trace", "debugging"],
  browser: ["compare sources and verify sources", "source_comparison"],
  data: ["trace data lineage and provenance", "lineage"],
  science: ["review the literature and references", "literature"],
  biomedical: ["require human review by a clinician", "human_review"],
  neuroscience: ["interpret an EEG neural signal", "signal_interpretation"],
  operations: ["rollback the production service", "rollback"],
  enterprise: ["map the governance policy and owner", "governance"],
  multi_agent: ["resolve the agent conflict and disagreement", "conflict_resolution"],
  multimodal: ["align modalities for cross modal fusion", "cross_modal_alignment"],
  cross_domain: ["synthesize the specialist findings", "synthesis"],
  evaluation: ["replay the deterministic evaluation trace", "replay"],
};

const parityDigests = {
  coding: "0a4b70be55be8d9e92e9f8583b064e0eef0d04c820d6c9dd2b9912578cd15ad3",
  operations: "63bdb39cae43015b485160f290189bc2a757c6627d64513c6c6004d281109633",
};

test("provider-free capability routing selects useful reviewed capabilities across all domains", () => {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const [task, expected] = examples[domain];
    const route = routeAutonomousCapability(task, domain);
    assert.equal(route.domain, domain);
    assert.equal(route.selected_capability, expected, domain);
    assert.equal(route.abstained, false, domain);
    assert.equal(route.reason, "selected", domain);
    assert.equal(route.route_digest.length, 64, domain);
    assert.ok(autonomousCapabilityVocabulary(domain).includes(expected), domain);
    if (parityDigests[domain]) assert.equal(route.route_digest, parityDigests[domain], domain);
    assert.deepEqual(validateAutonomousCapabilityRoute(task, route), route, domain);
  }
});

test("capability routing abstains on missing or ambiguous evidence and accepts explicit reviewed overrides", () => {
  const unknown = routeAutonomousCapability("zzzz qqqq", "coding");
  assert.equal(unknown.abstained, true);
  assert.equal(unknown.reason, "no_matching_capability");
  assert.equal(unknown.selected_capability, null);

  const ambiguous = routeAutonomousCapability("schema quality", "data", { minMargin: 0.5 });
  assert.equal(ambiguous.abstained, true);
  assert.equal(ambiguous.reason, "insufficient_margin");

  const explicit = routeAutonomousCapability("perform the bounded task", "coding", { explicitCapability: "custom_review" });
  assert.equal(explicit.selected_capability, "custom_review");
  assert.equal(explicit.reason, "explicit_capability");
  assert.throws(() => validateAutonomousCapabilityRoute("a different task", explicit), /task digest/);
  const tampered = { ...explicit, confidence: 0.5 };
  assert.throws(() => validateAutonomousCapabilityRoute("perform the bounded task", tampered), /digest/);
});

test("neurosurgical vocabulary selects specialty capabilities without a provider", () => {
  const intake = routeAutonomousCapability("specialty routing", "biomedical");
  assert.equal(intake.selected_capability, "neurosurgical_intake_routing");
  assert.equal(intake.abstained, false);
  const glioma = routeAutonomousCapability("review real glioma data and molecular panel assay coverage", "biomedical");
  assert.equal(glioma.selected_capability, "neurosurgical_research_route");
  assert.equal(glioma.abstained, false);
  const specialty = routeAutonomousCapability("catalogue Chiari and spinal dysraphism neurosurgery", "neuroscience");
  assert.equal(specialty.selected_capability, "neurosurgical_specialty_discovery");
  assert.equal(specialty.abstained, false);
  const nuanced = routeAutonomousCapability("review diffuse midline glioma and pseudoprogression", "biomedical");
  assert.equal(nuanced.selected_capability, "neurosurgical_specialty_discovery");
  const molecularMarker = routeAutonomousCapability("ground H3 K27 and CDKN2A molecular evidence", "biomedical");
  assert.equal(molecularMarker.selected_capability, "neurosurgical_glioma_molecular_map");
  const anatomy = routeAutonomousCapability("review Chiari cine MRI CSF flow and clivo-axial angle", "neuroscience");
  assert.equal(anatomy.selected_capability, "neurosurgical_research_route");
  const cranio = routeAutonomousCapability("compare scaphocephaly and Apert syndrome", "biomedical");
  assert.equal(cranio.selected_capability, "neurosurgical_specialty_discovery");
  const graph = routeAutonomousCapability("build an evidence graph and PMID crosswalk", "biomedical");
  assert.equal(graph.selected_capability, "neurosurgical_evidence_graph");
  const molecularCoverage = routeAutonomousCapability("inventory cBioPortal molecular assay availability by study", "biomedical");
  assert.equal(molecularCoverage.selected_capability, "neurosurgical_molecular_coverage");
  assert.equal(molecularCoverage.abstained, false);
  const coverage = routeAutonomousCapability("audit real data source coverage and temporal linkage gaps", "biomedical");
  assert.equal(coverage.selected_capability, "neurosurgical_real_data_coverage");
  const queue = routeAutonomousCapability("derive the real data metadata review queue", "biomedical");
  assert.equal(queue.selected_capability, "neurosurgical_real_data_review_queue");
  const disposition = routeAutonomousCapability("review disposition for a metadata task", "biomedical");
  assert.equal(disposition.selected_capability, "neurosurgical_real_data_review_disposition");
  const assetDisposition = routeAutonomousCapability("review imaging asset disposition", "biomedical");
  assert.equal(assetDisposition.selected_capability, "neurosurgical_case_asset_review_disposition");
  const dicom = routeAutonomousCapability("import DICOM JSON imaging series metadata", "biomedical");
  assert.equal(dicom.selected_capability, "neurosurgical_case_dicom_import");
  const packet = routeAutonomousCapability("assemble a real data evidence packet for reviewer handoff", "biomedical");
  assert.equal(packet.selected_capability, "neurosurgical_real_data_evidence_packet");
  const draft = routeAutonomousCapability("audit a citation-bound local model draft for grounded claims", "biomedical");
  assert.equal(draft.selected_capability, "neurosurgical_real_data_draft_audit");
});

test("new neurosurgical data tools are routable without a provider", () => {
  const cases = [
    ["import a FHIR bundle resource metadata manifest", "neurosurgical_case_fhir_import"],
    ["run the real data autonomous review wave and dependency closure", "neurosurgical_real_data_autonomous_workflow"],
    ["perform a PubMed literature refresh audit on a candidate literature snapshot", "neurosurgical_public_literature_refresh_audit"],
    ["audit PMID citation links for broken literature links", "neurosurgical_literature_link_audit"],
    ["check citation completeness and publication type completeness", "neurosurgical_public_literature_integrity_audit"],
    ["work the PubMed literature review queue", "neurosurgical_public_literature_review_queue"],
    ["open the citation evidence workbench", "neurosurgical_public_literature_workbench"],
    ["build a multi-lane literature portfolio", "neurosurgical_public_literature_portfolio"],
    ["create a glioma evidence program plan", "neurosurgical_evidence_program"],
    ["map the current clinical trial landscape for glioma", "neurosurgical_trial_landscape"],
  ];
  for (const [task, expected] of cases) {
    const route = routeAutonomousCapability(task, "biomedical");
    assert.equal(route.selected_capability, expected, `${task}: ${JSON.stringify(route)}`);
    assert.equal(route.abstained, false);
  }
});

test("automatic TypeScript blueprints carry the selected capability into planning context", async () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const envelope = await agent.blueprint("debug a failing stack trace", { domain: "coding" });
  assert.ok(envelope.blueprint);
  assert.equal(envelope.capability_route?.selected_capability, "debugging");
  assert.equal(envelope.blueprint.capability_route.selected_capability, "debugging");
  assert.equal(envelope.blueprint.selection_context.capability, "debugging");
  assert.equal(envelope.blueprint.task_intent.capability, "debugging");
});

test("cross-domain children route capability before compiling tools and workflow steps", async () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const task = "coordinate coding and biomedical evidence across disciplines";
  const route = await agent.route(task);
  assert.equal(route.cross_domain, true);
  const envelope = await agent.blueprint(task, {
    routeOverride: route,
    subtasks: [
      { id: "coding-child", domain: "coding", task: "debug a failing stack trace" },
      { id: "biomedical-child", domain: "biomedical", task: "require human review by a clinician" },
    ],
  });
  assert.ok(envelope.cross_domain_blueprint);
  assert.deepEqual(envelope.cross_domain_blueprint.child_ids, ["coding-child", "biomedical-child"]);
  for (const child of envelope.cross_domain_blueprint.child_blueprints) {
    const expected = child.capability_route.selected_capability ?? child.domain_profile.default_capability;
    assert.equal(child.selection_context.capability, expected, child.domain_profile.domain);
    assert.equal(child.task_intent.capability, expected, child.domain_profile.domain);
    assert.equal(child.plan.steps.every((step) => step.arguments.capability === expected), true, child.domain_profile.domain);
  }
  assert.equal(envelope.cross_domain_blueprint.child_blueprints[0].capability_route.selected_capability, "debugging");
  assert.equal(envelope.cross_domain_blueprint.child_blueprints[1].capability_route.selected_capability, "human_review");
});
