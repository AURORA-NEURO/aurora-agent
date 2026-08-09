//! The support relation, and the asymmetry it exists to encode.

use bioprism_modalities::{
    analysis_unit, descriptor, supported_claims, supports, supports_descriptor, ClaimKind,
    EvidenceDesign, Modality, ModalityDescriptor, Resolution, ResolutionStatus, Unsupported,
};

#[test]
fn a_bulk_measurement_cannot_support_a_cell_level_claim() {
    let refusal = supports(Modality::BulkTranscriptomics, ClaimKind::CellIntrinsicChange)
        .expect_err("bulk transcriptomics resolves no cells");
    assert!(matches!(
        refusal.root(),
        Unsupported::MissingResolution {
            axis: Resolution::Cell,
            ..
        }
    ));
}

#[test]
fn the_refusal_for_a_bulk_cell_claim_names_the_blueprint_failure_mode() {
    let refusal = supports(Modality::BulkTranscriptomics, ClaimKind::CellIntrinsicChange)
        .expect_err("bulk transcriptomics resolves no cells");
    assert_eq!(refusal.named_module(), Some("28.03"));
    assert!(refusal.to_string().contains("composition"));
}

#[test]
fn single_cell_supports_the_cell_level_claim_bulk_cannot() {
    assert!(supports(Modality::SingleCell, ClaimKind::CellIntrinsicChange).is_ok());
    assert!(supports(Modality::BulkTranscriptomics, ClaimKind::CellIntrinsicChange).is_err());
}

#[test]
fn single_cell_also_supports_the_population_claim_so_the_asymmetry_runs_one_way() {
    assert!(supports(Modality::SingleCell, ClaimKind::PopulationAverage).is_ok());
    assert!(supports(Modality::BulkTranscriptomics, ClaimKind::PopulationAverage).is_ok());

    let single_cell = supported_claims(Modality::SingleCell);
    for claim in supported_claims(Modality::BulkTranscriptomics) {
        assert!(
            single_cell.contains(&claim),
            "{claim} is supported by bulk but not by single-cell, which would break the \
             containment the asymmetry rests on"
        );
    }
    assert!(single_cell.len() > supported_claims(Modality::BulkTranscriptomics).len());
}

#[test]
fn spatial_refuses_a_cell_identity_claim_because_a_spot_is_a_mixture() {
    let refusal = supports(Modality::Spatial, ClaimKind::CellIdentity)
        .expect_err("the catalogue describes a spot-based platform");
    assert_eq!(refusal.named_module(), Some("28.05"));
    assert!(refusal.to_string().contains("resolution mismatch"));
}

#[test]
fn a_cell_resolved_spatial_platform_supports_the_claim_the_catalogue_refuses() {
    let imaging = descriptor(Modality::Spatial).resolving(Resolution::Cell);
    assert!(supports_descriptor(&imaging, ClaimKind::CellIdentity).is_ok());
}

#[test]
fn spatial_refuses_communication_because_co_location_is_observational() {
    let imaging = descriptor(Modality::Spatial).resolving(Resolution::Cell);
    let refusal = supports_descriptor(&imaging, ClaimKind::CellCommunication)
        .expect_err("co-localisation is not communication");
    assert!(matches!(
        refusal.root(),
        Unsupported::ObservationalOnly { .. }
    ));
}

#[test]
fn an_undeclared_axis_refuses_differently_from_an_unresolved_one() {
    let silent = ModalityDescriptor::new(
        Modality::BulkTranscriptomics,
        bioprism_modalities::Measurand::TranscriptAbundance,
        "a descriptor that declares nothing",
        EvidenceDesign::Observational,
    );
    let silent_refusal = supports_descriptor(&silent, ClaimKind::CellIntrinsicChange)
        .expect_err("nothing was declared");
    let stated_refusal = supports(Modality::BulkTranscriptomics, ClaimKind::CellIntrinsicChange)
        .expect_err("the catalogue states the assay cannot");

    assert!(matches!(
        silent_refusal.root(),
        Unsupported::UndeclaredResolution { .. }
    ));
    assert!(matches!(
        stated_refusal.root(),
        Unsupported::MissingResolution { .. }
    ));
    assert!(silent_refusal.is_silence());
    assert!(!stated_refusal.is_silence());
}

#[test]
fn proteomics_cannot_support_a_protein_activity_claim() {
    let refusal = supports(Modality::Proteomics, ClaimKind::ProteinActivity)
        .expect_err("abundance and modification are not activity");
    assert_eq!(refusal.named_module(), Some("28.06"));
    assert!(refusal.to_string().contains("PTM overreach"));
}

#[test]
fn a_predicted_structure_cannot_support_a_binding_claim() {
    let refusal = supports(Modality::ProteinStructure, ClaimKind::BindingAffinity)
        .expect_err("a docking score is not a binding measurement");
    assert!(matches!(
        refusal.root(),
        Unsupported::WrongMeasurand { .. }
    ));
    assert!(refusal.to_string().contains("docking overinterpretation"));
}

#[test]
fn metabolomics_cannot_support_a_flux_claim_from_a_single_acquisition() {
    let refusal = supports(Modality::Metabolomics, ClaimKind::FluxRate)
        .expect_err("a pool size is a snapshot");
    assert!(matches!(
        refusal.root(),
        Unsupported::MissingResolution {
            axis: Resolution::Timepoint,
            ..
        }
    ));
    assert!(refusal.to_string().contains("pool-versus-flux"));
}

#[test]
fn declaring_a_time_course_makes_the_flux_claim_supportable() {
    let tracing = descriptor(Modality::Metabolomics).resolving(Resolution::Timepoint);
    assert!(supports_descriptor(&tracing, ClaimKind::FluxRate).is_ok());
}

#[test]
fn pharmacology_cannot_support_exposure_at_a_site_it_did_not_measure() {
    let refusal = supports(Modality::Pharmacology, ClaimKind::ExposureAtSite)
        .expect_err("a plasma curve is not a brain concentration");
    assert!(matches!(
        refusal.root(),
        Unsupported::MissingResolution {
            axis: Resolution::Location,
            ..
        }
    ));
    assert!(refusal.to_string().contains("potency-versus-exposure"));
}

#[test]
fn microbiome_cannot_support_an_absolute_abundance_claim() {
    let refusal = supports(Modality::Microbiome, ClaimKind::AbsoluteAbundanceChange)
        .expect_err("relative abundance is a share of a fixed total");
    assert!(matches!(
        refusal.root(),
        Unsupported::WrongMeasurand { .. }
    ));
    assert!(refusal.to_string().contains("compositionality"));
}

#[test]
fn microbiome_cannot_support_a_host_mechanism_claim_without_an_intervention() {
    let refusal = supports(Modality::Microbiome, ClaimKind::HostMechanism)
        .expect_err("association is not mechanism");
    assert!(matches!(
        refusal.root(),
        Unsupported::ObservationalOnly { .. }
    ));
}

#[test]
fn a_mixed_registry_refuses_a_treatment_effect_for_want_of_a_declared_design() {
    let refusal = supports(Modality::TrialsAndRwe, ClaimKind::TreatmentEffect)
        .expect_err("randomised arms and real-world comparators sit under one heading");
    assert!(matches!(
        refusal.root(),
        Unsupported::DesignNotDeclared { .. }
    ));
    assert!(refusal.is_silence());
}

#[test]
fn declaring_the_arm_randomised_makes_the_treatment_effect_claim_supportable() {
    let mut randomised = descriptor(Modality::TrialsAndRwe);
    randomised.design = EvidenceDesign::Interventional;
    assert!(supports_descriptor(&randomised, ClaimKind::TreatmentEffect).is_ok());
}

#[test]
fn an_ehr_cohort_refuses_a_treatment_effect_because_treatment_was_recorded_not_assigned() {
    let refusal = supports(Modality::ClinicalEhr, ClaimKind::TreatmentEffect)
        .expect_err("observed treatment is not assigned treatment");
    assert!(matches!(
        refusal.root(),
        Unsupported::MissingResolution {
            axis: Resolution::Perturbation,
            ..
        }
    ));
    assert!(refusal.to_string().contains("confounding by indication"));
}

#[test]
fn no_modality_supports_cross_species_equivalence() {
    for modality in Modality::ALL {
        assert!(
            supports(modality, ClaimKind::CrossSpeciesEquivalence).is_err(),
            "{modality} was allowed to assert cross-species equivalence"
        );
    }
}

#[test]
fn the_cross_species_refusal_says_what_would_establish_the_claim() {
    let refusal = supports(Modality::ModelOrganism, ClaimKind::CrossSpeciesEquivalence)
        .expect_err("no modality measures equivalence");
    assert!(refusal.to_string().contains("both species"));
}

#[test]
fn a_dataset_passport_supports_nothing_biological() {
    for claim in ClaimKind::ALL {
        let outcome = supports(Modality::NeuroOncologyConnector, claim);
        if claim == ClaimKind::DatasetContent {
            assert!(outcome.is_ok(), "a passport should describe its dataset");
        } else {
            assert!(
                outcome.is_err(),
                "a dataset passport was allowed to carry a {claim} claim"
            );
        }
    }
}

#[test]
fn only_literature_supports_a_published_claim() {
    let supporting: Vec<Modality> = Modality::ALL
        .into_iter()
        .filter(|modality| supports(*modality, ClaimKind::PublishedClaimSupport).is_ok())
        .collect();
    assert_eq!(supporting, vec![Modality::Literature]);
}

#[test]
fn counting_cells_as_independent_replicates_is_refused() {
    let single_cell = descriptor(Modality::SingleCell);
    let refusal = analysis_unit(&single_cell, Resolution::Cell)
        .expect_err("cells from one donor are not independent replicates");
    assert!(matches!(
        refusal.root(),
        Unsupported::PseudoReplication {
            counted: Resolution::Cell,
            independent: Resolution::Subject,
            ..
        }
    ));
    assert!(refusal.to_string().contains("cell-level pseudoreplication"));
}

#[test]
fn counting_subjects_as_independent_replicates_is_admissible() {
    let single_cell = descriptor(Modality::SingleCell);
    assert!(analysis_unit(&single_cell, Resolution::Subject).is_ok());
}

#[test]
fn digital_pathology_supports_a_subject_claim_yet_refuses_patch_counting() {
    let pathology = descriptor(Modality::DigitalPathology);
    assert!(supports_descriptor(&pathology, ClaimKind::SubjectLevelOutcome).is_ok());
    let refusal = analysis_unit(&pathology, Resolution::Location)
        .expect_err("patches from one slide are not independent");
    assert!(refusal.to_string().contains("aggregation"));
}

#[test]
fn a_modality_that_declares_no_independence_unit_never_trips_the_replication_check() {
    let structure = descriptor(Modality::ProteinStructure);
    for axis in Resolution::ALL {
        assert!(analysis_unit(&structure, axis).is_ok());
    }
}

#[test]
fn the_first_blocking_dimension_is_the_measurand_not_the_resolution() {
    let refusal = supports(Modality::ProteinStructure, ClaimKind::ExposureAtSite)
        .expect_err("coordinates are not concentrations");
    assert!(
        matches!(refusal.root(), Unsupported::WrongMeasurand { .. }),
        "protein structure lacks both the measurand and the location axis; the measurand must \
         block first"
    );
}

#[test]
fn a_functional_screen_supports_gene_dependency_but_not_a_treatment_effect() {
    assert!(supports(Modality::FunctionalScreen, ClaimKind::GeneDependency).is_ok());
    let refusal = supports(Modality::FunctionalScreen, ClaimKind::TreatmentEffect)
        .expect_err("a cell line is not a subject");
    assert!(refusal.to_string().contains("translation gap"));
}

#[test]
fn an_imputed_axis_refuses_a_circular_claim_and_admits_an_estimable_one() {
    let deconvolved = descriptor(Modality::BulkTranscriptomics).with_status(
        Resolution::Cell,
        ResolutionStatus::Imputed {
            source: Modality::BulkTranscriptomics,
            by: "deconvolution against a signature matrix".to_string(),
        },
    );
    assert!(supports_descriptor(&deconvolved, ClaimKind::CellComposition).is_ok());
    let refusal = supports_descriptor(&deconvolved, ClaimKind::CellIntrinsicChange)
        .expect_err("the cell structure came from the reference, not the specimen");
    assert!(matches!(
        refusal.root(),
        Unsupported::ImputedResolution { .. }
    ));
}
