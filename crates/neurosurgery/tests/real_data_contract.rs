use bioprism_neurosurgery::{
    audit_mission, CaseAsset, CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery,
    CaseAssetReviewDecision, CaseAssetReviewDisposition, CaseAssetSourceKind, CaseRequest,
    DicomCaseImport, FhirCaseImport, GliomaEvidenceState, GliomaMarker, GliomaMarkerObservation,
    GliomaMolecularPanel, NeurosurgeryError, NeurosurgicalAgent, NeurosurgicalIntakePortfolioQuery,
    NeurosurgicalIntakeQuery, PublicLiteratureBundle, RealDataDiffChangeKind, RealDataDiffQuery,
    RealDataDraftAuditRequest, RealDataDraftCitation, RealDataDraftClaim, RealDataDraftClaimKind,
    RealDataDraftClaimStatus, RealDataDraftScope, RealDataEvidencePacketQuery,
    RealDataFreshnessQuery, RealDataQuery, RealDataReasoningContextQuery, RealDataRecordKind,
    RealDataReviewDecision, RealDataReviewDisposition, RealDataReviewKind,
    RealDataReviewQueueQuery, RealGliomaBundle, RealSourceKind, RequestUse, Specialty,
    ToolCapability, NEUROSURGERY_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("checked-in public snapshot parses")
}

fn extended_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_extended_snapshot.json"
    ))
    .expect("checked-in extended public snapshot parses")
}

fn research_request() -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: "glioma-population-research-2026-08-30".to_string(),
        specialty: Specialty::Glioma,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which real public sources and evidence gaps should a reviewer inspect?"
            .to_string(),
        direct_identifier_fields: Vec::new(),
        observations: Vec::new(),
        evidence: Vec::new(),
        requested_tools: Vec::new(),
        real_data_query: None,
        glioma_molecular: None,
    }
}

#[test]
fn extended_real_snapshot_adds_tcga_lgg_without_collapsing_provenance() {
    let data = extended_bundle();
    data.validate()
        .expect("the checked-in extended snapshot must pass provenance validation");
    let summary = data
        .summary()
        .expect("the checked-in extended snapshot summary must compute");
    assert!(!data.synthetic_data);
    assert_eq!(summary.record_count, 89);
    assert_eq!(summary.genomic_project_count, 2);
    assert_eq!(summary.genomic_case_count, 1_133);
    assert_eq!(summary.genomic_project_data_type_counts.len(), 50);
    assert!(summary
        .genomic_project_data_type_counts
        .iter()
        .any(|facet| facet.project_id == "TCGA-GBM"
            && facet.data_type == "Annotated Somatic Mutation"
            && facet.file_count > 0));
    let project_query = RealDataQuery {
        record_kind: Some(RealDataRecordKind::GenomicProject),
        source_id: Some("gdc_tcga_gbm".to_string()),
        ..RealDataQuery::default()
    };
    let project_hits = data
        .query(&project_query)
        .expect("genomic project query should preserve aggregate GDC facets");
    assert_eq!(project_hits.returned_matches, 1);
    assert!(project_hits.hits[0]
        .genomic_data_type_counts
        .iter()
        .any(|facet| facet.data_type == "Annotated Somatic Mutation"));
    let facet_query = RealDataQuery {
        genomic_data_type: Some("annotated somatic mutation".to_string()),
        limit: 16,
        ..RealDataQuery::default()
    };
    let facet_hits = data
        .query(&facet_query)
        .expect("GDC data-type facet query should be deterministic");
    assert_eq!(facet_hits.total_matches, 2);
    assert!(facet_hits.hits.iter().all(|hit| hit.record_kind
        == RealDataRecordKind::GenomicProject
        && hit.genomic_data_type_counts.iter().any(|facet| facet
            .data_type
            .eq_ignore_ascii_case("annotated somatic mutation"))));
    assert!(data
        .sources
        .iter()
        .any(|source| source.source_id == "gdc_tcga_lgg"));
    assert!(data
        .genomic_projects
        .iter()
        .any(|project| project.project_id == "TCGA-LGG" && project.case_count == 516));
    assert!(data
        .sources
        .iter()
        .any(|source| source.source_id == "pubmed_glioma_molecular"));
    assert!(data
        .literature
        .iter()
        .all(|article| article.source_id == "pubmed_glioma_molecular"));
}

#[test]
fn natural_language_intake_can_route_real_dicom_and_fhir_imports_together() {
    let agent = NeurosurgicalAgent::default();
    let real_data = extended_bundle();
    let dicom: DicomCaseImport = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM fixture parses");
    let fhir: FhirCaseImport = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/fhir_metadata.json"
    ))
    .expect("FHIR fixture parses");
    let intake = NeurosurgicalIntakeQuery {
        question: "Review glioma imaging and molecular evidence provenance".to_string(),
        specialty: Some(Specialty::Glioma),
        ..NeurosurgicalIntakeQuery::default()
    };

    let result = agent
        .run_intake_mission_with_case_imports(
            &intake,
            Some(&real_data),
            None,
            Some(&dicom),
            Some(&fhir),
            None,
            32,
        )
        .expect("intake should compose real imports into a mission");
    let mission = result.mission.expect("evidence-backed intake should run");
    assert_eq!(
        result.status,
        bioprism_neurosurgery::NeurosurgicalIntakeMissionStatus::ReadyForHumanReview
    );
    assert_eq!(mission.case_dicom_import.as_ref().unwrap().dataset_count, 2);
    assert_eq!(mission.case_fhir_import.as_ref().unwrap().resource_count, 2);
    assert_eq!(mission.case_asset_manifest.as_ref().unwrap().asset_count, 3);
    assert_eq!(
        mission
            .run
            .response
            .real_data
            .as_ref()
            .unwrap()
            .genomic_case_count,
        1_133
    );
    mission
        .validate_integrity()
        .expect("composed intake mission must validate");
}

#[test]
fn intake_mission_rejects_synthetic_assets_before_missing_snapshot_handoff() {
    let agent = NeurosurgicalAgent::default();
    let query = NeurosurgicalIntakeQuery {
        question: "Review glioma imaging evidence".to_string(),
        specialty: Some(Specialty::Glioma),
        ..NeurosurgicalIntakeQuery::default()
    };
    let manifest = CaseAssetManifest {
        schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1".to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: true,
        direct_identifier_fields: Vec::new(),
        assets: Vec::<CaseAsset>::new(),
    };
    let error = agent
        .run_intake_mission_with_case_assets(&query, None, None, Some(&manifest), None, 8)
        .expect_err("synthetic intake assets must fail closed before evidence handoff");
    assert!(error.to_string().contains("synthetic_data=true"));
}

#[test]
fn selected_intake_portfolio_lane_carries_assets_but_all_lane_refuses_ambiguous_attachment() {
    let agent = NeurosurgicalAgent::default();
    let real_data = bundle();
    let public_literature: PublicLiteratureBundle = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses");
    let query = NeurosurgicalIntakePortfolioQuery {
        intake: NeurosurgicalIntakeQuery {
            question: "Review glioma molecular evidence".to_string(),
            specialty: Some(Specialty::Glioma),
            ..NeurosurgicalIntakeQuery::default()
        },
        max_session_steps: 32,
        ..NeurosurgicalIntakePortfolioQuery::default()
    };
    let manifest = CaseAssetManifest {
        schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1".to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "deidentified-pathology-portfolio-001".to_string(),
            kind: CaseAssetKind::PathologyReport,
            status: bioprism_neurosurgery::ObservationStatus::NotCollected,
            source_kind: CaseAssetSourceKind::PathologyLaboratory,
            source_id: None,
            content_sha256: None,
            modality: None,
            body_region: None,
            observed_at: None,
            timepoint: None,
        }],
    };
    let asset_query = CaseAssetManifestQuery {
        requested_kinds: Some(vec![CaseAssetKind::PathologyReport]),
        max_review_items: 8,
    };
    let freshness = RealDataFreshnessQuery {
        as_of: "2027-08-31T00:00:00Z".to_string(),
        max_age_days: 14,
        source_id: None,
    };
    let selected = agent
        .run_intake_portfolio_with_case_assets_and_freshness(
            &query,
            Some(&real_data),
            Some(&public_literature),
            Some(&manifest),
            Some(&asset_query),
            Some(&freshness),
        )
        .expect("selected lane should carry de-identified assets");
    assert_eq!(
        selected
            .mission
            .as_ref()
            .and_then(|mission| mission.case_asset_manifest.as_ref())
            .map(|report| report.asset_count),
        Some(1)
    );
    assert_eq!(
        selected
            .portfolio
            .as_ref()
            .and_then(|report| report.freshness.as_ref())
            .map(|report| report.query.max_age_days),
        Some(14)
    );
    assert_eq!(
        selected
            .mission
            .as_ref()
            .and_then(|mission| mission.real_data_freshness.as_ref())
            .map(|report| report.query.max_age_days),
        Some(14)
    );

    let mut all_lanes = query;
    all_lanes.include_all_specialties = true;
    let error = agent
        .run_intake_portfolio_with_case_assets(
            &all_lanes,
            Some(&real_data),
            Some(&public_literature),
            Some(&manifest),
            None,
        )
        .expect_err("one specialty asset manifest cannot attach to all six lanes");
    assert!(error.to_string().contains("single selected intake lane"));
}

#[test]
fn checked_in_snapshot_is_real_and_hash_bound() {
    let data = bundle();
    let summary = data.summary().expect("snapshot validates");
    assert!(!summary.synthetic_data);
    assert!(summary.provenance_bound);
    assert_eq!(summary.source_count, 5);
    assert_eq!(summary.clinical_trial_count, 5);
    assert_eq!(summary.genomic_project_count, 1);
    assert_eq!(
        summary.genomic_project_case_counts,
        vec![bioprism_neurosurgery::RealGenomicProjectCaseCount {
            project_id: "TCGA-GBM".to_string(),
            case_count: 617,
        }]
    );
    assert_eq!(summary.portal_study_count, 7);
    assert_eq!(summary.portal_molecular_profile_count, 54);
    assert_eq!(summary.relationship_count, 60);
    assert_eq!(
        summary
            .portal_profile_type_counts
            .iter()
            .map(|entry| (entry.alteration_type.as_str(), entry.count))
            .collect::<Vec<_>>(),
        vec![
            ("COPY_NUMBER_ALTERATION", 9),
            ("GENERIC_ASSAY", 8),
            ("METHYLATION", 3),
            ("MRNA_EXPRESSION", 24),
            ("MUTATION_EXTENDED", 6),
            ("PROTEIN_LEVEL", 4),
        ]
    );
    assert_eq!(summary.reference_count, 1);
    assert_eq!(summary.literature_article_count, 20);
    assert_eq!(summary.literature_abstract_count, 20);
    assert_eq!(summary.literature_abstract_truncated_count, 0);
    assert_eq!(summary.portal_literature_linked_count, 6);
    assert_eq!(summary.portal_literature_unlinked_count, 0);
    assert_eq!(summary.literature_without_portal_count, 14);
    assert_eq!(summary.portal_study_without_pmid_count, 1);
    assert_eq!(
        summary
            .trial_status_counts
            .iter()
            .map(|entry| (entry.status.as_str(), entry.count))
            .collect::<Vec<_>>(),
        vec![
            ("COMPLETED", 2),
            ("RECRUITING", 1),
            ("SUSPENDED", 1),
            ("TERMINATED", 1)
        ]
    );
    assert_eq!(summary.latest_trial_update.as_deref(), Some("2025-03-13"));
    assert_eq!(summary.trial_study_type_count, 5);
    assert_eq!(summary.trial_enrollment_count, 5);
    assert_eq!(summary.trial_intervention_count, 5);
    assert_eq!(summary.record_count, 88);
}

#[test]
fn autonomous_workflow_composes_real_packet_into_ordered_review_wave() {
    let data = bundle();
    let report = NeurosurgicalAgent::default()
        .real_data_autonomous_workflow(
            &data,
            &bioprism_neurosurgery::RealDataAutonomousWorkflowQuery::default(),
        )
        .expect("validated real snapshot should produce a review wave");
    assert!(!report.synthetic_data);
    assert!(report.provenance_bound);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.bundle_digest, report.packet.bundle_digest);
    assert_eq!(report.packet_digest, report.packet.packet_digest);
    assert!(report.candidate_action_count > 0);
    assert!(report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::InspectCohortLandscape
            && action.record_kind == Some(RealDataRecordKind::GenomicProject)
    }));
    assert!(report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::ExpandEvidenceProjection
    }));
    assert!(report
        .actions
        .windows(2)
        .all(|window| (window[0].stage, window[0].kind) <= (window[1].stage, window[1].kind)));
    assert!(report
        .actions
        .iter()
        .all(|action| action.depends_on.iter().all(|dependency| report
            .actions
            .iter()
            .any(|candidate| candidate.action_id == *dependency))));
    assert_eq!(
        report.state,
        bioprism_neurosurgery::RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
    );
    assert_eq!(report.workflow_digest.len(), 64);
    report
        .validate_integrity()
        .expect("autonomous wave is self-consistent");
    report
        .validate_for_inputs(&data)
        .expect("autonomous wave replays against the exact snapshot");
    let mut tampered = report.clone();
    tampered.open_queue_item_count = tampered.open_queue_item_count.saturating_add(1);
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn autonomous_workflow_resumes_from_real_review_dispositions() {
    let data = bundle();
    let queue = data
        .review_queue(&RealDataReviewQueueQuery {
            max_items: 256,
            ..RealDataReviewQueueQuery::default()
        })
        .expect("queue should validate");
    let decisions = queue
        .items
        .iter()
        .map(|item| RealDataReviewDecision {
            task_id: item.task_id.clone(),
            disposition: RealDataReviewDisposition::Reviewed,
            reviewer_id: "reviewer-1".to_string(),
        })
        .collect::<Vec<_>>();
    let disposition = queue
        .apply_dispositions(&decisions)
        .expect("queue dispositions should validate");
    let report = NeurosurgicalAgent::default()
        .real_data_autonomous_workflow(
            &data,
            &bioprism_neurosurgery::RealDataAutonomousWorkflowQuery {
                dispositions: Some(disposition),
                packet: bioprism_neurosurgery::RealDataEvidencePacketQuery {
                    review_queue: RealDataReviewQueueQuery {
                        max_items: 256,
                        ..RealDataReviewQueueQuery::default()
                    },
                    ..RealDataEvidencePacketQuery::default()
                },
                ..bioprism_neurosurgery::RealDataAutonomousWorkflowQuery::default()
            },
        )
        .expect("disposition-bound wave should resume");
    assert_eq!(
        report.state,
        bioprism_neurosurgery::RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
    );
    assert_eq!(report.open_queue_item_count, 0);
    assert!(report.actions.iter().any(|action| action.kind
        == bioprism_neurosurgery::RealDataAutonomousActionKind::ExpandEvidenceProjection));
    assert!(!report.actions.iter().any(|action| action.kind
        == bioprism_neurosurgery::RealDataAutonomousActionKind::HumanSynthesisGate));
}

#[test]
fn autonomous_workflow_holds_when_explicit_freshness_policy_marks_sources_stale() {
    let data = bundle();
    let queue = data
        .review_queue(&RealDataReviewQueueQuery {
            max_items: 256,
            ..RealDataReviewQueueQuery::default()
        })
        .expect("queue should validate");
    let decisions = queue
        .items
        .iter()
        .map(|item| RealDataReviewDecision {
            task_id: item.task_id.clone(),
            disposition: RealDataReviewDisposition::Reviewed,
            reviewer_id: "reviewer-1".to_string(),
        })
        .collect::<Vec<_>>();
    let disposition = queue
        .apply_dispositions(&decisions)
        .expect("queue dispositions should validate");
    let report = NeurosurgicalAgent::default()
        .real_data_autonomous_workflow(
            &data,
            &bioprism_neurosurgery::RealDataAutonomousWorkflowQuery {
                packet: bioprism_neurosurgery::RealDataEvidencePacketQuery {
                    review_queue: RealDataReviewQueueQuery {
                        max_items: 256,
                        ..RealDataReviewQueueQuery::default()
                    },
                    freshness: Some(RealDataFreshnessQuery {
                        as_of: "2027-08-31T00:00:00Z".to_string(),
                        max_age_days: 30,
                        source_id: None,
                    }),
                    ..RealDataEvidencePacketQuery::default()
                },
                dispositions: Some(disposition),
                ..bioprism_neurosurgery::RealDataAutonomousWorkflowQuery::default()
            },
        )
        .expect("freshness-bound wave should compose");
    assert_eq!(
        report.state,
        bioprism_neurosurgery::RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
    );
    assert_eq!(report.open_queue_item_count, 0);
    assert!(report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::RefreshSourceSnapshot
            && action
                .title
                .as_deref()
                .is_some_and(|title| title.contains("stale"))
    }));
    assert!(!report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::HumanSynthesisGate
    }));
}

#[test]
fn autonomous_workflow_holds_when_action_projection_is_truncated() {
    let data = bundle();
    let queue = data
        .review_queue(&RealDataReviewQueueQuery {
            max_items: 256,
            ..RealDataReviewQueueQuery::default()
        })
        .expect("queue should validate");
    let decisions = queue
        .items
        .iter()
        .map(|item| RealDataReviewDecision {
            task_id: item.task_id.clone(),
            disposition: RealDataReviewDisposition::Reviewed,
            reviewer_id: "reviewer-1".to_string(),
        })
        .collect::<Vec<_>>();
    let disposition = queue
        .apply_dispositions(&decisions)
        .expect("queue dispositions should validate");
    let report = NeurosurgicalAgent::default()
        .real_data_autonomous_workflow(
            &data,
            &bioprism_neurosurgery::RealDataAutonomousWorkflowQuery {
                packet: RealDataEvidencePacketQuery {
                    query: RealDataQuery {
                        limit: 128,
                        ..RealDataQuery::default()
                    },
                    graph: bioprism_neurosurgery::EvidenceGraphQuery {
                        max_nodes: 512,
                        max_edges: 1024,
                        ..bioprism_neurosurgery::EvidenceGraphQuery::default()
                    },
                    review_queue: RealDataReviewQueueQuery {
                        max_items: 256,
                        ..RealDataReviewQueueQuery::default()
                    },
                    ..RealDataEvidencePacketQuery::default()
                },
                dispositions: Some(disposition),
                max_actions: 1,
            },
        )
        .expect("action-bounded wave should compose");
    assert!(!report.packet.data_query.truncated);
    assert!(!report.packet.graph.truncated);
    assert!(report.omitted_action_count > 0);
    assert_eq!(
        report.state,
        bioprism_neurosurgery::RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
    );
    assert!(!report.actions.iter().any(|action| {
        action.kind == bioprism_neurosurgery::RealDataAutonomousActionKind::HumanSynthesisGate
    }));
    report
        .validate_integrity()
        .expect("action-bounded wave remains self-consistent");
}

#[test]
fn tampering_with_a_source_hash_is_refused() {
    let mut data = bundle();
    data.sources[0].content_sha256 =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn real_run_attaches_public_data_identity_without_a_provider_key() {
    let response = NeurosurgicalAgent::default()
        .run_with_real_glioma_data(&research_request(), &bundle())
        .expect("real public snapshot should route locally");
    let summary = response.real_data.expect("real-data summary is attached");
    assert!(summary.provenance_bound);
    assert!(!summary.synthetic_data);
    assert_eq!(summary.record_count, 88);
    assert!(response
        .report
        .known_inputs
        .iter()
        .any(|input| input.contains("validated real-data bundle")));
    assert!(response.tool_runs.iter().any(|run| {
        run.findings
            .iter()
            .any(|finding| finding.code == "real_data_provenance")
    }));
    let inventory = response
        .tool_runs
        .iter()
        .find(|run| run.capability == ToolCapability::RealDataInventory)
        .expect("real-data route includes inventory");
    assert!(inventory
        .findings
        .iter()
        .any(|finding| finding.code == "real_data_inventory"));
    assert!(inventory
        .findings
        .iter()
        .all(|finding| finding.code != "input_inventory"));
    assert!(inventory.findings.iter().any(|finding| {
        finding.detail.contains("indexed PubMed citation(s)")
            && finding.detail.contains("PMID crosswalk links 6")
            && finding.detail.contains("leaves 0 portal PMID(s) unmatched")
            && finding.detail.contains(
                "status distribution: COMPLETED=2, RECRUITING=1, SUSPENDED=1, TERMINATED=1",
            )
            && finding
                .detail
                .contains("latest registry update: 2025-03-13")
    }));
}

#[test]
fn real_data_reasoning_context_is_bounded_digest_bound_and_source_addressable() {
    let report = NeurosurgicalAgent::default()
        .real_data_reasoning_context(
            &bundle(),
            &RealDataReasoningContextQuery {
                packet: RealDataEvidencePacketQuery {
                    query: RealDataQuery {
                        text: Some("glioblastoma".to_string()),
                        limit: 2,
                        ..RealDataQuery::default()
                    },
                    freshness: Some(RealDataFreshnessQuery {
                        as_of: "2027-08-31T00:00:00Z".to_string(),
                        max_age_days: 14,
                        source_id: None,
                    }),
                    ..RealDataEvidencePacketQuery::default()
                },
                max_chars: 8_000,
                include_abstracts: true,
            },
        )
        .expect("real snapshot should render a local reasoning context");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-reasoning-context/0.1"
    );
    assert_eq!(report.packet_digest.len(), 64);
    assert_eq!(report.context_digest.len(), 64);
    assert!(report.context_char_count <= 8_000);
    assert_eq!(report.included_citation_count, report.citations.len());
    assert!(report.citations.iter().all(|citation| {
        citation.source_uri.starts_with("https://") && !citation.record_id.is_empty()
    }));
    assert!(report.context_text.contains("BUNDLE_DIGEST:"));
    assert!(report.context_text.contains("SAFETY_BOUNDARY:"));
    assert!(report.context_text.contains("FRESHNESS: status="));
    assert!(report.context_text.contains("max_age_days=14"));
    assert!(report.context_text.contains("TRIAL_LANDSCAPE:"));
    assert!(report.context_text.contains("TRIAL_PHASE_COUNTS:"));
    assert!(report.context_text.contains("MOLECULAR_COVERAGE:"));
    assert!(report.context_text.contains("MOLECULAR_ALTERATION_COUNTS:"));
    assert!(report.context_text.contains("IDENTIFIER_RECONCILIATION:"));
    assert!(report.context_text.contains("REVIEW_QUEUE: candidate="));
    assert!(report.context_text.contains("<review_obligation>"));
    assert!(!report.synthetic_data);
    assert!(!report.network);
    report
        .validate_integrity()
        .expect("rendered context should carry a valid integrity envelope");
    report
        .validate_for_inputs(&bundle())
        .expect("rendered context should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.context_text.push_str("tampered");
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.packet.query.text = Some("surprise".to_string());
    assert!(rebound.validate_for_inputs(&bundle()).is_err());
}

#[test]
fn real_data_reasoning_context_exposes_gdc_availability_without_molecular_values() {
    let report = NeurosurgicalAgent::default()
        .real_data_reasoning_context(
            &extended_bundle(),
            &RealDataReasoningContextQuery {
                packet: RealDataEvidencePacketQuery {
                    query: RealDataQuery {
                        genomic_data_type: Some("annotated somatic mutation".to_string()),
                        limit: 16,
                        ..RealDataQuery::default()
                    },
                    ..RealDataEvidencePacketQuery::default()
                },
                ..RealDataReasoningContextQuery::default()
            },
        )
        .expect("extended real snapshot should render GDC availability context");
    assert!(report.context_text.contains("GENOMIC_COVERAGE: projects=2"));
    assert!(report
        .context_text
        .contains("COHORT_LANDSCAPE: matching_projects=2"));
    assert!(report.context_text.contains("COHORT_PROJECT_ROWS:"));
    assert!(report.context_text.contains("COHORT_PROJECT_RECORDS:"));
    assert!(report.citations.iter().any(|citation| citation.record_kind
        == RealDataRecordKind::GenomicProject
        && citation.record_id == "TCGA-GBM"));
    assert!(report
        .context_text
        .contains("TCGA-GBM:Annotated Somatic Mutation=4822"));
    assert!(report
        .context_text
        .contains("genomic_data_type=Some(\"annotated somatic mutation\")"));
    assert!(report
        .context_text
        .contains("SAFETY_BOUNDARY: population metadata and citation text only"));
    report
        .validate_for_inputs(&extended_bundle())
        .expect("GDC context should replay against the exact extended snapshot");
}

#[test]
fn real_data_reasoning_context_reports_omissions_instead_of_silent_truncation() {
    let report = NeurosurgicalAgent::default()
        .real_data_reasoning_context(
            &bundle(),
            &RealDataReasoningContextQuery {
                packet: RealDataEvidencePacketQuery {
                    query: RealDataQuery {
                        limit: 8,
                        ..RealDataQuery::default()
                    },
                    ..RealDataEvidencePacketQuery::default()
                },
                max_chars: 256,
                include_abstracts: true,
            },
        )
        .expect("small context bounds remain valid");
    assert!(report.truncated);
    assert!(report.omitted_citation_count > 0);
    assert!(report.context_char_count <= 256);
}

#[test]
fn synthetic_case_mode_cannot_be_mixed_into_a_real_run() {
    let mut request = research_request();
    request.request_use = RequestUse::SyntheticCaseSimulation;
    let error = NeurosurgicalAgent::default()
        .run_with_real_glioma_data(&request, &bundle())
        .unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn inventory_tool_requires_an_explicit_validated_bundle() {
    let mut request = research_request();
    request.requested_tools = vec![ToolCapability::RealDataInventory];
    let error = NeurosurgicalAgent::default().run(&request).unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn synthetic_markers_are_refused_across_real_request_text() {
    let mut request = research_request();
    request.question = "compare synthetic and public cohorts".to_string();
    let error = NeurosurgicalAgent::default()
        .run_with_real_glioma_data(&request, &bundle())
        .unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn synthetic_markers_are_refused_inside_real_request_panels() {
    let mut request = research_request();
    request.glioma_molecular = Some(GliomaMolecularPanel {
        observations: vec![GliomaMarkerObservation {
            marker: GliomaMarker::Idh1Mutation,
            state: GliomaEvidenceState::Present,
            assay: Some("synthetic-panel".to_string()),
            specimen: Some("tumour-baseline".to_string()),
            source_id: Some("caller-source".to_string()),
            observed_at: Some("2026-08-29T00:00:00Z".to_string()),
        }],
        ..GliomaMolecularPanel::default()
    });
    let error = NeurosurgicalAgent::default()
        .run_with_real_glioma_data(&request, &bundle())
        .unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn record_categories_are_bound_to_source_kind_and_public_ids_are_unique() {
    let mut wrong_kind = bundle();
    wrong_kind.sources[0].kind = RealSourceKind::Guideline;
    assert!(matches!(
        wrong_kind.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut duplicate_trial = bundle();
    duplicate_trial
        .clinical_trials
        .push(duplicate_trial.clinical_trials[0].clone());
    assert!(matches!(
        duplicate_trial.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut unknown_profile_study = bundle();
    unknown_profile_study.portal_molecular_profiles[0].study_id =
        "gbm-study-not-in-bundle".to_string();
    assert!(matches!(
        unknown_profile_study.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn retrieval_timestamps_are_fixed_width_utc_values() {
    let mut data = bundle();
    data.generated_at = "2026-08-30T00:53:19+00:00".to_string();
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn retrieval_timestamps_reject_impossible_calendar_values() {
    let mut data = bundle();
    data.generated_at = "2026-13-01T00:53:19Z".to_string();
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn source_retrieval_cannot_follow_bundle_generation() {
    let mut data = bundle();
    data.sources[0].retrieved_at = "9999-12-31T23:59:59Z".to_string();
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn real_bundle_rejects_synthetic_metadata_even_when_hashes_are_recomputed_later() {
    let mut data = bundle();
    data.sources[0].authority = "Synthetic registry".to_string();
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn clinical_trial_update_dates_are_calendar_values() {
    let mut data = bundle();
    data.clinical_trials[0].last_update = Some("2026-13-99".to_string());
    assert!(matches!(
        data.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn public_record_query_is_bounded_deterministic_and_source_linked() {
    let result = bundle()
        .query(&RealDataQuery {
            text: Some("enzastaurin".to_string()),
            status: Some("completed".to_string()),
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("query is valid");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.returned_matches, 1);
    assert!(!result.truncated);
    assert_eq!(result.hits[0].record_id, "NCT00402116");
    assert!(result.hits[0]
        .source_uri
        .starts_with("https://clinicaltrials.gov/"));
    result
        .validate_integrity()
        .expect("persisted real-data query result is structurally valid");
    result
        .validate_for_inputs(&bundle())
        .expect("persisted real-data query result replays against the exact bundle");

    let mut rebound = result.clone();
    rebound.query.text = Some("different query".to_string());
    assert!(rebound.validate_for_inputs(&bundle()).is_err());
}

#[test]
fn pubmed_metadata_query_is_source_linked_and_explicitly_typed() {
    let result = bundle()
        .query(&RealDataQuery {
            text: Some("somatic genomic landscape".to_string()),
            status: None,
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("literature query is valid");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.returned_matches, 1);
    assert!(!result.truncated);
    assert_eq!(result.portal_literature_linked_count, 6);
    assert_eq!(result.relationship_count, 60);
    assert_eq!(result.portal_literature_unlinked_count, 0);
    assert_eq!(result.literature_without_portal_count, 14);
    assert_eq!(result.portal_study_without_pmid_count, 1);
    assert_eq!(result.hits[0].record_kind.slug(), "literature_article");
    assert_eq!(result.hits[0].record_id, "24120142");
    assert!(result.hits[0]
        .source_uri
        .starts_with("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/"));
    assert!(result.literature_abstract_count >= 20);
    assert_eq!(result.literature_abstract_truncated_count, 0);
    assert!(result.hits[0]
        .abstract_excerpt
        .as_deref()
        .is_some_and(|excerpt| excerpt.contains("landscape")));
    assert!(result.hits[0]
        .mesh_terms
        .iter()
        .any(|term| term == "Glioblastoma"));

    let doi_result = bundle()
        .query(&RealDataQuery {
            text: Some("10.1016/j.cell.2013.09.034".to_string()),
            status: None,
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("DOI query is valid");
    assert_eq!(doi_result.total_matches, 1);
    assert_eq!(doi_result.hits[0].record_id, "24120142");
}

#[test]
fn pubmed_query_supports_publication_type_and_mesh_facets_without_cross_plane_leakage() {
    let publication_result = bundle()
        .query(&RealDataQuery {
            publication_type: Some("systematic review".to_string()),
            record_kind: Some(RealDataRecordKind::LiteratureArticle),
            limit: 16,
            ..RealDataQuery::default()
        })
        .expect("publication-type facet is valid");
    assert!(publication_result.total_matches > 0);
    assert!(publication_result.hits.iter().all(|hit| {
        hit.record_kind == RealDataRecordKind::LiteratureArticle
            && hit
                .publication_types
                .iter()
                .any(|value| value.to_ascii_lowercase().contains("systematic review"))
    }));

    let mesh_result = bundle()
        .query(&RealDataQuery {
            mesh_term: Some("glioblastoma".to_string()),
            record_kind: Some(RealDataRecordKind::LiteratureArticle),
            limit: 16,
            ..RealDataQuery::default()
        })
        .expect("MeSH facet is valid");
    assert!(mesh_result.total_matches > 0);
    assert!(mesh_result.hits.iter().all(|hit| {
        hit.mesh_terms
            .iter()
            .any(|value| value.to_ascii_lowercase().contains("glioblastoma"))
    }));

    let wrong_kind = RealDataQuery {
        publication_type: Some("review".to_string()),
        record_kind: Some(RealDataRecordKind::ClinicalTrial),
        ..RealDataQuery::default()
    };
    assert!(matches!(
        bundle().query(&wrong_kind),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn pubmed_query_supports_inclusive_publication_date_bounds_and_preserves_missing_dates() {
    let bounded = bundle()
        .query(&RealDataQuery {
            publication_date_from: Some("2019-01-01".to_string()),
            publication_date_to: Some("2019-12-31".to_string()),
            limit: 16,
            ..RealDataQuery::default()
        })
        .expect("publication-date bounds are valid");
    assert!(bounded.total_matches > 0);
    assert!(bounded.hits.iter().all(|hit| {
        hit.record_kind == RealDataRecordKind::LiteratureArticle
            && hit
                .publication_date
                .as_deref()
                .is_some_and(|date| ("2019-01-01"..="2019-12-31").contains(&date))
    }));
    assert!(bounded.hits.iter().all(|hit| hit.record_id != "24120142"));

    let reversed = RealDataQuery {
        publication_date_from: Some("2020-01-01".to_string()),
        publication_date_to: Some("2019-12-31".to_string()),
        ..RealDataQuery::default()
    };
    assert!(matches!(
        bundle().query(&reversed),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let wrong_kind = RealDataQuery {
        publication_date_from: Some("2019-01-01".to_string()),
        record_kind: Some(RealDataRecordKind::ClinicalTrial),
        ..RealDataQuery::default()
    };
    assert!(matches!(
        bundle().query(&wrong_kind),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn real_data_query_exposes_explicit_cross_source_relationships_and_facets() {
    let study_result = bundle()
        .query(&RealDataQuery {
            record_kind: Some(bioprism_neurosurgery::RealDataRecordKind::PortalStudy),
            related_record_id: Some("24120142".to_string()),
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("study relationship facet is valid");
    assert_eq!(study_result.total_matches, 1);
    assert_eq!(study_result.hits[0].record_id, "gbm_tcga_pub2013");
    assert!(study_result.hits[0].related_records.iter().any(|related| {
        related.record_id == "24120142"
            && related.record_kind == bioprism_neurosurgery::RealDataRecordKind::LiteratureArticle
            && related.relation == bioprism_neurosurgery::RealDataRelation::PublishedAs
    }));

    let profile_result = bundle()
        .query(&RealDataQuery {
            record_kind: Some(bioprism_neurosurgery::RealDataRecordKind::PortalMolecularProfile),
            related_record_id: Some("gbm_tcga_pub2013".to_string()),
            limit: 128,
            ..RealDataQuery::default()
        })
        .expect("profile relationship facet is valid");
    assert_eq!(profile_result.total_matches, 8);
    assert!(profile_result.hits.iter().all(|hit| {
        hit.related_records.iter().any(|related| {
            related.record_id == "gbm_tcga_pub2013"
                && related.relation == bioprism_neurosurgery::RealDataRelation::ProfileOfStudy
        })
    }));
}

#[test]
fn clinical_trial_query_preserves_optional_registry_metadata_without_inference() {
    let mut data = bundle();
    let trial_source = data.clinical_trials[0].source_id.clone();
    data.clinical_trials[0].study_type = Some("INTERVENTIONAL".to_string());
    data.clinical_trials[0].enrollment_count = Some(42);
    data.clinical_trials[0].intervention_names = vec!["observational registry arm".to_string()];
    let hash = data
        .canonical_source_hashes()
        .expect("modified trial metadata can be rehashed")
        .remove(&trial_source)
        .expect("trial source hash is present");
    data.sources
        .iter_mut()
        .find(|source| source.source_id == trial_source)
        .expect("trial source exists")
        .content_sha256 = hash;

    let result = data
        .query(&RealDataQuery {
            text: Some(data.clinical_trials[0].nct_id.clone()),
            record_kind: Some(RealDataRecordKind::ClinicalTrial),
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("trial query is valid");
    assert_eq!(result.total_matches, 1);
    let hit = &result.hits[0];
    assert_eq!(hit.study_type.as_deref(), Some("INTERVENTIONAL"));
    assert_eq!(hit.enrollment_count, Some(42));
    assert_eq!(hit.intervention_names, vec!["observational registry arm"]);
    assert_eq!(hit.phases, data.clinical_trials[0].phases);
    assert!(hit.publication_date.is_none());
    result
        .validate_for_inputs(&data)
        .expect("query metadata must replay against the exact snapshot");

    let intervention_result = bundle()
        .query(&RealDataQuery {
            text: Some("enzastaurin".to_string()),
            record_kind: Some(RealDataRecordKind::ClinicalTrial),
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("trial intervention text is searchable");
    assert_eq!(intervention_result.total_matches, 1);
    assert_eq!(intervention_result.hits[0].record_id, "NCT00402116");
}

#[test]
fn clinical_trial_query_supports_exact_phase_and_update_date_facets() {
    let phase_result = bundle()
        .query(&RealDataQuery {
            trial_phase: Some("phase2".to_string()),
            record_kind: Some(RealDataRecordKind::ClinicalTrial),
            limit: 16,
            ..RealDataQuery::default()
        })
        .expect("phase facet is valid");
    assert_eq!(phase_result.total_matches, 4);
    assert!(phase_result
        .hits
        .iter()
        .all(|hit| hit.record_kind == RealDataRecordKind::ClinicalTrial));

    let date_result = bundle()
        .query(&RealDataQuery {
            trial_updated_from: Some("2023-01-01".to_string()),
            trial_updated_to: Some("2024-12-31".to_string()),
            record_kind: Some(RealDataRecordKind::ClinicalTrial),
            limit: 16,
            ..RealDataQuery::default()
        })
        .expect("update-date facets are valid");
    assert_eq!(date_result.total_matches, 2);
    assert_eq!(
        date_result
            .hits
            .iter()
            .map(|hit| hit.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["NCT01933815", "NCT04915404"]
    );
    date_result
        .validate_for_inputs(&bundle())
        .expect("date-filtered result must replay");

    let reversed = RealDataQuery {
        trial_updated_from: Some("2025-01-01".to_string()),
        trial_updated_to: Some("2024-01-01".to_string()),
        ..RealDataQuery::default()
    };
    assert!(matches!(
        bundle().query(&reversed),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let wrong_kind = RealDataQuery {
        trial_phase: Some("phase2".to_string()),
        record_kind: Some(RealDataRecordKind::LiteratureArticle),
        ..RealDataQuery::default()
    };
    assert!(matches!(
        bundle().query(&wrong_kind),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn molecular_profile_metadata_query_is_typed_and_bounded() {
    let result = bundle()
        .query(&RealDataQuery {
            text: Some("methylation_hm450".to_string()),
            status: None,
            limit: 4,
            ..RealDataQuery::default()
        })
        .expect("molecular profile query is valid");
    assert!(result.total_matches >= result.returned_matches);
    assert!(result.returned_matches > 0);
    assert_eq!(result.portal_molecular_profile_count, 54);
    assert!(result
        .hits
        .iter()
        .all(|hit| hit.record_kind.slug() == "portal_molecular_profile"));
    assert!(result.hits.iter().all(|hit| {
        hit.molecular_alteration_type.as_deref() == Some("METHYLATION") && hit.datatype.is_some()
    }));
    assert!(result.hits.iter().all(|hit| {
        hit.source_uri
            .starts_with("https://www.cbioportal.org/api/studies?")
    }));
}

#[test]
fn literature_metadata_rejects_duplicate_pmids_and_malformed_dois() {
    let mut duplicate = bundle();
    duplicate.literature.push(duplicate.literature[0].clone());
    assert!(matches!(
        duplicate.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut malformed = bundle();
    malformed.literature[0].doi = Some("doi-not-a-prefix".to_string());
    assert!(matches!(
        malformed.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut oversized_abstract = bundle();
    oversized_abstract.literature[0].abstract_text = Some("x".repeat(12_001));
    assert!(matches!(
        oversized_abstract.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut clipped_without_text = bundle();
    clipped_without_text.literature[0].abstract_text = None;
    clipped_without_text.literature[0].abstract_truncated = true;
    assert!(matches!(
        clipped_without_text.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn real_data_query_can_be_bound_into_the_glioma_route() {
    let mut request = research_request();
    request.requested_tools = vec![ToolCapability::RealDataQuery];
    request.real_data_query = Some(RealDataQuery {
        text: Some("artificial intelligence".to_string()),
        status: None,
        limit: 4,
        ..RealDataQuery::default()
    });
    let response = NeurosurgicalAgent::default()
        .run_with_real_glioma_data(&request, &bundle())
        .expect("query route is valid");
    let query_run = response
        .tool_runs
        .iter()
        .find(|run| run.capability == ToolCapability::RealDataQuery)
        .expect("query tool is present");
    assert!(query_run
        .findings
        .iter()
        .any(|finding| finding.code == "real_data_query"));
}

#[test]
fn research_mission_composes_real_query_and_human_review_hold() {
    let request = research_request();
    let data = bundle();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_freshness(
            &request,
            Some(&data),
            Some(&RealDataQuery {
                text: Some("enzastaurin".to_string()),
                status: None,
                limit: 2,
                ..RealDataQuery::default()
            }),
            Some(&RealDataFreshnessQuery {
                as_of: "2027-08-31T00:00:00Z".to_string(),
                max_age_days: 30,
                source_id: None,
            }),
            32,
        )
        .expect("mission is bounded and provider-free");
    mission
        .validate_integrity()
        .expect("persisted real-data mission should pass its local integrity gate");
    mission
        .validate_for_inputs(&request, Some(&data), None)
        .expect("persisted real-data mission should replay against the exact snapshot");
    let mut tampered_query = mission.clone();
    tampered_query
        .real_data_query
        .as_mut()
        .expect("mission query is present")
        .query
        .text = Some("different query".to_string());
    assert!(tampered_query
        .validate_for_inputs(&request, Some(&data), None)
        .is_err());
    assert_eq!(
        mission.schema,
        "bioprism-neurosurgical-research-mission/0.1"
    );
    assert_eq!(mission.provider, "none");
    assert!(!mission.network);
    assert!(mission.human_review_required);
    let mission_audit = mission
        .mission_audit
        .as_ref()
        .expect("real-data mission includes its integrity fuse");
    assert!(mission_audit.integrity_ok);
    assert_eq!(mission_audit.fail_count, 0);
    let acquisition = mission
        .evidence_acquisition
        .as_ref()
        .expect("real-data missions include the bounded acquisition worker plan");
    assert_eq!(acquisition.provider, "none");
    assert!(!acquisition.network);
    assert!(acquisition.ready_for_local_replay);
    assert!(!acquisition.steps.is_empty());
    let acquisition_session = mission
        .evidence_acquisition_session
        .as_ref()
        .expect("mission exposes the initial acquisition checkpoint");
    assert_eq!(acquisition_session.plan_digest, acquisition.plan_digest);
    assert_eq!(acquisition_session.next_sequence, 1);
    assert!(acquisition_session.events.is_empty());
    assert_eq!(
        mission
            .real_data_freshness
            .as_ref()
            .map(|report| report.status),
        Some(bioprism_neurosurgery::RealDataFreshnessStatus::Stale)
    );
    assert_eq!(mission.real_data_query.unwrap().total_matches, 1);
    let coverage = mission
        .real_data_coverage
        .expect("real-data missions include the deterministic coverage audit");
    assert_eq!(coverage.total_record_count, 88);
    assert!(!coverage.synthetic_data);
    assert_eq!(coverage.provider, "none");
    assert!(!coverage.network);
    let trial_landscape = mission
        .real_data_trial_landscape
        .expect("real-data missions include the bounded trial landscape");
    assert_eq!(trial_landscape.bundle_digest, coverage.bundle_digest);
    assert_eq!(trial_landscape.total_matching_trials, 5);
    assert!(!trial_landscape.synthetic_data);
    assert!(trial_landscape.human_review_required);
    assert_eq!(trial_landscape.provider, "none");
    assert!(!trial_landscape.network);
    let molecular_coverage = mission
        .real_data_molecular_coverage
        .expect("real-data missions include the bounded molecular coverage ledger");
    assert_eq!(molecular_coverage.bundle_digest, coverage.bundle_digest);
    assert_eq!(molecular_coverage.total_matching_profile_count, 54);
    assert!(!molecular_coverage.synthetic_data);
    assert!(molecular_coverage.human_review_required);
    assert_eq!(molecular_coverage.provider, "none");
    assert!(!molecular_coverage.network);
    let cohort_landscape = mission
        .real_data_cohort_landscape
        .expect("real-data missions include the comparative cohort landscape");
    assert_eq!(cohort_landscape.bundle_digest, coverage.bundle_digest);
    assert_eq!(cohort_landscape.total_matching_projects, 1);
    assert!(!cohort_landscape.synthetic_data);
    assert!(cohort_landscape.human_review_required);
    let queue = mission
        .real_data_review_queue
        .expect("real-data missions include metadata review obligations");
    assert_eq!(queue.bundle_digest, coverage.bundle_digest);
    assert_eq!(queue.provider, "none");
    assert!(!queue.network);
    let packet = mission
        .real_data_evidence_packet
        .expect("real-data missions include one bounded evidence packet");
    assert_eq!(packet.bundle_digest, coverage.bundle_digest);
    assert_eq!(packet.provider, "none");
    assert!(!packet.network);
    let workflow = mission
        .real_data_autonomous_workflow
        .expect("real-data missions include the resumable autonomous review wave");
    assert_eq!(workflow.bundle_digest, coverage.bundle_digest);
    assert_eq!(workflow.packet_digest, packet.packet_digest);
    assert!(workflow.candidate_action_count > 0);
    assert!(workflow.human_review_required);
    assert_eq!(workflow.provider, "none");
    assert!(!workflow.network);
    let graph = mission
        .real_data_evidence_graph
        .expect("real-data missions include the explicit evidence graph");
    assert_eq!(graph.total_node_count, 88);
    assert_eq!(graph.specialty, Specialty::Glioma);
    assert!(graph.bundle_relationship_count > 0);
    assert!(graph.human_review_required);
    assert_eq!(graph.provider, "none");
    assert!(!graph.network);
    let context = mission
        .real_data_reasoning_context
        .expect("real-data missions include a bounded local-model context");
    assert_eq!(context.bundle_digest, coverage.bundle_digest);
    assert_eq!(context.packet_digest, packet.packet_digest);
    assert!(!context.synthetic_data);
    assert!(!context.network);
    assert!(context.human_review_required);
    assert!(context
        .context_text
        .contains("# AURORA REAL-GLIOMA REASONING CONTEXT"));
    assert!(context.context_digest.len() == 64);
    let plan = mission
        .research_plan
        .expect("real-data missions include the ordered research plan");
    assert_eq!(
        plan.real_data_digest.as_deref(),
        Some(coverage.bundle_digest.as_str())
    );
    assert!(!plan.tasks.is_empty());
    assert!(plan.tasks.iter().all(|task| !task.objective.is_empty()));
    assert!(plan.human_review_required);
    assert_eq!(plan.provider, "none");
    assert!(!plan.network);
    let brief = mission
        .research_brief
        .expect("real-data missions include the deterministic research brief");
    assert_eq!(
        brief.source,
        bioprism_neurosurgery::ResearchBriefSource::RealGlioma
    );
    assert_eq!(brief.bundle_digest, coverage.bundle_digest);
    assert!(brief.topic_count > 0);
    assert!(brief.human_review_required);
    assert_eq!(brief.provider, "none");
    assert!(!brief.network);
    assert_eq!(
        mission.run.session.status,
        bioprism_neurosurgery::SessionStatus::AwaitingHumanReview
    );
}

#[test]
fn persisted_mission_replay_rejects_changed_request_or_snapshot() {
    let request = research_request();
    let data = bundle();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission(&request, Some(&data), None, 32)
        .expect("mission is bounded and provider-free");

    let mut changed_request = request.clone();
    changed_request.question.push_str(" changed");
    assert!(mission
        .validate_for_inputs(&changed_request, Some(&data), None)
        .is_err());

    let mut changed_data = data.clone();
    changed_data.generated_at = "2027-08-31T00:00:00Z".to_string();
    assert!(mission
        .validate_for_inputs(&request, Some(&changed_data), None)
        .is_err());

    let mut tampered = mission.clone();
    tampered.run.steps_executed = tampered.run.steps_executed.saturating_add(1);
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn dual_bundle_mission_audit_replays_the_single_source_research_plan() {
    let request = research_request();
    let real_data = bundle();
    let public_literature: PublicLiteratureBundle = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses");
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_real_data_and_public_literature(
            &request,
            &real_data,
            &public_literature,
            None,
            None,
            None,
            None,
            32,
        )
        .expect("dual-bundle glioma mission is bounded and provider-free");

    assert!(mission
        .mission_audit
        .as_ref()
        .is_some_and(|audit| audit.integrity_ok));
    mission
        .validate_for_inputs(&request, Some(&real_data), Some(&public_literature))
        .expect("dual-bundle mission should replay against both exact snapshots");
}

#[test]
fn molecular_map_is_bound_into_real_mission_synthesis_and_audit() {
    let mut request = research_request();
    request.glioma_molecular = Some(GliomaMolecularPanel {
        observations: vec![GliomaMarkerObservation {
            marker: GliomaMarker::Idh1Mutation,
            state: GliomaEvidenceState::Present,
            assay: Some("validated-panel".to_string()),
            specimen: Some("tumour tissue".to_string()),
            source_id: Some("caller-source".to_string()),
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
        }],
        ..GliomaMolecularPanel::default()
    });
    let data = bundle();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission(&request, Some(&data), None, 32)
        .expect("typed molecular mission should compose");
    let map = mission
        .evidence_synthesis
        .as_ref()
        .and_then(|synthesis| synthesis.glioma_molecular_map.as_ref())
        .expect("typed glioma panel should attach a molecular map");
    assert!(map.validate_for_inputs(&request, Some(&data), None).is_ok());
    assert!(mission
        .mission_audit
        .as_ref()
        .is_some_and(|audit| audit.integrity_ok && audit.validate_integrity().is_ok()));

    let mut tampered = mission.clone();
    tampered
        .evidence_synthesis
        .as_mut()
        .expect("synthesis is present")
        .glioma_molecular_map
        .as_mut()
        .expect("molecular map is present")
        .map_digest = "0".repeat(64);
    let audit = audit_mission(&tampered, &request, Some(&data), None)
        .expect("tampered molecular map can be audited without dispatch");
    assert!(!audit.integrity_ok);
    assert!(audit.checks.iter().any(|check| {
        check.code == "glioma_molecular_map_integrity"
            && check.status == bioprism_neurosurgery::MissionAuditCheckStatus::Fail
    }));
}

#[test]
fn mission_audit_catches_tampered_snapshot_binding_before_handoff() {
    let request = research_request();
    let data = bundle();
    let mission = NeurosurgicalAgent::default()
        .run_research_mission(&request, Some(&data), None, 32)
        .expect("mission is bounded and provider-free");
    let mut tampered = mission.clone();
    tampered
        .real_data_coverage
        .as_mut()
        .expect("coverage is present")
        .bundle_digest = "0".repeat(64);
    let audit = audit_mission(&tampered, &request, Some(&data), None)
        .expect("tampered envelope can be audited without dispatch");
    assert!(!audit.integrity_ok);
    assert!(audit.fail_count > 0);
    assert!(audit
        .checks
        .iter()
        .any(|check| check.code == "real_data_digest_binding"));
    let mut tampered_landscape = mission.clone();
    tampered_landscape
        .real_data_trial_landscape
        .as_mut()
        .expect("trial landscape is present")
        .total_matching_trials = 0;
    let landscape_audit = audit_mission(&tampered_landscape, &request, Some(&data), None)
        .expect("tampered trial landscape can be audited without dispatch");
    assert!(!landscape_audit.integrity_ok);
    assert!(landscape_audit.checks.iter().any(|check| {
        check.code == "real_data_trial_landscape_integrity"
            && check.status == bioprism_neurosurgery::MissionAuditCheckStatus::Fail
    }));
    let mut tampered_map = mission.clone();
    tampered_map
        .specialty_evidence_map
        .as_mut()
        .expect("specialty map is present")
        .dimensions[0]
        .label = "tampered".to_string();
    let map_audit = audit_mission(&tampered_map, &request, Some(&data), None)
        .expect("tampered specialist map can be audited without dispatch");
    assert!(!map_audit.integrity_ok);
    assert!(map_audit
        .checks
        .iter()
        .any(|check| check.code == "specialty_evidence_map_binding"
            && check.status == bioprism_neurosurgery::MissionAuditCheckStatus::Fail));
}

#[test]
fn research_mission_attaches_digest_bound_multimodal_asset_projection() {
    let request = research_request();
    let data = bundle();
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "caller-local-mri-1".to_string(),
            kind: CaseAssetKind::ImagingSeries,
            status: bioprism_neurosurgery::ObservationStatus::Observed,
            source_kind: CaseAssetSourceKind::DicomArchive,
            source_id: Some("caller-dicom-archive".to_string()),
            content_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: Some("2026-01-01T00:00:00Z".to_string()),
            timepoint: Some("baseline".to_string()),
        }],
    };
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_assets(
            &request,
            Some(&data),
            None,
            None,
            Some(&manifest),
            Some(&CaseAssetManifestQuery {
                requested_kinds: Some(vec![
                    CaseAssetKind::ImagingSeries,
                    CaseAssetKind::MolecularAssay,
                ]),
                max_review_items: 16,
            }),
            32,
        )
        .expect("real asset manifest should attach to mission");
    let report = mission
        .case_asset_manifest
        .as_ref()
        .expect("mission should carry asset projection");
    assert!(mission.evidence_acquisition.is_some());
    assert_eq!(report.asset_count, 1);
    assert_eq!(
        report.missing_requested_kinds,
        vec![CaseAssetKind::MolecularAssay]
    );
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|synthesis| synthesis.case_asset_report_digest.as_deref()),
        Some(report.report_digest.as_str())
    );
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|synthesis| synthesis.case_asset_summary.as_ref())
            .map(|summary| summary.asset_count),
        Some(1)
    );
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|synthesis| synthesis.case_asset_summary.as_ref())
            .map(|summary| summary.missing_requested_kinds.clone()),
        Some(vec![CaseAssetKind::MolecularAssay])
    );
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .map(|synthesis| synthesis.case_asset_review_items.len()),
        Some(report.review_items.len())
    );
    assert!(mission
        .mission_audit
        .as_ref()
        .expect("mission includes an integrity fuse")
        .checks
        .iter()
        .any(|check| check.code == "case_asset_program_binding"
            && check.status == bioprism_neurosurgery::MissionAuditCheckStatus::Pass));
    let encoded = serde_json::to_string(&mission).expect("mission serializes");
    assert!(!encoded.contains("caller-local-mri-1"));
    assert!(!encoded.contains("caller-dicom-archive"));
}

#[test]
fn mission_replays_persisted_case_asset_dispositions_into_synthesis_and_audit() {
    let request = research_request();
    let data = bundle();
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "caller-local-disposition-mri".to_string(),
            kind: CaseAssetKind::ImagingSeries,
            status: bioprism_neurosurgery::ObservationStatus::Observed,
            source_kind: CaseAssetSourceKind::DicomArchive,
            source_id: Some("caller-disposition-archive".to_string()),
            content_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: None,
            timepoint: None,
        }],
    };
    let query = CaseAssetManifestQuery {
        requested_kinds: Some(vec![
            CaseAssetKind::ImagingSeries,
            CaseAssetKind::MolecularAssay,
        ]),
        max_review_items: 16,
    };
    let projection = NeurosurgicalAgent::default()
        .case_asset_manifest(&request, &manifest, &query)
        .expect("asset projection should compose");
    let decision = CaseAssetReviewDecision {
        sequence: projection.review_items[0].sequence,
        disposition: CaseAssetReviewDisposition::Unresolved,
        reviewer_id: "reviewer-disposition".to_string(),
    };
    let disposition = NeurosurgicalAgent::default()
        .case_asset_review_disposition(&projection, &[decision])
        .expect("disposition should bind to projection");
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_assets_and_dispositions(
            &request,
            Some(&data),
            None,
            None,
            Some(&manifest),
            Some(&query),
            Some(&disposition),
            32,
        )
        .expect("mission should replay persisted dispositions");
    assert_eq!(
        mission
            .case_asset_review_disposition
            .as_ref()
            .map(|report| report.disposition_digest.as_str()),
        Some(disposition.disposition_digest.as_str())
    );
    assert_eq!(
        mission
            .evidence_synthesis
            .as_ref()
            .and_then(|report| report.case_asset_review_pending_item_count),
        Some(disposition.pending_item_count)
    );
    assert_eq!(
        mission
            .evidence_program
            .as_ref()
            .and_then(|report| report.case_asset_review_disposition_digest.as_deref()),
        Some(disposition.disposition_digest.as_str())
    );
    assert_eq!(
        mission
            .evidence_acquisition
            .as_ref()
            .and_then(|report| report.case_asset_review_disposition_digest.as_deref()),
        Some(disposition.disposition_digest.as_str())
    );
    assert_eq!(
        mission
            .evidence_acquisition_session
            .as_ref()
            .and_then(|session| session.case_asset_review_disposition_digest.as_deref()),
        Some(disposition.disposition_digest.as_str())
    );
    let audit = mission
        .mission_audit
        .as_ref()
        .expect("mission audit is present");
    assert!(audit.integrity_ok);
    assert!(audit.checks.iter().any(|check| check.code
        == "case_asset_disposition_synthesis_binding"
        && check.status == bioprism_neurosurgery::MissionAuditCheckStatus::Pass));
}

#[test]
fn intake_mission_carries_persisted_case_asset_disposition_into_review_hold() {
    let agent = NeurosurgicalAgent::default();
    let request = research_request();
    let data = bundle();
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "caller-local-intake-disposition-mri".to_string(),
            kind: CaseAssetKind::ImagingSeries,
            status: bioprism_neurosurgery::ObservationStatus::Observed,
            source_kind: CaseAssetSourceKind::DicomArchive,
            source_id: None,
            content_sha256: Some(
                "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: None,
            timepoint: None,
        }],
    };
    let query = CaseAssetManifestQuery {
        requested_kinds: Some(vec![CaseAssetKind::ImagingSeries]),
        max_review_items: 8,
    };
    let projection = agent
        .case_asset_manifest(&request, &manifest, &query)
        .expect("intake asset projection should compose");
    let disposition = agent
        .case_asset_review_disposition(
            &projection,
            &[CaseAssetReviewDecision {
                sequence: projection.review_items[0].sequence,
                disposition: CaseAssetReviewDisposition::Unresolved,
                reviewer_id: "intake-reviewer".to_string(),
            }],
        )
        .expect("intake disposition should bind to projection");
    let intake = NeurosurgicalIntakeQuery {
        question: "Review glioma imaging evidence".to_string(),
        specialty: Some(Specialty::Glioma),
        case_request: Some(request),
        ..NeurosurgicalIntakeQuery::default()
    };
    let result = agent
        .run_intake_mission_with_case_assets_and_dispositions(
            &intake,
            Some(&data),
            None,
            Some(&manifest),
            Some(&query),
            None,
            Some(&disposition),
            32,
        )
        .expect("intake should carry a persisted disposition to review");
    assert_eq!(
        result.status,
        bioprism_neurosurgery::NeurosurgicalIntakeMissionStatus::ReadyForHumanReview
    );
    let mission = result.mission.expect("evidence-backed intake should run");
    assert_eq!(
        mission
            .real_data_query
            .as_ref()
            .and_then(|query| query.query.text.as_deref()),
        Some("glioblastoma")
    );
    assert_eq!(
        mission
            .case_asset_review_disposition
            .as_ref()
            .map(|report| report.disposition_digest.as_str()),
        Some(disposition.disposition_digest.as_str())
    );
    assert!(mission
        .mission_audit
        .as_ref()
        .is_some_and(|audit| audit.integrity_ok));
}

#[test]
fn intake_portfolio_carries_persisted_case_asset_disposition_into_selected_lane() {
    let agent = NeurosurgicalAgent::default();
    let request = research_request();
    let data = bundle();
    let literature: PublicLiteratureBundle = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses");
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: Vec::new(),
    };
    let manifest_query = CaseAssetManifestQuery::default();
    let projection = agent
        .case_asset_manifest(&request, &manifest, &manifest_query)
        .expect("empty real manifest should project");
    let disposition = agent
        .case_asset_review_disposition(&projection, &[])
        .expect("empty review ledger should remain valid");
    let query = NeurosurgicalIntakePortfolioQuery {
        intake: NeurosurgicalIntakeQuery {
            question: "Review glioma evidence for this case".to_string(),
            specialty: Some(Specialty::Glioma),
            case_request: Some(request),
            ..NeurosurgicalIntakeQuery::default()
        },
        max_hits_per_lane: 4,
        max_review_items_per_lane: 4,
        max_issues_per_lane: 8,
        max_session_steps: 16,
        ..NeurosurgicalIntakePortfolioQuery::default()
    };
    let report = agent
        .run_intake_portfolio_with_case_assets_and_freshness_and_dispositions(
            &query,
            Some(&data),
            Some(&literature),
            Some(&manifest),
            Some(&manifest_query),
            None,
            Some(&disposition),
        )
        .expect("selected-lane portfolio should replay the reviewer ledger");
    let mission = report
        .mission
        .expect("selected portfolio should include mission");
    assert_eq!(
        mission
            .case_asset_review_disposition
            .as_ref()
            .map(|report| report.disposition_digest.as_str()),
        Some(disposition.disposition_digest.as_str())
    );
}

#[test]
fn mission_audit_catches_tampered_asset_program_projection_before_handoff() {
    let request = research_request();
    let data = bundle();
    let manifest = CaseAssetManifest {
        schema_version: bioprism_neurosurgery::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
        specialty: Specialty::Glioma,
        synthetic_data: false,
        direct_identifier_fields: Vec::new(),
        assets: vec![CaseAsset {
            asset_id: "caller-local-mri-audit".to_string(),
            kind: CaseAssetKind::ImagingSeries,
            status: bioprism_neurosurgery::ObservationStatus::Observed,
            source_kind: CaseAssetSourceKind::DicomArchive,
            source_id: Some("caller-dicom-audit".to_string()),
            content_sha256: Some(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            ),
            modality: Some("MR".to_string()),
            body_region: Some("brain".to_string()),
            observed_at: None,
            timepoint: None,
        }],
    };
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_case_assets(
            &request,
            Some(&data),
            None,
            None,
            Some(&manifest),
            None,
            32,
        )
        .expect("asset mission should compose");
    let asset_report = mission
        .case_asset_manifest
        .as_ref()
        .expect("asset projection is present");
    let mut tampered = mission.clone();
    tampered
        .evidence_program
        .as_mut()
        .expect("evidence program is present")
        .lanes[0]
        .tracks[0]
        .asset_coverage
        .as_mut()
        .expect("track asset coverage is present")[0]
        .observed_count = 999;
    let audit = audit_mission(&tampered, &request, Some(&data), None)
        .expect("tampered asset projection can be audited without dispatch");
    assert!(!audit.integrity_ok);
    assert!(audit
        .checks
        .iter()
        .any(|check| check.code == "case_asset_program_binding"));
    assert_eq!(asset_report.asset_count, 1);
}

#[test]
fn real_data_diff_is_digest_bound_and_exposes_only_structural_changes() {
    let before = bundle();
    let mut after = before.clone();
    after.generated_at = "2026-08-31T05:16:19Z".to_string();
    after.clinical_trials[0]
        .title
        .push_str(" (metadata refresh)");
    let source_id = after.clinical_trials[0].source_id.clone();
    let hash = after
        .canonical_source_hashes()
        .expect("modified snapshot can be rehashed")
        .remove(&source_id)
        .expect("trial source has a canonical hash");
    after
        .sources
        .iter_mut()
        .find(|source| source.source_id == source_id)
        .expect("trial source exists")
        .content_sha256 = hash;

    let report = NeurosurgicalAgent::default()
        .real_data_diff(&before, &after, &RealDataDiffQuery::default())
        .expect("validated snapshots can be compared");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-diff/0.1"
    );
    assert_eq!(report.before_record_count, 88);
    assert_eq!(report.after_record_count, 88);
    assert!(report.record_counts.changed >= 1);
    assert!(report.source_counts.changed >= 1);
    assert!(report.record_changes.iter().any(|change| {
        change.record_id == "NCT05941234"
            && change.change == RealDataDiffChangeKind::Changed
            && change.changed_fields.contains(&"title".to_string())
    }));
    assert!(report.diff_digest.len() == 64);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report
        .validate_integrity()
        .expect("diff should carry a valid envelope");
    report
        .validate_for_inputs(&before, &after)
        .expect("diff should replay against exact before/after snapshots");
    let mut tampered = report.clone();
    tampered.total_change_count += 1;
    assert!(tampered.validate_integrity().is_err());

    let filtered = NeurosurgicalAgent::default()
        .real_data_diff(
            &before,
            &after,
            &RealDataDiffQuery {
                record_kind: Some(bioprism_neurosurgery::RealDataRecordKind::ClinicalTrial),
                source_id: Some(source_id),
                max_changes: 1,
            },
        )
        .expect("diff facets are bounded");
    assert_eq!(filtered.record_counts.changed, 1);
    assert!(filtered.record_changes.len() <= 1);
}

#[test]
fn real_data_review_queue_derives_bounded_metadata_obligations() {
    let data = bundle();
    let queue = NeurosurgicalAgent::default()
        .real_data_review_queue(&data, &RealDataReviewQueueQuery::default())
        .expect("validated snapshot yields a review queue");
    assert_eq!(
        queue.schema_version,
        "bioprism-neurosurgery-real-data-review-queue/0.1"
    );
    assert_eq!(queue.record_count, 88);
    assert_eq!(queue.source_count, 5);
    assert_eq!(queue.candidate_item_count, 15);
    assert_eq!(queue.returned_item_count, 15);
    assert_eq!(queue.omitted_item_count, 0);
    assert!(!queue.truncated);
    assert!(queue.items.iter().any(|item| {
        item.kind == RealDataReviewKind::MissingPortalPublicationLink
            && item.record_id == "gbm_tcga_gdc"
    }));
    assert_eq!(
        queue
            .items
            .iter()
            .filter(|item| item.kind == RealDataReviewKind::UnlinkedLiteratureCitation)
            .count(),
        14
    );
    assert!(queue
        .items
        .windows(2)
        .all(|pair| pair[0].task_id <= pair[1].task_id));
    assert!(queue.provenance_bound);
    assert!(!queue.synthetic_data);
    assert!(queue.human_review_required);
    assert_eq!(queue.provider, "none");
    assert!(!queue.network);
    queue
        .validate_integrity()
        .expect("review queue should pass its standalone integrity gate");

    let filtered = NeurosurgicalAgent::default()
        .real_data_review_queue(
            &data,
            &RealDataReviewQueueQuery {
                record_kind: Some(RealDataRecordKind::PortalStudy),
                source_id: None,
                max_items: 1,
            },
        )
        .expect("queue facets are bounded");
    assert_eq!(filtered.candidate_item_count, 1);
    assert_eq!(filtered.returned_item_count, 1);
    assert_eq!(filtered.items[0].record_id, "gbm_tcga_gdc");
    assert_eq!(
        filtered.items[0].source_uri,
        "https://www.cbioportal.org/api/studies?keyword=gbm"
    );
}

#[test]
fn glioma_research_mission_refuses_without_real_bundle() {
    let error = NeurosurgicalAgent::default()
        .run_research_mission(&research_request(), None, None, 32)
        .unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn non_glioma_research_mission_refuses_without_public_literature_bundle() {
    let mut request = research_request();
    request.specialty = Specialty::ChiariMalformation;
    let error = NeurosurgicalAgent::default()
        .run_research_mission(&request, None, None, 32)
        .unwrap_err();
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn real_data_review_dispositions_are_digest_bound_and_replay_safe() {
    let data = bundle();
    let queue = NeurosurgicalAgent::default()
        .real_data_review_queue(&data, &RealDataReviewQueueQuery::default())
        .expect("queue derives");
    let first = queue.items[0].task_id.clone();
    let second = queue.items[1].task_id.clone();
    let decisions = vec![
        RealDataReviewDecision {
            task_id: second.clone(),
            disposition: RealDataReviewDisposition::Unresolved,
            reviewer_id: "reviewer-b".to_string(),
        },
        RealDataReviewDecision {
            task_id: first.clone(),
            disposition: RealDataReviewDisposition::Reviewed,
            reviewer_id: "reviewer-a".to_string(),
        },
    ];
    let agent = NeurosurgicalAgent::default();
    let report = agent
        .real_data_review_disposition(&queue, &decisions)
        .expect("dispositions apply");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-review-disposition/0.1"
    );
    assert_eq!(report.submitted_decision_count, 2);
    assert_eq!(report.accepted_decision_count, 2);
    assert_eq!(report.resolved_decision_count, 1);
    assert_eq!(report.unresolved_decision_count, 1);
    assert_eq!(report.undecided_returned_item_count, 13);
    assert_eq!(report.pending_item_count, 14);
    assert_eq!(report.unresolved_task_ids, vec![second.clone()]);
    assert_eq!(report.decisions[0].task_id, first);
    assert_eq!(report.decisions[1].task_id, second);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.disposition_digest.len(), 64);
    assert!(report.validate_integrity(&queue).is_ok());

    let reversed = agent
        .real_data_review_disposition(&queue, &decisions.into_iter().rev().collect::<Vec<_>>())
        .expect("reordered replay applies");
    assert_eq!(reversed.disposition_digest, report.disposition_digest);

    let duplicate = vec![
        RealDataReviewDecision {
            task_id: queue.items[0].task_id.clone(),
            disposition: RealDataReviewDisposition::Reviewed,
            reviewer_id: "reviewer-a".to_string(),
        },
        RealDataReviewDecision {
            task_id: queue.items[0].task_id.clone(),
            disposition: RealDataReviewDisposition::Unresolved,
            reviewer_id: "reviewer-b".to_string(),
        },
    ];
    assert!(matches!(
        agent.real_data_review_disposition(&queue, &duplicate),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut tampered = queue.clone();
    tampered.items[0].title.push_str(" tampered");
    assert!(matches!(
        agent.real_data_review_disposition(&tampered, &[]),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
    let mut tampered_report = report.clone();
    tampered_report.pending_item_count += 1;
    assert!(tampered_report.validate_integrity(&queue).is_err());
}

#[test]
fn real_data_evidence_packet_composes_real_projections_without_a_provider() {
    let data = bundle();
    let query = RealDataEvidencePacketQuery {
        query: RealDataQuery {
            text: Some("glioblastoma".to_string()),
            limit: 4,
            ..RealDataQuery::default()
        },
        graph: bioprism_neurosurgery::EvidenceGraphQuery {
            max_nodes: 8,
            max_edges: 12,
            ..bioprism_neurosurgery::EvidenceGraphQuery::default()
        },
        review_queue: RealDataReviewQueueQuery {
            max_items: 3,
            ..RealDataReviewQueueQuery::default()
        },
        freshness: Some(RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: None,
        }),
        ..RealDataEvidencePacketQuery::default()
    };
    let packet = NeurosurgicalAgent::default()
        .real_data_evidence_packet(&data, &query)
        .expect("validated snapshot composes into a packet");
    assert_eq!(
        packet.schema_version,
        "bioprism-neurosurgery-real-data-evidence-packet/0.4"
    );
    assert_eq!(packet.bundle_digest, packet.summary.bundle_digest);
    assert_eq!(packet.source_count, 5);
    assert_eq!(packet.record_count, 88);
    assert_eq!(packet.data_query.query.limit, 4);
    assert!(packet.data_query.total_matches > 0);
    assert_eq!(packet.trial_landscape.total_matching_trials, 5);
    assert_eq!(packet.trial_landscape.returned_trial_count, 5);
    assert_eq!(packet.trial_landscape.phase_annotated_trial_count, 5);
    assert_eq!(packet.molecular_coverage.total_matching_profile_count, 54);
    assert_eq!(packet.molecular_coverage.emitted_study_count, 7);
    let cohort = packet
        .cohort_landscape
        .as_ref()
        .expect("new packets include the comparative cohort landscape");
    assert_eq!(cohort.total_matching_projects, 1);
    assert_eq!(cohort.returned_project_count, 1);
    assert_eq!(cohort.project_rows[0].project_id, "TCGA-GBM");
    assert_eq!(
        packet.reconciliation.schema_version,
        "bioprism-neurosurgery-real-data-reconciliation/0.1"
    );
    assert_eq!(packet.reconciliation.candidate_issue_count, 0);
    assert!(!packet.reconciliation.requires_review);
    assert_eq!(packet.review_queue.returned_item_count, 3);
    assert_eq!(packet.review_queue.omitted_item_count, 12);
    assert_eq!(
        packet.freshness.as_ref().map(|report| report.status),
        Some(bioprism_neurosurgery::RealDataFreshnessStatus::Stale)
    );
    assert_eq!(packet.graph.nodes.len(), 8);
    assert!(packet.packet_digest.len() == 64);
    assert!(packet.provenance_bound);
    assert!(!packet.synthetic_data);
    assert!(packet.human_review_required);
    assert_eq!(packet.provider, "none");
    assert!(!packet.network);
    packet
        .validate_integrity()
        .expect("evidence packet is self-consistent");
    packet
        .validate_for_inputs(&data)
        .expect("evidence packet replays against the exact snapshot");
    let mut tampered = packet.clone();
    tampered.query_match_count = tampered.query_match_count.saturating_add(1);
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn legacy_evidence_packets_without_cohort_landscape_still_deserialize_and_replay() {
    let data = bundle();
    let packet = NeurosurgicalAgent::default()
        .real_data_evidence_packet(&data, &RealDataEvidencePacketQuery::default())
        .expect("validated snapshot composes into a packet");
    let mut value = serde_json::to_value(&packet).expect("packet serializes");
    value
        .as_object_mut()
        .expect("packet is an object")
        .remove("cohort_landscape");
    let object = value.as_object_mut().expect("packet is an object");
    let freshness_digest = packet
        .freshness
        .as_ref()
        .map(|report| report.freshness_digest.as_str());
    let bytes = serde_json::to_vec(&(
        packet.bundle_digest.as_str(),
        &packet.query,
        packet.coverage.coverage_digest.as_str(),
        packet.graph.graph_digest.as_str(),
        packet.data_query.bundle_digest.as_str(),
        packet.trial_landscape.landscape_digest.as_str(),
        packet.molecular_coverage.coverage_digest.as_str(),
        packet.reconciliation.reconciliation_digest.as_str(),
        packet.review_queue.queue_digest.as_str(),
        freshness_digest,
    ))
    .expect("legacy packet digest tuple serializes");
    object.insert(
        "packet_digest".to_string(),
        serde_json::Value::String(format!("{:x}", Sha256::digest(bytes))),
    );
    let legacy: bioprism_neurosurgery::RealDataEvidencePacketReport =
        serde_json::from_value(value).expect("legacy packet deserializes");
    legacy
        .validate_integrity()
        .expect("legacy packet remains structurally valid");
    legacy
        .validate_for_inputs(&data)
        .expect("legacy packet remains replayable against the exact snapshot");
    assert!(legacy.cohort_landscape.is_none());
}

#[test]
fn real_data_draft_audit_requires_packet_citations_and_blocks_clinical_posture() {
    let data = bundle();
    let request = RealDataDraftAuditRequest {
        query: RealDataEvidencePacketQuery {
            query: RealDataQuery {
                text: Some("glioblastoma".to_string()),
                limit: 4,
                ..RealDataQuery::default()
            },
            graph: bioprism_neurosurgery::EvidenceGraphQuery {
                max_nodes: 8,
                max_edges: 12,
                ..bioprism_neurosurgery::EvidenceGraphQuery::default()
            },
            ..RealDataEvidencePacketQuery::default()
        },
        claims: vec![
            RealDataDraftClaim {
                claim_id: "population-summary".to_string(),
                kind: RealDataDraftClaimKind::PopulationSummary,
                scope: RealDataDraftScope::PopulationAggregate,
                text: "The snapshot reports an aggregate public cohort count.".to_string(),
                citations: vec![RealDataDraftCitation {
                    record_kind: RealDataRecordKind::GenomicProject,
                    record_id: "TCGA-GBM".to_string(),
                }],
                explicitly_hypothetical: false,
            },
            RealDataDraftClaim {
                claim_id: "missing-record".to_string(),
                kind: RealDataDraftClaimKind::SourceObservation,
                scope: RealDataDraftScope::PublicRecordMetadata,
                text: "This claim cites a record outside the bounded packet.".to_string(),
                citations: vec![RealDataDraftCitation {
                    record_kind: RealDataRecordKind::LiteratureArticle,
                    record_id: "not-in-packet".to_string(),
                }],
                explicitly_hypothetical: false,
            },
            RealDataDraftClaim {
                claim_id: "action".to_string(),
                kind: RealDataDraftClaimKind::ClinicalAction,
                scope: RealDataDraftScope::PublicRecordMetadata,
                text: "A clinical action is not allowed here.".to_string(),
                citations: vec![RealDataDraftCitation {
                    record_kind: RealDataRecordKind::ClinicalTrial,
                    record_id: "NCT00005955".to_string(),
                }],
                explicitly_hypothetical: false,
            },
        ],
    };
    let agent = NeurosurgicalAgent::default();
    let report = agent
        .real_data_draft_audit(&data, &request)
        .expect("draft audit returns a bounded structural report");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-real-data-draft-audit/0.1"
    );
    assert_eq!(report.claim_count, 3);
    assert_eq!(report.grounded_claim_count, 1);
    assert_eq!(report.blocked_claim_count, 2);
    assert_eq!(report.status, RealDataDraftClaimStatus::Blocked);
    assert_eq!(report.packet.query.query.limit, 4);
    assert_eq!(report.packet.graph.nodes.len(), 8);
    assert_eq!(report.packet_digest.len(), 64);
    assert_eq!(report.draft_digest.len(), 64);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.claims[0].claim_id, "action");
    assert_eq!(report.claims[0].status, RealDataDraftClaimStatus::Blocked);
    assert_eq!(report.claims[1].claim_id, "missing-record");
    assert_eq!(report.claims[2].claim_id, "population-summary");
    assert_eq!(
        report.claims[2].status,
        RealDataDraftClaimStatus::GroundedForHumanReview
    );

    let mut reordered = request.clone();
    reordered.claims.reverse();
    assert_eq!(
        agent
            .real_data_draft_audit(&data, &reordered)
            .expect("reordered draft replays")
            .draft_digest,
        report.draft_digest
    );
}
