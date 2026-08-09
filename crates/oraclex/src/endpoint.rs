//! Survival, endpoint, and clinical-research reference standards (31.12).
//!
//! 31.12's worked case: "A death date from one source and last-contact date from another are
//! reconciled under a declared source hierarchy and uncertainty rule." Two halves, and the second one
//! is where implementations fail. Reconciling under a hierarchy is easy. Saying so when the hierarchy
//! does not cover the pair — instead of taking whichever source the loop reached first — is the part
//! that has to be built deliberately.
//!
//! [`reconcile`] returns [`Determination::Unresolved`] naming the unranked pair, and
//! [`Reconciliation`] retains every dropped claim so a later hierarchy can revisit the decision. That
//! is `crates/choreography`'s rule in the endpoint's own currency: a dispute the evidence cannot
//! settle returns unresolved with the missing evidence named, never a default to the
//! higher-authority party.
//!
//! # Unknown is not censored and not ineligible
//!
//! 31.12's required functions end with "capture unknown and not-evaluable states", and its metric
//! list includes "criterion-level accuracy". Two places here honour that:
//!
//! * [`Outcome::Unknown`] has no path to [`Outcome::Censored`]. Imputing last contact as a censoring
//!   time for a subject nobody followed silently converts loss to follow-up into administrative
//!   censoring, and the two carry different bias.
//! * [`Eligibility::assess`] answers [`Determination::Unresolved`] when any criterion is unknown and
//!   no known criterion has already failed. A screening rule that treats unknown as ineligible
//!   excludes on missingness.
//!
//! # Versions are part of the endpoint
//!
//! 31.12 requires pinned response and protocol versions. [`Assessment`] carries its
//! [`ResponseCriteria`], and [`comparable`] refuses to compare two assessments made under different
//! criteria versions. They are not two measurements of one quantity.
//!
//! # Not implemented
//!
//! No survival estimation, no competing-risk model, no time-to-event arithmetic beyond ordering.
//! 31.12's "endpoint reproducibility" and "censoring completeness" are cohort-level statistics.
//! [`FollowUp::consistency`] checks the one thing that is decidable per subject: that an event did not
//! precede the record's own start or follow its own last contact.

use std::collections::BTreeMap;

use bioprism_oracle::{EvidenceTier, UtcTimestamp};
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::verdict::{Determination, Witness};

/// A declared precedence over data sources, strongest first.
///
/// Explicitly a total order over the sources it lists and silent about any it does not. The silence
/// is the feature: an unlisted source produces [`OracleXError::UnrankedSources`] rather than a
/// default rank.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHierarchy {
    ranked: Vec<String>,
}

impl SourceHierarchy {
    /// `sources` strongest first.
    pub fn new(sources: impl IntoIterator<Item = impl Into<String>>) -> Self {
        SourceHierarchy {
            ranked: sources.into_iter().map(Into::into).collect(),
        }
    }

    pub fn rank(&self, source: &str) -> Option<usize> {
        self.ranked.iter().position(|entry| entry == source)
    }

    /// Which of two sources wins, or an error naming the pair the hierarchy does not cover.
    pub fn prefer<'a>(&self, left: &'a str, right: &'a str) -> Result<&'a str, OracleXError> {
        match (self.rank(left), self.rank(right)) {
            (Some(l), Some(r)) if l < r => Ok(left),
            (Some(l), Some(r)) if r < l => Ok(right),
            _ => Err(OracleXError::UnrankedSources {
                left: left.to_string(),
                right: right.to_string(),
            }),
        }
    }
}

/// One source's assertion about one dated field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DateClaim {
    pub field: String,
    pub value: UtcTimestamp,
    pub source: String,
}

impl DateClaim {
    pub fn new(field: impl Into<String>, value: UtcTimestamp, source: impl Into<String>) -> Self {
        DateClaim {
            field: field.into(),
            value,
            source: source.into(),
        }
    }
}

/// The result of reconciling several sources for one field.
///
/// `dropped` is retained deliberately. 31.15's dissent-retention requirement applies to dates as much
/// as to opinions: a reconciliation that deletes the losing claim cannot be revisited when the
/// hierarchy changes, and 31.11 forbids rewriting history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reconciliation {
    pub outcome: Determination,
    pub kept: Option<DateClaim>,
    pub dropped: Vec<DateClaim>,
}

/// Reconciles every claim about one field under a declared hierarchy.
pub fn reconcile(claims: &[DateClaim], hierarchy: &SourceHierarchy) -> Reconciliation {
    if claims.is_empty() {
        return Reconciliation {
            outcome: Determination::not_evaluable("no source asserted a value for this field"),
            kept: None,
            dropped: Vec::new(),
        };
    }
    if claims.iter().all(|claim| claim.value == claims[0].value) {
        return Reconciliation {
            outcome: Determination::supported(
                EvidenceTier::Deterministic,
                format!(
                    "{} source(s) agree on {}",
                    claims.len(),
                    claims[0].field
                ),
            ),
            kept: Some(claims[0].clone()),
            dropped: Vec::new(),
        };
    }

    let mut best = &claims[0];
    for candidate in &claims[1..] {
        match hierarchy.prefer(best.source.as_str(), candidate.source.as_str()) {
            Ok(winner) => {
                if winner == candidate.source {
                    best = candidate;
                }
            }
            Err(OracleXError::UnrankedSources { left, right }) => {
                return Reconciliation {
                    outcome: Determination::unresolved(
                        format!("a hierarchy rank for '{left}' against '{right}'"),
                        format!(
                            "sources disagree on {} and the declared hierarchy does not rank them",
                            candidate.field
                        ),
                    ),
                    kept: None,
                    dropped: claims.to_vec(),
                }
            }
            Err(_) => unreachable!("prefer returns only UnrankedSources"),
        }
    }

    let dropped: Vec<DateClaim> = claims
        .iter()
        .filter(|claim| claim.source != best.source)
        .cloned()
        .collect();
    let basis = format!(
        "{} from {} outranks {} other claim(s) under the declared hierarchy",
        best.value.as_str(),
        best.source,
        dropped.len()
    );
    Reconciliation {
        outcome: Determination::supported(EvidenceTier::Deterministic, basis),
        kept: Some(best.clone()),
        dropped,
    }
}

/// The witness recording which claim a reconciliation dropped.
///
/// Separate from [`reconcile`] because a reconciliation that settled is [`Determination::Supported`]
/// and support carries no witnesses. The override still happened and still has to be auditable, so it
/// is available here rather than discarded.
pub fn override_witnesses(reconciliation: &Reconciliation) -> Vec<Witness> {
    let Some(kept) = &reconciliation.kept else {
        return Vec::new();
    };
    reconciliation
        .dropped
        .iter()
        .map(|loser| Witness::SourceOverridden {
            field: kept.field.clone(),
            kept: kept.value.as_str().to_string(),
            kept_source: kept.source.clone(),
            dropped: loser.value.as_str().to_string(),
            dropped_source: loser.source.clone(),
        })
        .collect()
}

/// What happened to one subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The endpoint event occurred, with its cause.
    Event { cause: String, at: UtcTimestamp },
    /// A different event occurred first and precludes the endpoint (31.12: competing risks).
    CompetingEvent { cause: String, at: UtcTimestamp },
    /// The subject was followed to `at` without the event.
    Censored { at: UtcTimestamp },
    /// Nobody knows. Has no conversion to any of the above.
    Unknown { reason: String },
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Event { .. } => "event",
            Outcome::CompetingEvent { .. } => "competing_event",
            Outcome::Censored { .. } => "censored",
            Outcome::Unknown { .. } => "unknown",
        }
    }

    /// The instant, when there is one. `None` for [`Outcome::Unknown`], and there is deliberately no
    /// `at_or_last_contact` helper: that helper is how loss to follow-up becomes censoring.
    pub fn at(&self) -> Option<&UtcTimestamp> {
        match self {
            Outcome::Event { at, .. }
            | Outcome::CompetingEvent { at, .. }
            | Outcome::Censored { at } => Some(at),
            Outcome::Unknown { .. } => None,
        }
    }
}

/// One subject's follow-up record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowUp {
    pub subject: String,
    /// 31.12's "define source hierarchy for dates" begins with the time origin, which is itself a
    /// choice and is recorded rather than assumed.
    pub time_origin: UtcTimestamp,
    pub last_contact: UtcTimestamp,
    pub outcome: Outcome,
}

impl FollowUp {
    pub fn new(
        subject: impl Into<String>,
        time_origin: UtcTimestamp,
        last_contact: UtcTimestamp,
        outcome: Outcome,
    ) -> Self {
        FollowUp {
            subject: subject.into(),
            time_origin,
            last_contact,
            outcome,
        }
    }

    /// The two temporal impossibilities a single record can prove on its own.
    ///
    /// An event before the time origin, or after the last contact, is a deterministic contradiction —
    /// 31.02's sense of the word, a proof of defect rather than an anomaly. An unknown outcome is
    /// [`Determination::NotEvaluable`]: there is nothing to check, and reporting it as consistent
    /// would be reporting a check that did not run.
    pub fn consistency(&self) -> Determination {
        let Some(at) = self.outcome.at() else {
            return Determination::not_evaluable(
                "the outcome has no date, so no temporal check applies",
            );
        };
        if at < &self.time_origin {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: format!("{} outcome follows its time origin", self.subject),
                    expected: format!("at or after {}", self.time_origin.as_str()),
                    observed: at.as_str().to_string(),
                },
            );
        }
        if at > &self.last_contact {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: format!("{} outcome precedes its last contact", self.subject),
                    expected: format!("at or before {}", self.last_contact.as_str()),
                    observed: at.as_str().to_string(),
                },
            );
        }
        Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "{} {} at {} lies within [{}, {}]",
                self.subject,
                self.outcome.as_str(),
                at.as_str(),
                self.time_origin.as_str(),
                self.last_contact.as_str()
            ),
        )
    }
}

/// A pinned response-criteria version (31.12: "pin response and protocol versions").
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResponseCriteria {
    pub name: String,
    pub version: String,
}

impl ResponseCriteria {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        ResponseCriteria {
            name: name.into(),
            version: version.into(),
        }
    }
}

/// One response call under one criteria version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assessment {
    pub criteria: ResponseCriteria,
    pub call: String,
}

impl Assessment {
    pub fn new(criteria: ResponseCriteria, call: impl Into<String>) -> Self {
        Assessment {
            criteria,
            call: call.into(),
        }
    }
}

/// Whether two assessments may be compared at all.
///
/// Different criteria versions define different calls, so a disagreement between them is not evidence
/// about the subject. Answering [`Determination::Unresolved`] rather than comparing anyway is the
/// whole point of pinning the version.
pub fn comparable(left: &Assessment, right: &Assessment) -> Determination {
    if left.criteria == right.criteria {
        Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "both assessments used {} {}",
                left.criteria.name, left.criteria.version
            ),
        )
    } else {
        Determination::unresolved(
            "an assessment under a common criteria version",
            format!(
                "one call used {} {} and the other used {} {}",
                left.criteria.name,
                left.criteria.version,
                right.criteria.name,
                right.criteria.version
            ),
        )
    }
}

/// A trial-eligibility screen, one criterion at a time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eligibility {
    criteria: BTreeMap<String, Option<bool>>,
}

impl Eligibility {
    pub fn new() -> Self {
        Eligibility::default()
    }

    pub fn met(mut self, criterion: impl Into<String>) -> Self {
        self.criteria.insert(criterion.into(), Some(true));
        self
    }

    pub fn failed(mut self, criterion: impl Into<String>) -> Self {
        self.criteria.insert(criterion.into(), Some(false));
        self
    }

    /// A criterion nobody has data for. Not the same as failing it.
    pub fn unknown(mut self, criterion: impl Into<String>) -> Self {
        self.criteria.insert(criterion.into(), None);
        self
    }

    /// Whether the subject is eligible.
    ///
    /// A failed criterion decides regardless of what is unknown — a definite exclusion is definite.
    /// An unknown criterion with no failures is [`Determination::Unresolved`] naming the criterion,
    /// so a screening pipeline reports "we do not know" instead of quietly excluding on missingness.
    pub fn assess(&self) -> Determination {
        if self.criteria.is_empty() {
            return Determination::not_evaluable("no eligibility criteria were declared");
        }
        if let Some((criterion, _)) = self
            .criteria
            .iter()
            .find(|(_, status)| **status == Some(false))
        {
            return Determination::contradicted(
                EvidenceTier::Deterministic,
                Witness::RelationViolated {
                    relation: "eligibility".to_string(),
                    expected: format!("{criterion} met"),
                    observed: format!("{criterion} not met"),
                },
            );
        }
        if let Some((criterion, _)) = self.criteria.iter().find(|(_, status)| status.is_none()) {
            return Determination::unresolved(
                format!("a value for criterion '{criterion}'"),
                "an unknown criterion is not a failed criterion",
            );
        }
        Determination::supported(
            EvidenceTier::Deterministic,
            format!("all {} criteria are met", self.criteria.len()),
        )
    }
}
