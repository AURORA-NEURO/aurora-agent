//! Failure localization and attribution.
//!
//! Blueprint 06.06. The output is a failure card: what broke, where the agent lost the thread, and
//! — the part that matters most — **whether it was the agent at all**. 06.06 is explicit that when
//! instructions, environment and verifier disagree, the compiler "labels benchmark defect and
//! routes to pack health rather than blaming the agent", and that is enforced here rather than left
//! to a reviewer's mood: a constraint the ledger records as *unsatisfiable* produces
//! [`Blame::TaskDefect`] before any other rule is considered.
//!
//! ## The evidence standard is a type
//!
//! 06.06: "Every label cites events, artifacts, state diffs, or counterfactual outcomes. Model-only
//! assertions without evidence remain hypotheses." So [`assert_with`] returns
//! [`Assertion::Hypothesis`] when the citation list is empty, and only [`Assertion::Evidenced`]
//! reaches [`FailureCard::findings`]. An uncited claim is not dropped — it is filed under
//! [`FailureCard::hypotheses`], where a reader can see the compiler suspected something and could
//! not show it.
//!
//! ## What is deliberately not implemented
//!
//! Constraints are recorded, not evaluated. 06.06's constraint ledger wants task requirements, tool
//! contracts and policies "translated into constraints" and evaluated after each state transition;
//! translating a natural-language instruction into a checkable constraint needs a model, and
//! evaluating one needs the environment. The caller supplies both the constraint and its outcome;
//! this module owns the consequences — the ordering of blame, and the refusal to blame the agent
//! for a contradiction between the task's own sources.
//!
//! No taxonomy of failure *modes* is defined. 06.06 asks for "multi-label taxonomy nodes" and names
//! no taxonomy; inventing one here would create a vocabulary the rest of the workspace has not
//! agreed to. [`AttributionLayer`] carries only the six layers 06.06 actually lists.

use crate::causal::{CausalAnalysis, CausalVerdict};
use serde::{Deserialize, Serialize};

/// A concrete, checkable pointer to why a claim is believed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cites", rename_all = "snake_case")]
pub enum Citation {
    Event { step: usize },
    Artifact { locator: String, sha256: String },
    StateDiff { description: String },
    CounterfactualOutcome { description: String },
}

/// A claim, and whether it is backed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Assertion {
    Evidenced {
        claim: String,
        citations: Vec<Citation>,
    },
    /// A claim nobody could back. Kept, labelled, and excluded from every conclusion.
    Hypothesis {
        claim: String,
        why: String,
    },
}

impl Assertion {
    pub fn claim(&self) -> &str {
        match self {
            Assertion::Evidenced { claim, .. } => claim,
            Assertion::Hypothesis { claim, .. } => claim,
        }
    }

    pub fn evidenced(&self) -> bool {
        matches!(self, Assertion::Evidenced { .. })
    }
}

/// Builds an assertion, downgrading it to a hypothesis when nothing backs it.
///
/// There is no constructor that produces an [`Assertion::Evidenced`] with an empty citation list.
pub fn assert_with(claim: impl Into<String>, citations: Vec<Citation>) -> Assertion {
    let claim = claim.into();
    if citations.is_empty() {
        return Assertion::Hypothesis {
            claim,
            why: "no event, artifact, state diff or counterfactual outcome was cited".to_string(),
        };
    }
    Assertion::Evidenced { claim, citations }
}

/// 06.06's attribution layers, from the symptom outward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionLayer {
    ObservedViolation,
    CriticalDecision,
    ContributingDecision,
    ArchitectureComponent,
    EnvironmentalFactor,
    EvaluatorWeakness,
}

/// Where a constraint came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintSource {
    TaskInstruction,
    ToolContract,
    Policy,
    OracleCondition,
}

/// What happened to a constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ConstraintOutcome {
    Satisfied,
    Violated { at_step: usize },
    /// Satisfying it would have violated another. The task, not the agent, is broken.
    Unsatisfiable { conflicts_with: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRecord {
    pub id: String,
    pub description: String,
    pub source: ConstraintSource,
    pub outcome: ConstraintOutcome,
}

/// Who or what the failure belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "blame", rename_all = "snake_case")]
pub enum Blame {
    Agent { at_step: usize },
    /// The environment produced a difference the agent did not control.
    Environment { at_step: usize },
    /// The task's own sources contradict each other. Routes to pack health, not to a scoreboard.
    TaskDefect { constraint: String, conflicts_with: String },
    /// The grader is the problem.
    Evaluator { dispute: String },
    /// Nothing cleared the bar. Better than a confident wrong answer.
    Undetermined { reason: String },
}

impl Blame {
    /// Whether this failure should count against an agent's score.
    pub fn counts_against_the_agent(&self) -> bool {
        matches!(self, Blame::Agent { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureCard {
    pub trace_id: String,
    pub terminal_step: usize,
    pub blame: Blame,
    /// Steps a cell could be extracted from, in the causal analysis's own order.
    pub recommended_cell_steps: Vec<usize>,
    /// Backed claims.
    pub findings: Vec<Assertion>,
    /// Unbacked claims, kept visible.
    pub hypotheses: Vec<Assertion>,
    pub violated_constraints: Vec<ConstraintRecord>,
    /// Explanations the evidence does not rule out. Named so a reader knows the card is not a
    /// proof.
    pub alternative_explanations: Vec<String>,
    /// What would have to be recorded for the card to be stronger.
    pub missing_evidence: Vec<String>,
}

impl FailureCard {
    /// Fraction of claims that are backed. Reported, never used to weight anything.
    pub fn evidence_ratio(&self) -> f64 {
        let total = self.findings.len() + self.hypotheses.len();
        if total == 0 {
            return 0.0;
        }
        self.findings.len() as f64 / total as f64
    }
}

/// Builds the failure card from a causal analysis and a constraint ledger.
///
/// The order of the blame rules is the point of the function, and it is the order 06.06 implies: a
/// contradiction inside the task outranks everything, because an agent cannot be wrong about a
/// question that has no consistent answer. An evaluator dispute comes next, then the causal
/// verdict, which is the only rule that can produce [`Blame::Agent`] — and only when the analysis
/// was willing to localize, which it refuses to do for environment-produced divergences.
pub fn failure_card(
    analysis: &CausalAnalysis,
    ledger: &[ConstraintRecord],
    evaluator_dispute: Option<&str>,
    claims: Vec<Assertion>,
) -> FailureCard {
    let unsatisfiable = ledger.iter().find_map(|record| match &record.outcome {
        ConstraintOutcome::Unsatisfiable { conflicts_with } => {
            Some((record.id.clone(), conflicts_with.clone()))
        }
        _ => None,
    });

    let blame = if let Some((constraint, conflicts_with)) = unsatisfiable {
        Blame::TaskDefect {
            constraint,
            conflicts_with,
        }
    } else if let Some(dispute) = evaluator_dispute {
        Blame::Evaluator {
            dispute: dispute.to_string(),
        }
    } else {
        match &analysis.verdict {
            CausalVerdict::FirstCausal { step, .. } => Blame::Agent { at_step: *step },
            CausalVerdict::Conjunction { steps } => Blame::Agent {
                at_step: steps.first().copied().unwrap_or(analysis.terminal_step),
            },
            CausalVerdict::EnvironmentDivergence { at_step, .. } => {
                Blame::Environment { at_step: *at_step }
            }
            CausalVerdict::NoDivergence => Blame::Undetermined {
                reason: "the runs never differed on anything the trace records".to_string(),
            },
            CausalVerdict::Unlocalizable { reason } => Blame::Undetermined {
                reason: reason.clone(),
            },
        }
    };

    let (findings, hypotheses): (Vec<Assertion>, Vec<Assertion>) =
        claims.into_iter().partition(|claim| claim.evidenced());

    let violated: Vec<ConstraintRecord> = ledger
        .iter()
        .filter(|record| !matches!(record.outcome, ConstraintOutcome::Satisfied))
        .cloned()
        .collect();

    let mut alternatives = Vec::new();
    let mut missing = Vec::new();
    if analysis.reference.is_none() {
        alternatives.push(
            "with no reference trajectory, any of the ranked candidates could be the cause"
                .to_string(),
        );
        missing.push("a passing or peer run to compare against".to_string());
    }
    if analysis.candidates.iter().any(|candidate| {
        !candidate.score.irreversibility_declared && candidate.score.irreversibility > 0.0
    }) {
        missing.push(
            "an `irreversible` flag on the actions; reversibility is currently this crate's default"
                .to_string(),
        );
    }
    if matches!(blame, Blame::Environment { .. }) {
        alternatives.push(
            "an earlier agent decision could have led the environment to answer differently, but \
             no such decision appears as a textual difference"
                .to_string(),
        );
    }

    FailureCard {
        trace_id: analysis.trace_id.clone(),
        terminal_step: analysis.terminal_step,
        recommended_cell_steps: if blame.counts_against_the_agent() {
            analysis
                .candidates
                .iter()
                .filter(|candidate| candidate.upstream_unresolved.is_none())
                .map(|candidate| candidate.step)
                .collect()
        } else {
            Vec::new()
        },
        blame,
        findings,
        hypotheses,
        violated_constraints: violated,
        alternative_explanations: alternatives,
        missing_evidence: missing,
    }
}
