//! Cross-modality transport, and the asymmetry between an exact move and an invertible one.

use bioprism_modalities::{
    descriptor, supports_descriptor, ClaimKind, Fidelity, Modality, ModalityTransport, Resolution,
    ResolutionStatus, TransportChain, TransportKind, TransportRefusal,
};
use bioprism_scope::{AggregationOperator, MappingCheck};

fn aggregate_cells_to_bulk() -> ModalityTransport {
    ModalityTransport::aggregating(
        &descriptor(Modality::SingleCell),
        Modality::BulkTranscriptomics,
        Resolution::Cell,
        AggregationOperator::Mean,
    )
    .expect("single-cell resolves cells")
}

fn deconvolve_bulk_to_cells() -> ModalityTransport {
    ModalityTransport::deconvolving(
        &descriptor(Modality::BulkTranscriptomics),
        Modality::SingleCell,
        Resolution::Cell,
        "a declared signature matrix",
        AggregationOperator::Sum,
    )
    .expect("bulk does not resolve cells, so the axis can be created")
}

#[test]
fn aggregating_single_cell_to_bulk_is_exact() {
    assert!(aggregate_cells_to_bulk().fidelity().is_exact());
}

#[test]
fn deconvolving_bulk_to_cells_is_an_estimate_conditioned_on_its_reference() {
    match deconvolve_bulk_to_cells().fidelity() {
        Fidelity::Estimated { conditioned_on } => {
            assert!(conditioned_on.contains("signature matrix"))
        }
        Fidelity::Exact => panic!("a deconvolution is never exact"),
    }
}

#[test]
fn an_aggregation_cannot_be_inverted() {
    let refusal = aggregate_cells_to_bulk()
        .invert()
        .expect_err("the distribution is gone");
    assert!(matches!(refusal, TransportRefusal::NotInvertible { .. }));
    assert!(refusal.to_string().contains("not recoverable"));
}

#[test]
fn a_deconvolution_can_be_inverted_and_its_inverse_is_an_aggregation() {
    let inverse = deconvolve_bulk_to_cells()
        .invert()
        .expect("recomposition returns the summary it was constrained by");
    assert!(matches!(
        inverse.kind,
        TransportKind::Aggregation {
            operator: AggregationOperator::Sum
        }
    ));
    assert_eq!(inverse.from, Modality::SingleCell);
    assert_eq!(inverse.to, Modality::BulkTranscriptomics);
}

#[test]
fn the_exact_transport_is_the_one_that_cannot_be_undone() {
    let aggregation = aggregate_cells_to_bulk();
    let deconvolution = deconvolve_bulk_to_cells();
    assert!(aggregation.fidelity().is_exact());
    assert!(!aggregation.is_invertible());
    assert!(!deconvolution.fidelity().is_exact());
    assert!(deconvolution.is_invertible());
}

#[test]
fn an_imputation_cannot_be_inverted_because_it_kept_no_mask() {
    let imputation = ModalityTransport::imputing(
        &descriptor(Modality::Proteomics),
        Modality::Proteomics,
        Resolution::Molecule,
        "a declared missingness model",
    )
    .expect("a named model is enough to declare the imputation");
    let refusal = imputation.invert().expect_err("no mask, nothing to remove");
    assert!(refusal.to_string().contains("nothing to remove"));
}

#[test]
fn aggregation_writes_its_own_loss_ledger() {
    let transport = aggregate_cells_to_bulk();
    assert!(!transport.loss().is_empty());
    assert!(transport
        .loss()
        .discarded
        .iter()
        .any(|entry| entry.contains("distribution")));
    assert!(transport.check().is_ok());
}

#[test]
fn a_deconvolution_records_the_reference_as_a_policy_condition() {
    let transport = deconvolve_bulk_to_cells();
    assert!(!transport.loss().uncertainty_added.is_empty());
    assert!(!transport.loss().policy_conditions.is_empty());
}

#[test]
fn a_deconvolution_without_a_named_reference_is_refused() {
    let refusal = ModalityTransport::deconvolving(
        &descriptor(Modality::BulkTranscriptomics),
        Modality::SingleCell,
        Resolution::Cell,
        "   ",
        AggregationOperator::Sum,
    )
    .expect_err("an unnamed reference makes the result unauditable");
    assert!(matches!(refusal, TransportRefusal::UnstatedBasis { .. }));
}

#[test]
fn deconvolving_an_axis_the_source_already_resolves_is_refused() {
    let refusal = ModalityTransport::deconvolving(
        &descriptor(Modality::SingleCell),
        Modality::SingleCell,
        Resolution::Cell,
        "a signature matrix",
        AggregationOperator::Sum,
    )
    .expect_err("deconvolving a measured axis would mark measurements as estimates");
    assert!(matches!(
        refusal,
        TransportRefusal::AggregationWouldAddResolution { .. }
    ));
}

#[test]
fn aggregating_an_axis_the_source_does_not_resolve_is_refused() {
    let refusal = ModalityTransport::aggregating(
        &descriptor(Modality::BulkTranscriptomics),
        Modality::BulkTranscriptomics,
        Resolution::Cell,
        AggregationOperator::Mean,
    )
    .expect_err("aggregating over cells that were never resolved is a relabelling");
    assert!(matches!(refusal, TransportRefusal::SourceLacksAxis { .. }));
}

#[test]
fn an_aggregated_descriptor_loses_the_claim_it_could_carry_before() {
    let single_cell = descriptor(Modality::SingleCell);
    assert!(supports_descriptor(&single_cell, ClaimKind::CellIntrinsicChange).is_ok());

    let aggregated = aggregate_cells_to_bulk()
        .apply(&single_cell)
        .expect("the ledger is non-empty");
    assert_eq!(
        aggregated.resolution(Resolution::Cell),
        ResolutionStatus::Unresolved
    );
    assert!(supports_descriptor(&aggregated, ClaimKind::CellIntrinsicChange).is_err());
}

#[test]
fn a_deconvolved_cell_axis_supports_composition_but_not_cell_intrinsic_change() {
    let deconvolved = deconvolve_bulk_to_cells()
        .apply(&descriptor(Modality::BulkTranscriptomics))
        .expect("the ledger is non-empty");
    assert!(matches!(
        deconvolved.resolution(Resolution::Cell),
        ResolutionStatus::Imputed { .. }
    ));
    assert!(supports_descriptor(&deconvolved, ClaimKind::CellComposition).is_ok());
    assert!(supports_descriptor(&deconvolved, ClaimKind::CellIntrinsicChange).is_err());
}

#[test]
fn a_round_trip_through_deconvolution_does_not_restore_the_descriptor() {
    let single_cell = descriptor(Modality::SingleCell);
    let aggregation = aggregate_cells_to_bulk();
    let recovery = ModalityTransport::deconvolving(
        &aggregation.apply(&single_cell).expect("aggregation applies"),
        Modality::SingleCell,
        Resolution::Cell,
        "a signature matrix",
        AggregationOperator::Sum,
    )
    .expect("the aggregated descriptor no longer resolves cells");

    let chain = TransportChain::new().then(aggregation).then(recovery);
    let result = chain.apply(&single_cell).expect("both steps apply");

    assert!(!chain.restores(&single_cell));
    assert!(matches!(
        result.resolution(Resolution::Cell),
        ResolutionStatus::Imputed { .. }
    ));
    assert_ne!(
        result.resolution(Resolution::Cell),
        single_cell.resolution(Resolution::Cell)
    );
}

#[test]
fn one_estimated_step_makes_the_whole_chain_estimated() {
    let chain = TransportChain::new()
        .then(aggregate_cells_to_bulk())
        .then(deconvolve_bulk_to_cells());
    assert!(!chain.fidelity().is_exact());
}

#[test]
fn a_chain_of_only_exact_steps_stays_exact() {
    let chain = TransportChain::new().then(aggregate_cells_to_bulk());
    assert!(chain.fidelity().is_exact());
}

#[test]
fn a_chain_accumulates_every_step_s_ledger() {
    let chain = TransportChain::new()
        .then(aggregate_cells_to_bulk())
        .then(deconvolve_bulk_to_cells());
    let loss = chain.loss();
    assert!(!loss.discarded.is_empty());
    assert!(!loss.uncertainty_added.is_empty());
    assert!(!loss.policy_conditions.is_empty());
}

#[test]
fn an_empty_chain_restores_trivially_and_carries_no_loss() {
    let chain = TransportChain::new();
    let single_cell = descriptor(Modality::SingleCell);
    assert!(chain.is_empty());
    assert!(chain.restores(&single_cell));
    assert!(chain.loss().is_empty());
}

#[test]
fn a_transport_renders_as_a_scope_mapping_that_passes_its_own_audit() {
    for transport in [aggregate_cells_to_bulk(), deconvolve_bulk_to_cells()] {
        let mapping = transport.to_scope_mapping();
        assert_eq!(mapping.check(), MappingCheck::Sound);
    }
}

#[test]
fn a_deserialised_transport_with_an_emptied_ledger_is_refused() {
    let transport = aggregate_cells_to_bulk();
    let mut value = serde_json::to_value(&transport).expect("transports serialise");
    value["loss"] = serde_json::json!({});
    let stripped: ModalityTransport =
        serde_json::from_value(value).expect("an empty ledger is representable in JSON");

    let refusal = stripped.check().expect_err("crossing modalities is never free");
    assert!(matches!(refusal, TransportRefusal::UndeclaredLoss { .. }));
    assert!(stripped
        .apply(&descriptor(Modality::SingleCell))
        .is_err());
}

#[test]
fn a_transport_round_trips_through_json() {
    let transport = deconvolve_bulk_to_cells();
    let text = serde_json::to_string(&transport).expect("transports serialise");
    let parsed: ModalityTransport = serde_json::from_str(&text).expect("and deserialise");
    assert_eq!(parsed, transport);
}
