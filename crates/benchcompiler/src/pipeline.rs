//! The assembled compiler.
//!
//! Blueprint 06.01. The pipeline it describes is
//! *segment → identify decisions → rank causal candidates → reconstruct candidates → minimize →
//! synthesize oracle proposals → human or deterministic validation → generate mutations → validate
//! relations → deduplicate → calibrate → exploit-audit → package*. [`compile`] runs the stages this
//! crate owns and stops at the review gate, because 06.01's human-in-the-loop clause is not advice:
//! "Early releases treat compiler models as assistants. Humans approve task intent, visible/hidden
//! boundaries, acceptable outcomes, and publication tier."
//!
//! A [`Compilation`] therefore contains a [`crate::oracle::ProposedOracle`] and never a graded
//! result. [`Compilation::approve`] is the only way to a `bioprism_prism::DecisionCell`, and it is
//! the same gate `bioprism_trace::CellProposal::approve` enforces one layer down — a compilation
//! that skipped review cannot produce a cell, because the type it would need does not exist yet.
//!
//! ## Confidence is decomposed
//!
//! 06.01: confidence is "decomposed by boundary detection, state reconstruction, minimization
//! fidelity, oracle adequacy, and mutation validity—not one opaque number". [`CompilerConfidence`]
//! carries exactly those five and offers **no** method that combines them. There is a
//! [`CompilerConfidence::limiting_stage`], because the useful question is which stage is weakest,
//! and a stage nobody measured is [`StageConfidence::Unmeasured`] rather than a low score.
//!
//! ## What is deliberately not implemented
//!
//! Mutation generation, relation validation and packaging are not here. `bioprism_mutation` owns
//! the first two (06.10–06.12) and a pack registry owns the third; a second mutation engine inside
//! the compiler would produce families the mutation crate's lineage graph does not know about,
//! which is exactly the identifier conflation 06.05's invariants forbid.

use crate::attribute::{failure_card, Assertion, Citation, ConstraintRecord, FailureCard};
use crate::boundary::{boundaries, episodes, Boundary};
use crate::causal::{analyse, CausalAnalysis, CausalVerdict};
use crate::error::CompileError;
use crate::minimize::{minimize, ContextItem, InterestProbe, MinimizeBudget, Minimization};
use crate::oracle::{synthesise, ProposedOracle, ReviewedOracle};
use bioprism_prism::{DecisionCell, InputRef};
use bioprism_trace::Trace;
use serde::{Deserialize, Serialize};

/// 06.01's output classes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "class", rename_all = "snake_case")]
pub enum OutputClass {
    /// Compiled and minimized, useful as a private regression test, not published.
    PrivateRegressionCell,
    /// Everything except review. The highest class this crate can reach on its own.
    CandidateResearchCell,
    /// Reviewed by a named human. Only [`Compilation::approve`] produces one.
    GoldReviewedCell { reviewer: String },
    /// Nothing compilable was found. Carries why, because "no cell" is a finding.
    RejectedOrUnresolved { reason: String },
}

/// One stage's confidence, or the fact that nobody measured it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum StageConfidence {
    Unmeasured { reason: String },
    Measured { value: f64, basis: String },
}

impl StageConfidence {
    fn measured(value: f64, basis: &str) -> Self {
        StageConfidence::Measured {
            value,
            basis: basis.to_string(),
        }
    }

    fn unmeasured(reason: &str) -> Self {
        StageConfidence::Unmeasured {
            reason: reason.to_string(),
        }
    }

    /// The measured value, or `None`. There is deliberately no `value_or_zero`.
    pub fn value(&self) -> Option<f64> {
        match self {
            StageConfidence::Measured { value, .. } => Some(*value),
            StageConfidence::Unmeasured { .. } => None,
        }
    }
}

/// Five stage confidences and no way to average them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerConfidence {
    pub boundary_detection: StageConfidence,
    pub state_reconstruction: StageConfidence,
    pub minimization_fidelity: StageConfidence,
    pub oracle_adequacy: StageConfidence,
    pub mutation_validity: StageConfidence,
}

impl CompilerConfidence {
    fn stages(&self) -> [(&'static str, &StageConfidence); 5] {
        [
            ("boundary_detection", &self.boundary_detection),
            ("state_reconstruction", &self.state_reconstruction),
            ("minimization_fidelity", &self.minimization_fidelity),
            ("oracle_adequacy", &self.oracle_adequacy),
            ("mutation_validity", &self.mutation_validity),
        ]
    }

    /// The weakest *measured* stage. The number a reader should act on.
    pub fn limiting_stage(&self) -> Option<(&'static str, f64)> {
        self.stages()
            .into_iter()
            .filter_map(|(name, stage)| stage.value().map(|value| (name, value)))
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Stages nobody measured. A compilation with several of these is not a confident one, and
    /// collapsing them into an average would hide exactly that.
    pub fn unmeasured_stages(&self) -> Vec<&'static str> {
        self.stages()
            .into_iter()
            .filter(|(_, stage)| stage.value().is_none())
            .map(|(name, _)| name)
            .collect()
    }
}

/// One line of 06.01's evidence-preservation log: an output field and what produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceEntry {
    pub output_field: String,
    pub rule: String,
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compilation {
    pub trace_id: String,
    pub episodes: usize,
    pub boundaries: Vec<Boundary>,
    pub analysis: CausalAnalysis,
    pub card: FailureCard,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimization: Option<Minimization>,
    /// Unreviewed by construction. It cannot grade anything until [`Compilation::approve`] runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle: Option<ProposedOracle>,
    pub class: OutputClass,
    pub confidence: CompilerConfidence,
    pub provenance: Vec<ProvenanceEntry>,
}

impl Compilation {
    /// The step a cell would freeze, when one was found.
    pub fn cell_step(&self) -> Option<usize> {
        self.analysis.first_causal_step()
    }

    /// The review gate. The only path from a compilation to a `DecisionCell`.
    ///
    /// Delegates every refusal to [`crate::oracle::ProposedOracle::review`], so an unattributed
    /// reviewer, a missing gap analysis and an unrebutted exploit block a cell here for the same
    /// reasons they block an oracle there. A compilation with no oracle cannot be approved at all.
    pub fn approve(
        self,
        reviewer: &str,
        world: InputRef,
        query: InputRef,
    ) -> Result<(DecisionCell, ReviewedOracle), CompileError> {
        let proposal = self.oracle.ok_or_else(|| CompileError::NotCompilable {
            trace_id: self.trace_id.clone(),
            reason: "no oracle was synthesized, so there is nothing to review".to_string(),
        })?;
        let step = self
            .analysis
            .first_causal_step()
            .ok_or_else(|| CompileError::NotCompilable {
                trace_id: self.trace_id.clone(),
                reason: "the causal analysis declined to localize a decision".to_string(),
            })?;
        let reviewed = proposal.review(reviewer)?;
        let cell_id = format!("dc_{}#step{}", self.trace_id, step);
        let cell = reviewed.clone().into_cell(cell_id, world, query);
        Ok((cell, reviewed))
    }
}

/// Runs every stage this crate owns, stopping at the review gate.
///
/// `context` and `probe` drive minimization. Supplying an empty context skips it, and the
/// compilation says so through an [`StageConfidence::Unmeasured`] minimization fidelity rather than
/// reporting a reduction that never happened.
pub fn compile<P: InterestProbe>(
    failing: &Trace,
    reference: Option<&Trace>,
    context: &[ContextItem],
    probe: &mut P,
    budget: MinimizeBudget,
    ledger: &[ConstraintRecord],
    claims: Vec<Assertion>,
) -> Result<Compilation, CompileError> {
    let analysis = analyse(failing, reference)?;
    let episodes = episodes(failing).len();
    let boundaries = boundaries(failing, analysis.first_causal_step());
    let card = failure_card(&analysis, ledger, None, claims);

    let minimization = if context.is_empty() {
        None
    } else {
        Some(minimize(context, probe, budget)?)
    };

    let oracle = match (&analysis.verdict, &minimization) {
        (CausalVerdict::FirstCausal { step, .. }, Some(minimization)) => Some(synthesise(
            format!("or_{}#step{}", failing.trace_id, step),
            format!(
                "step {step}: {}",
                boundaries
                    .iter()
                    .find(|boundary| boundary.step == *step)
                    .map(|boundary| boundary.summary.clone())
                    .unwrap_or_else(|| "<no summary recorded>".to_string())
            ),
            minimization,
        )),
        _ => None,
    };

    let class = match (&analysis.verdict, &oracle) {
        (CausalVerdict::FirstCausal { .. }, Some(_)) => OutputClass::CandidateResearchCell,
        (CausalVerdict::FirstCausal { .. }, None) => OutputClass::PrivateRegressionCell,
        (CausalVerdict::Conjunction { .. }, _) => OutputClass::PrivateRegressionCell,
        (verdict, _) => OutputClass::RejectedOrUnresolved {
            reason: match verdict {
                CausalVerdict::EnvironmentDivergence { at_step, kind, .. } => format!(
                    "the runs diverged at step {at_step}, which is a {kind}: the agent did not \
                     control it"
                ),
                CausalVerdict::NoDivergence => {
                    "the runs never differed on anything the trace records".to_string()
                }
                CausalVerdict::Unlocalizable { reason } => reason.clone(),
                _ => "unreachable".to_string(),
            },
        },
    };

    let confidence = CompilerConfidence {
        boundary_detection: match boundaries.first() {
            Some(boundary) => StageConfidence::measured(
                boundary.rank.total.min(1.0),
                "transparent rank from bioprism_trace::segment; never validated against expert \
                 boundary annotations",
            ),
            None => StageConfidence::unmeasured("no decision boundary was proposed"),
        },
        state_reconstruction: StageConfidence::unmeasured(
            "no replay was performed; this crate cannot execute a world, so it cannot check that \
             the frozen state reconstructs",
        ),
        minimization_fidelity: match &minimization {
            Some(_) => StageConfidence::measured(
                1.0,
                "preservation is proven by the verification pass rather than estimated: every \
                 remaining unit has a recorded probe showing the signature does not survive its \
                 removal",
            ),
            None => StageConfidence::unmeasured("no context was supplied to minimize"),
        },
        oracle_adequacy: StageConfidence::unmeasured(
            "no adversarial attempt was recorded against the proposed oracle; 06.08 requires \
             exploit generation, which this crate does not perform",
        ),
        mutation_validity: StageConfidence::unmeasured(
            "no mutations were generated; that is bioprism_mutation's contract (06.10-06.12)",
        ),
    };

    let mut provenance = vec![
        ProvenanceEntry {
            output_field: "analysis.verdict".to_string(),
            rule: "first causal divergence over the backward dependency graph, gated by \
                   bioprism_trace::is_actionable"
                .to_string(),
            citations: analysis
                .candidates
                .iter()
                .map(|candidate| Citation::Event {
                    step: candidate.step,
                })
                .collect(),
        },
        ProvenanceEntry {
            output_field: "card.blame".to_string(),
            rule: "task defect outranks evaluator dispute outranks the causal verdict".to_string(),
            citations: vec![Citation::Event {
                step: analysis.terminal_step,
            }],
        },
    ];
    if let Some(minimization) = &minimization {
        provenance.push(ProvenanceEntry {
            output_field: "minimization.minimal".to_string(),
            rule: format!(
                "delta debugging to a fixpoint over {} probe(s), preserving {}",
                minimization.evaluations,
                minimization.preserved.describe()
            ),
            citations: vec![Citation::StateDiff {
                description: format!(
                    "{} of {} context item(s) removed",
                    minimization.removed.len(),
                    minimization.started_from
                ),
            }],
        });
    }

    Ok(Compilation {
        trace_id: failing.trace_id.clone(),
        episodes,
        boundaries,
        analysis,
        card,
        minimization,
        oracle,
        class,
        confidence,
        provenance,
    })
}
