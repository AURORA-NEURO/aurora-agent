//! Trajectory segmentation, decision boundaries and decision types.
//!
//! Blueprint 06.02 and 06.03. `bioprism_trace::segment` already proposes and ranks decision
//! boundaries with a transparent score, and this module does not replace it: [`boundaries`] calls
//! it and carries its [`bioprism_trace::CandidateScore`] through unchanged, so the ranking a
//! reviewer sees here is the same arithmetic, auditable in one place. What is added is the part
//! 06.03 asks for and the trace crate does not attempt — *what kind* of decision each boundary is,
//! how reversible it was, and which boundaries are not standalone cells at all.
//!
//! ## Episodes and repetition
//!
//! 06.02's hierarchy is trace → episode → subtask → decision → operation → event. Only two of those
//! levels are recoverable from the Trace IR without guessing: episodes, anchored on `Goal` events,
//! which are the high-precision rule anchor 06.02 asks rules to establish; and repetition, which is
//! computed from content digests. Subtask and operation levels are **not** reconstructed — 06.02
//! assigns them to "a sequence model or LLM", and inventing them from indentation-free event
//! streams would produce a hierarchy that looks authoritative and is not.
//!
//! The repetition distinction is the one worth having. 06.02 asks the compiler to tell "deliberate
//! iterative refinement from stuck behavior", and there is a crisp criterion in the IR for it:
//! between two identical actions, did anything new become visible? If yes the agent was refining
//! against new evidence; if no it was repeating itself. That is a no-progress metric computed from
//! recorded state, not a judgement about intent.
//!
//! ## What is deliberately not implemented
//!
//! No idle-gap detection: 06.02 lists "long idle gaps" as a segmentation signal and the Trace IR
//! deliberately carries no wall-clock time, because a trajectory that reorders under a different
//! scheduler must segment identically. No branch or memory-operation signals, because the IR has no
//! event kind for either; a producer that records them puts them in `payload`, where this module
//! reads them only if they are declared.

use bioprism_trace::{segment, CandidateScore, Event, EventKind, Trace};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A goal-anchored span of the trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Episode {
    pub index: usize,
    /// The `Goal` event that opened it, when there was one. The first episode of a trace that never
    /// states a goal has none, and that is reported rather than fabricated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_step: Option<usize>,
    pub label: String,
    pub steps: Vec<usize>,
}

/// Partitions a trace into episodes at every `Goal` event.
pub fn episodes(trace: &Trace) -> Vec<Episode> {
    let mut episodes: Vec<Episode> = Vec::new();
    for event in &trace.events {
        let opens_episode = event.kind == EventKind::Goal;
        if opens_episode || episodes.is_empty() {
            let label = event
                .payload
                .get("summary")
                .and_then(|value| value.as_str())
                .unwrap_or("<no goal recorded>")
                .to_string();
            episodes.push(Episode {
                index: episodes.len(),
                goal_step: opens_episode.then_some(event.step),
                label: if opens_episode {
                    label
                } else {
                    "<steps before any stated goal>".to_string()
                },
                steps: Vec::new(),
            });
        }
        if let Some(current) = episodes.last_mut() {
            current.steps.push(event.step);
        }
    }
    episodes
}

/// Why the same action appears more than once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Repetition {
    /// New evidence became visible between the repeats. The agent was iterating against something.
    IterativeRefinement { evidence_gained: Vec<String> },
    /// Nothing new became visible. The agent repeated itself with no additional information.
    Stuck { repeats: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatedAction {
    pub summary: String,
    pub steps: Vec<usize>,
    pub classification: Repetition,
}

/// Finds repeated actions and says whether each repeat made progress.
pub fn repetitions(trace: &Trace) -> Vec<RepeatedAction> {
    let mut groups: Vec<(String, String, Vec<usize>)> = Vec::new();
    for event in &trace.events {
        if event.kind != EventKind::Action {
            continue;
        }
        let digest = event.content_digest().as_str().to_string();
        match groups.iter_mut().find(|(key, _, _)| key == &digest) {
            Some((_, _, steps)) => steps.push(event.step),
            None => groups.push((digest, describe(event), vec![event.step])),
        }
    }

    groups
        .into_iter()
        .filter(|(_, _, steps)| steps.len() > 1)
        .map(|(_, summary, steps)| {
            let first = steps[0];
            let last = *steps.last().unwrap_or(&first);
            let before: BTreeSet<&str> = trace
                .events
                .iter()
                .filter(|event| event.step <= first)
                .flat_map(|event| event.visible.iter().map(String::as_str))
                .collect();
            let during: BTreeSet<&str> = trace
                .events
                .iter()
                .filter(|event| event.step > first && event.step <= last)
                .flat_map(|event| event.visible.iter().map(String::as_str))
                .collect();
            let gained: Vec<String> = during
                .difference(&before)
                .map(|name| (*name).to_string())
                .collect();
            let classification = if gained.is_empty() {
                Repetition::Stuck {
                    repeats: steps.len(),
                }
            } else {
                Repetition::IterativeRefinement {
                    evidence_gained: gained,
                }
            };
            RepeatedAction {
                summary,
                steps,
                classification,
            }
        })
        .collect()
}

/// 06.03's decision taxonomy.
///
/// [`DecisionType::Unclassified`] is a real answer, not a fallback bucket. A structural rule that
/// cannot tell tool selection from plan choice should say so; assigning the most common label by
/// default would make the taxonomy's distribution an artefact of the classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    ContextAcquisition,
    EvidenceInterpretation,
    HypothesisUpdate,
    PlanChoice,
    ToolSelection,
    ToolArguments,
    MemoryAccess,
    Delegation,
    Verification,
    Recovery,
    Termination,
    ExternalSideEffect,
    AnswerFormulation,
    Unclassified,
}

impl DecisionType {
    pub fn as_str(self) -> &'static str {
        match self {
            DecisionType::ContextAcquisition => "context_acquisition",
            DecisionType::EvidenceInterpretation => "evidence_interpretation",
            DecisionType::HypothesisUpdate => "hypothesis_update",
            DecisionType::PlanChoice => "plan_choice",
            DecisionType::ToolSelection => "tool_selection",
            DecisionType::ToolArguments => "tool_arguments",
            DecisionType::MemoryAccess => "memory_access",
            DecisionType::Delegation => "delegation",
            DecisionType::Verification => "verification",
            DecisionType::Recovery => "recovery",
            DecisionType::Termination => "termination",
            DecisionType::ExternalSideEffect => "external_side_effect",
            DecisionType::AnswerFormulation => "answer_formulation",
            DecisionType::Unclassified => "unclassified",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [
            DecisionType::ContextAcquisition,
            DecisionType::EvidenceInterpretation,
            DecisionType::HypothesisUpdate,
            DecisionType::PlanChoice,
            DecisionType::ToolSelection,
            DecisionType::ToolArguments,
            DecisionType::MemoryAccess,
            DecisionType::Delegation,
            DecisionType::Verification,
            DecisionType::Recovery,
            DecisionType::Termination,
            DecisionType::ExternalSideEffect,
            DecisionType::AnswerFormulation,
        ]
        .into_iter()
        .find(|candidate| candidate.as_str() == name)
    }
}

/// Whether a step could be walked back, and on what authority that was decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Reversibility {
    /// The producer said so.
    Declared { irreversible: bool },
    /// This crate assumed it from the event kind, because nothing was declared. The blueprint does
    /// not specify a default, so the basis is carried alongside the value.
    Assumed { irreversible: bool, basis: String },
}

impl Reversibility {
    pub fn irreversible(&self) -> bool {
        match self {
            Reversibility::Declared { irreversible } => *irreversible,
            Reversibility::Assumed { irreversible, .. } => *irreversible,
        }
    }
}

/// A ranked decision boundary with its type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Boundary {
    pub step: usize,
    pub summary: String,
    pub decision_type: DecisionType,
    /// What the type was read from: a declaration, or the structural rule that fired.
    pub type_evidence: String,
    pub reversibility: Reversibility,
    /// The rank `bioprism_trace::segment` computed, carried through rather than recomputed.
    pub rank: CandidateScore,
    /// Set when the step is decision-bearing but should not become a standalone cell.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_op_reason: Option<String>,
}

impl Boundary {
    /// Whether a cell may be extracted here.
    pub fn extractable(&self) -> bool {
        self.no_op_reason.is_none()
    }
}

fn describe(event: &Event) -> String {
    event
        .payload
        .get("summary")
        .or_else(|| event.payload.get("tool"))
        .or_else(|| event.payload.get("action"))
        .and_then(|value| value.as_str())
        .unwrap_or("<no summary recorded>")
        .to_string()
}

fn classify(event: &Event) -> (DecisionType, String) {
    if let Some(declared) = event
        .payload
        .get("decision_type")
        .and_then(|value| value.as_str())
        .and_then(DecisionType::parse)
    {
        return (declared, "declared by the trace producer".to_string());
    }

    let alternatives: Vec<&serde_json::Value> = event
        .payload
        .get("alternatives")
        .and_then(|value| value.as_array())
        .map(|items| items.iter().collect())
        .unwrap_or_default();
    let tool = event.payload.get("tool").and_then(|value| value.as_str());

    if event
        .payload
        .get("external")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || event
            .payload
            .get("irreversible")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    {
        return (
            DecisionType::ExternalSideEffect,
            "the payload declares an external or irreversible effect".to_string(),
        );
    }

    if let (Some(tool), false) = (tool, alternatives.is_empty()) {
        let names: BTreeSet<&str> = alternatives
            .iter()
            .filter_map(|item| {
                item.get("tool")
                    .and_then(|value| value.as_str())
                    .or_else(|| item.as_str())
            })
            .collect();
        if names.iter().any(|name| *name != tool) {
            return (
                DecisionType::ToolSelection,
                "recorded alternatives name a different tool".to_string(),
            );
        }
        return (
            DecisionType::ToolArguments,
            "recorded alternatives share the tool and differ in arguments".to_string(),
        );
    }

    (
        DecisionType::Unclassified,
        "no declaration and no structural rule matched; the type is unknown, not defaulted"
            .to_string(),
    )
}

fn reversibility(event: &Event) -> Reversibility {
    match event
        .payload
        .get("irreversible")
        .and_then(|value| value.as_bool())
    {
        Some(irreversible) => Reversibility::Declared { irreversible },
        None => Reversibility::Assumed {
            irreversible: event.kind == EventKind::Action,
            basis: "an action ran a tool and may have moved the world; a choice did not"
                .to_string(),
        },
    }
}

/// 06.03's no-op filter, applied only to steps that are already decision-bearing.
///
/// `bioprism_trace::excluded` covers the other half — steps that can never host a cell because the
/// agent had no alternative. This covers steps where the agent nominally acted but had nothing to
/// weigh: no recorded alternatives, no new evidence, and a direct causal parent that forced it.
fn no_op_reason(event: &Event, score: &CandidateScore) -> Option<String> {
    if score.is_divergence {
        return None;
    }
    if score.alternatives > 0 || score.newly_visible > 0 {
        return None;
    }
    event
        .caused_by
        .map(|parent| format!("no alternatives and no new evidence; forced by step {parent}"))
}

/// Ranks decision boundaries and assigns each a type.
///
/// `divergence_step` is passed straight through to `bioprism_trace::segment`, which weights it. A
/// caller with a first causal divergence should supply it; a caller without one gets structural
/// ranking, and every boundary's `rank.is_divergence` will be false, which is how the weakness is
/// visible rather than hidden.
pub fn boundaries(trace: &Trace, divergence_step: Option<usize>) -> Vec<Boundary> {
    segment(trace, divergence_step)
        .into_iter()
        .filter_map(|candidate| {
            let event = trace.at(candidate.step)?;
            let (decision_type, type_evidence) = classify(event);
            Some(Boundary {
                step: candidate.step,
                summary: candidate.summary,
                decision_type,
                type_evidence,
                reversibility: reversibility(event),
                no_op_reason: no_op_reason(event, &candidate.score),
                rank: candidate.score,
            })
        })
        .collect()
}
