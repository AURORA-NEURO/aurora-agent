//! Literature, citation staleness, and adversarial evidence (32.15).
//!
//! 32.15's worked relation: "A review cites a primary paper for a survival claim the primary paper
//! never tested. The agent must trace the claim rather than trust citation proximity."
//!
//! Tracing means the citation carries the passage. [`Citation::support`] answers
//! [`Determination::Unresolved`] when there is no passage — a citation with no quoted support is not
//! weak support, it is an unchecked assertion, and 32.15 calls that citation laundering. When the
//! passage is present, the caller declares what it asserts and the check is a set membership. This
//! crate does no natural-language inference and says so rather than approximating one.
//!
//! # Untrusted text stays data
//!
//! 32.15's operator list ends with "prompt injection in documents" and its failure risk with "embedded
//! instructions alter agent behavior". The defence here is a type boundary: [`Directive`] can only be
//! built from [`Provenance::Trusted`], so text arriving inside a retrieved document has no path to
//! becoming an instruction. [`Directive::from_document`] does not exist, and its absence is the
//! control.
//!
//! # Newer is not better
//!
//! 32.15's second failure risk is "newer source assumed better without relevance". [`preferred`]
//! therefore refuses to rank two versions of a work whose stated populations differ, and returns
//! unresolved naming the population as the gap. A peer-reviewed version in a different population is
//! not an upgrade over a preprint in the right one.
//!
//! # Not implemented
//!
//! No retrieval, no passage matching, no publication-graph traversal beyond a single supersession
//! link, no retraction database. 32.15's validation program lists "source-passage matching" and
//! "retraction/correction metadata"; both are data this crate consumes rather than produces.

use std::collections::BTreeSet;

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Witness};

/// Where a piece of text came from, and therefore what it is allowed to be.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum Provenance {
    /// Supplied by the operator of the system.
    Trusted { authority: String },
    /// Arrived inside retrieved content.
    Untrusted { origin: String },
}

/// Text that may not become an instruction.
///
/// A newtype with no `Deref`, no `AsRef<str>`, and no `Display`. Every one of those would let the
/// contents slip into a position where something treats them as a command; [`UntrustedText::read`]
/// is the only accessor and its name says what the caller is doing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UntrustedText {
    origin: String,
    text: String,
}

impl UntrustedText {
    pub fn new(origin: impl Into<String>, text: impl Into<String>) -> Self {
        UntrustedText {
            origin: origin.into(),
            text: text.into(),
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The contents, as data.
    pub fn read(&self) -> &str {
        &self.text
    }
}

/// An instruction the system may act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directive {
    authority: String,
    text: String,
}

impl Directive {
    /// The only constructor. Rejects untrusted provenance, so a directive cannot originate in a
    /// retrieved document. There is deliberately no `from_untrusted` and no escape hatch.
    pub fn new(provenance: &Provenance, text: impl Into<String>) -> Option<Self> {
        match provenance {
            Provenance::Trusted { authority } => Some(Directive {
                authority: authority.clone(),
                text: text.into(),
            }),
            Provenance::Untrusted { .. } => None,
        }
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Where a document sits in the publication lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PublicationStatus {
    Preprint,
    PeerReviewed,
    /// A correction was issued. The document still supports what survived the correction, which is
    /// why this is not the same as retraction.
    Corrected {
        detail: String,
    },
    Retracted {
        reason: String,
    },
}

/// A source, with the metadata a citation check needs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    /// The work this document is a version of. Two versions of one paper share it.
    pub work: String,
    pub status: PublicationStatus,
    /// The population the document's findings are about, in the caller's words.
    pub population: String,
    /// A caller-supplied ordering label. Not a clock.
    pub published_at: String,
}

impl Document {
    pub fn new(
        id: impl Into<String>,
        work: impl Into<String>,
        status: PublicationStatus,
        population: impl Into<String>,
        published_at: impl Into<String>,
    ) -> Self {
        Document {
            id: id.into(),
            work: work.into(),
            status,
            population: population.into(),
            published_at: published_at.into(),
        }
    }

    pub fn is_retracted(&self) -> bool {
        matches!(self.status, PublicationStatus::Retracted { .. })
    }
}

/// The quoted support for a claim, with what the caller says it asserts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passage {
    pub quoted: String,
    /// The claims this passage actually makes. A survival claim absent from this set is absent from
    /// the passage, whatever the surrounding prose implies.
    pub asserts: BTreeSet<String>,
}

impl Passage {
    pub fn new(
        quoted: impl Into<String>,
        asserts: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Passage {
            quoted: quoted.into(),
            asserts: asserts.into_iter().map(Into::into).collect(),
        }
    }
}

/// A claim, the document cited for it, and the passage — if anyone checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub claim: String,
    pub document: Document,
    pub passage: Option<Passage>,
    /// The population the claim is being made about, which may differ from the document's.
    pub claim_population: String,
}

impl Citation {
    pub fn new(
        claim: impl Into<String>,
        document: Document,
        claim_population: impl Into<String>,
    ) -> Self {
        Citation {
            claim: claim.into(),
            document,
            passage: None,
            claim_population: claim_population.into(),
        }
    }

    pub fn with_passage(mut self, passage: Passage) -> Self {
        self.passage = Some(passage);
        self
    }

    /// Whether this citation supports its claim.
    ///
    /// Retraction is checked first and decides regardless of the passage: a retracted paper's passage
    /// still says what it said, and that is exactly why the check cannot be passage-first.
    pub fn support(&self) -> Determination {
        if let PublicationStatus::Retracted { reason } = &self.document.status {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: format!("citation of {} supports '{}'", self.document.id, self.claim),
                    expected: "a standing publication".to_string(),
                    observed: format!("retracted: {reason}"),
                },
            );
        }
        let Some(passage) = &self.passage else {
            return Determination::unresolved(
                "a source passage stating the claim",
                format!(
                    "{} is cited for '{}' with nothing quoted from it",
                    self.document.id, self.claim
                ),
            );
        };
        if !passage.asserts.contains(&self.claim) {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: format!("passage from {} asserts the cited claim", self.document.id),
                    expected: self.claim.clone(),
                    observed: format!("the passage asserts {:?}", passage.asserts),
                },
            );
        }
        if self.document.population != self.claim_population {
            return Determination::unresolved(
                "evidence that the finding transports to the claim's population",
                format!(
                    "{} reports in '{}' and the claim is about '{}'",
                    self.document.id, self.document.population, self.claim_population
                ),
            );
        }
        Determination::supported(
            EvidenceTier::Property,
            format!(
                "{} quotes a passage asserting '{}' in the claim's own population",
                self.document.id, self.claim
            ),
        )
    }
}

/// Which of two versions of one work should be cited.
///
/// Refuses three ways. Different works are not versions of each other; different populations make the
/// comparison a transport question rather than a version question; and a retracted version never
/// wins, even when it is newer.
pub fn preferred<'a>(left: &'a Document, right: &'a Document) -> Determination {
    if left.work != right.work {
        return Determination::not_evaluable(
            "the two documents are not versions of one work, so there is no version to prefer",
        );
    }
    match (left.is_retracted(), right.is_retracted()) {
        (true, true) => {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: format!("a citable version of {} exists", left.work),
                    expected: "at least one standing version".to_string(),
                    observed: "every supplied version is retracted".to_string(),
                },
            )
        }
        (true, false) => {
            return Determination::supported(
                EvidenceTier::Deterministic,
                format!("{} stands and {} is retracted", right.id, left.id),
            )
        }
        (false, true) => {
            return Determination::supported(
                EvidenceTier::Deterministic,
                format!("{} stands and {} is retracted", left.id, right.id),
            )
        }
        (false, false) => {}
    }
    if left.population != right.population {
        return Determination::unresolved(
            "a shared population between the two versions",
            format!(
                "{} reports in '{}' and {} in '{}'; the later one is not automatically the better one",
                left.id, left.population, right.id, right.population
            ),
        );
    }
    match (&left.status, &right.status) {
        (PublicationStatus::Preprint, PublicationStatus::PeerReviewed) => Determination::supported(
            EvidenceTier::Property,
            format!("{} is the peer-reviewed version of {}", right.id, left.work),
        ),
        (PublicationStatus::PeerReviewed, PublicationStatus::Preprint) => Determination::supported(
            EvidenceTier::Property,
            format!("{} is the peer-reviewed version of {}", left.id, left.work),
        ),
        _ => Determination::unresolved(
            "a distinguishing publication status between the two versions",
            format!(
                "{} and {} share a status, so recency alone would decide",
                left.id, right.id
            ),
        ),
    }
}
