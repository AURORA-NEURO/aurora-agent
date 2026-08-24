from prism_sdk.research_contracts import EXPERIMENT_DESIGN_FEATURE_ID, PRECLINICAL_BOUNDARY, PROTOCOL_SIMULATION_FEATURE_ID, REPLICATION_FEATURE_ID, QUALITY_CONTROL_FEATURE_ID, RESEARCH_CONTEXT_FEATURE_ID, REPLAY_AUDIT_FEATURE_ID, WORKFLOW_EXECUTION_FEATURE_ID, EVALUATION_OBSERVABILITY_FEATURE_ID, RESEARCH_RELEASE_FEATURE_ID, EvidenceReceipt, ExperimentDesignPlan, PolicyReceipt, ProtocolSimulationReport, ReplicationReport, QualityControlReceipt, ResearchContextReceipt, ReplayAuditReceipt, ReleaseReview, ResearchContractError, ResearchIngestionBundle, WorkflowExecutionReceipt, EvaluationCardReceipt, ResearchReleaseReceipt, research_artifact_digest


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


def test_quality_control_receipt_preserves_unknown_and_locality_gate():
    receipt = QualityControlReceipt(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": QUALITY_CONTROL_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": True,
            "summary": {"disposition": "unknown", "reasons": ["metric unmeasured"]},
        },
        artifact={"content_hash": "f" * 64},
    )
    receipt.validate()
    assert receipt.digest() == receipt.digest()


def test_research_context_receipt_preserves_closure_and_omission_state():
    receipt = ResearchContextReceipt(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": RESEARCH_CONTEXT_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "protected_closure_satisfied": True,
            "supports_sufficiency_claim": False,
            "unresolved_obligations": 2,
            "section_digest": "a" * 64,
            "certificate_digest": "b" * 64,
        },
        artifact={"content_hash": "c" * 64},
    )
    receipt.validate()
    assert receipt.digest() == receipt.digest()


def test_replay_audit_receipt_preserves_divergence_status():
    receipt = ReplayAuditReceipt(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": REPLAY_AUDIT_FEATURE_ID,
            "boundary": PRECLINICAL_BOUNDARY,
            "status": "diverged",
            "baseline_digest": "a" * 64,
            "candidate_digest": "b" * 64,
            "first_difference": "run.events",
            "reasons": ["first observable replay divergence: run.events"],
        },
        artifact={"content_hash": "c" * 64},
    )
    receipt.validate()
    assert receipt.digest() == receipt.digest()


def test_workflow_execution_receipt_preserves_order_and_dry_run_state():
    receipt = WorkflowExecutionReceipt(
        workflow_id="workflow:demo",
        mode="dry_run",
        status="dry_run",
        ordered_nodes=("a", "b"),
        completed_nodes=(),
        run={"workflow_id": "workflow:demo", "status": "planned"},
        run_digest="a" * 64,
        remaining_budget={"cpu_seconds": 4.0},
        artifact={"content_hash": "b" * 64},
        reasons=("preflight passed",),
    )
    receipt.validate()
    assert receipt.feature_id == WORKFLOW_EXECUTION_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_evaluation_card_receipt_keeps_baseline_omissions_explicit():
    receipt = EvaluationCardReceipt(
        card={
            "schema_version": "aurora-research-contract/1.0",
            "capability_id": "capability:demo",
            "benchmark_world": "synthetic-v1",
            "baselines": ["fixed"],
            "metrics": [{"name": "auditable_discovery_rate", "value": "0.4", "uncertainty": "95%"}],
            "uncertainty": [{"kind": "sampling", "statement": "small sample"}],
            "limitations": ["synthetic only"],
            "release_verdict": "blocked",
            "boundary": PRECLINICAL_BOUNDARY,
        },
        card_digest="a" * 64,
        observations_digest="b" * 64,
        baseline_counts={"fixed": 0},
        omissions=("baseline fixed is under-sampled",),
        reasons=("baseline coverage is incomplete",),
        artifact={"content_hash": "c" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == EVALUATION_OBSERVABILITY_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_research_release_receipt_preserves_localization_and_provenance():
    receipt = ResearchReleaseReceipt(
        release_id="release-1",
        research_object={
            "release_id": "release-1",
            "artifact_ids": ["artifact:one"],
            "evidence_receipt_ids": ["evidence:one"],
            "boundary": PRECLINICAL_BOUNDARY,
            "federation": {
                "envelope": {
                    "raw_data_local": True,
                    "signature": "ed25519:key:signature",
                    "localization_statement": "raw data remains local",
                    "export": {"content_hash": "c" * 64, "provenance": [{"source_id": "artifact:one"}]},
                }
            },
        },
        release_digest="a" * 64,
        omissions=("evidence:one:missing control",),
        reasons=("omission retained",),
    )
    receipt.validate()
    assert receipt.feature_id == RESEARCH_RELEASE_FEATURE_ID
    assert receipt.digest() == receipt.digest()
