//! Scoring the path without scoring one path (07.03).
//!
//! 07.03's responsibilities include "avoid exact-trajectory overfitting", and its design section
//! says how: "Prefer properties such as 'inspected authoritative evidence before irreversible
//! edit', 'did not repeat failed payment', or 'verified output after tool success' over one exact
//! tool sequence." Those three examples are three different shapes — an ordering constraint, a
//! prohibition on repetition, and a conditional follow-up — and they are the three
//! [`PathProperty`] variants. Nothing here compares a trajectory to a reference trajectory,
//! because a reference trajectory is the overfitting.
//!
//! # A property carries its witness
//!
//! A violated property returns the step indices that violated it, not a boolean. 07.03 exists to
//! be *diagnostic*, and "did not inspect evidence before the edit at step 7" is actionable in a way
//! that `path_score: 0.4` is not.
//!
//! # Immediate and downstream are separate numbers
//!
//! 07.03: "Evaluate whether a decision leads to improved state within a fixed horizon; report
//! immediate and downstream scores separately." [`BoundedSuffix`] holds both and offers no method
//! that combines them — a decision that looked good immediately and led somewhere bad is the
//! interesting case, and a single number is precisely the representation in which it disappears.
//!
//! The horizon must be declared: [`Trajectory::bounded_suffix`] refuses without one
//! ([`TrajectoryError::NoHorizon`]), because scoring to the end of the run makes the number depend
//! on how long the run happened to be, and a system that terminates early would score its
//! decisions over a shorter suffix than one that rambles.
//!
//! # Progress is a predicate, not a length
//!
//! 07.03: "Use task-specific progress predicates and state distance rather than token count or
//! number of tool calls." [`Step::progress`] is a caller-supplied state distance, and there is no
//! function in this module that counts steps and calls the result progress.
//!
//! # Not implemented
//!
//! No decision-quality model. 07.03's "score action feasibility, information value, risk,
//! uncertainty calibration" needs the action space and a value model; the acquisition half of it
//! is [`crate::acquisition`], and the rest is not defined by the section. No recovery-latency
//! metric beyond [`Trajectory::recovery`], which reports the steps between a failure and the next
//! strategy change and does not judge them. No partial observability: 07.03 lists it as a
//! responsibility and specifies nothing about how an unobserved step is represented.

use std::collections::BTreeSet;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::error::TrajectoryError;

const MAX_ACT_BYTES: usize = 512;
const MAX_PROPERTY_TEXT_BYTES: usize = 512;
const MAX_STEPS: usize = 100_000;
const MAX_PROPERTIES: usize = 10_000;

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() || value.trim() != value {
        return Err(format!("{field} must be non-empty and trimmed"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} exceeds the {max_bytes}-byte bound"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    Ok(())
}

/// What a step did, at the granularity a path property can reason about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// A caller-chosen act label: `inspect_evidence`, `edit_record`, `call_assay`. Properties are
    /// written over these labels.
    pub act: String,
    /// Whether this step cannot be undone. 07.03's canonical property is about ordering relative
    /// to an irreversible act, so the flag is first-class rather than encoded in the label.
    pub irreversible: bool,
    /// Whether the step succeeded.
    pub succeeded: bool,
    /// Distance to the goal state after this step, in a caller's own units. Lower is nearer.
    /// `None` when the caller has no state distance, which is honest and blocks progress claims
    /// rather than substituting step count.
    pub progress: Option<f64>,
}

impl Step {
    /// A successful, reversible step.
    pub fn new(act: impl Into<String>) -> Self {
        Step {
            act: act.into(),
            irreversible: false,
            succeeded: true,
            progress: None,
        }
    }

    /// Mark the step as irreversible.
    pub fn irreversible(mut self) -> Self {
        self.irreversible = true;
        self
    }

    /// Mark the step as failed.
    pub fn failed(mut self) -> Self {
        self.succeeded = false;
        self
    }

    /// Attach a state distance.
    pub fn at_distance(mut self, progress: f64) -> Self {
        self.progress = Some(progress);
        self
    }

    fn validate(&self, index: usize) -> Result<(), TrajectoryError> {
        validate_text("act", &self.act, MAX_ACT_BYTES)
            .map_err(|detail| TrajectoryError::InvalidStep { index, detail })?;
        if let Some(progress) = self.progress {
            if !(progress.is_finite() && progress >= 0.0) {
                return Err(TrajectoryError::InvalidStep {
                    index,
                    detail: "progress must be finite and non-negative".into(),
                });
            }
        }
        Ok(())
    }
}

/// A property a legitimate path must satisfy, in one of the three shapes 07.03 exemplifies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "shape")]
pub enum PathProperty {
    /// "inspected authoritative evidence before irreversible edit": every irreversible step
    /// matching `before` must be preceded somewhere by a successful `after` step.
    PrecededBy { before: String, after: String },
    /// "did not repeat failed payment": an act that failed must not be attempted again with no
    /// intervening step of a different act.
    NoBlindRetry { act: String },
    /// "verified output after tool success": a successful `trigger` must be followed by a
    /// `follow_up` before the run ends.
    FollowedBy { trigger: String, follow_up: String },
}

impl PathProperty {
    /// A stable name for reporting.
    pub fn name(&self) -> String {
        match self {
            PathProperty::PrecededBy { before, after } => format!("{after}-before-{before}"),
            PathProperty::NoBlindRetry { act } => format!("no-blind-retry-{act}"),
            PathProperty::FollowedBy { trigger, follow_up } => {
                format!("{follow_up}-after-{trigger}")
            }
        }
    }

    fn validate(&self) -> Result<(), TrajectoryError> {
        let result = match self {
            PathProperty::PrecededBy { before, after } => {
                validate_text("before", before, MAX_PROPERTY_TEXT_BYTES)
                    .and_then(|_| validate_text("after", after, MAX_PROPERTY_TEXT_BYTES))
            }
            PathProperty::NoBlindRetry { act } => {
                validate_text("act", act, MAX_PROPERTY_TEXT_BYTES)
            }
            PathProperty::FollowedBy { trigger, follow_up } => {
                validate_text("trigger", trigger, MAX_PROPERTY_TEXT_BYTES)
                    .and_then(|_| validate_text("follow_up", follow_up, MAX_PROPERTY_TEXT_BYTES))
            }
        };
        result.map_err(TrajectoryError::InvalidProperty)
    }
}

/// One property's outcome over one trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyOutcome {
    pub property: String,
    /// The step indices that violated it. Empty means held.
    pub violations: Vec<usize>,
    /// Whether the property had any opportunity to be violated. A property about irreversible
    /// edits over a trajectory containing none is *vacuous*, not satisfied, and reporting it as
    /// satisfied would credit a system for avoiding a situation it never entered.
    pub vacuous: bool,
}

impl PropertyOutcome {
    /// Whether the property was tested and held.
    pub fn held(&self) -> bool {
        self.violations.is_empty() && !self.vacuous
    }
}

/// Immediate and downstream outcomes of one decision, kept apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundedSuffix {
    pub step: usize,
    pub horizon: usize,
    /// State distance immediately after the decision, when the caller supplied one.
    pub immediate: Option<f64>,
    /// Best state distance reached within the horizon.
    pub downstream: Option<f64>,
    /// How many steps were actually available inside the horizon. A suffix truncated by the end of
    /// the run is a shorter observation, and comparing it to a full one without saying so is the
    /// bug the declared horizon exists to prevent.
    pub observed: usize,
}

impl BoundedSuffix {
    /// Whether the suffix ran to the full declared horizon.
    pub fn complete(&self) -> bool {
        self.observed == self.horizon
    }
}

/// A sequence of steps, and the properties it is judged against.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trajectory {
    steps: Vec<Step>,
    properties: Vec<PathProperty>,
}

impl Trajectory {
    /// A trajectory with no properties yet.
    pub fn of(steps: Vec<Step>) -> Self {
        Trajectory {
            steps,
            properties: Vec::new(),
        }
    }

    /// Construct a trajectory only after validating its persisted-input invariants.
    pub fn try_of(steps: Vec<Step>) -> Result<Self, TrajectoryError> {
        let trajectory = Self::of(steps);
        trajectory.validate()?;
        Ok(trajectory)
    }

    /// Validate the bounded, replayable trajectory representation.
    pub fn validate(&self) -> Result<(), TrajectoryError> {
        if self.steps.len() > MAX_STEPS {
            return Err(TrajectoryError::TooManySteps(self.steps.len()));
        }
        for (index, step) in self.steps.iter().enumerate() {
            step.validate(index)?;
        }
        if self.properties.len() > MAX_PROPERTIES {
            return Err(TrajectoryError::TooManyProperties(self.properties.len()));
        }
        let mut names = BTreeSet::new();
        for property in &self.properties {
            property.validate()?;
            let name = property.name();
            if !names.insert(name.clone()) {
                return Err(TrajectoryError::DuplicateProperty(name));
            }
        }
        Ok(())
    }

    /// Add a property, refusing a duplicate name.
    pub fn require(&mut self, property: PathProperty) -> Result<(), TrajectoryError> {
        self.validate_steps()?;
        property.validate()?;
        if self.properties.len() >= MAX_PROPERTIES {
            return Err(TrajectoryError::TooManyProperties(
                self.properties.len() + 1,
            ));
        }
        let name = property.name();
        if self.properties.iter().any(|p| p.name() == name) {
            return Err(TrajectoryError::DuplicateProperty(name));
        }
        self.properties.push(property);
        Ok(())
    }

    fn validate_steps(&self) -> Result<(), TrajectoryError> {
        if self.steps.len() > MAX_STEPS {
            return Err(TrajectoryError::TooManySteps(self.steps.len()));
        }
        for (index, step) in self.steps.iter().enumerate() {
            step.validate(index)?;
        }
        Ok(())
    }

    /// Check every declared property.
    pub fn check(&self) -> Vec<PropertyOutcome> {
        self.properties.iter().map(|p| self.check_one(p)).collect()
    }

    fn check_one(&self, property: &PathProperty) -> PropertyOutcome {
        let mut violations = Vec::new();
        let mut opportunities = 0usize;
        match property {
            PathProperty::PrecededBy { before, after } => {
                let mut seen_after = false;
                for (index, step) in self.steps.iter().enumerate() {
                    if step.act == *after && step.succeeded {
                        seen_after = true;
                    }
                    if step.act == *before && step.irreversible {
                        opportunities += 1;
                        if !seen_after {
                            violations.push(index);
                        }
                    }
                }
            }
            PathProperty::NoBlindRetry { act } => {
                let mut last_failed = false;
                for (index, step) in self.steps.iter().enumerate() {
                    if step.act == *act {
                        opportunities += 1;
                        if last_failed {
                            violations.push(index);
                        }
                        last_failed = !step.succeeded;
                    } else {
                        last_failed = false;
                    }
                }
            }
            PathProperty::FollowedBy { trigger, follow_up } => {
                for (index, step) in self.steps.iter().enumerate() {
                    if step.act != *trigger || !step.succeeded {
                        continue;
                    }
                    opportunities += 1;
                    let followed = self
                        .steps
                        .iter()
                        .skip(index + 1)
                        .any(|s| s.act == *follow_up);
                    if !followed {
                        violations.push(index);
                    }
                }
            }
        }
        PropertyOutcome {
            property: property.name(),
            violations,
            vacuous: opportunities == 0,
        }
    }

    /// Immediate and downstream state distance for the decision at `step`.
    ///
    /// Refuses a zero horizon: the bounded-suffix idea is that downstream is a *different* window
    /// from immediate, and a horizon of zero collapses them.
    pub fn bounded_suffix(
        &self,
        step: usize,
        horizon: usize,
    ) -> Result<BoundedSuffix, TrajectoryError> {
        self.validate()?;
        if step >= self.steps.len() {
            return Err(TrajectoryError::StepOutOfRange(step));
        }
        if horizon == 0 {
            return Err(TrajectoryError::NoHorizon);
        }
        let window: Vec<&Step> = self.steps.iter().skip(step + 1).take(horizon).collect();
        let downstream = window
            .iter()
            .filter_map(|s| s.progress)
            .fold(None::<f64>, |best, d| Some(best.map_or(d, |b| b.min(d))));
        Ok(BoundedSuffix {
            step,
            horizon,
            immediate: self.steps[step].progress,
            downstream,
            observed: window.len(),
        })
    }

    /// Steps between a failure and the next step with a different act.
    ///
    /// 07.03's "measure detection latency, retries, strategy change". Returns the pairs rather
    /// than a mean latency: a run with one instant recovery and one that never recovered has no
    /// meaningful average.
    pub fn recovery(&self) -> Vec<(usize, Option<usize>)> {
        let mut out = Vec::new();
        for (index, step) in self.steps.iter().enumerate() {
            if step.succeeded {
                continue;
            }
            let changed = self
                .steps
                .iter()
                .enumerate()
                .skip(index + 1)
                .find(|(_, s)| s.act != step.act)
                .map(|(i, _)| i - index);
            out.push((index, changed));
        }
        out
    }

    /// Acts that appear in the trajectory.
    pub fn acts(&self) -> BTreeSet<&str> {
        self.steps.iter().map(|s| s.act.as_str()).collect()
    }

    /// The steps.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The declared properties.
    pub fn properties(&self) -> &[PathProperty] {
        &self.properties
    }
}

impl<'de> Deserialize<'de> for Trajectory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct TrajectoryWire {
            steps: Vec<Step>,
            properties: Vec<PathProperty>,
        }

        let wire = TrajectoryWire::deserialize(deserializer)?;
        let trajectory = Trajectory::of(wire.steps);
        if trajectory.properties.len() > MAX_PROPERTIES {
            return Err(D::Error::custom(TrajectoryError::TooManyProperties(
                trajectory.properties.len(),
            )));
        }
        let mut trajectory = trajectory;
        trajectory.properties = wire.properties;
        trajectory.validate().map_err(D::Error::custom)?;
        Ok(trajectory)
    }
}
