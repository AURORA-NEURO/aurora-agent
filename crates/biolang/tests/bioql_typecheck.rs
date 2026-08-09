//! One test per rejection rule, and the acceptances that keep the rules from being vacuous.
//!
//! The valuable rejections are the incomparability ones. Each of those asserts the *named blocking
//! dimension*, not merely that an error occurred, because "type error" is the diagnostic this crate
//! exists to improve on.

use bioprism_biolang::bioql::{compile, BioType, CollectionDecl, FieldDecl, QuerySchema};
use bioprism_biolang::clock::Clock;
use bioprism_biolang::error::{QueryError, TypeError};
use bioprism_biolang::Canonical;
use bioprism_scope::{ScopeKey, ScopeValue};
use bioprism_standards::{
    BuildBinding, CoordinateConvention, CoordinateSpace, Frame, FrameBinding, GenomeBuild,
    Incomparability, OntologyId, Orientation, ReferenceSpace, TermBinding, Unit,
};
use std::collections::BTreeSet;

fn unit(symbol: &str) -> Unit {
    Unit::parse(symbol).expect("symbol is in the standards unit table")
}

fn frame(id: &str, subject: &str) -> FrameBinding {
    FrameBinding::Declared(Frame::new(
        id,
        CoordinateSpace::World,
        Orientation::RAS,
        ReferenceSpace::SubjectNative {
            subject: subject.to_string(),
        },
    ))
}

fn term(local: &str, curie: &str, release: &str) -> TermBinding {
    TermBinding::exact(
        local,
        OntologyId::parse(curie, release).expect("well-formed curie"),
    )
    .expect("binding has a local term")
}

/// A schema wide enough to write every rejection against.
fn schema() -> QuerySchema {
    QuerySchema::new()
        .with(
            CollectionDecl::new("lesions")
                .costing(10)
                .within(ScopeKey::new().exact("site", "SITE-A"))
                .declare("tumor_volume", BioType::quantity(unit("mm3")))
                .declare("enhancing_volume", BioType::quantity(unit("mm3")))
                .declare("perfusion_volume", BioType::quantity(unit("mL")))
                .declare("longest_diameter", BioType::extent(unit("mm")))
                .declare("dose", BioType::quantity(unit("mg/m2")))
                .declare("dose_per_kg", BioType::quantity(unit("mg/kg")))
                .declare("slice_count", BioType::Number)
                .declare("site", BioType::Text)
                .declare(
                    "centroid_a",
                    BioType::point(unit("mm"), FrameBinding::Unstated),
                )
                .declare(
                    "centroid_b",
                    BioType::point(unit("mm"), FrameBinding::Unstated),
                )
                .declare("centroid_c", BioType::point(unit("mm"), frame("f1", "S1")))
                .declare("centroid_d", BioType::point(unit("mm"), frame("f2", "S2")))
                .declare(
                    "variant_38",
                    BioType::locus(
                        BuildBinding::Declared(GenomeBuild::Grch38),
                        CoordinateConvention::ZeroBasedHalfOpen,
                    ),
                )
                .declare(
                    "variant_37",
                    BioType::locus(
                        BuildBinding::Declared(GenomeBuild::Grch37),
                        CoordinateConvention::ZeroBasedHalfOpen,
                    ),
                )
                .declare(
                    "variant_unstated",
                    BioType::locus(
                        BuildBinding::Unstated,
                        CoordinateConvention::ZeroBasedHalfOpen,
                    ),
                )
                .declare(
                    "variant_vcf",
                    BioType::locus(
                        BuildBinding::Declared(GenomeBuild::Grch38),
                        CoordinateConvention::OneBasedInclusive,
                    ),
                )
                .field(
                    "diagnosis",
                    FieldDecl::new(BioType::Text).of(term("glioblastoma", "MONDO:0018177", "2026-03-01")),
                ),
        )
        .with(
            CollectionDecl::new("states")
                .costing(4)
                .longitudinal()
                .declare("event_time", BioType::instant(Clock::Event))
                .declare("record_time", BioType::instant(Clock::Record))
                .declare("state_label", BioType::Text),
        )
}

fn blocking_dimension(source: &str) -> String {
    let error = compile(source, &schema()).unwrap_err();
    let QueryError::Type(TypeError::Incomparable { dimension, .. }) = error else {
        panic!("expected an incomparability, got {error:?}");
    };
    dimension
}

fn reason(source: &str) -> Incomparability {
    let error = compile(source, &schema()).unwrap_err();
    let QueryError::Type(TypeError::Incomparable { reason, .. }) = error else {
        panic!("expected an incomparability, got {error:?}");
    };
    *reason
}

// --- acceptances, so the rules below are not vacuous -------------------------------------------

#[test]
fn a_fully_declared_query_typechecks_and_reports_its_cost() {
    let typed = compile(
        r#"select tumor_volume from lesions
           in { site: "SITE-A" }
           where tumor_volume > 12.5 mm3 and enhancing_volume < tumor_volume
           labels { "phi:deidentified" }
           cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    assert_eq!(typed.collection, "lesions");
    assert_eq!(typed.cost_estimate, 30, "base 10 times (2 predicates + 1)");
    assert!(typed.labels.contains("phi:deidentified"));
}

#[test]
fn two_quantities_in_the_same_unit_compare() {
    compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           where tumor_volume > enhancing_volume labels {} cost limit 100"#,
        &schema(),
    )
    .expect("mm3 against mm3 is a comparison, not a conversion");
}

#[test]
fn a_star_projection_expands_so_the_query_digest_does_not_depend_on_the_star() {
    let typed = compile(
        r#"select * from states at event labels {} cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    assert_eq!(typed.projection.len(), 3);
    let explicit = compile(
        r#"select event_time, record_time, state_label from states at event labels {} cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    assert_eq!(
        typed.digest().expect("digests"),
        explicit.digest().expect("digests")
    );
}

// --- the incomparability rejections -------------------------------------------------------------

#[test]
fn a_query_comparing_quantities_in_different_units_does_not_typecheck() {
    assert_eq!(
        blocking_dimension(
            r#"select tumor_volume from lesions in { site: "SITE-A" }
               where tumor_volume > 12.5 cm3 labels {} cost limit 100"#
        ),
        "unit identity"
    );
}

#[test]
fn a_query_comparing_across_physical_dimensions_names_the_dimension_not_the_unit() {
    assert!(matches!(
        reason(
            r#"select tumor_volume from lesions in { site: "SITE-A" }
               where tumor_volume > 3 mm labels {} cost limit 100"#
        ),
        Incomparability::DimensionMismatch { .. }
    ));
}

#[test]
fn a_query_comparing_a_measured_quantity_to_a_bare_number_does_not_typecheck() {
    assert_eq!(
        blocking_dimension(
            r#"select tumor_volume from lesions in { site: "SITE-A" }
               where tumor_volume > 12.5 labels {} cost limit 100"#
        ),
        "observable kind",
        "a threshold with no unit is the commonest way a unit error enters a pipeline"
    );
}

#[test]
fn a_query_comparing_across_unstated_frames_does_not_typecheck() {
    let refusal = reason(
        r#"select centroid_a from lesions in { site: "SITE-A" }
           where centroid_a == centroid_b labels {} cost limit 100"#,
    );
    assert!(
        matches!(refusal, Incomparability::UnstatedFrame { .. }),
        "two silences are not a match; got {refusal:?}"
    );
    assert!(refusal.is_silence());
}

#[test]
fn a_query_comparing_two_different_declared_frames_does_not_typecheck() {
    assert!(matches!(
        reason(
            r#"select centroid_c from lesions in { site: "SITE-A" }
               where centroid_c == centroid_d labels {} cost limit 100"#
        ),
        Incomparability::FrameMismatch { .. }
    ));
}

#[test]
fn a_query_comparing_across_a_reference_build_boundary_does_not_typecheck() {
    assert_eq!(
        blocking_dimension(
            r#"select variant_38 from lesions in { site: "SITE-A" }
               where variant_38 == variant_37 labels {} cost limit 100"#
        ),
        "reference build"
    );
}

#[test]
fn a_query_comparing_a_locus_with_no_declared_build_does_not_typecheck() {
    assert_eq!(
        blocking_dimension(
            r#"select variant_38 from lesions in { site: "SITE-A" }
               where variant_38 == variant_unstated labels {} cost limit 100"#
        ),
        "reference build (undeclared)"
    );
}

#[test]
fn a_query_comparing_loci_read_under_different_conventions_does_not_typecheck() {
    assert_eq!(
        blocking_dimension(
            r#"select variant_38 from lesions in { site: "SITE-A" }
               where variant_38 == variant_vcf labels {} cost limit 100"#
        ),
        "coordinate convention"
    );
}

#[test]
fn a_query_comparing_a_point_to_an_extent_is_blocked_on_observable_kind() {
    assert_eq!(
        blocking_dimension(
            r#"select centroid_c from lesions in { site: "SITE-A" }
               where centroid_c == longest_diameter labels {} cost limit 100"#
        ),
        "observable kind"
    );
}

#[test]
fn a_query_whose_expansion_release_disagrees_with_the_field_binding_does_not_typecheck() {
    let refusal = reason(
        r#"select diagnosis from lesions in { site: "SITE-A" }
           expand ontology "MONDO" release "2025-01-01" policy descendants
           labels {} cost limit 100"#,
    );
    assert!(
        matches!(refusal, Incomparability::OntologyVersionDrift { .. }),
        "expanding against a hierarchy the data was not coded in; got {refusal:?}"
    );
}

// --- the declaration rejections -----------------------------------------------------------------

#[test]
fn a_query_with_no_access_labels_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" } cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::AccessLabelsNotDeclared)
    ));
}

#[test]
fn an_empty_label_set_is_a_declaration_and_an_absent_clause_is_not() {
    compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" } labels {} cost limit 100"#,
        &schema(),
    )
    .expect("an explicit empty label set is a declaration");
}

#[test]
fn a_query_with_no_cost_bound_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" } labels {}"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::CostBoundNotDeclared)
    ));
}

#[test]
fn a_query_whose_static_cost_exceeds_its_own_bound_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           where tumor_volume > 1 mm3 and enhancing_volume > 1 mm3 labels {} cost limit 5"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::CostBoundExceeded { estimate, limit }) = error else {
        panic!("expected a cost-bound failure, got {error:?}");
    };
    assert_eq!((estimate, limit), (30, 5));
}

#[test]
fn a_longitudinal_query_that_does_not_say_which_clock_it_means_does_not_typecheck() {
    let error = compile(
        r#"select state_label from states labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::TimeSemanticsNotDeclared { collection }) = error else {
        panic!("expected missing time semantics, got {error:?}");
    };
    assert_eq!(collection, "states");
}

#[test]
fn a_query_reading_an_ontology_bound_field_without_an_expansion_policy_does_not_typecheck() {
    let error = compile(
        r#"select diagnosis from lesions in { site: "SITE-A" } labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::OntologyExpansionNotDeclared { field, ontology, .. }) = error
    else {
        panic!("expected a missing expansion policy, got {error:?}");
    };
    assert_eq!(field, "diagnosis");
    assert_eq!(ontology, "MONDO");
}

#[test]
fn an_aggregate_without_declared_provenance_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           labels {} aggregate mean(tumor_volume) cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::AggregationWithoutProvenance { .. })
    ));
}

#[test]
fn an_aggregate_with_provenance_keeps_the_argument_unit_in_its_result_type() {
    let typed = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           labels {} aggregate mean(tumor_volume) provenance source_lineage cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    let aggregation = typed.aggregations.first().expect("one aggregation");
    assert_eq!(aggregation.result_type, BioType::quantity(unit("mm3")));
}

#[test]
fn an_aggregate_over_a_non_measured_field_does_not_typecheck() {
    let error = compile(
        r#"select site from lesions in { site: "SITE-A" }
           labels {} aggregate mean(site) provenance source_lineage cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::AggregateOverNonMeasured { .. })
    ));
}

#[test]
fn an_unknown_aggregate_function_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           labels {} aggregate variance(tumor_volume) provenance evidence_ids cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::UnknownFunction { .. })
    ));
}

#[test]
fn a_query_whose_scope_does_not_refine_the_collection_scope_names_the_dimension() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-B" } labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::ScopeNotRefining { dimension }) = error else {
        panic!("expected a scope failure, got {error:?}");
    };
    assert_eq!(dimension, "site");
}

#[test]
fn a_query_that_omits_a_bound_scope_dimension_entirely_is_refused_the_same_way() {
    let error = compile(
        r#"select tumor_volume from lesions labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::ScopeNotRefining { .. })
    ));
}

#[test]
fn a_scope_dimension_the_registry_cannot_classify_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A", vibes: "good" }
           labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::UnclassifiedScopeDimension { dimension, .. }) = error else {
        panic!("expected an unclassified dimension, got {error:?}");
    };
    assert_eq!(dimension, "vibes");
}

// --- the clock rule ------------------------------------------------------------------------------

#[test]
fn a_query_ordering_event_time_against_record_time_does_not_typecheck() {
    let error = compile(
        r#"select state_label from states
           where event_time < record_time at event labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::ClockMismatch { left, right, .. }) = error else {
        panic!("expected a clock mismatch, got {error:?}");
    };
    assert_eq!((left, right), (Clock::Event, Clock::Record));
}

#[test]
fn a_query_testing_equality_across_clocks_still_typechecks() {
    compile(
        r#"select state_label from states
           where event_time == record_time at event labels {} cost limit 100"#,
        &schema(),
    )
    .expect("whether a record was filed at the instant of the event is an integrity question");
}

#[test]
fn an_instant_field_orders_against_an_instant_literal() {
    compile(
        r#"select state_label from states
           where event_time > instant "2026-01-01T00:00:00Z" at event labels {} cost limit 100"#,
        &schema(),
    )
    .expect("a literal instant belongs to no clock");
}

// --- ordinary type failures ----------------------------------------------------------------------

#[test]
fn an_unknown_field_is_never_an_untyped_passthrough() {
    let error = compile(
        r#"select does_not_exist from lesions in { site: "SITE-A" } labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::UnknownField { path, declared, .. }) = error else {
        panic!("expected an unknown field, got {error:?}");
    };
    assert_eq!(path, "does_not_exist");
    assert!(declared > 0);
}

#[test]
fn an_unknown_collection_is_reported_with_its_span() {
    let error = compile(
        r#"select a from nowhere labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::UnknownCollection { .. })
    ));
    assert!(error.span().is_some());
}

#[test]
fn a_filter_that_is_not_boolean_does_not_typecheck() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           where tumor_volume labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::NotBoolean { .. })
    ));
}

#[test]
fn a_heterogeneous_set_literal_does_not_typecheck() {
    let error = compile(
        r#"select site from lesions in { site: "SITE-A" }
           where site in {"A", 3} labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QueryError::Type(TypeError::HeterogeneousSet { .. })
    ));
}

#[test]
fn adding_two_quantities_in_different_units_is_refused_like_comparing_them() {
    assert_eq!(
        blocking_dimension(
            r#"select tumor_volume from lesions in { site: "SITE-A" }
               where tumor_volume + perfusion_volume > 1 mm3 labels {} cost limit 100"#
        ),
        "unit identity"
    );
}

#[test]
fn a_ratio_unit_may_not_be_composed_by_multiplication() {
    let error = compile(
        r#"select dose_per_kg from lesions in { site: "SITE-A" }
           where dose_per_kg * dose_per_kg > 1 mm3 labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::UnitComposition { operator, .. }) = error else {
        panic!("expected a composition failure, got {error:?}");
    };
    assert_eq!(operator, "*");
}

#[test]
fn a_clause_level_type_error_carries_no_span_rather_than_a_made_up_one() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" } cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    assert!(
        error.span().is_none(),
        "a missing clause is not at a position, and pointing at one would be a false diagnostic"
    );
}

#[test]
fn every_incomparability_refusal_carries_the_scope_class_it_belongs_to() {
    let error = compile(
        r#"select tumor_volume from lesions in { site: "SITE-A" }
           where tumor_volume > 12.5 cm3 labels {} cost limit 100"#,
        &schema(),
    )
    .unwrap_err();
    let QueryError::Type(TypeError::Incomparable { class, reason, .. }) = error else {
        panic!("expected an incomparability");
    };
    assert_eq!(
        class,
        reason.blocking_class(),
        "the class is derived from the reason, never asserted beside it"
    );
}

#[test]
fn a_typed_query_digest_is_stable_across_scope_value_spellings_that_mean_the_same_thing() {
    let one_of: BTreeSet<String> = ["SITE-A".to_string()].into_iter().collect();
    let expected = ScopeKey::new().bind("site", ScopeValue::OneOf(one_of));
    let typed = compile(
        r#"select tumor_volume from lesions in { site: {"SITE-A"} } labels {} cost limit 100"#,
        &schema(),
    )
    .expect("a singleton set refines an exact binding");
    assert_eq!(typed.scope, expected);
}
