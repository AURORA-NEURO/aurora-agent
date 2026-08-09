//! The seventeen descriptors, transcribed from section 28.
//!
//! Everything here is derived from the blueprint text and nothing is invented. The `purpose`
//! string on each descriptor is 28.xx's own purpose sentence, the failure-mode `label` and
//! `statement` fields are its own "characteristic failure modes" list, and where a decision needs
//! a number the blueprint does not state, it is recorded on
//! [`ModalityDescriptor::caller_supplied_constants`] instead of being filled in.
//!
//! # The two judgement calls that recur
//!
//! **A single acquisition is a snapshot.** Most modalities are declared
//! [`ResolutionStatus::Unresolved`] on [`Resolution::Timepoint`], because one run of an assay
//! produces one time point and a rate or an ordering needs two. This is not a claim that the
//! modality *cannot* be longitudinal — a repeated-sampling design obviously can — it is a claim
//! about what a bare descriptor entitles you to. A caller with a time course declares it with
//! [`ModalityDescriptor::with_status`], and 28.07's flux claims and 28.04's trajectory claims
//! become supportable. Pushing the declaration to the caller is the same discipline
//! `crates/oncoworlds` used for detection sensitivity.
//!
//! **The general case, not the best case.** 28.05's descriptor declares
//! [`Resolution::Cell`] unresolved, because the blueprint's own failure-mode list says
//! "spot-level mixtures are interpreted as single cells" and its artifact list includes cell-type
//! deconvolution — both of which describe a spot-based platform. An imaging-based platform that
//! segments cells directly is a different descriptor, and the caller builds it. Declaring the
//! optimistic case in the catalogue would make the refusal that matters disappear.
//!
//! # How much of section 28 is boilerplate
//!
//! Measured over these seventeen modules by counting a non-empty line as boilerplate when the
//! identical line appears in a majority of them: **51.7% with YAML front matter included, 48.0%
//! without**. The two figures bracket the answer and the 3.7-point gap is entirely the seven-line
//! front matter, four lines of which (`status`, `owner`, `last_updated`, `product`) are constant.
//! The threshold barely matters here — the shared lines are shared by *all* seventeen, so moving
//! the majority threshold from 9 to 17 changes nothing, and dropping it to 2 adds only 1.1 points
//! of near-duplicate phrasing. Structurally: of ten headings, "required normalized contract" and
//! "release gates" are byte-identical across all seventeen, and four more end in an identical
//! trailing paragraph.
//!
//! # Coverage of the failure-mode lists
//!
//! Section 28 lists five characteristic failure modes per module, eighty-five across the seventeen
//! modules here. They are all transcribed. They are not all checkable, and
//! [`FailureMode::is_mechanised`] says which: a trigger of
//! [`FailureTrigger::NotMechanised`] means nothing in this crate detects it, and the `reason`
//! field says what would be needed. Most of the unmechanised ones need something this crate does
//! not hold — a count matrix, a peak set, a guide-to-gene map, a model lineage graph — or are
//! properties of a search strategy rather than of a measurement.

use crate::descriptor::{
    EvidenceDesign, FailureMode, FailureTrigger, Measurand, Modality, ModalityDescriptor,
    Resolution, ResolutionStatus,
};

/// The catalogue descriptor for a modality.
///
/// Built fresh on each call rather than held in a static. The descriptors are small, the crate is
/// deterministic, and a caller who mutates one is then working with their own descriptor rather
/// than with a shared mutable version of the blueprint's.
pub fn descriptor(modality: Modality) -> ModalityDescriptor {
    match modality {
        Modality::Epigenomics => epigenomics(),
        Modality::BulkTranscriptomics => bulk_transcriptomics(),
        Modality::SingleCell => single_cell(),
        Modality::Spatial => spatial(),
        Modality::Proteomics => proteomics(),
        Modality::Metabolomics => metabolomics(),
        Modality::FunctionalScreen => functional_screen(),
        Modality::ProteinStructure => protein_structure(),
        Modality::Pharmacology => pharmacology(),
        Modality::Microbiome => microbiome(),
        Modality::Microscopy => microscopy(),
        Modality::DigitalPathology => digital_pathology(),
        Modality::ClinicalEhr => clinical_ehr(),
        Modality::TrialsAndRwe => trials_and_rwe(),
        Modality::Literature => literature(),
        Modality::ModelOrganism => model_organism(),
        Modality::NeuroOncologyConnector => neuro_oncology_connector(),
    }
}

/// Every catalogue descriptor, in blueprint module order.
pub fn all() -> Vec<ModalityDescriptor> {
    Modality::ALL.into_iter().map(descriptor).collect()
}

/// The failure mode section 28 names for substituting one measurand for another.
///
/// Used by [`crate::comparability`] so a cross-modal refusal can cite 28.06 by name instead of
/// saying "different measurands". Returns the first match across the whole catalogue, since a
/// substitution is a mistake about a pair rather than about one modality.
pub fn substitution_failure_mode(from: Measurand, to: Measurand) -> Option<FailureMode> {
    all().into_iter().find_map(|descriptor| {
        descriptor
            .failure_modes()
            .iter()
            .find(|mode| {
                matches!(
                    mode.trigger,
                    FailureTrigger::MeasurandSubstitution { from: f, to: t } if f == from && t == to
                )
            })
            .cloned()
    })
}

fn unresolving_everything_except(
    mut descriptor: ModalityDescriptor,
    resolved: &[Resolution],
) -> ModalityDescriptor {
    for axis in Resolution::ALL {
        descriptor = if resolved.contains(&axis) {
            descriptor.resolving(axis)
        } else {
            descriptor.not_resolving(axis)
        };
    }
    descriptor
}

/// 28.02 — chromatin accessibility, DNA methylation, histone marks and 3D genome.
fn epigenomics() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Epigenomics,
            Measurand::ChromatinState,
            "Evaluate chromatin accessibility, DNA methylation, histone modification, \
             regulatory-element, and 3D-genome analyses with assay-aware controls and \
             reference-version discipline.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Population, Resolution::Molecule, Resolution::Subject],
    )
    .failing(FailureMode::new(
        "28.02",
        "composition confounding",
        "Bulk epigenomic signal may reflect changing cell fractions.",
        FailureTrigger::ClaimUnsupported {
            claim: "cell-intrinsic-change".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.02",
        "reference drift",
        "Classifier output can change with reference and preprocessing versions.",
        FailureTrigger::UndeclaredTransport {
            operation: "classifier scoring against a pinned reference and preprocessing version"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.02",
        "peak instability",
        "Different callers and thresholds change the universe of tested regions.",
        FailureTrigger::NotMechanised {
            reason: "detecting it needs two callers' region sets to compare; this crate holds no \
                     peaks"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.02",
        "causal overreach",
        "Accessibility or methylation association does not alone prove regulation.",
        FailureTrigger::ClaimUnsupported {
            claim: "causal-effect-of-perturbation".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.02",
        "multiple testing",
        "Large regulatory search spaces require controlled inference.",
        FailureTrigger::NotMechanised {
            reason: "an inference procedure rather than an instrument property; this crate \
                     computes no statistics"
                .to_string(),
        },
    ))
    .requiring_constant(
        "the peak-calling threshold and the caller that produced the tested region universe",
    )
}

/// 28.03 — RNA-seq and expression arrays over bulk tissue.
fn bulk_transcriptomics() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::BulkTranscriptomics,
            Measurand::TranscriptAbundance,
            "Evaluate RNA-seq and expression-array workflows from sample QC through differential \
             expression, pathway analysis, deconvolution, replication, and claim formation.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Population, Resolution::Molecule, Resolution::Subject],
    )
    .failing(FailureMode::new(
        "28.03",
        "pseudoreplication",
        "Technical or within-subject samples are treated as independent.",
        FailureTrigger::ReplicationUnitConfusion {
            counted: Resolution::Population,
            independent: Resolution::Subject,
        },
    ))
    .failing(FailureMode::new(
        "28.03",
        "normalization misuse",
        "Transformed values are passed to count models or units are mixed.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::comparable, for the mixed-units half; passing transformed \
                 values to a count model is a property of a pipeline this crate does not run"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.03",
        "composition",
        "Bulk changes are interpreted as cell-intrinsic regulation.",
        FailureTrigger::ClaimUnsupported {
            claim: "cell-intrinsic-change".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.03",
        "gene-set circularity",
        "Selection and enrichment use the same evidence without correction.",
        FailureTrigger::NotMechanised {
            reason: "detecting it needs the selection and enrichment evidence sets; this crate \
                     holds neither"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.03",
        "signature overfitting",
        "A discovery signature is reported without external replication.",
        FailureTrigger::NotMechanised {
            reason: "a property of an evaluation protocol; `bioprism-bioeval` owns held-out \
                     cohort discipline"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.06",
        "RNA-protein equivalence",
        "Transcript abundance is treated as protein activity.",
        FailureTrigger::MeasurandSubstitution {
            from: Measurand::TranscriptAbundance,
            to: Measurand::ProteinAbundance,
        },
    ))
}

/// 28.04 — single-cell and single-nucleus assays, including multiome.
fn single_cell() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::SingleCell,
            Measurand::TranscriptAbundance,
            "Evaluate long-horizon single-cell and single-nucleus workflows, including QC, \
             integration, annotation, state discovery, differential testing, trajectory, cell \
             interaction, and multimodal alignment.",
            EvidenceDesign::Observational,
        ),
        &[
            Resolution::Population,
            Resolution::Cell,
            Resolution::Molecule,
            Resolution::Subject,
        ],
    )
    .failing(FailureMode::new(
        "28.04",
        "cell-level pseudoreplication",
        "Cells are not independent biological replicates.",
        FailureTrigger::ReplicationUnitConfusion {
            counted: Resolution::Cell,
            independent: Resolution::Subject,
        },
    ))
    .failing(FailureMode::new(
        "28.04",
        "integration erasure",
        "Batch correction removes real disease structure.",
        FailureTrigger::NotMechanised {
            reason: "detecting it needs the pre- and post-integration embeddings".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.04",
        "annotation certainty",
        "Continuous or novel states are forced into known labels.",
        FailureTrigger::NotMechanised {
            reason: "a property of a label set and its uncertainty; `bioprism-standards` owns \
                     ontology binding precision, which is the closest available check"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.04",
        "layer confusion",
        "Raw counts, normalized values, and scaled data are mixed.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::comparable, which refuses a comparison across units without \
                 a recorded conversion"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.04",
        "trajectory causality",
        "Pseudotime is treated as observed temporal or causal order.",
        FailureTrigger::ClaimUnsupported {
            claim: "temporal-order".to_string(),
        },
    ))
}

/// 28.05 — molecular measurement carrying tissue coordinates.
fn spatial() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Spatial,
            Measurand::TranscriptAbundance,
            "Evaluate analyses that combine molecular measurements with tissue coordinates, \
             morphology, neighborhoods, and region-level provenance.",
            EvidenceDesign::Observational,
        ),
        &[
            Resolution::Population,
            Resolution::Location,
            Resolution::Molecule,
            Resolution::Subject,
        ],
    )
    .failing(FailureMode::new(
        "28.05",
        "resolution mismatch",
        "Spot-level mixtures are interpreted as single cells.",
        FailureTrigger::ClaimUnsupported {
            claim: "cell-identity".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.05",
        "coordinate mismatch",
        "Correct molecular values are attached to the wrong tissue regions.",
        FailureTrigger::UndeclaredTransport {
            operation: "registration of the expression matrix onto the image coordinate frame"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.05",
        "spatial autocorrelation",
        "Ordinary tests underestimate uncertainty.",
        FailureTrigger::NotMechanised {
            reason: "an inference procedure rather than an instrument property".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.05",
        "interaction inference",
        "Co-localization is treated as communication or causality.",
        FailureTrigger::ClaimUnsupported {
            claim: "cell-communication".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.05",
        "section sampling",
        "One section is generalized to the whole lesion.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_oncoworlds' 30.12 rule that a marker absent from a specimen is not \
                 absent from the tumour; the promotion from section to lesion is that crate's"
                .to_string(),
        },
    ))
    .requiring_constant("the platform's capture geometry and spot-to-cell ratio")
}

/// 28.06 — mass spectrometry, peptides, proteins and modification sites.
fn proteomics() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Proteomics,
            Measurand::ProteinAbundance,
            "Evaluate mass-spectrometry and protein-level analyses, including identification, \
             quantification, batch normalization, phosphoproteomics, protein complexes, and \
             genome-to-protein integration.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Population, Resolution::Molecule, Resolution::Subject],
    )
    .failing(FailureMode::new(
        "28.06",
        "PTM overreach",
        "Site detection is interpreted as functional activation without support.",
        FailureTrigger::ClaimUnsupported {
            claim: "protein-activity".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.06",
        "missing-not-at-random",
        "Low abundance and technical dropout are misinterpreted.",
        FailureTrigger::NotMechanised {
            reason: "distinguishing biological absence from assay non-detection needs the \
                     detection limit, which section 28 requires to be recorded but does not state"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.06",
        "protein inference",
        "Shared peptides create ambiguous protein assignments.",
        FailureTrigger::NotMechanised {
            reason: "needs the peptide-to-protein map".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.06",
        "RNA-protein equivalence",
        "Transcript abundance is treated as protein activity.",
        FailureTrigger::MeasurandSubstitution {
            from: Measurand::ProteinAbundance,
            to: Measurand::TranscriptAbundance,
        },
    ))
    .failing(FailureMode::new(
        "28.06",
        "batch structure",
        "Acquisition order or plex effects confound condition.",
        FailureTrigger::NotMechanised {
            reason: "needs the acquisition order and the condition assignment".to_string(),
        },
    ))
    .requiring_constant("the false-discovery rate at which identifications were accepted")
    .requiring_constant("the detection limit below which a missing protein is non-detection")
}

/// 28.07 — metabolite pools, isotope tracing and constraint-based flux models.
fn metabolomics() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Metabolomics,
            Measurand::MetabolitePool,
            "Evaluate metabolite identification, normalization, isotope tracing, pathway \
             interpretation, and metabolic-model analyses under substantial measurement ambiguity.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Population, Resolution::Molecule, Resolution::Subject],
    )
    .failing(FailureMode::new(
        "28.07",
        "pool-versus-flux",
        "Abundance changes are interpreted as flux changes.",
        FailureTrigger::ClaimUnsupported {
            claim: "flux-rate".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.07",
        "annotation ambiguity",
        "One mass feature may map to multiple metabolites.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::Incomparability::AmbiguousTerm, which refuses a local term \
                 with more than one candidate identifier"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.07",
        "media dependence",
        "In vitro conclusions depend strongly on culture conditions.",
        FailureTrigger::NotMechanised {
            reason: "needs the medium composition as a scope dimension; `bioprism-scope` can \
                     carry it, this crate does not supply it"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.07",
        "normalization",
        "Cell count, protein, or tissue mass choice changes interpretation.",
        FailureTrigger::NotMechanised {
            reason: "the normalisation basis is a caller-supplied declaration; see \
                     caller_supplied_constants"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.07",
        "model underdetermination",
        "Many flux states fit the same measurements.",
        FailureTrigger::NotMechanised {
            reason: "needs the stoichiometric model and its constraint set".to_string(),
        },
    ))
    .requiring_constant("the normalisation basis: cell count, protein, or tissue mass")
    .requiring_constant(
        "whether the design is a time course or a single acquisition, since a flux claim needs the \
         timepoint axis",
    )
}

/// 28.08 — pooled and arrayed CRISPR and other perturbation screens.
fn functional_screen() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::FunctionalScreen,
            Measurand::PerturbationPhenotype,
            "Evaluate pooled and arrayed perturbation analyses, guide design auditing, screen QC, \
             hit calling, genetic interactions, perturb-seq, and functional validation planning.",
            EvidenceDesign::Interventional,
        ),
        &[
            Resolution::Population,
            Resolution::Molecule,
            Resolution::Timepoint,
            Resolution::Perturbation,
        ],
    )
    .failing(FailureMode::new(
        "28.08",
        "guide-level artifacts",
        "Single-guide effects are treated as gene-level truth.",
        FailureTrigger::NotMechanised {
            reason: "needs the guide-to-gene map; a guide and its target are both \
                     Resolution::Molecule here and this crate cannot tell them apart"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.08",
        "context dependence",
        "Dependency is generalized beyond the tested model.",
        FailureTrigger::ClaimUnsupported {
            claim: "subject-level-outcome".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.08",
        "fitness confounding",
        "Slow growth or toxicity masks specific phenotypes.",
        FailureTrigger::NotMechanised {
            reason: "needs the viability readout alongside the phenotype".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.08",
        "combination multiplicity",
        "Large interaction spaces inflate false discoveries.",
        FailureTrigger::NotMechanised {
            reason: "an inference procedure rather than an instrument property".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.08",
        "translation gap",
        "Cell-line hits are presented as therapeutic targets without further evidence.",
        FailureTrigger::ClaimUnsupported {
            claim: "treatment-effect".to_string(),
        },
    ))
    .requiring_constant("the library coverage and multiplicity of infection the screen was run at")
}

/// 28.09 — experimental and predicted protein structure, docking and design.
fn protein_structure() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::ProteinStructure,
            Measurand::AtomicCoordinates,
            "Evaluate structure retrieval, confidence interpretation, binding or stability \
             modeling, sequence design, and experimental validation planning.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Molecule],
    )
    .failing(FailureMode::new(
        "28.09",
        "docking overinterpretation",
        "A score is treated as binding or efficacy evidence.",
        FailureTrigger::ClaimUnsupported {
            claim: "binding-affinity".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.09",
        "validation gap",
        "Designed sequences are reported without developability or functional tests.",
        FailureTrigger::ClaimUnsupported {
            claim: "protein-activity".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.09",
        "structure choice",
        "A high-resolution but biologically irrelevant construct is selected.",
        FailureTrigger::NotMechanised {
            reason: "needs the biological context the structure is being used to reason about"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.09",
        "confidence blindness",
        "Predicted coordinates are treated as equally reliable everywhere.",
        FailureTrigger::NotMechanised {
            reason: "needs the per-residue confidence map and a threshold section 28 does not \
                     state"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.09",
        "construct mismatch",
        "Residue numbering and domains differ from the experimental protein.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards' coordinate discipline, of which residue numbering is an \
                 instance: two numbering schemes are two frames"
                .to_string(),
        },
    ))
    .requiring_constant(
        "the per-residue confidence threshold below which predicted coordinates are not used",
    )
}

/// 28.10 — compound activity, dose-response, selectivity, ADME and exposure.
fn pharmacology() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Pharmacology,
            Measurand::CompoundActivity,
            "Evaluate target-to-compound reasoning, screening data, dose-response, selectivity, \
             combinations, PK/PD, brain exposure, and translational evidence without making \
             treatment recommendations.",
            EvidenceDesign::Interventional,
        ),
        &[
            Resolution::Population,
            Resolution::Molecule,
            Resolution::Subject,
            Resolution::Timepoint,
            Resolution::Perturbation,
        ],
    )
    .failing(FailureMode::new(
        "28.10",
        "potency-versus-exposure",
        "In vitro potency is interpreted without achievable brain or tumor exposure.",
        FailureTrigger::ClaimUnsupported {
            claim: "exposure-at-site".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.10",
        "assay interference",
        "Readout artifacts appear as activity.",
        FailureTrigger::NotMechanised {
            reason: "needs counter-screen readouts".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.10",
        "selectivity",
        "A broad toxic compound is misclassified as targeted.",
        FailureTrigger::NotMechanised {
            reason: "needs the off-target panel and a fold-difference threshold section 28 does \
                     not state"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.10",
        "model relevance",
        "Cell-line response is generalized to patient tumors.",
        FailureTrigger::CheckedElsewhere {
            by: "28.18's cross-species-equivalence claim, which no modality supports".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.10",
        "temporal leakage",
        "Later trial outcomes contaminate historical evaluation.",
        FailureTrigger::CheckedElsewhere {
            by: "crate::literature::EvaluationHorizon, for document-dated sources".to_string(),
        },
    ))
    .requiring_constant("the selectivity fold-difference that counts as targeted")
    .requiring_constant("the anatomical site at which exposure was measured, if any")
}

/// 28.11 — amplicon and shotgun metagenomics and community profiles.
fn microbiome() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Microbiome,
            Measurand::TaxonAbundance,
            "Evaluate taxonomic, functional, ecological, and host-associated microbiome analyses \
             with contamination, compositionality, batch, and causal limitations made explicit.",
            EvidenceDesign::Observational,
        ),
        &[Resolution::Population, Resolution::Molecule, Resolution::Subject],
    )
    .failing(FailureMode::new(
        "28.11",
        "compositionality",
        "Relative abundance changes can be misleading.",
        FailureTrigger::ClaimUnsupported {
            claim: "absolute-abundance-change".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.11",
        "causality",
        "Association is presented as host mechanism without intervention evidence.",
        FailureTrigger::ClaimUnsupported {
            claim: "host-mechanism".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.11",
        "contamination",
        "Low-biomass signals may be reagent or laboratory contaminants.",
        FailureTrigger::NotMechanised {
            reason: "needs the negative-control profiles and a biomass threshold section 28 does \
                     not state"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.11",
        "taxonomy drift",
        "Database versions alter assignments.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::Incomparability::OntologyVersionDrift, which refuses two \
                 bindings of one identifier at different releases"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.11",
        "host confounding",
        "Diet, medication, site, and collection dominate apparent disease effects.",
        FailureTrigger::NotMechanised {
            reason: "needs the host covariates as scope dimensions".to_string(),
        },
    ))
    .requiring_constant("the negative-control biomass floor below which a taxon is reagent signal")
}

/// 28.12 — bioimaging, segmentation, tracking and high-content plates.
fn microscopy() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Microscopy,
            Measurand::ImageIntensity,
            "Evaluate bioimage ingestion, segmentation, tracking, feature extraction, plate \
             design, phenotype interpretation, and image-to-claim provenance.",
            EvidenceDesign::PerRecord,
        ),
        &[
            Resolution::Population,
            Resolution::Cell,
            Resolution::Location,
            Resolution::Timepoint,
            Resolution::Perturbation,
        ],
    )
    .failing(FailureMode::new(
        "28.12",
        "field pseudoreplication",
        "Fields are treated as independent biological samples.",
        FailureTrigger::ReplicationUnitConfusion {
            counted: Resolution::Location,
            independent: Resolution::Subject,
        },
    ))
    .failing(FailureMode::new(
        "28.12",
        "segmentation propagation",
        "Small boundary errors distort downstream phenotypes.",
        FailureTrigger::UndeclaredTransport {
            operation: "segmentation, which is what puts the cell axis on an image".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.12",
        "metadata loss",
        "Channels or dimensions are reordered without detection.",
        FailureTrigger::NotMechanised {
            reason: "an ingestion concern; `bioprism-adapter` owns semantic-loss reporting"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.12",
        "plate confounding",
        "Treatment is correlated with plate position or batch.",
        FailureTrigger::NotMechanised {
            reason: "needs the plate layout and the treatment assignment".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.12",
        "representation shortcut",
        "A model uses imaging artifacts rather than biology.",
        FailureTrigger::NotMechanised {
            reason: "a property of a trained model; `bioprism-bioeval` owns shortcut probing"
                .to_string(),
        },
    ))
    .requiring_constant("the channel-to-marker map that would bind a channel to a molecule")
}

/// 28.14 — whole-slide imaging, stains, scanners and pathologist annotation.
fn digital_pathology() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::DigitalPathology,
            Measurand::ImageIntensity,
            "Evaluate whole-slide and region-level pathology workflows, stain and scanner \
             variation, patch sampling, annotation uncertainty, morphology-to-molecular claims, \
             and provenance.",
            EvidenceDesign::Observational,
        ),
        &[
            Resolution::Population,
            Resolution::Cell,
            Resolution::Location,
            Resolution::Subject,
        ],
    )
    .failing(FailureMode::new(
        "28.14",
        "aggregation",
        "Strong patch metrics do not imply patient-level validity.",
        FailureTrigger::ReplicationUnitConfusion {
            counted: Resolution::Location,
            independent: Resolution::Subject,
        },
    ))
    .failing(FailureMode::new(
        "28.14",
        "region mismatch",
        "Molecular assay and slide region are not aligned.",
        FailureTrigger::UndeclaredTransport {
            operation: "registration of a molecular assay onto a slide region".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.14",
        "patch leakage",
        "Patches from the same slide or patient cross splits.",
        FailureTrigger::NotMechanised {
            reason: "a property of a split; `bioprism-bioeval` owns leakage witnesses".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.14",
        "annotation certainty",
        "Subjective labels are treated as exact.",
        FailureTrigger::NotMechanised {
            reason: "needs inter-rater evidence, which section 28 requires be recorded but does \
                     not quantify"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.14",
        "scanner shortcut",
        "Model performance depends on acquisition artifacts.",
        FailureTrigger::NotMechanised {
            reason: "a property of a trained model; `bioprism-bioeval` owns shortcut probing"
                .to_string(),
        },
    ))
    .requiring_constant("the magnification and scanner the patches were drawn at")
}

/// 28.15 — cohort abstraction, longitudinal events and clinico-genomic linkage.
fn clinical_ehr() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::ClinicalEhr,
            Measurand::ClinicalEvent,
            "Evaluate cohort abstraction, longitudinal event alignment, endpoints, treatment \
             exposure, missingness, coding systems, and linkage to molecular or imaging artifacts \
             for research.",
            EvidenceDesign::Observational,
        ),
        &[
            Resolution::Population,
            Resolution::Subject,
            Resolution::Timepoint,
        ],
    )
    .failing(FailureMode::new(
        "28.15",
        "confounding by indication",
        "Treatment groups differ before treatment.",
        FailureTrigger::ClaimUnsupported {
            claim: "treatment-effect".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.15",
        "time zero",
        "Incorrect index date produces immortal-time or future leakage.",
        FailureTrigger::NotMechanised {
            reason: "needs the index-date definition and the event times; the definition is a \
                     caller-supplied constant"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.15",
        "coding drift",
        "Codes vary across sites and time.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::Incomparability::OntologyVersionDrift".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.15",
        "documentation bias",
        "Recorded data do not equal underlying state.",
        FailureTrigger::NotMechanised {
            reason: "the gap between record and state is unobservable from the record".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.15",
        "linkage",
        "Molecular and clinical records may refer to different episodes or specimens.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_oncoworlds' 30.03 patient, specimen, imaging and assay alignment"
                .to_string(),
        },
    ))
    .requiring_constant("the index-date definition that fixes time zero")
}

/// 28.16 — trial registries, protocols, endpoints and real-world comparators.
fn trials_and_rwe() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::TrialsAndRwe,
            Measurand::TrialRecord,
            "Evaluate trial retrieval, eligibility abstraction, endpoint reasoning, protocol \
             versioning, historical status, external controls, and evidence synthesis for research.",
            EvidenceDesign::PerRecord,
        ),
        &[
            Resolution::Population,
            Resolution::Subject,
            Resolution::Timepoint,
            Resolution::Perturbation,
        ],
    )
    .failing(FailureMode::new(
        "28.16",
        "causal overreach",
        "Nonrandomized comparisons are treated as treatment effects.",
        FailureTrigger::ClaimUnsupported {
            claim: "treatment-effect".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.16",
        "stale status",
        "Current registry information contaminates historical tasks.",
        FailureTrigger::CheckedElsewhere {
            by: "crate::literature::EvaluationHorizon, which refuses a source dated after the \
                 horizon"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.16",
        "eligibility simplification",
        "Complex criteria are reduced incorrectly.",
        FailureTrigger::NotMechanised {
            reason: "needs the criteria text and the reduction; comparing them is entailment \
                     checking"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.16",
        "endpoint mismatch",
        "PFS, OS, response, and surrogate endpoints are conflated.",
        FailureTrigger::NotMechanised {
            reason: "`bioprism-onco` owns response criteria and outcome estimands; the endpoint \
                     vocabulary is 30.26's, not this crate's"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.16",
        "publication bias",
        "Only successful or published evidence is used.",
        FailureTrigger::NotMechanised {
            reason: "a property of a search strategy over a corpus, not of any single record"
                .to_string(),
        },
    ))
    .requiring_constant("the registry snapshot date the records were retrieved at")
    .requiring_constant("whether this record is a randomised arm or a real-world comparator")
}

/// 28.17 — papers, preprints, databases and the citation graph.
fn literature() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::Literature,
            Measurand::PublishedClaim,
            "Evaluate source discovery, claim-level support, evidence hierarchy, conflict \
             reconciliation, versioning, and historical knowledge boundaries.",
            EvidenceDesign::Observational,
        ),
        &[],
    )
    .failing(FailureMode::new(
        "28.17",
        "citation laundering",
        "A review citation is used as if it were direct evidence.",
        FailureTrigger::CheckedElsewhere {
            by: "crate::literature::LiteratureClaim::bind, which refuses a review bound at the \
                 primary tier"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.17",
        "population mismatch",
        "Evidence from another disease or age group is generalized.",
        FailureTrigger::CheckedElsewhere {
            by: "crate::literature::LiteratureClaim::bind, which requires the target scope to \
                 refine the studied population"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.17",
        "temporal leakage",
        "Later discoveries are used in historical rediscovery.",
        FailureTrigger::CheckedElsewhere {
            by: "crate::literature::EvaluationHorizon".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.17",
        "authority bias",
        "Prestige replaces evidence quality.",
        FailureTrigger::NotMechanised {
            reason: "a property of a selection process; this crate ranks no sources".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.17",
        "claim drift",
        "The cited result is narrower than the agent statement.",
        FailureTrigger::NotMechanised {
            reason: "comparing a restatement against a source span is entailment checking, which \
                     this crate does not do; the source span is retained so a reader can"
                .to_string(),
        },
    ))
}

/// 28.18 — cell lines, organoids, xenografts and cross-species models.
fn model_organism() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::ModelOrganism,
            Measurand::ModelPhenotype,
            "Evaluate whether agents choose, interpret, and translate biological models \
             appropriately across species, cell lines, organoids, xenografts, and \
             patient-derived systems.",
            EvidenceDesign::Interventional,
        ),
        &[
            Resolution::Population,
            Resolution::Molecule,
            Resolution::Subject,
            Resolution::Timepoint,
            Resolution::Perturbation,
        ],
    )
    .failing(FailureMode::new(
        "28.18",
        "cross-species equivalence",
        "Orthologs and phenotypes are assumed to be identical.",
        FailureTrigger::ClaimUnsupported {
            claim: "cross-species-equivalence".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.18",
        "exposure gap",
        "In vivo dosing does not establish clinically relevant exposure.",
        FailureTrigger::ClaimUnsupported {
            claim: "exposure-at-site".to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.18",
        "model misidentification",
        "A model is used outside its molecular or histologic context.",
        FailureTrigger::NotMechanised {
            reason: "needs the model's authentication evidence and the context it is being used in"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.18",
        "correlated replication",
        "Closely related models are treated as independent evidence.",
        FailureTrigger::NotMechanised {
            reason: "needs a model lineage graph; this crate holds no model relationships"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.18",
        "selection bias",
        "Only responsive models are reported.",
        FailureTrigger::NotMechanised {
            reason: "needs the set of models run alongside the set reported".to_string(),
        },
    ))
    .requiring_constant("the model's authentication and contamination evidence")
}

/// 28.20 — first-party connectors and dataset passports for public resources.
///
/// Every axis is unresolved, and that is the descriptor's content rather than an omission. A
/// dataset passport records that a resource exists, at a release, with an access tier. It measures
/// nothing about biology, so every biological claim against it is refused for want of the axis the
/// claim is about — which is what stops a catalogue entry being read as evidence.
fn neuro_oncology_connector() -> ModalityDescriptor {
    unresolving_everything_except(
        ModalityDescriptor::new(
            Modality::NeuroOncologyConnector,
            Measurand::DatasetRecord,
            "Define first-party connectors and dataset passports for public and controlled \
             neuro-oncology research resources.",
            EvidenceDesign::Observational,
        ),
        &[],
    )
    .failing(FailureMode::new(
        "28.20",
        "release drift",
        "Counts and content change between releases.",
        FailureTrigger::NotMechanised {
            reason: "needs two releases to compare; the release identifier is a caller-supplied \
                     constant"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.20",
        "access confusion",
        "Public metadata and controlled raw data have different permissions.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_scope::ScopeClass::Policy, which is where access tier belongs as a \
                 scope dimension"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.20",
        "cohort overlap",
        "The same subjects may appear in derived challenge or repository datasets.",
        FailureTrigger::NotMechanised {
            reason: "needs subject membership across releases, which a dataset passport does not \
                     carry; treating two overlapping cohorts as independent replication cannot be \
                     detected from the passport alone"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.20",
        "label harmonization",
        "Disease and response labels differ by project and era.",
        FailureTrigger::CheckedElsewhere {
            by: "bioprism_standards::ontology binding, which keeps the local label beside the \
                 bound term rather than replacing it"
                .to_string(),
        },
    ))
    .failing(FailureMode::new(
        "28.20",
        "modality sparsity",
        "Not every subject has every assay or time point.",
        FailureTrigger::NotMechanised {
            reason: "needs the per-subject assay matrix; the honest representation of a missing \
                     assay is ResolutionStatus::Undeclared, never a measured zero"
                .to_string(),
        },
    ))
    .requiring_constant("the release identifier the passport describes")
}

/// The status a descriptor gives an axis, for callers that want the catalogue's answer directly.
pub fn resolution_of(modality: Modality, axis: Resolution) -> ResolutionStatus {
    descriptor(modality).resolution(axis)
}
