//! Counterfactual cell generation.
//!
//! Blueprint 06.09. A counterfactual pair changes exactly one thing and states how the correct
//! decision should respond. Its value comes entirely from the *matched design*: hold everything
//! else constant, or the pair measures the interaction of two changes and attributes it to one.
//!
//! So matching is checked, not assumed. An [`Intervention`] declares which fields of the cell it
//! touches; [`pair`] compares the source and follow-up cells field by field and returns
//! [`CounterfactualError::UnmatchedPair`] naming every field that moved without being declared.
//! This is the same discipline `bioprism_prism::matched_fork` applies to architectures — a
//! difference is attributable only when one thing was free — pushed back to cell construction.
//!
//! Cells are `bioprism_prism::DecisionCell`. This module does not define a variant cell type or
//! wrap one; it produces pairs of the workspace's cell and records what separates them.
//!
//! ## What is deliberately not implemented
//!
//! Realism checking is delegated. 06.09 requires interventions to produce "reachable, coherent
//! states" and assigns the judgement to environment validators and domain experts; this crate has
//! no environment and no domain model, so [`pair`] takes a [`RealismCheck`] from the caller and
//! turns its refusal into [`CounterfactualError::IncoherentState`]. Passing a check that always
//! succeeds is allowed and is exactly as meaningful as it sounds.
//!
//! No intervention is *applied*. Constructing the follow-up world is the caller's job — often
//! `bioprism_mutation`'s, whose controlled semantic mutations (06.12) are the same operation with a
//! metamorphic relation attached. This module validates the pair and records the contrast.

use crate::error::CounterfactualError;
use bioprism_prism::DecisionCell;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// 06.09's intervention targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterventionTarget {
    EvidenceAvailability,
    EvidenceFreshness,
    ToolCapability,
    Permission,
    Budget,
    MemoryContent,
    EnvironmentState,
    Timing,
    UserIntent,
    ArchitectureNode,
    VerifierFeedback,
}

impl InterventionTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            InterventionTarget::EvidenceAvailability => "evidence_availability",
            InterventionTarget::EvidenceFreshness => "evidence_freshness",
            InterventionTarget::ToolCapability => "tool_capability",
            InterventionTarget::Permission => "permission",
            InterventionTarget::Budget => "budget",
            InterventionTarget::MemoryContent => "memory_content",
            InterventionTarget::EnvironmentState => "environment_state",
            InterventionTarget::Timing => "timing",
            InterventionTarget::UserIntent => "user_intent",
            InterventionTarget::ArchitectureNode => "architecture_node",
            InterventionTarget::VerifierFeedback => "verifier_feedback",
        }
    }
}

/// Cell fields an intervention is permitted to change.
///
/// Named as strings matching `DecisionCell`'s own field names so a mismatch report reads directly
/// against the struct a reviewer is looking at.
pub const CELL_FIELDS: [&str; 5] = [
    "world",
    "query",
    "acceptable_verdicts",
    "required_witnesses",
    "require_protected_closure",
];

/// The one thing that changes between the two cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intervention {
    /// The factor in the caller's own vocabulary, for the report.
    pub factor: String,
    pub target: InterventionTarget,
    pub from: Value,
    pub to: Value,
    /// Which cell fields this intervention is allowed to move. Everything else must match.
    pub changes: BTreeSet<String>,
}

impl Intervention {
    pub fn new(
        factor: impl Into<String>,
        target: InterventionTarget,
        from: Value,
        to: Value,
    ) -> Self {
        Intervention {
            factor: factor.into(),
            target,
            from,
            to,
            changes: BTreeSet::new(),
        }
    }

    /// Declares a cell field the intervention moves.
    pub fn changing(mut self, field: impl Into<String>) -> Self {
        self.changes.insert(field.into());
        self
    }
}

/// How the correct decision should respond to the intervention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expect", rename_all = "snake_case")]
pub enum ExpectedResponse {
    /// The right answer does not move. A candidate whose answer does move is sensitive to
    /// something irrelevant.
    Invariant { rationale: String },
    /// The right answer must move, and to one of these verdicts.
    MustChange {
        to_verdicts: BTreeSet<String>,
        rationale: String,
    },
}

impl ExpectedResponse {
    /// Whether an observed verdict on the follow-up cell is what the pair predicted.
    pub fn satisfied_by(&self, source_verdict: &str, followup_verdict: &str) -> bool {
        match self {
            ExpectedResponse::Invariant { .. } => source_verdict == followup_verdict,
            ExpectedResponse::MustChange { to_verdicts, .. } => {
                followup_verdict != source_verdict && to_verdicts.contains(followup_verdict)
            }
        }
    }
}

/// A caller-supplied judgement about whether an intervened state can exist.
pub trait RealismCheck {
    fn coherent(&mut self, intervention: &Intervention) -> Result<(), String>;
}

impl<F> RealismCheck for F
where
    F: FnMut(&Intervention) -> Result<(), String>,
{
    fn coherent(&mut self, intervention: &Intervention) -> Result<(), String> {
        self(intervention)
    }
}

/// A realism check that accepts everything.
///
/// Provided so the permissive choice has to be written down. A pair validated with this has had no
/// realism review, and the pair's `realism_reviewed` flag records that.
pub struct NoRealismReview;

impl RealismCheck for NoRealismReview {
    fn coherent(&mut self, _intervention: &Intervention) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualPair {
    pub source: DecisionCell,
    pub followup: DecisionCell,
    pub intervention: Intervention,
    pub expected: ExpectedResponse,
    /// Fields that actually differ. Always a subset of `intervention.changes`, or the pair would
    /// not have been built.
    pub differing_fields: Vec<String>,
    /// Whether a realism check other than [`NoRealismReview`] passed on this pair.
    pub realism_reviewed: bool,
}

fn differing_fields(source: &DecisionCell, followup: &DecisionCell) -> Vec<String> {
    let mut differ = Vec::new();
    if source.world != followup.world {
        differ.push("world".to_string());
    }
    if source.query != followup.query {
        differ.push("query".to_string());
    }
    if source.acceptable_verdicts != followup.acceptable_verdicts {
        differ.push("acceptable_verdicts".to_string());
    }
    if source.required_witnesses != followup.required_witnesses {
        differ.push("required_witnesses".to_string());
    }
    if source.require_protected_closure != followup.require_protected_closure {
        differ.push("require_protected_closure".to_string());
    }
    differ
}

/// Validates a matched pair and records the contrast.
///
/// Refuses, in order: colliding ids, a null intervention, an incoherent state, and finally any
/// field that moved without being declared. The order matters — an unmatched pair whose
/// intervention was also null should report the null intervention, because that is the defect a
/// generator can actually fix.
pub fn pair<R: RealismCheck>(
    source: DecisionCell,
    followup: DecisionCell,
    intervention: Intervention,
    expected: ExpectedResponse,
    realism: &mut R,
    realism_reviewed: bool,
) -> Result<CounterfactualPair, CounterfactualError> {
    if source.cell_id == followup.cell_id {
        return Err(CounterfactualError::CollidingCellIds {
            cell_id: source.cell_id,
        });
    }
    if intervention.from == intervention.to {
        return Err(CounterfactualError::NullIntervention {
            factor: intervention.factor,
        });
    }

    let differ = differing_fields(&source, &followup);
    if differ.is_empty() {
        return Err(CounterfactualError::NullIntervention {
            factor: intervention.factor,
        });
    }

    if let Err(reason) = realism.coherent(&intervention) {
        return Err(CounterfactualError::IncoherentState {
            factor: intervention.factor,
            reason,
        });
    }

    let undeclared: Vec<String> = differ
        .iter()
        .filter(|field| !intervention.changes.contains(*field))
        .cloned()
        .collect();
    if !undeclared.is_empty() {
        return Err(CounterfactualError::UnmatchedPair { fields: undeclared });
    }

    Ok(CounterfactualPair {
        source,
        followup,
        intervention,
        expected,
        differing_fields: differ,
        realism_reviewed,
    })
}

/// What a candidate's two answers say about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ContrastOutcome {
    /// The candidate responded as the pair predicted.
    AsPredicted,
    /// The pair expected no change and the candidate changed its answer: it is sensitive to
    /// something the intervention says is irrelevant.
    SpuriousSensitivity { moved_to: String },
    /// The pair expected a change and the candidate did not move: it is insensitive to something
    /// that matters.
    MissedTheChange { stayed_at: String },
    /// It moved, but not to a verdict the pair accepts.
    WrongDirection { moved_to: String },
}

/// Grades a candidate's pair of answers against the expected response.
pub fn contrast(
    pair: &CounterfactualPair,
    source_verdict: &str,
    followup_verdict: &str,
) -> ContrastOutcome {
    if pair
        .expected
        .satisfied_by(source_verdict, followup_verdict)
    {
        return ContrastOutcome::AsPredicted;
    }
    match &pair.expected {
        ExpectedResponse::Invariant { .. } => ContrastOutcome::SpuriousSensitivity {
            moved_to: followup_verdict.to_string(),
        },
        ExpectedResponse::MustChange { .. } if followup_verdict == source_verdict => {
            ContrastOutcome::MissedTheChange {
                stayed_at: followup_verdict.to_string(),
            }
        }
        ExpectedResponse::MustChange { .. } => ContrastOutcome::WrongDirection {
            moved_to: followup_verdict.to_string(),
        },
    }
}
