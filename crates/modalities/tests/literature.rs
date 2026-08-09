//! 28.17: a claim from a paper is evidence about a paper until it is bound to a scope.

use bioprism_modalities::{
    cites, supports, BindingRefusal, BoundClaim, ClaimKind, EvaluationHorizon, EvidenceTier,
    LiteratureClaim, Modality, RetractionStatus, SourceProvenance,
};
use bioprism_scope::{ScopeKey, Timestamp};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("a well-formed RFC 3339 instant")
}

fn adult_glioma() -> ScopeKey {
    ScopeKey::new().exact("disease", "adult diffuse glioma")
}

fn adult_glioma_recurrent() -> ScopeKey {
    adult_glioma().exact("phase", "recurrent")
}

fn primary_source() -> SourceProvenance {
    SourceProvenance::new("doi:10.0000/example", EvidenceTier::Primary, at("2020-05-01T00:00:00Z"))
        .studying(adult_glioma())
}

fn primary_claim() -> LiteratureClaim {
    LiteratureClaim::new("the marker was detected in most specimens", primary_source())
}

#[test]
fn a_primary_source_binds_to_a_scope_inside_the_population_it_studied() {
    let bound = primary_claim()
        .bind(
            &adult_glioma_recurrent(),
            EvidenceTier::Primary,
            EvaluationHorizon::open(),
        )
        .expect("the target refines the studied population");
    assert!(bound.is_direct_evidence());
    assert_eq!(bound.source_text(), "the marker was detected in most specimens");
}

#[test]
fn a_review_cannot_be_bound_as_primary_evidence() {
    let review = LiteratureClaim::new(
        "the marker is generally reported as present",
        SourceProvenance::new("doi:10.0000/review", EvidenceTier::Review, at("2021-01-01T00:00:00Z"))
            .studying(adult_glioma()),
    );
    let refusal = review
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .expect_err("a report of a report is not the report");
    assert!(matches!(refusal, BindingRefusal::CitationLaundering { .. }));
}

#[test]
fn a_review_binds_perfectly_well_as_a_review() {
    let review = LiteratureClaim::new(
        "the marker is generally reported as present",
        SourceProvenance::new("doi:10.0000/review", EvidenceTier::Review, at("2021-01-01T00:00:00Z"))
            .studying(adult_glioma()),
    );
    let bound = review
        .bind(&adult_glioma(), EvidenceTier::Review, EvaluationHorizon::open())
        .expect("a review is real evidence about the state of a field");
    assert!(!bound.is_direct_evidence());
    assert_eq!(bound.cited_as(), EvidenceTier::Review);
    assert_eq!(
        cites(&bound, ClaimKind::PublishedClaimSupport)
            .expect("a bound review may still be cited"),
        EvidenceTier::Review,
        "the citation must carry the tier it was bound at, not the one it was asked for"
    );
}

#[test]
fn a_database_record_is_not_a_primary_source() {
    let record = LiteratureClaim::new(
        "the curated entry lists the association",
        SourceProvenance::new("db:entry-1", EvidenceTier::Database, at("2022-01-01T00:00:00Z"))
            .studying(adult_glioma()),
    );
    assert!(record
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .is_err());
}

#[test]
fn a_source_published_after_the_horizon_is_refused() {
    let refusal = primary_claim()
        .bind(
            &adult_glioma(),
            EvidenceTier::Primary,
            EvaluationHorizon::as_of(at("2019-01-01T00:00:00Z")),
        )
        .expect_err("the source postdates the task's knowledge boundary");
    assert!(matches!(refusal, BindingRefusal::TemporalLeakage { .. }));
}

#[test]
fn a_source_published_on_the_horizon_is_admitted() {
    assert!(primary_claim()
        .bind(
            &adult_glioma(),
            EvidenceTier::Primary,
            EvaluationHorizon::as_of(at("2020-05-01T00:00:00Z")),
        )
        .is_ok());
}

#[test]
fn a_source_with_no_stated_population_is_refused() {
    let unstated = LiteratureClaim::new(
        "the marker was detected",
        SourceProvenance::new("doi:10.0000/unstated", EvidenceTier::Primary, at("2020-05-01T00:00:00Z")),
    );
    let refusal = unstated
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .expect_err("without a stated population the mismatch check is unfalsifiable");
    assert!(matches!(refusal, BindingRefusal::UnstatedPopulation { .. }));
}

#[test]
fn a_target_scope_outside_the_studied_population_is_refused() {
    let paediatric = ScopeKey::new().exact("disease", "paediatric high-grade glioma");
    let refusal = primary_claim()
        .bind(&paediatric, EvidenceTier::Primary, EvaluationHorizon::open())
        .expect_err("a different disease is not a narrower scope");
    assert!(matches!(refusal, BindingRefusal::PopulationMismatch { .. }));
}

#[test]
fn a_broader_target_scope_is_refused_as_firmly_as_a_disjoint_one() {
    let everything = ScopeKey::new();
    assert!(
        matches!(
            primary_claim()
                .bind(&everything, EvidenceTier::Primary, EvaluationHorizon::open())
                .expect_err("an unconstrained target is wider than any studied population"),
            BindingRefusal::PopulationMismatch { .. }
        ),
        "the check runs target-refines-population, so a target that constrains nothing fails it"
    );

    let narrower_source = LiteratureClaim::new(
        "the marker was detected",
        SourceProvenance::new("doi:10.0000/narrow", EvidenceTier::Primary, at("2020-05-01T00:00:00Z"))
            .studying(adult_glioma_recurrent()),
    );
    assert!(matches!(
        narrower_source
            .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
            .expect_err("a study of recurrent disease does not cover all of it"),
        BindingRefusal::PopulationMismatch { .. }
    ));
}

#[test]
fn a_retracted_source_is_refused_without_a_warrant() {
    let retracted = LiteratureClaim::new(
        "the marker was detected",
        primary_source().flagged(RetractionStatus::Retracted),
    );
    let refusal = retracted
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .expect_err("a retracted source needs an explicit warrant");
    assert!(matches!(refusal, BindingRefusal::RetractedSource { .. }));
}

#[test]
fn a_flagged_source_binds_with_a_warrant_that_travels_with_the_claim() {
    let flagged = LiteratureClaim::new(
        "the marker was detected",
        primary_source().flagged(RetractionStatus::ExpressionOfConcern),
    );
    let bound = flagged
        .bind_despite_flag(
            &adult_glioma(),
            EvidenceTier::Primary,
            EvaluationHorizon::open(),
            "cited in a history of the field, not for its result",
        )
        .expect("a warrant makes the citation representable");
    assert_eq!(
        bound.flag_warrant(),
        Some("cited in a history of the field, not for its result")
    );
}

#[test]
fn a_warrant_does_not_excuse_the_other_three_checks() {
    let flagged = LiteratureClaim::new(
        "the marker was detected",
        primary_source().flagged(RetractionStatus::Retracted),
    );
    assert!(matches!(
        flagged
            .bind_despite_flag(
                &adult_glioma(),
                EvidenceTier::Primary,
                EvaluationHorizon::as_of(at("2019-01-01T00:00:00Z")),
                "a stated warrant",
            )
            .expect_err("the horizon still applies"),
        BindingRefusal::TemporalLeakage { .. }
    ));
}

#[test]
fn a_bound_claim_still_cannot_support_a_biological_claim() {
    let bound = primary_claim()
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .expect("the binding succeeds");
    for kind in ClaimKind::ALL {
        let outcome = cites(&bound, kind);
        if kind == ClaimKind::PublishedClaimSupport {
            assert_eq!(
                outcome.expect("a bound claim supports a claim about the paper"),
                EvidenceTier::Primary
            );
        } else {
            assert!(
                outcome.is_err(),
                "binding a claim to a scope turned it into a {kind} measurement"
            );
        }
    }
}

#[test]
fn the_literature_descriptor_resolves_no_axis_at_all() {
    let literature = bioprism_modalities::descriptor(Modality::Literature);
    assert!(literature.resolved_axes().is_empty());
    assert!(literature.is_complete());
    assert!(supports(Modality::Literature, ClaimKind::PopulationAverage).is_err());
}

#[test]
fn a_bound_claim_serialises_with_its_provenance_intact() {
    let bound: BoundClaim = primary_claim()
        .bind(&adult_glioma(), EvidenceTier::Primary, EvaluationHorizon::open())
        .expect("the binding succeeds");
    let text = serde_json::to_string(&bound).expect("BoundClaim is Serialize");
    assert!(text.contains("doi:10.0000/example"));
    assert!(text.contains("adult diffuse glioma"));
}

#[test]
fn an_unbound_claim_round_trips_because_reading_a_paper_is_not_privileged() {
    let claim = primary_claim();
    let text = serde_json::to_string(&claim).expect("LiteratureClaim is Serialize");
    let parsed: LiteratureClaim = serde_json::from_str(&text).expect("and Deserialize");
    assert_eq!(parsed, claim);
}

#[test]
fn the_horizon_has_no_default_so_silence_cannot_become_open() {
    let open = EvaluationHorizon::open();
    let bounded = EvaluationHorizon::as_of(at("2020-01-01T00:00:00Z"));
    assert!(open.admits(at("2030-01-01T00:00:00Z")));
    assert!(!bounded.admits(at("2030-01-01T00:00:00Z")));
    assert_eq!(open.describe(), "open");
}
