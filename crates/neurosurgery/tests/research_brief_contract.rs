use bioprism_neurosurgery::{
    CaseRequest, NeurosurgicalAgent, NeurosurgicalResearchBriefQuery, PublicLiteratureBundle,
    PublicLiteratureQuery, RealDataFreshnessQuery, RealDataQuery, RealDataRecordKind,
    RealGliomaBundle, RequestUse, ResearchBriefSource, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};

fn real_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("checked-in real glioma snapshot parses")
}

fn literature_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public-literature snapshot parses")
}

fn request(specialty: Specialty) -> CaseRequest {
    CaseRequest {
        schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
        case_id: format!("brief-{}", specialty.slug()),
        specialty,
        request_use: RequestUse::ResearchSynthesis,
        question:
            "Which source-linked topic lanes and unresolved review obligations should be inspected?"
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
fn real_glioma_brief_extracts_topics_from_the_checked_in_public_snapshot() {
    let agent = NeurosurgicalAgent::default();
    let query = NeurosurgicalResearchBriefQuery {
        real_data_query: Some(RealDataQuery {
            record_kind: Some(RealDataRecordKind::LiteratureArticle),
            limit: 128,
            ..RealDataQuery::default()
        }),
        focus_terms: vec!["MGMT".to_string()],
        max_topics: 12,
        max_records_per_topic: 4,
        include_abstracts: true,
        freshness: Some(RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: None,
        }),
        ..NeurosurgicalResearchBriefQuery::default()
    };
    let report = agent
        .research_brief(
            &request(Specialty::Glioma),
            Some(&real_bundle()),
            None,
            &query,
        )
        .expect("real snapshot brief is accepted");
    assert_eq!(report.source, ResearchBriefSource::RealGlioma);
    assert!(!report.synthetic_data);
    assert!(report.provenance_bound);
    assert_eq!(report.bundle_digest.len(), 64);
    assert_eq!(report.brief_digest.len(), 64);
    assert!(report.non_empty_topic_count > 0);
    assert!(report
        .topics
        .iter()
        .any(|topic| topic.topic_id == "caller_focus"));
    assert!(report.freshness.as_ref().is_some_and(
        |freshness| freshness.status == bioprism_neurosurgery::RealDataFreshnessStatus::Stale
    ));
    assert!(report
        .topics
        .iter()
        .flat_map(|topic| topic.records.iter())
        .any(|record| record.abstract_excerpt.is_some()));
    assert!(report
        .limitations
        .iter()
        .any(|limitation| limitation.contains("lexical extraction")));
    report
        .validate_integrity()
        .expect("brief digest and topic projections are self-consistent");
    report
        .validate_for_inputs(&request(Specialty::Glioma), Some(&real_bundle()), None)
        .expect("brief replays against the exact real snapshot");
    let mut tampered = report.clone();
    tampered.topics[0].returned_record_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound_request = request(Specialty::Glioma);
    rebound_request.question.push_str(" (rebound)");
    assert!(report
        .validate_for_inputs(&rebound_request, Some(&real_bundle()), None)
        .is_err());
}

#[test]
fn public_literature_brief_is_specialty_scoped_and_digest_deterministic() {
    let agent = NeurosurgicalAgent::default();
    let query = NeurosurgicalResearchBriefQuery {
        public_literature_query: Some(PublicLiteratureQuery {
            specialty: Some(Specialty::ChiariMalformation),
            limit: 128,
            ..PublicLiteratureQuery::default()
        }),
        max_topics: 8,
        max_records_per_topic: 3,
        ..NeurosurgicalResearchBriefQuery::default()
    };
    let literature = literature_bundle();
    let first = agent
        .research_brief(
            &request(Specialty::ChiariMalformation),
            None,
            Some(&literature),
            &query,
        )
        .expect("Chiari public literature brief is accepted");
    let second = agent
        .research_brief(
            &request(Specialty::ChiariMalformation),
            None,
            Some(&literature),
            &query,
        )
        .expect("repeat brief is accepted");
    assert_eq!(first.source, ResearchBriefSource::PublicLiterature);
    assert_eq!(first.brief_digest, second.brief_digest);
    assert!(first
        .topics
        .iter()
        .flat_map(|topic| topic.records.iter())
        .all(|record| record.specialty == Specialty::ChiariMalformation));
    assert!(first
        .review_prompts
        .iter()
        .any(|prompt| prompt.contains("reference")));
}

#[test]
fn brief_refuses_dual_source_queries_and_unbounded_limits() {
    let agent = NeurosurgicalAgent::default();
    let both = NeurosurgicalResearchBriefQuery {
        real_data_query: Some(RealDataQuery::default()),
        public_literature_query: Some(PublicLiteratureQuery::default()),
        ..NeurosurgicalResearchBriefQuery::default()
    };
    assert!(agent
        .research_brief(
            &request(Specialty::Glioma),
            Some(&real_bundle()),
            None,
            &both
        )
        .is_err());
    let too_many = NeurosurgicalResearchBriefQuery {
        max_records_per_topic: 33,
        ..NeurosurgicalResearchBriefQuery::default()
    };
    assert!(agent
        .research_brief(
            &request(Specialty::Glioma),
            Some(&real_bundle()),
            None,
            &too_many
        )
        .is_err());
}
