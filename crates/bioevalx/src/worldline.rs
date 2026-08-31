//! Four clocks per observation, and the leakage that shows up between them (26.07).
//!
//! 26.07's design detail is the whole module: "Every result includes an availability audit that
//! distinguishes when biology occurred, when it was measured, when it was recorded, and when the
//! system could access it." Most temporal-leakage bugs are invisible with fewer clocks. A
//! pathology result *about* day 10 that was signed out on day 24 is legitimately in a day-30
//! context and illegitimately in a day-12 one, and a system holding only the event date cannot
//! tell those apart — it sees "day 10" and concludes the evidence was available on day 12.
//!
//! # The audit is a partial order, not a timestamp
//!
//! [`Observation`] carries four [`bioprism_scope::Timestamp`]s and [`Observation::new`] refuses any
//! ordering that is physically impossible: a measurement before the biology, a record before the
//! measurement, access before the record. Those three refusals catch 26.07's "treatment date
//! shifted across scan" class at construction rather than at analysis.
//!
//! # Leakage is a witness, not a rate
//!
//! [`Worldline::audit`] returns a [`LeakWitness`] per offending observation, naming the decision,
//! the observation, and which clock made it inadmissible. 26.07 lists "future-leakage rate" as a
//! metric and never says what the denominator is — observations, decisions, or cells — so this
//! module produces the numerator's members and leaves the ratio to a caller who has chosen one.
//!
//! # Not implemented
//!
//! No forecast calibration and no censoring-aware metric. Both appear in 26.07's metric list; both
//! need a survival model and a scoring rule this crate does not have. `bioprism-bioeval` knows one
//! proper scoring rule and says so; nothing here duplicates it. No revision quality: 26.07 asks to
//! "measure belief revision after evidence", which needs the belief trajectory, and this module
//! sees only what was available, not what was believed. No clock skew model — record clocks and
//! event clocks are taken as given, because 26.07's step 2 ("validate event and record clocks")
//! describes a data-quality process over a real site's systems rather than a predicate over an
//! artifact.

use std::collections::BTreeMap;

use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};

use crate::error::WorldlineError;

const MAX_WORLDLINE_TEXT_BYTES: usize = 256;
const MAX_OBSERVATIONS: usize = 8192;
const MAX_DECISIONS: usize = 8192;
const MAX_CONTEXT_REFERENCES: usize = 4096;

/// Which of the four clocks a fact is being read on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Clock {
    /// When the biology happened.
    Occurred,
    /// When an instrument or a person measured it.
    Measured,
    /// When the measurement entered a record.
    Recorded,
    /// When the record became reachable by the system under evaluation.
    Accessible,
}

impl Clock {
    /// All four, in causal order.
    pub const ALL: [Clock; 4] = [
        Clock::Occurred,
        Clock::Measured,
        Clock::Recorded,
        Clock::Accessible,
    ];
}

/// One measured fact, with all four of its times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub id: String,
    occurred: Timestamp,
    measured: Timestamp,
    recorded: Timestamp,
    accessible: Timestamp,
}

impl Observation {
    /// Build an observation, refusing an impossible ordering.
    ///
    /// The fields are private and this is the only constructor, so an `Observation` in hand has
    /// already had its clocks checked. Equality of two clocks is allowed — a bedside measurement
    /// recorded instantly is real — only strict inversion is refused.
    pub fn new(
        id: impl Into<String>,
        occurred: Timestamp,
        measured: Timestamp,
        recorded: Timestamp,
        accessible: Timestamp,
    ) -> Result<Self, WorldlineError> {
        let id = id.into();
        validate_worldline_text(&id).map_err(|detail| WorldlineError::InvalidObservation {
            observation: id.clone(),
            detail,
        })?;
        if measured < occurred {
            return Err(WorldlineError::MeasuredBeforeOccurred {
                observation: id,
                occurred: occurred.to_rfc3339(),
                measured: measured.to_rfc3339(),
            });
        }
        if recorded < measured {
            return Err(WorldlineError::RecordedBeforeMeasured {
                observation: id,
                measured: measured.to_rfc3339(),
                recorded: recorded.to_rfc3339(),
            });
        }
        if accessible < recorded {
            return Err(WorldlineError::AccessibleBeforeRecorded {
                observation: id,
                recorded: recorded.to_rfc3339(),
                accessible: accessible.to_rfc3339(),
            });
        }
        Ok(Observation {
            id,
            occurred,
            measured,
            recorded,
            accessible,
        })
    }

    fn validate(&self) -> Result<(), WorldlineError> {
        validate_worldline_text(&self.id).map_err(|detail| WorldlineError::InvalidObservation {
            observation: self.id.clone(),
            detail,
        })?;
        if self.measured < self.occurred {
            return Err(WorldlineError::MeasuredBeforeOccurred {
                observation: self.id.clone(),
                occurred: self.occurred.to_rfc3339(),
                measured: self.measured.to_rfc3339(),
            });
        }
        if self.recorded < self.measured {
            return Err(WorldlineError::RecordedBeforeMeasured {
                observation: self.id.clone(),
                measured: self.measured.to_rfc3339(),
                recorded: self.recorded.to_rfc3339(),
            });
        }
        if self.accessible < self.recorded {
            return Err(WorldlineError::AccessibleBeforeRecorded {
                observation: self.id.clone(),
                recorded: self.recorded.to_rfc3339(),
                accessible: self.accessible.to_rfc3339(),
            });
        }
        Ok(())
    }

    /// Read one clock.
    pub fn at(&self, clock: Clock) -> Timestamp {
        match clock {
            Clock::Occurred => self.occurred,
            Clock::Measured => self.measured,
            Clock::Recorded => self.recorded,
            Clock::Accessible => self.accessible,
        }
    }

    /// The gap between the biology and the moment a system could see it.
    ///
    /// This is the quantity 26.07's failure mode "later pathology appears in earlier context"
    /// turns on, and it is invisible to a one-clock model.
    pub fn availability_lag_nanos(&self) -> i128 {
        self.accessible.as_nanos_utc() - self.occurred.as_nanos_utc()
    }
}

/// A point at which the system was asked to decide, and what it was given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    /// The moment the decision was made. 26.07 step 1: "freeze information available at each
    /// decision time."
    pub at: Timestamp,
    /// Observation ids that were placed in the decision's context.
    pub context: Vec<String>,
}

/// A concrete statement that a decision saw something it could not have seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakWitness {
    pub decision: String,
    pub observation: String,
    /// The clock that makes this inadmissible. Always [`Clock::Accessible`] today; carried
    /// explicitly so a caller can see *which* time was violated rather than inferring it.
    pub clock: Clock,
    pub decision_at: String,
    pub available_at: String,
}

/// A set of observations and the decisions taken over them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Worldline {
    observations: BTreeMap<String, Observation>,
    decisions: Vec<Decision>,
}

impl Worldline {
    /// An empty worldline.
    pub fn new() -> Self {
        Worldline::default()
    }

    /// Add an observation.
    pub fn observe(&mut self, observation: Observation) -> Result<(), WorldlineError> {
        observation.validate()?;
        if self.observations.len() >= MAX_OBSERVATIONS {
            return Err(WorldlineError::TooManyObservations {
                limit: MAX_OBSERVATIONS,
            });
        }
        if self.observations.contains_key(&observation.id) {
            return Err(WorldlineError::DuplicateObservation(observation.id));
        }
        self.observations
            .insert(observation.id.clone(), observation);
        Ok(())
    }

    /// Add a decision.
    pub fn decide(&mut self, decision: Decision) -> Result<(), WorldlineError> {
        self.validate_decision(&decision)?;
        if self.decisions.len() >= MAX_DECISIONS {
            return Err(WorldlineError::TooManyDecisions {
                limit: MAX_DECISIONS,
            });
        }
        if self.decisions.iter().any(|existing| existing.id == decision.id) {
            return Err(WorldlineError::DuplicateDecision(decision.id));
        }
        self.decisions.push(decision);
        Ok(())
    }

    /// Every observation a decision saw before it was available.
    ///
    /// An observation named in a decision's context but absent from the worldline is *not*
    /// reported as leakage. It is a different defect — an unresolvable reference — and reporting
    /// it here would let a harness hide leakage by dropping the observation record.
    /// [`Worldline::dangling`] reports those separately so neither can mask the other.
    pub fn audit(&self) -> Vec<LeakWitness> {
        let mut out = Vec::new();
        for decision in &self.decisions {
            for id in &decision.context {
                let Some(observation) = self.observations.get(id) else {
                    continue;
                };
                if observation.at(Clock::Accessible) > decision.at {
                    out.push(LeakWitness {
                        decision: decision.id.clone(),
                        observation: id.clone(),
                        clock: Clock::Accessible,
                        decision_at: decision.at.to_rfc3339(),
                        available_at: observation.at(Clock::Accessible).to_rfc3339(),
                    });
                }
            }
        }
        out
    }

    /// Context references that name no known observation.
    pub fn dangling(&self) -> Vec<(&str, &str)> {
        let mut out = Vec::new();
        for decision in &self.decisions {
            for id in &decision.context {
                if !self.observations.contains_key(id) {
                    out.push((decision.id.as_str(), id.as_str()));
                }
            }
        }
        out
    }

    /// The observations a decision *could* legitimately have used, whether or not it did.
    ///
    /// Together with the decision's own context this is the pair 26.07 needs: what was withheld
    /// and what was smuggled in are different findings, and a harness that reports only the second
    /// looks clean on a run that simply gave the system nothing.
    pub fn admissible_at(&self, at: Timestamp) -> Vec<&str> {
        self.observations
            .values()
            .filter(|o| o.at(Clock::Accessible) <= at)
            .map(|o| o.id.as_str())
            .collect()
    }

    /// An observation by id.
    pub fn observation(&self, id: &str) -> Option<&Observation> {
        self.observations.get(id)
    }

    /// The decisions recorded, in order.
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }

    fn validate(&self) -> Result<(), WorldlineError> {
        if self.observations.len() > MAX_OBSERVATIONS {
            return Err(WorldlineError::TooManyObservations {
                limit: MAX_OBSERVATIONS,
            });
        }
        for (key, observation) in &self.observations {
            if key != &observation.id {
                return Err(WorldlineError::InvalidObservation {
                    observation: observation.id.clone(),
                    detail: "observation map key must equal the observation id".into(),
                });
            }
            observation.validate()?;
        }
        if self.decisions.len() > MAX_DECISIONS {
            return Err(WorldlineError::TooManyDecisions {
                limit: MAX_DECISIONS,
            });
        }
        let mut ids = std::collections::BTreeSet::new();
        for decision in &self.decisions {
            if !ids.insert(decision.id.clone()) {
                return Err(WorldlineError::DuplicateDecision(decision.id.clone()));
            }
            self.validate_decision(decision)?;
        }
        Ok(())
    }

    fn validate_decision(&self, decision: &Decision) -> Result<(), WorldlineError> {
        validate_worldline_text(&decision.id).map_err(|detail| WorldlineError::InvalidDecision {
            decision: decision.id.clone(),
            detail,
        })?;
        if decision.context.len() > MAX_CONTEXT_REFERENCES {
            return Err(WorldlineError::TooManyContextReferences {
                limit: MAX_CONTEXT_REFERENCES,
            });
        }
        let mut context = std::collections::BTreeSet::new();
        for observation in &decision.context {
            validate_worldline_text(observation).map_err(|detail| {
                WorldlineError::InvalidDecision {
                    decision: decision.id.clone(),
                    detail: format!("context reference is invalid: {detail}"),
                }
            })?;
            if !context.insert(observation.clone()) {
                return Err(WorldlineError::DuplicateContextReference {
                    decision: decision.id.clone(),
                    observation: observation.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct WorldlineWire {
    observations: BTreeMap<String, Observation>,
    decisions: Vec<Decision>,
}

impl<'de> Deserialize<'de> for Worldline {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = WorldlineWire::deserialize(deserializer)?;
        let worldline = Worldline {
            observations: wire.observations,
            decisions: wire.decisions,
        };
        worldline.validate().map_err(serde::de::Error::custom)?;
        Ok(worldline)
    }
}

fn validate_worldline_text(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_WORLDLINE_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err("value must be a bounded, trimmed, control-free string".into());
    }
    Ok(())
}
