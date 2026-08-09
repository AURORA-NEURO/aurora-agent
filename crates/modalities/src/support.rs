//! The support relation: which claims a modality can carry.
//!
//! [`supports`] is the function the crate exists for. Asking bulk transcriptomics to support a
//! cell-level claim is not a weak result to be flagged in prose; it is a typed refusal naming the
//! resolution the assay does not have, and where section 28 already lists that mistake among a
//! module's characteristic failure modes, the refusal carries the module id and the blueprint's
//! own label for it.
//!
//! # The order of the checks, and why
//!
//! [`CHECK_ORDER`] runs measurand, then resolution, then design. The progression is from what a
//! value *is*, through what it is *about*, to what it *licenses*, and each later check is only
//! meaningful once the earlier one holds: telling a caller "you lack single-cell resolution" when
//! their number was protein abundance and the claim was about transcripts would send them to buy
//! the wrong instrument. Like [`bioprism_standards::comparable`], the first blocking dimension is
//! returned rather than all of them, for the same reason — one actionable complaint beats six
//! simultaneous ones, most of which are downstream of the first.
//!
//! # Imputed axes are admissible for some claims and circular for others
//!
//! Deconvolving bulk expression against a reference panel produces per-cell-type numbers. Using
//! them to state estimated composition is what deconvolution is *for*, and 28.03 lists
//! deconvolution among its benchmark decisions with the rider "with appropriate uncertainty".
//! Using them to argue that a change is cell-intrinsic is circular, because the cell-level
//! structure came from the reference panel rather than from the specimen — and 28.03 lists exactly
//! that as its "composition" failure mode. So [`ImputedAxisPolicy`] is a property of the claim,
//! not a global rule.
//!
//! # What is deliberately not here
//!
//! No statistics. Nothing in this module computes a power calculation, an effect size, a false
//! discovery rate or a confidence interval, so `supports` returning `Ok(())` means "this modality
//! is the right kind of instrument for this kind of claim", never "this claim is true" or "this
//! study was adequately powered". Several of section 28's failure modes — multiple testing,
//! signature overfitting, publication bias — are about evidence strength rather than about
//! instrument fit, and they are recorded as
//! [`crate::descriptor::FailureTrigger::NotMechanised`] rather than given a check that would only
//! appear to work.

use crate::catalog::descriptor;
use crate::descriptor::{
    EvidenceDesign, FailureMode, FailureTrigger, Measurand, Modality, ModalityDescriptor,
    Resolution, ResolutionStatus,
};
use crate::error::Unsupported;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The dimensions checked, in the order they are checked.
pub const CHECK_ORDER: &[&str] = &["measurand", "resolution", "evidence design"];

/// A kind of scientific statement, described by what it would take to be entitled to it.
///
/// The list is derived from section 28's "benchmark decisions" and "characteristic failure modes"
/// entries, which between them describe the claims the blueprint expects agents to make and the
/// ones it expects them to get wrong. It is not a taxonomy of biology; it is the smallest set of
/// distinctions that makes section 28's prohibitions checkable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// The average value over a bulk sample changed.
    PopulationAverage,
    /// The absolute amount of something changed, not merely its share of a fixed total.
    ///
    /// 28.11's "compositionality: relative abundance changes can be misleading". A closed-sum
    /// readout cannot distinguish a component growing from everything else shrinking, so the claim
    /// needs a measurand that is not compositional.
    AbsoluteAbundanceChange,
    /// A given cell is of a given type or state.
    CellIdentity,
    /// The fraction of cell types making up a sample changed.
    ///
    /// The claim deconvolution is for. Estimable from an imputed cell axis.
    CellComposition,
    /// A change is intrinsic to cells rather than a shift in which cells are present.
    ///
    /// 28.03's "composition" failure mode. Circular from an imputed cell axis.
    CellIntrinsicChange,
    /// A molecule is present at a stated place in the tissue.
    SpatialLocalization,
    /// Two cell populations are communicating.
    ///
    /// 28.05: "interaction inference: co-localization is treated as communication or causality".
    /// Co-location is observational; communication is a causal claim about a mechanism, so this
    /// crate requires an interventional design for it. That requirement is this crate's reading of
    /// the failure mode rather than a rule section 28 states in those words.
    CellCommunication,
    /// A protein is functionally active, not merely present or modified.
    ///
    /// 28.06's "PTM overreach: site detection is interpreted as functional activation without
    /// support". The support the blueprint gestures at is a functional experiment, so this crate
    /// requires a perturbation phenotype. Also this crate's reading rather than a stated rule.
    ProteinActivity,
    /// A metabolic reaction is carrying a given rate.
    ///
    /// 28.07's "pool-versus-flux: abundance changes are interpreted as flux changes". A pool size
    /// is a snapshot; a rate is a statement about change, so this requires a timepoint axis.
    FluxRate,
    /// A gene is required for a phenotype in the tested system.
    GeneDependency,
    /// An intervention caused an observed change.
    CausalEffectOfPerturbation,
    /// A compound binds a target.
    ///
    /// 28.09's "docking overinterpretation: a score is treated as binding or efficacy evidence".
    BindingAffinity,
    /// A compound reaches a stated concentration at a stated anatomical site.
    ///
    /// 28.10's "potency-versus-exposure: in vitro potency is interpreted without achievable brain
    /// or tumor exposure".
    ExposureAtSite,
    /// A microbial feature acts on host biology.
    ///
    /// 28.11's "causality: association is presented as host mechanism without intervention
    /// evidence".
    HostMechanism,
    /// A statement holds at the level of the whole subject.
    ///
    /// 28.14's "aggregation: strong patch metrics do not imply patient-level validity".
    SubjectLevelOutcome,
    /// A treatment caused a difference in outcome between groups.
    ///
    /// 28.16's "causal overreach: nonrandomized comparisons are treated as treatment effects".
    TreatmentEffect,
    /// Events occurred in a stated order in time.
    ///
    /// 28.04's "trajectory causality: pseudotime is treated as observed temporal or causal order".
    TemporalOrder,
    /// A finding in one species holds in another.
    ///
    /// 28.18's "cross-species equivalence: orthologs and phenotypes are assumed to be identical".
    /// No modality measures this, and saying so is the honest answer.
    CrossSpeciesEquivalence,
    /// A published source asserts something.
    ///
    /// Note what this claim is *about*: the paper, not the world. Getting from here to a claim
    /// about the world is [`crate::literature`]'s binding step.
    PublishedClaimSupport,
    /// A dataset exists, at a stated release, with stated contents and access tier.
    DatasetContent,
}

impl ClaimKind {
    pub const ALL: [ClaimKind; 20] = [
        ClaimKind::PopulationAverage,
        ClaimKind::AbsoluteAbundanceChange,
        ClaimKind::CellIdentity,
        ClaimKind::CellComposition,
        ClaimKind::CellIntrinsicChange,
        ClaimKind::SpatialLocalization,
        ClaimKind::CellCommunication,
        ClaimKind::ProteinActivity,
        ClaimKind::FluxRate,
        ClaimKind::GeneDependency,
        ClaimKind::CausalEffectOfPerturbation,
        ClaimKind::BindingAffinity,
        ClaimKind::ExposureAtSite,
        ClaimKind::HostMechanism,
        ClaimKind::SubjectLevelOutcome,
        ClaimKind::TreatmentEffect,
        ClaimKind::TemporalOrder,
        ClaimKind::CrossSpeciesEquivalence,
        ClaimKind::PublishedClaimSupport,
        ClaimKind::DatasetContent,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ClaimKind::PopulationAverage => "population-average",
            ClaimKind::AbsoluteAbundanceChange => "absolute-abundance-change",
            ClaimKind::CellIdentity => "cell-identity",
            ClaimKind::CellComposition => "cell-composition",
            ClaimKind::CellIntrinsicChange => "cell-intrinsic-change",
            ClaimKind::SpatialLocalization => "spatial-localization",
            ClaimKind::CellCommunication => "cell-communication",
            ClaimKind::ProteinActivity => "protein-activity",
            ClaimKind::FluxRate => "flux-rate",
            ClaimKind::GeneDependency => "gene-dependency",
            ClaimKind::CausalEffectOfPerturbation => "causal-effect-of-perturbation",
            ClaimKind::BindingAffinity => "binding-affinity",
            ClaimKind::ExposureAtSite => "exposure-at-site",
            ClaimKind::HostMechanism => "host-mechanism",
            ClaimKind::SubjectLevelOutcome => "subject-level-outcome",
            ClaimKind::TreatmentEffect => "treatment-effect",
            ClaimKind::TemporalOrder => "temporal-order",
            ClaimKind::CrossSpeciesEquivalence => "cross-species-equivalence",
            ClaimKind::PublishedClaimSupport => "published-claim-support",
            ClaimKind::DatasetContent => "dataset-content",
        }
    }

    /// What a modality must declare in order to carry this claim.
    pub fn requirements(self) -> ClaimRequirements {
        use ClaimKind::*;
        use ImputedAxisPolicy::{Circular, Estimable};
        use Measurand::*;
        use Resolution::*;
        match self {
            PopulationAverage => ClaimRequirements::new(&[Population], Circular),
            AbsoluteAbundanceChange => {
                ClaimRequirements::new(&[Population], Circular).measured_absolutely()
            }
            CellIdentity => ClaimRequirements::new(&[Cell], Circular),
            CellComposition => ClaimRequirements::new(&[Cell], Estimable),
            CellIntrinsicChange => ClaimRequirements::new(&[Cell], Circular),
            SpatialLocalization => ClaimRequirements::new(&[Location, Molecule], Circular),
            CellCommunication => ClaimRequirements::new(&[Location, Cell], Circular)
                .needing_design(EvidenceDesign::Interventional),
            ProteinActivity => ClaimRequirements::new(&[Perturbation], Circular)
                .measured_as(&[PerturbationPhenotype])
                .needing_design(EvidenceDesign::Interventional),
            FluxRate => ClaimRequirements::new(&[Molecule, Timepoint], Circular)
                .measured_as(&[MetabolitePool]),
            GeneDependency => ClaimRequirements::new(&[Perturbation], Circular)
                .measured_as(&[PerturbationPhenotype])
                .needing_design(EvidenceDesign::Interventional),
            CausalEffectOfPerturbation => ClaimRequirements::new(&[Perturbation], Circular)
                .needing_design(EvidenceDesign::Interventional),
            BindingAffinity => {
                ClaimRequirements::new(&[Molecule], Circular).measured_as(&[CompoundActivity])
            }
            ExposureAtSite => ClaimRequirements::new(&[Molecule, Location, Timepoint], Circular)
                .measured_as(&[CompoundActivity]),
            HostMechanism => ClaimRequirements::new(&[Subject], Circular)
                .needing_design(EvidenceDesign::Interventional),
            SubjectLevelOutcome => ClaimRequirements::new(&[Subject], Circular),
            TreatmentEffect => ClaimRequirements::new(&[Subject, Perturbation], Circular)
                .needing_design(EvidenceDesign::Interventional),
            TemporalOrder => ClaimRequirements::new(&[Timepoint], Circular),
            CrossSpeciesEquivalence => ClaimRequirements::new(&[], Circular).measured_as_nothing(
                "the same measurement made in both species, which is a study design rather than a modality",
            ),
            PublishedClaimSupport => {
                ClaimRequirements::new(&[], Circular).measured_as(&[PublishedClaim])
            }
            DatasetContent => ClaimRequirements::new(&[], Circular).measured_as(&[DatasetRecord]),
        }
    }
}

impl fmt::Display for ClaimKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether an axis a transport created may carry a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImputedAxisPolicy {
    /// The claim is *about* the estimate, so the imputed axis supports it.
    Estimable,
    /// The claim would use structure the imputation assumed as evidence for that structure.
    Circular,
}

/// What must be declared for a claim to be supportable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimRequirements {
    pub axes: Vec<Resolution>,
    pub measurand: MeasurandRequirement,
    /// `None` when the claim is descriptive and any design carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<EvidenceDesign>,
    pub imputed_axes: ImputedAxisPolicy,
}

impl ClaimRequirements {
    fn new(axes: &[Resolution], imputed_axes: ImputedAxisPolicy) -> Self {
        ClaimRequirements {
            axes: axes.to_vec(),
            measurand: MeasurandRequirement::Any,
            design: None,
            imputed_axes,
        }
    }

    fn measured_as(mut self, accepted: &[Measurand]) -> Self {
        self.measurand = MeasurandRequirement::OneOf(accepted.to_vec());
        self
    }

    fn measured_absolutely(mut self) -> Self {
        self.measurand = MeasurandRequirement::NotCompositional;
        self
    }

    fn measured_as_nothing(mut self, would_need: &'static str) -> Self {
        self.measurand = MeasurandRequirement::NoneInSection28 {
            would_need: would_need.to_string(),
        };
        self
    }

    fn needing_design(mut self, design: EvidenceDesign) -> Self {
        self.design = Some(design);
        self
    }
}

/// Which measurands can carry a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "measurand_requirement", rename_all = "snake_case")]
pub enum MeasurandRequirement {
    /// The claim is about a value, whatever it is a value of.
    Any,
    OneOf(Vec<Measurand>),
    /// Any measurand that is not a share of a fixed total.
    NotCompositional,
    /// Nothing in section 28 measures this.
    ///
    /// Worth a variant rather than an empty [`MeasurandRequirement::OneOf`], because the refusal
    /// should say what *would* establish the claim rather than enumerate seventeen things that do
    /// not.
    NoneInSection28 { would_need: String },
}

impl MeasurandRequirement {
    fn describe(&self) -> String {
        match self {
            MeasurandRequirement::Any => "any measured value".to_string(),
            MeasurandRequirement::OneOf(list) => list
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(" or "),
            MeasurandRequirement::NotCompositional => {
                "an absolute amount rather than a share of a fixed total".to_string()
            }
            MeasurandRequirement::NoneInSection28 { would_need } => would_need.clone(),
        }
    }

    fn admits(&self, measurand: Measurand) -> bool {
        match self {
            MeasurandRequirement::Any => true,
            MeasurandRequirement::OneOf(list) => list.contains(&measurand),
            MeasurandRequirement::NotCompositional => !measurand.is_compositional(),
            MeasurandRequirement::NoneInSection28 { .. } => false,
        }
    }
}

/// Whether a modality can support a claim, by its catalogue descriptor.
///
/// The convenience form of [`supports_descriptor`] for the seventeen modalities section 28
/// describes. A caller with a specific dataset — a longitudinal metabolomics series, a
/// microdialysis exposure study — should build a descriptor and use [`supports_descriptor`], since
/// the catalogue describes the modality in general and a study can resolve an axis the general
/// case does not.
pub fn supports(modality: Modality, claim: ClaimKind) -> Result<(), Unsupported> {
    supports_descriptor(&descriptor(modality), claim)
}

/// Whether a specific dataset's descriptor can support a claim.
pub fn supports_descriptor(
    descriptor: &ModalityDescriptor,
    claim: ClaimKind,
) -> Result<(), Unsupported> {
    let requirements = claim.requirements();
    let refusal = first_refusal(descriptor, claim, &requirements);
    match refusal {
        None => Ok(()),
        Some(refusal) => Err(name_failure_mode(descriptor, refusal)),
    }
}

fn first_refusal(
    descriptor: &ModalityDescriptor,
    claim: ClaimKind,
    requirements: &ClaimRequirements,
) -> Option<Unsupported> {
    if !requirements.measurand.admits(descriptor.measurand) {
        return Some(Unsupported::WrongMeasurand {
            modality: descriptor.modality,
            claim,
            measured: descriptor.measurand,
            required: requirements.measurand.describe(),
        });
    }
    for axis in Resolution::ALL {
        if !requirements.axes.contains(&axis) {
            continue;
        }
        match descriptor.resolution(axis) {
            ResolutionStatus::Resolved => {}
            ResolutionStatus::Unresolved => {
                return Some(Unsupported::MissingResolution {
                    modality: descriptor.modality,
                    claim,
                    axis,
                })
            }
            ResolutionStatus::Undeclared => {
                return Some(Unsupported::UndeclaredResolution {
                    modality: descriptor.modality,
                    claim,
                    axis,
                })
            }
            ResolutionStatus::Imputed { source, by } => {
                if requirements.imputed_axes == ImputedAxisPolicy::Circular {
                    return Some(Unsupported::ImputedResolution {
                        modality: descriptor.modality,
                        claim,
                        axis,
                        imputed_by: format!("{by} from {source}"),
                    });
                }
            }
        }
    }
    match (requirements.design, descriptor.design) {
        (Some(EvidenceDesign::Interventional), EvidenceDesign::Observational) => {
            Some(Unsupported::ObservationalOnly {
                modality: descriptor.modality,
                claim,
            })
        }
        (Some(EvidenceDesign::Interventional), EvidenceDesign::PerRecord) => {
            Some(Unsupported::DesignNotDeclared {
                modality: descriptor.modality,
                claim,
            })
        }
        _ => None,
    }
}

/// Wraps a mechanical refusal in the blueprint's own name for it, when the descriptor has one.
fn name_failure_mode(descriptor: &ModalityDescriptor, refusal: Unsupported) -> Unsupported {
    match descriptor
        .failure_modes()
        .iter()
        .find(|mode| trigger_matches(mode, descriptor.measurand, &refusal))
    {
        Some(mode) => Unsupported::NamedFailureMode {
            module: mode.module.clone(),
            label: mode.label.clone(),
            statement: mode.statement.clone(),
            inner: Box::new(refusal),
        },
        None => refusal,
    }
}

fn trigger_matches(mode: &FailureMode, measurand: Measurand, refusal: &Unsupported) -> bool {
    match (&mode.trigger, refusal) {
        (FailureTrigger::ClaimUnsupported { claim: named }, refusal) => {
            claim_of(refusal).is_some_and(|claim| claim.as_str() == named)
        }
        (
            FailureTrigger::ReplicationUnitConfusion { independent, .. },
            Unsupported::MissingResolution { axis, .. }
            | Unsupported::UndeclaredResolution { axis, .. }
            | Unsupported::PseudoReplication {
                independent: axis, ..
            },
        ) => independent == axis,
        (
            FailureTrigger::MeasurandSubstitution { from, to },
            Unsupported::WrongMeasurand { required, .. },
        ) => *from == measurand && required.contains(to.as_str()),
        (
            FailureTrigger::UndeclaredTransport { .. },
            Unsupported::ImputedResolution { .. },
        ) => true,
        _ => false,
    }
}

fn claim_of(refusal: &Unsupported) -> Option<ClaimKind> {
    match refusal {
        Unsupported::MissingResolution { claim, .. }
        | Unsupported::UndeclaredResolution { claim, .. }
        | Unsupported::ImputedResolution { claim, .. }
        | Unsupported::WrongMeasurand { claim, .. }
        | Unsupported::ObservationalOnly { claim, .. }
        | Unsupported::DesignNotDeclared { claim, .. } => Some(*claim),
        Unsupported::PseudoReplication { .. } => None,
        Unsupported::NamedFailureMode { inner, .. } => claim_of(inner),
    }
}

/// The axis along which values from this modality are independent, when it declares one.
///
/// Read off the [`FailureTrigger::ReplicationUnitConfusion`] entry in the descriptor's failure
/// modes. Four of section 28's seventeen modules carry that trigger — 28.03, 28.04, 28.12 and
/// 28.14 — and all four say the same thing in different words: the thing you have many of is not
/// the thing you have replicates of.
pub fn independent_unit(descriptor: &ModalityDescriptor) -> Option<Resolution> {
    descriptor.failure_modes().iter().find_map(|mode| match mode.trigger {
        FailureTrigger::ReplicationUnitConfusion { independent, .. } => Some(independent),
        _ => None,
    })
}

/// Whether values counted at `counted` may be treated as independent replicates.
///
/// Separate from [`supports`] because pseudoreplication is not a claim a modality cannot make; it
/// is an arithmetic mistake made while making a claim the modality *can* support. Digital
/// pathology genuinely supports a subject-level outcome claim — 28.14's "aggregation" failure mode
/// is not that patient-level claims are impossible, it is that patch-level counts do not stand in
/// for patient-level ones. So the check takes the unit the analysis counted and compares it to the
/// unit the descriptor says is independent.
pub fn analysis_unit(
    descriptor: &ModalityDescriptor,
    counted: Resolution,
) -> Result<(), Unsupported> {
    let Some(independent) = independent_unit(descriptor) else {
        return Ok(());
    };
    if counted == independent {
        return Ok(());
    }
    let refusal = Unsupported::PseudoReplication {
        modality: descriptor.modality,
        counted,
        independent,
    };
    Err(name_failure_mode(descriptor, refusal))
}

/// Every claim the modality's catalogue descriptor can carry.
///
/// Useful as a summary, and used by the test that pins the asymmetry between bulk and single-cell.
pub fn supported_claims(modality: Modality) -> Vec<ClaimKind> {
    ClaimKind::ALL
        .into_iter()
        .filter(|claim| supports(modality, *claim).is_ok())
        .collect()
}
