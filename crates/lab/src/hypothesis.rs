//! Hypothesis separation and evidence obligations.
//!
//! Blueprint 09.03. Its purpose sentence is *"keep multiple plausible explanations or plans alive
//! long enough to retrieve evidence that distinguishes them"*, and its responsibility list ends
//! with "avoid performative branching". Both halves have a failure mode, and they are opposite
//! failures:
//!
//! - A lab that always picks a winner. Two hypotheses go in, one comes out, and the thing that
//!   chose was confidence rather than evidence.
//! - A lab that keeps everything alive forever. Branching becomes decorative, and the report says
//!   "three hypotheses remain" whichever way the evidence pointed.
//!
//! [`separate`] refuses the first by construction: the only thing that retires a hypothesis is a
//! *discharged* evidence obligation, or an observation that was actually made. A confidence number
//! cannot retire anything, and none is read here. It refuses the second by returning
//! [`SeparationVerdict::Separated`] the moment evidence in hand does distinguish, while still
//! reporting the disagreements that remain open among the survivors.
//!
//! And it has a third answer, which is the one that matters:
//! [`SeparationVerdict::NotSeparable`]. Two hypotheses that commit to the same things, or whose
//! only points of difference nothing reachable can settle, are **not separable**, and saying so is
//! the correct output. There is no tiebreak.
//!
//! # Reuse rather than reimplementation
//!
//! Evidence obligations come from `bioprism-obligation`, which already implements 39.06's decision
//! obligation graph: the closed eight-state set, the requirement that every state record carries
//! actor, time, confidence and evidence, and — load-bearing here —
//! [`bioprism_obligation::ObligationGraph::effective_states`], which caps an obligation at what its
//! dependencies allow. An obligation marked `satisfied` on top of an `open` precondition does not
//! separate anything, and reading `recorded_state` instead of the effective state is exactly how a
//! separation claim would be manufactured. A second obligation type in this crate would be a
//! second place for that rule to drift.
//!
//! The mapping from obligation state to discriminating power is deliberately narrow:
//!
//! | Effective state | Meaning here |
//! |---|---|
//! | `satisfied` | the assumption holds; hypotheses that deny it are retired |
//! | `contradicted` | the assumption fails; hypotheses that assert it are retired |
//! | `unresolvable` | this discriminator is dead and will not separate anything, ever |
//! | `waived_with_reason`, `not_applicable` | inadmissible — a waiver is a decision, not evidence |
//! | `unseen`, `open`, `partially_supported` | pending: names evidence still worth acquiring |
//!
//! # Not implemented, deliberately
//!
//! No hypothesis *generation*. 09.03 asks for diversity via "structural templates, independent
//! samples, counterfactual prompting, or specialist components"; all four need a model, and there
//! is none in this workspace. [`HypothesisSet`] takes hypotheses as given and enforces the one
//! diversity rule that can be checked without one: deduplication **by assumptions, not wording**.
//! No confidence propagation, no posterior, no Bayes factor — the evidence here is discrete and a
//! probability over it would be invented. No acquisition execution: naming the obligation to
//! discharge is where this module stops, and [`crate::context_value`] orders the naming.

use crate::error::SeparationError;
use bioprism_obligation::{ObligationGraph, ObligationState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a hypothesis says about one assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Asserts,
    Denies,
    /// Takes no position. Silence is **not** disagreement: an agnostic hypothesis is absent from
    /// the disagreement point rather than opposed to everything on it.
    Agnostic,
}

impl Stance {
    pub fn as_str(self) -> &'static str {
        match self {
            Stance::Asserts => "asserts",
            Stance::Denies => "denies",
            Stance::Agnostic => "agnostic",
        }
    }
}

/// Where two hypotheses differ.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "locus", content = "id")]
pub enum Locus {
    /// An assumption, settled by an evidence obligation of the same id.
    Assumption(String),
    /// A predicted observation, settled by having observed it.
    Observation(String),
}

impl Locus {
    pub fn id(&self) -> &str {
        match self {
            Locus::Assumption(id) | Locus::Observation(id) => id,
        }
    }
}

/// Why a hypothesis is no longer live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retirement {
    pub hypothesis: String,
    /// The locus whose settled outcome retired it.
    pub by: Locus,
    /// What the evidence said, in the words a reviewer would check.
    pub because: String,
}

/// A claim or plan with its commitments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub claim: String,
    /// Assumption id to stance. The identity of a hypothesis, per 09.03's deduplication rule.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assumptions: BTreeMap<String, Stance>,
    /// Observation id to the value this hypothesis predicts.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub predictions: BTreeMap<String, String>,
    /// Where the hypothesis came from. 09.03 requires it on the record.
    pub provenance: String,
    retired: Option<Retirement>,
}

impl Hypothesis {
    pub fn new(
        id: impl Into<String>,
        claim: impl Into<String>,
        provenance: impl Into<String>,
    ) -> Self {
        Hypothesis {
            id: id.into(),
            claim: claim.into(),
            assumptions: BTreeMap::new(),
            predictions: BTreeMap::new(),
            provenance: provenance.into(),
            retired: None,
        }
    }

    pub fn asserting(mut self, assumption: impl Into<String>) -> Self {
        self.assumptions.insert(assumption.into(), Stance::Asserts);
        self
    }

    pub fn denying(mut self, assumption: impl Into<String>) -> Self {
        self.assumptions.insert(assumption.into(), Stance::Denies);
        self
    }

    pub fn agnostic_about(mut self, assumption: impl Into<String>) -> Self {
        self.assumptions.insert(assumption.into(), Stance::Agnostic);
        self
    }

    pub fn predicting(mut self, observation: impl Into<String>, value: impl Into<String>) -> Self {
        self.predictions.insert(observation.into(), value.into());
        self
    }

    pub fn is_live(&self) -> bool {
        self.retired.is_none()
    }

    pub fn retirement(&self) -> Option<&Retirement> {
        self.retired.as_ref()
    }

    /// The commitments that make this hypothesis the hypothesis it is.
    ///
    /// 09.03: *"Deduplicate by assumptions, not wording."* Agnostic entries are excluded, because
    /// declining to take a position is not a position; two hypotheses that differ only in which
    /// assumptions they shrug at are one hypothesis with two write-ups.
    pub fn assumption_signature(&self) -> String {
        let mut parts: Vec<String> = self
            .assumptions
            .iter()
            .filter(|(_, stance)| **stance != Stance::Agnostic)
            .map(|(id, stance)| format!("{id}={}", stance.as_str()))
            .collect();
        parts.extend(
            self.predictions
                .iter()
                .map(|(id, value)| format!("{id}~{value}")),
        );
        parts.join(";")
    }

    fn commitment_at(&self, locus: &Locus) -> Option<String> {
        match locus {
            Locus::Assumption(id) => match self.assumptions.get(id) {
                Some(Stance::Asserts) => Some(Stance::Asserts.as_str().to_string()),
                Some(Stance::Denies) => Some(Stance::Denies.as_str().to_string()),
                Some(Stance::Agnostic) | None => None,
            },
            Locus::Observation(id) => self.predictions.get(id).cloned(),
        }
    }
}

/// One point on which live hypotheses differ. The nodes of 09.03's disagreement graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisagreementPoint {
    pub locus: Locus,
    /// Hypothesis id to the commitment it makes here. Hypotheses that are silent are absent.
    pub commitments: BTreeMap<String, String>,
}

impl DisagreementPoint {
    /// The distinct positions taken, in order. Two or more is what makes this a disagreement.
    pub fn sides(&self) -> Vec<&str> {
        let mut sides: Vec<&str> = self
            .commitments
            .values()
            .map(String::as_str)
            .collect::<BTreeSet<&str>>()
            .into_iter()
            .collect();
        sides.sort_unstable();
        sides
    }
}

/// A set of hypotheses, deduplicated by assumptions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HypothesisSet {
    hypotheses: BTreeMap<String, Hypothesis>,
}

impl HypothesisSet {
    pub fn new() -> Self {
        HypothesisSet::default()
    }

    /// Adds a hypothesis, refusing a duplicate id and a duplicate assumption signature.
    ///
    /// The second refusal is 09.03's diversity rule with teeth. A generator that paraphrases one
    /// explanation five ways has produced one hypothesis, and a lab that counts five is measuring
    /// its own prose.
    pub fn insert(&mut self, hypothesis: Hypothesis) -> Result<(), SeparationError> {
        if self.hypotheses.contains_key(&hypothesis.id) {
            return Err(SeparationError::DuplicateHypothesis(hypothesis.id));
        }
        let signature = hypothesis.assumption_signature();
        if let Some(existing) = self
            .hypotheses
            .values()
            .find(|other| other.assumption_signature() == signature)
        {
            return Err(SeparationError::DuplicateByAssumptions {
                incoming: hypothesis.id,
                existing: existing.id.clone(),
            });
        }
        self.hypotheses.insert(hypothesis.id.clone(), hypothesis);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Hypothesis> {
        self.hypotheses.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hypothesis> {
        self.hypotheses.values()
    }

    pub fn live(&self) -> Vec<&Hypothesis> {
        self.hypotheses
            .values()
            .filter(|hypothesis| hypothesis.is_live())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.hypotheses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hypotheses.is_empty()
    }

    /// Every locus on which two or more live hypotheses take different positions.
    pub fn disagreements(&self) -> Vec<DisagreementPoint> {
        let live = self.live();
        let mut loci: BTreeSet<Locus> = BTreeSet::new();
        for hypothesis in &live {
            for (id, stance) in &hypothesis.assumptions {
                if *stance != Stance::Agnostic {
                    loci.insert(Locus::Assumption(id.clone()));
                }
            }
            for id in hypothesis.predictions.keys() {
                loci.insert(Locus::Observation(id.clone()));
            }
        }

        let mut points = Vec::new();
        for locus in loci {
            let commitments: BTreeMap<String, String> = live
                .iter()
                .filter_map(|hypothesis| {
                    hypothesis
                        .commitment_at(&locus)
                        .map(|commitment| (hypothesis.id.clone(), commitment))
                })
                .collect();
            let point = DisagreementPoint { locus, commitments };
            if point.sides().len() >= 2 {
                points.push(point);
            }
        }
        points
    }

    /// Applies a verdict's retirements. The only path to a retired hypothesis.
    ///
    /// Refuses to empty the set: evidence that retires every live hypothesis is evidence against
    /// the hypothesis *set*, and silently leaving nothing alive would present that as a resolution.
    pub fn apply(&mut self, verdict: &SeparationVerdict) -> Result<usize, SeparationError> {
        let SeparationVerdict::Separated { retired, .. } = verdict else {
            return Ok(0);
        };
        let live_after = self
            .live()
            .iter()
            .filter(|hypothesis| !retired.iter().any(|r| r.hypothesis == hypothesis.id))
            .count();
        if live_after == 0 {
            return Err(SeparationError::WouldRetireAll);
        }
        let mut applied = 0usize;
        for retirement in retired {
            let hypothesis = self
                .hypotheses
                .get_mut(&retirement.hypothesis)
                .ok_or_else(|| SeparationError::UnknownHypothesis(retirement.hypothesis.clone()))?;
            if hypothesis.is_live() {
                hypothesis.retired = Some(retirement.clone());
                applied += 1;
            }
        }
        Ok(applied)
    }
}

/// An evidence obligation that would separate hypotheses but has not yet been discharged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSeparator {
    pub locus: Locus,
    /// The obligation id in the graph, or `None` when the locus is an observation nobody has made.
    pub obligation: Option<String>,
    /// The effective state of that obligation, or `None` for an unobserved observation.
    pub state: Option<ObligationState>,
    /// Which hypotheses it would tell apart.
    pub separates: Vec<String>,
}

/// Evidence that did settle a locus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DischargedSeparator {
    pub locus: Locus,
    /// What the evidence established, as a commitment string comparable to a hypothesis's own.
    pub outcome: String,
    /// Evidence locators from the obligation's history, or from the observation record.
    pub evidence: Vec<String>,
}

/// Why two hypotheses cannot be told apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum NotSeparableReason {
    /// The live hypotheses make the same commitments wherever both of them speak. Different
    /// wording, one hypothesis.
    NoDisagreement,
    /// They differ, but the obligation graph contains nothing that would settle any difference.
    /// The correct next step is to compile an obligation, not to pick a side.
    NoObligationCovers { loci: Vec<Locus> },
    /// Every discriminating obligation is `unresolvable`: no evidence reachable at this decision
    /// time can settle it. This is the case a tiebreak would silently paper over.
    EveryDiscriminatorUnresolvable { loci: Vec<Locus> },
    /// Every discriminating obligation was closed by decision rather than by evidence — waived or
    /// declared not applicable. A waiver may let an action proceed; it never tells you which
    /// hypothesis was right.
    EveryDiscriminatorInadmissible { loci: Vec<Locus> },
}

/// The three answers, of which the third is the one a lab usually owes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum SeparationVerdict {
    /// Evidence in hand distinguishes them.
    Separated {
        by: Vec<DischargedSeparator>,
        retired: Vec<Retirement>,
        surviving: Vec<String>,
        /// Disagreements still open among the survivors. Non-empty here is 09.03's collapse
        /// control: one settled question does not license retiring the rest of the argument.
        remaining: Vec<PendingSeparator>,
    },
    /// Not yet distinguished, but the evidence that would distinguish them is named.
    Separable { obligations: Vec<PendingSeparator> },
    /// They cannot be told apart, and this is the answer rather than a failure to produce one.
    NotSeparable {
        reason: NotSeparableReason,
        disagreements: Vec<DisagreementPoint>,
    },
}

impl SeparationVerdict {
    /// Whether a caller may act as though one hypothesis has won.
    pub fn licenses_a_winner(&self) -> bool {
        matches!(
            self,
            SeparationVerdict::Separated { surviving, .. } if surviving.len() == 1
        )
    }
}

/// Observations actually made, keyed by observation id.
///
/// Separate from the obligation graph because an obligation records *whether a question was
/// answered*, and separating two predictions needs *what the answer was*.
pub type Observations = BTreeMap<String, String>;

/// Decides whether the live hypotheses can be told apart, and by what.
///
/// `graph` supplies evidence obligations for assumption loci; `observations` supplies values for
/// observation loci. Both are read, never written: this function has no side effects and applying
/// its retirements is [`HypothesisSet::apply`]'s job, so a caller can inspect a verdict before
/// acting on it.
pub fn separate(
    set: &HypothesisSet,
    graph: &ObligationGraph,
    observations: &Observations,
) -> Result<SeparationVerdict, SeparationError> {
    let live = set.live();
    if live.len() < 2 {
        return Err(SeparationError::TooFewHypotheses(live.len()));
    }
    let effective = graph
        .effective_states()
        .map_err(|error| SeparationError::ObligationGraph(error.to_string()))?;

    let disagreements = set.disagreements();
    if disagreements.is_empty() {
        return Ok(SeparationVerdict::NotSeparable {
            reason: NotSeparableReason::NoDisagreement,
            disagreements,
        });
    }

    let mut discharged: Vec<DischargedSeparator> = Vec::new();
    let mut retired: Vec<Retirement> = Vec::new();
    let mut pending: Vec<PendingSeparator> = Vec::new();
    let mut uncovered: Vec<Locus> = Vec::new();
    let mut unresolvable: Vec<Locus> = Vec::new();
    let mut inadmissible: Vec<Locus> = Vec::new();

    for point in &disagreements {
        let separates: Vec<String> = point.commitments.keys().cloned().collect();
        match &point.locus {
            Locus::Assumption(id) => {
                let Some(state) = effective.get(id).copied() else {
                    uncovered.push(point.locus.clone());
                    continue;
                };
                match state {
                    ObligationState::Satisfied | ObligationState::Contradicted => {
                        let outcome = if state == ObligationState::Satisfied {
                            Stance::Asserts
                        } else {
                            Stance::Denies
                        };
                        let evidence = graph
                            .get(id)
                            .map(|obligation| {
                                obligation
                                    .evidence()
                                    .into_iter()
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default();
                        discharged.push(DischargedSeparator {
                            locus: point.locus.clone(),
                            outcome: outcome.as_str().to_string(),
                            evidence,
                        });
                        for (hypothesis, commitment) in &point.commitments {
                            if commitment != outcome.as_str() {
                                retired.push(Retirement {
                                    hypothesis: hypothesis.clone(),
                                    by: point.locus.clone(),
                                    because: format!(
                                        "hypothesis {commitment} `{id}`; the evidence says the assumption {}",
                                        if outcome == Stance::Asserts { "holds" } else { "fails" }
                                    ),
                                });
                            }
                        }
                    }
                    ObligationState::Unresolvable => unresolvable.push(point.locus.clone()),
                    ObligationState::WaivedWithReason | ObligationState::NotApplicable => {
                        inadmissible.push(point.locus.clone())
                    }
                    ObligationState::Unseen
                    | ObligationState::Open
                    | ObligationState::PartiallySupported => pending.push(PendingSeparator {
                        locus: point.locus.clone(),
                        obligation: Some(id.clone()),
                        state: Some(state),
                        separates,
                    }),
                }
            }
            Locus::Observation(id) => match observations.get(id) {
                Some(observed) => {
                    discharged.push(DischargedSeparator {
                        locus: point.locus.clone(),
                        outcome: observed.clone(),
                        evidence: vec![format!("observation:{id}")],
                    });
                    for (hypothesis, commitment) in &point.commitments {
                        if commitment != observed {
                            retired.push(Retirement {
                                hypothesis: hypothesis.clone(),
                                by: point.locus.clone(),
                                because: format!(
                                    "predicted `{commitment}` for `{id}`; observed `{observed}`"
                                ),
                            });
                        }
                    }
                }
                None => pending.push(PendingSeparator {
                    locus: point.locus.clone(),
                    obligation: effective.contains_key(id).then(|| id.clone()),
                    state: effective.get(id).copied(),
                    separates,
                }),
            },
        }
    }

    if !retired.is_empty() {
        let retired_ids: BTreeSet<&str> = retired
            .iter()
            .map(|retirement| retirement.hypothesis.as_str())
            .collect();
        let surviving: Vec<String> = live
            .iter()
            .filter(|hypothesis| !retired_ids.contains(hypothesis.id.as_str()))
            .map(|hypothesis| hypothesis.id.clone())
            .collect();
        if surviving.is_empty() {
            return Err(SeparationError::WouldRetireAll);
        }
        let remaining: Vec<PendingSeparator> = pending
            .into_iter()
            .filter(|entry| {
                entry
                    .separates
                    .iter()
                    .filter(|id| surviving.contains(id))
                    .count()
                    >= 2
            })
            .collect();
        return Ok(SeparationVerdict::Separated {
            by: discharged,
            retired,
            surviving,
            remaining,
        });
    }

    if !pending.is_empty() {
        return Ok(SeparationVerdict::Separable {
            obligations: pending,
        });
    }
    if !unresolvable.is_empty() {
        return Ok(SeparationVerdict::NotSeparable {
            reason: NotSeparableReason::EveryDiscriminatorUnresolvable { loci: unresolvable },
            disagreements,
        });
    }
    if !inadmissible.is_empty() {
        return Ok(SeparationVerdict::NotSeparable {
            reason: NotSeparableReason::EveryDiscriminatorInadmissible { loci: inadmissible },
            disagreements,
        });
    }
    Ok(SeparationVerdict::NotSeparable {
        reason: NotSeparableReason::NoObligationCovers { loci: uncovered },
        disagreements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_obligation::{Obligation, StateRecord};
    use bioprism_scope::Timestamp;

    const AT: Timestamp = Timestamp::from_nanos_utc(1_700_000_000_000_000_000);

    fn graph_with(states: &[(&str, ObligationState, &[&str])]) -> ObligationGraph {
        let mut graph = ObligationGraph::new("distinguish the two explanations");
        for (id, _, _) in states {
            graph
                .insert(Obligation::new(*id, format!("is `{id}` true?")))
                .unwrap();
        }
        for (id, state, evidence) in states {
            if *state == ObligationState::Unseen {
                continue;
            }
            let mut record = StateRecord::new(*state, "analyst", AT, 0.9)
                .with_evidence(evidence.iter().copied());
            if state.requires_reason() {
                record = record.with_reason("stated for the test");
            }
            graph.record(id, record).unwrap();
        }
        graph
    }

    fn two_rival_hypotheses() -> HypothesisSet {
        let mut set = HypothesisSet::new();
        set.insert(
            Hypothesis::new(
                "retry",
                "the retry path double-writes",
                "template:structural",
            )
            .asserting("idempotency_key_stable")
            .predicting("duplicate_rows", "present"),
        )
        .unwrap();
        set.insert(
            Hypothesis::new(
                "clock",
                "the clock skew reorders writes",
                "template:structural",
            )
            .denying("idempotency_key_stable")
            .predicting("duplicate_rows", "absent"),
        )
        .unwrap();
        set
    }

    /// The same rivalry with the predicted observation removed, so the only locus that could
    /// separate them is the assumption. Used wherever a test needs the assumption's fate to be the
    /// whole story.
    fn two_rivals_on_one_assumption() -> HypothesisSet {
        let mut set = HypothesisSet::new();
        set.insert(
            Hypothesis::new(
                "retry",
                "the retry path double-writes",
                "template:structural",
            )
            .asserting("idempotency_key_stable"),
        )
        .unwrap();
        set.insert(
            Hypothesis::new(
                "clock",
                "the clock skew reorders writes",
                "template:structural",
            )
            .denying("idempotency_key_stable"),
        )
        .unwrap();
        set
    }

    #[test]
    fn two_hypotheses_with_the_same_assumptions_and_different_wording_are_one_hypothesis() {
        let mut set = HypothesisSet::new();
        set.insert(Hypothesis::new("a", "the key is unstable", "sample:1").denying("key_stable"))
            .unwrap();
        assert_eq!(
            set.insert(
                Hypothesis::new("b", "the idempotency key does not hold", "sample:2")
                    .denying("key_stable")
            ),
            Err(SeparationError::DuplicateByAssumptions {
                incoming: "b".to_string(),
                existing: "a".to_string(),
            })
        );
    }

    #[test]
    fn a_hypothesis_that_is_agnostic_is_absent_from_the_disagreement_not_opposed_on_it() {
        let mut set = HypothesisSet::new();
        set.insert(Hypothesis::new("a", "asserts", "t").asserting("k"))
            .unwrap();
        set.insert(Hypothesis::new("b", "denies", "t").denying("k"))
            .unwrap();
        set.insert(
            Hypothesis::new("c", "agnostic", "t")
                .agnostic_about("k")
                .predicting("other", "x"),
        )
        .unwrap();
        let points = set.disagreements();
        let assumption = points
            .iter()
            .find(|point| point.locus == Locus::Assumption("k".to_string()))
            .unwrap();
        assert_eq!(
            assumption.commitments.keys().collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn hypotheses_that_differ_with_no_obligation_covering_the_difference_are_not_separable() {
        let set = two_rivals_on_one_assumption();
        let graph = ObligationGraph::new("nothing compiled yet");
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        assert!(matches!(
            verdict,
            SeparationVerdict::NotSeparable {
                reason: NotSeparableReason::NoObligationCovers { .. },
                ..
            }
        ));
        assert!(!verdict.licenses_a_winner());
    }

    #[test]
    fn an_open_obligation_makes_the_pair_separable_and_names_the_evidence_to_acquire() {
        let set = two_rival_hypotheses();
        let graph = graph_with(&[("idempotency_key_stable", ObligationState::Open, &[])]);
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        let SeparationVerdict::Separable { obligations } = verdict else {
            panic!("expected separable, got {verdict:?}");
        };
        assert!(obligations.iter().any(|entry| entry.obligation.as_deref()
            == Some("idempotency_key_stable")
            && entry.separates == vec!["clock".to_string(), "retry".to_string()]));
    }

    #[test]
    fn a_satisfied_obligation_retires_the_hypotheses_that_deny_it() {
        let mut set = two_rival_hypotheses();
        let graph = graph_with(&[(
            "idempotency_key_stable",
            ObligationState::Satisfied,
            &["trace://retry-log#42"],
        )]);
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        let SeparationVerdict::Separated {
            ref retired,
            ref surviving,
            ..
        } = verdict
        else {
            panic!("expected separated, got {verdict:?}");
        };
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].hypothesis, "clock");
        assert_eq!(surviving, &vec!["retry".to_string()]);
        assert!(verdict.licenses_a_winner());
        assert_eq!(set.apply(&verdict).unwrap(), 1);
        assert!(!set.get("clock").unwrap().is_live());
    }

    #[test]
    fn an_obligation_satisfied_on_top_of_an_open_precondition_separates_nothing() {
        let set = two_rival_hypotheses();
        let mut graph = ObligationGraph::new("distinguish");
        graph
            .insert(Obligation::new("trace_complete", "is the trace complete?"))
            .unwrap();
        graph
            .insert(
                Obligation::new("idempotency_key_stable", "is the key stable?")
                    .depending_on(["trace_complete"]),
            )
            .unwrap();
        graph
            .record(
                "idempotency_key_stable",
                StateRecord::new(ObligationState::Satisfied, "analyst", AT, 1.0)
                    .with_evidence(["trace://retry-log#42"]),
            )
            .unwrap();
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        assert!(matches!(verdict, SeparationVerdict::Separable { .. }));
    }

    #[test]
    fn an_unresolvable_discriminator_leaves_the_pair_not_separable_rather_than_tied() {
        let set = two_rivals_on_one_assumption();
        let graph = graph_with(&[("idempotency_key_stable", ObligationState::Unresolvable, &[])]);
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        assert!(matches!(
            verdict,
            SeparationVerdict::NotSeparable {
                reason: NotSeparableReason::EveryDiscriminatorUnresolvable { .. },
                ..
            }
        ));
    }

    #[test]
    fn a_waived_discriminator_is_a_decision_and_never_names_a_winner() {
        let set = two_rivals_on_one_assumption();
        let graph = graph_with(&[(
            "idempotency_key_stable",
            ObligationState::WaivedWithReason,
            &[],
        )]);
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        assert!(matches!(
            verdict,
            SeparationVerdict::NotSeparable {
                reason: NotSeparableReason::EveryDiscriminatorInadmissible { .. },
                ..
            }
        ));
    }

    #[test]
    fn an_observation_that_matches_neither_prediction_retires_both_and_is_refused() {
        let mut set = HypothesisSet::new();
        set.insert(Hypothesis::new("a", "a", "t").predicting("rows", "present"))
            .unwrap();
        set.insert(Hypothesis::new("b", "b", "t").predicting("rows", "absent"))
            .unwrap();
        let graph = ObligationGraph::new("none");
        let observations: Observations =
            [("rows".to_string(), "partially_present".to_string())].into();
        assert_eq!(
            separate(&set, &graph, &observations),
            Err(SeparationError::WouldRetireAll)
        );
    }

    #[test]
    fn separation_on_one_locus_reports_the_disagreements_that_remain_among_survivors() {
        let mut set = HypothesisSet::new();
        set.insert(Hypothesis::new("a", "a", "t").asserting("k").asserting("m"))
            .unwrap();
        set.insert(Hypothesis::new("b", "b", "t").asserting("k").denying("m"))
            .unwrap();
        set.insert(Hypothesis::new("c", "c", "t").denying("k").denying("m"))
            .unwrap();
        let graph = graph_with(&[
            ("k", ObligationState::Satisfied, &["doc://k"]),
            ("m", ObligationState::Open, &[]),
        ]);
        let verdict = separate(&set, &graph, &Observations::new()).unwrap();
        let SeparationVerdict::Separated {
            ref surviving,
            ref remaining,
            ..
        } = verdict
        else {
            panic!("expected separated, got {verdict:?}");
        };
        assert_eq!(surviving, &vec!["a".to_string(), "b".to_string()]);
        assert_eq!(remaining.len(), 1);
        assert!(!verdict.licenses_a_winner());
    }

    #[test]
    fn confidence_alone_never_retires_a_hypothesis() {
        let set = two_rival_hypotheses();
        let mut graph = ObligationGraph::new("distinguish");
        graph
            .insert(Obligation::new("idempotency_key_stable", "stable?"))
            .unwrap();
        graph
            .record(
                "idempotency_key_stable",
                StateRecord::new(ObligationState::Open, "confident-analyst", AT, 1.0),
            )
            .unwrap();
        assert!(matches!(
            separate(&set, &graph, &Observations::new()).unwrap(),
            SeparationVerdict::Separable { .. }
        ));
    }

    #[test]
    fn a_single_live_hypothesis_is_a_typed_error_not_a_declared_winner() {
        let mut set = HypothesisSet::new();
        set.insert(Hypothesis::new("only", "the only story", "t").asserting("k"))
            .unwrap();
        assert_eq!(
            separate(&set, &ObligationGraph::new("g"), &Observations::new()),
            Err(SeparationError::TooFewHypotheses(1))
        );
    }
}
