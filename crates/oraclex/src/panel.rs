//! Experts as a measured reference process (31.06), and the policy mutations that vary it (32.18).
//!
//! 31.06's purpose is a warning about a shortcut: "use experts as a measured reference process
//! rather than an anonymous source of final labels". The shortcut is to take the majority call, ship
//! it as the label, and discard the reads. What is lost is everything the reference was made of.
//!
//! 32.18 states the invariant that stops it, as a metamorphic relation: "Changing the consensus rule
//! should change the reference distribution but not erase the original reads." So [`ReaderPanel`]
//! owns the reads and [`ConsensusRule`] is an argument to [`ReaderPanel::reference`], never a field.
//! Applying a rule cannot mutate the panel, because it takes `&self` and returns a fresh
//! [`ReferenceDistribution` ] that carries a copy of every read it was computed from. Two rules
//! applied to one panel therefore agree on the reads by construction, not by test — though the test
//! exists too, because the construction is the kind of thing a later refactor quietly loses.
//!
//! # A split panel is unresolved, not the first call
//!
//! [`ReferenceDistribution::consensus`] returns a [`Determination`]. When the rule does not settle,
//! it is [`Determination::Unresolved`] naming the split. There is no tiebreak, no "first listed", no
//! seniority. 31.06's metric list includes "adjudication change rate", which only means anything if
//! an unadjudicated split is visibly unadjudicated.
//!
//! # Blinding is a precondition, not a note
//!
//! 31.06's failure containment: "An expert adjudicator cannot see hidden system identity unless
//! required." [`Adjudication`] carries a [`Blinding`], and an adjudication that saw the system
//! identity cannot become the reference — [`ReferenceDistribution::consensus`] answers
//! [`Determination::Unresolved`] and names a blinded adjudication as the missing evidence. The
//! unblinded reads are kept; they are just not a reference.
//!
//! # Not implemented
//!
//! No agreement statistic. 31.06 lists reader agreement, model-to-reader distribution distance and
//! confidence calibration among its metrics, and every one of them is a choice with literature behind
//! it that this crate has no basis for making. [`ReferenceDistribution::per_reader`] returns the raw
//! per-reader comparison so a caller can compute whichever statistic they will defend.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::standard::ReferenceBasis;
use crate::verdict::Determination;

/// When a read was recorded relative to panel discussion.
///
/// 31.06's first required function is "record independent reads before discussion". A panel whose
/// reads are all [`ReadPhase::PostDiscussion`] has one opinion recorded several times, and the
/// distinction has to survive into the data or nobody can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadPhase {
    Independent,
    PostDiscussion,
}

/// One reader's call, with what they cited for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Read {
    pub reader: String,
    pub call: String,
    pub phase: ReadPhase,
    /// What the reader pointed at. Free-form, and empty when nobody recorded it — which 31.06's
    /// "capture confidence, evidence, and disagreement type" asks callers to avoid.
    pub evidence: Vec<String>,
}

impl Read {
    pub fn independent(reader: impl Into<String>, call: impl Into<String>) -> Self {
        Read {
            reader: reader.into(),
            call: call.into(),
            phase: ReadPhase::Independent,
            evidence: Vec::new(),
        }
    }

    pub fn post_discussion(reader: impl Into<String>, call: impl Into<String>) -> Self {
        Read {
            reader: reader.into(),
            call: call.into(),
            phase: ReadPhase::PostDiscussion,
            evidence: Vec::new(),
        }
    }

    pub fn citing(mut self, evidence: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.evidence.extend(evidence.into_iter().map(Into::into));
        self
    }
}

/// What the adjudicator was and was not shown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Blinding {
    /// Whether the adjudicator could see which system produced the candidate answer.
    pub system_identity_hidden: bool,
    /// Whether the adjudicator could see the other readers' calls before their own.
    pub peer_calls_hidden: bool,
}

impl Blinding {
    /// The blinding 31.06 asks for by default.
    pub fn blinded() -> Self {
        Blinding {
            system_identity_hidden: true,
            peer_calls_hidden: true,
        }
    }

    /// An adjudication run with the system identity visible. Constructible on purpose: the case
    /// exists in real panels and must be representable, so that it can be refused rather than
    /// silently absent.
    pub fn unblinded() -> Self {
        Blinding {
            system_identity_hidden: false,
            peer_calls_hidden: false,
        }
    }

    pub fn is_sufficient(&self) -> bool {
        self.system_identity_hidden
    }
}

/// A final call from an adjudicator, and the conditions it was made under.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Adjudication {
    pub adjudicator: String,
    pub call: String,
    pub blinding: Blinding,
}

impl Adjudication {
    pub fn new(
        adjudicator: impl Into<String>,
        call: impl Into<String>,
        blinding: Blinding,
    ) -> Self {
        Adjudication {
            adjudicator: adjudicator.into(),
            call: call.into(),
            blinding,
        }
    }
}

/// How a set of reads becomes a reference.
///
/// 32.18 mutates exactly this: "reader expertise and specialty", "consensus rule", "blinding and
/// information access". None of the variants carries a hardcoded fraction; a supermajority states its
/// own numerator and denominator, so a study that used four of five and a study that used two of
/// three are distinguishable in the record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum ConsensusRule {
    /// Every independent read agrees.
    Unanimous,
    /// Strictly more than half.
    Majority,
    /// At least `numerator` out of every `denominator` readers.
    SuperMajority { numerator: u32, denominator: u32 },
    /// A named adjudicator settles it.
    Adjudicated,
}

impl ConsensusRule {
    pub fn label(&self) -> String {
        match self {
            ConsensusRule::Unanimous => "unanimous".to_string(),
            ConsensusRule::Majority => "majority".to_string(),
            ConsensusRule::SuperMajority {
                numerator,
                denominator,
            } => format!("supermajority_{numerator}_of_{denominator}"),
            ConsensusRule::Adjudicated => "adjudicated".to_string(),
        }
    }
}

/// The reads, plus any adjudication. Owns nothing about how they combine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReaderPanel {
    reads: Vec<Read>,
    adjudication: Option<Adjudication>,
}

impl ReaderPanel {
    /// Rejects a panel that lists one reader twice: two reads from one reader are one opinion, and a
    /// panel that counts them twice reports a quorum it does not have. `crates/choreography` makes
    /// the same refusal for jurors reading one source.
    pub fn new(reads: impl IntoIterator<Item = Read>) -> Result<Self, OracleXError> {
        let reads: Vec<Read> = reads.into_iter().collect();
        let mut seen: BTreeSet<(&str, ReadPhase)> = BTreeSet::new();
        for read in &reads {
            if !seen.insert((read.reader.as_str(), read.phase)) {
                return Err(OracleXError::DuplicateReader {
                    reader: read.reader.clone(),
                });
            }
        }
        Ok(ReaderPanel {
            reads,
            adjudication: None,
        })
    }

    pub fn with_adjudication(mut self, adjudication: Adjudication) -> Self {
        self.adjudication = Some(adjudication);
        self
    }

    pub fn reads(&self) -> &[Read] {
        &self.reads
    }

    pub fn adjudication(&self) -> Option<&Adjudication> {
        self.adjudication.as_ref()
    }

    /// The reads recorded before discussion, which are the only ones a consensus rule may count.
    pub fn independent_reads(&self) -> Vec<&Read> {
        self.reads
            .iter()
            .filter(|read| read.phase == ReadPhase::Independent)
            .collect()
    }

    /// Applies a rule. Takes `&self`: no rule can consume or alter the panel.
    pub fn reference(&self, rule: ConsensusRule) -> Result<ReferenceDistribution, OracleXError> {
        let independent = self.independent_reads();
        if independent.is_empty() {
            return Err(OracleXError::EmptyPanel { rule: rule.label() });
        }
        let mut tally: BTreeMap<String, usize> = BTreeMap::new();
        for read in &independent {
            *tally.entry(read.call.clone()).or_insert(0) += 1;
        }
        Ok(ReferenceDistribution {
            rule,
            tally,
            reads: self.reads.clone(),
            adjudication: self.adjudication.clone(),
        })
    }
}

/// A reference computed under one rule, carrying the reads it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceDistribution {
    pub rule: ConsensusRule,
    tally: BTreeMap<String, usize>,
    reads: Vec<Read>,
    adjudication: Option<Adjudication>,
}

impl ReferenceDistribution {
    /// Every read that went into this reference. 32.18's relation lives here: this is a copy of the
    /// panel's reads, so no rule can shrink it.
    pub fn reads(&self) -> &[Read] {
        &self.reads
    }

    /// Counts per call.
    pub fn tally(&self) -> &BTreeMap<String, usize> {
        &self.tally
    }

    pub fn readers(&self) -> usize {
        self.reads
            .iter()
            .filter(|read| read.phase == ReadPhase::Independent)
            .count()
    }

    /// Calls held by at least one reader but not by the plurality.
    ///
    /// 32.18's failure risk is "minority supported view erased". A minority call with cited evidence
    /// is a position the reference has to keep even when the rule does not select it.
    pub fn minority_calls(&self) -> BTreeSet<&str> {
        let peak = self.tally.values().copied().max().unwrap_or(0);
        self.tally
            .iter()
            .filter(|(_, count)| **count < peak)
            .map(|(call, _)| call.as_str())
            .collect()
    }

    /// What this reference says, under its rule.
    pub fn consensus(&self) -> Determination {
        let total = self.readers();
        let peak = self.tally.values().copied().max().unwrap_or(0);
        let leaders: Vec<&str> = self
            .tally
            .iter()
            .filter(|(_, count)| **count == peak)
            .map(|(call, _)| call.as_str())
            .collect();

        let settled = match self.rule {
            ConsensusRule::Unanimous => leaders.len() == 1 && peak == total,
            ConsensusRule::Majority => leaders.len() == 1 && peak * 2 > total,
            ConsensusRule::SuperMajority {
                numerator,
                denominator,
            } => {
                denominator > 0
                    && leaders.len() == 1
                    && peak as u64 * denominator as u64 >= numerator as u64 * total as u64
            }
            ConsensusRule::Adjudicated => match &self.adjudication {
                Some(adjudication) => {
                    if !adjudication.blinding.is_sufficient() {
                        return Determination::unresolved(
                            "a blinded adjudication",
                            format!(
                                "adjudicator {} could see the system identity, so their call is not a reference",
                                adjudication.adjudicator
                            ),
                        );
                    }
                    return Determination::supported(
                        ReferenceBasis::ReaderConsensus {
                            rule: self.rule.label(),
                            readers: total,
                        }
                        .ceiling(),
                        format!(
                            "adjudicator {} called {} under blinding",
                            adjudication.adjudicator, adjudication.call
                        ),
                    );
                }
                None => {
                    return Determination::unresolved(
                        "an adjudication",
                        "the rule is adjudicated and no adjudicator has ruled",
                    )
                }
            },
        };

        if settled {
            Determination::supported(
                ReferenceBasis::ReaderConsensus {
                    rule: self.rule.label(),
                    readers: total,
                }
                .ceiling(),
                format!(
                    "{} of {} independent reads called {} under the {} rule",
                    peak,
                    total,
                    leaders[0],
                    self.rule.label()
                ),
            )
        } else {
            Determination::unresolved(
                format!("a call reaching the {} threshold", self.rule.label()),
                format!(
                    "the panel split {:?} across {} independent reads",
                    self.tally, total
                ),
            )
        }
    }

    /// Whether the model's call matches each reader's, one reader at a time.
    ///
    /// 31.06's worked case asks for exactly this shape: "model agreement with each reader and the
    /// adjudicated distribution, rather than rewarding only the final majority label". There is no
    /// method here that reduces the map to a score.
    pub fn per_reader(&self, model_call: &str) -> BTreeMap<&str, bool> {
        self.reads
            .iter()
            .filter(|read| read.phase == ReadPhase::Independent)
            .map(|read| (read.reader.as_str(), read.call == model_call))
            .collect()
    }
}
