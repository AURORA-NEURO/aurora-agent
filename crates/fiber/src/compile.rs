//! The compiler pipeline.
//!
//! Blueprint 43.16 stages the compiler as `q → QIR → PCIR → SIR → LIR → AIR → PIR → render`, with
//! each pass replayable and each emitting receipts. The v0.1 engine implements the passes that
//! the wire schema can express — protected closure, dependency slice, temporal cut, plan
//! selection, render — and records the ones it cannot in [`CompileTrace::deferred_passes`]
//! rather than pretending they ran.
//!
//! Pass order is normative, not incidental: closure is computed *before* slicing so that
//! protected evidence enters the selection whether or not a dependency path reaches it, and the
//! policy screen runs *after* both so that a collision between "mandatory" and "forbidden" is
//! visible rather than pre-filtered away. [`crate::policy::screen`] carries that argument in full.
//!
//! The one gate that runs ahead of every pass is [`crate::policy::PolicyEnvelope::resolve`]. It
//! needs no evidence — one `data_policy` lookup — so a query whose declared clauses conflict with
//! the corpus is refused before any closure, slice or materialisation happens. It emits no pass
//! receipt because it selects nothing; it is an admission check on the query-world pair, in the
//! same family as the schema-version check in [`Query::from_json`].

use crate::closure::{dropped_protected, protected_closure, unmatched_tags};
use crate::error::FiberError;
use crate::influence::{self, WithheldSplit};
use crate::oracle;
use crate::plan::{self, PlanEvaluation};
use crate::policy::{self, PolicyEnvelope, PolicyOutcome, PolicyScreen, POLICY_REFINEMENT_ACTION};
use crate::qir::Query;
use crate::slice::{backward_slice, max_selected_arity};
use crate::temporal::{temporal_cut, TemporalCut};
use bioprism_epistemic::{
    adaptive::AdaptivePolicy,
    adaptive_policy as epistemic_adaptive_policy, decision_equivalence_quotient,
    ratedistortion::{
        frontier as epistemic_frontier, identification as epistemic_identification,
        AbstentionReason, DistortionCriterion, Frontier, Identification, Sufficiency,
    },
    DecisionEquivalenceQuotient,
};
use bioprism_ids::ContentHash;
use bioprism_influence::summarise;
use bioprism_section::{
    ContextCertificate, DecisionSection, EvidenceCapsule, InfluenceClass, OmissionGroup,
    OmissionManifest, ReferenceOmissions, RefinementOption, SourceHashes, UnresolvedObligation,
};
use bioprism_world::{Fact, WorldSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const REFERENCE_LIMITATION: &str = "Reference slicer uses dependency reachability and protected tags; it does not yet implement sheaf cohomology, FAQ-width optimization, abstract interpretation, or formal influence bounds.";
const OMISSION_CLASSIFICATION: &str = "no_backward_dependency_path_or_temporally_inaccessible";
const RETROSPECTIVE_ACTION: &str = "advance_time_cut_or_use_retrospective_mode";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassReceipt {
    pub name: &'static str,
    pub retained: usize,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct CompileTrace {
    pub passes: Vec<PassReceipt>,
    /// Passes the v0.1 wire schema cannot support, with the reason.
    pub deferred_passes: Vec<(&'static str, &'static str)>,
    /// Protected tags no fact in the world carries.
    pub unmatched_protected_tags: Vec<String>,
    /// Protected facts the temporal cut removed. Non-empty means the mandatory closure was not
    /// delivered and the consumer must refine or abstain.
    ///
    /// Policy never contributes to this list: a policy exclusion inside the closure is refused
    /// outright, so a compile that returns at all has a closure policy left intact.
    pub dropped_protected: Vec<String>,
    pub temporal_cut: TemporalCut,
    /// What the policy screen held and what it withheld (43.33).
    pub policy: PolicyOutcome,
    /// What the physical backend portfolio said about the compiled region (43.36, 43.37).
    ///
    /// Off the wire for the reason [`crate::plan`] argues at length: the portfolio costs a
    /// sum-product evaluation the compiler did not perform and, on any world `fiber-world/0.1` can
    /// state, could not have performed. A certificate naming that plan would name an engine that
    /// never ran.
    pub plan: PlanEvaluation,
    /// Influence bounds on the temporally withheld facts, and the split they license (43.28).
    pub withheld_influence: WithheldSplit,
    /// The exact 43.10 quotient when the query supplied a `fiber-query/0.3` decision contract.
    /// `None` is meaningful: older wire versions remain executable but cannot claim this pass.
    pub decision_quotient: Option<DecisionEquivalenceQuotient>,
    /// The exhaustive rate-distortion audit when a `fiber-query/0.4` observed-evidence contract
    /// was supplied. `None` is meaningful: older queries cannot make a context-minimality claim.
    pub rate_distortion: Option<RateDistortionTrace>,
    /// The exact finite-horizon adaptive acquisition policy when a `fiber-query/0.5` contract was
    /// supplied. This is a plan projection only; it carries no execution receipt or authority.
    pub adaptive_acquisition: Option<AdaptiveAcquisitionTrace>,
}

/// The full bounded rate-distortion result projected into a compile trace.
///
/// The frontier is retained alongside identification and sufficiency because they answer
/// different questions: model disagreement, whether any context meets tolerance, and the
/// cheapest such context. A compact summary may omit points at the transport boundary, but the
/// compiler keeps the exact kernel result available to explain/replay callers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateDistortionTrace {
    pub criterion: DistortionCriterion,
    pub tolerance: f64,
    pub compatibility_floor: f64,
    pub evidence_count: usize,
    pub full_rate: f64,
    pub identification: Identification,
    pub sufficiency: Sufficiency,
    pub frontier: Frontier,
}

/// The bounded adaptive policy plus the exact caller inputs needed to replay its objective.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveAcquisitionTrace {
    pub budget: f64,
    pub max_steps: usize,
    pub prior: Vec<f64>,
    pub problem: bioprism_epistemic::DecisionProblem,
    pub acquisitions: Vec<bioprism_epistemic::Acquisition>,
    pub policy: AdaptivePolicy,
}

impl AdaptiveAcquisitionTrace {
    /// Rebinds the compiler's policy projection to the execution-layer contract.
    ///
    /// Compilation remains side-effect free: this method only revalidates the exact policy and
    /// returns a plan that a caller may later execute with an explicit provider grant. Keeping the
    /// conversion here prevents SDK and MCP callers from reconstructing the prior, acquisitions,
    /// and policy independently (which would weaken the compiler-to-executor digest boundary).
    pub fn execution_plan(
        &self,
    ) -> Result<bioprism_epistemic::AdaptivePlan, bioprism_epistemic::AdaptiveExecutionError> {
        let belief = bioprism_epistemic::Belief::new(self.prior.clone()).map_err(|error| {
            bioprism_epistemic::AdaptiveExecutionError::InvalidPlan(error.to_string())
        })?;
        bioprism_epistemic::AdaptivePlan::from_policy(
            self.problem.clone(),
            belief,
            self.acquisitions.clone(),
            self.budget,
            self.max_steps,
            self.policy.clone(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub section: DecisionSection,
    pub certificate: ContextCertificate,
    pub trace: CompileTrace,
}

impl CompileOutput {
    /// Whether every protected fact survived into the delivered section.
    pub fn protected_closure_satisfied(&self) -> bool {
        self.trace.dropped_protected.is_empty()
    }

    /// Whether the policy screen released every candidate the slice and closure asked for.
    pub fn policy_released_everything(&self) -> bool {
        self.trace.policy.released_everything_requested()
    }
}

fn execute_rate_distortion(
    contract: &crate::qir::RateDistortionContract,
    problem: &bioprism_epistemic::DecisionProblem,
    tolerance: f64,
) -> Result<RateDistortionTrace, FiberError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(FiberError::InvalidRateDistortionContract(
            "distortion_tolerance must be finite and non-negative".into(),
        ));
    }
    let identification = epistemic_identification(
        problem,
        &contract.prior,
        &contract.evidence_pool,
        tolerance,
        contract.compatibility_floor,
    )
    .map_err(|error| FiberError::InvalidRateDistortionContract(error.to_string()))?;
    let frontier = epistemic_frontier(
        problem,
        &contract.prior,
        &contract.evidence_pool,
        contract.criterion,
        contract.compatibility_floor,
    )
    .map_err(|error| FiberError::InvalidRateDistortionContract(error.to_string()))?;
    let full_rate = contract
        .evidence_pool
        .rate(&contract.evidence_pool.everything())
        .map_err(|error| FiberError::InvalidRateDistortionContract(error.to_string()))?;

    let sufficiency = if contract.criterion == DistortionCriterion::MinimaxRegret
        && matches!(&identification, Identification::NonIdentified { .. })
    {
        let best_distortion = frontier
            .points
            .iter()
            .map(|point| point.distortion)
            .fold(f64::INFINITY, f64::min);
        Sufficiency::Abstain {
            reason: AbstentionReason::NonIdentifiedUnderAllEvidence,
            best_distortion,
            tolerance,
        }
    } else if let Some(point) = frontier.cheapest_within(tolerance) {
        Sufficiency::Sufficient {
            retained: point.retained.clone(),
            rate: point.rate,
            distortion: point.distortion,
            full_rate,
        }
    } else {
        let best_distortion = frontier
            .points
            .iter()
            .map(|point| point.distortion)
            .fold(f64::INFINITY, f64::min);
        Sufficiency::Abstain {
            reason: AbstentionReason::ToleranceUnattainable,
            best_distortion,
            tolerance,
        }
    };

    Ok(RateDistortionTrace {
        criterion: contract.criterion,
        tolerance,
        compatibility_floor: contract.compatibility_floor,
        evidence_count: contract.evidence_pool.len(),
        full_rate,
        identification,
        sufficiency,
        frontier,
    })
}

fn execute_adaptive_acquisition(
    contract: &crate::qir::AdaptiveAcquisitionContract,
    problem: &bioprism_epistemic::DecisionProblem,
) -> Result<AdaptiveAcquisitionTrace, FiberError> {
    let policy = epistemic_adaptive_policy(
        problem,
        &contract.prior,
        &contract.acquisitions,
        contract.budget,
        contract.max_steps,
    )
    .map_err(|error| FiberError::InvalidAdaptiveAcquisitionContract(error.to_string()))?;
    Ok(AdaptiveAcquisitionTrace {
        budget: contract.budget,
        max_steps: contract.max_steps,
        prior: contract.prior.masses().to_vec(),
        problem: problem.clone(),
        acquisitions: contract.acquisitions.clone(),
        policy,
    })
}

pub fn compile<S: WorldSource + ?Sized>(
    source: &S,
    query: &Query,
) -> Result<CompileOutput, FiberError> {
    let mut passes = Vec::new();

    let decision_quotient = query
        .decision_contract
        .as_ref()
        .map(|contract| {
            decision_equivalence_quotient(&contract.problem, &contract.permitted_actions)
                .map_err(|error| FiberError::InvalidDecisionContract(error.to_string()))
        })
        .transpose()?;
    if let Some(quotient) = &decision_quotient {
        passes.push(PassReceipt {
            name: "decision_quotient",
            retained: quotient.quotient_model_count,
            note: format!(
                "{} model(s) reduced to {} decision-equivalence class(es); {} merged",
                quotient.original_model_count,
                quotient.quotient_model_count,
                quotient.merged_model_count
            ),
        });
    }

    let rate_distortion = query
        .rate_distortion
        .as_ref()
        .map(|contract| {
            let problem = &query
                .decision_contract
                .as_ref()
                .ok_or(FiberError::InvalidRateDistortionContract(
                    "rate-distortion requires the decision contract".into(),
                ))?
                .problem;
            let tolerance =
                query
                    .distortion_tolerance
                    .ok_or(FiberError::InvalidRateDistortionContract(
                        "rate-distortion requires distortion_tolerance".into(),
                    ))?;
            execute_rate_distortion(contract, problem, tolerance)
        })
        .transpose()?;
    if let Some(report) = &rate_distortion {
        let retained = match &report.sufficiency {
            Sufficiency::Sufficient { retained, .. } => retained.len(),
            Sufficiency::Abstain { .. } => 0,
        };
        passes.push(PassReceipt {
            name: "rate_distortion",
            retained,
            note: format!(
                "{} observed evidence item(s), {} exhaustive contexts evaluated under {}",
                report.evidence_count,
                report.frontier.evaluated,
                match report.criterion {
                    DistortionCriterion::BayesRegret => "Bayes regret",
                    DistortionCriterion::MinimaxRegret => "minimax regret",
                }
            ),
        });
    }

    let adaptive_acquisition = query
        .adaptive_acquisition
        .as_ref()
        .map(|contract| {
            let problem = &query
                .decision_contract
                .as_ref()
                .ok_or(FiberError::InvalidAdaptiveAcquisitionContract(
                    "adaptive acquisition requires the decision contract".into(),
                ))?
                .problem;
            execute_adaptive_acquisition(contract, problem)
        })
        .transpose()?;
    if let Some(report) = &adaptive_acquisition {
        passes.push(PassReceipt {
            name: "adaptive_acquisition",
            retained: report.policy.selected_depth,
            note: format!(
                "exact policy under budget {} and {}-step horizon; {} state node(s) evaluated",
                report.budget, report.max_steps, report.policy.nodes_evaluated
            ),
        });
    }

    let envelope = PolicyEnvelope::resolve(source, query)?;

    let protected = protected_closure(source, &query.protected_tags);
    passes.push(PassReceipt {
        name: "protected_closure",
        retained: protected.len(),
        note: format!("{} protected tags requested", query.protected_tags.len()),
    });

    let slice = backward_slice(source, query.targets.iter().map(|t| t.as_str()));
    passes.push(PassReceipt {
        name: "backward_slice",
        retained: slice.selected_factors.len(),
        note: format!(
            "{} variables reachable from targets",
            slice.needed_variables.len()
        ),
    });

    let mut selected_facts: BTreeSet<String> = slice
        .needed_variables
        .iter()
        .filter_map(|variable| source.fact_providing(variable))
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    selected_facts.extend(protected.iter().cloned());

    // Materialise only the selected region. Everything downstream reads from this vector, so
    // compile cost tracks the compiled region rather than the corpus (43.34).
    let mut resolved: BTreeMap<String, Fact> = BTreeMap::new();
    for id in &selected_facts {
        if let Some(fact) = source.fact(id) {
            resolved.insert(id.clone(), fact);
        }
    }

    let screen = policy::screen(&envelope, &resolved, &protected)?;
    let withheld_by_policy = screen.withheld_ids();
    for id in &withheld_by_policy {
        selected_facts.remove(id);
    }
    passes.push(PassReceipt {
        name: "policy",
        retained: selected_facts.len(),
        note: format!(
            "{} clause(s) in force, {} candidate(s) declared a requirement, {} withheld",
            envelope.in_force().len(),
            screen.requirements_seen(),
            withheld_by_policy.len()
        ),
    });

    let cut = temporal_cut(source, query.decision_time);
    let inaccessible: Vec<String> = selected_facts
        .iter()
        .filter(|id| {
            resolved
                .get(id.as_str())
                .is_some_and(|fact| !cut.is_accessible(fact.provides.as_str()))
        })
        .cloned()
        .collect();
    for id in &inaccessible {
        selected_facts.remove(id);
    }
    passes.push(PassReceipt {
        name: "temporal_cut",
        retained: selected_facts.len(),
        note: format!("{} facts withheld at the decision cut", inaccessible.len()),
    });

    if selected_facts.len() > query.budgets.max_facts {
        return Err(FiberError::BudgetExceeded {
            selected: selected_facts.len(),
            max_facts: query.budgets.max_facts,
        });
    }

    let ordered_facts: Vec<&Fact> = selected_facts
        .iter()
        .filter_map(|id| resolved.get(id.as_str()))
        .collect();

    let values: BTreeMap<String, Value> = ordered_facts
        .iter()
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    let verdict = oracle::evaluate(&values)?;
    passes.push(PassReceipt {
        name: "oracle",
        retained: verdict.witnesses.len(),
        note: format!("status {}", verdict.status.as_str()),
    });

    let selected_evidence: Vec<EvidenceCapsule> = ordered_facts
        .iter()
        .map(|fact| EvidenceCapsule::from_raw_fact(fact.raw()))
        .collect();
    let selected_factor_docs: Vec<Value> = slice
        .selected_factors
        .iter()
        .filter_map(|id| source.factor(id))
        .map(|factor| factor.raw().clone())
        .collect();

    // Obligations and refinements are listed in pass order, so a reader walking the section meets
    // the exclusions in the order the compiler decided them.
    let mut unresolved: Vec<UnresolvedObligation> = withheld_by_policy
        .iter()
        .map(|id| UnresolvedObligation::PolicyBlocked {
            detail: PolicyScreen::obligation_detail(
                id,
                screen
                    .missing_for(id)
                    .expect("withheld ids carry their clauses"),
            ),
        })
        .collect();
    unresolved.extend(
        inaccessible
            .iter()
            .map(|id| UnresolvedObligation::InaccessibleAtCut {
                fact_id: id.clone(),
            }),
    );

    let mut frontier = Vec::new();
    if !withheld_by_policy.is_empty() {
        frontier.push(RefinementOption {
            action: POLICY_REFINEMENT_ACTION.into(),
            facts: withheld_by_policy.clone(),
        });
    }
    if !inaccessible.is_empty() {
        frontier.push(RefinementOption {
            action: RETROSPECTIVE_ACTION.into(),
            facts: inaccessible.clone(),
        });
    }

    let section = DecisionSection {
        world_id: source.world_id().to_string(),
        query_id: query.query_id.as_str().to_string(),
        decision_time: query.decision_time_raw.clone(),
        goal: query.goal_text().to_string(),
        selected_evidence,
        selected_factors: selected_factor_docs,
        oracle: verdict.clone(),
        unresolved_obligations: unresolved,
        refinement_frontier: frontier,
    };

    // Counted, never enumerated: the omitted set is the corpus minus the selection, and
    // materialising it would reintroduce the very whole-world traversal the design rejects.
    let omitted_total = source.total_facts().saturating_sub(selected_facts.len());
    let selected_exploratory = ordered_facts
        .iter()
        .filter(|fact| fact.has_tag("exploratory"))
        .count();
    let omitted_exploratory = source
        .count_with_tag("exploratory")
        .saturating_sub(selected_exploratory);

    let region = plan::compile_region(
        source,
        query.query_id.as_str(),
        query.targets.iter().map(|t| t.as_str()),
    );
    let evaluation = plan::evaluate(region.as_ref());
    let plan = evaluation.descriptor(
        slice.selected_factors.len(),
        selected_facts.len(),
        source.total_factors(),
        source.total_facts(),
        max_selected_arity(source, &slice.selected_factors),
    );
    passes.push(PassReceipt {
        name: "plan_selection",
        retained: plan.compiled_fact_count,
        note: evaluation.receipt_note(plan.fact_selection_ratio()),
    });

    let withheld_influence =
        influence::split_withheld(source, region.as_ref().ok(), &inaccessible, &cut);
    let manifest = build_manifest(
        omitted_total,
        &withheld_influence,
        &withheld_by_policy,
        omitted_exploratory,
    );
    let bounded = summarise(manifest.groups.iter());
    passes.push(PassReceipt {
        name: "influence_bounds",
        retained: bounded.bounded_groups,
        note: format!(
            "{} of {} withheld fact(s) bounded, {} group(s) informative, worst informative bound {}",
            withheld_influence.promoted(),
            inaccessible.len(),
            bounded.informative_groups,
            bounded
                .worst_informative_bound
                .map_or_else(|| "none".to_string(), |value| format!("{value}"))
        ),
    });

    let certificate = ContextCertificate {
        world_id: source.world_id().to_string(),
        query_id: query.query_id.as_str().to_string(),
        selected_facts: selected_facts.iter().cloned().collect(),
        selected_factors: slice.selected_factors.iter().cloned().collect(),
        protected_closure: protected.iter().cloned().collect(),
        omissions: ReferenceOmissions {
            total_facts: omitted_total,
            exploratory_facts: omitted_exploratory,
            classification: OMISSION_CLASSIFICATION.into(),
            inaccessible_selected_before_cut: inaccessible.clone(),
        },
        plan,
        oracle: verdict,
        source_hashes: SourceHashes {
            world_sha256: source.world_digest().as_str().to_string(),
            query_sha256: ContentHash::of_value(query.raw())
                .expect("query parsed from finite JSON")
                .as_str()
                .to_string(),
            decision_section_sha256: section
                .content_hash()
                .expect("section built from finite JSON")
                .as_str()
                .to_string(),
        },
        limitations: vec![REFERENCE_LIMITATION.into()],
        manifest,
    };

    Ok(CompileOutput {
        section,
        certificate,
        trace: CompileTrace {
            passes,
            deferred_passes: deferred_passes(query),
            unmatched_protected_tags: unmatched_tags(source, &query.protected_tags),
            dropped_protected: dropped_protected(&protected, &selected_facts),
            temporal_cut: cut,
            policy: PolicyOutcome::new(&envelope, &screen),
            plan: evaluation,
            withheld_influence,
            decision_quotient,
            rate_distortion,
            adaptive_acquisition,
        },
    })
}

/// Groups omissions by structural reason and assigns each an influence class.
///
/// Facts with no backward dependency path are classed [`InfluenceClass::Zero`] *conditional on
/// the declared factor graph being complete* — the reason string states that assumption, because
/// an incomplete factor graph would turn a zero-influence claim into an unknown-influence one.
/// Temporally withheld facts are [`InfluenceClass::DeferredAcquisition`], never zero: they might
/// well change the decision, they are simply not readable yet.
///
/// Policy-withheld facts get [`InfluenceClass::InaccessibleByPolicy`] and are counted out of the
/// structural group before it is formed. The three classes are kept apart deliberately. Zero says
/// the omission provably cannot matter; deferred says it may matter and will be readable later;
/// policy says it may matter and no amount of waiting will produce it. Folding policy into
/// deferred would promise a retry that cannot succeed, and folding it into zero would assert a
/// bound nobody computed. Because the class does not support a sufficiency claim, one withheld
/// fact is enough to make [`OmissionManifest::supports_sufficiency_claim`] false — which is the
/// honest reading, since the oracle then ran on a value map missing evidence it asked for.
///
/// The withheld population arrives already split by [`crate::influence`] into a bounded part and a
/// still-deferred part, and both are emitted: a withheld fact whose influence is bounded is *both*
/// bounded and deferred, and this shape has one class per group. The refinement frontier is built
/// from the unsplit list, so a promoted member keeps its entry there.
fn build_manifest(
    omitted_total: usize,
    withheld: &WithheldSplit,
    withheld_by_policy: &[String],
    exploratory: usize,
) -> OmissionManifest {
    let mut manifest = OmissionManifest::default();
    let unreachable = omitted_total
        .saturating_sub(withheld.bounded.len())
        .saturating_sub(withheld.deferred.len())
        .saturating_sub(withheld_by_policy.len());

    if unreachable > 0 {
        manifest.push(OmissionGroup {
            reason: "no backward dependency path to any target under the declared factor graph"
                .into(),
            influence: InfluenceClass::Zero,
            count: unreachable,
            bound: Some(0.0),
            examples: Vec::new(),
        });
    }
    if !withheld_by_policy.is_empty() {
        manifest.push(OmissionGroup {
            reason: "withheld by the world's data policy: the query does not hold the clauses the fact requires"
                .into(),
            influence: InfluenceClass::InaccessibleByPolicy,
            count: withheld_by_policy.len(),
            bound: None,
            examples: withheld_by_policy.iter().take(3).cloned().collect(),
        });
    }
    if let Some(group) = withheld.bounded_group() {
        manifest.push(group);
    }
    if !withheld.deferred.is_empty() {
        manifest.push(OmissionGroup {
            reason: "governed by an event not yet available at the decision cut".into(),
            influence: InfluenceClass::DeferredAcquisition,
            count: withheld.deferred.len(),
            bound: None,
            examples: withheld.deferred.iter().take(3).cloned().collect(),
        });
    }
    let _ = exploratory;
    manifest
}

/// Passes the wire formats cannot support, each with the field that is missing.
///
/// The two policy entries are the half of 43.33 [`crate::policy`] enforces nothing of. Declaring
/// them here rather than only in prose is the difference between a consumer being able to check
/// that role filtering did not happen and having to read the source to find out.
fn deferred_passes(query: &Query) -> Vec<(&'static str, &'static str)> {
    let mut deferred = vec![
        (
            "obstruction_tests",
            "43.06 gluing requires a declared cover; fiber-world/0.1 carries no cover",
        ),
        (
            "abstract_interpretation",
            "43.11 requires an abstract-domain registry absent from fiber-world/0.1",
        ),
        (
            "role_and_purpose_filter",
            "43.33 binds role and purpose to the query before selection; fiber-query/0.2 carries a role string and no purpose, and no pass reads either",
        ),
        (
            "information_flow_export",
            "43.33 orders outputs by policy label; fiber-world/0.1 declares no labels and no rules attached at scopes, so only read access is decided",
        ),
    ];
    if !query.has_decision_contract() {
        deferred.push((
            "decision_quotient",
            "43.10 is defined relative to permitted actions and decision loss; fiber-query/0.1 and fiber-query/0.2 do not carry the executable contract",
        ));
    }
    if !query.has_rate_distortion_contract() {
        deferred.push((
            "rate_distortion",
            "43.12 requires a normalized model prior, an ordered observed evidence-pool likelihood binding, a compatible-model floor and a distortion tolerance; fiber-query/0.1 through fiber-query/0.3 carry no complete binding",
        ));
    }
    if !query.has_adaptive_acquisition_contract() {
        deferred.push((
            "adaptive_acquisition",
            "43.15 requires an explicit model prior, outcome likelihood partitions, scalarized budget and finite horizon; fiber-query/0.1 through fiber-query/0.4 carry no complete binding",
        ));
    }
    deferred
}
