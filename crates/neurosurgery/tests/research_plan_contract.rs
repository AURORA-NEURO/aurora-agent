use bioprism_neurosurgery::{
    CaseRequest, EvidenceState, GliomaMolecularPanel, NeurosurgeryError, NeurosurgicalAgent,
    PublicLiteratureBundle, RealGliomaBundle, RequestUse, ResearchPlanSource, ResearchPlanTaskKind,
    Specialty, NEUROSURGERY_SCHEMA_VERSION,
};
use serde_json::Value;

// These tests exercise the bounded planner rather than a live source worker.
fn request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "research-plan-contract-001".to_string(),
        specialty,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which evidence should a reviewer inspect next?".to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: (specialty == Specialty::Glioma)
            .then_some(GliomaMolecularPanel::default()),
    }
}

fn public_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses")
}

fn real_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

#[test]
fn planner_turns_unmeasured_intake_into_bounded_caller_tasks() {
    let report = NeurosurgicalAgent::default()
        .plan_research(&request(Specialty::ChiariMalformation), None, None, 4, 2)
        .expect("research-only plan compiles");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-research-plan/0.1"
    );
    assert_eq!(report.specialty, Specialty::ChiariMalformation);
    assert!(!report.coverage_complete);
    assert!(report.candidate_task_count > report.tasks.len());
    assert!(report.truncated);
    assert_eq!(report.omitted_task_count, report.candidate_task_count - 4);
    assert!(report
        .tasks
        .iter()
        .all(|task| task.kind == ResearchPlanTaskKind::AcquireCallerObservation));
    assert!(report.tasks.iter().all(|task| task.source_query.is_none()));
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert!(report.human_review_required);
    let first = &report.tasks[0];
    assert_eq!(first.evidence_state, Some(EvidenceState::Unmeasured));
    assert!(first.task_id.starts_with("research-task-"));
}

#[test]
fn public_literature_plan_attaches_only_source_linked_citation_candidates() {
    let report = NeurosurgicalAgent::default()
        .plan_research(
            &request(Specialty::Encephalocele),
            None,
            Some(&public_bundle()),
            16,
            2,
        )
        .expect("public-literature plan compiles");
    assert_eq!(
        report.public_literature_digest.as_deref().map(str::len),
        Some(64)
    );
    assert!(report.source_query_count > 0);
    assert!(report
        .tasks
        .iter()
        .filter_map(|task| task.source_query.as_ref())
        .all(|query| query.source == ResearchPlanSource::PublicLiterature));
    let references = report
        .tasks
        .iter()
        .flat_map(|task| task.source_references.iter())
        .collect::<Vec<_>>();
    assert!(references.iter().all(|reference| {
        reference.source == ResearchPlanSource::PublicLiterature
            && reference
                .uri
                .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
            && !reference.record_id.is_empty()
    }));
    let serialized: Value = serde_json::to_value(report).expect("plan serializes");
    assert!(serialized.get("diagnosis").is_none());
    assert!(serialized.get("treatment_recommendation").is_none());
}

#[test]
fn real_glioma_plan_keeps_population_context_digest_separate_from_case_intake() {
    let report = NeurosurgicalAgent::default()
        .plan_research(
            &request(Specialty::Glioma),
            Some(&real_bundle()),
            None,
            32,
            3,
        )
        .expect("real-data plan compiles");
    assert_eq!(report.real_data_digest.as_deref().map(str::len), Some(64));
    assert!(report.public_literature_digest.is_none());
    assert!(report.tasks.iter().any(|task| {
        task.kind == ResearchPlanTaskKind::ReviewPopulationContext
            && task
                .source_query
                .as_ref()
                .is_some_and(|query| query.source == ResearchPlanSource::RealGliomaPopulation)
    }));
    assert!(report.source_candidate_count > 0);
    assert!(report.tasks.iter().any(|task| {
        task.source_query
            .as_ref()
            .is_some_and(|query| query.text.is_none())
    }));
    assert!(report.tasks.iter().all(|task| task
        .source_references
        .iter()
        .all(|reference| reference.source == ResearchPlanSource::RealGliomaPopulation)));
}

#[test]
fn planner_refuses_clinical_use_dual_bundles_and_unbounded_limits() {
    let agent = NeurosurgicalAgent::default();
    let mut clinical = request(Specialty::Glioma);
    clinical.request_use = RequestUse::IndividualDiagnosis;
    assert!(matches!(
        agent.plan_research(&clinical, None, None, 8, 2),
        Err(NeurosurgeryError::ClinicalUseRefused { .. })
    ));
    assert!(matches!(
        agent.plan_research(
            &request(Specialty::Glioma),
            Some(&real_bundle()),
            Some(&public_bundle()),
            8,
            2
        ),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
    assert!(matches!(
        agent.plan_research(&request(Specialty::Glioma), None, None, 0, 2),
        Err(NeurosurgeryError::TooMany { .. })
    ));
    assert!(matches!(
        agent.plan_research(&request(Specialty::Glioma), None, None, 8, 0),
        Err(NeurosurgeryError::TooMany { .. })
    ));
}

#[test]
fn persisted_plan_is_integrity_checked_and_replayed_against_bounds() {
    let request = request(Specialty::Glioma);
    let real = real_bundle();
    let report = NeurosurgicalAgent::default()
        .plan_research(&request, Some(&real), None, 12, 3)
        .expect("real-data plan compiles");

    assert_eq!(report.max_tasks, 12);
    assert_eq!(report.max_references_per_task, 3);
    assert_eq!(report.plan_digest.len(), 64);
    report
        .validate_integrity()
        .expect("fresh plan satisfies its envelope contract");
    report
        .validate_for_inputs(&request, Some(&real), None, 12, 3)
        .expect("fresh plan replays against exact inputs");

    let mut tampered = report.clone();
    tampered.tasks[0].objective.push_str(" (tampered)");
    assert!(tampered.validate_integrity().is_err());
    assert!(report
        .validate_for_inputs(&request, Some(&real), None, 11, 3)
        .is_err());

    let mut rebound = request.clone();
    rebound.question.push_str(" (rebound)");
    assert!(report
        .validate_for_inputs(&rebound, Some(&real), None, 12, 3)
        .is_err());
}
