//! Typed refusals.
//!
//! Every refusal in this crate is a *value* before it is an error. Blueprint 30.03 names
//! "discarding mismatches rather than reporting them" as a characteristic failure, so a join
//! that cannot be made must be representable in a report, not merely thrown. The enums below are
//! therefore `Serialize`, and each module returns them in `Result::Err` positions where the
//! caller asked for a thing that does not exist, and inside report structs where the caller asked
//! *whether* the thing exists.
//!
//! [`OncoWorldsError`] exists only so that a caller composing several modules can use `?` across
//! them. Nothing in this crate matches on it.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Why two artefacts may not be joined (30.03).
///
/// Ordered by [`crate::identity::JOIN_CHECK_ORDER`] and reported first-blocking, following
/// `bioprism_standards::comparability`: a caller told "these are different patients" has
/// something to do, whereas a caller handed five simultaneous complaints has to guess which
/// matters, and the later checks are meaningless until the earlier one is resolved. Comparing
/// specimen epochs across two different patients would be answering a question nobody asked.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum JoinRefusal {
    #[error("artefacts belong to different participants ({left} vs {right})")]
    DifferentParticipant { left: String, right: String },
    #[error("identifier {short} is a truncation of {long}, not a match")]
    TruncatedIdentifier { short: String, long: String },
    #[error("artefacts describe different lesions ({left} vs {right})")]
    DifferentLesion { left: String, right: String },
    #[error("artefacts sit in different disease epochs ({left} vs {right}); {detail}")]
    IncompatibleEpoch {
        left: String,
        right: String,
        detail: String,
    },
    #[error("artefacts come from different specimens ({left} vs {right}) and the requested unit is specimen-level")]
    DifferentSpecimen { left: String, right: String },
    #[error("no identity evidence links {left} to {right}")]
    NoIdentityEvidence { left: String, right: String },
    #[error("identity relation between {left} and {right} is {relation}, which does not license a join")]
    UnlicensedRelation {
        left: String,
        right: String,
        relation: String,
    },
    #[error("identity link between {left} and {right} declares no permissible use")]
    UndeclaredPermissibleUse { left: String, right: String },
    #[error("regional tissue {specimen} has no regional provenance, so it cannot be aligned to image region {region}")]
    NoRegionalProvenance { specimen: String, region: String },
    #[error("the tissue and image coordinates are not comparable: {detail}")]
    IncomparableCoordinates { detail: String },
}

/// Why a specimen-level observation may not be restated as a tumour-level one (30.12).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum PromotionRefusal {
    #[error("the assay's limit of detection is undeclared, so a negative result bounds nothing")]
    UndeclaredSensitivity,
    #[error("no region of the tumour was sampled, so there is no specimen to reason from")]
    NoRegionSampled,
    #[error("{marker} was not measured in this specimen ({status}); that is silence, not absence")]
    NotAnAbsence { marker: String, status: String },
    #[error("converting variant allele fraction to cellular fraction needs purity, local copy number and multiplicity; {missing} is undeclared")]
    CopyNumberUnknown { missing: String },
}

/// Why a number is not a fraction of a whole (30.12).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum FractionError {
    #[error("{parts} parts per ten thousand is more than the whole")]
    AboveWhole { parts: u32 },
    #[error("ratio {value} is not a finite number in [0, 1]")]
    NotAUnitRatio { value: String },
}

/// Why a set of subclone observations does not determine one history (30.12).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum PhylogenyRefusal {
    #[error("subclone fractions sum to {total}, which exceeds the tumour")]
    FractionsExceedWhole { total: String },
    #[error("subclone {child} has fraction {child_fraction} but its parent {parent} has {parent_fraction}")]
    ChildExceedsParent {
        parent: String,
        child: String,
        parent_fraction: String,
        child_fraction: String,
    },
    #[error("ancestry edges contain a cycle through {subclone}")]
    Cyclic { subclone: String },
    #[error("subclone {subclone} appears in an edge but not in the population")]
    UnknownSubclone { subclone: String },
    #[error("{count} histories are compatible with the observations; committing to one is unsupported")]
    Ambiguous { count: usize },
    #[error("temporal association between {treatment} and {alteration} does not establish causation")]
    UnsupportedDirectionality {
        treatment: String,
        alteration: String,
    },
}

/// Why a methylation result may not be read the way the caller asked (30.11).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum MethylationRefusal {
    #[error("classifier {classifier} declares no reporting threshold, so no class can be emitted")]
    UndeclaredThreshold { classifier: String },
    #[error("score {score} is outside the unit interval")]
    ScoreOutOfRange { score: String },
    #[error("scores from {left} and {right} are uncalibrated and cross-version comparison of raw scores is undefined")]
    UncalibratedCrossVersion { left: String, right: String },
    #[error("copy-number profile derived from the same array as the class call is not independent corroboration of that call")]
    CircularCopyNumber,
    #[error("the classifier label is already used as {existing_use} in this analysis and cannot also be used as {requested_use}")]
    CircularLabelUse {
        existing_use: String,
        requested_use: String,
    },
    #[error("the sample is unclassifiable ({reason}); there is no nearest class to fall back to")]
    Unclassifiable { reason: String },
}

/// Why a claim may not cross a modality, a model system or a scope (30.08, 30.19).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum TransportRefusal {
    #[error("the transport from {from} to {to} declares no loss; a lossless cross-scope move is a modelling error")]
    UndeclaredLoss { from: String, to: String },
    #[error("assumption {assumption} is required by this transport and is not stated")]
    UnstatedAssumption { assumption: String },
    #[error("the evaluation splits on {unit}, which puts material from one participant on both sides")]
    LeakySplit { unit: String },
    #[error("the claim asserts {target} but the evidence is stratified only by {available}")]
    UnstratifiedClaim { target: String, available: String },
    #[error("the target label describes the specimen, not the tumour; promoting it needs the sampling argument of 30.12")]
    SpecimenScopedTarget { detail: String },
    #[error("the external cohort {cohort} was chosen after results were seen")]
    PostHocCohortSelection { cohort: String },
    #[error("model {model} has no identity or contamination check against its source specimen {specimen}")]
    UnverifiedModelIdentity { model: String, specimen: String },
    #[error("{established} of {attempted} specimens established a model and establishment selection is unmodelled")]
    UnmodelledEstablishmentSelection { attempted: usize, established: usize },
    #[error("{wells} technical wells are not {claimed} biological replicates")]
    TechnicalReplicatesAsBiological { wells: usize, claimed: usize },
    #[error("fidelity across {axis} is unmeasured for this model, so the transport is undeclared on that axis")]
    UnmeasuredFidelity { axis: String },
}

/// Why two cohorts may not be compared across a site, population or era boundary (30.27).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ShiftRefusal {
    #[error("cohorts were classified under {left} and {right} and no entity mapping between them is stated")]
    UnmappedClassificationChange { left: String, right: String },
    #[error("the stated mapping does not cover entity {entity} of {version}")]
    IncompleteMapping { entity: String, version: String },
    #[error("{descriptor} is a social and administrative descriptor; using it as {use_} asserts a biology it does not carry")]
    DescriptorUsedAsMechanism { descriptor: String, use_: String },
    #[error("assay {assay} is unavailable at {site}; that absence is a resource fact, not a negative result")]
    ResourceAbsenceReadAsBiology { assay: String, site: String },
    #[error("a pooled score cannot support an equity claim without the per-subgroup breakdown")]
    PooledScoreOnly,
    #[error("subgroup {subgroup} has {n} cases and the claim states no uncertainty interval")]
    UnquantifiedSubgroup { subgroup: String, n: usize },
    #[error("subgroup {subgroup} has no cases")]
    EmptySubgroup { subgroup: String },
}

/// Why an entity-world operation was refused (30.20–30.24).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum EntityWorldRefusal {
    #[error("pooling {left} with {right} material without modelling the selection between them is unsupported")]
    UnmodelledProvenanceSelection { left: String, right: String },
    #[error("alterations {left} and {right} share a pathway but differ in mechanism; pooling them needs a stated estimand")]
    MechanismCollapse { left: String, right: String },
    #[error("a macro-averaged score without per-class case counts hides the rare classes it averages over")]
    MacroScoreWithoutCounts,
    #[error("no reliable benchmark can be formed: {reason}")]
    BenchmarkInfeasible { reason: String },
    #[error("{lesions} lesions come from {participants} participants; a lesion-level analysis must declare the participant cluster")]
    UndeclaredCluster { lesions: usize, participants: usize },
    #[error("{event} is a competing event for this endpoint and cannot be treated as censoring")]
    CompetingEventAsCensoring { event: String },
}

/// Umbrella error, for callers composing several modules with `?`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OncoWorldsError {
    #[error(transparent)]
    Join(#[from] JoinRefusal),
    #[error(transparent)]
    Promotion(#[from] PromotionRefusal),
    #[error(transparent)]
    Fraction(#[from] FractionError),
    #[error(transparent)]
    Phylogeny(#[from] PhylogenyRefusal),
    #[error(transparent)]
    Methylation(#[from] MethylationRefusal),
    #[error(transparent)]
    Transport(#[from] TransportRefusal),
    #[error(transparent)]
    Shift(#[from] ShiftRefusal),
    #[error(transparent)]
    EntityWorld(#[from] EntityWorldRefusal),
}
