use bioprism_neurosurgery::{
    LiteratureLinkAuditQuery, LiteratureLinkKind, PublicLiteratureBundle, RealGliomaBundle,
    Specialty,
};

fn real_bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("checked-in real glioma snapshot parses")
}

fn public_bundle() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public literature snapshot parses")
}

#[test]
fn real_snapshot_links_exact_pmids_and_dois() {
    let report = real_bundle()
        .literature_link_audit(&public_bundle(), &LiteratureLinkAuditQuery::default())
        .expect("validated public snapshots link");

    assert_eq!(report.counts.real_literature_records, 20);
    assert_eq!(report.counts.selected_public_literature_records, 25);
    assert_eq!(report.counts.linked_real_records, 12);
    assert_eq!(report.counts.linked_public_records, 12);
    assert_eq!(report.counts.unmatched_real_records, 8);
    assert_eq!(report.counts.unmatched_public_records, 13);
    assert_eq!(report.counts.pmid_match_count, 12);
    assert_eq!(report.counts.doi_match_count, 12);
    assert_eq!(report.counts.metadata_mismatch_count, 0);
    assert_eq!(report.counts.identifier_conflict_count, 0);
    assert_eq!(report.links.len(), 12);
    assert!(report
        .links
        .iter()
        .all(|link| link.match_kinds == vec![LiteratureLinkKind::Pmid, LiteratureLinkKind::Doi]));
    assert!(report.requires_link_review);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "unmatched_real_literature"));
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "unmatched_public_literature"));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.audit_digest.len(), 64);
}

#[test]
fn widened_lane_and_projection_bounds_remain_explicit() {
    let query = LiteratureLinkAuditQuery {
        public_specialty: None,
        max_links: 128,
        max_unmatched_ids: 4,
    };
    let report = real_bundle()
        .literature_link_audit(&public_bundle(), &query)
        .expect("widened lane audit succeeds");

    assert_eq!(report.query.public_specialty, None);
    assert_eq!(report.counts.selected_public_literature_records, 145);
    assert!(report.unmatched_real_pmids.len() <= 4);
    assert!(report.unmatched_public_pmids.len() <= 4);
    assert!(report.truncated);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "projection_truncated"));
}

#[test]
fn invalid_projection_bounds_are_rejected() {
    let mut query = LiteratureLinkAuditQuery {
        max_links: 0,
        ..Default::default()
    };
    assert!(real_bundle()
        .literature_link_audit(&public_bundle(), &query)
        .is_err());

    query = LiteratureLinkAuditQuery {
        public_specialty: Some(Specialty::Glioma),
        max_links: 128,
        max_unmatched_ids: 257,
    };
    assert!(real_bundle()
        .literature_link_audit(&public_bundle(), &query)
        .is_err());
}
