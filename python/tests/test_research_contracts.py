from prism_sdk.research_contracts import EXPERIMENT_DESIGN_FEATURE_ID, PRECLINICAL_BOUNDARY, PROTOCOL_SIMULATION_FEATURE_ID, REPLICATION_FEATURE_ID, QUALITY_CONTROL_FEATURE_ID, RESEARCH_CONTEXT_FEATURE_ID, REPLAY_AUDIT_FEATURE_ID, WORKFLOW_EXECUTION_FEATURE_ID, EVALUATION_OBSERVABILITY_FEATURE_ID, RESEARCH_RELEASE_FEATURE_ID, RESEARCH_RELEASE_BATCH_FEATURE_ID, FEDERATED_EVALUATION_FEATURE_ID, RESOURCE_WORKBENCH_FEATURE_ID, INSTRUMENT_PREFLIGHT_FEATURE_ID, MULTIMODAL_HARMONIZATION_FEATURE_ID, ANALYSIS_QUALIFICATION_FEATURE_ID, PROTOCOL_MATRIX_FEATURE_ID, MULTIMODAL_REPLICATION_FEATURE_ID, QUALITY_DRIFT_FEATURE_ID, DESIGN_FRONTIER_FEATURE_ID, AUTONOMY_BATCH_FEATURE_ID, WORKFLOW_BATCH_FEATURE_ID, EvidenceReceipt, ExperimentDesignPlan, PolicyReceipt, ProtocolSimulationReport, ReplicationReport, QualityControlReceipt, QualityDriftReceipt, DesignFrontierReceipt, BatchAdmissionReceipt, WorkflowBatchReceipt, ResearchReleaseBatchReceipt, FederatedEvaluationReceipt, QualifiedResourceSet, ResearchContextReceipt, ReplayAuditReceipt, ReleaseReview, ResearchContractError, ResearchIngestionBundle, WorkflowExecutionReceipt, EvaluationCardReceipt, ResearchReleaseReceipt, InstrumentPreflightReceipt, HarmonizedResearchObject, QualifiedAnalysisResult, ProtocolMatrixReceipt, MultimodalReplicationReport, research_artifact_digest


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


def test_instrument_preflight_receipt_preserves_no_hardware_boundary():
    receipt = InstrumentPreflightReceipt(
        run_id="run:instrument-1",
        study_id="study:organoid-1",
        decision="ready",
        ordered_actions=("action-1",),
        action_digests={"action-1": "a" * 64},
        remaining_budget={"minutes": 2.0},
        omissions=(),
        reasons=("checks passed; no hardware effect performed",),
        artifact={"content_hash": "b" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == INSTRUMENT_PREFLIGHT_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_harmonized_research_object_preserves_local_multimodal_limits():
    object_ = HarmonizedResearchObject(
        study_id="study:organoid-1",
        reference_schema="aurora-multimodal/1",
        decision="partial",
        modality_order=("image", "rna"),
        alignment={"image": ("a", "z"), "rna": ("a", "z")},
        omitted_modalities=("proteomics",),
        semantic_loss=({"field": "image.qc", "reason": "not supplied"},),
        reasons=("required modality omitted",),
        artifact={"content_hash": "d" * 64},
        raw_data_local=True,
    )
    object_.validate()
    assert object_.feature_id == MULTIMODAL_HARMONIZATION_FEATURE_ID
    assert object_.digest() == object_.digest()


def test_qualified_analysis_result_cannot_hide_unidentified_status():
    result = QualifiedAnalysisResult(
        question_id="question:effect",
        estimand="average treatment effect in organoid model",
        verdict="conditional",
        selected_candidate="candidate-a",
        candidate_order=("candidate-a",),
        uncertainty=("candidate-a: interval is bounded",),
        omissions=("missing independent site",),
        negative_evidence=("candidate-a: null replication pending",),
        reasons=("protected omissions prevent unconditional qualification",),
        artifact={"content_hash": "e" * 64},
        raw_data_local=True,
    )
    result.validate()
    assert result.feature_id == ANALYSIS_QUALIFICATION_FEATURE_ID
    assert result.digest() == result.digest()


def test_protocol_matrix_receipt_partitions_statuses_and_preserves_digest():
    receipt = ProtocolMatrixReceipt(
        protocol_id="protocol:matrix-1",
        total_cells=2,
        passed_cells=1,
        failed_closed_cells=1,
        approval_cells=0,
        cells=(
            {"cell_id": "matrix-cell-0000", "status": "passed", "reasons": ["simulation passed"]},
            {"cell_id": "matrix-cell-0001", "status": "failed_closed", "reasons": ["budget exhausted"]},
        ),
        artifact={"content_hash": "f" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == PROTOCOL_MATRIX_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_multimodal_replication_receipt_preserves_comparability_omissions():
    report = MultimodalReplicationReport(
        payload={
            "schema_version": "aurora-research-contract/1.0",
            "feature_id": MULTIMODAL_REPLICATION_FEATURE_ID,
            "capability_id": "capability:multimodal-replication",
            "claim": "organoid mechanism reproduces across sites",
            "required_modalities": ["image", "rna"],
            "summary": {"disposition": "partially_replicated", "total_observations": 2, "reasons": ["one study omitted rna"]},
            "studies": [
                {"study_id": "study-a", "site": "site-a", "reasons": [], "comparable": True},
                {"study_id": "study-b", "site": "site-b", "reasons": ["required modalities omitted: rna"], "comparable": False},
            ],
            "boundary": PRECLINICAL_BOUNDARY,
        },
        artifact={"content_hash": "a" * 64},
    )
    report.validate()
    assert report.digest() == report.digest()


def test_quality_drift_receipt_keeps_unknown_metric_and_baseline_digest():
    receipt = QualityDriftReceipt(
        dataset_id="dataset:drift",
        modality="image",
        request_digest="a" * 64,
        summary={"disposition": "unknown", "stable": 1, "drifted": 0, "unknown": 1, "reasons": ["metric snr is unmeasured"]},
        metrics=(
            {"metric_id": "focus", "status": "stable", "delta": 0.01, "reasons": []},
            {"metric_id": "snr", "status": "unknown", "delta": None, "reasons": ["metric snr is unmeasured"]},
        ),
        artifact={"content_hash": "b" * 64},
        raw_data_local=True,
    )
    receipt.validate()
    assert receipt.feature_id == QUALITY_DRIFT_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_design_frontier_receipt_retains_blocked_scenario():
    receipt = DesignFrontierReceipt(
        study_id="study:frontier",
        feasible_scenarios=1,
        blocked_scenarios=1,
        scenarios=(
            {"scenario_id": "nominal", "disposition": "feasible", "reasons": ["compiled"]},
            {"scenario_id": "underpowered", "disposition": "blocked", "reasons": ["resource limit"]},
        ),
        artifact={"content_hash": "c" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == DESIGN_FRONTIER_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_autonomy_batch_receipt_retains_denied_action():
    receipt = BatchAdmissionReceipt(
        actor="agent:batch",
        total_actions=3,
        allowed_actions=1,
        approval_actions=1,
        denied_actions=1,
        actions=(
            {"action_id": "a", "decision": "allowed", "reasons": ["grant admits action"]},
            {"action_id": "b", "decision": "approval_required", "reasons": ["signed preflight required"]},
            {"action_id": "c", "decision": "denied", "reasons": ["unknown evidence"]},
        ),
        artifact={"content_hash": "d" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == AUTONOMY_BATCH_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_workflow_batch_receipt_retains_blocked_run():
    receipt = WorkflowBatchReceipt(
        total_workflows=2,
        succeeded_workflows=1,
        dry_run_workflows=0,
        blocked_workflows=1,
        entries=(
            {"workflow_id": "workflow:a", "disposition": "succeeded", "reasons": ["completed"], "ordered_nodes": ["a"]},
            {"workflow_id": "workflow:b", "disposition": "blocked", "reasons": ["budget exceeded"], "ordered_nodes": []},
        ),
        artifact={"content_hash": "e" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == WORKFLOW_BATCH_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_research_release_batch_receipt_retains_blocked_release():
    receipt = ResearchReleaseBatchReceipt(
        total_releases=2,
        published_releases=1,
        blocked_releases=1,
        entries=(
            {"release_id": "release:a", "disposition": "published", "release_digest": "f" * 64, "reasons": ["signed"]},
            {"release_id": "release:b", "disposition": "blocked", "release_digest": None, "reasons": ["policy denied"]},
        ),
        artifact={"content_hash": "a" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == RESEARCH_RELEASE_BATCH_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_federated_evaluation_receipt_preserves_contradiction():
    receipt = FederatedEvaluationReceipt(
        capability_id="capability:mechanism",
        benchmark_world="world:preclinical",
        minimum_sites=2,
        total_sites=3,
        agreeing_sites=2,
        contradictory_sites=1,
        blocked_sites=0,
        disposition="contradicted",
        entries=(
            {"site_id": "site:a", "disposition": "accepted", "card_digest": "a" * 64, "reasons": ["matches consensus"]},
            {"site_id": "site:b", "disposition": "accepted", "card_digest": "a" * 64, "reasons": ["matches consensus"]},
            {"site_id": "site:c", "disposition": "contradictory", "card_digest": "b" * 64, "reasons": ["digest differs"]},
        ),
        artifact={"content_hash": "b" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == FEDERATED_EVALUATION_FEATURE_ID
    assert receipt.digest() == receipt.digest()


def test_resource_workbench_receipt_preserves_protected_omission():
    receipt = QualifiedResourceSet(
        need_id="need:organoid",
        requester="researcher:alice",
        disposition="blocked",
        considered_candidates=1,
        qualified_count=0,
        resources=(),
        omissions=({"resource_id": "resource:protected", "reason": "raw research data is not institution-local"},),
        reasons=("no candidate satisfied the typed resource need; omissions remain explicit",),
        artifact={"content_hash": "c" * 64},
    )
    receipt.validate()
    assert receipt.feature_id == RESOURCE_WORKBENCH_FEATURE_ID
    assert receipt.digest() == receipt.digest()
