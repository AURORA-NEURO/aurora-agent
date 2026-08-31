//! What a system's response to a mutation says about it (26.12).
//!
//! `bioprism-mutation` already owns the generative half: it applies a mutation, checks the
//! declared relation's postcondition against the rebuilt world, and reports effective diversity
//! rather than instance count. This module owns the other half, which is a different question. The
//! mutation crate asks *did the mutation do what it said*. This one asks *did the system respond
//! the way the relation requires* — and 26.12's purpose sentence is precisely that: "Test whether
//! outputs change only when the declared biological semantics change."
//!
//! # Two failures, opposite in sign, both named
//!
//! 26.12's metric list contains "false invariance" and "false sensitivity" and they are not two
//! ends of one scale:
//!
//! - **False sensitivity** is a system whose answer moved when the biology did not — it keyed on a
//!   filename, an ordering, an alias. 26.12's "agent relies on filename shortcuts".
//! - **False invariance** is a system whose answer held still when the biology changed — it
//!   ignored a real molecular difference. 26.12's "model ignores meaningful molecular change".
//!
//! A single "metamorphic consistency" percentage adds them, and adding them is how a system that
//! never changes its answer at all scores well on half the suite. [`FamilyReport`] reports them
//! separately and offers no combined number.
//!
//! # An unresolved relation is not a pass
//!
//! [`Relation::DirectionalChange`] needs a direction, and a trial whose observed direction is
//! unknown — because the response was not comparable, or the trial errored — yields
//! [`TrialVerdict::Undetermined`], which counts toward neither consistency nor violation. 26.12's
//! last protocol step is "audit failures for oracle defects", and an undetermined trial is the
//! thing that audit is looking for.
//!
//! # Not implemented
//!
//! No robustness slope: 26.12 lists it and defines no perturbation magnitude axis to take a slope
//! along. No mutation-family coverage against the validated registry — `bioprism-mutation`'s
//! lineage owns which descendants are admitted, and duplicating its admission rule here would
//! create a second registry with its own opinion.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::MetamorphicError;

const MAX_METAMORPHIC_TEXT_BYTES: usize = 256;
const MAX_FAMILY_TRIALS: usize = 4096;
const MAX_SUITE_FAMILIES: usize = 1024;

/// What a mutation declares about how the system's answer should move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// The biology is unchanged; the answer must not move. Format, naming, aliasing, evidence
    /// order — 26.12's first three evaluation targets.
    Invariant,
    /// The biology changed in a known direction; the answer must move that way.
    DirectionalChange { expected: Direction },
}

/// Which way a response is expected or observed to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Increase,
    Decrease,
}

/// What the system actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "response")]
pub enum Response {
    /// The answer did not move.
    Unchanged,
    /// The answer moved in this direction.
    Moved { direction: Direction },
    /// The response could not be compared — the trial errored, or the outputs are not on a
    /// comparable scale. A real state, not a silent zero.
    Incomparable,
}

/// One mutated instance and the system's response to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    pub id: String,
    /// The mutation family this instance belongs to. Instances of one family are not independent
    /// evidence, which is why [`FamilyReport`] reports its family and never sums across families.
    pub relation: Relation,
    pub response: Response,
}

/// How one trial came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialVerdict {
    /// The response matched the declared relation.
    Consistent,
    /// The biology did not change and the answer did. A shortcut.
    FalseSensitivity,
    /// The biology changed and the answer did not. A blind spot.
    FalseInvariance,
    /// The biology changed and the answer moved the wrong way.
    WrongDirection,
    /// The response was not comparable, so the trial says nothing either way.
    Undetermined,
}

impl TrialVerdict {
    /// Whether this trial supports the relation.
    pub fn is_consistent(self) -> bool {
        matches!(self, TrialVerdict::Consistent)
    }

    /// Whether this trial is evidence at all.
    ///
    /// [`TrialVerdict::Undetermined`] is not. Every rate in [`FamilyReport`] is over the trials
    /// for which this returns `true`, and the count of the others is reported beside it so a
    /// reader can see how much of the suite failed to produce a reading.
    pub fn is_evidence(self) -> bool {
        !matches!(self, TrialVerdict::Undetermined)
    }
}

/// Classify one trial.
pub fn verdict(relation: Relation, response: Response) -> TrialVerdict {
    match (relation, response) {
        (_, Response::Incomparable) => TrialVerdict::Undetermined,
        (Relation::Invariant, Response::Unchanged) => TrialVerdict::Consistent,
        (Relation::Invariant, Response::Moved { .. }) => TrialVerdict::FalseSensitivity,
        (Relation::DirectionalChange { .. }, Response::Unchanged) => TrialVerdict::FalseInvariance,
        (Relation::DirectionalChange { expected }, Response::Moved { direction }) => {
            if expected == direction {
                TrialVerdict::Consistent
            } else {
                TrialVerdict::WrongDirection
            }
        }
    }
}

/// One mutation family's trials, all declaring the same relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Family {
    pub id: String,
    pub relation: Relation,
    trials: Vec<Trial>,
}

#[derive(Deserialize)]
struct FamilyWire {
    id: String,
    relation: Relation,
    trials: Vec<Trial>,
}

impl<'de> Deserialize<'de> for Family {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = FamilyWire::deserialize(deserializer)?;
        let mut family = Family::declaring(wire.id, wire.relation);
        for trial in wire.trials {
            family.record(trial).map_err(serde::de::Error::custom)?;
        }
        Ok(family)
    }
}

impl Family {
    /// Start a family under one declared relation.
    pub fn declaring(id: impl Into<String>, relation: Relation) -> Self {
        Family {
            id: id.into(),
            relation,
            trials: Vec::new(),
        }
    }

    /// Add a trial, refusing one whose relation differs from the family's.
    ///
    /// A family whose members declare different relations has no single consistency figure, and
    /// computing one anyway would average an invariance check with a directional one.
    pub fn record(&mut self, trial: Trial) -> Result<(), MetamorphicError> {
        validate_family_id(&self.id)?;
        validate_trial_id(&trial.id)?;
        if self.trials.len() >= MAX_FAMILY_TRIALS {
            return Err(MetamorphicError::TooManyTrials(MAX_FAMILY_TRIALS));
        }
        if self.trials.iter().any(|t| t.id == trial.id) {
            return Err(MetamorphicError::DuplicateTrial(trial.id));
        }
        if trial.relation != self.relation {
            return Err(MetamorphicError::RelationMismatch {
                trial: trial.id,
                relation: format!("{:?}", trial.relation),
                family: format!("{:?}", self.relation),
            });
        }
        self.trials.push(trial);
        Ok(())
    }

    /// Summarise, refusing an empty family.
    pub fn report(&self) -> Result<FamilyReport, MetamorphicError> {
        self.validate()?;
        if self.trials.is_empty() {
            return Err(MetamorphicError::EmptyFamily);
        }
        let mut report = FamilyReport {
            family: self.id.clone(),
            relation: self.relation,
            ..FamilyReport::default()
        };
        for trial in &self.trials {
            match verdict(trial.relation, trial.response) {
                TrialVerdict::Consistent => report.consistent += 1,
                TrialVerdict::FalseSensitivity => {
                    report.false_sensitivity += 1;
                    report.witnesses.push(trial.id.clone());
                }
                TrialVerdict::FalseInvariance => {
                    report.false_invariance += 1;
                    report.witnesses.push(trial.id.clone());
                }
                TrialVerdict::WrongDirection => {
                    report.wrong_direction += 1;
                    report.witnesses.push(trial.id.clone());
                }
                TrialVerdict::Undetermined => report.undetermined += 1,
            }
        }
        Ok(report)
    }

    /// The trials, in record order.
    pub fn trials(&self) -> &[Trial] {
        &self.trials
    }

    fn validate(&self) -> Result<(), MetamorphicError> {
        validate_family_id(&self.id)?;
        if self.trials.len() > MAX_FAMILY_TRIALS {
            return Err(MetamorphicError::TooManyTrials(MAX_FAMILY_TRIALS));
        }
        let mut ids = BTreeSet::new();
        for trial in &self.trials {
            validate_trial_id(&trial.id)?;
            if trial.relation != self.relation {
                return Err(MetamorphicError::RelationMismatch {
                    trial: trial.id.clone(),
                    relation: format!("{:?}", trial.relation),
                    family: format!("{:?}", self.relation),
                });
            }
            if !ids.insert(&trial.id) {
                return Err(MetamorphicError::DuplicateTrial(trial.id.clone()));
            }
        }
        Ok(())
    }
}

/// One family's outcome, with the two failure kinds kept apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FamilyReport {
    pub family: String,
    pub relation: Relation,
    pub consistent: usize,
    pub false_sensitivity: usize,
    pub false_invariance: usize,
    pub wrong_direction: usize,
    /// Trials that produced no reading. Never folded into any rate.
    pub undetermined: usize,
    /// The trial ids that failed, so a failure is a checkable object rather than a count.
    pub witnesses: Vec<String>,
}

impl Default for FamilyReport {
    fn default() -> Self {
        FamilyReport {
            family: String::new(),
            relation: Relation::Invariant,
            consistent: 0,
            false_sensitivity: 0,
            false_invariance: 0,
            wrong_direction: 0,
            undetermined: 0,
            witnesses: Vec::new(),
        }
    }
}

impl FamilyReport {
    /// Trials that produced a reading.
    pub fn evidential(&self) -> usize {
        self.consistent + self.false_sensitivity + self.false_invariance + self.wrong_direction
    }

    /// Consistency over the trials that produced a reading, or `None` when none did.
    ///
    /// `None` rather than `0.0`: a family in which every trial was incomparable has not been shown
    /// to be inconsistent, it has not been measured, and this crate's spine
    /// ([`crate::plane`]) exists to keep those apart.
    pub fn consistency(&self) -> Option<f64> {
        let denominator = self.evidential();
        if denominator == 0 {
            None
        } else {
            Some(self.consistent as f64 / denominator as f64)
        }
    }
}

/// A suite of families, reported family by family.
///
/// There is no method that returns a suite-wide consistency figure. 26.12's statistical-analysis
/// block says generated descendants "cannot be treated as independent observations merely because
/// they have different identifiers", and a suite average over families of unequal size is exactly
/// that treatment. What a suite offers instead is the worst family and the families that failed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Suite {
    families: Vec<Family>,
}

#[derive(Deserialize)]
struct SuiteWire {
    families: Vec<Family>,
}

impl<'de> Deserialize<'de> for Suite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SuiteWire::deserialize(deserializer)?;
        let mut suite = Suite::new();
        for family in wire.families {
            suite.add(family).map_err(serde::de::Error::custom)?;
        }
        Ok(suite)
    }
}

impl Suite {
    /// An empty suite.
    pub fn new() -> Self {
        Suite::default()
    }

    /// Add a family.
    pub fn add(&mut self, family: Family) -> Result<(), MetamorphicError> {
        family.validate()?;
        if self.families.len() >= MAX_SUITE_FAMILIES {
            return Err(MetamorphicError::TooManyFamilies(MAX_SUITE_FAMILIES));
        }
        if self.families.iter().any(|existing| existing.id == family.id) {
            return Err(MetamorphicError::DuplicateFamily(family.id));
        }
        self.families.push(family);
        Ok(())
    }

    /// One report per family, in the order they were added.
    pub fn reports(&self) -> Result<Vec<FamilyReport>, MetamorphicError> {
        if self.families.len() > MAX_SUITE_FAMILIES {
            return Err(MetamorphicError::TooManyFamilies(MAX_SUITE_FAMILIES));
        }
        self.families.iter().map(Family::report).collect()
    }

    /// Families with at least one shortcut or blind spot.
    pub fn failing(&self) -> Result<Vec<FamilyReport>, MetamorphicError> {
        Ok(self
            .reports()?
            .into_iter()
            .filter(|r| !r.witnesses.is_empty())
            .collect())
    }

    /// The relations this suite covers.
    ///
    /// A suite of only [`Relation::Invariant`] families cannot detect false invariance at all, and
    /// this is how a caller sees that before quoting a consistency figure.
    pub fn relations_covered(&self) -> BTreeSet<Relation> {
        self.families.iter().map(|f| f.relation).collect()
    }
}

fn validate_family_id(id: &str) -> Result<(), MetamorphicError> {
    if id.trim().is_empty() {
        return Err(MetamorphicError::InvalidFamily {
            id: id.to_string(),
            detail: "identifier must not be empty".into(),
        });
    }
    if id != id.trim() {
        return Err(MetamorphicError::InvalidFamily {
            id: id.to_string(),
            detail: "identifier must not have leading or trailing whitespace".into(),
        });
    }
    if id.len() > MAX_METAMORPHIC_TEXT_BYTES {
        return Err(MetamorphicError::InvalidFamily {
            id: id.to_string(),
            detail: format!("identifier exceeds {MAX_METAMORPHIC_TEXT_BYTES} bytes"),
        });
    }
    if id.chars().any(char::is_control) {
        return Err(MetamorphicError::InvalidFamily {
            id: id.to_string(),
            detail: "identifier contains a control character".into(),
        });
    }
    Ok(())
}

fn validate_trial_id(id: &str) -> Result<(), MetamorphicError> {
    if id.trim().is_empty() {
        return Err(MetamorphicError::InvalidTrial {
            id: id.to_string(),
            detail: "identifier must not be empty".into(),
        });
    }
    if id != id.trim() {
        return Err(MetamorphicError::InvalidTrial {
            id: id.to_string(),
            detail: "identifier must not have leading or trailing whitespace".into(),
        });
    }
    if id.len() > MAX_METAMORPHIC_TEXT_BYTES {
        return Err(MetamorphicError::InvalidTrial {
            id: id.to_string(),
            detail: format!("identifier exceeds {MAX_METAMORPHIC_TEXT_BYTES} bytes"),
        });
    }
    if id.chars().any(char::is_control) {
        return Err(MetamorphicError::InvalidTrial {
            id: id.to_string(),
            detail: "identifier contains a control character".into(),
        });
    }
    Ok(())
}
