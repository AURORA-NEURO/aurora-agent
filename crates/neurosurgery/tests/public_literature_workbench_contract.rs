// Source touch permits Windows Application Control to relink this contract binary when needed.
use bioprism_neurosurgery::{
    PublicLiteratureBundle, PublicLiteratureWorkbenchQuery, RealDataFreshnessQuery, Specialty,
};

fn snapshot() -> PublicLiteratureBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("checked-in public-literature snapshot parses")
}

#[test]
fn workbench_joins_real_lane_coverage_to_explicit_specialty_profiles() {
    let report = snapshot()
        .specialty_workbench(&PublicLiteratureWorkbenchQuery::default())
        .expect("all-lane workbench validates");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-public-literature-workbench/0.1"
    );
    assert_eq!(report.specialty_count, 6);
    assert_eq!(report.non_empty_lane_count, 6);
    assert!(report.empty_lane_specialties.is_empty());
    assert_eq!(report.total_record_count, 145);
    assert_eq!(report.total_review_issue_count, 91);
    assert_eq!(report.omitted_review_issue_count, 0);
    assert_eq!(report.truncated_lane_count, 0);
    assert_eq!(report.lanes[0].specialty, Specialty::Glioma);
    assert_eq!(report.lanes[0].record_count, 25);
    assert_eq!(report.lanes[0].abstract_count, 25);
    assert_eq!(report.lanes[0].empty_mesh_term_count, 3);
    assert!(!report.lanes[0].profile.identity_axes.is_empty());
    assert!(!report.lanes[0].profile.evidence_questions.is_empty());
    assert!(report.lanes.iter().all(|lane| {
        !lane.source_ids.is_empty()
            && !lane.integrity_audit_digest.is_empty()
            && !lane.profile.human_review_roles.is_empty()
            && !lane.design_strata.is_empty()
    }));
    assert!(report.lanes.iter().any(|lane| {
        lane.design_strata.iter().any(|stratum| {
            stratum.stratum == bioprism_neurosurgery::PublicLiteratureDesignStratum::HumanIndexed
                && !stratum.pmids.is_empty()
        })
    }));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    assert_eq!(report.workbench_digest.len(), 64);
    report
        .validate_integrity()
        .expect("workbench should carry a valid envelope");
    report
        .validate_for_inputs(&snapshot())
        .expect("workbench should replay against the exact snapshot");
    let mut tampered = report.clone();
    tampered.total_record_count += 1;
    assert!(tampered.validate_integrity().is_err());
    let mut rebound = report;
    rebound.query.specialties = Some(vec![Specialty::Glioma]);
    assert!(rebound.validate_for_inputs(&snapshot()).is_err());
}

#[test]
fn workbench_scopes_freshness_and_rejects_out_of_scope_sources() {
    let report = snapshot()
        .specialty_workbench(&PublicLiteratureWorkbenchQuery {
            specialties: Some(vec![Specialty::Glioma, Specialty::ChiariMalformation]),
            max_issues_per_lane: 8,
            freshness: Some(RealDataFreshnessQuery {
                as_of: "2027-08-31T00:00:00Z".to_string(),
                max_age_days: 30,
                source_id: Some("pubmed_glioma".to_string()),
            }),
        })
        .expect("scoped workbench validates");
    assert_eq!(report.specialty_count, 2);
    assert_eq!(report.total_record_count, 47);
    assert_eq!(report.lanes[0].specialty, Specialty::Glioma);
    assert_eq!(report.lanes[0].review_issue_count, 3);
    assert_eq!(report.lanes[0].omitted_review_issue_count, 0);
    assert_eq!(report.lanes[0].review_reasons[0].code, "missing_mesh_terms");
    assert_eq!(
        report
            .freshness
            .as_ref()
            .expect("freshness report")
            .sources
            .len(),
        1
    );

    let error = snapshot().specialty_workbench(&PublicLiteratureWorkbenchQuery {
        specialties: Some(vec![Specialty::Glioma]),
        freshness: Some(RealDataFreshnessQuery {
            as_of: "2027-08-31T00:00:00Z".to_string(),
            max_age_days: 30,
            source_id: Some("pubmed_chiari_malformation".to_string()),
        }),
        ..Default::default()
    });
    assert!(error.is_err());
}
