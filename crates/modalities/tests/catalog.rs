//! Integrity of the transcription: that the catalogue says what section 28 says, and that its
//! honesty machinery is used rather than decorative.

use bioprism_modalities::{
    catalog, descriptor, substitution_failure_mode, ClaimKind, FailureTrigger, Measurand, Modality,
    ModalityDescriptor, Resolution,
};
use std::collections::BTreeSet;

#[test]
fn every_descriptor_declares_every_axis() {
    for descriptor in catalog::all() {
        assert!(
            descriptor.is_complete(),
            "{} leaves {:?} undeclared; the catalogue must never refuse for silence",
            descriptor.modality,
            descriptor.undeclared_axes()
        );
    }
}

#[test]
fn resolved_and_unresolved_axes_partition_a_complete_descriptor() {
    for descriptor in catalog::all() {
        let total = descriptor.resolved_axes().len() + descriptor.unresolved_axes().len();
        assert_eq!(
            total,
            Resolution::ALL.len(),
            "{} declares {total} of {} axes",
            descriptor.modality,
            Resolution::ALL.len()
        );
    }
}

#[test]
fn every_module_contributes_the_five_failure_modes_the_blueprint_lists() {
    for descriptor in catalog::all() {
        let own: Vec<_> = descriptor
            .failure_modes()
            .iter()
            .filter(|mode| mode.module == descriptor.modality.blueprint_module())
            .collect();
        assert_eq!(
            own.len(),
            5,
            "{} carries {} of its own five failure modes",
            descriptor.modality,
            own.len()
        );
    }
}

#[test]
fn the_failure_mode_labels_are_distinct_within_each_module() {
    for descriptor in catalog::all() {
        let labels: BTreeSet<&str> = descriptor
            .failure_modes()
            .iter()
            .filter(|mode| mode.module == descriptor.modality.blueprint_module())
            .map(|mode| mode.label.as_str())
            .collect();
        assert_eq!(labels.len(), 5, "{} repeats a label", descriptor.modality);
    }
}

#[test]
fn every_claim_named_by_a_trigger_is_a_real_claim_kind() {
    let known: BTreeSet<&str> = ClaimKind::ALL.into_iter().map(ClaimKind::as_str).collect();
    for descriptor in catalog::all() {
        for mode in descriptor.failure_modes() {
            if let FailureTrigger::ClaimUnsupported { claim } = &mode.trigger {
                assert!(
                    known.contains(claim.as_str()),
                    "{}'s {:?} names {claim:?}, which is not a ClaimKind",
                    descriptor.modality,
                    mode.label
                );
            }
        }
    }
}

#[test]
fn every_mechanised_trigger_actually_fires_for_its_modality() {
    for descriptor in catalog::all() {
        for mode in descriptor.failure_modes() {
            let FailureTrigger::ClaimUnsupported { claim } = &mode.trigger else {
                continue;
            };
            let kind = ClaimKind::ALL
                .into_iter()
                .find(|candidate| candidate.as_str() == claim)
                .expect("checked by every_claim_named_by_a_trigger_is_a_real_claim_kind");
            let refusal = bioprism_modalities::supports_descriptor(&descriptor, kind).expect_err(
                "a failure mode that names a claim the modality happily supports is decoration",
            );
            assert!(
                refusal.named_module().is_some(),
                "{}'s {:?} did not surface for a {kind} claim",
                descriptor.modality,
                mode.label
            );
        }
    }
}

#[test]
fn every_unmechanised_failure_mode_says_what_would_be_needed() {
    for descriptor in catalog::all() {
        for mode in descriptor.failure_modes() {
            match &mode.trigger {
                FailureTrigger::NotMechanised { reason } => assert!(
                    reason.len() > 20,
                    "{}'s {:?} is unchecked with no useful reason",
                    descriptor.modality,
                    mode.label
                ),
                FailureTrigger::CheckedElsewhere { by } => assert!(
                    by.len() > 10,
                    "{}'s {:?} claims to be checked elsewhere without saying where",
                    descriptor.modality,
                    mode.label
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn the_unchecked_failure_modes_are_counted_rather_than_hidden() {
    let modes: Vec<_> = catalog::all()
        .into_iter()
        .flat_map(|descriptor| {
            let own = descriptor.modality.blueprint_module();
            descriptor
                .failure_modes()
                .iter()
                .filter(|mode| mode.module == own)
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        modes.len(),
        85,
        "seventeen modules of five failure modes each"
    );

    let by_support = modes
        .iter()
        .filter(|mode| mode.is_checked_by_support_relation())
        .count();
    let elsewhere = modes
        .iter()
        .filter(|mode| mode.is_mechanised() && !mode.is_checked_by_support_relation())
        .count();
    let unchecked = modes.iter().filter(|mode| !mode.is_mechanised()).count();

    assert_eq!(
        by_support + elsewhere + unchecked,
        modes.len(),
        "the three categories must partition the transcribed failure modes"
    );
    assert_eq!(
        (by_support, elsewhere, unchecked),
        (28, 16, 41),
        "the coverage of section 28's failure-mode lists changed; update the figure in lib.rs \
         rather than letting the claim drift"
    );
}

#[test]
fn each_modality_cites_a_distinct_blueprint_module() {
    let modules: BTreeSet<&str> = Modality::ALL
        .into_iter()
        .map(Modality::blueprint_module)
        .collect();
    assert_eq!(modules.len(), Modality::ALL.len());
}

#[test]
fn the_catalogue_covers_exactly_the_modules_this_crate_claims() {
    let modules: BTreeSet<&str> = Modality::ALL
        .into_iter()
        .map(Modality::blueprint_module)
        .collect();
    assert!(!modules.contains("28.01"), "genomics belongs to bioprism-onco");
    assert!(!modules.contains("28.13"), "medical imaging belongs elsewhere");
    assert!(
        !modules.contains("28.19"),
        "ontologies and identifiers are bioprism-standards' subject"
    );
    assert_eq!(modules.len(), 17);
}

#[test]
fn descriptors_round_trip_through_json() {
    for descriptor in catalog::all() {
        let text = serde_json::to_string(&descriptor).expect("descriptors serialise");
        let parsed: ModalityDescriptor = serde_json::from_str(&text).expect("and deserialise");
        assert_eq!(parsed, descriptor);
    }
}

#[test]
fn a_descriptor_is_consistent_with_itself_and_inconsistent_with_its_negation() {
    let bulk = descriptor(Modality::BulkTranscriptomics);
    assert!(bulk.check_consistent_with(&bulk).is_ok());

    let contradicting = bulk.clone().resolving(Resolution::Cell);
    assert!(
        bulk.check_consistent_with(&contradicting).is_err(),
        "one descriptor says the assay resolves cells and the other says it does not"
    );
}

#[test]
fn an_undeclared_axis_never_contradicts_a_declared_one() {
    let bulk = descriptor(Modality::BulkTranscriptomics);
    let silent = ModalityDescriptor::new(
        Modality::BulkTranscriptomics,
        Measurand::TranscriptAbundance,
        "declares nothing",
        bioprism_modalities::EvidenceDesign::Observational,
    );
    assert!(bulk.check_consistent_with(&silent).is_ok());
}

#[test]
fn the_constants_section_28_does_not_supply_are_recorded_rather_than_invented() {
    let recorded: Vec<String> = catalog::all()
        .into_iter()
        .flat_map(|descriptor| descriptor.caller_supplied_constants().to_vec())
        .collect();
    assert!(
        recorded.len() >= 15,
        "only {} caller-supplied constants are recorded; section 28 leaves more open than that",
        recorded.len()
    );
    for constant in &recorded {
        assert!(
            !constant.chars().any(|c| c.is_ascii_digit()),
            "{constant:?} contains a number, which would be this crate inventing a threshold"
        );
    }
}

#[test]
fn the_substitution_lookup_finds_the_module_that_names_the_pair() {
    let mode = substitution_failure_mode(Measurand::TranscriptAbundance, Measurand::ProteinAbundance)
        .expect("28.06 names this substitution");
    assert_eq!(mode.module, "28.06");
    assert_eq!(mode.label, "RNA-protein equivalence");
}

#[test]
fn the_substitution_lookup_returns_nothing_for_a_pair_no_module_names() {
    assert!(
        substitution_failure_mode(Measurand::AtomicCoordinates, Measurand::TaxonAbundance)
            .is_none(),
        "inventing a failure mode for an unnamed pair would put words in the blueprint's mouth"
    );
}

#[test]
fn only_taxon_abundance_is_declared_compositional() {
    let compositional: Vec<Measurand> = catalog::all()
        .into_iter()
        .map(|descriptor| descriptor.measurand)
        .filter(|measurand| measurand.is_compositional())
        .collect();
    assert_eq!(compositional, vec![Measurand::TaxonAbundance]);
}

#[test]
fn the_catalogue_is_deterministic_across_calls() {
    assert_eq!(catalog::all(), catalog::all());
    assert_eq!(
        descriptor(Modality::Spatial),
        descriptor(Modality::Spatial),
    );
}

#[test]
fn a_purpose_sentence_is_present_for_every_modality() {
    for descriptor in catalog::all() {
        assert!(
            descriptor.purpose.len() > 40,
            "{} carries no purpose sentence from its blueprint module",
            descriptor.modality
        );
    }
}
