//! Deprecation as a lifecycle with stated reasons, not a comment in a changelog.
//!
//! Blueprint 14.16 requires "announcement, rationale, replacement, automated detection, migration
//! tool, minimum window, telemetry where consented, and final removal release", and 14.17 requires
//! that a deprecation "announce reason, replacement, affected resources, migration, timeline, and
//! score comparability". Both are lists of things a human should write down. What is encoded here
//! is the subset a type can enforce: a field moves `active -> deprecated -> sunset -> removed`, one
//! stage at a time, and every step carries a reason and a replacement or it does not happen.
//!
//! # There is no clock
//!
//! 14.16's "minimum window" and 14.17's "timeline" are wall-clock notions and this crate has no
//! wall clock, deliberately: a lifecycle that advances when the machine's date changes is not
//! reproducible, and every artifact in this workspace is supposed to be. So the lifecycle advances
//! on an **operator-supplied epoch** — a release number, a governance meeting, a sprint, whatever
//! the deployment decides — and [`LifecyclePolicy::min_epochs_in_stage`] is a dwell in those
//! units. Nothing here reads the system time, and no transition happens without somebody asking
//! for it.
//!
//! # Expedited removal waives the dwell, never the ladder
//!
//! 14.16 allows security emergencies to shorten the window and 14.17 says "critical security
//! removals can be immediate". Both are honoured by [`Transition::expedited`], which may collapse
//! the dwell to zero — all three transitions can land on one epoch — but still requires each stage
//! to be entered and each to carry its advisory. A field never appears in a `removed` record with
//! no `deprecated` record behind it, because the record of *why* is the thing an appeal (14.17)
//! and a withdrawal notice (14.19) are argued from.
//!
//! Not implemented: announcements, telemetry, appeals, migration-tool distribution, and the
//! comparability study that 14.16 requires when scoring semantics change. Those are process, and a
//! type that claimed to represent them would be claiming they happened.

use crate::diff::{FieldChange, SchemaDiff};
use crate::error::DeprecationError;
use crate::version::SchemaId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Where a field or format sits in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// In use, with no announced end.
    Active,
    /// Announced as going away. Still written, still read.
    Deprecated,
    /// No longer written. Still read, so archived documents stay resolvable.
    Sunset,
    /// Gone from the wire format. Historical documents keep it as an unknown field, which is why
    /// the declared compatibility mode of a format matters most *after* a removal.
    Removed,
}

impl Stage {
    pub const LADDER: [Stage; 4] = [Stage::Active, Stage::Deprecated, Stage::Sunset, Stage::Removed];

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Active => "active",
            Stage::Deprecated => "deprecated",
            Stage::Sunset => "sunset",
            Stage::Removed => "removed",
        }
    }

    /// The only stage that may follow this one.
    pub fn next(self) -> Option<Stage> {
        match self {
            Stage::Active => Some(Stage::Deprecated),
            Stage::Deprecated => Some(Stage::Sunset),
            Stage::Sunset => Some(Stage::Removed),
            Stage::Removed => None,
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a caller should use instead.
///
/// [`Replacement::Nothing`] exists because sometimes there genuinely is no successor — a field
/// that encoded a mistake, a format that should never have shipped. It carries a justification
/// rather than being expressible as an empty string, so "no replacement" is a statement somebody
/// made rather than a field nobody filled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "replacement", rename_all = "snake_case")]
pub enum Replacement {
    Field { path: String },
    Schema { id: SchemaId },
    Nothing { justification: String },
}

impl Replacement {
    pub fn field(path: impl Into<String>) -> Self {
        Replacement::Field { path: path.into() }
    }

    pub fn schema(id: SchemaId) -> Self {
        Replacement::Schema { id }
    }

    pub fn nothing(justification: impl Into<String>) -> Self {
        Replacement::Nothing {
            justification: justification.into(),
        }
    }

    fn is_stated(&self) -> bool {
        match self {
            Replacement::Field { path } => !path.trim().is_empty(),
            Replacement::Schema { .. } => true,
            Replacement::Nothing { justification } => !justification.trim().is_empty(),
        }
    }
}

impl fmt::Display for Replacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Replacement::Field { path } => write!(f, "use {path}"),
            Replacement::Schema { id } => write!(f, "use {id}"),
            Replacement::Nothing { justification } => write!(f, "no replacement: {justification}"),
        }
    }
}

/// One step along the ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub to: Stage,
    /// An operator-supplied ordinal. See the module docs on why it is not a timestamp.
    pub epoch: u64,
    pub reason: String,
    pub replacement: Replacement,
    /// Present when the dwell is being waived. 14.16 permits that for a security emergency and
    /// requires "explicit notice"; this is the notice.
    pub advisory: Option<String>,
}

impl Transition {
    pub fn new(
        to: Stage,
        epoch: u64,
        reason: impl Into<String>,
        replacement: Replacement,
    ) -> Self {
        Transition {
            to,
            epoch,
            reason: reason.into(),
            replacement,
            advisory: None,
        }
    }

    /// A transition that waives the dwell. It still cannot skip a stage.
    pub fn expedited(mut self, advisory: impl Into<String>) -> Self {
        self.advisory = Some(advisory.into());
        self
    }

    pub fn is_expedited(&self) -> bool {
        self.advisory.is_some()
    }
}

/// The dwell the blueprint calls a "minimum window", in operator epochs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecyclePolicy {
    /// How many epochs a subject must spend in a stage before advancing.
    ///
    /// One, not zero, so that the default forbids announcing a deprecation and removing the field
    /// in the same breath. 14.16 fixes no number; a deployment that wants three releases of notice
    /// sets three here rather than editing this crate.
    pub min_epochs_in_stage: u64,
}

impl Default for LifecyclePolicy {
    fn default() -> Self {
        LifecyclePolicy {
            min_epochs_in_stage: 1,
        }
    }
}

/// The lifecycle of one field or format, with the argument for every step it took.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeprecationRecord {
    pub subject: String,
    stage: Stage,
    entered_at: u64,
    history: Vec<Transition>,
}

impl DeprecationRecord {
    /// A subject that is in use and has no announced end.
    pub fn active(subject: impl Into<String>, epoch: u64) -> Self {
        DeprecationRecord {
            subject: subject.into(),
            stage: Stage::Active,
            entered_at: epoch,
            history: Vec::new(),
        }
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn entered_at(&self) -> u64 {
        self.entered_at
    }

    pub fn history(&self) -> &[Transition] {
        &self.history
    }

    pub fn is_removed(&self) -> bool {
        self.stage == Stage::Removed
    }

    /// The replacement named by the most recent transition, if the subject has moved at all.
    pub fn replacement(&self) -> Option<&Replacement> {
        self.history.last().map(|entry| &entry.replacement)
    }

    /// Advances one stage, or explains why it cannot.
    pub fn advance(
        &mut self,
        transition: Transition,
        policy: &LifecyclePolicy,
    ) -> Result<(), DeprecationError> {
        let subject = self.subject.clone();
        if transition.to <= self.stage {
            return Err(DeprecationError::StageReversed {
                subject,
                from: self.stage.to_string(),
                to: transition.to.to_string(),
            });
        }
        if Some(transition.to) != self.stage.next() {
            return Err(DeprecationError::StageSkipped {
                subject,
                from: self.stage.to_string(),
                to: transition.to.to_string(),
            });
        }
        if transition.reason.trim().is_empty() {
            return Err(DeprecationError::ReasonMissing {
                subject,
                to: transition.to.to_string(),
            });
        }
        if !transition.replacement.is_stated() {
            return Err(DeprecationError::ReplacementMissing {
                subject,
                to: transition.to.to_string(),
            });
        }
        if let Some(advisory) = &transition.advisory {
            if advisory.trim().is_empty() {
                return Err(DeprecationError::AdvisoryMissing {
                    subject,
                    to: transition.to.to_string(),
                });
            }
        }
        if transition.epoch < self.entered_at {
            return Err(DeprecationError::EpochNotAdvancing {
                subject,
                to: transition.to.to_string(),
                epoch: transition.epoch,
                previous: self.entered_at,
            });
        }
        if !transition.is_expedited() {
            let dwell = transition.epoch - self.entered_at;
            if dwell < policy.min_epochs_in_stage {
                return Err(DeprecationError::DwellTooShort {
                    subject,
                    stage: self.stage.to_string(),
                    observed: dwell,
                    required: policy.min_epochs_in_stage,
                });
            }
        }

        self.stage = transition.to;
        self.entered_at = transition.epoch;
        self.history.push(transition);
        Ok(())
    }
}

/// Every subject under lifecycle management, and the gate that ties it to a schema change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeprecationLedger {
    records: BTreeMap<String, DeprecationRecord>,
}

impl DeprecationLedger {
    pub fn new() -> Self {
        DeprecationLedger::default()
    }

    pub fn declare(&mut self, subject: impl Into<String>, epoch: u64) -> Result<(), DeprecationError> {
        let subject = subject.into();
        if self.records.contains_key(&subject) {
            return Err(DeprecationError::DuplicateSubject(subject));
        }
        self.records
            .insert(subject.clone(), DeprecationRecord::active(subject, epoch));
        Ok(())
    }

    pub fn advance(
        &mut self,
        subject: &str,
        transition: Transition,
        policy: &LifecyclePolicy,
    ) -> Result<(), DeprecationError> {
        match self.records.get_mut(subject) {
            None => Err(DeprecationError::RemovedWithoutRecord {
                path: subject.to_string(),
            }),
            Some(record) => record.advance(transition, policy),
        }
    }

    pub fn record(&self, subject: &str) -> Option<&DeprecationRecord> {
        self.records.get(subject)
    }

    pub fn stage_of(&self, subject: &str) -> Option<Stage> {
        self.records.get(subject).map(DeprecationRecord::stage)
    }

    pub fn subjects_at(&self, stage: Stage) -> Vec<&str> {
        self.records
            .values()
            .filter(|record| record.stage == stage)
            .map(|record| record.subject.as_str())
            .collect()
    }

    /// Checks a schema change against the lifecycle.
    ///
    /// This is the join between [`crate::diff`] and this module: a field may leave the wire format
    /// only once its record says `removed`. A removal with no record at all is the worse case and
    /// gets its own error, because it means the field went from shipping to gone with nothing
    /// written down anywhere.
    pub fn audit_removals(&self, diff: &SchemaDiff) -> Vec<DeprecationError> {
        diff.changes
            .iter()
            .filter_map(|change| match change {
                FieldChange::Removed(spec) => match self.stage_of(&spec.path) {
                    None => Some(DeprecationError::RemovedWithoutRecord {
                        path: spec.path.clone(),
                    }),
                    Some(Stage::Removed) => None,
                    Some(stage) => Some(DeprecationError::RemovedBeforeSunset {
                        path: spec.path.clone(),
                        stage: stage.to_string(),
                    }),
                },
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{FieldSpec, FieldType, SchemaDescriptor};
    use crate::diff::diff;
    use crate::mode::CompatibilityMode;

    fn policy() -> LifecyclePolicy {
        LifecyclePolicy::default()
    }

    fn record() -> DeprecationRecord {
        DeprecationRecord::active("world", 0)
    }

    fn deprecate(epoch: u64) -> Transition {
        Transition::new(
            Stage::Deprecated,
            epoch,
            "superseded by world_id, which carries the scope",
            Replacement::field("world_id"),
        )
    }

    #[test]
    fn a_field_cannot_go_straight_to_removed() {
        let mut subject = record();
        let error = subject
            .advance(
                Transition::new(
                    Stage::Removed,
                    3,
                    "cleanup",
                    Replacement::field("world_id"),
                ),
                &policy(),
            )
            .expect_err("the ladder is walked one rung at a time");
        assert!(matches!(error, DeprecationError::StageSkipped { .. }));
        assert_eq!(subject.stage(), Stage::Active);
    }

    #[test]
    fn the_lifecycle_does_not_run_backwards() {
        let mut subject = record();
        subject.advance(deprecate(1), &policy()).expect("advances");
        let error = subject
            .advance(
                Transition::new(Stage::Active, 2, "reinstated", Replacement::field("world")),
                &policy(),
            )
            .expect_err("undoing a deprecation is a new subject, not a reverse transition");
        assert!(matches!(error, DeprecationError::StageReversed { .. }));
    }

    #[test]
    fn a_transition_without_a_reason_does_not_happen() {
        let mut subject = record();
        let error = subject
            .advance(
                Transition::new(Stage::Deprecated, 1, "   ", Replacement::field("world_id")),
                &policy(),
            )
            .expect_err("14.16 requires a rationale");
        assert!(matches!(error, DeprecationError::ReasonMissing { .. }));
    }

    #[test]
    fn a_transition_without_a_stated_replacement_does_not_happen() {
        let mut subject = record();
        let error = subject
            .advance(
                Transition::new(Stage::Deprecated, 1, "cleanup", Replacement::field("  ")),
                &policy(),
            )
            .expect_err("14.16 requires a replacement");
        assert!(matches!(
            error,
            DeprecationError::ReplacementMissing { .. }
        ));
    }

    #[test]
    fn having_no_successor_is_allowed_but_must_be_argued_rather_than_left_blank() {
        let mut subject = record();
        assert!(matches!(
            subject.advance(
                Transition::new(Stage::Deprecated, 1, "encoded a mistake", Replacement::nothing("")),
                &policy(),
            ),
            Err(DeprecationError::ReplacementMissing { .. })
        ));
        subject
            .advance(
                Transition::new(
                    Stage::Deprecated,
                    1,
                    "encoded a mistake",
                    Replacement::nothing(
                        "the field conflated specimen and subject identity; no single successor \
                         can mean both",
                    ),
                ),
                &policy(),
            )
            .expect("a justified absence of a replacement is a statement, not a blank");
        assert_eq!(subject.stage(), Stage::Deprecated);
    }

    #[test]
    fn advancing_before_the_minimum_window_fails_without_an_advisory() {
        let mut subject = record();
        let error = subject
            .advance(deprecate(0), &policy())
            .expect_err("zero epochs of notice is not notice");
        assert!(matches!(
            error,
            DeprecationError::DwellTooShort {
                observed: 0,
                required: 1,
                ..
            }
        ));
    }

    #[test]
    fn a_security_emergency_may_collapse_the_whole_ladder_into_one_epoch_with_advisories() {
        let mut subject = record();
        let policy = LifecyclePolicy {
            min_epochs_in_stage: 4,
        };
        for stage in [Stage::Deprecated, Stage::Sunset, Stage::Removed] {
            subject
                .advance(
                    Transition::new(
                        stage,
                        0,
                        "the field leaks a holdout label",
                        Replacement::nothing("the value was never safe to publish"),
                    )
                    .expedited("advisory BIOPRISM-2026-01: immediate removal, see disclosure"),
                    &policy,
                )
                .expect("an expedited transition waives the dwell");
        }
        assert!(subject.is_removed());
        assert_eq!(subject.history().len(), 3);
        assert!(subject.history().iter().all(Transition::is_expedited));
    }

    #[test]
    fn an_expedited_transition_still_cannot_skip_a_stage() {
        let mut subject = record();
        let error = subject
            .advance(
                Transition::new(
                    Stage::Removed,
                    0,
                    "the field leaks a holdout label",
                    Replacement::nothing("the value was never safe to publish"),
                )
                .expedited("advisory BIOPRISM-2026-01"),
                &policy(),
            )
            .expect_err("urgency shortens the window, it does not erase the record");
        assert!(matches!(error, DeprecationError::StageSkipped { .. }));
    }

    #[test]
    fn an_expedited_transition_with_an_empty_advisory_is_refused() {
        let mut subject = record();
        let error = subject
            .advance(deprecate(0).expedited("   "), &policy())
            .expect_err("14.16 permits shortening only with explicit notice");
        assert!(matches!(error, DeprecationError::AdvisoryMissing { .. }));
    }

    #[test]
    fn an_epoch_that_moves_backwards_is_refused_even_when_expedited() {
        let mut subject = DeprecationRecord::active("world", 10);
        let error = subject
            .advance(deprecate(9).expedited("advisory"), &policy())
            .expect_err("epochs are an ordering, and this one is out of order");
        assert!(matches!(
            error,
            DeprecationError::EpochNotAdvancing {
                epoch: 9,
                previous: 10,
                ..
            }
        ));
    }

    #[test]
    fn the_history_records_the_argument_for_every_step_taken() {
        let mut subject = record();
        subject.advance(deprecate(1), &policy()).expect("advances");
        subject
            .advance(
                Transition::new(
                    Stage::Sunset,
                    2,
                    "no writer emits it as of release 2",
                    Replacement::field("world_id"),
                ),
                &policy(),
            )
            .expect("advances");
        assert_eq!(subject.stage(), Stage::Sunset);
        assert_eq!(subject.entered_at(), 2);
        assert_eq!(subject.history().len(), 2);
        assert_eq!(
            subject.replacement(),
            Some(&Replacement::field("world_id"))
        );
    }

    fn schema(version: &str, fields: Vec<FieldSpec>) -> SchemaDescriptor {
        SchemaDescriptor::new(
            crate::version::SchemaId::parse(&format!("test-format/{version}")).expect("parses"),
            CompatibilityMode::PreserveAndForward,
            fields,
        )
        .expect("well formed")
    }

    #[test]
    fn removing_a_field_that_never_had_a_deprecation_record_is_reported() {
        let from = schema(
            "1.0",
            vec![
                FieldSpec::required("world", FieldType::String),
                FieldSpec::required("query", FieldType::String),
            ],
        );
        let to = schema("2.0", vec![FieldSpec::required("query", FieldType::String)]);
        let ledger = DeprecationLedger::new();
        let findings = ledger.audit_removals(&diff(&from, &to).expect("lineage"));
        assert_eq!(findings.len(), 1);
        assert!(matches!(
            findings[0],
            DeprecationError::RemovedWithoutRecord { .. }
        ));
    }

    #[test]
    fn removing_a_field_that_is_only_deprecated_is_reported_with_the_stage_it_is_actually_at() {
        let from = schema(
            "1.0",
            vec![
                FieldSpec::required("world", FieldType::String),
                FieldSpec::required("query", FieldType::String),
            ],
        );
        let to = schema("2.0", vec![FieldSpec::required("query", FieldType::String)]);
        let mut ledger = DeprecationLedger::new();
        ledger.declare("world", 0).expect("new subject");
        ledger
            .advance("world", deprecate(1), &policy())
            .expect("advances");

        let findings = ledger.audit_removals(&diff(&from, &to).expect("lineage"));
        assert!(matches!(
            findings.as_slice(),
            [DeprecationError::RemovedBeforeSunset { stage, .. }] if stage == "deprecated"
        ));
    }

    #[test]
    fn a_field_whose_lifecycle_finished_may_leave_the_wire_format() {
        let from = schema("1.0", vec![FieldSpec::required("world", FieldType::String)]);
        let to = schema("2.0", vec![]);
        let mut ledger = DeprecationLedger::new();
        ledger.declare("world", 0).expect("new subject");
        ledger
            .advance("world", deprecate(1), &policy())
            .expect("advances");
        ledger
            .advance(
                "world",
                Transition::new(Stage::Sunset, 2, "no writer emits it", Replacement::field("world_id")),
                &policy(),
            )
            .expect("advances");
        ledger
            .advance(
                "world",
                Transition::new(Stage::Removed, 3, "readers no longer accept it", Replacement::field("world_id")),
                &policy(),
            )
            .expect("advances");

        assert_eq!(ledger.stage_of("world"), Some(Stage::Removed));
        assert!(ledger
            .audit_removals(&diff(&from, &to).expect("lineage"))
            .is_empty());
        assert_eq!(ledger.subjects_at(Stage::Removed), ["world"]);
    }

    #[test]
    fn declaring_the_same_subject_twice_is_refused() {
        let mut ledger = DeprecationLedger::new();
        ledger.declare("world", 0).expect("new subject");
        assert!(matches!(
            ledger.declare("world", 1),
            Err(DeprecationError::DuplicateSubject(subject)) if subject == "world"
        ));
    }
}
