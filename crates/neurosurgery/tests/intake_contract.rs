use bioprism_neurosurgery::{
    CaseRequest, NeurosurgicalAgent, NeurosurgicalIntakePortfolioQuery, NeurosurgicalIntakeQuery,
    Observation, ObservationKind, ObservationStatus, RequestUse, Specialty, ToolCapability,
    ToolEffect, NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION,
    NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION, NEUROSURGERY_INTAKE_SCHEMA_VERSION,
    NEUROSURGERY_SCHEMA_VERSION,
};

fn real_bundle() -> bioprism_neurosurgery::RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

fn literature_bundle() -> bioprism_neurosurgery::PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses")
}

#[test]
fn lexical_intake_selects_glioma_and_returns_the_closed_route() {
    let query = NeurosurgicalIntakeQuery {
        question:
            "Compare IDH, MGMT methylation, TERT and EGFR evidence in diffuse glioma research."
                .to_string(),
        ..NeurosurgicalIntakeQuery::default()
    };
    let plan = NeurosurgicalAgent::default().intake_plan(&query).unwrap();

    assert_eq!(plan.schema_version, NEUROSURGERY_INTAKE_SCHEMA_VERSION);
    assert_eq!(plan.selected_specialty, Some(Specialty::Glioma));
    assert!(!plan.abstained);
    assert_eq!(plan.reason, "selected");
    assert_eq!(plan.route.first(), Some(&ToolCapability::SafetyGate));
    assert_eq!(plan.route.last(), Some(&ToolCapability::HumanReviewHold));
    assert_eq!(
        plan.evidence_sources,
        vec!["real_glioma_snapshot", "pubmed_snapshot"]
    );
    assert_eq!(plan.effect, ToolEffect::ReadOnly);
    assert_eq!(plan.provider, "none");
    assert!(!plan.network);
    assert!(plan.human_review_required);
    assert!(plan
        .plan_digest
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
}

#[test]
fn canonical_single_word_specialty_anchor_selects_a_real_data_route() {
    let plan = NeurosurgicalAgent::default()
        .intake_plan(&NeurosurgicalIntakeQuery {
            question: "glioma".to_string(),
            ..NeurosurgicalIntakeQuery::default()
        })
        .expect("a canonical specialty anchor should be routable");
    assert_eq!(plan.selected_specialty, Some(Specialty::Glioma));
    assert!(!plan.abstained);
    assert_eq!(plan.confidence_bps, 250);
    assert_eq!(plan.reason, "selected");
}

#[test]
fn expanded_specialty_vocabulary_routes_domain_specific_terms() {
    let cases = [
        (
            "review diffuse midline glioma and pseudoprogression",
            Specialty::Glioma,
        ),
        (
            "map petroclival lesion and cavernous sinus cranial nerves",
            Specialty::CranialBase,
        ),
        (
            "compare scaphocephaly and Apert syndrome evidence",
            Specialty::Craniosynostosis,
        ),
        (
            "review basal encephalocele and CSF rhinorrhea",
            Specialty::Encephalocele,
        ),
        (
            "study neurogenic bladder and split cord in spina bifida",
            Specialty::SpinaBifida,
        ),
        (
            "review Chiari cine MRI CSF flow and clivo-axial angle",
            Specialty::ChiariMalformation,
        ),
    ];
    for (question, expected_specialty) in cases {
        let plan = NeurosurgicalAgent::default()
            .intake_plan(&NeurosurgicalIntakeQuery {
                question: question.to_string(),
                ..NeurosurgicalIntakeQuery::default()
            })
            .expect("expanded specialty vocabulary should remain valid");
        assert_eq!(
            plan.selected_specialty,
            Some(expected_specialty),
            "{question}"
        );
        assert!(!plan.abstained, "{question}");
    }
}

#[test]
fn ambiguous_intake_abstains_without_inventing_a_specialty() {
    let query = NeurosurgicalIntakeQuery {
        question: "Review the case and identify the best next research direction.".to_string(),
        ..NeurosurgicalIntakeQuery::default()
    };
    let plan = NeurosurgicalAgent::default().intake_plan(&query).unwrap();

    assert!(plan.abstained);
    assert!(plan.selected_specialty.is_none());
    assert!(plan.route.is_empty());
    assert!(plan.evidence_sources.is_empty());
    assert!(plan.reason == "no_matching_specialty" || plan.reason == "insufficient_confidence");
    assert!(plan
        .next_actions
        .iter()
        .any(|action| action.contains("ambiguity")));
}

#[test]
fn explicit_specialty_is_a_routing_override_not_a_clinical_authorization() {
    let query = NeurosurgicalIntakeQuery {
        question: "Research the relevant evidence.".to_string(),
        specialty: Some(Specialty::ChiariMalformation),
        max_candidates: 1,
        case_request: None,
    };
    let plan = NeurosurgicalAgent::default().intake_plan(&query).unwrap();

    assert_eq!(plan.selected_specialty, Some(Specialty::ChiariMalformation));
    assert_eq!(plan.reason, "explicit_specialty");
    assert!(!plan.abstained);
    assert_eq!(plan.candidates.len(), 1);
    assert_eq!(
        plan.candidates[0].matched_terms,
        vec!["caller_explicit_specialty"]
    );
    assert!(plan
        .limitations
        .iter()
        .any(|limitation| limitation.contains("never authorizes clinical use")));
}

#[test]
fn explicit_glioma_intake_uses_a_canonical_snapshot_selector() {
    let query = NeurosurgicalIntakeQuery {
        question: "Research the relevant molecular evidence.".to_string(),
        specialty: Some(Specialty::Glioma),
        ..NeurosurgicalIntakeQuery::default()
    };
    let result = NeurosurgicalAgent::default()
        .run_intake_mission(&query, Some(&real_bundle()), Some(&literature_bundle()), 32)
        .expect("explicit glioma route should compose against the real snapshot");
    let mission = result
        .mission
        .expect("evidence-backed mission should be present");
    assert_eq!(
        mission
            .real_data_query
            .as_ref()
            .and_then(|query| query.query.text.as_deref()),
        Some("glioblastoma")
    );
}

#[test]
fn intake_rejects_empty_and_control_character_questions() {
    let empty = NeurosurgicalIntakeQuery {
        question: " ".to_string(),
        ..NeurosurgicalIntakeQuery::default()
    };
    assert!(NeurosurgicalAgent::default().intake_plan(&empty).is_err());

    let control = NeurosurgicalIntakeQuery {
        question: "glioma\nresearch".to_string(),
        ..NeurosurgicalIntakeQuery::default()
    };
    assert!(NeurosurgicalAgent::default().intake_plan(&control).is_err());
}

#[test]
fn explicit_all_lane_portfolio_preserves_independent_real_evidence_gates() {
    let query = NeurosurgicalIntakePortfolioQuery {
        intake: NeurosurgicalIntakeQuery {
            question: "Compare evidence gaps across all neurosurgical lanes".to_string(),
            ..NeurosurgicalIntakeQuery::default()
        },
        include_all_specialties: true,
        max_hits_per_lane: 4,
        max_review_items_per_lane: 4,
        max_issues_per_lane: 8,
        max_session_steps: 16,
    };
    let report = NeurosurgicalAgent::default()
        .run_intake_portfolio(&query, Some(&real_bundle()), Some(&literature_bundle()))
        .expect("validated snapshots should support all-lane review");
    assert_eq!(
        report.schema_version,
        NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION
    );
    assert_eq!(
        report.status,
        bioprism_neurosurgery::NeurosurgicalIntakeMissionStatus::ReadyForHumanReview
    );
    assert_eq!(report.selected_specialties.len(), Specialty::ALL.len());
    let portfolio = report.portfolio.expect("portfolio is present");
    assert_eq!(portfolio.specialty_count, Specialty::ALL.len());
    assert!(portfolio
        .lanes
        .iter()
        .all(|lane| lane.specialty != Specialty::Glioma || lane.workbench.record_count > 0));
    assert!(
        report.mission.is_none(),
        "ambiguous all-lane intake must not invent one route"
    );
}

#[test]
fn portfolio_reports_both_required_snapshots_before_execution() {
    let query = NeurosurgicalIntakePortfolioQuery {
        intake: NeurosurgicalIntakeQuery {
            question: "Review all six neurosurgical evidence lanes".to_string(),
            ..NeurosurgicalIntakeQuery::default()
        },
        include_all_specialties: true,
        ..NeurosurgicalIntakePortfolioQuery::default()
    };
    let report = NeurosurgicalAgent::default()
        .run_intake_portfolio(&query, None, None)
        .expect("missing snapshots should be a structured handoff");
    assert_eq!(
        report.status,
        bioprism_neurosurgery::NeurosurgicalIntakeMissionStatus::NeedsEvidence
    );
    assert_eq!(
        report.required_evidence,
        vec!["pubmed_snapshot", "real_glioma_snapshot"]
    );
    assert!(report.portfolio.is_none());
}

#[test]
fn intake_mission_carries_a_validated_deidentified_case_without_echoing_payload() {
    let case_question = "caller case question must remain transient";
    let query = NeurosurgicalIntakeQuery {
        question: "Review glioma IDH and MGMT evidence for this case".to_string(),
        specialty: Some(Specialty::Glioma),
        max_candidates: 4,
        case_request: Some(CaseRequest {
            schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
            case_id: "case-deidentified-001".to_string(),
            specialty: Specialty::Glioma,
            request_use: RequestUse::ResearchSynthesis,
            question: case_question.to_string(),
            direct_identifier_fields: Vec::new(),
            observations: vec![Observation {
                kind: ObservationKind::Molecular,
                label: "IDH1 assay status".to_string(),
                value: "caller-declared result".to_string(),
                status: ObservationStatus::Observed,
                source_id: Some("pathology-record-1".to_string()),
                observed_at: Some("2025-01-15T00:00:00Z".to_string()),
                timepoint: Some("baseline".to_string()),
            }],
            evidence: Vec::new(),
            requested_tools: Vec::new(),
            real_data_query: None,
            glioma_molecular: None,
        }),
    };
    let result = NeurosurgicalAgent::default()
        .run_intake_mission(&query, Some(&real_bundle()), Some(&literature_bundle()), 32)
        .expect("validated case should execute the guarded mission");
    assert_eq!(
        result.schema_version,
        NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION
    );
    assert_eq!(
        result.status,
        bioprism_neurosurgery::NeurosurgicalIntakeMissionStatus::ReadyForHumanReview
    );
    let mission = result.mission.as_ref().expect("nested mission is present");
    assert_eq!(mission.specialty, Specialty::Glioma);
    assert!(mission.run.response.report.observed_finding_count >= 1);
    assert!(result.request_digest.is_some());
    let encoded = serde_json::to_string(&result).expect("intake mission is serialisable");
    assert!(!encoded.contains(case_question));
}
// Keep this integration target relinkable on Windows hosts with transient application-control policies.
