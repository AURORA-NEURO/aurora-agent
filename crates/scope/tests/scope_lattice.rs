use bioprism_scope::{
    meet, AggregationOperator, DimensionRegistry, EmptyReason, Interval, LossLedger, MappingCheck,
    MappingKind, Meet, ScopeClass, ScopeKey, ScopeMapping, ScopeValue, Timestamp,
};
use serde_json::json;
use std::collections::BTreeSet;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("valid timestamp")
}

#[test]
fn parses_the_fixture_timestamp_format() {
    let parsed = ts("2025-01-01T00:00:00Z");
    assert_eq!(parsed.to_rfc3339(), "2025-01-01T00:00:00Z");
    assert_eq!(parsed.as_nanos_utc(), 1_735_689_600 * 1_000_000_000);
}

#[test]
fn offsets_normalise_to_the_same_instant() {
    assert_eq!(ts("2025-06-15T00:00:00Z"), ts("2025-06-15T02:00:00+02:00"));
    assert_eq!(ts("2025-06-15T00:00:00Z"), ts("2025-06-14T19:00:00-05:00"));
    assert_eq!(ts("2025-06-15T00:00:00Z"), ts("2025-06-15T05:30:00+0530"));
}

#[test]
fn fractional_seconds_are_preserved_to_nanoseconds() {
    assert_eq!(ts("2025-01-01T00:00:00.5Z").as_nanos_utc() % 1_000_000_000, 500_000_000);
    assert_eq!(ts("2025-01-01T00:00:00.000000001Z").as_nanos_utc() % 1_000_000_000, 1);
    assert!(ts("2025-01-01T00:00:00.5Z") > ts("2025-01-01T00:00:00Z"));
}

#[test]
fn leap_years_and_epoch_boundaries_round_trip() {
    for text in [
        "1970-01-01T00:00:00Z",
        "1969-12-31T23:59:59Z",
        "2024-02-29T12:00:00Z",
        "2000-02-29T00:00:00Z",
        "2100-03-01T00:00:00Z",
        "2026-08-08T13:45:07Z",
    ] {
        assert_eq!(ts(text).to_rfc3339(), text, "round trip failed for {text}");
    }
}

#[test]
fn malformed_timestamps_are_rejected_not_coerced() {
    for text in [
        "2025-13-01T00:00:00Z",
        "2025-02-30T00:00:00Z",
        "2025-01-01T25:00:00Z",
        "2025-01-01",
        "not-a-time",
        "2025-01-01T00:00:00Zjunk",
    ] {
        assert!(Timestamp::parse(text).is_err(), "should reject {text}");
    }
}

#[test]
fn refinement_is_a_partial_order_with_incomparable_scopes() {
    let cohort = ScopeKey::new().exact("cohort", "RG-DEMO-001");
    let cohort_subject = ScopeKey::new()
        .exact("cohort", "RG-DEMO-001")
        .exact("subject", "S001");
    let other_site = ScopeKey::new().exact("site", "SITE-A");

    assert!(cohort.refines(&cohort), "refinement is reflexive");
    assert!(cohort_subject.refines(&cohort), "more bindings is narrower");
    assert!(!cohort.refines(&cohort_subject));

    assert!(!cohort.refines(&other_site));
    assert!(!other_site.refines(&cohort));
}

#[test]
fn one_of_narrows_to_exact_but_not_the_reverse() {
    let sites: BTreeSet<String> = ["SITE-A", "SITE-B"].into_iter().map(String::from).collect();
    let broad = ScopeKey::new().bind("site", ScopeValue::OneOf(sites));
    let narrow = ScopeKey::new().exact("site", "SITE-A");
    assert!(narrow.refines(&broad));
    assert!(!broad.refines(&narrow));
}

#[test]
fn meet_of_disjoint_dimensions_is_their_union() {
    let left = ScopeKey::new().exact("cohort", "RG-DEMO-001");
    let right = ScopeKey::new().exact("site", "SITE-A");
    let combined = meet(&left, &right);
    let scope = combined.scope().expect("inhabited");
    assert_eq!(scope.len(), 2);
    assert!(scope.refines(&left) && scope.refines(&right));
}

#[test]
fn unequal_exact_values_are_empty_not_conflict() {
    let left = ScopeKey::new().exact("cohort", "A");
    let right = ScopeKey::new().exact("cohort", "B");
    match meet(&left, &right) {
        Meet::Empty { dimension, reason } => {
            assert_eq!(dimension, "cohort");
            assert_eq!(reason, EmptyReason::UnequalExactValues);
        }
        other => panic!("expected Empty, got {other:?}"),
    }
}

#[test]
fn disjoint_sets_and_empty_intervals_report_their_own_reasons() {
    let a: BTreeSet<String> = ["X"].into_iter().map(String::from).collect();
    let b: BTreeSet<String> = ["Y"].into_iter().map(String::from).collect();
    let left = ScopeKey::new().bind("site", ScopeValue::OneOf(a));
    let right = ScopeKey::new().bind("site", ScopeValue::OneOf(b));
    assert!(matches!(
        meet(&left, &right),
        Meet::Empty { reason: EmptyReason::DisjointSets, .. }
    ));

    let early = ScopeKey::new().bind(
        "valid_time",
        ScopeValue::Window(Interval { start: None, end: Some(ts("2025-01-01T00:00:00Z")) }),
    );
    let late = ScopeKey::new().bind(
        "valid_time",
        ScopeValue::Window(Interval { start: Some(ts("2025-06-01T00:00:00Z")), end: None }),
    );
    assert!(matches!(
        meet(&early, &late),
        Meet::Empty { reason: EmptyReason::EmptyInterval, .. }
    ));
}

#[test]
fn kind_mismatch_is_a_conflict_not_an_empty_overlap() {
    let as_string = ScopeKey::new().exact("valid_time", "2025-01-01");
    let as_window = ScopeKey::new().bind(
        "valid_time",
        ScopeValue::Window(Interval { start: Some(ts("2025-01-01T00:00:00Z")), end: None }),
    );
    match meet(&as_string, &as_window) {
        Meet::Conflict { dimension, .. } => assert_eq!(dimension, "valid_time"),
        other => panic!("a modelling error must not be reported as no-overlap, got {other:?}"),
    }
}

#[test]
fn meet_is_commutative_on_inhabited_scopes() {
    let left = ScopeKey::new().exact("cohort", "C").exact("site", "S");
    let right = ScopeKey::new().exact("cohort", "C").exact("subject", "S001");
    assert_eq!(meet(&left, &right), meet(&right, &left));
}

#[test]
fn parses_scope_objects_carried_by_world_facts() {
    let scope = ScopeKey::from_json(&json!({ "cohort": "RG-DEMO-001" })).unwrap();
    assert_eq!(scope.get("cohort"), Some(&ScopeValue::Exact("RG-DEMO-001".into())));

    let windowed = ScopeKey::from_json(&json!({
        "cohort": "RG-DEMO-001",
        "valid_time": { "start": "2025-01-01T00:00:00Z", "end": "2025-06-01T00:00:00Z" },
        "site": ["SITE-A", "SITE-B"]
    }))
    .unwrap();
    assert_eq!(windowed.len(), 3);
    assert!(matches!(windowed.get("valid_time"), Some(ScopeValue::Window(_))));
    assert!(matches!(windowed.get("site"), Some(ScopeValue::OneOf(_))));

    assert!(ScopeKey::from_json(&json!(["not", "an", "object"])).is_err());
    assert!(ScopeKey::from_json(&json!({ "cohort": null })).is_err());
}

#[test]
fn dimension_classification_flags_unknown_names() {
    let registry = DimensionRegistry::default();
    assert_eq!(registry.classify("cohort"), ScopeClass::Identity);
    assert_eq!(registry.classify("valid_time"), ScopeClass::Time);
    assert_eq!(registry.classify("consent"), ScopeClass::Policy);
    assert_eq!(registry.classify("genome_build"), ScopeClass::Coordinate);
    assert_eq!(registry.classify("wobble"), ScopeClass::Unclassified);

    let scope = ScopeKey::from_json(&json!({ "cohort": "C", "wobble": "w" })).unwrap();
    assert_eq!(scope.unclassified_dimensions(&registry), vec!["wobble".to_string()]);
}

#[test]
fn canonical_dimensions_cannot_be_silently_reclassified() {
    let mut registry = DimensionRegistry::default();
    assert!(registry.register("cohort", ScopeClass::Policy).is_err());
    assert!(registry.register("tumour_grade", ScopeClass::Ontology).is_ok());
    assert_eq!(registry.classify("tumour_grade"), ScopeClass::Ontology);
}

#[test]
fn a_restriction_that_widens_scope_is_caught() {
    let narrow = ScopeKey::new().exact("cohort", "C").exact("subject", "S001");
    let wide = ScopeKey::new().exact("cohort", "C");

    let honest = ScopeMapping {
        from: wide.clone(),
        to: narrow.clone(),
        kind: MappingKind::Restriction,
        loss: LossLedger::default(),
    };
    assert_eq!(honest.check(), MappingCheck::Sound);

    let mislabelled = ScopeMapping {
        from: narrow,
        to: wide,
        kind: MappingKind::Restriction,
        loss: LossLedger::default(),
    };
    assert_eq!(mislabelled.check(), MappingCheck::MisdeclaredRestriction);
}

#[test]
fn transport_and_extension_must_declare_what_they_lost() {
    let from = ScopeKey::new().exact("specimen", "SP-1");
    let to = ScopeKey::new().exact("specimen", "SP-2");

    let silent = ScopeMapping {
        from: from.clone(),
        to: to.clone(),
        kind: MappingKind::Transport { justification: "same block".into() },
        loss: LossLedger::default(),
    };
    assert_eq!(silent.check(), MappingCheck::UndeclaredLoss);

    let declared = ScopeMapping {
        from,
        to,
        kind: MappingKind::Transport { justification: "same block".into() },
        loss: LossLedger::default()
            .adding_uncertainty("cross-aliquot batch effect")
            .conditioned_on("research-only"),
    };
    assert_eq!(declared.check(), MappingCheck::Sound);
}

#[test]
fn aggregation_must_name_its_operator() {
    let mapping = ScopeMapping {
        from: ScopeKey::new().exact("subject", "S001"),
        to: ScopeKey::new().exact("cohort", "C"),
        kind: MappingKind::Aggregation { operator: AggregationOperator::Mean },
        loss: LossLedger::default().discarding("per-subject variance"),
    };
    assert_eq!(mapping.check(), MappingCheck::Sound);
    let encoded = serde_json::to_string(&mapping.kind).unwrap();
    assert!(encoded.contains("\"operator\":\"mean\""), "got {encoded}");
}
