//! Invariants of blueprint 25.05, AssayLens.

use bioprism_bioir::{
    AssayLens, Calibration, CalibrationKind, ComparabilityRule, ErrorModel, Identifiability,
    Incomparability, LensCatalog, LensError, LensId, LineageGraph, MaterialRequirement, Measurement,
    MeasurementScale, MeasurementTarget, MissingnessClass, ProcessKind, ProcessingStep,
    ProtocolChain, QcContract, QcMetric, QcOutcome, Quantity, Reading, Specimen, SpecimenId,
    SubjectId,
};
use bioprism_scope::Timestamp;
use std::collections::BTreeSet;

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("well-formed timestamp")
}

fn lens_id(text: &str) -> LensId {
    LensId::parse(text).expect("well-formed lens id")
}

fn sid(text: &str) -> SpecimenId {
    SpecimenId::parse(text).expect("well-formed specimen id")
}

fn subject(text: &str) -> SubjectId {
    SubjectId::parse(text).expect("well-formed subject id")
}

/// A relative bulk expression lens: normalised counts, no external standard.
fn expression_lens() -> AssayLens {
    AssayLens {
        id: lens_id("bulk-rnaseq-tpm"),
        version: "1.0.0".to_string(),
        target: MeasurementTarget {
            quantity: "gene expression".to_string(),
            entity: "transcript".to_string(),
            unit: "TPM".to_string(),
            scale: MeasurementScale::Ratio,
            identifiability: Identifiability::SemiQuantitative,
        },
        material: MaterialRequirement {
            material: "fresh tissue".to_string(),
            minimum: Quantity::new(2.0, "mg"),
            destructive: true,
        },
        protocol: ProtocolChain {
            instrument: "NovaSeq X".to_string(),
            protocol: "poly-A selection, 150bp paired end".to_string(),
            protocol_version: "3".to_string(),
            steps: vec![
                ProcessingStep::new("aligner", "STAR-2.7.10a"),
                ProcessingStep::new("quantifier", "salmon-1.10.1"),
            ],
        },
        calibration: Calibration {
            kind: CalibrationKind::Relative {
                anchor: "library size".to_string(),
            },
            calibrated_at: Some(ts("2026-01-15T00:00:00Z")),
            limit_of_detection: Some(0.5),
            limit_of_quantification: Some(1.0),
        },
        error_model: ErrorModel {
            form: "negative binomial".to_string(),
            noise_sd: None,
            missingness: MissingnessClass::BelowDetection,
            known_artifacts: vec!["index hopping on patterned flow cells".to_string()],
        },
        comparability: ComparabilityRule {
            requires_same_lens: true,
            requires_same_processing_versions: true,
            requires_same_batch: true,
            requires_same_site: false,
            bridging_study: None,
        },
        qc: QcContract {
            metrics: vec![QcMetric {
                name: "RIN".to_string(),
                minimum: Some(6.0),
                maximum: None,
            }],
        },
        known_failure_modes: BTreeSet::new(),
    }
}

fn measurement(lens: &AssayLens, specimen: &str, value: f64) -> Measurement {
    Measurement::new(
        lens.id.clone(),
        lens.version.clone(),
        sid(specimen),
        Reading::Quantity { value },
        lens.target.unit.clone(),
        ts("2026-02-01T00:00:00Z"),
    )
}

#[test]
fn a_lens_cannot_claim_absolute_quantity_on_an_uncalibrated_pipeline() {
    let mut lens = expression_lens();
    lens.target.identifiability = Identifiability::AbsoluteQuantity;
    assert_eq!(
        lens.validate(),
        Err(LensError::UncalibratedAbsoluteClaim {
            lens: "bulk-rnaseq-tpm".to_string(),
            claimed: "absolute".to_string(),
            calibration: "relative to library size".to_string(),
        })
    );

    lens.calibration.kind = CalibrationKind::AbsoluteAgainstStandard {
        standard: "ERCC spike-in mix 1".to_string(),
    };
    assert_eq!(
        lens.validate(),
        Ok(()),
        "the claim becomes legitimate once the calibration supports it"
    );
}

#[test]
fn a_lens_may_claim_less_identifiability_than_its_calibration_supports() {
    let mut lens = expression_lens();
    lens.target.identifiability = Identifiability::Relative;
    assert_eq!(lens.validate(), Ok(()));
}

#[test]
fn changing_a_processing_step_version_changes_the_lens_identity_hash() {
    let lens = expression_lens();
    let mut upgraded = expression_lens();
    upgraded.protocol.steps[0] = ProcessingStep::new("aligner", "STAR-2.7.11b");

    assert_ne!(
        lens.identity_hash().expect("hashes"),
        upgraded.identity_hash().expect("hashes"),
        "processing versions are part of lens identity"
    );
}

#[test]
fn documenting_a_new_failure_mode_does_not_change_the_lens_identity_hash() {
    let lens = expression_lens();
    let mut annotated = expression_lens();
    annotated
        .known_failure_modes
        .insert("3' bias on degraded RNA".to_string());

    assert_eq!(
        lens.identity_hash().expect("hashes"),
        annotated.identity_hash().expect("hashes"),
        "documentation is not a different measuring device"
    );
}

#[test]
fn comparing_across_processing_versions_names_the_step_that_changed() {
    let lens = expression_lens();
    let mut upgraded = expression_lens();
    upgraded.version = "1.1.0".to_string();
    upgraded.protocol.steps[0] = ProcessingStep::new("aligner", "STAR-2.7.11b");

    assert_eq!(
        lens.comparable_with(&upgraded),
        Err(Incomparability::ProcessingVersionChanged {
            step: "aligner".to_string(),
            left: "STAR-2.7.10a".to_string(),
            right: "STAR-2.7.11b".to_string(),
        })
    );
}

#[test]
fn a_step_present_on_only_one_lens_is_reported_as_absent_not_ignored() {
    let lens = expression_lens();
    let mut trimmed = expression_lens();
    trimmed.protocol.steps.push(ProcessingStep::new("filter", "1.2.0"));

    assert_eq!(
        lens.comparable_with(&trimmed),
        Err(Incomparability::ProcessingVersionChanged {
            step: "filter".to_string(),
            left: "absent".to_string(),
            right: "1.2.0".to_string(),
        })
    );
}

#[test]
fn two_lenses_measuring_different_quantities_report_the_target_difference_first() {
    let lens = expression_lens();
    let mut other = expression_lens();
    other.target.quantity = "protein abundance".to_string();
    other.target.unit = "ng/mL".to_string();
    other.protocol.steps.clear();

    assert_eq!(
        lens.comparable_with(&other),
        Err(Incomparability::DifferentTarget {
            left: "gene expression".to_string(),
            right: "protein abundance".to_string(),
        }),
        "a batch or version reason would bury the real problem"
    );
}

#[test]
fn measurements_in_different_batches_are_incomparable_when_the_lens_says_batch_matters() {
    let lens = expression_lens();
    let mut catalog = LensCatalog::new();
    catalog.register(lens.clone()).expect("registers");

    let left = measurement(&lens, "blk-1.s1", 12.0).in_batch("run-a");
    let right = measurement(&lens, "blk-1.s2", 14.0).in_batch("run-b");
    assert_eq!(
        catalog.comparable_with(&left, &right),
        Err(Incomparability::UncontrolledBatch {
            left: "run-a".to_string(),
            right: "run-b".to_string(),
        })
    );

    let same_batch = measurement(&lens, "blk-1.s2", 14.0).in_batch("run-a");
    assert_eq!(catalog.comparable_with(&left, &same_batch), Ok(()));
}

#[test]
fn an_unrecorded_batch_is_named_as_unrecorded_rather_than_matched() {
    let lens = expression_lens();
    let mut catalog = LensCatalog::new();
    catalog.register(lens.clone()).expect("registers");

    let left = measurement(&lens, "blk-1.s1", 12.0).in_batch("run-a");
    let right = measurement(&lens, "blk-1.s2", 14.0);
    assert_eq!(
        catalog.comparable_with(&left, &right),
        Err(Incomparability::UncontrolledBatch {
            left: "run-a".to_string(),
            right: "unrecorded".to_string(),
        })
    );
}

#[test]
fn a_measurement_that_failed_qc_is_incomparable_and_says_which_metric_failed() {
    let lens = expression_lens();
    let mut catalog = LensCatalog::new();
    catalog.register(lens.clone()).expect("registers");

    let good = measurement(&lens, "blk-1.s1", 12.0).in_batch("run-a");
    let bad = measurement(&lens, "blk-1.s2", 14.0)
        .in_batch("run-a")
        .with_qc(QcOutcome::Fail {
            metric: "RIN".to_string(),
            value: "3.1".to_string(),
        });

    let reason = catalog
        .comparable_with(&good, &bad)
        .expect_err("a failed measurement is not a value");
    assert!(matches!(reason, Incomparability::QualityGateFailed { .. }));
    assert!(reason.to_string().contains("RIN"));
}

#[test]
fn an_ungradable_measurement_is_not_the_same_as_a_failed_one() {
    let lens = expression_lens();
    let mut catalog = LensCatalog::new();
    catalog.register(lens.clone()).expect("registers");

    let good = measurement(&lens, "blk-1.s1", 12.0).in_batch("run-a");
    let ungradable = measurement(&lens, "blk-1.s2", 14.0)
        .in_batch("run-a")
        .with_qc(QcOutcome::Ungradable {
            reason: "slide could not be scanned".to_string(),
        });

    let reason = catalog
        .comparable_with(&good, &ungradable)
        .expect_err("an unread measurement is not a value");
    assert!(reason.to_string().contains("could not be scanned"));
}

#[test]
fn a_negative_call_without_a_limit_of_detection_is_refused() {
    let mut lens = expression_lens();
    lens.calibration.limit_of_detection = None;
    let negative = Measurement::new(
        lens.id.clone(),
        lens.version.clone(),
        sid("blk-1.s1"),
        Reading::Absent,
        lens.target.unit.clone(),
        ts("2026-02-01T00:00:00Z"),
    );

    assert_eq!(
        lens.check_reading(&negative),
        Err(LensError::NegativeWithoutSensitivity {
            lens: "bulk-rnaseq-tpm".to_string()
        })
    );
}

#[test]
fn a_negative_call_with_a_limit_of_detection_is_a_real_claim() {
    let lens = expression_lens();
    let negative = Measurement::new(
        lens.id.clone(),
        lens.version.clone(),
        sid("blk-1.s1"),
        Reading::BelowLimitOfDetection,
        lens.target.unit.clone(),
        ts("2026-02-01T00:00:00Z"),
    );
    assert_eq!(lens.check_reading(&negative), Ok(()));
    assert!(Reading::BelowLimitOfDetection.is_negative_call());
    assert!(!Reading::Quantity { value: 0.0 }.is_negative_call());
}

#[test]
fn a_lens_requiring_more_material_than_remains_is_refused() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("biopsy-1"),
            subject("pt-1"),
            ts("2026-01-01T00:00:00Z"),
            "tumour core",
            "fresh tissue",
            Quantity::new(3.0, "mg"),
        ))
        .expect("root inserts");
    let lens = expression_lens();
    assert_eq!(lens.admits(&graph, &sid("biopsy-1")), Ok(()));

    graph
        .insert(Specimen::derived(
            sid("biopsy-1.a1"),
            sid("biopsy-1"),
            ProcessKind::Aliquot,
            ts("2026-01-02T00:00:00Z"),
            "fresh tissue",
            Quantity::new(2.5, "mg"),
        ))
        .expect("aliquot inserts");

    assert_eq!(
        lens.admits(&graph, &sid("biopsy-1")),
        Err(LensError::InsufficientMaterial {
            lens: "bulk-rnaseq-tpm".to_string(),
            specimen: "biopsy-1".to_string(),
            required: "2 mg".to_string(),
            available: "0.5 mg".to_string(),
        }),
        "the collection record is not what is left in the tube"
    );
}

#[test]
fn a_lens_refuses_material_of_the_wrong_type() {
    let mut graph = LineageGraph::new();
    graph
        .insert(Specimen::collected(
            sid("tube-1"),
            subject("pt-1"),
            ts("2026-01-01T00:00:00Z"),
            "antecubital vein",
            "plasma".to_string(),
            Quantity::new(10.0, "mg"),
        ))
        .expect("root inserts");

    assert!(matches!(
        expression_lens().admits(&graph, &sid("tube-1")),
        Err(LensError::WrongMaterial { .. })
    ));
}

#[test]
fn registering_the_same_lens_version_twice_is_refused() {
    let mut catalog = LensCatalog::new();
    catalog.register(expression_lens()).expect("registers");
    assert_eq!(
        catalog.register(expression_lens()),
        Err(LensError::DuplicateLens {
            lens: "bulk-rnaseq-tpm".to_string(),
            version: "1.0.0".to_string(),
        })
    );

    let mut next = expression_lens();
    next.version = "1.1.0".to_string();
    catalog.register(next).expect("a new version coexists");
    assert_eq!(catalog.len(), 2);
}

#[test]
fn a_bridging_study_is_named_as_the_missing_work_not_treated_as_a_pass() {
    let mut lens = expression_lens();
    lens.comparability.bridging_study = Some("GTEx-to-registry harmonisation 2025".to_string());
    let mut other = expression_lens();
    other.id = lens_id("nanostring-panel");

    assert_eq!(
        lens.comparable_with(&other),
        Err(Incomparability::BridgingStudyRequired {
            study: "GTEx-to-registry harmonisation 2025".to_string(),
        })
    );
}

#[test]
fn a_measurement_naming_an_unregistered_lens_version_is_incomparable() {
    let lens = expression_lens();
    let mut catalog = LensCatalog::new();
    catalog.register(lens.clone()).expect("registers");

    let left = measurement(&lens, "blk-1.s1", 12.0).in_batch("run-a");
    let mut stranger = measurement(&lens, "blk-1.s2", 14.0).in_batch("run-a");
    stranger.lens_version = "9.9.9".to_string();

    assert_eq!(
        catalog.comparable_with(&left, &stranger),
        Err(Incomparability::UnknownLens {
            lens: "bulk-rnaseq-tpm".to_string(),
            version: "9.9.9".to_string(),
        })
    );
    assert!(catalog.get(&lens_id("bulk-rnaseq-tpm"), "9.9.9").is_err());
}

#[test]
fn a_qc_metric_with_an_empty_acceptance_band_is_refused_at_registration() {
    let mut lens = expression_lens();
    lens.qc.metrics[0].minimum = Some(9.0);
    lens.qc.metrics[0].maximum = Some(6.0);

    let mut catalog = LensCatalog::new();
    assert_eq!(
        catalog.register(lens),
        Err(LensError::EmptyQcBand {
            lens: "bulk-rnaseq-tpm".to_string(),
            metric: "RIN".to_string(),
        })
    );
}
