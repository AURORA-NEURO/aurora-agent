//! What redaction costs an evaluator, stated rather than absorbed.
//!
//! Implements the *evaluation impact* half of blueprint 04.05 (Redaction, Privacy and Data
//! Minimization). The mechanics half — receipts, rules, semantic replacement, the difference
//! between "we looked and nothing matched" and "nobody looked" — is **already discharged** by
//! `bioprism-policy`'s `redaction` module, which implements 43.33 and 39.19 and does it properly.
//! This module does not rebuild it and does not depend on it; it takes a set of redacted field
//! names as its input and answers the one question 04.05 asks that 43.33 does not:
//!
//! > "Redaction may make a cell non-replayable or invalidate some evaluators. The completeness map
//! > and result bundle state this; **missing evidence never becomes a pass**."
//!
//! # The sentence that shapes the module
//!
//! "Missing evidence never becomes a pass" is only enforceable if the outcome type has somewhere
//! else to go. [`ScoredOutcome`] therefore has three variants, and the two properties that make
//! the third real are:
//!
//! * [`ScoredOutcome::is_pass`] returns `false` for [`ScoredOutcome::Unevaluable`], and
//!   [`ScoredOutcome::is_fail`] also returns `false`. It is not a quiet failure either — an
//!   evaluator that could not run tells you nothing about the architecture, and scoring it as a
//!   failure would make redaction look like a capability regression.
//! * [`Tally`] counts it in a third bin, so a pass *rate* computed from a tally has an explicit
//!   denominator and [`Tally::rate`] returns `None` when nothing was evaluable rather than `0.0`.
//!
//! There is no `Default` for `ScoredOutcome` and no `From<Option<bool>>`. The conversions that
//! would let `Unevaluable` decay into a boolean are the ones deliberately absent.
//!
//! # Structure survives; content does not
//!
//! 04.05: "A redacted trace may retain event types, tool names, argument schemas, token counts,
//! timings, branch structure, and capability labels while removing content." [`StructuralField`]
//! enumerates those seven. They are what makes a redacted trace still worth something, so
//! [`RedactionPlan::retains`] answers whether a given evaluator's requirements survive.
//!
//! # What is not implemented
//!
//! No classifier, no detector, no crypto. Which fields are sensitive is an input. 04.05's
//! biomedical clause (PHI-bearing sources need an institutional policy, legal basis, access
//! control, audit, retention and de-identification validation) is a **programme**, not a predicate
//! over an artifact: nothing in this crate can check that an institutional policy exists. It is
//! named here and enforced nowhere, which is the honest arrangement.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// 04.05's eight policy actions, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    Drop,
    HashCommitment,
    Tokenize,
    RedactSpans,
    ExtractApprovedFeatures,
    Encrypt,
    RetainLocally,
    PublishAggregateOnly,
}

impl PolicyAction {
    pub const ALL: [PolicyAction; 8] = [
        PolicyAction::Drop,
        PolicyAction::HashCommitment,
        PolicyAction::Tokenize,
        PolicyAction::RedactSpans,
        PolicyAction::ExtractApprovedFeatures,
        PolicyAction::Encrypt,
        PolicyAction::RetainLocally,
        PolicyAction::PublishAggregateOnly,
    ];

    /// Whether the original bytes can still be recovered locally.
    ///
    /// `Tokenize` is 04.05's "reversible local tokenization" and `RetainLocally` and `Encrypt`
    /// keep the content; the rest do not. This is what decides replayability: a run cannot be
    /// replayed through a value that no longer exists anywhere, but it can be replayed through one
    /// that exists behind a local mapping.
    pub fn content_recoverable_locally(self) -> bool {
        matches!(
            self,
            PolicyAction::Tokenize | PolicyAction::Encrypt | PolicyAction::RetainLocally
        )
    }
}

/// The seven structural properties 04.05 says a redacted trace may keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralField {
    EventTypes,
    ToolNames,
    ArgumentSchemas,
    TokenCounts,
    Timings,
    BranchStructure,
    CapabilityLabels,
}

impl StructuralField {
    pub const ALL: [StructuralField; 7] = [
        StructuralField::EventTypes,
        StructuralField::ToolNames,
        StructuralField::ArgumentSchemas,
        StructuralField::TokenCounts,
        StructuralField::Timings,
        StructuralField::BranchStructure,
        StructuralField::CapabilityLabels,
    ];
}

/// One field and what was done to it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldAction {
    pub field: String,
    pub action: PolicyAction,
}

/// What a redaction did to a trace, expressed as fields and retained structure.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPlan {
    actions: Vec<FieldAction>,
    retained: BTreeSet<StructuralField>,
}

impl RedactionPlan {
    pub fn new() -> Self {
        RedactionPlan::default()
    }

    pub fn acting_on(
        mut self,
        field: impl Into<String>,
        action: PolicyAction,
    ) -> Result<Self, SweepError> {
        let field = field.into();
        require_nonempty(&field, "RedactionPlan", "field")?;
        self.actions.push(FieldAction { field, action });
        Ok(self)
    }

    pub fn retaining(mut self, field: StructuralField) -> Self {
        self.retained.insert(field);
        self
    }

    pub fn retains(&self, field: StructuralField) -> bool {
        self.retained.contains(&field)
    }

    pub fn action_for(&self, field: &str) -> Option<PolicyAction> {
        self.actions.iter().find(|a| a.field == field).map(|a| a.action)
    }

    /// Fields whose content is gone for good.
    pub fn irrecoverable(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter(|a| !a.action.content_recoverable_locally())
            .map(|a| a.field.as_str())
            .collect()
    }

    /// Whether a cell can still be replayed after this redaction.
    ///
    /// `replay_requires` is the set of fields replay reads. A field the plan made irrecoverable and
    /// replay needs makes the cell non-replayable, and the reason names the field — 04.05 asks the
    /// completeness map to state this, and a boolean states nothing.
    pub fn replayability(&self, replay_requires: &[&str]) -> Replayability {
        let irrecoverable: BTreeSet<&str> = self.irrecoverable().into_iter().collect();
        let blocking: Vec<String> = replay_requires
            .iter()
            .filter(|f| irrecoverable.contains(*f))
            .map(|f| f.to_string())
            .collect();
        if blocking.is_empty() {
            Replayability::Replayable
        } else {
            Replayability::NonReplayable { missing: blocking }
        }
    }
}

/// Whether a redacted cell can still be re-executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Replayability {
    Replayable,
    NonReplayable { missing: Vec<String> },
}

/// What an evaluator needs in order to have an opinion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorRequirements {
    pub evaluator: String,
    pub fields: Vec<String>,
    pub structure: Vec<StructuralField>,
}

impl EvaluatorRequirements {
    pub fn new(evaluator: impl Into<String>) -> Self {
        EvaluatorRequirements {
            evaluator: evaluator.into(),
            fields: Vec::new(),
            structure: Vec::new(),
        }
    }

    pub fn needing_field(mut self, field: impl Into<String>) -> Self {
        self.fields.push(field.into());
        self
    }

    pub fn needing_structure(mut self, field: StructuralField) -> Self {
        self.structure.push(field);
        self
    }
}

/// The result of running an evaluator against a redacted trace.
///
/// Three variants, no `Default`, and no conversion from `Option<bool>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ScoredOutcome {
    Pass,
    Fail,
    /// The evidence the evaluator needed was redacted away. Neither a pass nor a failure.
    Unevaluable { missing: Vec<String> },
}

impl ScoredOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, ScoredOutcome::Pass)
    }

    pub fn is_fail(&self) -> bool {
        matches!(self, ScoredOutcome::Fail)
    }

    /// Whether this outcome says anything about the architecture at all.
    pub fn is_evidence(&self) -> bool {
        !matches!(self, ScoredOutcome::Unevaluable { .. })
    }
}

/// Decide whether an evaluator can run at all, before it runs.
///
/// Returns `None` when the evaluator is admissible, and `Some(Unevaluable)` naming the missing
/// evidence when it is not. Callers that ignore the return value get no outcome, which is the
/// intended friction: there is no code path where a redacted requirement produces a `Pass`.
pub fn admissibility(
    plan: &RedactionPlan,
    requirements: &EvaluatorRequirements,
) -> Option<ScoredOutcome> {
    let irrecoverable: BTreeSet<&str> = plan.irrecoverable().into_iter().collect();
    let mut missing: Vec<String> = requirements
        .fields
        .iter()
        .filter(|f| irrecoverable.contains(f.as_str()))
        .cloned()
        .collect();
    for structure in &requirements.structure {
        if !plan.retains(*structure) {
            missing.push(format!("{structure:?}"));
        }
    }
    if missing.is_empty() {
        None
    } else {
        Some(ScoredOutcome::Unevaluable { missing })
    }
}

/// A count of outcomes with three bins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub passed: u32,
    pub failed: u32,
    pub unevaluable: u32,
}

impl Tally {
    pub fn add(&mut self, outcome: &ScoredOutcome) {
        match outcome {
            ScoredOutcome::Pass => self.passed += 1,
            ScoredOutcome::Fail => self.failed += 1,
            ScoredOutcome::Unevaluable { .. } => self.unevaluable += 1,
        }
    }

    /// Pass rate over the trials that produced evidence, or `None` when none did.
    ///
    /// `None` rather than `0.0`. A panel where every evaluator was blocked by redaction has a pass
    /// rate that does not exist, and zero is a specific, wrong, and unflattering claim about it.
    pub fn rate(&self) -> Option<f64> {
        let denominator = self.passed + self.failed;
        if denominator == 0 {
            None
        } else {
            Some(f64::from(self.passed) / f64::from(denominator))
        }
    }

    /// How many trials produced no evidence. Reported alongside any rate.
    pub fn unevaluable(&self) -> u32 {
        self.unevaluable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> RedactionPlan {
        RedactionPlan::new()
            .acting_on("messages.content", PolicyAction::Drop)
            .unwrap()
            .acting_on("tool.arguments", PolicyAction::Tokenize)
            .unwrap()
            .retaining(StructuralField::EventTypes)
            .retaining(StructuralField::ToolNames)
            .retaining(StructuralField::BranchStructure)
    }

    #[test]
    fn the_action_and_structure_lists_are_the_blueprints_eight_and_seven() {
        assert_eq!(PolicyAction::ALL.len(), 8);
        assert_eq!(StructuralField::ALL.len(), 7);
    }

    #[test]
    fn tokenized_content_stays_recoverable_and_dropped_content_does_not() {
        assert!(PolicyAction::Tokenize.content_recoverable_locally());
        assert!(!PolicyAction::Drop.content_recoverable_locally());
        assert!(!PolicyAction::HashCommitment.content_recoverable_locally());
        assert_eq!(plan().irrecoverable(), ["messages.content"]);
    }

    #[test]
    fn a_dropped_field_that_replay_needs_makes_the_cell_non_replayable_and_names_it() {
        match plan().replayability(&["messages.content", "tool.arguments"]) {
            Replayability::NonReplayable { missing } => {
                assert_eq!(missing, ["messages.content"]);
            }
            other => panic!("expected NonReplayable, got {other:?}"),
        }
    }

    #[test]
    fn tokenization_alone_leaves_a_cell_replayable() {
        assert_eq!(
            plan().replayability(&["tool.arguments"]),
            Replayability::Replayable
        );
    }

    #[test]
    fn an_evaluator_whose_evidence_was_dropped_is_unevaluable_not_a_pass() {
        let requirements =
            EvaluatorRequirements::new("content-grader").needing_field("messages.content");
        let outcome = admissibility(&plan(), &requirements).unwrap();
        assert!(!outcome.is_pass());
        assert!(!outcome.is_fail());
        assert!(!outcome.is_evidence());
        assert!(matches!(outcome, ScoredOutcome::Unevaluable { .. }));
    }

    #[test]
    fn an_evaluator_whose_evidence_survived_is_admissible() {
        let requirements = EvaluatorRequirements::new("tool-sequence")
            .needing_field("tool.arguments")
            .needing_structure(StructuralField::ToolNames);
        assert!(admissibility(&plan(), &requirements).is_none());
    }

    #[test]
    fn a_missing_structural_field_also_makes_an_evaluator_unevaluable() {
        let requirements =
            EvaluatorRequirements::new("latency").needing_structure(StructuralField::Timings);
        let outcome = admissibility(&plan(), &requirements).unwrap();
        match outcome {
            ScoredOutcome::Unevaluable { missing } => assert_eq!(missing, ["Timings"]),
            other => panic!("expected Unevaluable, got {other:?}"),
        }
    }

    #[test]
    fn unevaluable_trials_stay_out_of_both_halves_of_the_pass_rate() {
        let mut tally = Tally::default();
        tally.add(&ScoredOutcome::Pass);
        tally.add(&ScoredOutcome::Fail);
        tally.add(&ScoredOutcome::Unevaluable { missing: vec!["messages.content".into()] });
        assert_eq!(tally.rate(), Some(0.5));
        assert_eq!(tally.unevaluable(), 1);
    }

    #[test]
    fn a_panel_with_no_evaluable_trials_has_no_rate_rather_than_a_rate_of_zero() {
        let mut tally = Tally::default();
        tally.add(&ScoredOutcome::Unevaluable { missing: vec!["x".into()] });
        tally.add(&ScoredOutcome::Unevaluable { missing: vec!["y".into()] });
        assert_eq!(tally.rate(), None);
        assert_eq!(tally.unevaluable(), 2);
    }

    #[test]
    fn a_redaction_plan_field_name_cannot_be_empty() {
        assert!(RedactionPlan::new().acting_on("  ", PolicyAction::Drop).is_err());
    }

    #[test]
    fn an_outcome_round_trips_through_json_keeping_its_missing_list() {
        let outcome = ScoredOutcome::Unevaluable { missing: vec!["messages.content".into()] };
        let back: ScoredOutcome =
            serde_json::from_str(&serde_json::to_string(&outcome).unwrap()).unwrap();
        assert_eq!(back, outcome);
    }
}
