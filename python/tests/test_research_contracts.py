from prism_sdk.research_contracts import EXPERIMENT_DESIGN_FEATURE_ID, PRECLINICAL_BOUNDARY, PROTOCOL_SIMULATION_FEATURE_ID, REPLICATION_FEATURE_ID, EvidenceReceipt, ExperimentDesignPlan, PolicyReceipt, ProtocolSimulationReport, ReplicationReport, ReleaseReview, ResearchContractError, ResearchIngestionBundle, research_artifact_digest


def test_empty_evidence_is_explicit_unknown():
    receipt = EvidenceReceipt(
        receipt_id="evidence:q1",
        intent="retrieve",
        sources=(),
        derivation=("feature:AFA-bioir-P02-F01",),
        uncertainty=(("kind", "no admissible evidence"),),
        omissions=(("item", "query:q1"),),
        conclusion_state="unknown",
    )
    receipt.validate()


def test_unresolved_policy_cannot_allow():
    receipt = PolicyReceipt(receipt_id="policy:q1", decision="allow", reasons=("unresolved",))
    try:
        receipt.validate()
    except ResearchContractError:
        return
    raise AssertionError("unresolved policy was accepted")


def test_artifact_digest_is_stable_for_key_order():
    assert research_artifact_digest({"b": 2, "a": 1}) == research_artifact_digest({"a": 1, "b": 2})


def test_release_review_rejects_pass_without_provenance():
    review = ReleaseReview(
        capability_id="capability:demo",
        card_digest="a" * 64,
        verdict="pass",
        reasons=("all gates passed",),
        provenance_complete=False,
    )
    try:
        review.validate()
    except ResearchContractError:
        return
    raise AssertionError("a passing review without provenance was accepted")


def test_release_review_digest_is_stable():
    review = ReleaseReview(
        capability_id="capability:demo",
        card_digest="a" * 64,
        verdict="blocked",
        reasons=("replication floor unmet",),
    )
    assert review.digest() == review.digest()


def test_research_ingestion_bundle_keeps_raw_data_local():
    bundle = ResearchIngestionBundle(
        source_id="study-a",
        adapter="tabular",
        adapter_version="0.1.0",
        source_digest="a" * 64,
        ingestion_digest="b" * 64,
        artifact={"content_hash": "b" * 64},
        conformance={"verified": True},
    )
    bundle.validate()
    assert bundle.digest() == bundle.digest()


def test_experiment_design_plan_preserves_allocation_total():
    plan = ExperimentDesignPlan(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": EXPERIMENT_DESIGN_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "allocations": [{"arm_id": "control", "units": 4}, {"arm_id": "treatment", "units": 4}],
            "total_units": 8,
        },
        artifact={"content_hash": "c" * 64},
    )
    plan.validate()
    assert plan.digest() == plan.digest()


def test_protocol_simulation_report_preserves_fail_closed_statuses():
    report = ProtocolSimulationReport(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": PROTOCOL_SIMULATION_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "results": [{"scenario_id": "partition", "status": "requires_approval"}],
        },
        artifact={"content_hash": "d" * 64},
    )
    report.validate()
    assert report.digest() == report.digest()


def test_replication_report_preserves_null_and_contradiction_dispositions():
    report = ReplicationReport(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": REPLICATION_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "summary": {
                "disposition": "null_result",
                "total_observations": 2,
                "reasons": ["null result retained as evidence"],
            },
        },
        artifact={"content_hash": "e" * 64},
    )
    report.validate()
    assert report.digest() == report.digest()
