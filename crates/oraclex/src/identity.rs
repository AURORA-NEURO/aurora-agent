//! Sample identity, relatedness, and lineage (31.05), and the specimen-swap mutations that stress
//! it (32.05).
//!
//! 31.05's worked case is one sentence and the whole module falls out of it: "Two samples share a
//! truncated identifier but have incompatible fingerprints; **identity evidence overrides the textual
//! join**."
//!
//! That override is not implemented as a special case. A molecular signal declares
//! [`EvidenceTier::Deterministic`] and a textual crosswalk declares [`EvidenceTier::Property`], and
//! the ordering is then `bioprism-oracle`'s [`EvidenceTier::may_override`]. Nothing here re-derives
//! the ladder; note in particular that `may_override` answers `true` for *equal* tiers, because a
//! same-tier conflict is a disagreement and not an override — `crates/biolang` got that backwards
//! once and a projection test caught it.
//!
//! # One evidence set, three claims
//!
//! [`decide`] takes an [`IdentityClaim`] as well as the signals, because the same fingerprint answers
//! different questions differently. A two-contributor mixture *contradicts* "this is a single-source
//! specimen" and leaves "these two aliquots are the same participant" *unresolved*: the contaminating
//! fraction could have come from anywhere. An implementation with one `is_same()` entry point has to
//! pick one of those answers for both questions, and whichever it picks is wrong half the time.
//!
//! # Where the abstentions are
//!
//! * No signal at all — [`Determination::NotEvaluable`]. The oracle does not apply.
//! * Textual join only — [`Determination::Unresolved`], naming molecular evidence as the gap. This is
//!   the case 31.05 exists for. A crosswalk that agrees is not confirmation; it is the absence of a
//!   test. 32.05's first failure risk is "agent trusts textual IDs over molecular evidence", and an
//!   oracle that returns `supported` here has committed it on the agent's behalf.
//! * Ambiguous molecular signal — [`Determination::Unresolved`], naming what would settle it.
//!
//! # Not implemented
//!
//! No fingerprint computation, no allele calling, no relatedness coefficient, no mixture
//! deconvolution. [`Concordance`] is a caller's conclusion about a comparison this crate did not
//! perform, and [`Mixture`] carries a contributor count the caller established. 31.05's "compare
//! genotype fingerprints" and "check sex-chromosome and copy-number concordance" are assay work.
//! This module is the part that decides what those comparisons license, which is the part that keeps
//! getting it wrong.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Missing, Unresolved, Witness};

/// A point in the participant → lesion → specimen → block → aliquot → library/image → time chain
/// that 31.05 requires artifacts to be correctly assigned across.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SpecimenRef {
    pub participant: String,
    /// `None` for a specimen not tied to a named lesion; distinct from a lesion named "unknown".
    pub lesion: Option<String>,
    pub aliquot: String,
    /// A caller-supplied ordering label for time. Not a clock: this crate reads no clock, and a
    /// timepoint is whatever the study called it.
    pub timepoint: String,
}

impl SpecimenRef {
    pub fn new(
        participant: impl Into<String>,
        aliquot: impl Into<String>,
        timepoint: impl Into<String>,
    ) -> Self {
        SpecimenRef {
            participant: participant.into(),
            lesion: None,
            aliquot: aliquot.into(),
            timepoint: timepoint.into(),
        }
    }

    pub fn label(&self) -> String {
        match &self.lesion {
            Some(lesion) => format!(
                "{}/{}/{}@{}",
                self.participant, lesion, self.aliquot, self.timepoint
            ),
            None => format!("{}/{}@{}", self.participant, self.aliquot, self.timepoint),
        }
    }
}

/// What a comparison of two artifacts concluded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "concordance", rename_all = "snake_case")]
pub enum Concordance {
    Concordant,
    Discordant,
    /// The comparison ran and did not separate the hypotheses. Carries what would.
    Ambiguous {
        reason: String,
        would_settle: String,
    },
}

/// Evidence bearing on whether two artifacts belong to the same subject.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum IdentitySignal {
    /// A genotype fingerprint comparison.
    GenotypeFingerprint { concordance: Concordance },
    /// Sex-chromosome concordance: a strong falsifier, a weak confirmer, and typed as such by
    /// [`IdentitySignal::confirms`].
    SexChromosome { concordance: Concordance },
    /// Copy-number profile concordance.
    CopyNumber { concordance: Concordance },
    /// A join on a shared identifier. Deterministic about the *string*, and silent about the
    /// specimen, which is the distinction this whole module exists to hold.
    TextualCrosswalk { join_key: String, agrees: bool },
    /// More than one contributor detected in one artifact (32.05's mixtures).
    Mixture { mixture: Mixture },
}

/// A detected multi-contributor artifact.
///
/// The fraction is optional and caller-supplied. This crate hardcodes no mixture threshold: 32.05's
/// worked relation uses a 15% cross-sample mixture as *its illustration*, not as a decision rule, and
/// promoting an illustration to a constant is how a benchmark acquires a number nobody can defend.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Mixture {
    pub contributors: u32,
    pub minor_fraction_permille: Option<u32>,
}

impl Mixture {
    pub fn new(contributors: u32) -> Self {
        Mixture {
            contributors,
            minor_fraction_permille: None,
        }
    }

    pub fn with_minor_fraction_permille(mut self, permille: u32) -> Self {
        self.minor_fraction_permille = Some(permille);
        self
    }
}

impl IdentitySignal {
    /// The rung this signal carries.
    ///
    /// Molecular signals are [`EvidenceTier::Deterministic`] in 31.02's sense — a mismatch is a proof
    /// of defect in the assignment. A crosswalk is [`EvidenceTier::Property`]: recomputable from the
    /// metadata, and a statement about the metadata.
    pub fn tier(&self) -> EvidenceTier {
        match self {
            IdentitySignal::GenotypeFingerprint { .. }
            | IdentitySignal::SexChromosome { .. }
            | IdentitySignal::CopyNumber { .. }
            | IdentitySignal::Mixture { .. } => EvidenceTier::Deterministic,
            IdentitySignal::TextualCrosswalk { .. } => EvidenceTier::Property,
        }
    }

    /// Whether this signal is molecular evidence about the specimen rather than about its labels.
    pub fn is_molecular(&self) -> bool {
        !matches!(self, IdentitySignal::TextualCrosswalk { .. })
    }

    /// Whether agreement on this signal is evidence *for* identity, as opposed to merely not being
    /// evidence against it.
    ///
    /// Sex-chromosome concordance is the standing example: two unrelated participants of the same
    /// sex are concordant, so the signal falsifies and does not confirm. Treating it as confirmation
    /// is how a swap survives a check that was run.
    pub fn confirms(&self) -> bool {
        match self {
            IdentitySignal::GenotypeFingerprint { .. } => true,
            IdentitySignal::CopyNumber { .. } => true,
            IdentitySignal::SexChromosome { .. } => false,
            IdentitySignal::TextualCrosswalk { .. } => false,
            IdentitySignal::Mixture { .. } => false,
        }
    }

    fn concordance(&self) -> Option<&Concordance> {
        match self {
            IdentitySignal::GenotypeFingerprint { concordance }
            | IdentitySignal::SexChromosome { concordance }
            | IdentitySignal::CopyNumber { concordance } => Some(concordance),
            IdentitySignal::TextualCrosswalk { .. } | IdentitySignal::Mixture { .. } => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            IdentitySignal::GenotypeFingerprint { .. } => "genotype_fingerprint",
            IdentitySignal::SexChromosome { .. } => "sex_chromosome",
            IdentitySignal::CopyNumber { .. } => "copy_number",
            IdentitySignal::TextualCrosswalk { .. } => "textual_crosswalk",
            IdentitySignal::Mixture { .. } => "mixture",
        }
    }
}

/// The question being asked of the identity evidence.
///
/// Separate claims because they have separate answers under the same evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityClaim {
    /// These two artifacts came from the same participant.
    SameSubject,
    /// This artifact has exactly one contributor.
    SingleSource,
    /// These two artifacts came from different participants.
    DistinctSubjects,
}

impl IdentityClaim {
    pub fn as_str(self) -> &'static str {
        match self {
            IdentityClaim::SameSubject => "same_subject",
            IdentityClaim::SingleSource => "single_source",
            IdentityClaim::DistinctSubjects => "distinct_subjects",
        }
    }
}

/// Decides one identity claim from the signals available about a pair of artifacts.
///
/// The order of the branches is the epistemics. A contradiction at the deterministic rung is checked
/// before any support, a mixture is checked before same-subject reasoning, and the textual crosswalk
/// is never allowed to decide anything on its own.
pub fn decide(
    claim: IdentityClaim,
    left: &SpecimenRef,
    right: &SpecimenRef,
    signals: &[IdentitySignal],
) -> Determination {
    if signals.is_empty() {
        return Determination::not_evaluable("no identity signal was recorded for this pair");
    }

    let mixture = signals.iter().find_map(|signal| match signal {
        IdentitySignal::Mixture { mixture } if mixture.contributors > 1 => Some(mixture),
        _ => None,
    });

    if claim == IdentityClaim::SingleSource {
        return match mixture {
            Some(mixture) => Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::IdentityConflict {
                    left: left.label(),
                    right: right.label(),
                    joined_on: "single-source assumption".to_string(),
                    conflicting_evidence: format!("{} contributors detected", mixture.contributors),
                },
            ),
            None if signals.iter().any(IdentitySignal::is_molecular) => Determination::supported(
                EvidenceTier::Deterministic,
                "no multi-contributor signal in the molecular evidence",
            ),
            None => Determination::unresolved(
                "molecular contributor count",
                "only metadata was available, and metadata cannot see a mixture",
            ),
        };
    }

    if let Some(mixture) = mixture {
        return Determination::Unresolved(
            Unresolved::new([Missing::new(
                "deconvolved per-contributor genotype",
                format!(
                    "{} contributors are present, so a pairwise identity call is underdetermined",
                    mixture.contributors
                ),
            )])
            .expect("one missing item is not zero"),
        );
    }

    let discordant: Vec<&IdentitySignal> = signals
        .iter()
        .filter(|signal| signal.concordance() == Some(&Concordance::Discordant))
        .collect();

    if !discordant.is_empty() {
        let conflicting = discordant
            .iter()
            .map(|signal| signal.name())
            .collect::<Vec<_>>()
            .join(", ");
        let joined_on = signals
            .iter()
            .find_map(|signal| match signal {
                IdentitySignal::TextualCrosswalk { join_key, .. } => Some(join_key.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "no declared join".to_string());
        let witness = Witness::IdentityConflict {
            left: left.label(),
            right: right.label(),
            joined_on,
            conflicting_evidence: conflicting,
        };
        return match claim {
            IdentityClaim::SameSubject => {
                Determination::contradicted(EvidenceTier::Deterministic, witness)
            }
            IdentityClaim::DistinctSubjects => Determination::supported(
                EvidenceTier::Deterministic,
                "molecular evidence separates the two artifacts",
            ),
            IdentityClaim::SingleSource => unreachable!("handled above"),
        };
    }

    let ambiguous: Vec<Missing> = signals
        .iter()
        .filter_map(|signal| match signal.concordance() {
            Some(Concordance::Ambiguous {
                reason,
                would_settle,
            }) => Some(Missing::new(
                would_settle.clone(),
                format!("{} was ambiguous: {reason}", signal.name()),
            )),
            _ => None,
        })
        .collect();
    if !ambiguous.is_empty() {
        return Determination::Unresolved(
            Unresolved::new(ambiguous).expect("the vector was checked non-empty"),
        );
    }

    let confirming = signals
        .iter()
        .any(|signal| signal.confirms() && signal.concordance() == Some(&Concordance::Concordant));

    match (claim, confirming) {
        (IdentityClaim::SameSubject, true) => Determination::supported(
            EvidenceTier::Deterministic,
            "a confirming molecular signal is concordant",
        ),
        (IdentityClaim::SameSubject, false) => Determination::unresolved(
            "a confirming molecular signal",
            "the available signals can falsify identity but cannot establish it",
        ),
        (IdentityClaim::DistinctSubjects, _) => Determination::unresolved(
            "a discordant molecular signal",
            "nothing separates these artifacts; concordance is not evidence of distinctness",
        ),
        (IdentityClaim::SingleSource, _) => unreachable!("handled above"),
    }
}

/// A specimen-derivation graph (31.05: "lineage completeness").
///
/// Deliberately a plain edge list rather than a tree: 32.05 generates duplicate aliquots and
/// same-patient longitudinal series, and both are legitimate graph shapes that a tree would reject
/// for the wrong reason.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lineage {
    parents: BTreeMap<SpecimenRef, SpecimenRef>,
    nodes: BTreeSet<SpecimenRef>,
}

impl Lineage {
    pub fn new() -> Self {
        Lineage::default()
    }

    pub fn declare(mut self, node: SpecimenRef) -> Self {
        self.nodes.insert(node);
        self
    }

    pub fn derive(mut self, child: SpecimenRef, parent: SpecimenRef) -> Self {
        self.nodes.insert(child.clone());
        self.nodes.insert(parent.clone());
        self.parents.insert(child, parent);
        self
    }

    /// Nodes with no recorded parent.
    ///
    /// Not an error: a primary specimen has no parent. The value of the list is that a *derived*
    /// model appearing in it is a hole in the lineage, and a caller who knows which nodes should be
    /// rooted can check the difference. This crate cannot know that, and says so rather than
    /// guessing from the identifier shape.
    pub fn unrooted(&self) -> BTreeSet<&SpecimenRef> {
        self.nodes
            .iter()
            .filter(|node| !self.parents.contains_key(*node))
            .collect()
    }

    /// Groups of nodes sharing one participant, which is what makes a random split leak.
    ///
    /// 32.05's failure risk "duplicates leak across splits" is not detectable from aliquot
    /// identifiers, which is why this groups on participant and returns every group of size greater
    /// than one rather than trying to decide which of them is a duplicate.
    pub fn participant_groups(&self) -> BTreeMap<String, BTreeSet<&SpecimenRef>> {
        let mut groups: BTreeMap<String, BTreeSet<&SpecimenRef>> = BTreeMap::new();
        for node in &self.nodes {
            groups
                .entry(node.participant.clone())
                .or_default()
                .insert(node);
        }
        groups.retain(|_, members| members.len() > 1);
        groups
    }

    /// Whether every node reaches a root without revisiting itself.
    ///
    /// A cycle in a derivation graph is a deterministic contradiction: nothing is derived from its
    /// own descendant. Returned as a [`Determination`] so it composes with the rest of the crate.
    pub fn acyclic(&self) -> Determination {
        for start in &self.nodes {
            let mut seen: BTreeSet<&SpecimenRef> = BTreeSet::new();
            let mut cursor = start;
            while let Some(parent) = self.parents.get(cursor) {
                if !seen.insert(cursor) || parent == start {
                    return Determination::contradicted(
                        EvidenceTier::Deterministic,
                        Witness::IdentityConflict {
                            left: start.label(),
                            right: parent.label(),
                            joined_on: "derivation edge".to_string(),
                            conflicting_evidence: "the derivation graph contains a cycle"
                                .to_string(),
                        },
                    );
                }
                cursor = parent;
            }
        }
        if self.nodes.is_empty() {
            return Determination::not_evaluable("the lineage graph has no nodes");
        }
        Determination::supported(
            EvidenceTier::Deterministic,
            "every derivation chain terminates",
        )
    }
}
