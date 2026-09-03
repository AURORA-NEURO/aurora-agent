use bioprism_neurosurgery::{
    CaseRequest, NeurosurgicalAgent, NeurosurgicalResearchBriefQuery, RealDataRefreshAuditQuery,
    RealGliomaBundle, ResearchBriefSource,
};

fn request() -> CaseRequest {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses")
}

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses")
}

fn audit_query() -> RealDataRefreshAuditQuery {
    RealDataRefreshAuditQuery {
        brief: NeurosurgicalResearchBriefQuery {
            focus_terms: vec!["MGMT".to_string()],
            include_abstracts: true,
            freshness: Some(bioprism_neurosurgery::RealDataFreshnessQuery {
                as_of: "2027-08-31T00:00:00Z".to_string(),
                max_age_days: 30,
                source_id: None,
            }),
            ..NeurosurgicalResearchBriefQuery::default()
        },
        ..RealDataRefreshAuditQuery::default()
    }
}

#[test]
fn refresh_audit_composes_real_snapshot_projections_without_accepting_the_refresh() {
    let agent = NeurosurgicalAgent::default();
    let before = bundle();
    let after = bundle();
    let report = agent
        .real_data_refresh_audit(&request(), &before, &after, &audit_query())
        .expect("unchanged real snapshots reconcile");
    assert_eq!(report.before_bundle_digest, report.after_bundle_digest);
    assert!(!report.structural_change_detected);
    assert!(report.source_identity_stable);
    assert!(report.record_identity_stable);
    assert!(report.requires_refresh_review);
    assert_eq!(
        report.research_brief.source,
        ResearchBriefSource::RealGlioma
    );
    assert!(report.research_brief.non_empty_topic_count > 0);
    assert!(report.freshness.is_some());
    assert!(!report.synthetic_data);
    assert!(report.provenance_bound);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(
        before
            .sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "clinicaltrials_glioblastoma",
            "gdc_tcga_gbm",
            "cbioportal_gbm_catalog",
            "nci_adult_cns_pdq",
            "pubmed_glioblastoma",
        ]
    );
    assert_eq!(report.audit_digest.len(), 64);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "freshness_review"));
    report
        .validate_integrity()
        .expect("refresh audit should carry a valid envelope");
    report
        .validate_for_inputs(&before, &after, &request())
        .expect("refresh audit should replay against exact snapshots and request");
    let mut tampered = report.clone();
    tampered.structural_change_detected = true;
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn refresh_audit_surfaces_source_metadata_drift_and_keeps_identity_facts_separate() {
    let agent = NeurosurgicalAgent::default();
    let before = bundle();
    let mut after = bundle();
    after.sources[0].retrieved_at = "2026-08-30T05:00:00Z".to_string();
    let report = agent
        .real_data_refresh_audit(&request(), &before, &after, &audit_query())
        .expect("metadata-only source drift remains auditable");
    assert!(report.structural_change_detected);
    assert!(report.source_identity_stable);
    assert!(report.record_identity_stable);
    assert!(report
        .diff
        .source_changes
        .iter()
        .any(|change| change.source_id == before.sources[0].source_id));
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "structural_changes"));
    assert!(report.requires_refresh_review);
    assert_ne!(report.before_bundle_digest, report.after_bundle_digest);
    assert_ne!(report.audit_digest, "".repeat(64));
    assert_eq!(report.coverage.total_record_count, 88);
}
