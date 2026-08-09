//! Oracle synthesis and review.
//!
//! Blueprint 06.08. This is the module where a mistake is most expensive, because a bad oracle does
//! not fail — it *grades*, and everything downstream inherits its judgement as if it were evidence.
//! So the invariant is structural rather than procedural: a synthesized oracle and a reviewed
//! oracle are **different types**, and only [`ReviewedOracle`] has a `grade` method.
//!
//! [`ProposedOracle`] deliberately has no way to score anything. Not a private one, not a
//! `#[doc(hidden)]` one — there is no code path from a proposal to a verdict that does not pass
//! through [`ProposedOracle::review`], which is the same shape as
//! `bioprism_trace::CellProposal::approve` and for the same reason (Gate 2: "humans approve cells;
//! the system does not silently publish model-generated tests"). [`ReviewedOracle`] has private
//! fields and implements `Serialize` but **not** `Deserialize`, so a reviewed oracle cannot be
//! conjured by parsing JSON that claims to be one.
//!
//! Review is not a rubber stamp. It refuses, with a typed error, when:
//!
//! - the reviewer is unnamed — an unattributed approval is indistinguishable from none;
//! - the proposal declares no blind spots, because 06.08 makes gap analysis mandatory and an
//!   oracle whose author could not name one thing it cannot see has not been examined;
//! - a recorded exploit scored as a pass without fulfilling task intent. 06.08: "Successful attacks
//!   block publication." Review cannot clear this one, which is why it is checked at the gate
//!   rather than left to the reviewer's discretion;
//! - the oracle is a model judge or a statistical tolerance with no deterministic companion. 35.08
//!   quality gate 3 wants a non-LLM oracle covering the primary defect wherever one is feasible,
//!   and a weak oracle standing alone is how a scoring bug becomes a published result.
//!
//! ## What is deliberately not implemented
//!
//! Oracles here are **declarative contracts**, not executable code: a proposal names the verdicts it
//! accepts and the witnesses it requires, and grading compares an observed outcome against them.
//! 06.08's "execution test" rung of the synthesis ladder therefore cannot be *run* by this crate —
//! running untrusted checker code needs the sandbox 06.08's invariants require and this workspace
//! has no sandbox. [`OracleStrength::ExecutionTest`] can be declared and reviewed; the execution is
//! the caller's, and the strength label records what kind of evidence backs the contract.
//!
//! Adversarial *generation* is likewise not here. This module records attack attempts and enforces
//! their outcome; it does not invent them. Automatically generating attacks that a proposal itself
//! evaluated would let the oracle mark its own homework, which is the failure mode the gate exists
//! to prevent.

use crate::error::OracleError;
use crate::minimize::Minimization;
use bioprism_ids::ContentHash;
use bioprism_prism::{Acceptance, DecisionCell, InputRef};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

/// 06.08's synthesis order, strongest first.
///
/// The order is the blueprint's: "Exact/state predicate → execution test → property relation →
/// trajectory constraint → statistical tolerance → calibrated model judge → human review." Human
/// review is not a rung here because it is the gate every rung passes through, not an alternative
/// to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleStrength {
    ExactStatePredicate,
    ExecutionTest,
    PropertyRelation,
    TrajectoryConstraint,
    StatisticalTolerance,
    ModelJudge,
}

/// The ladder in the order a synthesizer should try it.
pub const SYNTHESIS_ORDER: [OracleStrength; 6] = [
    OracleStrength::ExactStatePredicate,
    OracleStrength::ExecutionTest,
    OracleStrength::PropertyRelation,
    OracleStrength::TrajectoryConstraint,
    OracleStrength::StatisticalTolerance,
    OracleStrength::ModelJudge,
];

impl OracleStrength {
    pub fn as_str(self) -> &'static str {
        match self {
            OracleStrength::ExactStatePredicate => "exact_state_predicate",
            OracleStrength::ExecutionTest => "execution_test",
            OracleStrength::PropertyRelation => "property_relation",
            OracleStrength::TrajectoryConstraint => "trajectory_constraint",
            OracleStrength::StatisticalTolerance => "statistical_tolerance",
            OracleStrength::ModelJudge => "model_judge",
        }
    }

    /// Whether the same input always produces the same grade.
    pub fn deterministic(self) -> bool {
        !matches!(
            self,
            OracleStrength::StatisticalTolerance | OracleStrength::ModelJudge
        )
    }

    /// Whether this rung may be the only thing grading a cell.
    ///
    /// The two non-deterministic rungs may not. A nondeterministic judgement that silently
    /// overrides execution-grounded evidence is exactly what 06.05's invariant list forbids, and a
    /// weak oracle with no deterministic companion has nothing to be overridden *by*.
    pub fn may_stand_alone(self) -> bool {
        self.deterministic()
    }
}

/// One attempt to satisfy the score without doing the task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploitAttempt {
    pub name: String,
    /// What the attack did: hard-coded the output, read the grader, exploited metadata.
    pub description: String,
    /// Whether the proposed oracle would have called it a pass.
    pub scored_as_pass: bool,
    /// Whether it actually did what the task asked. Recorded separately because the whole point is
    /// that these two can disagree.
    pub fulfils_task_intent: bool,
}

impl ExploitAttempt {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        ExploitAttempt {
            name: name.into(),
            description: description.into(),
            scored_as_pass: false,
            fulfils_task_intent: false,
        }
    }

    /// Records that the oracle accepted this attack.
    pub fn scored_as_pass(mut self) -> Self {
        self.scored_as_pass = true;
        self
    }

    /// Records that the attack, despite its shape, did the task properly after all.
    pub fn fulfils_task_intent(mut self) -> Self {
        self.fulfils_task_intent = true;
        self
    }

    /// A successful attack: graded as a pass without doing the task.
    pub fn succeeded(&self) -> bool {
        self.scored_as_pass && !self.fulfils_task_intent
    }
}

/// A synthesized oracle that has not been reviewed.
///
/// It cannot grade. That is the entire design: this type has no method that produces an
/// [`Acceptance`], so a caller holding one cannot accidentally use it to score a candidate however
/// convenient that would be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedOracle {
    pub oracle_id: String,
    /// The decision this oracle grades, in one line.
    pub decision_point: String,
    pub strength: OracleStrength,
    /// Verdicts that count as passing. Set-valued, matching `bioprism_prism`'s cell contract: an
    /// oracle that names one right answer scores two equally correct continuations differently.
    pub acceptable_verdicts: BTreeSet<String>,
    pub required_witnesses: BTreeSet<String>,
    /// What this oracle can observe.
    pub can_see: Vec<String>,
    /// What it cannot. 06.08 makes this mandatory; review refuses an empty list.
    pub blind_spots: Vec<String>,
    /// Attacks run against it, successful or not.
    pub exploits: Vec<ExploitAttempt>,
    /// A deterministic companion oracle, required when this one is too weak to stand alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired_with: Option<String>,
}

impl ProposedOracle {
    pub fn new(
        oracle_id: impl Into<String>,
        decision_point: impl Into<String>,
        strength: OracleStrength,
    ) -> Self {
        ProposedOracle {
            oracle_id: oracle_id.into(),
            decision_point: decision_point.into(),
            strength,
            acceptable_verdicts: BTreeSet::new(),
            required_witnesses: BTreeSet::new(),
            can_see: Vec::new(),
            blind_spots: Vec::new(),
            exploits: Vec::new(),
            paired_with: None,
        }
    }

    pub fn accepting(mut self, verdict: impl Into<String>) -> Self {
        self.acceptable_verdicts.insert(verdict.into());
        self
    }

    pub fn requiring_witness(mut self, witness: impl Into<String>) -> Self {
        self.required_witnesses.insert(witness.into());
        self
    }

    pub fn seeing(mut self, what: impl Into<String>) -> Self {
        self.can_see.push(what.into());
        self
    }

    pub fn blind_to(mut self, what: impl Into<String>) -> Self {
        self.blind_spots.push(what.into());
        self
    }

    pub fn attacked_with(mut self, attempt: ExploitAttempt) -> Self {
        self.exploits.push(attempt);
        self
    }

    pub fn paired_with(mut self, oracle_id: impl Into<String>) -> Self {
        self.paired_with = Some(oracle_id.into());
        self
    }

    /// Attacks that scored as a pass without doing the task. Non-empty blocks publication.
    pub fn successful_exploits(&self) -> Vec<&ExploitAttempt> {
        self.exploits
            .iter()
            .filter(|attempt| attempt.succeeded())
            .collect()
    }

    /// The only path from a proposal to something that can grade.
    ///
    /// Returns [`ReviewedOracle`] by value; there is no `From`, no `Default`, and no deserializer
    /// that produces one.
    pub fn review(self, reviewer: &str) -> Result<ReviewedOracle, OracleError> {
        if reviewer.trim().is_empty() {
            return Err(OracleError::UnattributedReview);
        }
        if self.acceptable_verdicts.is_empty() {
            return Err(OracleError::EmptyAcceptanceSet {
                oracle: self.oracle_id,
            });
        }
        if self.blind_spots.is_empty() {
            return Err(OracleError::NoGapAnalysis {
                oracle: self.oracle_id,
            });
        }
        if let Some(attack) = self.exploits.iter().find(|attempt| attempt.succeeded()) {
            return Err(OracleError::UnrebuttedExploit {
                attack: attack.name.clone(),
            });
        }
        if !self.strength.may_stand_alone() && self.paired_with.is_none() {
            return Err(OracleError::WeakOracleAlone {
                oracle: self.oracle_id,
                strength: self.strength.as_str(),
            });
        }

        let record = json!({ "oracle": self, "reviewer": reviewer });
        let review_digest = ContentHash::of_value(&record)
            .map_err(|_| OracleError::UnattributedReview)?
            .as_str()
            .to_string();

        Ok(ReviewedOracle {
            inner: self,
            reviewer: reviewer.to_string(),
            review_digest,
        })
    }
}

/// A reviewed oracle. The only kind that can grade anything.
///
/// `Serialize` but not `Deserialize`: a reviewed oracle can be published and audited, and cannot be
/// reconstituted from bytes that merely claim review happened. Round-tripping one through JSON
/// yields a [`ProposedOracle`] plus a reviewer name, which must go through the gate again — that is
/// the intended cost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewedOracle {
    inner: ProposedOracle,
    reviewer: String,
    review_digest: String,
}

impl ReviewedOracle {
    pub fn proposal(&self) -> &ProposedOracle {
        &self.inner
    }

    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    /// Hash over the proposal and the reviewer's name. Changing either changes this.
    pub fn review_digest(&self) -> &str {
        &self.review_digest
    }

    /// Grades an observed outcome.
    ///
    /// Comparison is over the oracle's own verdict vocabulary rather than
    /// `bioprism_section::OracleStatus`, because a synthesized oracle may accept verdicts the
    /// section-level enum does not name. `bioprism_prism::Acceptance` is reused unchanged so a
    /// reader sees the same four outcomes here as everywhere else, including the one that most
    /// looks like a pass: a right answer reached from an incomplete basis.
    pub fn grade(
        &self,
        verdict: &str,
        witnesses: &BTreeSet<String>,
        closure_complete: bool,
    ) -> Acceptance {
        if !self.inner.acceptable_verdicts.contains(verdict) {
            return Acceptance::WrongVerdict {
                observed: verdict.to_string(),
            };
        }
        let missing: Vec<String> = self
            .inner
            .required_witnesses
            .difference(witnesses)
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Acceptance::MissingWitnesses(missing);
        }
        if !closure_complete {
            return Acceptance::ClosureIncomplete;
        }
        Acceptance::Passed
    }

    /// Freezes the reviewed contract into a `bioprism_prism` cell.
    ///
    /// The cell type is not redefined here. This crate produces cells; `bioprism_prism` owns what
    /// one is, and its `acceptable_verdicts` / `required_witnesses` fields are populated from the
    /// reviewed contract rather than from anything this crate invents at packaging time.
    pub fn into_cell(self, cell_id: impl Into<String>, world: InputRef, query: InputRef) -> DecisionCell {
        let mut cell = DecisionCell::new(cell_id, self.inner.decision_point.clone(), world, query);
        cell.acceptable_verdicts = self.inner.acceptable_verdicts;
        cell.required_witnesses = self.inner.required_witnesses;
        cell
    }
}

/// Synthesizes a proposal from what minimization proved about the reduced context.
///
/// This is 06.08's "synthesis order" applied to the evidence this crate actually has. A preserved
/// signature carrying witnesses can be checked by an exact state predicate, the strongest rung; one
/// carrying only a verdict cannot, and drops to a trajectory constraint, which is weaker and says
/// so rather than claiming the top rung by default.
///
/// The gap analysis is derived rather than invented: everything minimization removed is, by
/// construction, invisible to an oracle grading the reduced context. That is a real blind spot and
/// stating it is the difference between a reduction and a reduction whose consequences are known.
pub fn synthesise(
    oracle_id: impl Into<String>,
    decision_point: impl Into<String>,
    minimization: &Minimization,
) -> ProposedOracle {
    let strength = if minimization.preserved.witnesses.is_empty() {
        OracleStrength::TrajectoryConstraint
    } else {
        OracleStrength::ExactStatePredicate
    };

    let mut proposal = ProposedOracle::new(oracle_id, decision_point, strength)
        .accepting(minimization.preserved.verdict.clone())
        .seeing(format!(
            "{} context item(s) retained by minimization",
            minimization.minimal.len()
        ))
        .blind_to(format!(
            "{} context item(s) removed during minimization are outside what this oracle can check",
            minimization.removed.len()
        ));

    for witness in &minimization.preserved.witnesses {
        proposal = proposal.requiring_witness(witness.clone());
    }
    if !minimization.pinned.is_empty() {
        proposal = proposal.blind_to(format!(
            "{} item(s) are retained by the task-intent guard rather than by observed effect; \
             this oracle cannot tell whether they still matter",
            minimization.pinned.len()
        ));
    }
    if minimization.preserved.witnesses.is_empty() {
        proposal = proposal.blind_to(
            "the preserved signature carries no witnesses, so this oracle checks the verdict but \
             not the reason for it"
                .to_string(),
        );
    }
    proposal
}
