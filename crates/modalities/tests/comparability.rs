//! Comparability once the modality dimension is added to the standards layer's checks.

use bioprism_modalities::{
    comparable_across, descriptor, report, CrossModalIncomparability, Modality, ModalMeasurement,
    Resolution, ResolutionStatus,
};
use bioprism_scope::ScopeClass;
use bioprism_standards::{
    ComparabilityPolicy, Incomparability, Measurement, OntologyId, Quantity, TermBinding, Unit,
};

fn fraction(label: &str, value: f64) -> Measurement {
    Measurement::scalar(
        label,
        Quantity::new(value, Unit::parse("1").expect("the plain fraction is in the table")),
    )
}

fn tp53() -> TermBinding {
    TermBinding::exact(
        "TP53",
        OntologyId::parse("HGNC:11998", "2026-01").expect("a well-formed CURIE with a release"),
    )
    .expect("a non-empty local term")
}

fn rna_measurement() -> ModalMeasurement {
    ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        fraction("TP53 by RNA-seq", 0.4).of(tp53()),
    )
}

fn protein_measurement() -> ModalMeasurement {
    ModalMeasurement::new(
        descriptor(Modality::Proteomics),
        Resolution::Population,
        fraction("TP53 by mass spectrometry", 0.4).of(tp53()),
    )
}

#[test]
fn rna_and_protein_measurements_of_the_same_gene_are_not_comparable() {
    let refusal = comparable_across(&rna_measurement(), &protein_measurement())
        .expect_err("transcript abundance and protein abundance are different quantities");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::MeasurandMismatch { .. }
    ));
}

#[test]
fn the_rna_protein_refusal_cites_the_blueprint_module_that_names_it() {
    let refusal = comparable_across(&rna_measurement(), &protein_measurement())
        .expect_err("different quantities");
    let text = refusal.to_string();
    assert!(text.contains("28.06"), "expected a module citation in {text:?}");
    assert!(text.contains("RNA-protein equivalence"));
}

#[test]
fn the_standards_layer_agrees_on_everything_the_modality_layer_blocks() {
    let rna = rna_measurement();
    let protein = protein_measurement();
    assert!(
        bioprism_standards::comparable(&rna.measurement, &protein.measurement).is_ok(),
        "the standards layer has no view on measurand; if it blocked here the test would prove \
         nothing about this crate"
    );
    assert!(comparable_across(&rna, &protein).is_err());
}

#[test]
fn two_measurements_of_the_same_quantity_by_the_same_modality_are_comparable() {
    let left = rna_measurement();
    let right = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        fraction("TP53 in the second cohort", 0.6).of(tp53()),
    );
    assert!(comparable_across(&left, &right).is_ok());
}

#[test]
fn a_population_value_and_a_cell_value_are_not_comparable_without_an_aggregation() {
    let bulk = rna_measurement();
    let per_cell = ModalMeasurement::new(
        descriptor(Modality::SingleCell),
        Resolution::Cell,
        fraction("TP53 in one cell", 0.4).of(tp53()),
    );
    let refusal = comparable_across(&bulk, &per_cell)
        .expect_err("an average and a per-cell value are not two estimates of one number");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::ResolutionMismatch { .. }
    ));
}

#[test]
fn a_single_cell_pseudobulk_value_is_comparable_with_a_bulk_value() {
    let bulk = rna_measurement();
    let pseudobulk = ModalMeasurement::new(
        descriptor(Modality::SingleCell),
        Resolution::Population,
        fraction("TP53 pseudobulk", 0.4).of(tp53()),
    );
    assert!(comparable_across(&bulk, &pseudobulk).is_ok());
}

#[test]
fn an_imputed_axis_is_not_comparable_against_a_measured_one() {
    let deconvolved = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics).with_status(
            Resolution::Cell,
            ResolutionStatus::Imputed {
                source: Modality::BulkTranscriptomics,
                by: "deconvolution against a signature matrix".to_string(),
            },
        ),
        Resolution::Cell,
        fraction("estimated malignant fraction", 0.4),
    );
    let measured = ModalMeasurement::new(
        descriptor(Modality::SingleCell),
        Resolution::Cell,
        fraction("counted malignant fraction", 0.4),
    );

    let refusal = comparable_across(&deconvolved, &measured)
        .expect_err("an estimate read against an observation");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::ImputedAgainstMeasured { .. }
    ));
    assert!(!refusal.is_silence());
}

#[test]
fn reporting_at_an_axis_the_modality_does_not_resolve_is_refused() {
    let malformed = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics),
        Resolution::Cell,
        fraction("a per-cell value from a bulk assay", 0.4),
    );
    let refusal = comparable_across(&malformed, &malformed)
        .expect_err("bulk states that it does not resolve cells");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::UnreportableAxis { .. }
    ));
    assert!(!refusal.is_silence());
}

#[test]
fn reporting_at_an_undeclared_axis_is_a_silence_rather_than_a_disagreement() {
    let silent = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics)
            .with_status(Resolution::Cell, ResolutionStatus::Undeclared),
        Resolution::Cell,
        fraction("a per-cell value from an undeclared assay", 0.4),
    );
    let refusal = comparable_across(&silent, &silent).expect_err("nobody said");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::UndeclaredAxis { .. }
    ));
    assert!(refusal.is_silence());
}

#[test]
fn the_measurand_check_runs_before_the_unit_check() {
    let rna = rna_measurement();
    let protein_in_percent = ModalMeasurement::new(
        descriptor(Modality::Proteomics),
        Resolution::Population,
        Measurement::scalar(
            "TP53 by mass spectrometry",
            Quantity::new(40.0, Unit::parse("%").expect("percent is in the table")),
        )
        .of(tp53()),
    );
    let refusal = comparable_across(&rna, &protein_in_percent)
        .expect_err("both the measurand and the unit differ");
    assert!(
        matches!(refusal, CrossModalIncomparability::MeasurandMismatch { .. }),
        "a conversion factor for a conversion that does not exist is not actionable advice"
    );
}

#[test]
fn the_standards_layer_still_blocks_what_it_blocked_before() {
    let left = rna_measurement();
    let right = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        Measurement::scalar(
            "TP53 in percent",
            Quantity::new(40.0, Unit::parse("%").expect("percent is in the table")),
        )
        .of(tp53()),
    );
    let refusal =
        comparable_across(&left, &right).expect_err("a conversion must be recorded to happen");
    assert!(matches!(
        refusal,
        CrossModalIncomparability::Standards(Incomparability::ConversionRequired { .. })
    ));
}

#[test]
fn a_delegated_refusal_keeps_the_standards_layer_s_scope_class() {
    let left = rna_measurement();
    let right = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        Measurement::scalar(
            "TP53 in percent",
            Quantity::new(40.0, Unit::parse("%").expect("percent is in the table")),
        )
        .of(tp53()),
    );
    let refusal = comparable_across(&left, &right).expect_err("units differ");
    assert_eq!(refusal.blocking_class(), ScopeClass::Coordinate);
}

#[test]
fn a_modality_refusal_maps_onto_the_specimen_scope_class() {
    let refusal = comparable_across(&rna_measurement(), &protein_measurement())
        .expect_err("different quantities");
    assert_eq!(refusal.blocking_class(), ScopeClass::Specimen);
}

#[test]
fn two_compositional_readouts_are_comparable_but_carry_a_caveat() {
    let left = ModalMeasurement::new(
        descriptor(Modality::Microbiome),
        Resolution::Population,
        fraction("Bacteroides share, before", 0.3),
    );
    let right = ModalMeasurement::new(
        descriptor(Modality::Microbiome),
        Resolution::Population,
        fraction("Bacteroides share, after", 0.5),
    );
    let receipt = report(&left, &right, ComparabilityPolicy::default());
    assert!(receipt.verdict.is_comparable());
    assert!(receipt
        .caveats
        .iter()
        .any(|caveat| caveat.contains("compositional")));
}

#[test]
fn a_blocked_modality_check_produces_no_standards_report() {
    let receipt = report(
        &rna_measurement(),
        &protein_measurement(),
        ComparabilityPolicy::default(),
    );
    assert!(!receipt.verdict.is_comparable());
    assert!(
        receipt.standards.is_none(),
        "the standards layer was never consulted, and saying it was would be a fabricated receipt"
    );
}

#[test]
fn a_comparable_verdict_carries_the_standards_layer_s_own_report() {
    let receipt = report(
        &rna_measurement(),
        &rna_measurement(),
        ComparabilityPolicy::default(),
    );
    assert!(receipt.verdict.is_comparable());
    assert!(receipt.standards.is_some());
}

#[test]
fn a_report_digest_is_deterministic() {
    let first = report(
        &rna_measurement(),
        &protein_measurement(),
        ComparabilityPolicy::default(),
    );
    let second = report(
        &rna_measurement(),
        &protein_measurement(),
        ComparabilityPolicy::default(),
    );
    assert_eq!(
        first.digest().expect("finite values hash"),
        second.digest().expect("finite values hash")
    );
}

#[test]
fn a_report_digest_changes_when_the_verdict_does() {
    let blocked = report(
        &rna_measurement(),
        &protein_measurement(),
        ComparabilityPolicy::default(),
    );
    let comparable = report(
        &rna_measurement(),
        &rna_measurement(),
        ComparabilityPolicy::default(),
    );
    assert_ne!(
        blocked.digest().expect("finite values hash"),
        comparable.digest().expect("finite values hash")
    );
}

#[test]
fn an_imputed_side_is_named_in_the_caveats_even_when_the_verdict_is_blocked() {
    let deconvolved = ModalMeasurement::new(
        descriptor(Modality::BulkTranscriptomics).with_status(
            Resolution::Cell,
            ResolutionStatus::Imputed {
                source: Modality::BulkTranscriptomics,
                by: "deconvolution against a signature matrix".to_string(),
            },
        ),
        Resolution::Cell,
        fraction("estimated malignant fraction", 0.4),
    );
    let measured = ModalMeasurement::new(
        descriptor(Modality::SingleCell),
        Resolution::Cell,
        fraction("counted malignant fraction", 0.4),
    );
    let receipt = report(&deconvolved, &measured, ComparabilityPolicy::default());
    assert!(!receipt.verdict.is_comparable());
    assert!(receipt
        .caveats
        .iter()
        .any(|caveat| caveat.contains("estimate")));
}
