//! Orthogonal and cross-modal confirmation (31.07), and the contradictions that stress it (32.13).
//!
//! 31.07's purpose: "Strengthen or challenge a claim using a measurement process with **different
//! failure modes**." The qualifier is the whole content. Two assays run off one aliquot, through one
//! preprocessing pipeline, against one reference build, are two readings of the same failure. Their
//! agreement is not confirmation and this module refuses to report it as such.
//!
//! # Confirmation requires a declared expectation
//!
//! 31.07's required functions include "define expected direction and tolerance". [`confirm`]
//! therefore takes an [`Expectation`], and there is no overload without one. Without a declared
//! direction, "the numbers came out similar" is a fact about two numbers and not about a claim.
//!
//! # Discordance is a question, not an error
//!
//! 31.07's worked case: "RNA and protein disagreement is not automatically error; the oracle asks
//! whether timing, regulation, compartment, or assay explains it." So a discordant pair produces
//! [`Determination::Unresolved`] listing the [`Explanation`]s still open, and the caller closes them
//! by supplying evidence. It becomes a contradiction only when every candidate explanation has been
//! ruled out — at which point the disagreement really is about the claim.
//!
//! # Two functions that are deliberately absent
//!
//! There is no majority vote across modalities and no privileged modality. Those are 32.13's first
//! two failure risks, in that order, and both are one convenience method away in any design that
//! returns a single call. The return type here is a determination over the *claim*, so there is
//! nowhere for a winner to be recorded.
//!
//! # Not implemented
//!
//! No effect-size arithmetic, no concordance correlation, no tolerance checking against real
//! measurements. [`Observation`] carries a caller's directional conclusion, not a value. 31.07's
//! "concordance conditional on quality" and "incremental information gain" need the measurements
//! themselves and a model of what quality means for each assay.

use std::collections::BTreeSet;

use bioprism_oracle::{EvidenceTier, SharedResource};
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Missing, Unresolved, Witness};

/// Something two modalities can have in common that would make them fail together.
///
/// The oracle-level taxonomy is `bioprism_oracle::SharedResource`, which covers what an oracle can
/// share with the *system it evaluates*. This adds the specimen-level channels two *measurements* can
/// share with each other, and wraps rather than restates the existing enum.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "shared", rename_all = "snake_case")]
pub enum SharedFailureMode {
    /// The same physical aliquot. A swap upstream moves both readings together.
    Aliquot,
    /// The same fixation, extraction, or library preparation.
    SamplePreparation { step: String },
    /// The same reference build, annotation release, or panel definition.
    ReferenceAsset { asset: String },
    /// Anything the oracle crate's independence declaration already names.
    Resource { resource: SharedResource },
}

/// One measurement channel.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Modality {
    pub name: String,
    /// Which specimen this channel measured. Two channels on different specimens cannot exclude
    /// [`Explanation::Identity`] or [`Explanation::Region`] by themselves.
    pub specimen: String,
    pub timepoint: String,
    pub shares: BTreeSet<SharedFailureMode>,
}

impl Modality {
    pub fn new(
        name: impl Into<String>,
        specimen: impl Into<String>,
        timepoint: impl Into<String>,
    ) -> Self {
        Modality {
            name: name.into(),
            specimen: specimen.into(),
            timepoint: timepoint.into(),
            shares: BTreeSet::new(),
        }
    }

    pub fn sharing(mut self, shared: impl IntoIterator<Item = SharedFailureMode>) -> Self {
        self.shares.extend(shared);
        self
    }
}

/// The direction a claim predicts a measurement will move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Higher,
    Lower,
    Unchanged,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Higher => "higher",
            Direction::Lower => "lower",
            Direction::Unchanged => "unchanged",
        }
    }
}

/// What the claim predicts, per modality, before either measurement is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expectation {
    pub claim: String,
    pub direction: Direction,
    /// The tolerance the caller declared, in the caller's own words. A string rather than a number
    /// because this crate holds no measurements to apply it to, and a number it could not use would
    /// be decoration.
    pub tolerance: String,
}

impl Expectation {
    pub fn new(
        claim: impl Into<String>,
        direction: Direction,
        tolerance: impl Into<String>,
    ) -> Self {
        Expectation {
            claim: claim.into(),
            direction,
            tolerance: tolerance.into(),
        }
    }
}

/// What one modality actually showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub modality: Modality,
    pub observed: Direction,
    /// Whether the caller judged this channel's quality adequate for the comparison. `None` means
    /// nobody assessed it, which 31.07's "concordance conditional on quality" treats as unknown
    /// rather than adequate.
    pub quality_adequate: Option<bool>,
}

impl Observation {
    pub fn new(modality: Modality, observed: Direction) -> Self {
        Observation {
            modality,
            observed,
            quality_adequate: None,
        }
    }
}

/// Why two modalities might disagree without either being wrong about the claim.
///
/// The six come from 31.07's worked case ("timing, regulation, compartment, or assay") and 32.13's
/// purpose ("because of biology, measurement, time, region, or identity").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Explanation {
    /// The two channels were measured at different times.
    Timing,
    /// A regulatory step sits between the two quantities.
    Regulation,
    /// The quantity is in a different cellular compartment in each channel.
    Compartment,
    /// A known limitation of one assay.
    Assay,
    /// The two channels may not be the same subject or specimen.
    Identity,
    /// The two channels sampled different parts of a heterogeneous lesion.
    Region,
}

impl Explanation {
    pub const ALL: [Explanation; 6] = [
        Explanation::Timing,
        Explanation::Regulation,
        Explanation::Compartment,
        Explanation::Assay,
        Explanation::Identity,
        Explanation::Region,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Explanation::Timing => "timing",
            Explanation::Regulation => "regulation",
            Explanation::Compartment => "compartment",
            Explanation::Assay => "assay",
            Explanation::Identity => "identity",
            Explanation::Region => "region",
        }
    }
}

/// Explanations the caller has ruled out, each with what ruled it out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Excluded {
    entries: BTreeSet<(Explanation, String)>,
}

impl Excluded {
    pub fn none() -> Self {
        Excluded::default()
    }

    /// Excluding an explanation requires saying what excluded it. An unexplained exclusion is
    /// indistinguishable from not having checked.
    pub fn rule_out(mut self, explanation: Explanation, evidence: impl Into<String>) -> Self {
        self.entries.insert((explanation, evidence.into()));
        self
    }

    pub fn contains(&self, explanation: Explanation) -> bool {
        self.entries
            .iter()
            .any(|(candidate, _)| *candidate == explanation)
    }

    pub fn evidence_for(&self, explanation: Explanation) -> Option<&str> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == explanation)
            .map(|(_, evidence)| evidence.as_str())
    }
}

/// The failure modes both modalities carry.
pub fn shared_failure_modes(left: &Modality, right: &Modality) -> BTreeSet<SharedFailureMode> {
    left.shares.intersection(&right.shares).cloned().collect()
}

/// Whether two channels are orthogonal enough for agreement between them to mean anything.
pub fn are_orthogonal(left: &Modality, right: &Modality) -> bool {
    shared_failure_modes(left, right).is_empty()
}

/// Explanations that the pair's own metadata leaves open regardless of what the caller excluded.
///
/// Different specimens leave identity and region open; different timepoints leave timing open. 31.07
/// asks implementations to "account for specimen and time mismatch", and the accounting has to be
/// automatic or it is the thing everyone forgets.
pub fn structurally_open(left: &Modality, right: &Modality) -> BTreeSet<Explanation> {
    let mut open = BTreeSet::new();
    if left.specimen != right.specimen {
        open.insert(Explanation::Identity);
        open.insert(Explanation::Region);
    }
    if left.timepoint != right.timepoint {
        open.insert(Explanation::Timing);
    }
    open
}

/// Decides what two channels say about one claim.
pub fn confirm(
    expectation: &Expectation,
    left: &Observation,
    right: &Observation,
    excluded: &Excluded,
) -> Determination {
    if expectation.tolerance.trim().is_empty() {
        return Determination::not_evaluable(
            "the expectation declares no tolerance, so agreement and disagreement are undefined",
        );
    }

    let concordant = left.observed == right.observed;

    if concordant {
        let shared = shared_failure_modes(&left.modality, &right.modality);
        if !shared.is_empty() {
            return Determination::unresolved(
                "a channel not sharing the pair's common failure modes",
                format!(
                    "{} and {} agree but share {:?}; agreement between them is not confirmation",
                    left.modality.name, right.modality.name, shared
                ),
            );
        }
        if left.observed != expectation.direction {
            return Determination::contradicted(
                EvidenceTier::Statistical,
                Witness::RelationViolated {
                    relation: format!("cross-modal expectation for '{}'", expectation.claim),
                    expected: expectation.direction.as_str().to_string(),
                    observed: format!(
                        "both channels {} (tolerance {})",
                        left.observed.as_str(),
                        expectation.tolerance
                    ),
                },
            );
        }
        return Determination::supported(
            EvidenceTier::Statistical,
            format!(
                "{} and {} independently moved {} as '{}' predicts",
                left.modality.name,
                right.modality.name,
                expectation.direction.as_str(),
                expectation.claim
            ),
        );
    }

    let structural = structurally_open(&left.modality, &right.modality);
    let open: Vec<Explanation> = Explanation::ALL
        .into_iter()
        .filter(|explanation| structural.contains(explanation) || !excluded.contains(*explanation))
        .collect();

    if open.is_empty() {
        return Determination::contradicted(
            EvidenceTier::Statistical,
            Witness::RelationViolated {
                relation: format!("cross-modal expectation for '{}'", expectation.claim),
                expected: format!(
                    "{} in both channels",
                    expectation.direction.as_str()
                ),
                observed: format!(
                    "{} {} while {} {}, with every candidate explanation ruled out",
                    left.modality.name,
                    left.observed.as_str(),
                    right.modality.name,
                    right.observed.as_str()
                ),
            },
        );
    }

    let missing = open.into_iter().map(|explanation| {
        Missing::new(
            format!("evidence ruling out {}", explanation.as_str()),
            if structural.contains(&explanation) {
                format!(
                    "{} and {} differ in specimen or timepoint, which leaves {} open",
                    left.modality.name,
                    right.modality.name,
                    explanation.as_str()
                )
            } else {
                format!(
                    "the channels disagree and {} has not been ruled out",
                    explanation.as_str()
                )
            },
        )
    });

    Determination::Unresolved(
        Unresolved::new(missing).expect("the open list was checked non-empty"),
    )
}
