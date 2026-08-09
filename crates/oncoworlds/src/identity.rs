//! The identity spine: participant, specimen, block, section, assay run, image series (30.03).
//!
//! Blueprint 30.03 exists to "prevent cross-modal analyses from joining artifacts that belong to
//! different people, lesions, regions, time points, or treatment states". Everything else in this
//! crate hangs off it: a methylation class is a class *of a section of a block of a specimen*, a
//! radiogenomic pair is a pair *of one participant's series and one participant's library*, and a
//! subclone fraction is a fraction *in the fragment that reached the sequencer*.
//!
//! # These are different scopes, not different labels
//!
//! `bioprism_scope` already has the vocabulary — [`ScopeClass::Identity`], [`ScopeClass::Region`],
//! [`ScopeClass::Specimen`] — and [`DimensionRegistry`] already carries `patient`, `lesion`,
//! `specimen`, `block` and `aliquot`. This module extends that registry with the levels 30.03
//! names and that the default table omits ([`onco_dimension_registry`]); it does not build a
//! parallel taxonomy. An [`Artifact`]'s [`Artifact::scope_key`] is a genuine
//! [`ScopeKey`], so a join refusal here and a scope refusal in `bioprism-scope` speak about the
//! same object.
//!
//! Two measurements on different specimens from one participant are therefore not interchangeable,
//! and [`joinable`] says which dimension blocks: it returns the *first* refusal in
//! [`JOIN_CHECK_ORDER`], the same first-blocking discipline
//! `bioprism_standards::comparability::comparable` uses. Reporting "different specimens" alongside
//! "different participants" would be answering a question nobody asked, because specimen
//! identifiers are only meaningful within a participant.
//!
//! # A declined join is a result
//!
//! 30.03 names "discarding mismatches rather than reporting them" as a characteristic failure, so
//! [`JoinReport`] carries [`JoinVerdict::Declined`] with its reason and is `Serialize`. Task 6 of
//! the module's ladder — "decline a multimodal join whose identity evidence is insufficient" — is
//! a success condition, not an error path.
//!
//! # Not implemented
//!
//! No repository connectors, no fingerprint or sex-chromosome concordance computation, no
//! record-linkage model, no de-identification. [`LinkBasis`] names which oracle in 30.03's oracle
//! mesh asserted a link; running that oracle is somebody else's job. [`LinkConfidence`] is an
//! ordinal declared by whoever produced the crosswalk — this crate ships no calibration for it,
//! because 30.03 specifies "uncertain-link calibration" as a metric and supplies no scale.

use crate::error::JoinRefusal;
use bioprism_scope::{DimensionRegistry, ScopeClass, ScopeKey};
use bioprism_standards::{comparable, Measurement, Position};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A pseudonymous identifier (30.03, "participant and encounter pseudonyms").
///
/// The only reason this is a newtype rather than a `String` is [`Pseudonym::is_truncation_of`].
/// "Joining by a truncated identifier" is the first characteristic failure 30.03 lists, and it
/// happens because `==` on prefixes is one careless `starts_with` away. Equality here is exact and
/// truncation is a separate, named question.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Pseudonym(String);

impl Pseudonym {
    pub fn new(value: impl Into<String>) -> Self {
        Pseudonym(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when `self` is a strict prefix of `longer`.
    ///
    /// Deliberately not `PartialEq`, and deliberately not used by [`joinable`] to *accept* a
    /// match: a truncation is evidence that two systems disagree about identifier width, which is
    /// a mismatch to report, not a link to make.
    pub fn is_truncation_of(&self, longer: &Pseudonym) -> bool {
        !self.0.is_empty() && self.0.len() < longer.0.len() && longer.0.starts_with(&self.0)
    }
}

/// A level of the lineage 30.03 requires: "block, slide, core, aliquot, library, and assay
/// lineage" and "imaging study, series, segmentation, and derived-feature lineage".
///
/// Declaration order is root-first so that a `BTreeMap` keyed on this type iterates a lineage from
/// the participant down. [`ArtifactLevel::parent`] is a tree, not a total order: a section and an
/// aliquot are both children of their upstream container and neither contains the other, which is
/// why `Ord` here is an iteration convenience and never a containment test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactLevel {
    Participant,
    Encounter,
    Specimen,
    Block,
    Section,
    Core,
    Aliquot,
    Library,
    AssayRun,
    ImagingStudy,
    ImagingSeries,
    Segmentation,
    DerivedFeature,
}

impl ArtifactLevel {
    /// The containing level, or `None` for the root.
    pub const fn parent(self) -> Option<ArtifactLevel> {
        use ArtifactLevel::*;
        match self {
            Participant => None,
            Encounter => Some(Participant),
            Specimen => Some(Encounter),
            Block => Some(Specimen),
            Section | Core => Some(Block),
            Aliquot => Some(Specimen),
            Library => Some(Aliquot),
            AssayRun => Some(Library),
            ImagingStudy => Some(Encounter),
            ImagingSeries => Some(ImagingStudy),
            Segmentation => Some(ImagingSeries),
            DerivedFeature => Some(Segmentation),
        }
    }

    /// The scope dimension this level binds.
    pub const fn dimension(self) -> &'static str {
        use ArtifactLevel::*;
        match self {
            Participant => "patient",
            Encounter => "encounter",
            Specimen => "specimen",
            Block => "block",
            Section => "section",
            Core => "core",
            Aliquot => "aliquot",
            Library => "library",
            AssayRun => "assay_run",
            ImagingStudy => "imaging_study",
            ImagingSeries => "imaging_series",
            Segmentation => "segmentation",
            DerivedFeature => "derived_feature",
        }
    }

    /// Which of `bioprism_scope`'s protected classes this level belongs to.
    ///
    /// A segmentation and a derived feature are [`ScopeClass::Region`] rather than
    /// [`ScopeClass::Specimen`]: what they identify is a delineated part of an image, and 30.03
    /// keeps "lesion and anatomical-region identifiers" separate from specimen containers.
    pub const fn scope_class(self) -> ScopeClass {
        use ArtifactLevel::*;
        match self {
            Participant | Encounter | ImagingStudy | ImagingSeries => ScopeClass::Identity,
            Specimen | Block | Section | Core | Aliquot | Library | AssayRun => {
                ScopeClass::Specimen
            }
            Segmentation | DerivedFeature => ScopeClass::Region,
        }
    }

    pub const ALL: [ArtifactLevel; 13] = [
        ArtifactLevel::Participant,
        ArtifactLevel::Encounter,
        ArtifactLevel::Specimen,
        ArtifactLevel::Block,
        ArtifactLevel::Section,
        ArtifactLevel::Core,
        ArtifactLevel::Aliquot,
        ArtifactLevel::Library,
        ArtifactLevel::AssayRun,
        ArtifactLevel::ImagingStudy,
        ArtifactLevel::ImagingSeries,
        ArtifactLevel::Segmentation,
        ArtifactLevel::DerivedFeature,
    ];
}

/// `bioprism_scope`'s default registry, extended with the levels 30.03 names.
///
/// The default table already classifies `patient`, `specimen`, `block`, `aliquot` and `lesion`;
/// this adds the rest of the tissue and imaging chains plus `treatment_epoch` and
/// `classification_version`. Extension, not replacement: reclassifying a canonical dimension is
/// rejected by the registry itself and this function never attempts it.
pub fn onco_dimension_registry() -> DimensionRegistry {
    let mut registry = DimensionRegistry::default();
    for level in ArtifactLevel::ALL {
        let _ = registry.register(level.dimension(), level.scope_class());
    }
    let _ = registry.register("treatment_epoch", ScopeClass::Time);
    let _ = registry.register("classification_version", ScopeClass::Ontology);
    let _ = registry.register("feature_version", ScopeClass::Ontology);
    registry
}

/// The disease epoch an artefact belongs to.
///
/// The four phases are 30.23's — "preoperative, postoperative, treatment, and recurrence imaging",
/// with "diagnosis and recurrence specimens" — read as the treatment states 30.03 asks joins to
/// respect. No duration, no window and no ordering is attached, because none is specified
/// anywhere in the section and a fabricated interval would be exactly the invented clinical fact
/// this crate must not contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiseaseEpoch {
    Preoperative,
    Postoperative,
    OnTreatment,
    Recurrence,
}

impl DiseaseEpoch {
    pub const fn as_str(self) -> &'static str {
        match self {
            DiseaseEpoch::Preoperative => "preoperative",
            DiseaseEpoch::Postoperative => "postoperative",
            DiseaseEpoch::OnTreatment => "on_treatment",
            DiseaseEpoch::Recurrence => "recurrence",
        }
    }
}

/// A stated argument for pairing artefacts from two different epochs.
///
/// There is no compatibility matrix over [`DiseaseEpoch`] in this crate, and inventing one would
/// mean asserting, for instance, that postoperative and on-treatment material is interchangeable —
/// a clinical claim the blueprint does not make. Epochs are compatible when equal; anything else
/// needs a bridge whose `warrant` a reader can disagree with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochBridge {
    pub from: DiseaseEpoch,
    pub to: DiseaseEpoch,
    /// Why material from `from` may be paired with material from `to` for the stated purpose.
    pub warrant: String,
}

/// Whether a tissue artefact records where in the tumour it came from (30.03).
///
/// "Aligning tissue to a whole-tumor image without regional provenance" is a named failure, so
/// [`RegionProvenance::WholeTumour`] and [`RegionProvenance::Unrecorded`] are distinct: the first
/// says the specimen deliberately represents no single region, the second says nobody wrote it
/// down. Neither supports a regional alignment, and they fail for different reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum RegionProvenance {
    Region { lesion: Pseudonym, region: Pseudonym },
    WholeTumour,
    Unrecorded,
}

/// An artefact and its lineage.
///
/// The lineage is a map from level to identifier; [`Artifact::lineage_gaps`] reports levels whose
/// parent is absent. A gap is not an error at construction — 30.03 requires that incomplete
/// lineage be *reported* ("lineage completeness" is a primary metric), so an artefact with a gap
/// exists and is inspectable rather than being unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub label: String,
    lineage: BTreeMap<ArtifactLevel, Pseudonym>,
    pub lesion: Option<Pseudonym>,
    pub region: RegionProvenance,
    pub epoch: DiseaseEpoch,
}

impl Artifact {
    pub fn new(label: impl Into<String>, participant: Pseudonym, epoch: DiseaseEpoch) -> Self {
        let mut lineage = BTreeMap::new();
        lineage.insert(ArtifactLevel::Participant, participant);
        Artifact {
            label: label.into(),
            lineage,
            lesion: None,
            region: RegionProvenance::Unrecorded,
            epoch,
        }
    }

    pub fn at(mut self, level: ArtifactLevel, id: Pseudonym) -> Self {
        self.lineage.insert(level, id);
        self
    }

    pub fn in_lesion(mut self, lesion: Pseudonym) -> Self {
        self.lesion = Some(lesion);
        self
    }

    pub fn with_region(mut self, region: RegionProvenance) -> Self {
        self.region = region;
        self
    }

    pub fn id_at(&self, level: ArtifactLevel) -> Option<&Pseudonym> {
        self.lineage.get(&level)
    }

    pub fn participant(&self) -> &Pseudonym {
        self.lineage
            .get(&ArtifactLevel::Participant)
            .expect("participant is inserted by the constructor and never removed")
    }

    /// The deepest level present, which is what this artefact *is*.
    pub fn level(&self) -> ArtifactLevel {
        self.lineage
            .keys()
            .copied()
            .max_by_key(|level| depth(*level))
            .unwrap_or(ArtifactLevel::Participant)
    }

    /// Levels present whose containing level is absent.
    pub fn lineage_gaps(&self) -> Vec<ArtifactLevel> {
        self.lineage
            .keys()
            .copied()
            .filter(|level| level.parent().is_some_and(|p| !self.lineage.contains_key(&p)))
            .collect()
    }

    /// The scope this artefact is valid in, as a real [`ScopeKey`].
    pub fn scope_key(&self) -> ScopeKey {
        let mut key = ScopeKey::new();
        for (level, id) in &self.lineage {
            key = key.exact(level.dimension(), id.as_str());
        }
        if let Some(lesion) = &self.lesion {
            key = key.exact("lesion", lesion.as_str());
        }
        key.exact("treatment_epoch", self.epoch.as_str())
    }
}

fn depth(level: ArtifactLevel) -> usize {
    let mut depth = 0;
    let mut cursor = level;
    while let Some(parent) = cursor.parent() {
        depth += 1;
        cursor = parent;
    }
    depth
}

/// Which oracle in 30.03's oracle mesh asserted an identity link.
///
/// This crate runs none of them. The variant records provenance so that
/// "no single oracle is presumed infallible" is expressible in a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkBasis {
    AuthoritativeCrosswalk,
    GenotypeOrFingerprintConcordance,
    SexChromosomeConsistency,
    CopyNumberConsistency,
    AcquisitionTimestamp,
    ExpertSpecimenMap,
}

/// The identity relationships 30.03 requires: "duplicate, related, pooled, and uncertain".
///
/// `Same` is the only one that licenses a join without further argument. `Pooled` is the
/// interesting one: pooled material *is* the participant's, and is also somebody else's, so it
/// can neither be joined as that participant nor dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRelation {
    Same,
    Duplicate,
    Related,
    Pooled,
    Uncertain,
}

impl IdentityRelation {
    pub const fn as_str(self) -> &'static str {
        match self {
            IdentityRelation::Same => "same",
            IdentityRelation::Duplicate => "duplicate",
            IdentityRelation::Related => "related",
            IdentityRelation::Pooled => "pooled",
            IdentityRelation::Uncertain => "uncertain",
        }
    }

    /// Whether this relation, on its own, licenses treating two artefacts as one participant's.
    pub const fn licenses_join(self) -> bool {
        matches!(self, IdentityRelation::Same | IdentityRelation::Duplicate)
    }

    /// Whether two artefacts under this relation may be counted as independent observations.
    ///
    /// A duplicate links two artefacts *and* destroys their independence, which is why
    /// [`IdentityRelation::licenses_join`] and this function disagree on it. 30.03 lists
    /// "duplicate handling" and "effective independent sample count" as separate metrics for the
    /// same reason.
    pub const fn independent(self) -> bool {
        matches!(self, IdentityRelation::Related)
    }
}

/// An ordinal confidence declared by whoever produced the crosswalk.
///
/// Not a probability: 30.03 asks for "uncertain-link calibration" as a metric and specifies no
/// scale, so attaching numbers here would be this crate inventing the calibration it is supposed
/// to be measured on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkConfidence {
    Asserted,
    Probable,
    Uncertain,
}

/// What a crosswalk permits the link to be used for (30.03, "crosswalks with confidence and
/// permissible use").
///
/// An empty set is meaningful and is refused by [`joinable`]: a link that permits nothing is not
/// a link, and silently reading it as permitting everything is the failure this field exists to
/// prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissibleUse {
    CohortConstruction,
    MultimodalJoin,
    OutcomeLinkage,
}

/// One asserted relationship between two identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityLink {
    pub left: Pseudonym,
    pub right: Pseudonym,
    pub relation: IdentityRelation,
    pub basis: LinkBasis,
    pub confidence: LinkConfidence,
    pub permitted: BTreeSet<PermissibleUse>,
}

impl IdentityLink {
    pub fn new(
        left: Pseudonym,
        right: Pseudonym,
        relation: IdentityRelation,
        basis: LinkBasis,
        confidence: LinkConfidence,
    ) -> Self {
        IdentityLink {
            left,
            right,
            relation,
            basis,
            confidence,
            permitted: BTreeSet::new(),
        }
    }

    pub fn permitting(mut self, use_: PermissibleUse) -> Self {
        self.permitted.insert(use_);
        self
    }

    fn covers(&self, a: &Pseudonym, b: &Pseudonym) -> bool {
        (&self.left == a && &self.right == b) || (&self.left == b && &self.right == a)
    }
}

/// The crosswalks available when deciding a join.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityEvidence {
    links: Vec<IdentityLink>,
}

impl IdentityEvidence {
    pub fn new() -> Self {
        IdentityEvidence::default()
    }

    pub fn with(mut self, link: IdentityLink) -> Self {
        self.links.push(link);
        self
    }

    pub fn link_between(&self, a: &Pseudonym, b: &Pseudonym) -> Option<&IdentityLink> {
        self.links.iter().find(|link| link.covers(a, b))
    }

    pub fn links(&self) -> &[IdentityLink] {
        &self.links
    }
}

/// The unit an analysis treats as one observation (30.03 ladder item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisUnit {
    Participant,
    Lesion,
    Specimen,
    ImagingSeries,
}

impl AnalysisUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            AnalysisUnit::Participant => "participant",
            AnalysisUnit::Lesion => "lesion",
            AnalysisUnit::Specimen => "specimen",
            AnalysisUnit::ImagingSeries => "imaging_series",
        }
    }
}

/// The dimensions [`joinable`] checks, in the order it checks them.
pub const JOIN_CHECK_ORDER: &[&str] = &[
    "participant identity",
    "identifier width",
    "identity evidence",
    "relation licence",
    "permissible use",
    "lesion identity",
    "disease epoch",
    "specimen identity",
];

/// The dimensions [`align_regional_position`] checks after the provenance check, in order.
///
/// Delegated wholesale to `bioprism_standards::comparability::CHECK_ORDER`, because a
/// tissue-to-image alignment is a comparison of two located points and that crate already decides
/// what blocks one.
pub const COORDINATE_CHECK_ORDER: &[&str] = bioprism_standards::CHECK_ORDER;

/// Whether two artefacts may be joined at `unit`, or the first dimension that blocks.
///
/// The participant check runs before everything because specimen, lesion and epoch identifiers are
/// only meaningful inside a participant: "specimen S1 vs specimen S1" across two people is a
/// coincidence of local numbering, and reporting it as agreement would be false in the same way
/// `bioprism_standards` says `chr7` in two builds is false.
pub fn joinable(
    left: &Artifact,
    right: &Artifact,
    unit: AnalysisUnit,
    evidence: &IdentityEvidence,
) -> Result<(), JoinRefusal> {
    joinable_with_bridge(left, right, unit, evidence, None)
}

/// [`joinable`], with a stated argument for crossing an epoch boundary.
pub fn joinable_with_bridge(
    left: &Artifact,
    right: &Artifact,
    unit: AnalysisUnit,
    evidence: &IdentityEvidence,
    bridge: Option<&EpochBridge>,
) -> Result<(), JoinRefusal> {
    check_participant(left, right, evidence)?;
    check_lesion(left, right, unit)?;
    check_epoch(left, right, bridge)?;
    check_specimen(left, right, unit)
}

fn check_participant(
    left: &Artifact,
    right: &Artifact,
    evidence: &IdentityEvidence,
) -> Result<(), JoinRefusal> {
    let (a, b) = (left.participant(), right.participant());
    if a == b {
        return Ok(());
    }
    if a.is_truncation_of(b) {
        return Err(JoinRefusal::TruncatedIdentifier {
            short: a.as_str().to_string(),
            long: b.as_str().to_string(),
        });
    }
    if b.is_truncation_of(a) {
        return Err(JoinRefusal::TruncatedIdentifier {
            short: b.as_str().to_string(),
            long: a.as_str().to_string(),
        });
    }
    let Some(link) = evidence.link_between(a, b) else {
        return Err(JoinRefusal::NoIdentityEvidence {
            left: a.as_str().to_string(),
            right: b.as_str().to_string(),
        });
    };
    if !link.relation.licenses_join() {
        return Err(JoinRefusal::UnlicensedRelation {
            left: a.as_str().to_string(),
            right: b.as_str().to_string(),
            relation: link.relation.as_str().to_string(),
        });
    }
    if !link.permitted.contains(&PermissibleUse::MultimodalJoin) {
        return Err(JoinRefusal::UndeclaredPermissibleUse {
            left: a.as_str().to_string(),
            right: b.as_str().to_string(),
        });
    }
    Ok(())
}

fn check_lesion(left: &Artifact, right: &Artifact, unit: AnalysisUnit) -> Result<(), JoinRefusal> {
    if unit != AnalysisUnit::Lesion {
        return Ok(());
    }
    match (&left.lesion, &right.lesion) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (Some(a), Some(b)) => Err(JoinRefusal::DifferentLesion {
            left: a.as_str().to_string(),
            right: b.as_str().to_string(),
        }),
        _ => Err(JoinRefusal::DifferentLesion {
            left: describe_lesion(&left.lesion),
            right: describe_lesion(&right.lesion),
        }),
    }
}

fn describe_lesion(lesion: &Option<Pseudonym>) -> String {
    lesion
        .as_ref()
        .map(|id| id.as_str().to_string())
        .unwrap_or_else(|| "<no lesion identifier>".to_string())
}

fn check_epoch(
    left: &Artifact,
    right: &Artifact,
    bridge: Option<&EpochBridge>,
) -> Result<(), JoinRefusal> {
    if left.epoch == right.epoch {
        return Ok(());
    }
    let bridged = bridge.is_some_and(|bridge| {
        (bridge.from == left.epoch && bridge.to == right.epoch)
            || (bridge.from == right.epoch && bridge.to == left.epoch)
    });
    if bridged {
        return Ok(());
    }
    Err(JoinRefusal::IncompatibleEpoch {
        left: left.epoch.as_str().to_string(),
        right: right.epoch.as_str().to_string(),
        detail: "epochs are compatible only when equal or bridged by a stated warrant".to_string(),
    })
}

fn check_specimen(left: &Artifact, right: &Artifact, unit: AnalysisUnit) -> Result<(), JoinRefusal> {
    if unit != AnalysisUnit::Specimen {
        return Ok(());
    }
    match (
        left.id_at(ArtifactLevel::Specimen),
        right.id_at(ArtifactLevel::Specimen),
    ) {
        (Some(a), Some(b)) if a == b => Ok(()),
        (a, b) => Err(JoinRefusal::DifferentSpecimen {
            left: a.map(|id| id.as_str().to_string()).unwrap_or_else(|| "<none>".into()),
            right: b.map(|id| id.as_str().to_string()).unwrap_or_else(|| "<none>".into()),
        }),
    }
}

/// Whether tissue may be aligned to a delineated image region (30.03 ladder item 5).
///
/// Refused for both [`RegionProvenance::WholeTumour`] and [`RegionProvenance::Unrecorded`]. The
/// failure mode 30.03 names — "aligning tissue to a whole-tumor image without regional
/// provenance" — is the case where a fragment's measurement is attributed to a delineated region
/// it may not have come from.
pub fn align_to_image_region(
    tissue: &Artifact,
    image_region: &Pseudonym,
) -> Result<(), JoinRefusal> {
    match &tissue.region {
        RegionProvenance::Region { region, .. } if region == image_region => Ok(()),
        RegionProvenance::Region { region, .. } => Err(JoinRefusal::NoRegionalProvenance {
            specimen: format!("{} (recorded region {})", tissue.label, region.as_str()),
            region: image_region.as_str().to_string(),
        }),
        RegionProvenance::WholeTumour | RegionProvenance::Unrecorded => {
            Err(JoinRefusal::NoRegionalProvenance {
                specimen: tissue.label.clone(),
                region: image_region.as_str().to_string(),
            })
        }
    }
}

/// [`align_to_image_region`], and then whether the two coordinates are in the same frame.
///
/// Regional provenance says the tissue came from a named region; it does not say the numbers
/// describing it and the numbers describing the image region mean the same thing. That is
/// `bioprism_standards`' question, and this function asks it rather than answering it here:
/// `comparable` returns [`bioprism_standards::Incomparability`], which refuses unstated frames,
/// disagreeing orientations, and — through
/// [`bioprism_standards::ReferenceSpace::SubjectNative`] — two participants' native spaces, which
/// are never the same space however similar their numbers look.
pub fn align_regional_position(
    tissue: &Artifact,
    tissue_point: &Position,
    image_region: &Pseudonym,
    image_point: &Position,
) -> Result<(), JoinRefusal> {
    align_to_image_region(tissue, image_region)?;
    comparable(
        &Measurement::located(format!("{} tissue coordinate", tissue.label), tissue_point.clone()),
        &Measurement::located(
            format!("image region {}", image_region.as_str()),
            image_point.clone(),
        ),
    )
    .map_err(|reason| JoinRefusal::IncomparableCoordinates {
        detail: reason.to_string(),
    })
}

/// The outcome of asking whether a join may be made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum JoinVerdict {
    Joinable,
    Declined { reason: JoinRefusal },
}

impl JoinVerdict {
    pub fn is_joinable(&self) -> bool {
        matches!(self, JoinVerdict::Joinable)
    }
}

/// A join decision, reportable whichever way it went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinReport {
    pub left: String,
    pub right: String,
    pub unit: AnalysisUnit,
    pub verdict: JoinVerdict,
}

/// Decides a join and records the decision, including a refusal.
pub fn report_join(
    left: &Artifact,
    right: &Artifact,
    unit: AnalysisUnit,
    evidence: &IdentityEvidence,
) -> JoinReport {
    let verdict = match joinable(left, right, unit, evidence) {
        Ok(()) => JoinVerdict::Joinable,
        Err(reason) => JoinVerdict::Declined { reason },
    };
    JoinReport {
        left: left.label.clone(),
        right: right.label.clone(),
        unit,
        verdict,
    }
}

/// How many independent observations a set of artefacts supplies at a given unit (30.03).
///
/// Distinct identifiers at the unit's level, not artefact count. "Treating aliquots as independent
/// patients" is a named failure and it is arithmetic: five libraries from one participant are one
/// participant. Artefacts that carry no identifier at the unit's level are reported separately
/// rather than counted, because an artefact whose specimen is unrecorded is neither a new specimen
/// nor a repeat of a known one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitCount {
    pub unit: AnalysisUnit,
    pub artifacts: usize,
    pub effective: usize,
    pub unattributable: Vec<String>,
}

impl UnitCount {
    /// True when the analysis would over-count if it treated each artefact as one observation.
    pub fn pseudoreplicated(&self) -> bool {
        self.effective < self.artifacts - self.unattributable.len()
    }
}

pub fn count_units(artifacts: &[Artifact], unit: AnalysisUnit) -> UnitCount {
    let mut seen: BTreeSet<&Pseudonym> = BTreeSet::new();
    let mut unattributable = Vec::new();
    for artifact in artifacts {
        let id = match unit {
            AnalysisUnit::Participant => Some(artifact.participant()),
            AnalysisUnit::Lesion => artifact.lesion.as_ref(),
            AnalysisUnit::Specimen => artifact.id_at(ArtifactLevel::Specimen),
            AnalysisUnit::ImagingSeries => artifact.id_at(ArtifactLevel::ImagingSeries),
        };
        match id {
            Some(id) => {
                seen.insert(id);
            }
            None => unattributable.push(artifact.label.clone()),
        }
    }
    UnitCount {
        unit,
        artifacts: artifacts.len(),
        effective: seen.len(),
        unattributable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_standards::{Frame, FrameBinding, Orientation, ReferenceSpace, Unit};

    fn participant(id: &str) -> Pseudonym {
        Pseudonym::new(id)
    }

    fn library(label: &str, patient: &str, specimen: &str, epoch: DiseaseEpoch) -> Artifact {
        Artifact::new(label, participant(patient), epoch)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E1"))
            .at(ArtifactLevel::Specimen, Pseudonym::new(specimen))
            .at(ArtifactLevel::Aliquot, Pseudonym::new("A1"))
            .at(ArtifactLevel::Library, Pseudonym::new(label))
    }

    #[test]
    fn a_truncated_identifier_is_not_a_match() {
        let short = Pseudonym::new("PT-0001");
        let long = Pseudonym::new("PT-00011");
        assert_ne!(short, long);
        assert!(short.is_truncation_of(&long));
        assert!(!long.is_truncation_of(&short));
    }

    #[test]
    fn joining_on_a_truncated_identifier_names_truncation_rather_than_a_bare_mismatch() {
        let left = library("L1", "PT-0001", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-00011", "S1", DiseaseEpoch::Preoperative);
        let refusal = joinable(
            &left,
            &right,
            AnalysisUnit::Participant,
            &IdentityEvidence::new(),
        )
        .unwrap_err();
        assert!(matches!(refusal, JoinRefusal::TruncatedIdentifier { .. }));
    }

    #[test]
    fn two_specimens_from_one_participant_are_not_interchangeable_at_specimen_unit() {
        let left = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-1", "S2", DiseaseEpoch::Preoperative);
        assert!(joinable(
            &left,
            &right,
            AnalysisUnit::Participant,
            &IdentityEvidence::new()
        )
        .is_ok());
        let refusal = joinable(
            &left,
            &right,
            AnalysisUnit::Specimen,
            &IdentityEvidence::new(),
        )
        .unwrap_err();
        assert!(matches!(refusal, JoinRefusal::DifferentSpecimen { .. }));
    }

    #[test]
    fn the_participant_dimension_blocks_before_the_specimen_dimension() {
        let left = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-2", "S9", DiseaseEpoch::Preoperative);
        let refusal = joinable(
            &left,
            &right,
            AnalysisUnit::Specimen,
            &IdentityEvidence::new(),
        )
        .unwrap_err();
        assert!(matches!(refusal, JoinRefusal::NoIdentityEvidence { .. }));
        assert_eq!(JOIN_CHECK_ORDER[0], "participant identity");
    }

    #[test]
    fn material_from_different_epochs_is_refused_until_a_bridge_is_stated() {
        let pre = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let post = library("L2", "PT-1", "S2", DiseaseEpoch::Postoperative);
        let refusal = joinable(
            &pre,
            &post,
            AnalysisUnit::Participant,
            &IdentityEvidence::new(),
        )
        .unwrap_err();
        assert!(matches!(refusal, JoinRefusal::IncompatibleEpoch { .. }));

        let bridge = EpochBridge {
            from: DiseaseEpoch::Preoperative,
            to: DiseaseEpoch::Postoperative,
            warrant: "repeated measures within one surgical episode, declared as such".to_string(),
        };
        assert!(joinable_with_bridge(
            &pre,
            &post,
            AnalysisUnit::Participant,
            &IdentityEvidence::new(),
            Some(&bridge)
        )
        .is_ok());
    }

    #[test]
    fn a_bridge_between_other_epochs_does_not_licence_this_pair() {
        let pre = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let recurrence = library("L2", "PT-1", "S2", DiseaseEpoch::Recurrence);
        let bridge = EpochBridge {
            from: DiseaseEpoch::Postoperative,
            to: DiseaseEpoch::OnTreatment,
            warrant: "unrelated warrant".to_string(),
        };
        assert!(joinable_with_bridge(
            &pre,
            &recurrence,
            AnalysisUnit::Participant,
            &IdentityEvidence::new(),
            Some(&bridge)
        )
        .is_err());
    }

    #[test]
    fn an_uncertain_identity_relation_does_not_licence_a_join() {
        let left = library("L1", "PT-A", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-B", "S1", DiseaseEpoch::Preoperative);
        let evidence = IdentityEvidence::new().with(
            IdentityLink::new(
                participant("PT-A"),
                participant("PT-B"),
                IdentityRelation::Uncertain,
                LinkBasis::GenotypeOrFingerprintConcordance,
                LinkConfidence::Uncertain,
            )
            .permitting(PermissibleUse::MultimodalJoin),
        );
        let refusal =
            joinable(&left, &right, AnalysisUnit::Participant, &evidence).unwrap_err();
        assert!(matches!(refusal, JoinRefusal::UnlicensedRelation { .. }));
    }

    #[test]
    fn a_link_that_permits_nothing_does_not_permit_a_multimodal_join() {
        let left = library("L1", "PT-A", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-B", "S1", DiseaseEpoch::Preoperative);
        let evidence = IdentityEvidence::new().with(IdentityLink::new(
            participant("PT-A"),
            participant("PT-B"),
            IdentityRelation::Same,
            LinkBasis::AuthoritativeCrosswalk,
            LinkConfidence::Asserted,
        ));
        let refusal =
            joinable(&left, &right, AnalysisUnit::Participant, &evidence).unwrap_err();
        assert!(matches!(
            refusal,
            JoinRefusal::UndeclaredPermissibleUse { .. }
        ));
    }

    #[test]
    fn a_duplicate_links_two_artifacts_and_destroys_their_independence() {
        assert!(IdentityRelation::Duplicate.licenses_join());
        assert!(!IdentityRelation::Duplicate.independent());
        assert!(IdentityRelation::Related.independent());
    }

    #[test]
    fn aliquots_of_one_participant_are_one_participant() {
        let artifacts = vec![
            library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative),
            library("L2", "PT-1", "S1", DiseaseEpoch::Preoperative),
            library("L3", "PT-1", "S2", DiseaseEpoch::Preoperative),
        ];
        let by_participant = count_units(&artifacts, AnalysisUnit::Participant);
        assert_eq!(by_participant.effective, 1);
        assert!(by_participant.pseudoreplicated());
        let by_specimen = count_units(&artifacts, AnalysisUnit::Specimen);
        assert_eq!(by_specimen.effective, 2);
    }

    #[test]
    fn an_artifact_with_no_identifier_at_the_unit_is_unattributable_rather_than_new() {
        let artifacts = vec![
            library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative),
            Artifact::new("imaging only", participant("PT-1"), DiseaseEpoch::Preoperative),
        ];
        let count = count_units(&artifacts, AnalysisUnit::Specimen);
        assert_eq!(count.effective, 1);
        assert_eq!(count.unattributable, vec!["imaging only".to_string()]);
        assert!(!count.pseudoreplicated());
    }

    #[test]
    fn whole_tumour_tissue_cannot_be_aligned_to_a_delineated_image_region() {
        let tissue = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative)
            .with_region(RegionProvenance::WholeTumour);
        let refusal = align_to_image_region(&tissue, &Pseudonym::new("R-enhancing")).unwrap_err();
        assert!(matches!(refusal, JoinRefusal::NoRegionalProvenance { .. }));
    }

    #[test]
    fn unrecorded_regional_provenance_is_refused_the_same_way_whole_tumour_is() {
        let tissue = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        assert!(align_to_image_region(&tissue, &Pseudonym::new("R-enhancing")).is_err());
        let regional = tissue.with_region(RegionProvenance::Region {
            lesion: Pseudonym::new("LES-1"),
            region: Pseudonym::new("R-enhancing"),
        });
        assert!(align_to_image_region(&regional, &Pseudonym::new("R-enhancing")).is_ok());
    }

    fn regional_tissue() -> Artifact {
        library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative).with_region(
            RegionProvenance::Region {
                lesion: Pseudonym::new("LES-1"),
                region: Pseudonym::new("R-enhancing"),
            },
        )
    }

    fn point_in_native_space(subject: &str) -> Position {
        Position::new(
            [62.0, -18.0, 31.0],
            Unit::parse("mm").expect("mm is in the table"),
            FrameBinding::Declared(Frame::world(
                "acquisition frame",
                Orientation::parse("RAS").expect("RAS is a valid orientation code"),
                ReferenceSpace::SubjectNative {
                    subject: subject.to_string(),
                },
            )),
        )
    }

    #[test]
    fn regional_provenance_alone_does_not_make_two_coordinates_comparable() {
        let mm = Unit::parse("mm").expect("mm is in the table");
        let unstated = Position::unstated([62.0, -18.0, 31.0], mm);
        let refusal = align_regional_position(
            &regional_tissue(),
            &unstated.clone(),
            &Pseudonym::new("R-enhancing"),
            &unstated,
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            JoinRefusal::IncomparableCoordinates { .. }
        ));
    }

    #[test]
    fn two_participants_native_spaces_are_never_the_same_space() {
        let refusal = align_regional_position(
            &regional_tissue(),
            &point_in_native_space("PT-1"),
            &Pseudonym::new("R-enhancing"),
            &point_in_native_space("PT-2"),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            JoinRefusal::IncomparableCoordinates { .. }
        ));
        assert!(align_regional_position(
            &regional_tissue(),
            &point_in_native_space("PT-1"),
            &Pseudonym::new("R-enhancing"),
            &point_in_native_space("PT-1"),
        )
        .is_ok());
    }

    #[test]
    fn the_provenance_check_runs_before_the_coordinate_check() {
        let unrecorded = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let refusal = align_regional_position(
            &unrecorded,
            &point_in_native_space("PT-1"),
            &Pseudonym::new("R-enhancing"),
            &point_in_native_space("PT-1"),
        )
        .unwrap_err();
        assert!(matches!(refusal, JoinRefusal::NoRegionalProvenance { .. }));
        assert_eq!(COORDINATE_CHECK_ORDER[0], "observable kind");
    }

    #[test]
    fn a_declined_join_is_a_serialisable_result_not_a_discarded_mismatch() {
        let left = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        let right = library("L2", "PT-2", "S1", DiseaseEpoch::Preoperative);
        let report = report_join(&left, &right, AnalysisUnit::Participant, &IdentityEvidence::new());
        assert!(!report.verdict.is_joinable());
        let encoded = serde_json::to_string(&report).expect("report serialises");
        assert!(encoded.contains("no_identity_evidence"));
    }

    #[test]
    fn a_lineage_gap_is_reported_rather_than_made_unrepresentable() {
        let orphan = Artifact::new("library with no specimen", participant("PT-1"), DiseaseEpoch::Preoperative)
            .at(ArtifactLevel::Library, Pseudonym::new("L1"));
        assert_eq!(orphan.lineage_gaps(), vec![ArtifactLevel::Library]);
        assert_eq!(orphan.level(), ArtifactLevel::Library);
    }

    #[test]
    fn the_scope_key_binds_every_lineage_level_as_a_real_scope_dimension() {
        let artifact = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative)
            .in_lesion(Pseudonym::new("LES-1"));
        let key = artifact.scope_key();
        let registry = onco_dimension_registry();
        assert!(key.unclassified_dimensions(&registry).is_empty());
        let classes = key.classes(&registry);
        assert!(classes.contains(&ScopeClass::Identity));
        assert!(classes.contains(&ScopeClass::Specimen));
        assert!(classes.contains(&ScopeClass::Region));
        assert!(classes.contains(&ScopeClass::Time));
    }

    #[test]
    fn the_registry_extends_rather_than_reclassifies_the_canonical_vocabulary() {
        let registry = onco_dimension_registry();
        assert_eq!(registry.classify("patient"), ScopeClass::Identity);
        assert_eq!(registry.classify("specimen"), ScopeClass::Specimen);
        assert_eq!(registry.classify("library"), ScopeClass::Specimen);
        assert_eq!(registry.classify("segmentation"), ScopeClass::Region);
    }

    #[test]
    fn narrowing_from_a_specimen_to_one_of_its_libraries_is_a_scope_refinement() {
        let specimen = Artifact::new("specimen", participant("PT-1"), DiseaseEpoch::Preoperative)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E1"))
            .at(ArtifactLevel::Specimen, Pseudonym::new("S1"));
        let lib = library("L1", "PT-1", "S1", DiseaseEpoch::Preoperative);
        assert!(lib.scope_key().refines(&specimen.scope_key()));
        assert!(!specimen.scope_key().refines(&lib.scope_key()));
    }
}
