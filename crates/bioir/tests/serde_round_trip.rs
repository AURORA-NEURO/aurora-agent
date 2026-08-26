//! Every public IR object survives a JSON round trip.
//!
//! Blueprint 25.04, 25.05, 25.11, 25.12 and 25.13 all require the object to "receive a
//! canonical serialization and content hash" and to be reconstructible by a second adapter.
//! That is not possible if a typed field is lost on the way out, and a lost field is invisible
//! in Rust-only tests — nothing errors, the value is simply gone.

use bioprism_bioir::{
    AssayLens, Calibration, CalibrationKind, CohortId, ComparabilityRule, ConsumptionEvent,
    ErrorModel, EvidenceId, Identifiability, IdentityAssertion, IdentityConfidence, LensId,
    LineageGraph, MaterialRequirement, MeasurementScale, MeasurementTarget, MissingnessClass,
    ObservationId, Origin, Predicate, ProcessKind, ProcessingStep, ProtocolChain, QcContract,
    QcMetric, Quantity, Representation, ReviewerAssessment, ReviewerDistribution, Specimen,
    SpecimenId, SplitPlan, SplitUnit, SubjectId, UncertaintyBudget, UncertaintyComponent,
    UncertaintyKind,
};
use bioprism_ids::IdError;
use bioprism_scope::{Interval, Timestamp};
use std::collections::BTreeSet;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("well-formed timestamp")
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let text = serde_json::to_string(value).expect("serialises");
    serde_json::from_str(&text).expect("deserialises")
}

#[test]
fn a_lineage_graph_round_trips_with_its_origins_identities_and_consumption() {
    let mut graph = LineageGraph::new();
    let mut conflicting = BTreeSet::new();
    conflicting.insert(SubjectId::parse("pt-1").expect("subject id"));
    conflicting.insert(SubjectId::parse("pt-9").expect("subject id"));

    graph
        .insert(
            Specimen::collected(
                SpecimenId::parse("blk-1").expect("specimen id"),
                SubjectId::parse("pt-1").expect("subject id"),
                ts("2026-01-01T00:00:00Z"),
                "left temporal lobe",
                "FFPE block",
                Quantity::new(10.0, "mL"),
            )
            .with_consent(["research-use"])
            .with_consumption(ConsumptionEvent {
                consumed_at: ts("2026-03-01T00:00:00Z"),
                amount: Some(Quantity::new(1.0, "mL")),
                reason: "diagnostic section".to_string(),
            }),
        )
        .expect("root inserts");
    graph
        .insert(
            Specimen::derived(
                SpecimenId::parse("blk-1.s1").expect("specimen id"),
                SpecimenId::parse("blk-1").expect("specimen id"),
                ProcessKind::Extraction {
                    analyte: "genomic DNA".to_string(),
                },
                ts("2026-01-02T00:00:00Z"),
                "FFPE block",
                Quantity::new(2.0, "mL"),
            )
            .with_identity(IdentityAssertion {
                asserted_subject: SubjectId::parse("pt-1").expect("subject id"),
                confidence: IdentityConfidence::Disputed { conflicting },
                evidence: vec!["STR concordance 0.61".to_string()],
            }),
        )
        .expect("section inserts");

    let restored = round_trip(&graph);
    assert_eq!(restored, graph);
    assert!(matches!(
        restored
            .get(&SpecimenId::parse("blk-1.s1").expect("specimen id"))
            .expect("present")
            .origin,
        Origin::Derived { .. }
    ));
}

#[test]
fn a_nested_eligibility_predicate_round_trips() {
    let predicate = Predicate::All {
        of: vec![
            Predicate::AttributeAtLeast {
                key: "age".to_string(),
                threshold: 18.0,
            },
            Predicate::Not {
                inner: Box::new(Predicate::Any {
                    of: vec![
                        Predicate::AttributeEquals {
                            key: "prior_radiation".to_string(),
                            value: serde_json::json!(true),
                        },
                        Predicate::AttributePresent {
                            key: "withdrawn_at".to_string(),
                        },
                    ],
                }),
            },
            Predicate::IndexDateWithin {
                window: Interval {
                    start: Some(ts("2026-01-01T00:00:00Z")),
                    end: Some(ts("2027-01-01T00:00:00Z")),
                },
            },
        ],
    };
    assert_eq!(round_trip(&predicate), predicate);
}

#[test]
fn a_split_plan_round_trips_with_its_chronological_boundary() {
    let plan = SplitPlan::new(SplitUnit::Attribute {
        key: "scanner".to_string(),
    })
    .assign(
        ObservationId::parse("obs-1").expect("observation id"),
        "train",
    )
    .assign(
        ObservationId::parse("obs-2").expect("observation id"),
        "test",
    )
    .with_boundary(bioprism_bioir::ChronologicalBoundary {
        earlier: bioprism_bioir::Fold::new("train"),
        later: bioprism_bioir::Fold::new("test"),
        at: ts("2026-06-01T00:00:00Z"),
    });
    assert_eq!(round_trip(&plan), plan);
}

#[test]
fn an_assay_lens_round_trips_and_keeps_its_identity_hash() {
    let lens = AssayLens {
        id: LensId::parse("bulk-rnaseq-tpm").expect("lens id"),
        version: "1.0.0".to_string(),
        target: MeasurementTarget {
            quantity: "gene expression".to_string(),
            entity: "transcript".to_string(),
            unit: "TPM".to_string(),
            scale: MeasurementScale::Ratio,
            identifiability: Identifiability::AbsoluteQuantity,
        },
        material: MaterialRequirement {
            material: "fresh tissue".to_string(),
            minimum: Quantity::new(2.0, "mg"),
            destructive: true,
        },
        protocol: ProtocolChain {
            instrument: "NovaSeq X".to_string(),
            protocol: "poly-A".to_string(),
            protocol_version: "3".to_string(),
            steps: vec![ProcessingStep::new("aligner", "STAR-2.7.10a")],
        },
        calibration: Calibration {
            kind: CalibrationKind::AbsoluteAgainstStandard {
                standard: "ERCC spike-in mix 1".to_string(),
            },
            calibrated_at: Some(ts("2026-01-15T00:00:00Z")),
            limit_of_detection: Some(0.5),
            limit_of_quantification: Some(1.0),
        },
        error_model: ErrorModel {
            form: "negative binomial".to_string(),
            noise_sd: Some(0.12),
            missingness: MissingnessClass::MissingNotAtRandom,
            known_artifacts: vec!["index hopping".to_string()],
        },
        comparability: ComparabilityRule {
            requires_same_lens: true,
            requires_same_processing_versions: true,
            requires_same_batch: true,
            requires_same_site: true,
            bridging_study: Some("harmonisation-2025".to_string()),
        },
        qc: QcContract {
            metrics: vec![QcMetric {
                name: "RIN".to_string(),
                minimum: Some(6.0),
                maximum: None,
            }],
        },
        known_failure_modes: ["3' bias on degraded RNA".to_string()]
            .into_iter()
            .collect(),
    };

    let restored = round_trip(&lens);
    assert_eq!(restored, lens);
    assert_eq!(
        restored.identity_hash().expect("hashes"),
        lens.identity_hash().expect("hashes"),
        "a round trip must not change what the lens is"
    );
}

#[test]
fn an_uncertainty_budget_round_trips_with_every_kind_kept_apart() {
    let mut budget = UncertaintyBudget::new();
    budget
        .declare(
            "claim-1",
            UncertaintyComponent::new(
                UncertaintyKind::Aleatoric,
                Representation::Interval {
                    lower: 0.3,
                    upper: 0.5,
                    coverage: 0.95,
                },
                "12-month overall survival",
            )
            .because("bootstrap over the outcome distribution"),
        )
        .expect("declares");
    budget
        .declare(
            "claim-1",
            UncertaintyComponent::new(
                UncertaintyKind::Expert,
                Representation::Panel {
                    distribution: ReviewerDistribution::new(vec![
                        ReviewerAssessment::new("reader-1", "progression", true),
                        ReviewerAssessment::new("reader-2", "pseudoprogression", true),
                    ]),
                },
                "response assessment",
            ),
        )
        .expect("declares");
    budget
        .declare(
            "claim-1",
            UncertaintyComponent::new(
                UncertaintyKind::DistributionShift,
                Representation::ShiftDiagnostic {
                    statistic: "energy distance".to_string(),
                    value: 0.41,
                    reference_cohort: "training-2024".to_string(),
                },
                "feature distribution",
            ),
        )
        .expect("declares");

    let restored = round_trip(&budget);
    assert_eq!(restored, budget);
    assert_eq!(restored.kinds().len(), 3);
    assert!(restored.component(UncertaintyKind::Epistemic).is_none());
}

/// Asserts the six observable properties of one `bioprism_ids::validated_string_id!` expansion.
///
/// The macro is shared with `bioprism-ids` and `bioprism-hub` and the wire form it produces is
/// published surface in all three, so a change to the expansion is a wire-format change in three
/// crates at once. `bioprism-ids` asserts these bytes for its own identifiers; without the same
/// assertion here, a divergence introduced in the expansion would be caught for `WorldId` and
/// missed for `SpecimenId`.
///
/// `round_trip` above cannot stand in for this. Rewriting both halves of
/// `#[serde(try_from, into)]` to an object form still round-trips cleanly; only a literal
/// assertion on the emitted bytes pins the shape.
macro_rules! assert_shared_identifier_contract {
    ($ty:ty, $kind:literal) => {{
        let id = <$ty>::parse("sample-1").expect("well-formed identifier");
        assert_eq!(id.as_str(), "sample-1");
        assert_eq!(id.to_string(), "sample-1");
        assert_eq!(
            serde_json::to_string(&id).expect("identifier serialises"),
            "\"sample-1\""
        );
        let decoded: $ty = serde_json::from_str("\"sample-1\"").expect("identifier deserialises");
        assert_eq!(decoded, id);
        assert_eq!(String::from(id), "sample-1");

        assert_eq!(<$ty>::KIND, $kind);
        assert_eq!(<$ty>::parse(""), Err(IdError::Empty { kind: $kind }));
        assert_eq!(
            <$ty>::parse("a\u{7}b"),
            Err(IdError::ControlCharacter {
                kind: $kind,
                value: "a\u{7}b".to_string(),
            })
        );
    }};
}

#[test]
fn every_biological_identifier_serialises_as_a_bare_json_string_and_names_its_own_kind() {
    assert_shared_identifier_contract!(SubjectId, "subject");
    assert_shared_identifier_contract!(SpecimenId, "specimen");
    assert_shared_identifier_contract!(LensId, "lens");
    assert_shared_identifier_contract!(CohortId, "cohort");
    assert_shared_identifier_contract!(ObservationId, "observation");
    assert_shared_identifier_contract!(EvidenceId, "evidence");
}
