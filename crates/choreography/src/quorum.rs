//! Aggregation, quorum and jury models.
//!
//! Blueprint 23.19. The failure this module is built around is stated there in one line: *"Five
//! agents using the same model, prompt, and context are not five independent reviewers."* A count
//! of approvals is only a quorum if the approvals are independent, and independence is a property
//! of where the jurors got their information, not of how many names appear on the list.
//!
//! # Independence is declared and then checked
//!
//! Every [`Juror`] carries an [`EvidenceBasis`]: the evidence sources it read, the model family it
//! ran on, and a digest of the context it was given. [`Jury::decide`] partitions the jurors into
//! correlation clusters — two jurors that share an evidence source, a model family or a context
//! are in the same cluster — and counts one effective vote per cluster. Five jurors reading one
//! source are one vote.
//!
//! A juror that declares no basis is not assumed independent and is not assumed correlated: the
//! whole tally returns [`QuorumOutcome::IndependenceUnverified`], naming the jurors. This is the
//! type-level half of the requirement. [`QuorumOutcome::Reached`] holds a [`CheckedIndependence`]
//! rather than an enum that could also mean "unverified", so a result that reached quorum cannot
//! be constructed without the clustering that justified it, and a reader of a serialised outcome
//! can always see which correlations were found.
//!
//! # What a vote may not do
//!
//! Two rules from 23.19 are enforced rather than documented. A [`QuorumRule`] declares which
//! [`QuestionKind`]s it may decide, so a rule written for preferences cannot be pointed at a
//! factual question ([`QuorumOutcome::NotDecidableByVote`]). And a deterministic finding that
//! contradicts the majority produces [`QuorumOutcome::OverriddenByDeterministicEvidence`] instead
//! of a quorum: *"majority does not override deterministic failure evidence"*.
//!
//! # Deliberately not implemented
//!
//! No weighted jury: 23.19 weights judgements by "domain- and condition-specific PRISM capability
//! posteriors, calibration, and error correlation", and none of those quantities is defined
//! anywhere in the section — a weight invented here would be a number with no provenance
//! pretending to be a measurement. Historical error correlation is likewise named but never
//! given a definition, so it is not part of [`EvidenceBasis`]; only correlations that can be read
//! off a declaration are checked. Pareto retention, best-evidence synthesis and adversarial-judge
//! rubrics are separate aggregation families and are not implemented. There is no tie-break: a
//! tally that does not reach the threshold is [`QuorumOutcome::NotReached`], not a coin flip.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::session::Role;

/// What kind of question is being put to the jury.
///
/// 23.19: "Votes are appropriate for preferences or underdetermined choices." Recording the kind
/// is what lets [`QuorumRule::decides`] refuse the ones a vote should not touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionKind {
    /// A choice among acceptable options, where taste is the deciding input.
    Preference,
    /// A question the available evidence genuinely does not settle.
    Underdetermined,
    /// A question about the world, which has an answer whether or not the jury likes it.
    Factual,
    /// A question about what is permitted, which belongs to an authority.
    Normative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vote {
    Approve,
    Reject,
    Abstain,
    /// A blocking objection from a role the rule grants veto power.
    Veto,
}

/// Where a juror's judgement came from.
///
/// Behavioural diversity is what 23.19 asks for, and this is the part of it that can be declared
/// and mechanically compared. Tool overlap is recorded but deliberately does not create a
/// correlation on its own: two reviewers using the same test runner are not thereby one reviewer,
/// whereas two reviewers reading the same single document are.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EvidenceBasis {
    /// Identifiers of the evidence this juror actually read.
    #[serde(default)]
    pub sources: BTreeSet<String>,
    /// Model family, where known. `None` means unknown, not "different from everyone else".
    #[serde(default)]
    pub model_family: Option<String>,
    /// Digest of the context capsule this juror was given (23.09).
    #[serde(default)]
    pub context_digest: Option<String>,
    /// Tools available to this juror. Recorded for the record, not used for clustering.
    #[serde(default)]
    pub tools: BTreeSet<String>,
}

impl EvidenceBasis {
    pub fn from_sources(sources: impl IntoIterator<Item = impl Into<String>>) -> Self {
        EvidenceBasis {
            sources: sources.into_iter().map(Into::into).collect(),
            ..EvidenceBasis::default()
        }
    }

    pub fn on_model(mut self, family: impl Into<String>) -> Self {
        self.model_family = Some(family.into());
        self
    }

    pub fn with_context(mut self, digest: impl Into<String>) -> Self {
        self.context_digest = Some(digest.into());
        self
    }

    /// A basis with nothing in it declares nothing, and an undeclared basis is not evidence of
    /// independence.
    pub fn is_declared(&self) -> bool {
        !self.sources.is_empty() || self.model_family.is_some() || self.context_digest.is_some()
    }

    fn correlation_with(&self, other: &EvidenceBasis) -> Option<CorrelationReason> {
        if let Some(shared) = self.sources.intersection(&other.sources).next() {
            return Some(CorrelationReason::SharedEvidenceSource {
                source: shared.clone(),
            });
        }
        match (&self.context_digest, &other.context_digest) {
            (Some(mine), Some(theirs)) if mine == theirs => {
                return Some(CorrelationReason::IdenticalContext {
                    digest: mine.clone(),
                })
            }
            _ => {}
        }
        match (&self.model_family, &other.model_family) {
            (Some(mine), Some(theirs)) if mine == theirs => {
                Some(CorrelationReason::SameModelFamily {
                    family: mine.clone(),
                })
            }
            _ => None,
        }
    }
}

/// Why two jurors are not independent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum CorrelationReason {
    SharedEvidenceSource { source: String },
    IdenticalContext { digest: String },
    SameModelFamily { family: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Correlation {
    pub left: String,
    pub right: String,
    pub reason: CorrelationReason,
}

/// A set of jurors that vote as one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    pub jurors: Vec<String>,
    /// The cluster's single effective vote. `None` when its members disagree with each other
    /// despite a shared basis, which counts for neither side and is worth seeing.
    pub effective_vote: Option<Vote>,
}

/// The independence analysis that justifies a tally.
///
/// Present only on outcomes where the clustering actually ran. This is why
/// [`QuorumOutcome::Reached`] cannot exist without it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedIndependence {
    pub clusters: Vec<Cluster>,
    pub correlations: Vec<Correlation>,
}

impl CheckedIndependence {
    /// The number of genuinely independent voices, which is the number that may be compared
    /// against a threshold.
    pub fn independent_voices(&self) -> usize {
        self.clusters.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Juror {
    pub id: String,
    pub role: Role,
    pub vote: Vote,
    /// Absent when the juror did not declare where its judgement came from. Absence is fatal to
    /// the tally rather than treated as independence.
    #[serde(default)]
    pub basis: Option<EvidenceBasis>,
    #[serde(default)]
    pub rationale: String,
}

impl Juror {
    pub fn new(id: impl Into<String>, role: impl Into<Role>, vote: Vote) -> Self {
        Juror {
            id: id.into(),
            role: role.into(),
            vote,
            basis: None,
            rationale: String::new(),
        }
    }

    pub fn declaring(mut self, basis: EvidenceBasis) -> Self {
        self.basis = Some(basis);
        self
    }

    pub fn because(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }
}

/// A juror's position, preserved whichever way the tally went (23.19, dissent preservation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JurorDissent {
    pub juror: String,
    pub role: Role,
    pub vote: Vote,
    pub rationale: String,
}

/// A deterministic result bearing on the question.
///
/// If one exists, the vote is measured against it rather than the other way round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub summary: String,
    /// The conclusion the finding supports.
    pub supports: Vote,
}

/// What is being put to the jury.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub kind: QuestionKind,
    /// Prerequisite identifiers that have actually passed.
    #[serde(default)]
    pub satisfied_prerequisites: BTreeSet<String>,
    /// A deterministic result, if one is available.
    #[serde(default)]
    pub finding: Option<Finding>,
}

impl Question {
    pub fn new(id: impl Into<String>, kind: QuestionKind) -> Self {
        Question {
            id: id.into(),
            kind,
            satisfied_prerequisites: BTreeSet::new(),
            finding: None,
        }
    }

    pub fn satisfying(mut self, prerequisite: impl Into<String>) -> Self {
        self.satisfied_prerequisites.insert(prerequisite.into());
        self
    }

    pub fn with_finding(mut self, finding: Finding) -> Self {
        self.finding = Some(finding);
        self
    }
}

/// The declared decision rule: threshold, vetoes, prerequisites, and the questions it may decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumRule {
    pub name: String,
    /// Threshold in *effective* votes, one per independence cluster. Writing a threshold in raw
    /// participant counts is what makes correlated jurors look like agreement.
    pub required_approvals: usize,
    #[serde(default)]
    pub veto_roles: BTreeSet<Role>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
    pub decides: BTreeSet<QuestionKind>,
}

impl QuorumRule {
    /// The 23.19 example: two of three reviewers, a safety veto, and tests that must pass first.
    pub fn two_of_three_with_safety_veto() -> Self {
        QuorumRule {
            name: "2-of-3-with-safety-veto".into(),
            required_approvals: 2,
            veto_roles: BTreeSet::from([Role::new("safety")]),
            prerequisites: vec!["tests.pass".into()],
            decides: BTreeSet::from([QuestionKind::Preference, QuestionKind::Underdetermined]),
        }
    }

    pub fn new(name: impl Into<String>, required_approvals: usize) -> Self {
        QuorumRule {
            name: name.into(),
            required_approvals,
            veto_roles: BTreeSet::new(),
            prerequisites: Vec::new(),
            decides: BTreeSet::from([QuestionKind::Preference, QuestionKind::Underdetermined]),
        }
    }

    pub fn requiring(mut self, prerequisite: impl Into<String>) -> Self {
        self.prerequisites.push(prerequisite.into());
        self
    }

    pub fn deciding(mut self, kind: QuestionKind) -> Self {
        self.decides.insert(kind);
        self
    }
}

/// The result of a tally.
///
/// [`QuorumOutcome::is_quorum`] is true for exactly one variant. Everything else is a reason the
/// question was not decided by counting, and each names the reason precisely enough to act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum QuorumOutcome {
    Reached {
        rule: String,
        raw_approvals: usize,
        effective_approvals: usize,
        required: usize,
        independence: CheckedIndependence,
        dissent: Vec<JurorDissent>,
    },
    NotReached {
        rule: String,
        raw_approvals: usize,
        effective_approvals: usize,
        required: usize,
        independence: CheckedIndependence,
        dissent: Vec<JurorDissent>,
    },
    /// A veto role objected. Counting does not continue: a veto is not outvoted.
    Vetoed {
        rule: String,
        by: String,
        role: Role,
        rationale: String,
    },
    PrerequisiteUnmet {
        rule: String,
        missing: Vec<String>,
    },
    /// At least one juror did not declare an evidence basis, so no clustering was possible and no
    /// count can be called a quorum.
    IndependenceUnverified {
        rule: String,
        jurors: Vec<String>,
    },
    /// The rule does not claim authority over this kind of question.
    NotDecidableByVote {
        rule: String,
        kind: QuestionKind,
        reason: String,
    },
    /// The majority went one way and a deterministic result went the other.
    OverriddenByDeterministicEvidence {
        rule: String,
        finding: String,
        summary: String,
        effective_approvals: usize,
    },
}

impl QuorumOutcome {
    /// True only for [`QuorumOutcome::Reached`].
    pub fn is_quorum(&self) -> bool {
        matches!(self, QuorumOutcome::Reached { .. })
    }

    pub fn independence(&self) -> Option<&CheckedIndependence> {
        match self {
            QuorumOutcome::Reached { independence, .. }
            | QuorumOutcome::NotReached { independence, .. } => Some(independence),
            _ => None,
        }
    }

    pub fn effective_approvals(&self) -> Option<usize> {
        match self {
            QuorumOutcome::Reached {
                effective_approvals,
                ..
            }
            | QuorumOutcome::NotReached {
                effective_approvals,
                ..
            }
            | QuorumOutcome::OverriddenByDeterministicEvidence {
                effective_approvals,
                ..
            } => Some(*effective_approvals),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Jury {
    pub jurors: Vec<Juror>,
}

impl Jury {
    pub fn new(jurors: Vec<Juror>) -> Self {
        Jury { jurors }
    }

    /// Runs the rule over the jury. Order of checks is the argument, so it is stated here:
    ///
    /// 1. Is this even a question a vote may decide? A rule that does not claim the question
    ///    kind stops before anything is counted.
    /// 2. Did the prerequisites pass? 23.19 puts `tests.pass` alongside the approval count for a
    ///    reason — a review of a broken artifact is not a review.
    /// 3. Did a veto role object? A veto is not a vote and is not outvoted.
    /// 4. Did every juror declare a basis? If not, nothing downstream is a quorum.
    /// 5. Cluster, count one vote per cluster, compare with the threshold.
    /// 6. Does a deterministic finding contradict the result? Then the finding wins.
    pub fn decide(&self, rule: &QuorumRule, question: &Question) -> QuorumOutcome {
        if !rule.decides.contains(&question.kind) {
            return QuorumOutcome::NotDecidableByVote {
                rule: rule.name.clone(),
                kind: question.kind,
                reason: format!(
                    "rule {} declares authority over {:?} and this question is {:?}",
                    rule.name, rule.decides, question.kind
                ),
            };
        }

        let missing: Vec<String> = rule
            .prerequisites
            .iter()
            .filter(|prerequisite| !question.satisfied_prerequisites.contains(*prerequisite))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return QuorumOutcome::PrerequisiteUnmet {
                rule: rule.name.clone(),
                missing,
            };
        }

        if let Some(vetoer) = self
            .jurors
            .iter()
            .find(|juror| juror.vote == Vote::Veto && rule.veto_roles.contains(&juror.role))
        {
            return QuorumOutcome::Vetoed {
                rule: rule.name.clone(),
                by: vetoer.id.clone(),
                role: vetoer.role.clone(),
                rationale: vetoer.rationale.clone(),
            };
        }

        let undeclared: Vec<String> = self
            .jurors
            .iter()
            .filter(|juror| !juror.basis.as_ref().is_some_and(EvidenceBasis::is_declared))
            .map(|juror| juror.id.clone())
            .collect();
        if !undeclared.is_empty() {
            return QuorumOutcome::IndependenceUnverified {
                rule: rule.name.clone(),
                jurors: undeclared,
            };
        }

        let independence = self.cluster();
        let effective_approvals = independence
            .clusters
            .iter()
            .filter(|cluster| cluster.effective_vote == Some(Vote::Approve))
            .count();
        let raw_approvals = self
            .jurors
            .iter()
            .filter(|juror| juror.vote == Vote::Approve)
            .count();
        let dissent: Vec<JurorDissent> = self
            .jurors
            .iter()
            .filter(|juror| juror.vote != Vote::Approve)
            .map(|juror| JurorDissent {
                juror: juror.id.clone(),
                role: juror.role.clone(),
                vote: juror.vote,
                rationale: juror.rationale.clone(),
            })
            .collect();

        let reached = effective_approvals >= rule.required_approvals;
        if reached {
            if let Some(finding) = &question.finding {
                if finding.supports != Vote::Approve {
                    return QuorumOutcome::OverriddenByDeterministicEvidence {
                        rule: rule.name.clone(),
                        finding: finding.id.clone(),
                        summary: finding.summary.clone(),
                        effective_approvals,
                    };
                }
            }
            return QuorumOutcome::Reached {
                rule: rule.name.clone(),
                raw_approvals,
                effective_approvals,
                required: rule.required_approvals,
                independence,
                dissent,
            };
        }

        QuorumOutcome::NotReached {
            rule: rule.name.clone(),
            raw_approvals,
            effective_approvals,
            required: rule.required_approvals,
            independence,
            dissent,
        }
    }

    /// Partitions jurors into correlation clusters by union-find over declared bases.
    ///
    /// Correlation is transitive here: if A and B share a source and B and C share a model
    /// family, all three are one voice. That is the conservative direction, and the conservative
    /// direction is the correct one when the quantity being estimated is how much independent
    /// support a conclusion has.
    pub fn cluster(&self) -> CheckedIndependence {
        let count = self.jurors.len();
        let mut parent: Vec<usize> = (0..count).collect();
        let mut correlations = Vec::new();

        for left in 0..count {
            for right in (left + 1)..count {
                let (Some(left_basis), Some(right_basis)) =
                    (&self.jurors[left].basis, &self.jurors[right].basis)
                else {
                    continue;
                };
                if let Some(reason) = left_basis.correlation_with(right_basis) {
                    correlations.push(Correlation {
                        left: self.jurors[left].id.clone(),
                        right: self.jurors[right].id.clone(),
                        reason,
                    });
                    union(&mut parent, left, right);
                }
            }
        }

        let mut grouped: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for member in 0..count {
            grouped
                .entry(find(&mut parent, member))
                .or_default()
                .push(member);
        }

        let clusters = grouped
            .into_values()
            .map(|members| Cluster {
                jurors: members
                    .iter()
                    .map(|member| self.jurors[*member].id.clone())
                    .collect(),
                effective_vote: unify(members.iter().map(|member| self.jurors[*member].vote)),
            })
            .collect();

        CheckedIndependence {
            clusters,
            correlations,
        }
    }
}

/// A cluster's single vote: the one its non-abstaining members agree on, or `None` if they do not.
fn unify(votes: impl Iterator<Item = Vote>) -> Option<Vote> {
    let mut decided: Option<Vote> = None;
    for vote in votes {
        if vote == Vote::Abstain {
            continue;
        }
        match decided {
            None => decided = Some(vote),
            Some(existing) if existing == vote => {}
            Some(_) => return None,
        }
    }
    decided
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        parent[right_root.max(left_root)] = right_root.min(left_root);
    }
}
