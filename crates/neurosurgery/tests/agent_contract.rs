use bioprism_neurosurgery::{
    required_capabilities, tool_catalogue, AgentStatus, CaseRequest, EvidenceRecord, EvidenceState,
    EvidenceTier, NeurosurgeryError, NeurosurgicalAgent, Observation, ObservationKind,
    ObservationStatus, RequestUse, Specialty, ToolCapability, ToolEffect, ToolRunStatus,
    NEUROSURGERY_SCHEMA_VERSION,
};

fn base_request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "synthetic-case-001".to_string(),
        specialty,
        request_use: RequestUse::SyntheticCaseSimulation,
        question: "Which evidence and competing explanations should a reviewer inspect?"
            .to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

fn observation(kind: ObservationKind, label: &str) -> Observation {
    Observation {
        kind,
        label: label.to_string(),
        value: "caller-supplied summary".to_string(),
        status: ObservationStatus::Observed,
        source_id: Some(format!("source-{}", label.replace(' ', "-"))),
        observed_at: Some("2024-01-01T00:00:00Z".to_string()),
        timepoint: Some("caller-timepoint".to_string()),
    }
}

#[test]
fn the_catalogue_is_closed_and_every_tool_is_read_only() {
    let catalogue = tool_catalogue();
    assert_eq!(catalogue.len(), ToolCapability::ALL.len());
    assert!(catalogue
        .iter()
        .all(|spec| spec.effect == ToolEffect::ReadOnly));
    assert!(catalogue
        .iter()
        .all(|spec| !spec.label.is_empty() && !spec.purpose.is_empty()));
    for specialty in Specialty::ALL {
        let route = required_capabilities(specialty);
        assert_eq!(route.first(), Some(&ToolCapability::SafetyGate));
        assert_eq!(route.last(), Some(&ToolCapability::HumanReviewHold));
        assert!(route.contains(&ToolCapability::EvidenceSynthesis));
    }
}

#[test]
fn specialty_profiles_carry_domain_specific_research_axes() {
    let glioma = Specialty::Glioma.profile();
    assert!(glioma
        .focus_areas
        .iter()
        .any(|area| area.label().contains("histomolecular")));
    assert!(glioma.identity_axes.iter().any(|axis| axis.contains("IDH")));
    assert!(glioma
        .confounders
        .iter()
        .any(|item| item.contains("heterogeneity")));

    let chiari = Specialty::ChiariMalformation.profile();
    assert!(chiari
        .focus_areas
        .iter()
        .any(|area| area.label().contains("CSF-flow")));
    assert!(chiari
        .spatial_axes
        .iter()
        .any(|axis| axis.contains("foramen magnum")));
    assert_ne!(glioma.evidence_questions, chiari.evidence_questions);
}

#[test]
fn specialist_evidence_map_covers_all_six_lanes_with_four_dimensions() {
    let agent = NeurosurgicalAgent::default();
    for specialty in Specialty::ALL {
        let request = base_request(specialty);
        let report = agent
            .specialty_evidence_map(&request)
            .expect("specialty map is provider-free");
        assert_eq!(report.specialty, specialty);
        assert_eq!(report.dimensions.len(), 4);
        assert!(report
            .dimensions
            .iter()
            .all(|dimension| dimension.required_kind_count > 0));
        report
            .validate_for_request(&request)
            .expect("map remains bound to the exact request");
    }
}

#[test]
fn a_keyless_local_run_is_reproducible_and_reports_unmeasured_inputs() {
    let request = base_request(Specialty::Glioma);
    let agent = NeurosurgicalAgent::default();
    let first = agent.run(&request).expect("synthetic request is valid");
    let replay = agent.run(&request).expect("replay is valid");
    assert_eq!(first, replay);
    assert_eq!(first.response_digest.len(), 64);
    first
        .validate_integrity()
        .expect("terminal response is self-consistent");
    first
        .validate_for_request(&request)
        .expect("terminal response remains bound to its request");
    let mut tampered = first.clone();
    tampered.plan[1].purpose.push_str(" (tampered)");
    assert!(tampered.validate_integrity().is_err());
    let mut rebound_request = request.clone();
    rebound_request.question.push_str(" (rebound)");
    assert!(first.validate_for_request(&rebound_request).is_err());
    assert_eq!(first.status, AgentStatus::NeedsEvidence);
    assert_eq!(first.request_digest.len(), 64);
    assert!(first
        .evidence_gaps
        .iter()
        .any(|gap| gap.capability == ToolCapability::MolecularContext
            && gap.state == EvidenceState::Unmeasured));
    let molecular_task = first
        .report
        .research_worklist
        .iter()
        .find(|task| task.capability == ToolCapability::MolecularContext)
        .expect("unmeasured molecular context becomes a work item");
    assert_eq!(
        molecular_task.status,
        bioprism_neurosurgery::ResearchWorkItemStatus::NeedsCallerEvidence
    );
    assert!(molecular_task
        .required_observations
        .contains(&ObservationKind::Histology));
    assert_eq!(
        first.tool_runs.last().map(|run| run.status),
        Some(ToolRunStatus::HeldForHumanReview)
    );
    assert!(first
        .report
        .non_clinical_use_notice
        .contains("does not diagnose"));
    assert_eq!(first.specialty_profile.specialty, Specialty::Glioma);
    let map = first
        .specialty_evidence_map
        .as_ref()
        .expect("every route includes the specialist evidence map");
    assert!(map.validate_integrity().is_ok());
    assert_eq!(
        map.state,
        bioprism_neurosurgery::SpecialtyEvidenceMapState::NotCollected
    );
    assert!(map
        .dimensions
        .iter()
        .any(|dimension| dimension.key == "tumor_identity"));
    let mut tampered_map = map.clone();
    tampered_map.reviewer_questions.push("tampered".to_string());
    assert!(tampered_map.validate_integrity().is_err());
}

#[test]
fn integrated_glioma_inputs_are_ready_only_for_human_review() {
    let mut request = base_request(Specialty::Glioma);
    request.observations = vec![
        observation(ObservationKind::Imaging, "serial imaging"),
        observation(ObservationKind::Histology, "histology"),
        observation(ObservationKind::Molecular, "molecular calls"),
        observation(ObservationKind::Neuroanatomy, "anatomic relationships"),
        observation(ObservationKind::LongitudinalOutcome, "time-aligned outcome"),
    ];
    request.evidence.push(EvidenceRecord {
        id: "evidence-1".to_string(),
        title: "caller-selected guideline record".to_string(),
        citation: "example citation supplied by caller".to_string(),
        tier: EvidenceTier::Guideline,
        population: Some("declared study population".to_string()),
        year: Some(2024),
        supports: vec![ToolCapability::EvidenceSynthesis],
    });
    let response = NeurosurgicalAgent::default()
        .run(&request)
        .expect("complete synthetic input is valid");
    assert_eq!(response.status, AgentStatus::ReadyForHumanReview);
    assert!(response.evidence_gaps.is_empty());
    assert!(response.report.research_worklist.is_empty());
    assert!(response
        .tool_runs
        .iter()
        .all(|run| run.capability == ToolCapability::HumanReviewHold
            || run.status == ToolRunStatus::Completed));
    assert!(response
        .plan
        .iter()
        .all(|step| { step.effect == ToolEffect::ReadOnly && step.requires_human_review }));
    let map = response
        .specialty_evidence_map
        .expect("integrated routes retain the specialist evidence map");
    assert_eq!(map.specialty, Specialty::Glioma);
    assert!(map.observed_observation_count >= 5);
    assert!(map
        .dimensions
        .iter()
        .any(|dimension| dimension.key == "lesion_spatial_context"
            && dimension.state == bioprism_neurosurgery::SpecialtyEvidenceMapState::Complete));
}

#[test]
fn clinical_requests_are_refused_before_any_tool_runs() {
    let mut request = base_request(Specialty::ChiariMalformation);
    request.request_use = RequestUse::TreatmentRecommendation;
    let error = NeurosurgicalAgent::default().run(&request).unwrap_err();
    assert!(matches!(
        error,
        NeurosurgeryError::ClinicalUseRefused {
            use_case: RequestUse::TreatmentRecommendation,
            ..
        }
    ));
}

#[test]
fn direct_identifiers_are_refused_without_echoing_their_values() {
    let mut request = base_request(Specialty::SpinaBifida);
    request.direct_identifier_fields =
        vec!["name".to_string(), "medical_record_number".to_string()];
    let error = NeurosurgicalAgent::default().run(&request).unwrap_err();
    assert!(
        matches!(error, NeurosurgeryError::DirectIdentifiers { ref fields } if fields == &vec!["name".to_string(), "medical_record_number".to_string()])
    );
    assert!(!error.to_string().contains("synthetic-case-001"));
}

#[test]
fn absent_uninterpretable_and_conflicting_observations_remain_distinct() {
    let absent = NeurosurgicalAgent::default()
        .run(&base_request(Specialty::ChiariMalformation))
        .unwrap();
    assert!(absent.evidence_gaps.iter().any(|gap| {
        gap.capability == ToolCapability::ImagingReview && gap.state == EvidenceState::Unmeasured
    }));

    let mut uninterpretable = base_request(Specialty::ChiariMalformation);
    let mut imaging = observation(ObservationKind::Imaging, "junction imaging");
    imaging.status = ObservationStatus::Uninterpretable;
    uninterpretable.observations.push(imaging);
    let uninterpretable_response = NeurosurgicalAgent::default().run(&uninterpretable).unwrap();
    assert!(uninterpretable_response.evidence_gaps.iter().any(|gap| {
        gap.capability == ToolCapability::ImagingReview
            && gap.state == EvidenceState::Uninterpretable
    }));
    assert!(uninterpretable_response
        .report
        .research_worklist
        .iter()
        .any(|task| task.capability == ToolCapability::ImagingReview
            && task.status == bioprism_neurosurgery::ResearchWorkItemStatus::NeedsHumanReview));

    let mut conflicting = base_request(Specialty::ChiariMalformation);
    let mut first = observation(ObservationKind::Imaging, "junction imaging A");
    first.status = ObservationStatus::Conflicting;
    conflicting.observations.push(first);
    let conflicting_response = NeurosurgicalAgent::default().run(&conflicting).unwrap();
    assert!(conflicting_response.evidence_gaps.iter().any(|gap| {
        gap.capability == ToolCapability::ImagingReview && gap.state == EvidenceState::Conflicting
    }));
}

#[test]
fn requested_tool_duplicates_are_rejected_instead_of_deduplicated_silently() {
    let mut request = base_request(Specialty::CranialBase);
    request.requested_tools = vec![ToolCapability::ImagingReview, ToolCapability::ImagingReview];
    let error = NeurosurgicalAgent::default().run(&request).unwrap_err();
    assert!(matches!(
        error,
        NeurosurgeryError::DuplicateTool {
            tool: ToolCapability::ImagingReview
        }
    ));
}

#[test]
fn bounded_autonomous_session_returns_the_report_and_terminal_checkpoint() {
    let request = base_request(Specialty::Glioma);
    let result = NeurosurgicalAgent::default()
        .run_session_to_review(&request, None, 32)
        .expect("bounded route reaches the review hold");
    assert_eq!(result.steps_executed, result.session.route.len());
    assert_eq!(
        result.session.status,
        bioprism_neurosurgery::SessionStatus::AwaitingHumanReview
    );
    assert_eq!(result.response.tool_runs.len(), result.steps_executed);
    assert_eq!(
        result.response.tool_runs.last().map(|run| run.status),
        Some(ToolRunStatus::HeldForHumanReview)
    );
    assert!(matches!(
        NeurosurgicalAgent::default().run_session_to_review(&request, None, 1),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));
}

#[test]
fn resumable_session_rejects_identity_route_and_status_tampering() {
    let agent = NeurosurgicalAgent::default();
    let request = base_request(Specialty::Glioma);
    let session = agent.start_session(&request, None).expect("session starts");

    let mut wrong_specialty = session.clone();
    wrong_specialty.specialty = Specialty::ChiariMalformation;
    assert!(matches!(
        agent.advance_session(&wrong_specialty, &request, None),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));

    let mut wrong_status = session.clone();
    wrong_status.status = bioprism_neurosurgery::SessionStatus::Running;
    assert!(matches!(
        agent.advance_session(&wrong_status, &request, None),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));

    let mut wrong_id = session;
    wrong_id.session_id = "ns-session-tampered".to_string();
    assert!(matches!(
        agent.advance_session(&wrong_id, &request, None),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));
}
