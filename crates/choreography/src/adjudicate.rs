//! Challenge, dispute and adjudication.
//!
//! Blueprint 23.18. The weave kernel records a claim and a challenge against it and then stops,
//! on purpose: a trusted computing base that decided which of two participants was right would
//! have to know some science. So the dispute arrives here as two positions and a pile of evidence,
//! and something has to settle it.
//!
//! # The procedure is the object
//!
//! [`Procedure`] is data — an ordered list of named [`Rule`]s, each a declared condition drawn
//! from a closed enum. Adjudication evaluates them in order and the first match fires, so a
//! [`Ruling::Resolved`] always carries *which rule fired* and why. This is the difference between
//! a verdict and an opinion: a reader who disagrees with the outcome can point at the rule rather
//! than at the adjudicator. Changing how disputes are settled means writing a different
//! `Procedure`, which is a reviewable artifact, rather than editing a function.
//!
//! # What happens when the evidence does not settle it
//!
//! [`Ruling::Unresolved`], carrying the [`EvidenceRequirement`]s nobody satisfied and both
//! positions intact. There is deliberately no rule in any procedure here that breaks a tie by
//! authority, seniority, participant count, recency or fluency, and there is no randomness in the
//! crate at all, so a coin flip is not available even by accident. 23.18 asks for preserved
//! dissent and 23.19 says votes are not evidence; between them they rule out every tiebreak that
//! does not name a fact. "The available evidence does not separate these positions, and here is
//! what would" is a useful answer. A confident wrong one is not.
//!
//! Two deterministic tests that contradict each other are also not settled by this crate: the
//! shipped procedures escalate, because a disagreement between two reproducible results is a
//! defect in the question, and no decision rule over the same evidence can repair it.
//!
//! # Deliberately not implemented
//!
//! No evidence acquisition, no reproduction runner, no specialist binding and no human in the
//! loop — [`RuleOutcome::Escalate`] names a destination and stops, because this crate has no
//! transport. Challenge lifecycle timers and deadlines are absent along with the clock. The
//! anti-convergence measures of 23.18 (sealed initial judgments, randomised evidence order,
//! independent context projections) are properties of how positions are *gathered*, not of how
//! they are adjudicated; the independence half of that concern is checked in [`crate::quorum`].

use bioprism_ids::ContentHash;
use bioprism_weave::{ActKind, Ledger};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::session::Role;

/// The taxonomy of 23.18. Recorded because the right procedure depends on it: a factual dispute
/// is settled by evidence and a policy dispute is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisputeKind {
    /// Disagreement about world state.
    Factual,
    /// Disagreement about meaning or schema.
    Interpretive,
    /// Disagreement about procedure or validity.
    Methodological,
    /// Disagreement about whether evidence transfers.
    Applicability,
    /// Disagreement about allowed action.
    Policy,
    /// Disagreement about decision rights.
    Authority,
    /// Disagreement about budget or priority.
    Resource,
    /// Disagreement about choreography state.
    Protocol,
}

impl DisputeKind {
    /// Whether evidence about the world can, in principle, decide this kind of dispute.
    ///
    /// False for policy, authority and resource disputes: those are normative, and 23.19's rule
    /// that votes are not evidence has a converse — evidence is not a decision right.
    pub fn settleable_by_evidence(self) -> bool {
        matches!(
            self,
            DisputeKind::Factual
                | DisputeKind::Interpretive
                | DisputeKind::Methodological
                | DisputeKind::Applicability
                | DisputeKind::Protocol
        )
    }
}

/// Which of the two parties. A dispute here has exactly two sides; an n-way disagreement is
/// decomposed by the caller, because "who is opposing whom" is not something to infer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The party whose claim is under challenge.
    Claimant,
    /// The party challenging it.
    Challenger,
}

impl Side {
    pub fn opposite(self) -> Side {
        match self {
            Side::Claimant => Side::Challenger,
            Side::Challenger => Side::Claimant,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Claimant => "claimant",
            Side::Challenger => "challenger",
        }
    }
}

/// How much weight a piece of evidence can bear.
///
/// Declared weakest-first so the derived ordering runs the right way: a deterministic test
/// outranks an opinion, and 23.19's "deterministic and higher-assurance evidence dominates
/// lower-assurance opinions" becomes a comparison rather than a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Assurance {
    /// An assertion with no independent check behind it.
    Opinion,
    /// An observation whose method is stated but not independently reproduced.
    Measurement,
    /// A citation to a document that says so.
    SourceDocument,
    /// An independent rerun that agreed.
    Reproduction,
    /// A test that either passes or fails and does the same thing every time.
    DeterministicTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    EventLog,
    TestResult,
    Document,
    Measurement,
    Statement,
}

/// One submitted item, tied to the side it supports and the requirements it discharges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub kind: EvidenceKind,
    pub assurance: Assurance,
    pub supports: Side,
    pub summary: String,
    /// Ids of the [`EvidenceRequirement`]s this item satisfies. Stated by the submitter and not
    /// inferred: deciding whether a document answers a question is exactly the kind of judgement
    /// this crate must not make silently.
    #[serde(default)]
    pub satisfies: BTreeSet<String>,
}

impl Evidence {
    pub fn new(
        id: impl Into<String>,
        kind: EvidenceKind,
        assurance: Assurance,
        supports: Side,
        summary: impl Into<String>,
    ) -> Self {
        Evidence {
            id: id.into(),
            kind,
            assurance,
            supports,
            summary: summary.into(),
            satisfies: BTreeSet::new(),
        }
    }

    pub fn satisfying(mut self, requirement: impl Into<String>) -> Self {
        self.satisfies.insert(requirement.into());
        self
    }
}

/// Evidence the dispute needs and does not have.
///
/// The `acquisition` field is 23.18's `requested_resolution`: naming the missing evidence is only
/// half an answer if nobody can tell how to go and get it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequirement {
    pub id: String,
    pub description: String,
    /// What someone would do to obtain it: "inspect event-log", "run concurrency-test".
    pub acquisition: String,
}

impl EvidenceRequirement {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        acquisition: impl Into<String>,
    ) -> Self {
        EvidenceRequirement {
            id: id.into(),
            description: description.into(),
            acquisition: acquisition.into(),
        }
    }
}

/// One party's stance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub holder: Role,
    pub claim: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    /// Set when this party has withdrawn. 23.18 asks participants to "concede when contradicted",
    /// so conceding is a first-class move rather than silence.
    #[serde(default)]
    pub conceded: bool,
    /// Whether this party's objection blocks the transition under dispute.
    #[serde(default)]
    pub blocking: bool,
}

impl Position {
    pub fn new(holder: impl Into<Role>, claim: impl Into<String>) -> Self {
        Position {
            holder: holder.into(),
            claim: claim.into(),
            evidence: Vec::new(),
            conceded: false,
            blocking: false,
        }
    }

    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn conceding(mut self) -> Self {
        self.conceded = true;
        self
    }

    pub fn blocking(mut self) -> Self {
        self.blocking = true;
        self
    }

    fn strongest_for(&self, side: Side) -> Option<Assurance> {
        self.evidence
            .iter()
            .filter(|item| item.supports == side)
            .map(|item| item.assurance)
            .max()
    }
}

/// A two-party disagreement, with the burden of proof declared before anyone looks at the
/// evidence.
///
/// 23.18 makes the choreography declare who bears the burden. Recording it on the dispute rather
/// than deriving it means the burden cannot quietly shift to whoever is losing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dispute {
    pub id: String,
    pub kind: DisputeKind,
    pub claimant: Position,
    pub challenger: Position,
    /// The side that must produce support. Usually the claimant, but 23.18 puts it on the actor
    /// for an irreversible action and on the verifier for a rejection.
    pub burden: Side,
    #[serde(default)]
    pub required_evidence: Vec<EvidenceRequirement>,
}

impl Dispute {
    pub fn new(
        id: impl Into<String>,
        kind: DisputeKind,
        claimant: Position,
        challenger: Position,
        burden: Side,
    ) -> Self {
        Dispute {
            id: id.into(),
            kind,
            claimant,
            challenger,
            burden,
            required_evidence: Vec::new(),
        }
    }

    pub fn requiring(mut self, requirement: EvidenceRequirement) -> Self {
        self.required_evidence.push(requirement);
        self
    }

    pub fn position(&self, side: Side) -> &Position {
        match side {
            Side::Claimant => &self.claimant,
            Side::Challenger => &self.challenger,
        }
    }

    /// The strongest assurance any submitted item offers in favour of `side`.
    ///
    /// Both positions are searched: evidence supporting the challenger may perfectly well have
    /// been submitted by the claimant, and a procedure that only counted a party's own filings
    /// would reward withholding.
    pub fn strongest_support(&self, side: Side) -> Option<Assurance> {
        [
            self.claimant.strongest_for(side),
            self.challenger.strongest_for(side),
        ]
        .into_iter()
        .flatten()
        .max()
    }

    /// Requirements no submitted item claims to satisfy.
    pub fn missing_evidence(&self) -> Vec<EvidenceRequirement> {
        let satisfied: BTreeSet<&String> = self
            .claimant
            .evidence
            .iter()
            .chain(self.challenger.evidence.iter())
            .flat_map(|item| item.satisfies.iter())
            .collect();
        self.required_evidence
            .iter()
            .filter(|requirement| !satisfied.contains(&requirement.id))
            .cloned()
            .collect()
    }

    fn dissent(&self) -> Vec<Dissent> {
        vec![
            Dissent::of(Side::Claimant, &self.claimant),
            Dissent::of(Side::Challenger, &self.challenger),
        ]
    }
}

/// A position preserved in the record whether or not it prevailed.
///
/// 23.18 and 23.19 both insist on this: an aggregated result that discards the losing position
/// is indistinguishable from one where nobody disagreed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dissent {
    pub side: Side,
    pub holder: Role,
    pub claim: String,
    pub blocking: bool,
}

impl Dissent {
    fn of(side: Side, position: &Position) -> Self {
        Dissent {
            side,
            holder: position.holder.clone(),
            claim: position.claim.clone(),
            blocking: position.blocking,
        }
    }
}

/// A declared, checkable trigger. The closed set is the point: a procedure cannot smuggle in a
/// condition that inspects something the record does not contain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case")]
pub enum RuleCondition {
    /// The named side has withdrawn.
    Conceded { side: Side },
    /// The named side is supported at or above `assurance` and the other side is not.
    SoleSupportAtLeast { side: Side, assurance: Assurance },
    /// Both sides are supported at or above `assurance`, so the evidence contradicts itself.
    ContradictorySupportAtLeast { assurance: Assurance },
    /// The side bearing the burden has nothing at or above `minimum` while the other side does.
    ///
    /// The second half matters. Without it, a dispute in which neither party has produced
    /// anything would resolve against whoever happened to carry the burden, which is a default
    /// dressed as a finding.
    BurdenUnmet { minimum: Assurance },
    /// The dispute is of a kind evidence about the world cannot settle.
    NotSettleableByEvidence,
}

/// What firing a rule does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RuleOutcome {
    Upholds { side: Side },
    /// Upholds whichever side did not carry the burden.
    UpholdsOpposingBurden,
    /// Hand the dispute to a named destination. This crate does not deliver it.
    Escalate { to: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub condition: RuleCondition,
    pub outcome: RuleOutcome,
    /// Why this rule is legitimate. Copied into the ruling, so the record explains itself.
    pub rationale: String,
}

/// Whether a ruling binds beyond the thread that produced it (23.18's adjudication record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RulingScope {
    ThreadOnly,
    Precedent,
}

/// An ordered list of rules, evaluated first-match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Procedure {
    pub name: String,
    pub rules: Vec<Rule>,
    pub scope: RulingScope,
}

impl Procedure {
    pub fn new(name: impl Into<String>, rules: Vec<Rule>, scope: RulingScope) -> Self {
        Procedure {
            name: name.into(),
            rules,
            scope,
        }
    }

    /// The default procedure for disputes about the world.
    ///
    /// Order is load-bearing and is the argument of the procedure. Concession first, because a
    /// party that has withdrawn should not be defended by the evidence rules. Contradiction next,
    /// so that two conflicting reproducible results escalate instead of being resolved by
    /// whichever rule happens to match. Sole deterministic support then decides, then documentary
    /// support. The burden rule is last and its threshold is deliberately lower than the
    /// documentary rules': a higher threshold would make it unreachable, since any case that met
    /// it would already have been decided by sole documentary support.
    pub fn evidence_first() -> Self {
        Procedure::new(
            "evidence-first",
            vec![
                Rule {
                    id: "concession-by-claimant".into(),
                    name: "the claimant conceded".into(),
                    condition: RuleCondition::Conceded {
                        side: Side::Claimant,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Challenger,
                    },
                    rationale: "a withdrawn claim is not defended on its behalf".into(),
                },
                Rule {
                    id: "concession-by-challenger".into(),
                    name: "the challenger conceded".into(),
                    condition: RuleCondition::Conceded {
                        side: Side::Challenger,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Claimant,
                    },
                    rationale: "a withdrawn challenge leaves the claim standing".into(),
                },
                Rule {
                    id: "contradictory-deterministic-evidence".into(),
                    name: "two deterministic results disagree".into(),
                    condition: RuleCondition::ContradictorySupportAtLeast {
                        assurance: Assurance::DeterministicTest,
                    },
                    outcome: RuleOutcome::Escalate {
                        to: "human-adjudicator".into(),
                    },
                    rationale: "reproducible results that contradict each other indicate a defect \
                                in the question; no decision rule over the same evidence can \
                                repair it"
                        .into(),
                },
                Rule {
                    id: "deterministic-evidence-for-claimant".into(),
                    name: "only the claim has a deterministic test behind it".into(),
                    condition: RuleCondition::SoleSupportAtLeast {
                        side: Side::Claimant,
                        assurance: Assurance::DeterministicTest,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Claimant,
                    },
                    rationale: "a reproducible result dominates unreproduced disagreement".into(),
                },
                Rule {
                    id: "deterministic-evidence-for-challenger".into(),
                    name: "only the challenge has a deterministic test behind it".into(),
                    condition: RuleCondition::SoleSupportAtLeast {
                        side: Side::Challenger,
                        assurance: Assurance::DeterministicTest,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Challenger,
                    },
                    rationale: "a reproducible counterexample dominates an unreproduced claim"
                        .into(),
                },
                Rule {
                    id: "documentary-evidence-for-claimant".into(),
                    name: "only the claim is documented".into(),
                    condition: RuleCondition::SoleSupportAtLeast {
                        side: Side::Claimant,
                        assurance: Assurance::SourceDocument,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Claimant,
                    },
                    rationale: "a cited source outranks an uncited contradiction".into(),
                },
                Rule {
                    id: "documentary-evidence-for-challenger".into(),
                    name: "only the challenge is documented".into(),
                    condition: RuleCondition::SoleSupportAtLeast {
                        side: Side::Challenger,
                        assurance: Assurance::SourceDocument,
                    },
                    outcome: RuleOutcome::Upholds {
                        side: Side::Challenger,
                    },
                    rationale: "a cited source outranks an uncited claim".into(),
                },
                Rule {
                    id: "burden-unmet".into(),
                    name: "the side bearing the burden produced nothing".into(),
                    condition: RuleCondition::BurdenUnmet {
                        minimum: Assurance::Measurement,
                    },
                    outcome: RuleOutcome::UpholdsOpposingBurden,
                    rationale: "the declared burden of proof was not discharged while the other \
                                side produced support"
                        .into(),
                },
            ],
            RulingScope::ThreadOnly,
        )
    }

    /// Evidence first, then referral for the questions evidence cannot reach.
    ///
    /// 23.18 allows a declared decision rule "when evidence does not uniquely settle a normative
    /// or underdetermined question". The referral is appended *after* the evidence rules, never
    /// before: a jury that can overrule a deterministic test is the failure mode 23.19 names.
    pub fn with_jury_referral(mut self, jury: impl Into<String>) -> Self {
        self.name = format!("{}+jury", self.name);
        self.rules.push(Rule {
            id: "refer-normative-question-to-jury".into(),
            name: "the question is normative".into(),
            condition: RuleCondition::NotSettleableByEvidence,
            outcome: RuleOutcome::Escalate { to: jury.into() },
            rationale: "policy, authority and resource disputes are decided by a declared decision \
                        rule, and only after the evidence rules have failed to apply"
                .into(),
        });
        self
    }

    /// Runs the procedure. Deterministic: the same dispute and procedure always give the same
    /// ruling, which is what makes an adjudication record auditable.
    pub fn adjudicate(&self, dispute: &Dispute) -> Ruling {
        for rule in &self.rules {
            if !holds(&rule.condition, dispute) {
                continue;
            }
            let side = match &rule.outcome {
                RuleOutcome::Upholds { side } => *side,
                RuleOutcome::UpholdsOpposingBurden => dispute.burden.opposite(),
                RuleOutcome::Escalate { to } => {
                    return Ruling::Escalated {
                        rule: rule.id.clone(),
                        rule_name: rule.name.clone(),
                        to: to.clone(),
                        reason: rule.rationale.clone(),
                        positions: dispute.dissent(),
                        missing_evidence: dispute.missing_evidence(),
                    }
                }
            };
            return Ruling::Resolved {
                rule: rule.id.clone(),
                rule_name: rule.name.clone(),
                prevailing: side,
                holder: dispute.position(side).holder.clone(),
                basis: rule.rationale.clone(),
                dissent: vec![Dissent::of(
                    side.opposite(),
                    dispute.position(side.opposite()),
                )],
                scope: self.scope,
                still_missing: dispute.missing_evidence(),
            };
        }

        Ruling::Unresolved {
            reason: format!(
                "no rule of procedure {} applies to the submitted evidence",
                self.name
            ),
            missing_evidence: dispute.missing_evidence(),
            positions: dispute.dissent(),
        }
    }
}

fn holds(condition: &RuleCondition, dispute: &Dispute) -> bool {
    match condition {
        RuleCondition::Conceded { side } => dispute.position(*side).conceded,
        RuleCondition::SoleSupportAtLeast { side, assurance } => {
            let mine = dispute.strongest_support(*side);
            let theirs = dispute.strongest_support(side.opposite());
            mine.is_some_and(|level| level >= *assurance)
                && !theirs.is_some_and(|level| level >= *assurance)
        }
        RuleCondition::ContradictorySupportAtLeast { assurance } => {
            dispute
                .strongest_support(Side::Claimant)
                .is_some_and(|level| level >= *assurance)
                && dispute
                    .strongest_support(Side::Challenger)
                    .is_some_and(|level| level >= *assurance)
        }
        RuleCondition::BurdenUnmet { minimum } => {
            let burdened = dispute.strongest_support(dispute.burden);
            let other = dispute.strongest_support(dispute.burden.opposite());
            !burdened.is_some_and(|level| level >= *minimum)
                && other.is_some_and(|level| level >= *minimum)
        }
        RuleCondition::NotSettleableByEvidence => !dispute.kind.settleable_by_evidence(),
    }
}

/// The outcome of running a procedure over a dispute.
///
/// Three shapes, and the third is not a weaker version of the first. [`Ruling::Unresolved`] is
/// the answer whenever the procedure ran to the end without a rule applying, and it names what is
/// missing so the dispute can be reopened by acquiring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ruling", rename_all = "snake_case")]
pub enum Ruling {
    Resolved {
        rule: String,
        rule_name: String,
        prevailing: Side,
        holder: Role,
        basis: String,
        dissent: Vec<Dissent>,
        scope: RulingScope,
        /// Requirements still open. A dispute can be settled by a rule that did not need them,
        /// and hiding that would overstate the result.
        still_missing: Vec<EvidenceRequirement>,
    },
    Escalated {
        rule: String,
        rule_name: String,
        to: String,
        reason: String,
        positions: Vec<Dissent>,
        missing_evidence: Vec<EvidenceRequirement>,
    },
    Unresolved {
        reason: String,
        /// Named, not counted. "Insufficient evidence" without a list is not actionable.
        missing_evidence: Vec<EvidenceRequirement>,
        positions: Vec<Dissent>,
    },
}

impl Ruling {
    /// True only for [`Ruling::Resolved`]. An escalation is a referral, not a decision.
    pub fn is_settled(&self) -> bool {
        matches!(self, Ruling::Resolved { .. })
    }

    pub fn prevailing(&self) -> Option<Side> {
        match self {
            Ruling::Resolved { prevailing, .. } => Some(*prevailing),
            _ => None,
        }
    }

    /// The rule that fired, if one did.
    pub fn rule(&self) -> Option<&str> {
        match self {
            Ruling::Resolved { rule, .. } | Ruling::Escalated { rule, .. } => Some(rule),
            Ruling::Unresolved { .. } => None,
        }
    }

    pub fn missing_evidence(&self) -> &[EvidenceRequirement] {
        match self {
            Ruling::Resolved { still_missing, .. } => still_missing,
            Ruling::Escalated {
                missing_evidence, ..
            }
            | Ruling::Unresolved {
                missing_evidence, ..
            } => missing_evidence,
        }
    }
}

/// The record 23.18 asks for: positions, evidence, procedure, result, dissent and scope, under
/// one digest so a later reader can tell whether they are looking at the same adjudication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdjudicationRecord {
    pub dispute: Dispute,
    pub procedure: Procedure,
    pub ruling: Ruling,
}

impl AdjudicationRecord {
    pub fn new(dispute: Dispute, procedure: Procedure) -> Self {
        let ruling = procedure.adjudicate(&dispute);
        AdjudicationRecord {
            dispute,
            procedure,
            ruling,
        }
    }

    /// A content hash over the whole record. Deterministic, so two adjudicators running the same
    /// procedure over the same dispute produce the same digest, and a disagreement about the
    /// digest is a disagreement about the inputs rather than about the outcome.
    pub fn digest(&self) -> Option<String> {
        let value = serde_json::to_value(self).ok()?;
        ContentHash::of_value(&value)
            .ok()
            .map(|hash| hash.as_str().to_string())
    }
}

/// A challenge in the weave ledger, paired with the claim it targets.
///
/// This is the seam between the kernel and this crate. The kernel enforced that a challenge
/// refers to a claim (23.05's antecedent rule) and then stopped; here the pair becomes something
/// a procedure can be run against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenChallenge {
    pub challenge_event: String,
    pub claim_event: String,
    pub challenger: Role,
    pub claimant: Role,
    pub claim: serde_json::Value,
    pub grounds: serde_json::Value,
}

impl OpenChallenge {
    /// Builds the two positions, with no evidence attached.
    ///
    /// Evidence is not read out of the ledger payloads: to the kernel a payload is opaque, and
    /// this crate is not going to start interpreting it. The caller attaches evidence explicitly,
    /// which is also what makes [`Ruling::Unresolved`] the honest default for a freshly opened
    /// challenge.
    pub fn dispute(&self, id: impl Into<String>, kind: DisputeKind, burden: Side) -> Dispute {
        Dispute::new(
            id,
            kind,
            Position::new(self.claimant.clone(), render(&self.claim)),
            Position::new(self.challenger.clone(), render(&self.grounds)),
            burden,
        )
    }
}

fn render(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Every challenge in a weave ledger, paired with the claim it targets.
///
/// A challenge whose antecedent is missing is skipped rather than reported: the kernel does not
/// admit such an act, so its presence would mean the ledger was tampered with, and
/// [`bioprism_weave::Ledger::verify_chain`] is the right tool for that question.
pub fn open_challenges(ledger: &Ledger) -> Vec<OpenChallenge> {
    ledger
        .events()
        .iter()
        .filter(|event| event.act.kind == ActKind::Challenge)
        .filter_map(|event| {
            let target = event.act.in_reply_to.as_ref()?;
            let claim = ledger.get(target)?;
            if claim.act.kind != ActKind::Claim {
                return None;
            }
            Some(OpenChallenge {
                challenge_event: event.id.clone(),
                claim_event: claim.id.clone(),
                challenger: Role::new(event.act.from.clone()),
                claimant: Role::new(claim.act.from.clone()),
                claim: claim.act.payload.clone(),
                grounds: event.act.payload.clone(),
            })
        })
        .collect()
}
