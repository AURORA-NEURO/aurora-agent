//! What an assay family measures, at what resolution, and what it characteristically gets wrong.
//!
//! Blueprint section 28 gives each modality module the same ten headings. Two of them — "required
//! normalized contract" and "release gates" — are byte-identical across all seventeen modules, and
//! four more end in an identical trailing paragraph, so roughly half of each module's lines are
//! shared. The content that differs is concentrated in three places: the purpose sentence, the
//! native-artifact list, and the five-item **characteristic failure modes** list. This module turns
//! the third of those into data, because it is the only one already written as a set of checkable
//! prohibitions.
//!
//! # Resolution is the load-bearing axis
//!
//! Seventeen modalities all produce numbers. The mistake the crate exists to prevent is treating
//! them as interchangeable observations of one underlying truth, and the dimension along which
//! they most obviously are not is *what a single number is about*. 28.03 measures a population
//! average, 28.04 measures a distribution over cells, 28.05 measures a distribution over cells
//! *with tissue coordinates*. So [`Resolution`] enumerates the axes a value can be indexed by,
//! and a [`ModalityDescriptor`] states, per axis, whether the assay resolves it.
//!
//! # Four states, not two
//!
//! [`ResolutionStatus`] has four variants and the extra two are the point.
//! [`ResolutionStatus::Undeclared`] is not [`ResolutionStatus::Unresolved`]: one is a fact about
//! an assay, the other is a fact about a descriptor, and AGENTS.md forbids them sharing a
//! representation. [`ResolutionStatus::Imputed`] is neither: it marks an axis that a transport
//! created — a deconvolved cell fraction, an imputed missing intensity — and it exists so that
//! [`crate::support::supports`] can refuse the circular argument where cell-level structure
//! introduced by a reference panel is then used as evidence about cells.
//!
//! # What this module is not
//!
//! It holds no assay data, no detection limits, no platform thresholds and no instrument
//! parameters. Section 28 states none of those numbers, and `crates/oncoworlds` set the precedent
//! of pushing every missing constant back to the caller rather than inventing one that would then
//! wear the blueprint's authority. Where a decision needs a number — a coverage floor, an FDR, a
//! purity estimate — the descriptor records that the number is required and who must supply it.

use crate::error::{contradictory, ModalityError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The seventeen assay families this crate describes, each naming its blueprint module.
///
/// 28.01 (genomics and sequence), 28.13 (medical imaging and radiomics) and 28.19 (ontologies and
/// identifiers) are absent on purpose: the first two are covered by `bioprism-onco` and
/// `bioprism-oncoworlds`, and the third is `bioprism-standards`' subject rather than a modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// 28.02 — chromatin accessibility, DNA methylation, histone marks, 3D genome.
    Epigenomics,
    /// 28.03 — RNA-seq and expression arrays over bulk tissue.
    BulkTranscriptomics,
    /// 28.04 — single-cell and single-nucleus assays, including multiome.
    SingleCell,
    /// 28.05 — molecular measurement carrying tissue coordinates.
    Spatial,
    /// 28.06 — mass spectrometry, peptides, proteins and post-translational modifications.
    Proteomics,
    /// 28.07 — metabolite pools, isotope tracing and constraint-based flux models.
    Metabolomics,
    /// 28.08 — pooled and arrayed CRISPR and other perturbation screens.
    FunctionalScreen,
    /// 28.09 — experimental and predicted protein structure, docking and design.
    ProteinStructure,
    /// 28.10 — compound activity, dose-response, selectivity, ADME and exposure.
    Pharmacology,
    /// 28.11 — amplicon and shotgun metagenomics and community profiles.
    Microbiome,
    /// 28.12 — bioimaging, segmentation, tracking and high-content plates.
    Microscopy,
    /// 28.14 — whole-slide imaging, stains, scanners and pathologist annotation.
    DigitalPathology,
    /// 28.15 — cohort abstraction, longitudinal events and clinico-genomic linkage.
    ClinicalEhr,
    /// 28.16 — trial registries, protocols, endpoints and real-world comparators.
    TrialsAndRwe,
    /// 28.17 — papers, preprints, databases and the citation graph.
    Literature,
    /// 28.18 — cell lines, organoids, xenografts and cross-species models.
    ModelOrganism,
    /// 28.20 — first-party connectors and dataset passports for public resources.
    NeuroOncologyConnector,
}

impl Modality {
    /// Every modality in this crate, in blueprint module order.
    pub const ALL: [Modality; 17] = [
        Modality::Epigenomics,
        Modality::BulkTranscriptomics,
        Modality::SingleCell,
        Modality::Spatial,
        Modality::Proteomics,
        Modality::Metabolomics,
        Modality::FunctionalScreen,
        Modality::ProteinStructure,
        Modality::Pharmacology,
        Modality::Microbiome,
        Modality::Microscopy,
        Modality::DigitalPathology,
        Modality::ClinicalEhr,
        Modality::TrialsAndRwe,
        Modality::Literature,
        Modality::ModelOrganism,
        Modality::NeuroOncologyConnector,
    ];

    /// The blueprint module id, so a refusal can be traced to the text that motivates it.
    pub fn blueprint_module(self) -> &'static str {
        match self {
            Modality::Epigenomics => "28.02",
            Modality::BulkTranscriptomics => "28.03",
            Modality::SingleCell => "28.04",
            Modality::Spatial => "28.05",
            Modality::Proteomics => "28.06",
            Modality::Metabolomics => "28.07",
            Modality::FunctionalScreen => "28.08",
            Modality::ProteinStructure => "28.09",
            Modality::Pharmacology => "28.10",
            Modality::Microbiome => "28.11",
            Modality::Microscopy => "28.12",
            Modality::DigitalPathology => "28.14",
            Modality::ClinicalEhr => "28.15",
            Modality::TrialsAndRwe => "28.16",
            Modality::Literature => "28.17",
            Modality::ModelOrganism => "28.18",
            Modality::NeuroOncologyConnector => "28.20",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Modality::Epigenomics => "epigenomics",
            Modality::BulkTranscriptomics => "bulk transcriptomics",
            Modality::SingleCell => "single-cell and multiome",
            Modality::Spatial => "spatial omics",
            Modality::Proteomics => "proteomics",
            Modality::Metabolomics => "metabolomics and flux",
            Modality::FunctionalScreen => "CRISPR and functional screens",
            Modality::ProteinStructure => "protein structure",
            Modality::Pharmacology => "pharmacology and PK",
            Modality::Microbiome => "microbiome and metagenomics",
            Modality::Microscopy => "microscopy and high-content imaging",
            Modality::DigitalPathology => "digital pathology",
            Modality::ClinicalEhr => "clinical research and EHR",
            Modality::TrialsAndRwe => "trials and real-world evidence",
            Modality::Literature => "literature and knowledge bases",
            Modality::ModelOrganism => "model organisms and cross-species",
            Modality::NeuroOncologyConnector => "neuro-oncology connectors",
        }
    }
}

impl fmt::Display for Modality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.as_str(), self.blueprint_module())
    }
}

/// The axes a measurement can be indexed by.
///
/// Not a lattice and not ordered. Resolving cells does not imply resolving tissue location —
/// dissociated single-cell assays lose position, which is exactly why 28.05 exists as a separate
/// module — and resolving location does not imply resolving cells, which is 28.05's own
/// "resolution mismatch: spot-level mixtures are interpreted as single cells".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// A value exists per bulk sample. Almost every modality has this one.
    Population,
    /// A value exists per cell.
    Cell,
    /// A value carries tissue or plate coordinates.
    Location,
    /// A value exists per molecular species, residue, site or feature.
    Molecule,
    /// A value can be attributed to an identified subject or organism.
    Subject,
    /// A value carries a time point, so change can be observed rather than inferred.
    Timepoint,
    /// A value can be attributed to a deliberate intervention on a named target.
    Perturbation,
}

impl Resolution {
    pub const ALL: [Resolution; 7] = [
        Resolution::Population,
        Resolution::Cell,
        Resolution::Location,
        Resolution::Molecule,
        Resolution::Subject,
        Resolution::Timepoint,
        Resolution::Perturbation,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Resolution::Population => "population",
            Resolution::Cell => "cell",
            Resolution::Location => "location",
            Resolution::Molecule => "molecule",
            Resolution::Subject => "subject",
            Resolution::Timepoint => "timepoint",
            Resolution::Perturbation => "perturbation",
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a modality resolves an axis, and if not, why not.
///
/// The variants are not ranked and there is no `unwrap_or(Unresolved)`. A caller who wants
/// permissive behaviour must ask for it by matching, which is the point: the default reading of
/// "the descriptor said nothing" must never be "the assay cannot".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// The assay indexes values by this axis directly.
    Resolved,
    /// The assay demonstrably does not, and the descriptor says so.
    Unresolved,
    /// The axis was supplied by a transport rather than by the assay.
    ///
    /// Carries the source modality and a description of the operation, so a refusal can say
    /// "cell fractions here came from deconvolving bulk against a reference panel" rather than
    /// "not measured".
    Imputed { source: Modality, by: String },
    /// Nobody said.
    Undeclared,
}

impl ResolutionStatus {
    pub fn is_resolved(&self) -> bool {
        matches!(self, ResolutionStatus::Resolved)
    }

    /// True when the status is a stated fact about the assay rather than an absence of one.
    pub fn is_declared(&self) -> bool {
        !matches!(self, ResolutionStatus::Undeclared)
    }

    pub fn describe(&self) -> String {
        match self {
            ResolutionStatus::Resolved => "resolved".to_string(),
            ResolutionStatus::Unresolved => "unresolved".to_string(),
            ResolutionStatus::Imputed { source, by } => {
                format!("imputed from {source} by {by}")
            }
            ResolutionStatus::Undeclared => "undeclared".to_string(),
        }
    }
}

/// The quantity a modality actually measures.
///
/// Vocabulary taken from section 28's own "native artifacts and metadata" lists. The list is
/// deliberately coarse: its job is to block the substitutions section 28 names as failure modes,
/// not to be a assay ontology. Where two modalities share a measurand — 28.12 and 28.14 both
/// report [`Measurand::ImageIntensity`] — the resolution axes and the failure modes carry the
/// difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Measurand {
    /// Accessibility, methylation fraction, or histone-mark occupancy (28.02).
    ChromatinState,
    /// Transcript abundance, as counts or a transformation of them (28.03, 28.04, 28.05).
    TranscriptAbundance,
    /// Peptide or protein abundance and modification-site occupancy (28.06).
    ProteinAbundance,
    /// Metabolite pool size or isotope-labelling pattern (28.07).
    MetabolitePool,
    /// A phenotype readout following a deliberate perturbation (28.08).
    PerturbationPhenotype,
    /// Atomic coordinates and per-residue confidence (28.09).
    AtomicCoordinates,
    /// Compound activity, exposure or toxicity against a target or system (28.10).
    CompoundActivity,
    /// Relative abundance of taxa or gene functions in a community (28.11).
    TaxonAbundance,
    /// Pixel or voxel intensity and the features derived from it (28.12, 28.14).
    ImageIntensity,
    /// A recorded clinical event, code, measurement or outcome (28.15).
    ClinicalEvent,
    /// A registry or protocol record: arms, criteria, endpoints, status (28.16).
    TrialRecord,
    /// An assertion made in a document, with its provenance (28.17).
    PublishedClaim,
    /// A phenotype measured in a model system, with the model's identity (28.18).
    ModelPhenotype,
    /// A dataset's existence, release, access tier and contents (28.20).
    DatasetRecord,
}

impl Measurand {
    pub fn as_str(self) -> &'static str {
        match self {
            Measurand::ChromatinState => "chromatin state",
            Measurand::TranscriptAbundance => "transcript abundance",
            Measurand::ProteinAbundance => "protein abundance",
            Measurand::MetabolitePool => "metabolite pool size",
            Measurand::PerturbationPhenotype => "perturbation phenotype",
            Measurand::AtomicCoordinates => "atomic coordinates",
            Measurand::CompoundActivity => "compound activity",
            Measurand::TaxonAbundance => "taxon relative abundance",
            Measurand::ImageIntensity => "image intensity",
            Measurand::ClinicalEvent => "recorded clinical event",
            Measurand::TrialRecord => "trial registry record",
            Measurand::PublishedClaim => "published claim",
            Measurand::ModelPhenotype => "model-system phenotype",
            Measurand::DatasetRecord => "dataset record",
        }
    }

    /// True when the value is a proportion of a fixed total rather than an absolute amount.
    ///
    /// 28.11 names "compositionality: relative abundance changes can be misleading" as a
    /// characteristic failure mode, and the same arithmetic applies to any closed-sum readout: a
    /// component can rise because it grew or because something else shrank. Keeping this as a
    /// property of the measurand rather than a note means a comparison can consult it.
    pub fn is_compositional(self) -> bool {
        matches!(self, Measurand::TaxonAbundance)
    }
}

impl fmt::Display for Measurand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether values from this modality arise from an intervention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDesign {
    /// The world was watched, not changed.
    Observational,
    /// A named target was deliberately altered and the consequence measured.
    Interventional,
    /// The modality carries both and the record must say which.
    ///
    /// 28.16 is the reason this variant exists: a registry snapshot holds randomised arms beside
    /// real-world comparator cohorts. The blueprint gives no rule for telling them apart from the
    /// modality alone, so the requirement goes to the caller rather than to a guess.
    PerRecord,
}

impl EvidenceDesign {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceDesign::Observational => "observational",
            EvidenceDesign::Interventional => "interventional",
            EvidenceDesign::PerRecord => "per-record",
        }
    }
}

impl fmt::Display for EvidenceDesign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One of section 28's characteristic failure modes, made checkable.
///
/// The `label` and `statement` are the blueprint's own words. What this crate adds is
/// [`FailureMode::trigger`]: the mechanical condition under which the failure has occurred, so
/// that a refusal can name the failure mode rather than leaving a reader to match a prose list
/// against an error string.
///
/// Not every failure mode in section 28 has a trigger this crate can check. "Publication bias"
/// (28.16) and "authority bias" (28.17) are properties of a search strategy, not of a single
/// claim, and they are recorded with [`FailureTrigger::NotMechanised`] rather than being given a
/// check that would only appear to work. A third group is checked, but somewhere other than the
/// support relation, and carries [`FailureTrigger::CheckedElsewhere`] naming where — collapsing
/// those into "not mechanised" would understate the crate as badly as the reverse overstates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureMode {
    /// The blueprint module that lists it, for example `"28.03"`.
    pub module: String,
    /// The bolded label from the blueprint's list, for example `"composition"`.
    pub label: String,
    /// The blueprint's one-sentence statement of the failure.
    pub statement: String,
    pub trigger: FailureTrigger,
}

impl FailureMode {
    pub fn new(
        module: impl Into<String>,
        label: impl Into<String>,
        statement: impl Into<String>,
        trigger: FailureTrigger,
    ) -> Self {
        FailureMode {
            module: module.into(),
            label: label.into(),
            statement: statement.into(),
            trigger,
        }
    }

    /// True when some function in this crate checks the failure mode.
    pub fn is_mechanised(&self) -> bool {
        !matches!(self.trigger, FailureTrigger::NotMechanised { .. })
    }

    /// True when [`crate::support::supports`] specifically is the check.
    pub fn is_checked_by_support_relation(&self) -> bool {
        !matches!(
            self.trigger,
            FailureTrigger::NotMechanised { .. } | FailureTrigger::CheckedElsewhere { .. }
        )
    }
}

/// The mechanical condition that constitutes a failure mode having occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum FailureTrigger {
    /// A claim of this kind was made from this modality.
    ///
    /// Covers the majority of section 28's lists: 28.03's "composition", 28.05's "resolution
    /// mismatch" and "interaction inference", 28.06's "RNA-protein equivalence", 28.07's
    /// "pool-versus-flux", 28.09's "docking overinterpretation".
    ClaimUnsupported { claim: String },

    /// A value at one resolution was counted as if it were independent at another.
    ///
    /// 28.03's "pseudoreplication", 28.04's "cell-level pseudoreplication", 28.12's "field
    /// pseudoreplication" and 28.14's "aggregation" are all this shape: the analysis unit and the
    /// independence unit are different axes and the analysis used the wrong one.
    ReplicationUnitConfusion {
        counted: Resolution,
        independent: Resolution,
    },

    /// A measurement of one quantity was used in place of another.
    MeasurandSubstitution { from: Measurand, to: Measurand },

    /// A transport's output was read as if it were an observation.
    ///
    /// 28.02's "reference drift" and 28.05's "coordinate mismatch" both land here: the value is
    /// downstream of an operation whose inputs were not declared.
    UndeclaredTransport { operation: String },

    /// Checked, but by something other than the support relation.
    ///
    /// Names the function that does the checking, so a reader can follow it. 28.17's "temporal
    /// leakage" is the clearest case: it is a real check, it just lives in
    /// [`crate::literature`] where the publication dates are.
    CheckedElsewhere { by: String },

    /// The blueprint names it, but it is a property of a process this crate does not model.
    ///
    /// Recorded rather than dropped, because a failure mode that is listed but unchecked is a
    /// limitation, while one that is silently absent reads as coverage.
    NotMechanised { reason: String },
}

/// Everything the crate knows about one assay family.
///
/// Constructed through the builder methods rather than by literal, so that the resolution map
/// cannot be built with two contradictory entries for one axis and so that undeclared axes stay
/// visibly undeclared instead of defaulting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityDescriptor {
    pub modality: Modality,
    pub measurand: Measurand,
    /// The blueprint's own purpose sentence, so a reader can check this crate against its source.
    pub purpose: String,
    pub design: EvidenceDesign,
    resolutions: BTreeMap<Resolution, ResolutionStatus>,
    failure_modes: Vec<FailureMode>,
    /// Numbers a decision from this modality needs that section 28 does not supply.
    ///
    /// Recorded as prose obligations on the caller. `crates/oncoworlds` established the rule this
    /// follows: a fabricated detection limit is worse than a missing one, because the missing one
    /// is visible.
    caller_supplied_constants: Vec<String>,
}

impl ModalityDescriptor {
    /// Starts a descriptor with every axis [`ResolutionStatus::Undeclared`].
    ///
    /// Starting from silence rather than from "unresolved" is deliberate: a half-written
    /// descriptor should refuse claims for lack of a declaration, not appear to have made one.
    pub fn new(
        modality: Modality,
        measurand: Measurand,
        purpose: impl Into<String>,
        design: EvidenceDesign,
    ) -> Self {
        ModalityDescriptor {
            modality,
            measurand,
            purpose: purpose.into(),
            design,
            resolutions: BTreeMap::new(),
            failure_modes: Vec::new(),
            caller_supplied_constants: Vec::new(),
        }
    }

    pub fn resolving(mut self, axis: Resolution) -> Self {
        self.resolutions.insert(axis, ResolutionStatus::Resolved);
        self
    }

    pub fn not_resolving(mut self, axis: Resolution) -> Self {
        self.resolutions.insert(axis, ResolutionStatus::Unresolved);
        self
    }

    pub fn with_status(mut self, axis: Resolution, status: ResolutionStatus) -> Self {
        self.resolutions.insert(axis, status);
        self
    }

    pub fn failing(mut self, mode: FailureMode) -> Self {
        self.failure_modes.push(mode);
        self
    }

    pub fn requiring_constant(mut self, what: impl Into<String>) -> Self {
        self.caller_supplied_constants.push(what.into());
        self
    }

    /// The status of one axis, defaulting to [`ResolutionStatus::Undeclared`].
    ///
    /// The default is the honest one: an axis nobody wrote down is undeclared, never unresolved.
    pub fn resolution(&self, axis: Resolution) -> ResolutionStatus {
        self.resolutions
            .get(&axis)
            .cloned()
            .unwrap_or(ResolutionStatus::Undeclared)
    }

    pub fn resolutions(&self) -> impl Iterator<Item = (Resolution, &ResolutionStatus)> {
        self.resolutions.iter().map(|(axis, status)| (*axis, status))
    }

    /// Axes the assay indexes values by.
    pub fn resolved_axes(&self) -> Vec<Resolution> {
        Resolution::ALL
            .into_iter()
            .filter(|axis| self.resolution(*axis).is_resolved())
            .collect()
    }

    /// Axes the descriptor states the assay does not index values by.
    ///
    /// The counterpart of [`ModalityDescriptor::resolved_axes`], and deliberately not its
    /// complement: an undeclared axis appears in neither.
    pub fn unresolved_axes(&self) -> Vec<Resolution> {
        Resolution::ALL
            .into_iter()
            .filter(|axis| matches!(self.resolution(*axis), ResolutionStatus::Unresolved))
            .collect()
    }

    pub fn undeclared_axes(&self) -> Vec<Resolution> {
        Resolution::ALL
            .into_iter()
            .filter(|axis| !self.resolution(*axis).is_declared())
            .collect()
    }

    /// True when every axis has a stated status.
    ///
    /// A complete descriptor is not a correct one, but an incomplete one cannot be argued with,
    /// and the catalogue in [`crate::catalog`] is tested for completeness.
    pub fn is_complete(&self) -> bool {
        self.undeclared_axes().is_empty()
    }

    pub fn failure_modes(&self) -> &[FailureMode] {
        &self.failure_modes
    }

    pub fn caller_supplied_constants(&self) -> &[String] {
        &self.caller_supplied_constants
    }

    /// Rejects a descriptor that declares one axis two different ways.
    ///
    /// The builder overwrites rather than accumulates, so this can only fire on a deserialised
    /// descriptor merged from two sources — which is exactly where it matters.
    pub fn check_consistent_with(&self, other: &ModalityDescriptor) -> Result<(), ModalityError> {
        for axis in Resolution::ALL {
            let mine = self.resolution(axis);
            let theirs = other.resolution(axis);
            if !mine.is_declared() || !theirs.is_declared() {
                continue;
            }
            if let Some(error) = contradictory(self.modality, axis, &mine, &theirs) {
                return Err(error);
            }
        }
        Ok(())
    }
}
