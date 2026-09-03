import json

import pytest

from prism_sdk.autonomous_capability_routing import (
    AUTONOMOUS_CAPABILITY_ROUTE_REASONS,
    autonomous_capability_vocabulary,
    route_autonomous_capability,
    validate_autonomous_capability_route,
)
from prism_sdk.autonomy import (
    AUTONOMOUS_DOMAINS,
    AutonomousTaskOrchestrator,
    _memory_selection_context,
)
from prism_sdk.brain import AutonomousBrain
from prism_sdk.llm_runtime import LLMRuntime


EXAMPLES = {
    "coding": ("debug a failing stack trace", "debugging"),
    "browser": ("compare sources and verify sources", "source_comparison"),
    "data": ("trace data lineage and provenance", "lineage"),
    "science": ("review the literature and references", "literature"),
    "biomedical": ("require human review by a clinician", "human_review"),
    "neuroscience": ("interpret an EEG neural signal", "signal_interpretation"),
    "operations": ("rollback the production service", "rollback"),
    "enterprise": ("map the governance policy and owner", "governance"),
    "multi_agent": ("resolve the agent conflict and disagreement", "conflict_resolution"),
    "multimodal": ("align modalities for cross modal fusion", "cross_modal_alignment"),
    "cross_domain": ("synthesize the specialist findings", "synthesis"),
    "evaluation": ("replay the deterministic evaluation trace", "replay"),
}

PARITY_DIGESTS = {
    "coding": "0a4b70be55be8d9e92e9f8583b064e0eef0d04c820d6c9dd2b9912578cd15ad3",
    "operations": "63bdb39cae43015b485160f290189bc2a757c6627d64513c6c6004d281109633",
}


def test_provider_free_capability_routing_selects_all_domains() -> None:
    for domain in AUTONOMOUS_DOMAINS:
        task, expected = EXAMPLES[domain]
        route = route_autonomous_capability(task, domain)
        assert route.domain == domain
        assert route.selected_capability == expected
        assert route.abstained is False
        assert route.reason == "selected"
        assert len(route.route_digest) == 64
        assert expected in autonomous_capability_vocabulary(domain)
        if domain in PARITY_DIGESTS:
            assert route.route_digest == PARITY_DIGESTS[domain]
        assert validate_autonomous_capability_route(task, route) == route


def test_capability_routing_abstains_and_supports_explicit_overrides() -> None:
    unknown = route_autonomous_capability("zzzz qqqq", "coding")
    assert unknown.abstained is True
    assert unknown.reason == "no_matching_capability"
    assert unknown.selected_capability is None

    ambiguous = route_autonomous_capability("schema quality", "data", min_margin=0.5)
    assert ambiguous.abstained is True
    assert ambiguous.reason == "insufficient_margin"

    explicit = route_autonomous_capability("perform the bounded task", "coding", explicit_capability="custom_review")
    assert explicit.selected_capability == "custom_review"
    assert explicit.reason == "explicit_capability"
    with pytest.raises(Exception, match="task digest"):
        validate_autonomous_capability_route("a different task", explicit)
    tampered = explicit.to_dict()
    tampered["confidence"] = 0.5
    with pytest.raises(Exception, match="digest"):
        validate_autonomous_capability_route("perform the bounded task", tampered)
    assert set(AUTONOMOUS_CAPABILITY_ROUTE_REASONS) >= {"selected", "explicit_capability"}


def test_neurosurgical_terms_select_specialty_capabilities_without_a_provider() -> None:
    intake = route_autonomous_capability("specialty routing", "biomedical")
    assert intake.selected_capability == "neurosurgical_intake_routing"
    assert intake.abstained is False
    glioma = route_autonomous_capability("review real glioma data and molecular panel assay coverage", "biomedical")
    assert glioma.selected_capability == "neurosurgical_research_route"
    assert glioma.abstained is False
    specialty = route_autonomous_capability("catalogue Chiari and spinal dysraphism neurosurgery", "neuroscience")
    assert specialty.selected_capability == "neurosurgical_specialty_discovery"
    assert specialty.abstained is False
    nuanced = route_autonomous_capability("review diffuse midline glioma and pseudoprogression", "biomedical")
    assert nuanced.selected_capability == "neurosurgical_specialty_discovery"
    molecular_marker = route_autonomous_capability("ground H3 K27 and CDKN2A molecular evidence", "biomedical")
    assert molecular_marker.selected_capability == "neurosurgical_glioma_molecular_map"
    anatomy = route_autonomous_capability("review Chiari cine MRI CSF flow and clivo-axial angle", "neuroscience")
    assert anatomy.selected_capability == "neurosurgical_research_route"
    cranio = route_autonomous_capability("compare scaphocephaly and Apert syndrome", "biomedical")
    assert cranio.selected_capability == "neurosurgical_specialty_discovery"
    graph = route_autonomous_capability("build an evidence graph and PMID crosswalk", "biomedical")
    assert graph.selected_capability == "neurosurgical_evidence_graph"
    molecular_coverage = route_autonomous_capability("inventory cBioPortal molecular assay availability by study", "biomedical")
    assert molecular_coverage.selected_capability == "neurosurgical_molecular_coverage"
    assert molecular_coverage.abstained is False
    coverage = route_autonomous_capability("audit real data source coverage and temporal linkage gaps", "biomedical")
    assert coverage.selected_capability == "neurosurgical_real_data_coverage"
    queue = route_autonomous_capability("derive the real data metadata review queue", "biomedical")
    assert queue.selected_capability == "neurosurgical_real_data_review_queue"
    disposition = route_autonomous_capability("review disposition for a metadata task", "biomedical")
    assert disposition.selected_capability == "neurosurgical_real_data_review_disposition"
    asset_disposition = route_autonomous_capability("review imaging asset disposition", "biomedical")
    assert asset_disposition.selected_capability == "neurosurgical_case_asset_review_disposition"
    dicom = route_autonomous_capability("import DICOM JSON imaging series metadata", "biomedical")
    assert dicom.selected_capability == "neurosurgical_case_dicom_import"
    packet = route_autonomous_capability("assemble a real data evidence packet for reviewer handoff", "biomedical")
    assert packet.selected_capability == "neurosurgical_real_data_evidence_packet"
    draft = route_autonomous_capability("audit a citation-bound local model draft for grounded claims", "biomedical")
    assert draft.selected_capability == "neurosurgical_real_data_draft_audit"


def test_new_neurosurgical_data_tools_are_routable_without_a_provider() -> None:
    cases = {
        "import a FHIR bundle resource metadata manifest": "neurosurgical_case_fhir_import",
        "run the real data autonomous review wave and dependency closure": "neurosurgical_real_data_autonomous_workflow",
        "perform a PubMed literature refresh audit on a candidate literature snapshot": "neurosurgical_public_literature_refresh_audit",
        "audit PMID citation links for broken literature links": "neurosurgical_literature_link_audit",
        "check citation completeness and publication type completeness": "neurosurgical_public_literature_integrity_audit",
        "work the PubMed literature review queue": "neurosurgical_public_literature_review_queue",
        "open the citation evidence workbench": "neurosurgical_public_literature_workbench",
        "build a multi-lane literature portfolio": "neurosurgical_public_literature_portfolio",
        "create a glioma evidence program plan": "neurosurgical_evidence_program",
        "map the current clinical trial landscape for glioma": "neurosurgical_trial_landscape",
    }
    for task, expected in cases.items():
        route = route_autonomous_capability(task, "biomedical")
        assert route.selected_capability == expected, (task, route.to_dict())
        assert route.abstained is False


def test_automatic_python_blueprints_use_selected_capability() -> None:
    blueprint = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime())).prepare(
        task="debug a failing stack trace",
        domain="coding",
    )
    assert blueprint.capability_route is not None
    assert blueprint.capability_route.selected_capability == "debugging"
    assert blueprint.spec.capability == "debugging"
    assert blueprint.selection_context["capability"] == "debugging"
    assert blueprint.task_intent is not None
    assert blueprint.task_intent.capability == "debugging"


def test_automatic_python_cross_domain_children_keep_capability_identity() -> None:
    orchestrator = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime()))
    automatic = orchestrator.prepare_auto(
        task="coordinate coding and biomedical evidence across disciplines",
        hints=("coding", "biomedical"),
        max_domains=2,
    )
    assert automatic.cross_domain_blueprint is not None
    children = automatic.cross_domain_blueprint.child_blueprints
    assert {child.profile.domain for child in children} == {"coding", "biomedical"}
    for child in children:
        assert child.capability_route is not None
        expected = child.capability_route.selected_capability or child.profile.default_capability
        assert child.spec.capability == expected
        assert child.selection_context["capability"] == expected
        assert child.task_intent is not None
        assert child.task_intent.capability == expected


def test_memory_projection_keeps_route_identity_inside_its_bounded_envelope() -> None:
    blueprint = AutonomousTaskOrchestrator(AutonomousBrain(object(), LLMRuntime())).prepare(
        task="debug a failing stack trace",
        domain="coding",
    )
    projected = _memory_selection_context(blueprint)
    assert len(projected) <= 32
    assert projected["capability"] == "debugging"
    assert projected["capability_route_digest"] == blueprint.capability_route.route_digest
    assert "domain_capabilities" not in projected
