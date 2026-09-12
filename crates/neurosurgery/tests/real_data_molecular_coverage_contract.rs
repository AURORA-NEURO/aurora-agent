use bioprism_neurosurgery::{
    NeurosurgeryError, RealDataMolecularCoverageQuery, RealDataQuery, RealDataRecordKind,
    RealGliomaBundle,
};

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

#[test]
fn inventories_real_profile_modalities_per_study_without_patient_values() {
    let report = bundle()
        .molecular_coverage(&RealDataMolecularCoverageQuery {
            query: RealDataQuery {
                record_kind: Some(RealDataRecordKind::PortalMolecularProfile),
                limit: 128,
                ..RealDataQuery::default()
            },
            max_studies: 128,
        })
        .expect("real molecular metadata inventory should build");
    assert_eq!(report.total_matching_profile_count, 54);
    assert_eq!(report.returned_profile_count, 54);
    assert_eq!(report.emitted_profile_count, 54);
    assert_eq!(report.distinct_returned_study_count, 7);
    assert_eq!(report.emitted_study_count, 7);
    assert!(!report.truncated);
    assert!(!report.study_rows_truncated);
    assert!(report
        .alteration_type_counts
        .iter()
        .any(|bucket| bucket.label == "MRNA_EXPRESSION" && bucket.count == 24));
    assert!(report
        .datatype_counts
        .iter()
        .any(|bucket| bucket.label == "Z-SCORE" && bucket.count == 16));
    assert_eq!(report.patient_level_profile_count, 0);
    assert_eq!(report.description_present_count, 54);
    assert_eq!(report.missing_description_count, 0);
    assert_eq!(report.missing_alteration_type_count, 0);
    assert_eq!(report.missing_datatype_count, 0);
    assert_eq!(report.missing_study_link_count, 0);
    assert_eq!(report.genomic_project_count, 1);
    assert_eq!(report.genomic_project_data_type_counts.len(), 0);
    assert!(report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "missing_gdc_data_type_facets" && reason.count == 1));
    assert!(report.provenance_bound);
    assert!(!report.synthetic_data);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report
        .validate_for_inputs(&bundle())
        .expect("inventory should replay");
}

#[test]
fn molecular_facets_are_exact_and_case_insensitive() {
    let report = bundle()
        .molecular_coverage(&RealDataMolecularCoverageQuery {
            query: RealDataQuery {
                molecular_alteration_type: Some("mutation_extended".to_string()),
                molecular_datatype: Some("maf".to_string()),
                limit: 128,
                ..RealDataQuery::default()
            },
            ..RealDataMolecularCoverageQuery::default()
        })
        .expect("molecular facets should query local profile metadata");
    assert_eq!(report.total_matching_profile_count, 6);
    assert_eq!(report.returned_profile_count, 6);
    assert_eq!(report.emitted_profile_count, 6);
    assert_eq!(report.distinct_returned_study_count, 6);
    assert_eq!(report.alteration_type_counts[0].label, "MUTATION_EXTENDED");
    assert_eq!(report.datatype_counts[0].label, "MAF");
}

#[test]
fn molecular_coverage_preserves_explicit_bounds_and_missingness() {
    let report = bundle()
        .molecular_coverage(&RealDataMolecularCoverageQuery {
            query: RealDataQuery {
                limit: 4,
                ..RealDataQuery::default()
            },
            max_studies: 1,
        })
        .expect("bounded inventory should build");
    assert_eq!(report.total_matching_profile_count, 54);
    assert_eq!(report.returned_profile_count, 4);
    assert_eq!(report.emitted_study_count, 1);
    assert_eq!(report.omitted_study_count, 1);
    assert!(report.study_rows_truncated);
    assert_eq!(
        report.emitted_profile_count,
        report.study_rows[0].profile_count
    );
}

#[test]
fn molecular_coverage_includes_aggregate_gdc_data_type_facets() {
    let bundle = extended_bundle();
    let report = bundle
        .molecular_coverage(&RealDataMolecularCoverageQuery::default())
        .expect("extended molecular inventory should build");
    assert_eq!(report.genomic_project_count, 2);
    assert_eq!(report.genomic_project_data_type_counts.len(), 50);
    assert!(report.genomic_project_file_count > 0);
    assert!(report
        .genomic_project_data_type_counts
        .iter()
        .any(|row| row.project_id == "TCGA-GBM"
            && row.data_type == "Annotated Somatic Mutation"
            && row.file_count == 4822));
    assert!(report
        .source_ids
        .iter()
        .any(|source_id| source_id == "gdc_tcga_gbm"));
    assert!(!report
        .review_reasons
        .iter()
        .any(|reason| reason.code == "missing_gdc_data_type_facets"));
    report
        .validate_for_inputs(&bundle)
        .expect("extended inventory should replay");
}

#[test]
fn molecular_coverage_rejects_a_gdc_query_facet_in_the_profile_plane() {
    let query = RealDataMolecularCoverageQuery {
        query: RealDataQuery {
            genomic_data_type: Some("Annotated Somatic Mutation".to_string()),
            ..RealDataQuery::default()
        },
        ..RealDataMolecularCoverageQuery::default()
    };
    assert!(matches!(
        bundle().molecular_coverage(&query),
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}
