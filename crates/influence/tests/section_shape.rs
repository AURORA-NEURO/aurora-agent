//! The shape `bioprism-section` expects, produced end to end.
//!
//! `InfluenceClass::Bounded` exists in `bioprism-section` and nothing in the workspace constructs
//! it. These tests construct it from a real analysis of a real region and assert the resulting
//! certificate fragment is exactly what a consumer would read — so integration is a wiring change
//! rather than a redesign.

use bioprism_influence::{
    manifest, smallworld, Family, InfluenceAnalyzer, InfluenceEstimate, Perturbation,
    SmallWorldSpec, UnknownReason,
};
use bioprism_section::{InfluenceClass, OmissionManifest};

fn chain() -> bioprism_backends::QueryRegion {
    smallworld::generate(&SmallWorldSpec {
        family: Family::Chain,
        size: 3,
        cardinality: 3,
        seed: 0x5EED_0001,
    })
    .expect("the family builds")
}

#[test]
fn a_real_analysis_produces_a_bounded_group_carrying_its_numeric_bound() {
    let region = chain();
    let analysis = InfluenceAnalyzer::default()
        .analyse_factor(&region, "f.prior", &Perturbation::Removal)
        .unwrap();
    let group = manifest::omission_group_from_analysis(
        &analysis,
        7,
        ["fact.a".to_string(), "fact.b".to_string()],
    );

    assert_eq!(group.influence, InfluenceClass::Bounded);
    assert_eq!(group.count, 7);
    let bound = group.bound.expect("a bounded group carries its bound");
    assert!(bound > 0.0 && bound < 1.0);
    assert_eq!(group.examples, vec!["fact.a", "fact.b"]);
    assert!(group.influence.supports_sufficiency());
}

#[test]
fn the_generated_reason_names_the_method_and_the_perturbation_class() {
    let region = chain();
    let analysis = InfluenceAnalyzer::default()
        .structural_only()
        .analyse_factor(&region, "f.t1", &Perturbation::Removal)
        .unwrap();
    let group = manifest::omission_group_from_analysis(&analysis, 1, Vec::new());
    assert!(group.reason.contains("chain_contraction"));
    assert!(group.reason.contains("removal"));
    assert!(group.reason.contains("total_variation_on_normalised_answer"));
}

#[test]
fn an_unknown_analysis_produces_a_group_that_voids_the_sufficiency_claim() {
    let analysis = bioprism_influence::InfluenceAnalysis {
        subject: vec!["f.x".to_string()],
        perturbation: Perturbation::Removal,
        estimate: InfluenceEstimate::Unknown(UnknownReason::NoFactorTable {
            factor: "f.x".to_string(),
        }),
        attempted: Vec::new(),
    };
    let group = manifest::omission_group_from_analysis(&analysis, 3, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Unknown);
    assert_eq!(group.bound, None);
    assert!(group.reason.contains("not bounded by any implemented method"));

    let mut sheet = OmissionManifest::default();
    sheet.push(group);
    assert!(!sheet.supports_sufficiency_claim());
}

#[test]
fn a_manifest_of_bounded_groups_serialises_with_every_bound_on_the_wire() {
    let region = chain();
    let analyzer = InfluenceAnalyzer::default();
    let mut sheet = OmissionManifest::default();
    for factor in region.factors() {
        let analysis = analyzer
            .analyse_factor(&region, factor.id(), &Perturbation::Removal)
            .unwrap();
        sheet.push(manifest::omission_group_from_analysis(&analysis, 1, Vec::new()));
    }

    assert!(sheet.supports_sufficiency_claim());
    assert_eq!(sheet.count_in(InfluenceClass::Bounded), 4);
    assert_eq!(sheet.count_in(InfluenceClass::Unknown), 0);

    let json = serde_json::to_value(&sheet).unwrap();
    let groups = json["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 4);
    for group in groups {
        assert_eq!(group["influence"], "bounded");
        assert!(group["bound"].is_number());
    }
}

#[test]
fn the_summary_of_a_real_manifest_reports_the_worst_informative_bound() {
    let region = chain();
    let analyzer = InfluenceAnalyzer::default();
    let groups: Vec<_> = region
        .factors()
        .iter()
        .map(|factor| {
            let analysis = analyzer
                .analyse_factor(&region, factor.id(), &Perturbation::Removal)
                .unwrap();
            manifest::omission_group_from_analysis(&analysis, 1, Vec::new())
        })
        .collect();
    let summary = manifest::summarise(&groups);
    assert_eq!(summary.bounded_groups, 4);
    assert_eq!(summary.vacuous_groups, 0);
    assert_eq!(summary.unknown_groups, 0);
    let worst = summary.worst_informative_bound.expect("informative groups");
    assert!(
        groups
            .iter()
            .all(|group| group.bound.is_some_and(|value| value <= worst)),
        "the summary's worst bound is not the maximum"
    );
}

#[test]
fn an_empty_group_is_bounded_at_zero_rather_than_unknown() {
    let region = chain();
    let analysis = InfluenceAnalyzer::default()
        .analyse_group(&region, &[], &Perturbation::Removal)
        .unwrap();
    let group = manifest::omission_group_from_analysis(&analysis, 0, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Zero);
    assert_eq!(group.bound, Some(0.0));
    assert!(group.influence.supports_sufficiency());
}

#[test]
fn the_integration_note_states_what_becomes_false_and_what_stays_true() {
    let note = bioprism_influence::INTEGRATION_NOTE;
    assert!(note.contains("formal influence bounds"));
    assert!(note.contains("Sheaf cohomology"));
    assert!(note.contains("abstract interpretation"));
    assert!(note.contains("never potentials"));
    assert!(note.contains("structural_only"));
}
