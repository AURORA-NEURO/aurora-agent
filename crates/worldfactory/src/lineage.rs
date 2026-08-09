//! Specimen lineage, mix-up and identity mutations (27.11).
//!
//! 27.11's *Critical design decision* is the best sentence in §27: "specimen errors are especially
//! valuable because a sophisticated analysis can be perfectly executed on the wrong material."
//! Nothing downstream fails. Every quality metric passes. The pipeline is correct, the statistics
//! are correct, and the answer is about somebody else.
//!
//! # The lesson `crates/mutation` learned, from the other side
//!
//! `bioprism_mutation::lineage::content_digest` deliberately excludes a world's label, because
//! otherwise a generator could manufacture a million "instances" by renaming one. Dedup must not
//! be defeatable by renaming.
//!
//! Here the identical exclusion has the opposite consequence. [`SpecimenNode::content_digest`]
//! excludes the donor label too — and that is *precisely why a relabelled specimen is invisible to
//! it*. The content is unchanged; only the name moved; the digest is identical, which is the
//! correct answer to the dedup question and useless for the identity question. A relabel is caught
//! by identity evidence or it is not caught at all.
//!
//! That is why [`apply`] refuses a program that expects detection when the world exposes no route
//! to it: [`crate::error::IdentityProgramRefusal::UndetectableByConstruction`]. 27.11's workflow
//! step 4 is "add **or withhold** identity evidence", and withholding it is legitimate — but then
//! step 5's "expected detection or abstention" must say abstention. Demanding detection of
//! something the world cannot reveal measures nothing.
//!
//! # Routes, computed from evidence rather than declared
//!
//! [`detection_routes`] does not ask what operation was applied. It compares the registry before
//! and after and reports what a careful analyst could actually notice: a fingerprint that
//! contradicts a label, aliquots that outweigh their parent, a child collected before its parent, a
//! cycle in the ancestry, two identifiers holding identical material, or artifacts of one specimen
//! that no longer agree with each other. An operation whose every trace was propagated away leaves
//! an empty set, and that is [`crate::error::IdentityProgramRefusal::PropagatedEverywhere`] —
//! 27.11's failure "all modalities changed so no contradiction remains".
//!
//! # Three states, again
//!
//! [`FingerprintCheck`] has three variants. 27.11's validation item says "genetic fingerprint
//! consistency **when available**", and the unavailable case is not consistency. A specimen with no
//! fingerprint on file has not passed the identity check; nobody ran it. This is the same rule
//! `bioprism_onco::ObservationStatus` fixes for assays and [`crate::contradiction`] fixes for
//! modalities, and it is the single most repeated mistake §27's failure lists describe.
//!
//! # What is deliberately not here
//!
//! No genotyping, no fingerprint comparison arithmetic, no consent model and no access-control
//! engine. A [`Fingerprint`] carries an opaque donor identifier and comparison is equality;
//! computing concordance from real markers is an assay this crate does not have.
//! [`SpecimenNode::access_domain`] is a declared string and
//! [`crate::error::IdentityProgramRefusal::CrossesAccessBoundary`] compares strings — the actual
//! enforcement of a data-use agreement belongs to the governance layer, and a check here would
//! imply an authority this crate does not hold.

use crate::error::IdentityProgramRefusal;
use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A specimen identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpecimenId(String);

impl SpecimenId {
    pub fn new(value: impl Into<String>) -> Self {
        SpecimenId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SpecimenId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An artifact derived from a specimen — a slide, a sequencing run, an image series.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(String);

impl ArtifactId {
    pub fn new(value: impl Into<String>) -> Self {
        ArtifactId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identity evidence: which donor the *material* is from, independent of what the tube says.
///
/// Opaque donor identifier, compared by equality. Real fingerprint concordance is an assay with a
/// call rate and a discrimination power; none of that is here, and pretending otherwise would put
/// an invented sensitivity into an identity check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub donor: String,
}

impl Fingerprint {
    pub fn new(donor: impl Into<String>) -> Self {
        Fingerprint {
            donor: donor.into(),
        }
    }
}

/// One node of the material ancestry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecimenNode {
    pub id: SpecimenId,
    pub derived_from: Option<SpecimenId>,
    /// What the label says. The thing a relabel attacks, and the thing a fingerprint contradicts.
    pub donor_label: String,
    /// Mass in micrograms. Integer, so conservation is exact arithmetic rather than a tolerance.
    pub mass_ug: u64,
    pub collected_at: Option<Timestamp>,
    /// What the material *is*, opaque to this crate. Excluded from nothing except the label — see
    /// [`SpecimenNode::content_digest`].
    pub content: BTreeMap<String, Value>,
    /// Identity evidence, when the world exposes it. `None` is the withheld case of 27.11 workflow
    /// step 4, not a passing check.
    pub fingerprint: Option<Fingerprint>,
    /// The access regime the material sits under. Declared; not enforced here.
    pub access_domain: String,
}

impl SpecimenNode {
    pub fn new(id: impl Into<String>, donor_label: impl Into<String>, mass_ug: u64) -> Self {
        SpecimenNode {
            id: SpecimenId::new(id),
            derived_from: None,
            donor_label: donor_label.into(),
            mass_ug,
            collected_at: None,
            content: BTreeMap::new(),
            fingerprint: None,
            access_domain: "default".to_string(),
        }
    }

    pub fn derived_from(mut self, parent: impl Into<String>) -> Self {
        self.derived_from = Some(SpecimenId::new(parent));
        self
    }

    pub fn collected_at(mut self, at: Timestamp) -> Self {
        self.collected_at = Some(at);
        self
    }

    pub fn with_content(mut self, key: impl Into<String>, value: Value) -> Self {
        self.content.insert(key.into(), value);
        self
    }

    pub fn fingerprinted(mut self, donor: impl Into<String>) -> Self {
        self.fingerprint = Some(Fingerprint::new(donor));
        self
    }

    pub fn in_domain(mut self, domain: impl Into<String>) -> Self {
        self.access_domain = domain.into();
        self
    }

    /// Digest of the material, excluding every label.
    ///
    /// Excludes `id`, `donor_label` and `access_domain` for the same reason
    /// `bioprism_mutation::lineage::content_digest` excludes a world's name: two records holding
    /// the same material are the same material however they are named. The consequence is stated
    /// in the module header and is the whole point of the module — a relabel does not move this
    /// number.
    pub fn content_digest(&self) -> String {
        serde_json::to_value(&self.content)
            .ok()
            .and_then(|v| ContentHash::of_value(&v).ok())
            .map(|h| h.as_str().to_string())
            .unwrap_or_else(|| "uncanonicalisable".to_string())
    }

    /// Whether the identity evidence contradicts the label, or is absent.
    pub fn fingerprint_check(&self) -> FingerprintCheck {
        match &self.fingerprint {
            None => FingerprintCheck::NoEvidenceAvailable {
                specimen: self.id.clone(),
            },
            Some(fp) if fp.donor == self.donor_label => FingerprintCheck::Consistent {
                specimen: self.id.clone(),
            },
            Some(fp) => FingerprintCheck::Mismatch {
                specimen: self.id.clone(),
                declared_donor: self.donor_label.clone(),
                fingerprint_donor: fp.donor.clone(),
            },
        }
    }
}

/// The three-state identity check. See the module header.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "fingerprint", rename_all = "snake_case")]
pub enum FingerprintCheck {
    Consistent {
        specimen: SpecimenId,
    },
    Mismatch {
        specimen: SpecimenId,
        declared_donor: String,
        fingerprint_donor: String,
    },
    /// Nobody ran it. Categorically distinct from [`FingerprintCheck::Consistent`].
    NoEvidenceAvailable {
        specimen: SpecimenId,
    },
}

impl FingerprintCheck {
    /// True only for [`FingerprintCheck::Consistent`]. There is deliberately no `is_clean` that
    /// would fold the absent case in with the passing one.
    pub fn is_consistent(&self) -> bool {
        matches!(self, FingerprintCheck::Consistent { .. })
    }
}

/// An artifact and the specimen it is currently attributed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub specimen: SpecimenId,
    pub modality: String,
    /// The content digest of the material this artifact was taken from, recorded at acquisition.
    ///
    /// This is what makes a partially-propagated mix-up findable without any genotyping: an
    /// artifact that still remembers what it was cut from disagrees with a tube whose contents
    /// moved. `None` means the acquisition recorded nothing, which is common and is why
    /// [`SpecimenRegistry::seal`] exists to stamp it for a fixture.
    pub observed_digest: Option<String>,
}

impl Artifact {
    pub fn new(
        id: impl Into<String>,
        specimen: impl Into<String>,
        modality: impl Into<String>,
    ) -> Self {
        Artifact {
            id: ArtifactId::new(id),
            specimen: SpecimenId::new(specimen),
            modality: modality.into(),
            observed_digest: None,
        }
    }
}

/// The material ancestry and everything derived from it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SpecimenRegistry {
    pub nodes: BTreeMap<SpecimenId, SpecimenNode>,
    pub artifacts: BTreeMap<ArtifactId, Artifact>,
}

impl SpecimenRegistry {
    pub fn new() -> Self {
        SpecimenRegistry::default()
    }

    pub fn with_specimen(mut self, node: SpecimenNode) -> Self {
        self.nodes.insert(node.id.clone(), node);
        self
    }

    pub fn with_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.insert(artifact.id.clone(), artifact);
        self
    }

    /// Record, for every artifact, the digest of the material it currently points at.
    ///
    /// Represents acquisition: at the moment a slide is cut or a library is prepared, the artifact
    /// is of *this* material. Everything the audit later notices about mix-ups follows from
    /// artifacts remembering that and tubes not.
    pub fn seal(mut self) -> Self {
        let digests: BTreeMap<SpecimenId, String> = self
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.content_digest()))
            .collect();
        for artifact in self.artifacts.values_mut() {
            artifact.observed_digest = digests.get(&artifact.specimen).cloned();
        }
        self
    }

    pub fn artifacts_of(&self, specimen: &SpecimenId) -> BTreeSet<ArtifactId> {
        self.artifacts
            .values()
            .filter(|a| &a.specimen == specimen)
            .map(|a| a.id.clone())
            .collect()
    }

    fn ancestors(&self, start: &SpecimenId) -> Result<Vec<SpecimenId>, SpecimenId> {
        let mut seen = BTreeSet::new();
        let mut chain = Vec::new();
        let mut cursor = Some(start.clone());
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                return Err(current);
            }
            chain.push(current.clone());
            cursor = self
                .nodes
                .get(&current)
                .and_then(|n| n.derived_from.clone());
        }
        Ok(chain)
    }
}

/// Something an audit noticed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    LineageCycle {
        specimen: SpecimenId,
    },
    /// 27.11 validation "quantity conservation".
    MassNotConserved {
        parent: SpecimenId,
        parent_mass_ug: u64,
        child_total_ug: u64,
    },
    /// 27.11 validation "temporal plausibility": a child cannot predate the material it came from.
    TemporalImplausibility {
        child: SpecimenId,
        parent: SpecimenId,
    },
    /// Two identifiers holding byte-identical material.
    DuplicateContent {
        left: SpecimenId,
        right: SpecimenId,
    },
    IdentityMismatch(FingerprintCheck),
    /// A specimen's artifacts no longer describe the same material.
    ArtifactsDisagree {
        specimen: SpecimenId,
        artifacts: BTreeSet<ArtifactId>,
    },
}

/// 27.11's validation list, run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageAudit {
    pub findings: Vec<Finding>,
    /// The identity check for every specimen, including the ones with no evidence. Reported in
    /// full rather than filtered to failures, because "twelve specimens have no fingerprint on
    /// file" is the finding a reviewer most needs and the one a failures-only list hides.
    pub fingerprints: Vec<FingerprintCheck>,
}

impl LineageAudit {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Specimens whose identity nobody checked.
    pub fn unchecked_specimens(&self) -> Vec<&SpecimenId> {
        self.fingerprints
            .iter()
            .filter_map(|check| match check {
                FingerprintCheck::NoEvidenceAvailable { specimen } => Some(specimen),
                _ => None,
            })
            .collect()
    }
}

/// Run 27.11's validation list over a registry.
pub fn audit(registry: &SpecimenRegistry) -> LineageAudit {
    let mut findings = Vec::new();

    for id in registry.nodes.keys() {
        if let Err(specimen) = registry.ancestors(id) {
            findings.push(Finding::LineageCycle { specimen });
        }
    }

    let mut child_mass: BTreeMap<SpecimenId, u64> = BTreeMap::new();
    for node in registry.nodes.values() {
        if let Some(parent) = &node.derived_from {
            *child_mass.entry(parent.clone()).or_insert(0) += node.mass_ug;
        }
    }
    for (parent, total) in &child_mass {
        if let Some(node) = registry.nodes.get(parent) {
            if *total > node.mass_ug {
                findings.push(Finding::MassNotConserved {
                    parent: parent.clone(),
                    parent_mass_ug: node.mass_ug,
                    child_total_ug: *total,
                });
            }
        }
    }

    for node in registry.nodes.values() {
        let (Some(parent_id), Some(child_at)) = (&node.derived_from, node.collected_at) else {
            continue;
        };
        let Some(parent_at) = registry
            .nodes
            .get(parent_id)
            .and_then(|p| p.collected_at)
        else {
            continue;
        };
        if child_at < parent_at {
            findings.push(Finding::TemporalImplausibility {
                child: node.id.clone(),
                parent: parent_id.clone(),
            });
        }
    }

    let mut by_digest: BTreeMap<String, Vec<SpecimenId>> = BTreeMap::new();
    for node in registry.nodes.values() {
        if node.content.is_empty() {
            continue;
        }
        by_digest
            .entry(node.content_digest())
            .or_default()
            .push(node.id.clone());
    }
    for ids in by_digest.values() {
        for pair in ids.windows(2) {
            findings.push(Finding::DuplicateContent {
                left: pair[0].clone(),
                right: pair[1].clone(),
            });
        }
    }

    let mut disagreeing: BTreeMap<SpecimenId, BTreeSet<ArtifactId>> = BTreeMap::new();
    for artifact in registry.artifacts.values() {
        let (Some(recorded), Some(node)) = (
            artifact.observed_digest.as_ref(),
            registry.nodes.get(&artifact.specimen),
        ) else {
            continue;
        };
        if recorded != &node.content_digest() {
            disagreeing
                .entry(artifact.specimen.clone())
                .or_default()
                .insert(artifact.id.clone());
        }
    }
    for (specimen, artifacts) in disagreeing {
        findings.push(Finding::ArtifactsDisagree {
            specimen,
            artifacts,
        });
    }

    let mut fingerprints = Vec::new();
    for node in registry.nodes.values() {
        let check = node.fingerprint_check();
        if matches!(check, FingerprintCheck::Mismatch { .. }) {
            findings.push(Finding::IdentityMismatch(check.clone()));
        }
        fingerprints.push(check);
    }

    findings.sort();
    LineageAudit {
        findings,
        fingerprints,
    }
}

/// 27.11 workflow step 2's five operations, verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum IdentityOperation {
    /// Two tubes receive each other's labels. The material moves; the names do not.
    Swap { a: SpecimenId, b: SpecimenId },
    /// The same material enters the registry twice under different identifiers.
    Duplicate {
        source: SpecimenId,
        as_id: SpecimenId,
    },
    /// Material from one specimen enters another.
    Contamination {
        host: SpecimenId,
        contaminant: SpecimenId,
    },
    /// One tube's label changes. Nothing else does — which is the point.
    Relabel {
        specimen: SpecimenId,
        to_label: String,
    },
    /// The recorded quantity stops matching what was taken.
    QuantityInconsistency { specimen: SpecimenId, mass_ug: u64 },
}

impl IdentityOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdentityOperation::Swap { .. } => "swap",
            IdentityOperation::Duplicate { .. } => "duplicate",
            IdentityOperation::Contamination { .. } => "contamination",
            IdentityOperation::Relabel { .. } => "relabel",
            IdentityOperation::QuantityInconsistency { .. } => "quantity_inconsistency",
        }
    }

    fn touches(&self) -> Vec<SpecimenId> {
        match self {
            IdentityOperation::Swap { a, b } => vec![a.clone(), b.clone()],
            IdentityOperation::Duplicate { source, as_id } => vec![source.clone(), as_id.clone()],
            IdentityOperation::Contamination { host, contaminant } => {
                vec![host.clone(), contaminant.clone()]
            }
            IdentityOperation::Relabel { specimen, .. }
            | IdentityOperation::QuantityInconsistency { specimen, .. } => vec![specimen.clone()],
        }
    }
}

/// What 27.11 workflow step 5 asks of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedDetection {
    /// The world exposes a route and the agent should find it.
    Detect,
    /// The world does not, and the correct behaviour is to say so rather than to proceed.
    Abstain,
}

/// A seeded identity error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityProgram {
    pub id: String,
    pub operation: IdentityOperation,
    /// Artifacts that follow the material. 27.11 workflow step 3: "propagate to affected data
    /// **only**". Artifacts left behind are what makes the error findable.
    pub propagate_to: BTreeSet<ArtifactId>,
    pub expected: ExpectedDetection,
}

impl IdentityProgram {
    pub fn new(
        id: impl Into<String>,
        operation: IdentityOperation,
        expected: ExpectedDetection,
    ) -> Self {
        IdentityProgram {
            id: id.into(),
            operation,
            propagate_to: BTreeSet::new(),
            expected,
        }
    }

    pub fn propagating_to(mut self, artifact: impl Into<String>) -> Self {
        self.propagate_to.insert(ArtifactId::new(artifact));
        self
    }
}

/// A way a careful analyst could notice the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionRoute {
    GeneticFingerprint,
    MassBalance,
    TemporalOrder,
    LineageCycle,
    DuplicateContent,
    /// Artifacts of one specimen that no longer describe the same material.
    CrossArtifactDisagreement,
}

/// What actually changed, and what it left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityDelta {
    pub program: String,
    pub operation: IdentityOperation,
    pub affected: BTreeSet<SpecimenId>,
    pub artifacts_repointed: BTreeSet<ArtifactId>,
    pub routes: BTreeSet<DetectionRoute>,
}

/// What a careful analyst could notice, computed by comparing registries rather than by asking the
/// operation what it did.
///
/// The distinction matters: an operation that *claims* to be detectable and whose traces were all
/// propagated away is exactly 27.11's failure "all modalities changed so no contradiction remains",
/// and only the comparison catches it.
pub fn detection_routes(
    before: &SpecimenRegistry,
    after: &SpecimenRegistry,
    affected: &BTreeSet<SpecimenId>,
    repointed: &BTreeSet<ArtifactId>,
) -> BTreeSet<DetectionRoute> {
    let mut routes = BTreeSet::new();
    let before_audit = audit(before);
    let after_audit = audit(after);

    let new_findings: Vec<&Finding> = after_audit
        .findings
        .iter()
        .filter(|f| !before_audit.findings.contains(f))
        .collect();

    for finding in new_findings {
        match finding {
            Finding::IdentityMismatch(_) => {
                routes.insert(DetectionRoute::GeneticFingerprint);
            }
            Finding::MassNotConserved { .. } => {
                routes.insert(DetectionRoute::MassBalance);
            }
            Finding::TemporalImplausibility { .. } => {
                routes.insert(DetectionRoute::TemporalOrder);
            }
            Finding::LineageCycle { .. } => {
                routes.insert(DetectionRoute::LineageCycle);
            }
            Finding::DuplicateContent { .. } => {
                routes.insert(DetectionRoute::DuplicateContent);
            }
            Finding::ArtifactsDisagree { .. } => {
                routes.insert(DetectionRoute::CrossArtifactDisagreement);
            }
        }
    }

    for specimen in affected {
        let artifacts = before.artifacts_of(specimen);
        let moved = artifacts.intersection(repointed).count();
        if moved > 0 && moved < artifacts.len() {
            routes.insert(DetectionRoute::CrossArtifactDisagreement);
        }
    }

    routes
}

/// Seed an identity error, or refuse the program.
///
/// The checks run in this order:
///
/// 1. **Access boundary** — 27.11's failure "swap crosses inaccessible datasets". Moving material
///    between access domains is a governance event, not a benchmark.
/// 2. **Cycle** — an operation that makes a specimen its own ancestor has produced a world nothing
///    can reason about.
/// 3. **Propagated everywhere** — the operation touched two or more artifacts and every one of
///    them followed, so nothing in the world disagrees with anything else.
/// 4. **Undetectable by construction** — no route at all, and the program expects detection.
///
/// Checks 3 and 4 are separate because the situations differ. Three is an author erasing a trace
/// that existed; four is an author demanding detection from a world that never had one — a relabel
/// with the fingerprints withheld. Both are legitimate constructions when the expectation is
/// [`ExpectedDetection::Abstain`], and neither fires then.
pub fn apply(
    registry: &SpecimenRegistry,
    program: &IdentityProgram,
) -> Result<(SpecimenRegistry, IdentityDelta), IdentityProgramRefusal> {
    let affected: BTreeSet<SpecimenId> = program.operation.touches().into_iter().collect();

    if let IdentityOperation::Swap { a, b } | IdentityOperation::Contamination { host: a, contaminant: b } =
        &program.operation
    {
        let left = registry.nodes.get(a).map(|n| n.access_domain.clone());
        let right = registry.nodes.get(b).map(|n| n.access_domain.clone());
        if let (Some(left), Some(right)) = (left, right) {
            if left != right {
                return Err(IdentityProgramRefusal::CrossesAccessBoundary {
                    operation: program.operation.as_str().to_string(),
                    left,
                    right,
                });
            }
        }
    }

    let mut after = registry.clone();
    match &program.operation {
        IdentityOperation::Swap { a, b } => {
            let (Some(node_a), Some(node_b)) =
                (registry.nodes.get(a).cloned(), registry.nodes.get(b).cloned())
            else {
                return Ok((after, empty_delta(program, affected)));
            };
            if let Some(target) = after.nodes.get_mut(a) {
                target.content = node_b.content.clone();
                target.fingerprint = node_b.fingerprint.clone();
            }
            if let Some(target) = after.nodes.get_mut(b) {
                target.content = node_a.content.clone();
                target.fingerprint = node_a.fingerprint.clone();
            }
        }
        IdentityOperation::Duplicate { source, as_id } => {
            if let Some(node) = registry.nodes.get(source).cloned() {
                let mut copy = node;
                copy.id = as_id.clone();
                after.nodes.insert(as_id.clone(), copy);
            }
        }
        IdentityOperation::Contamination { host, contaminant } => {
            if let Some(donor) = registry.nodes.get(contaminant).cloned() {
                if let Some(target) = after.nodes.get_mut(host) {
                    for (key, value) in donor.content {
                        target.content.insert(key, value);
                    }
                }
            }
        }
        IdentityOperation::Relabel { specimen, to_label } => {
            if let Some(target) = after.nodes.get_mut(specimen) {
                target.donor_label = to_label.clone();
            }
        }
        IdentityOperation::QuantityInconsistency { specimen, mass_ug } => {
            if let Some(target) = after.nodes.get_mut(specimen) {
                target.mass_ug = *mass_ug;
            }
        }
    }

    let repointed_digests: BTreeMap<SpecimenId, String> = after
        .nodes
        .iter()
        .map(|(id, node)| (id.clone(), node.content_digest()))
        .collect();
    for artifact in &program.propagate_to {
        if let Some(entry) = after.artifacts.get_mut(artifact) {
            if let IdentityOperation::Swap { a, b } = &program.operation {
                entry.specimen = if &entry.specimen == a {
                    b.clone()
                } else if &entry.specimen == b {
                    a.clone()
                } else {
                    entry.specimen.clone()
                };
            }
            if entry.observed_digest.is_some() {
                entry.observed_digest = repointed_digests.get(&entry.specimen).cloned();
            }
        }
    }

    for id in &affected {
        if after.ancestors(id).is_err() {
            return Err(IdentityProgramRefusal::LineageCycle {
                specimen: id.to_string(),
            });
        }
    }

    let routes = detection_routes(registry, &after, &affected, &program.propagate_to);

    if program.expected == ExpectedDetection::Detect {
        let touched_artifacts: BTreeSet<ArtifactId> = affected
            .iter()
            .flat_map(|s| registry.artifacts_of(s))
            .collect();
        let all_followed = touched_artifacts.len() >= 2
            && touched_artifacts.is_subset(&program.propagate_to);
        if all_followed && routes.is_empty() {
            return Err(IdentityProgramRefusal::PropagatedEverywhere {
                operation: program.operation.as_str().to_string(),
                artifacts: touched_artifacts.len(),
            });
        }
        if routes.is_empty() {
            return Err(IdentityProgramRefusal::UndetectableByConstruction {
                program: program.id.clone(),
                operation: program.operation.as_str().to_string(),
            });
        }
    }

    let delta = IdentityDelta {
        program: program.id.clone(),
        operation: program.operation.clone(),
        affected,
        artifacts_repointed: program.propagate_to.clone(),
        routes,
    };
    Ok((after, delta))
}

fn empty_delta(program: &IdentityProgram, affected: BTreeSet<SpecimenId>) -> IdentityDelta {
    IdentityDelta {
        program: program.id.clone(),
        operation: program.operation.clone(),
        affected,
        artifacts_repointed: program.propagate_to.clone(),
        routes: BTreeSet::new(),
    }
}
