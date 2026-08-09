//! Evaluator independence, and the vote-counting that follows from it (26.01).
//!
//! 26.01's first protocol step is "declare evaluator independence and inputs", and its first
//! failure mode is a "circular oracle built from the evaluated model". Everything else in that
//! module — priority, veto, evidence weighting — is already discharged by
//! [`bioprism_evalengine::ladder`], which composes tiered evidence strongest-first so that a judge
//! has no code path by which to raise a deterministic conclusion. This module does not build a
//! second ladder. It builds the thing the ladder assumes and never checks: that the evaluators
//! feeding it are actually different sources.
//!
//! # Two evaluators reading one artifact are one evaluator
//!
//! This is `bioprism-choreography`'s quorum rule — five jurors reading one source are one vote —
//! and it applies to an oracle mesh for exactly the same reason. Three expert readers who each
//! read the same radiology report are not three independent reads of the patient; they are three
//! reads of one report, and a mesh that counts them as three has manufactured agreement out of a
//! shared input. [`Mesh::independence_classes`] partitions evaluators by shared input, and
//! [`Mesh::census`] reports **classes, not evaluators**, as the count.
//!
//! # Disagreement inside a class is a different event from disagreement across classes
//!
//! Two evaluators in the same class disagreeing means one of them is wrong about a shared input:
//! that is an evaluator defect, and [`Disagreement::WithinClass`] says so. Two classes
//! disagreeing means the evidence itself is not decisive: that is a finding about the case, and
//! [`Disagreement::AcrossClasses`] carries the witness. Averaging them destroys the distinction,
//! so nothing here averages.
//!
//! # The join with the reader panel
//!
//! `bioprism-bioeval` turns a reader panel into a reference *distribution* rather than a label, and
//! tallies one vote per rating. That is right as far as it goes, and it has no way to know that
//! three of its five raters read one report. [`Mesh::independent_ratings`] is the correction:
//! one rating per class, the class members named in the rater id, and a refusal when a class
//! disagrees with itself. Without it, the distribution that the rest of the scoring machinery
//! consumes has correlated mass in it, and correlated mass reads as confidence.
//!
//! # Not implemented
//!
//! No verdict. [`Mesh::contributions`] emits [`bioprism_evalengine::Contribution`] values for
//! [`bioprism_evalengine::compose`]; the priority policy, the veto arithmetic and the unknown
//! policy are that crate's. No calibration of a model judge — 26.01 lists "calibration error" as a
//! metric and defines no estimator, and a judge's calibration needs held-out labelled outcomes
//! this module never sees. No adjudication workflow: 26.01 step 6 routes unresolved cases to
//! adjudication, and adjudication procedures — positions, evidence requirements, rulings and the
//! `Unresolved` outcome — already exist in `bioprism-choreography`.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_evalengine::{Conclusion, Contribution, ScoreTier};
use serde::{Deserialize, Serialize};

use crate::error::MeshError;

/// The seven evaluator kinds 26.01 lists under "Evaluation target", in its own order.
///
/// The mapping onto [`ScoreTier`] is stated here rather than invented per call site, because a
/// mesh that lets each caller decide which tier an expert review sits in has no priority policy at
/// all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatorKind {
    /// "deterministic and formal properties"
    DeterministicProperty,
    /// "executable analysis and state-transition outcomes"
    ExecutableAnalysis,
    /// "metamorphic and differential relations"
    MetamorphicRelation,
    /// "statistical reference distributions"
    StatisticalReference,
    /// "longitudinal or prospective reveals"
    ProspectiveReveal,
    /// "expert review"
    ExpertReview,
    /// "calibrated model judges"
    CalibratedModelJudge,
}

impl EvaluatorKind {
    /// Every kind, in blueprint listing order.
    pub const ALL: [EvaluatorKind; 7] = [
        EvaluatorKind::DeterministicProperty,
        EvaluatorKind::ExecutableAnalysis,
        EvaluatorKind::MetamorphicRelation,
        EvaluatorKind::StatisticalReference,
        EvaluatorKind::ProspectiveReveal,
        EvaluatorKind::ExpertReview,
        EvaluatorKind::CalibratedModelJudge,
    ];

    /// The evidence tier a conclusion from this kind of evaluator enters the ladder at.
    ///
    /// Expert review and model judges both land on [`ScoreTier::Judge`]. That is not a claim that
    /// they are equally trustworthy — it is 26.01's own rule that neither may override a failing
    /// executable invariant, and the ladder enforces the rule by tier.
    pub fn tier(self) -> ScoreTier {
        match self {
            EvaluatorKind::DeterministicProperty => ScoreTier::Deterministic,
            EvaluatorKind::ExecutableAnalysis => ScoreTier::Execution,
            EvaluatorKind::MetamorphicRelation => ScoreTier::Property,
            EvaluatorKind::StatisticalReference | EvaluatorKind::ProspectiveReveal => {
                ScoreTier::Statistical
            }
            EvaluatorKind::ExpertReview | EvaluatorKind::CalibratedModelJudge => ScoreTier::Judge,
        }
    }

    /// Whether this evaluator's conclusion is derived from a model's own output distribution.
    pub fn is_model_judge(self) -> bool {
        matches!(self, EvaluatorKind::CalibratedModelJudge)
    }
}

/// One evaluator's declaration of what it is and what it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorDecl {
    pub id: String,
    pub kind: EvaluatorKind,
    /// Artifacts this evaluator consumed. Two evaluators sharing any of these are not independent.
    pub inputs: BTreeSet<String>,
    /// Artifacts this evaluator was *built from* — training data, a fine-tune, a prompt distilled
    /// from the system's own outputs. Distinct from `inputs`: reading the system's answer is
    /// normal, being made of it is circular.
    #[serde(default)]
    pub derived_from: BTreeSet<String>,
}

impl EvaluatorDecl {
    /// Declare an evaluator with no inputs yet.
    pub fn new(id: impl Into<String>, kind: EvaluatorKind) -> Self {
        EvaluatorDecl {
            id: id.into(),
            kind,
            inputs: BTreeSet::new(),
            derived_from: BTreeSet::new(),
        }
    }

    /// Record an artifact this evaluator read.
    pub fn reading(mut self, artifact: impl Into<String>) -> Self {
        self.inputs.insert(artifact.into());
        self
    }

    /// Record an artifact this evaluator was constructed from.
    pub fn built_from(mut self, artifact: impl Into<String>) -> Self {
        self.derived_from.insert(artifact.into());
        self
    }
}

/// A verdict an evaluator reached, before any priority policy is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorVerdict {
    pub evaluator: String,
    /// The state called. A free string so the mesh does not impose a state space on biology it
    /// cannot see; equality of this string is what "agreement" means here.
    pub position: String,
    /// Whether this evaluator declined to call the case. Retained rather than dropped: 26.01
    /// requires "retain disagreement and abstention", and an abstention is not a missing row.
    #[serde(default)]
    pub abstained: bool,
}

impl EvaluatorVerdict {
    /// A called position.
    pub fn called(evaluator: impl Into<String>, position: impl Into<String>) -> Self {
        EvaluatorVerdict {
            evaluator: evaluator.into(),
            position: position.into(),
            abstained: false,
        }
    }

    /// An explicit abstention, which is evidence about the case's difficulty.
    pub fn abstention(evaluator: impl Into<String>) -> Self {
        EvaluatorVerdict {
            evaluator: evaluator.into(),
            position: String::new(),
            abstained: true,
        }
    }
}

/// A set of evaluators declared against one system under evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    /// Artifacts belonging to the system under evaluation. An evaluator derived from any of these
    /// is circular and is refused at admission.
    pub system_artifacts: BTreeSet<String>,
    evaluators: Vec<EvaluatorDecl>,
}

impl Mesh {
    /// Start a mesh naming the artifacts that constitute the system under evaluation.
    pub fn for_system(system_artifacts: impl IntoIterator<Item = String>) -> Self {
        Mesh {
            system_artifacts: system_artifacts.into_iter().collect(),
            evaluators: Vec::new(),
        }
    }

    /// Admit an evaluator, refusing one built from the system it is meant to grade.
    ///
    /// The refusal is at admission rather than at scoring, because a circular oracle that has
    /// already produced a verdict has already produced a number somebody will quote.
    pub fn admit(&mut self, decl: EvaluatorDecl) -> Result<(), MeshError> {
        if self.evaluators.iter().any(|e| e.id == decl.id) {
            return Err(MeshError::DuplicateEvaluator(decl.id));
        }
        if let Some(artifact) = decl.derived_from.intersection(&self.system_artifacts).next() {
            return Err(MeshError::CircularOracle {
                evaluator: decl.id.clone(),
                artifact: artifact.clone(),
            });
        }
        self.evaluators.push(decl);
        Ok(())
    }

    /// The declared evaluators, in admission order.
    pub fn evaluators(&self) -> &[EvaluatorDecl] {
        &self.evaluators
    }

    /// Partition evaluators into independence classes by shared input.
    ///
    /// Transitive: A and B share artifact `x`, B and C share artifact `y`, so all three are one
    /// class. That is stronger than pairwise sharing and it is the right strength — C's read is
    /// contaminated by B's shared context whether or not C ever saw `x`.
    ///
    /// An evaluator with no declared inputs is its own class. That is deliberately generous, and
    /// [`Mesh::census`] flags it, because "declared nothing" and "shares nothing" look the same
    /// from here and only one of them is independence.
    pub fn independence_classes(&self) -> Vec<Vec<&str>> {
        let mut parent: Vec<usize> = (0..self.evaluators.len()).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let mut by_artifact: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, evaluator) in self.evaluators.iter().enumerate() {
            for artifact in &evaluator.inputs {
                match by_artifact.get(artifact.as_str()) {
                    Some(&other) => {
                        let (a, b) = (find(&mut parent, index), find(&mut parent, other));
                        if a != b {
                            parent[a] = b;
                        }
                    }
                    None => {
                        by_artifact.insert(artifact.as_str(), index);
                    }
                }
            }
        }
        let mut classes: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        for index in 0..self.evaluators.len() {
            let root = find(&mut parent, index);
            classes
                .entry(root)
                .or_default()
                .push(self.evaluators[index].id.as_str());
        }
        classes.into_values().collect()
    }

    /// Coverage of the mesh, counted in classes rather than in evaluators.
    pub fn census(&self) -> Result<Census, MeshError> {
        if self.evaluators.is_empty() {
            return Err(MeshError::Empty);
        }
        let classes = self.independence_classes();
        let undeclared: Vec<String> = self
            .evaluators
            .iter()
            .filter(|e| e.inputs.is_empty())
            .map(|e| e.id.clone())
            .collect();
        let kinds: BTreeSet<EvaluatorKind> = self.evaluators.iter().map(|e| e.kind).collect();
        let non_model_classes = classes
            .iter()
            .filter(|class| {
                class.iter().any(|id| {
                    self.evaluators
                        .iter()
                        .any(|e| e.id == *id && !e.kind.is_model_judge())
                })
            })
            .count();
        Ok(Census {
            evaluators: self.evaluators.len(),
            independent_classes: classes.len(),
            non_model_classes,
            kinds_present: kinds.into_iter().collect(),
            inputs_undeclared: undeclared,
        })
    }

    /// Classify the disagreement among a set of verdicts.
    ///
    /// Returns every disagreeing pair rather than a rate. 26.01 lists "oracle disagreement rate"
    /// as a metric and never defines its denominator — pairs, cases, or classes — so this returns
    /// the pairs and lets a caller who has decided on a denominator compute it.
    pub fn disagreements(
        &self,
        verdicts: &[EvaluatorVerdict],
    ) -> Result<Vec<Disagreement>, MeshError> {
        for verdict in verdicts {
            if !self.evaluators.iter().any(|e| e.id == verdict.evaluator) {
                return Err(MeshError::UnknownEvaluator(verdict.evaluator.clone()));
            }
        }
        let class_of: BTreeMap<&str, usize> = self
            .independence_classes()
            .into_iter()
            .enumerate()
            .flat_map(|(index, class)| class.into_iter().map(move |id| (id, index)))
            .collect();
        let called: Vec<&EvaluatorVerdict> = verdicts.iter().filter(|v| !v.abstained).collect();
        let mut found = Vec::new();
        for (i, left) in called.iter().enumerate() {
            for right in called.iter().skip(i + 1) {
                if left.position == right.position {
                    continue;
                }
                let same_class = class_of.get(left.evaluator.as_str())
                    == class_of.get(right.evaluator.as_str());
                let witness = Witness {
                    left: left.evaluator.clone(),
                    left_position: left.position.clone(),
                    right: right.evaluator.clone(),
                    right_position: right.position.clone(),
                };
                found.push(if same_class {
                    Disagreement::WithinClass(witness)
                } else {
                    Disagreement::AcrossClasses(witness)
                });
            }
        }
        Ok(found)
    }

    /// One [`bioprism_bioeval::Rating`] per independence class, for
    /// [`bioprism_bioeval::PanelAggregate::tally`].
    ///
    /// `bioprism-bioeval` turns a reader panel into a reference distribution rather than a label,
    /// which is the right move — and it tallies one vote per rating, so three readers of one
    /// report produce a 3/3 distribution that looks like strong agreement and is one read. This is
    /// the join: the mesh knows which raters share a source, so it collapses each class to a single
    /// rating before the panel is tallied. The rating's `rater` is the class members joined by
    /// `+`, so the composition survives into the aggregate rather than being an invisible
    /// correction.
    ///
    /// Refuses a class that disagrees with itself. That is a
    /// [`Disagreement::WithinClass`] — an evaluator defect — and collapsing it to a class position
    /// would resolve a defect by voting, which is exactly what a distribution over an unreliable
    /// panel launders. Abstentions are dropped from their class; a class in which everyone
    /// abstained contributes no rating, because an abstention is a fact about the case and not a
    /// position on it.
    pub fn independent_ratings(
        &self,
        verdicts: &[EvaluatorVerdict],
    ) -> Result<Vec<bioprism_bioeval::Rating>, MeshError> {
        for verdict in verdicts {
            if !self.evaluators.iter().any(|e| e.id == verdict.evaluator) {
                return Err(MeshError::UnknownEvaluator(verdict.evaluator.clone()));
            }
        }
        let mut out = Vec::new();
        for class in self.independence_classes() {
            let held: Vec<&EvaluatorVerdict> = verdicts
                .iter()
                .filter(|v| !v.abstained && class.contains(&v.evaluator.as_str()))
                .collect();
            if held.is_empty() {
                continue;
            }
            let positions: BTreeSet<&str> = held.iter().map(|v| v.position.as_str()).collect();
            if positions.len() > 1 {
                return Err(MeshError::ClassSplit {
                    class: class.iter().map(|id| (*id).to_string()).collect(),
                    positions: positions.into_iter().map(str::to_string).collect(),
                });
            }
            let voters: Vec<&str> = held.iter().map(|v| v.evaluator.as_str()).collect();
            out.push(bioprism_bioeval::Rating::new(
                voters.join("+"),
                held[0].position.clone(),
            ));
        }
        Ok(out)
    }

    /// Convert verdicts into ladder contributions for [`bioprism_evalengine::compose`].
    ///
    /// One contribution per evaluator, at the tier its kind implies. Abstentions become
    /// [`Conclusion::Unknown`] with the evaluator's own reason, which is what keeps an abstention
    /// from being read as a fail by whatever unknown policy the caller declared.
    pub fn contributions(
        &self,
        verdicts: &[EvaluatorVerdict],
        expected: &str,
    ) -> Result<Vec<Contribution>, MeshError> {
        let mut out = Vec::new();
        for verdict in verdicts {
            let decl = self
                .evaluators
                .iter()
                .find(|e| e.id == verdict.evaluator)
                .ok_or_else(|| MeshError::UnknownEvaluator(verdict.evaluator.clone()))?;
            let (conclusion, note) = if verdict.abstained {
                (Conclusion::Unknown, format!("{} abstained", decl.id))
            } else if verdict.position == expected {
                (Conclusion::Pass, format!("called `{expected}`"))
            } else {
                (
                    Conclusion::Fail,
                    format!("called `{}`, expected `{expected}`", verdict.position),
                )
            };
            let mut contribution = Contribution::new(decl.kind.tier(), &decl.id, conclusion);
            contribution.notes.push(note);
            out.push(contribution);
        }
        Ok(out)
    }
}

/// A concrete, checkable statement that two named evaluators called different states.
///
/// A witness, in the workspace's sense: an object, not a score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Witness {
    pub left: String,
    pub left_position: String,
    pub right: String,
    pub right_position: String,
}

/// What kind of event a disagreement is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Disagreement {
    /// Two evaluators reading the same evidence reached different states. One of them has a
    /// defect; the case has not been shown to be hard.
    WithinClass(Witness),
    /// Two independent lines of evidence point different ways. The case is genuinely unresolved
    /// and 26.01 routes it to adjudication rather than to a majority.
    AcrossClasses(Witness),
}

impl Disagreement {
    /// The witness, whichever kind this is.
    pub fn witness(&self) -> &Witness {
        match self {
            Disagreement::WithinClass(w) | Disagreement::AcrossClasses(w) => w,
        }
    }

    /// Whether this disagreement is evidence about the case rather than about the mesh.
    pub fn is_about_the_case(&self) -> bool {
        matches!(self, Disagreement::AcrossClasses(_))
    }
}

/// Mesh coverage, reported in the unit that survives shared inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Census {
    /// How many evaluators were declared. Never the coverage figure.
    pub evaluators: usize,
    /// How many independent evidence sources those evaluators actually represent.
    pub independent_classes: usize,
    /// Classes containing at least one evaluator that is not a model judge. 26.01's protocol runs
    /// "the strongest available non-model oracles first"; a mesh where this is zero has none.
    pub non_model_classes: usize,
    pub kinds_present: Vec<EvaluatorKind>,
    /// Evaluators that declared no inputs, so their independence is unverified rather than
    /// established. Named after `bioprism-choreography`'s `IndependenceUnverified`.
    pub inputs_undeclared: Vec<String>,
}

impl Census {
    /// Whether the independence claim behind [`Census::independent_classes`] is checked.
    pub fn independence_verified(&self) -> bool {
        self.inputs_undeclared.is_empty()
    }
}
