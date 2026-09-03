use bioprism_neurosurgery::{
    CaseRequest, EvidenceTier, NeurosurgeryError, NeurosurgicalAgent, PublicLiteratureBundle,
    PublicLiteratureDraftAuditRequest, PublicLiteratureEvidencePacketQuery,
    PublicLiteratureMatrixQuery, PublicLiteratureQuery, PublicLiteratureReasoningContextQuery,
    PublicLiteratureRecord, PublicLiteratureSource, RealDataDraftCitation, RealDataDraftClaim,
    RealDataDraftClaimKind, RealDataDraftClaimStatus, RealDataDraftScope, RealDataFreshnessQuery,
    RealDataFreshnessStatus, RequestUse, SessionStatus, Specialty, ToolCapability,
    NEUROSURGERY_SCHEMA_VERSION, PUBLIC_LITERATURE_SCHEMA_VERSION,
};

fn bundle() -> PublicLiteratureBundle {
    let records = Specialty::ALL
        .iter()
        .enumerate()
        .map(|(index, specialty)| PublicLiteratureRecord {
            source_id: "pubmed_neurosurgery_2026-08-30".to_string(),
            specialty: *specialty,
            pmid: format!("{}", 100_000 + index),
            title: format!("{} public evidence record", specialty.display_name()),
            journal: "Journal of Public Neurosurgical Research".to_string(),
            publication_date: Some("2025-01-01".to_string()),
            doi: Some(format!("10.1000/neurosurgery.{}", index)),
            abstract_text: Some(format!(
                "Metadata-only abstract for the {} research lane.",
                specialty.slug()
            )),
            abstract_truncated: false,
            publication_types: vec!["Review".to_string()],
            mesh_terms: vec![specialty.display_name().to_string()],
        })
        .collect::<Vec<_>>();
    let mut data = PublicLiteratureBundle {
        schema_version: PUBLIC_LITERATURE_SCHEMA_VERSION.to_string(),
        generated_at: "2026-08-30T00:00:00Z".to_string(),
        synthetic_data: false,
        sources: vec![PublicLiteratureSource {
            source_id: "pubmed_neurosurgery_2026-08-30".to_string(),
            authority: "U.S. National Library of Medicine PubMed".to_string(),
            uri: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed".to_string(),
            retrieved_at: "2026-08-30T00:00:00Z".to_string(),
            content_sha256: "0".repeat(64),
            record_count: records.len(),
        }],
        records,
    };
    data.sources[0].content_sha256 = data
        .canonical_source_hashes()
        .expect("hash helper works")
        .remove("pubmed_neurosurgery_2026-08-30")
        .expect("source hash exists");
    data
}

fn live_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in cross-specialty snapshot parses")
}

#[test]
fn checked_in_public_literature_uses_stable_source_ids() {
    let literature = live_bundle();
    assert_eq!(
        literature
            .sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "pubmed_glioma",
            "pubmed_cranial_base",
            "pubmed_craniosynostosis",
            "pubmed_encephalocele",
            "pubmed_spina_bifida",
            "pubmed_chiari_malformation",
        ]
    );
    assert!(
        literature
            .summary()
            .expect("snapshot validates")
            .provenance_bound
    );
}

fn research_request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: format!("{}-public-research", specialty.slug()),
        specialty,
        request_use: RequestUse::ResearchSynthesis,
        question: "Which public citation metadata and evidence gaps should a reviewer inspect?"
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
fn cross_specialty_bundle_validates_and_reports_explicit_coverage() {
    let data = bundle();
    let summary = data.summary().expect("public literature validates");
    assert!(summary.provenance_bound);
    assert!(!summary.synthetic_data);
    assert_eq!(summary.source_count, 1);
    assert_eq!(summary.record_count, 6);
    assert_eq!(summary.abstract_count, 6);
    assert_eq!(summary.abstract_truncated_count, 0);
    assert_eq!(summary.specialty_counts.len(), Specialty::ALL.len());
    assert!(summary
        .specialty_counts
        .iter()
        .all(|entry| entry.count == 1));
}

#[test]
fn query_is_specialty_scoped_source_linked_and_bounded() {
    let data = bundle();
    let result = data
        .query(&PublicLiteratureQuery {
            specialty: Some(Specialty::ChiariMalformation),
            text: Some("chiari_malformation".to_string()),
            limit: 1,
            ..Default::default()
        })
        .expect("query validates");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.returned_matches, 1);
    assert!(!result.truncated);
    assert_eq!(result.hits[0].specialty, Specialty::ChiariMalformation);
    assert!(result.hits[0]
        .source_uri
        .starts_with("https://eutils.ncbi.nlm.nih.gov/"));
    assert_eq!(
        result.hits[0].record_uri,
        "https://pubmed.ncbi.nlm.nih.gov/100005/"
    );
    assert!(result.hits[0]
        .abstract_excerpt
        .as_deref()
        .is_some_and(|value| value.contains("chiari_malformation")));
    result
        .validate_integrity()
        .expect("persisted public-literature query result is structurally valid");
    result
        .validate_for_inputs(&data)
        .expect("persisted public-literature query result replays against the exact snapshot");
    let mut rebound = result.clone();
    rebound.query.text = Some("different query".to_string());
    assert!(rebound.validate_for_inputs(&data).is_err());
}

#[test]
fn query_truncation_and_zero_match_remain_distinct() {
    let data = bundle();
    let truncated = data
        .query(&PublicLiteratureQuery {
            specialty: None,
            text: Some("public evidence".to_string()),
            limit: 2,
            ..Default::default()
        })
        .expect("bounded query works");
    assert_eq!(truncated.total_matches, 6);
    assert_eq!(truncated.returned_matches, 2);
    assert!(truncated.truncated);

    let empty = data
        .query(&PublicLiteratureQuery {
            specialty: Some(Specialty::Glioma),
            text: Some("not-present".to_string()),
            limit: 2,
            ..Default::default()
        })
        .expect("empty query works");
    assert_eq!(empty.total_matches, 0);
    assert_eq!(empty.returned_matches, 0);
    assert!(!empty.truncated);
}

#[test]
fn public_literature_packet_is_digest_bound_and_exactly_replayable() {
    let packet = bundle()
        .evidence_packet(&PublicLiteratureEvidencePacketQuery {
            query: PublicLiteratureQuery {
                specialty: Some(Specialty::Glioma),
                limit: 1,
                ..Default::default()
            },
            freshness: None,
        })
        .expect("public literature packet composes");
    packet
        .validate_integrity()
        .expect("public packet should carry a valid envelope");
    packet
        .validate_for_inputs(&bundle())
        .expect("public packet should replay against the exact snapshot");
    let mut tampered = packet.clone();
    tampered.query_match_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = packet;
    rebound.query.query.specialty = Some(Specialty::ChiariMalformation);
    assert!(rebound.validate_for_inputs(&bundle()).is_err());
}

#[test]
fn public_literature_reasoning_context_is_bounded_and_source_addressable() {
    let report = bundle()
        .reasoning_context(&PublicLiteratureReasoningContextQuery {
            packet: PublicLiteratureEvidencePacketQuery {
                query: PublicLiteratureQuery {
                    specialty: Some(Specialty::ChiariMalformation),
                    text: Some("public evidence".to_string()),
                    limit: 1,
                    ..Default::default()
                },
                freshness: Some(RealDataFreshnessQuery {
                    as_of: "2027-08-31T00:00:00Z".to_string(),
                    max_age_days: 14,
                    source_id: None,
                }),
            },
            max_chars: 8_000,
            include_abstracts: true,
        })
        .expect("public literature context renders");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-public-literature-reasoning-context/0.1"
    );
    assert_eq!(report.included_citation_count, 1);
    assert_eq!(report.omitted_citation_count, 0);
    assert!(!report.synthetic_data);
    assert!(!report.network);
    assert!(report
        .context_text
        .contains("AURORA PUBLIC-NEUROSURGICAL LITERATURE"));
    assert!(report.context_text.contains("<pubmed_record>"));
    assert!(report.context_text.contains("FRESHNESS: status="));
    assert!(report.context_text.contains("max_age_days=14"));
    assert!(report.context_text.contains("pmid: 100005"));
    assert!(report.context_text.contains("untrusted source text"));
    assert_eq!(
        report.citations[0].record_uri,
        "https://pubmed.ncbi.nlm.nih.gov/100005/"
    );
    assert_eq!(report.context_digest.len(), 64);
    report
        .validate_integrity()
        .expect("public literature context should carry a valid envelope");
    report
        .validate_for_inputs(&bundle())
        .expect("public literature context should replay against the snapshot");
    let mut tampered = report.clone();
    tampered.context_text.push_str("tampered");
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.packet.query.specialty = Some(Specialty::Glioma);
    assert!(rebound.validate_for_inputs(&bundle()).is_err());
}

#[test]
fn public_literature_reasoning_context_reports_query_and_character_omissions() {
    let report = bundle()
        .reasoning_context(&PublicLiteratureReasoningContextQuery {
            max_chars: 256,
            ..Default::default()
        })
        .expect("small public literature context remains bounded");
    assert!(report.truncated);
    assert_eq!(report.omitted_citation_count, 6);
    assert!(report.context_char_count <= 256);
    assert_eq!(report.included_citation_count, 0);
}

#[test]
fn literature_matrix_fans_out_deterministically_and_preserves_empty_lanes() {
    let data = bundle();
    let report = data
        .literature_matrix(&PublicLiteratureMatrixQuery {
            specialties: vec![Specialty::ChiariMalformation, Specialty::Glioma],
            query: PublicLiteratureQuery {
                text: Some("public evidence".to_string()),
                limit: 1,
                ..Default::default()
            },
        })
        .expect("matrix validates");
    assert_eq!(report.specialty_count, 2);
    assert_eq!(report.lanes.len(), 2);
    assert_eq!(report.lanes[0].specialty, Specialty::Glioma);
    assert_eq!(report.lanes[1].specialty, Specialty::ChiariMalformation);
    assert_eq!(report.total_match_count, 2);
    assert_eq!(report.total_returned_count, 2);
    assert_eq!(report.non_empty_lane_count, 2);
    assert!(report.empty_lane_specialties.is_empty());
    assert!(report
        .lanes
        .iter()
        .all(|lane| lane.packet.query_result.query.specialty == Some(lane.specialty)));
    assert_eq!(report.matrix_digest.len(), 64);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    report
        .validate_integrity()
        .expect("literature matrix should carry a valid envelope");
    report
        .validate_for_inputs(&bundle())
        .expect("literature matrix should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.total_match_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.query.text = Some("not-present".to_string());
    assert!(rebound.validate_for_inputs(&bundle()).is_err());

    let empty = data
        .literature_matrix(&PublicLiteratureMatrixQuery {
            specialties: vec![Specialty::Encephalocele, Specialty::SpinaBifida],
            query: PublicLiteratureQuery {
                text: Some("not-present".to_string()),
                limit: 1,
                ..Default::default()
            },
        })
        .expect("empty lanes remain valid");
    assert_eq!(empty.non_empty_lane_count, 0);
    assert_eq!(empty.empty_lane_specialties.len(), 2);
    assert_eq!(empty.total_match_count, 0);

    let duplicate = data.literature_matrix(&PublicLiteratureMatrixQuery {
        specialties: vec![Specialty::Glioma, Specialty::Glioma],
        ..Default::default()
    });
    assert!(matches!(
        duplicate,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn query_supports_review_facing_tag_and_date_facets() {
    let data = bundle();
    let result = data
        .query(&PublicLiteratureQuery {
            specialty: Some(Specialty::Glioma),
            publication_type: Some("review".to_string()),
            mesh_term: Some("glioma".to_string()),
            from_date: Some("2024-01-01".to_string()),
            to_date: Some("2025-12-31".to_string()),
            limit: 4,
            ..Default::default()
        })
        .expect("tag and date facets validate");
    assert_eq!(result.total_matches, 1);
    assert_eq!(result.returned_matches, 1);

    let invalid = data.query(&PublicLiteratureQuery {
        from_date: Some("2026-01-01".to_string()),
        to_date: Some("2025-01-01".to_string()),
        ..Default::default()
    });
    assert!(matches!(
        invalid,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn public_literature_packet_binds_local_draft_claims_to_emitted_pmids() {
    let data = bundle();
    let request = PublicLiteratureDraftAuditRequest {
        query: PublicLiteratureEvidencePacketQuery {
            query: PublicLiteratureQuery {
                specialty: Some(Specialty::ChiariMalformation),
                limit: 1,
                ..Default::default()
            },
            freshness: Some(RealDataFreshnessQuery {
                as_of: "2027-08-31T00:00:00Z".to_string(),
                max_age_days: 30,
                source_id: None,
            }),
        },
        claims: vec![
            RealDataDraftClaim {
                claim_id: "chiari-citation".to_string(),
                kind: RealDataDraftClaimKind::SourceObservation,
                scope: RealDataDraftScope::CitationMetadata,
                text: "The packet contains one Chiari citation record.".to_string(),
                citations: vec![RealDataDraftCitation {
                    record_kind: bioprism_neurosurgery::RealDataRecordKind::LiteratureArticle,
                    record_id: "100005".to_string(),
                }],
                explicitly_hypothetical: false,
            },
            RealDataDraftClaim {
                claim_id: "blocked-action".to_string(),
                kind: RealDataDraftClaimKind::ClinicalAction,
                scope: RealDataDraftScope::CitationMetadata,
                text: "A reviewer should perform an intervention.".to_string(),
                citations: vec![RealDataDraftCitation {
                    record_kind: bioprism_neurosurgery::RealDataRecordKind::LiteratureArticle,
                    record_id: "100005".to_string(),
                }],
                explicitly_hypothetical: false,
            },
        ],
    };
    let report = data
        .audit_draft(&request)
        .expect("public-literature draft audit validates");
    assert_eq!(report.packet.query_result.returned_matches, 1);
    assert_eq!(report.grounded_claim_count, 1);
    assert_eq!(report.blocked_claim_count, 1);
    assert_eq!(report.status, RealDataDraftClaimStatus::Blocked);
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert_eq!(report.packet_digest.len(), 64);
    assert_eq!(report.draft_digest.len(), 64);
    assert_eq!(
        report
            .packet
            .freshness
            .as_ref()
            .map(|freshness| freshness.status),
        Some(RealDataFreshnessStatus::Stale)
    );

    let mut reordered = request.clone();
    reordered.claims.reverse();
    assert_eq!(
        report.draft_digest,
        data.audit_draft(&reordered)
            .expect("claim order is canonicalized")
            .draft_digest
    );
}

#[test]
fn tampering_and_synthetic_metadata_are_refused() {
    let mut tampered = bundle();
    tampered.records[0].title.push_str(" changed");
    assert!(matches!(
        tampered.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));

    let mut marked = bundle();
    marked.records[0].journal = "Synthetic fixture journal".to_string();
    assert!(matches!(
        marked.validate(),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}

#[test]
fn evidence_projection_preserves_unverified_and_research_only_labels() {
    let evidence = bundle().evidence_records();
    assert_eq!(evidence.len(), 6);
    assert!(evidence.iter().all(|record| {
        record.tier == EvidenceTier::Unverified
            && record.supports == vec![ToolCapability::EvidenceSynthesis]
            && record.citation.contains("specialty=")
    }));
    let chiari = bundle().evidence_records_for_specialty(Some(Specialty::ChiariMalformation));
    assert_eq!(chiari.len(), 1);
    assert!(chiari[0].citation.contains("specialty=chiari_malformation"));
}

#[test]
fn live_cross_specialty_literature_routes_non_glioma_requests_without_a_provider() {
    let response = NeurosurgicalAgent::default()
        .run_with_public_literature(
            &research_request(Specialty::ChiariMalformation),
            &live_bundle(),
        )
        .expect("cross-specialty public literature route works");
    let summary = response
        .public_literature
        .expect("public-literature summary is attached");
    assert_eq!(summary.record_count, 145);
    assert_eq!(summary.specialty_counts.len(), Specialty::ALL.len());
    assert!(response.report.known_inputs.iter().any(|input| {
        input.contains("validated public literature") && input.contains(&summary.bundle_digest)
    }));
    assert!(response.tool_runs.iter().any(|run| {
        run.capability == ToolCapability::EvidenceSynthesis
            && run
                .findings
                .iter()
                .any(|finding| finding.code == "public_literature_provenance")
    }));
}

#[test]
fn public_literature_route_refuses_unsupported_registry_tools() {
    let mut request = research_request(Specialty::Glioma);
    request.requested_tools = vec![ToolCapability::RealDataQuery];
    let error = NeurosurgicalAgent::default()
        .run_with_public_literature(&request, &live_bundle())
        .expect_err("generic literature must not impersonate a registry/profile bundle");
    assert!(matches!(error, NeurosurgeryError::RealDataRejected { .. }));
}

#[test]
fn public_literature_sessions_and_missions_bind_the_bundle_digest() {
    let agent = NeurosurgicalAgent::default();
    let literature = live_bundle();
    let request = research_request(Specialty::Encephalocele);
    let run = agent
        .run_session_to_review_with_public_literature(&request, &literature, 32)
        .expect("public-literature session reaches review hold");
    assert_eq!(run.session.status, SessionStatus::AwaitingHumanReview);
    assert_eq!(
        run.session.public_literature_digest.as_deref(),
        Some(
            literature
                .summary()
                .expect("snapshot validates")
                .bundle_digest
                .as_str(),
        )
    );
    assert!(run.session.real_data_digest.is_none());
    let query = PublicLiteratureQuery {
        specialty: Some(Specialty::Encephalocele),
        text: Some("encephalocele".to_string()),
        limit: 2,
        ..Default::default()
    };
    let freshness = RealDataFreshnessQuery {
        as_of: "2027-08-31T00:00:00Z".to_string(),
        max_age_days: 30,
        source_id: None,
    };
    let mission = agent
        .run_research_mission_with_public_literature_freshness(
            &request,
            &literature,
            Some(&query),
            Some(&freshness),
            32,
        )
        .expect("public-literature mission reaches review hold");
    mission
        .validate_integrity()
        .expect("persisted public-literature mission should pass its local integrity gate");
    mission
        .validate_for_inputs(&request, None, Some(&literature))
        .expect("persisted public-literature mission should replay against the exact snapshot");
    let mut tampered_query = mission.clone();
    tampered_query
        .public_literature_query
        .as_mut()
        .expect("mission query is present")
        .query
        .text = Some("different query".to_string());
    assert!(tampered_query
        .validate_for_inputs(&request, None, Some(&literature))
        .is_err());
    assert!(mission.real_data_query.is_none());
    let mission_audit = mission
        .mission_audit
        .as_ref()
        .expect("public-literature mission includes its integrity fuse");
    assert!(mission_audit.integrity_ok);
    mission_audit
        .validate_integrity()
        .expect("mission audit receipt should remain digest-valid");
    assert_eq!(mission_audit.fail_count, 0);
    let acquisition = mission
        .evidence_acquisition
        .as_ref()
        .expect("public-literature missions include the bounded acquisition worker plan");
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
    assert_eq!(
        mission
            .public_literature_query
            .as_ref()
            .unwrap()
            .total_matches,
        21
    );
    assert_eq!(
        mission
            .public_literature_freshness
            .as_ref()
            .map(|report| report.status),
        Some(RealDataFreshnessStatus::Stale)
    );
    let integrity = mission
        .public_literature_integrity_audit
        .expect("public-literature missions include integrity audit");
    assert_eq!(integrity.counts.selected_record_count, 23);
    assert!(integrity.requires_integrity_review);
    assert!(!integrity.synthetic_data);
    assert_eq!(integrity.provider, "none");
    assert!(!integrity.network);
    let queue = mission
        .public_literature_review_queue
        .expect("public-literature missions include review queue");
    assert_eq!(queue.candidate_item_count, 13);
    assert!(!queue.truncated);
    assert_eq!(queue.integrity_audit_digest, integrity.audit_digest);
    assert!(queue.human_review_required);
    assert_eq!(queue.provider, "none");
    assert!(!queue.network);
    let workbench = mission
        .public_literature_workbench
        .expect("public-literature missions include a specialty workbench");
    assert_eq!(
        workbench.schema_version,
        "bioprism-neurosurgery-public-literature-workbench/0.1"
    );
    assert_eq!(workbench.lanes.len(), 1);
    assert_eq!(workbench.lanes[0].specialty, Specialty::Encephalocele);
    assert_eq!(
        workbench.lanes[0].integrity_audit_digest,
        integrity.audit_digest
    );
    assert_eq!(
        workbench.bundle_digest,
        run.session.public_literature_digest.clone().unwrap()
    );
    assert!(!workbench.lanes[0].profile.evidence_questions.is_empty());
    assert!(workbench.human_review_required);
    assert_eq!(workbench.provider, "none");
    assert!(!workbench.network);
    assert_eq!(
        mission.run.session.public_literature_digest,
        run.session.public_literature_digest
    );
    let context = mission
        .public_literature_reasoning_context
        .expect("public-literature missions include a bounded local-model context");
    let packet = mission
        .public_literature_evidence_packet
        .as_ref()
        .expect("public-literature missions include a bounded evidence packet");
    assert_eq!(
        context.bundle_digest,
        run.session.public_literature_digest.unwrap()
    );
    assert_eq!(context.packet_digest, packet.packet_digest);
    packet
        .validate_integrity()
        .expect("mission public packet should remain digest-valid");
    context
        .validate_integrity()
        .expect("mission public context should remain digest-valid");
    assert!(!context.synthetic_data);
    assert!(!context.network);
    assert!(context
        .context_text
        .contains("AURORA PUBLIC-NEUROSURGICAL LITERATURE"));
    let plan = mission
        .research_plan
        .expect("public-literature missions include the ordered research plan");
    assert_eq!(
        plan.public_literature_digest.as_deref(),
        Some(context.bundle_digest.as_str())
    );
    assert!(!plan.tasks.is_empty());
    assert!(plan.human_review_required);
    assert_eq!(plan.provider, "none");
    assert!(!plan.network);
    let brief = mission
        .research_brief
        .expect("public-literature missions include the deterministic research brief");
    assert_eq!(
        brief.source,
        bioprism_neurosurgery::ResearchBriefSource::PublicLiterature
    );
    assert_eq!(brief.bundle_digest, context.bundle_digest);
    assert!(brief.non_empty_topic_count > 0);
    assert!(brief.human_review_required);
    assert_eq!(brief.provider, "none");
    assert!(!brief.network);
}

#[test]
fn public_literature_mission_canonicalizes_omitted_specialty_across_query_packet_and_context() {
    let literature = bundle();
    let request = research_request(Specialty::Encephalocele);
    let mission = NeurosurgicalAgent::default()
        .run_research_mission_with_public_literature(
            &request,
            &literature,
            Some(&PublicLiteratureQuery {
                specialty: None,
                text: Some("public evidence".to_string()),
                limit: 4,
                ..Default::default()
            }),
            32,
        )
        .expect("omitted specialty is canonicalized to the request lane");
    let query_result = mission
        .public_literature_query
        .as_ref()
        .expect("explicit query remains visible");
    assert_eq!(query_result.query.specialty, Some(Specialty::Encephalocele));
    assert_eq!(query_result.total_matches, 1);
    let packet = mission
        .public_literature_evidence_packet
        .as_ref()
        .expect("packet is present");
    assert_eq!(packet.query.query.specialty, Some(Specialty::Encephalocele));
    let context = mission
        .public_literature_reasoning_context
        .as_ref()
        .expect("context is present");
    assert_eq!(context.packet_digest, packet.packet_digest);
    assert_eq!(context.query.packet.query, packet.query.query);
}
