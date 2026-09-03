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
use bioprism_backends::{QueryRegion, RegionError};
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
    ContextCertificate, DecisionSection, EvidenceCapsule, InfluenceClass, OmissionAccountingError,
    OmissionGroup, OmissionManifest, ProvenUnreachable, ReferenceOmissions, RefinementOption,
    SourceHashes, UnresolvedObligation,
};
use bioprism_world::{Fact, WorldSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const REFERENCE_LIMITATION: &str = "Reference slicer uses dependency reachability and protected tags; it does not yet implement sheaf cohomology, FAQ-width optimization, abstract interpretation, or formal influence bounds.";

/// The frozen `fiber-context-certificate/0.1` classification string.
///
/// A constant, and *not* a computed classification of this compile's omissions. The v0.1 wire
/// format gives the omitted population one count and one string, and one string cannot name the
/// several structural reasons a fact can be omitted for. It is emitted verbatim by all three parity
/// implementations, so it is a schema literal in the same family as `schema_version`; changing it
/// would move the reference digest and is a version bump, not an edit.
///
/// It is also, read as a classification, incomplete: a fact shadowed by a later fact providing the
/// same variable *does* have a backward dependency path and *is* accessible at the cut, and this
/// string names neither case. [`CertificateProfile::Extended`]'s manifest is where the omitted
/// population is actually classified, per group and per class, and where a shadowed omission is
/// separated from a proven-irrelevant one. A consumer that needs the distinction must read the
/// manifest; a consumer reading only this field has a count and a label, which is what
/// `bioprism_section::certificate` says about the v0.1 shape in its own documentation.
const OMISSION_CLASSIFICATION: &str = "no_backward_dependency_path_or_temporally_inaccessible";
const RETROSPECTIVE_ACTION: &str = "advance_time_cut_or_use_retrospective_mode";

/// How many members the region-carried group's `OmissionGroup::examples` names.
///
/// Named rather than written at each use because that group renders the same members twice — once
/// as identifiers in `examples` and once as the sites its reason string names — and two literals
/// would let the two lists drift onto different facts as soon as the group outgrew them. The other
/// groups render their members once and take their own three.
const EXAMPLES_SHOWN: usize = 3;

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
    /// Why the omitted remainder carries no zero-influence proof, when it carries none.
    ///
    /// `None` is the ordinary case and means the accounting balanced: either a proven group is on
    /// the manifest or every omission was classified elsewhere. `Some` means the compiler declined
    /// to mint the proof and says which check declined it.
    pub unproven_remainder: Option<UnprovenRemainder>,
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
    compile_with_oracle(source, query, &oracle::SplitIntegrityOracle)
}

/// The same pipeline, judged by a caller-supplied oracle.
///
/// Every pass before and after the oracle is identical to [`compile`]; only the verdict — and
/// therefore the certificate bytes that carry it — depends on the oracle. [`compile`] fixes the
/// oracle to [`oracle::SplitIntegrityOracle`], which is what the CPython parity contract pins;
/// this entry point exists because a world whose decision the reference oracle does not know
/// would otherwise compile to `valid` with an empty witness list and read as clean rather than
/// as unjudged.
pub fn compile_with_oracle<S: WorldSource + ?Sized>(
    source: &S,
    query: &Query,
    decision_oracle: &dyn oracle::DecisionOracle,
) -> Result<CompileOutput, FiberError> {
    let mut passes = Vec::new();

    // Admission must precede every analytical pass.  In particular, a conflicting policy
    // declaration is an authorization failure, not a malformed rate-distortion or acquisition
    // request; resolving it first keeps the observable error deterministic and prevents any
    // downstream pass from inspecting evidence the caller was never allowed to use.
    let envelope = PolicyEnvelope::resolve(source, query)?;

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
    let verdict = decision_oracle.evaluate(&values)?;
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

    // Built before the omission classification rather than after it, because the classification
    // reads this region's factor scopes: scope membership is what decides whether an omitted fact
    // has an image the region could perturb, and deriving factor scopes a second time here would
    // let the compiler's answer and the region's drift apart on exactly the multi-output shape
    // where they differ.
    let region = plan::compile_region(
        source,
        query.query_id.as_str(),
        query.targets.iter().map(|t| t.as_str()),
    );

    // Counted, never enumerated: the omitted set is the corpus minus the selection, and
    // materialising it would reintroduce the very whole-world traversal the design rejects.
    let omitted_total = source.total_facts().saturating_sub(selected_facts.len());
    let reachable_but_unselected = reachable_but_unselected(
        source,
        region.as_ref(),
        &slice.needed_variables,
        &selected_facts,
        &withheld_by_policy,
        &inaccessible,
    );
    let selected_exploratory = ordered_facts
        .iter()
        .filter(|fact| fact.has_tag("exploratory"))
        .count();
    let omitted_exploratory = source
        .count_with_tag("exploratory")
        .saturating_sub(selected_exploratory);

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
    let (manifest, unproven_remainder) = build_manifest(
        omitted_total,
        &withheld_influence,
        &withheld_by_policy,
        &reachable_but_unselected,
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
            unproven_remainder,
            decision_quotient,
            rate_distortion,
            adaptive_acquisition,
        },
    })
}

/// Omitted facts that the compiled region still reaches, and that nothing else accounts for.
///
/// The structural predicate behind [`InfluenceClass::Zero`], in the two relations that defeat it.
///
/// The first is the backward slice. A fact whose variable *is* in `needed` has a dependency path
/// to a target, so its omission is proved to be irrelevant by nothing at all.
///
/// The second is scope membership, and it is the relation that "not in `needed`" fails to
/// capture. [`crate::slice::backward_slice`] admits a variable only when a selected factor
/// *consumes* it, so a factor's sibling outputs never enter `needed` — while
/// [`bioprism_backends::QueryRegion::from_world_slice`] puts inputs and outputs alike into that
/// factor's scope. A fact providing a sibling output is therefore absent from `needed` and present
/// in the compiled region, and [`crate::influence`] treats scope membership as exactly the
/// relation that makes a withholding perturbable — reporting its *absence* as
/// [`influence::NotPosable::OutsideCompiledRegion`], documented there as not zero influence.
/// Classing such a fact zero would have the certificate assert a proof the region it ships
/// alongside contradicts. [`carried_by_region`] is that second pass.
///
/// Exactly one fact per needed variable survives selection —
/// [`bioprism_world::WorldSource::fact_providing`] returns the last in document order — so every
/// other provider of that variable was dropped by a document-order tiebreak. Before this pass
/// existed they fell into the zero group by subtraction and were published with a bound of `0.0`,
/// which asserts a proof of irrelevance for a fact the compiler never looked at. `AGENTS.md` names
/// this exact collapse as non-negotiable: "provably cannot matter" and "nobody checked" must never
/// share a representation.
///
/// Policy-withheld and temporally withheld facts are excluded here because they already carry
/// their own class; leaving them in would count one omission in two groups and inflate
/// [`OmissionManifest::total_omitted`] past the corpus.
///
/// Output-sensitive: one lookup per needed variable, and no traversal of the omitted population.
/// Enumerated rather than counted, because unlike the unreachable group this one is bounded by the
/// compiled region and its members can therefore be named on the certificate.
///
/// Sorted rather than set-collected, and the difference is the whole point. A `BTreeSet` here
/// would collapse an identifier reported for two needed variables into one entry before
/// [`ProvenUnreachable::from_classified`] could see it — and one identifier standing for two facts
/// is precisely the [`OmissionAccountingError::NamedTwice`] condition that constructor exists to
/// refuse. Deduplicating would answer the question on the constructor's behalf, and answer it the
/// reassuring way: the second fact would vanish from the classified population and reappear in the
/// remainder, published as provably unable to matter.
fn reachable_but_unselected<S: WorldSource + ?Sized>(
    source: &S,
    region: Result<&QueryRegion, &RegionError>,
    needed: &BTreeSet<String>,
    selected: &BTreeSet<String>,
    withheld_by_policy: &[String],
    inaccessible: &[String],
) -> ReachingOmissions {
    let mut reaching = ReachingOmissions::default();
    let mut unselected: Vec<String> = Vec::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    for variable in needed {
        let winner = source.fact_providing(variable);
        for id in source.shadowed_provider_ids(variable) {
            if winner
                .as_ref()
                .is_some_and(|fact| fact.id.as_str() == id.as_str())
            {
                ambiguous.insert(variable.clone());
                continue;
            }
            if selected.contains(&id)
                || withheld_by_policy.contains(&id)
                || inaccessible.contains(&id)
            {
                continue;
            }
            unselected.push(id);
        }
    }
    unselected.sort();
    reaching.unselected = unselected;
    match region {
        Ok(region) => {
            let carried = carried_by_region(
                source,
                region,
                needed,
                selected,
                withheld_by_policy,
                inaccessible,
            );
            reaching.region_carried = carried.carried;
            ambiguous.extend(carried.ambiguous);
        }
        Err(error) => reaching.region_unavailable = Some(error.to_string()),
    }
    reaching.ambiguous_variables = ambiguous.into_iter().collect();
    reaching
}

/// Omitted facts providing a variable a selected factor carries but no target needs.
///
/// The sibling-output population. Every variable here is one the compiled region declares and
/// sizes for elimination, so an omitted fact providing it has an image the region can perturb, and
/// the certificate may not call it provably unable to move the decision.
///
/// The region is read rather than recomputed. `bioprism-backends` owns the scope construction —
/// inputs and outputs, deduplicated, per selected factor — and it is the same region object handed
/// to [`crate::influence`], so the compiler's judgement about what the region carries and the
/// pass that perturbs it cannot disagree.
///
/// Both the winner and the displaced providers are examined, which is the difference from
/// [`reachable_but_unselected`]'s loop. There, the winner of a needed variable is *selected* and so
/// not omitted at all; here the variable is not needed, so unless the fact carries one of the
/// query's protected tags nothing selected any provider of it, and the winner is omitted exactly
/// like its shadowed siblings — it is the fact the defect actually misclassified. Anything the
/// protected closure did select is filtered out below along with the policy- and cut-withheld.
///
/// The winner-among-displaced guard is [`reachable_but_unselected`]'s, and it is here for the
/// reason the accounting exists: two passes reading one corpus defect may not reach two verdicts
/// about it. Left unguarded this pass pushed both colliding copies, and the collision did
/// self-report — [`ProvenUnreachable::from_classified`] saw the repeat and refused as
/// [`OmissionAccountingError::NamedTwice`]. That refusal was the wrong one twice over. It named a
/// classifier disagreement where the corpus, not the classifier, is what cannot tell two facts
/// apart, and it left this group counting one fact twice, so
/// [`OmissionManifest::total_omitted`] fell short of the corpus while the sibling pass's
/// [`UnprovenRemainder::AmbiguousIdentifier`] keeps the books balanced on the same defect.
///
/// A variable is visited once however many factors carry it, and the site recorded is the
/// lowest-numbered of them — [`QueryRegion::factors`] is sorted by identifier. Visiting per factor
/// instead would push one fact once per carrying factor and turn a two-factor scope overlap into a
/// spurious [`OmissionAccountingError::NamedTwice`].
///
/// Output-sensitive: the scan is over the compiled region's factor scopes, and the lookups are two
/// per scope variable the slice did not already need. Nothing here touches the omitted corpus.
///
/// A region is required rather than optional. Without one this pass cannot run at all, and its
/// whole population would fall back into the remainder and be published with a bound of `0.0` —
/// the defect the pass exists to remove, restored silently. [`reachable_but_unselected`] declines
/// the proof in that case instead. No compile is known to reach it: every refusal
/// [`QueryRegion::from_world_slice`] can raise is ruled out by how it builds the region under
/// `CardinalityPolicy::default()`. That argument belongs to another crate, so the type keeps the
/// obligation here rather than importing the conclusion.
fn carried_by_region<S: WorldSource + ?Sized>(
    source: &S,
    region: &QueryRegion,
    needed: &BTreeSet<String>,
    selected: &BTreeSet<String>,
    withheld_by_policy: &[String],
    inaccessible: &[String],
) -> CarriedByRegion {
    let mut sites: BTreeMap<&str, &str> = BTreeMap::new();
    for factor in region.factors() {
        for variable in factor.scope() {
            if needed.contains(variable.as_str()) {
                continue;
            }
            sites
                .entry(variable.as_str())
                .or_insert_with(|| factor.id());
        }
    }

    let mut found = CarriedByRegion::default();
    for (variable, factor) in sites {
        let winner = source
            .fact_providing(variable)
            .map(|fact| fact.id.as_str().to_string());
        for (position, id) in winner
            .iter()
            .cloned()
            .chain(source.shadowed_provider_ids(variable))
            .enumerate()
        {
            if position > 0 && winner.as_deref() == Some(id.as_str()) {
                found.ambiguous.push(variable.to_string());
                continue;
            }
            if selected.contains(&id)
                || withheld_by_policy.contains(&id)
                || inaccessible.contains(&id)
            {
                continue;
            }
            found.carried.push(RegionCarried {
                fact: id,
                variable: variable.to_string(),
                factor: factor.to_string(),
            });
        }
    }
    found
        .carried
        .sort_by(|left, right| left.fact.cmp(&right.fact));
    found
}

/// What [`carried_by_region`] found: the classified population, and the variables it could not
/// classify over.
///
/// Two outputs for the same reason [`ReachingOmissions`] has two: naming a fact is only a
/// classification when the corpus can tell that fact from another one, and a variable whose
/// displaced provider carries the winner's own identifier fails that precondition. The ambiguity
/// travels back to [`ReachingOmissions::ambiguous_variables`] rather than being answered here, so
/// one collision reaches one refusal however many passes noticed it.
#[derive(Debug, Default)]
struct CarriedByRegion {
    carried: Vec<RegionCarried>,
    ambiguous: Vec<String>,
}

/// An omitted fact, the region variable it provides, and a selected factor carrying that variable.
///
/// The variable and the factor are kept beside the identifier so the manifest can say *where* the
/// region touches the omission. A group whose reason said only that some factor carried some
/// variable would leave a reader holding a fact identifier and no way to find the contradiction on
/// the certificate they are already reading, when the certificate names the selected factors.
#[derive(Debug, Clone)]
struct RegionCarried {
    fact: String,
    variable: String,
    factor: String,
}

/// The omitted facts that still reach a needed variable, and whether the corpus could name them.
///
/// Two outputs rather than one because the second is the precondition of the first. Everything
/// downstream of the slice is keyed by fact identifier — the selection, the policy screen, the
/// withheld list — so the classification is a partition of *identifiers*. When two facts providing
/// the same needed variable carry the same identifier, that partition is not a partition of facts:
/// the shadowed one is indistinguishable from the selected one at every lookup, so it is filtered
/// out of [`Self::unselected`] as though it had been delivered, and the remainder absorbs it
/// silently under the strongest class on the manifest. Recording the variable is what lets
/// [`build_manifest`] decline to mint a proof over a corpus that cannot tell the two apart.
///
/// [`bioprism_world::validate`] reports a shadowed variable as an error, but it is advisory by
/// construction and no compile runs it, so this pass cannot assume it did.
#[derive(Debug, Clone, Default)]
struct ReachingOmissions {
    /// Omitted facts providing a needed variable that nothing else accounts for, in certificate
    /// order, duplicates kept.
    unselected: Vec<String>,
    /// Variables whose shadowed provider carries the winning fact's own identifier, from both
    /// passes, sorted and deduplicated.
    ///
    /// One list rather than one per pass, because the defect is in the corpus and not in whichever
    /// pass met it: a needed variable and a variable only the region carries produce the same
    /// refusal, and a reader of [`UnprovenRemainder::AmbiguousIdentifier`] should not have to know
    /// which loop noticed.
    ambiguous_variables: Vec<String>,
    /// Omitted facts providing a variable a selected factor's scope carries but no target needs,
    /// in certificate order, duplicates kept.
    ///
    /// Disjoint from [`Self::unselected`] on any world [`bioprism_world::World`] can load: a fact
    /// provides exactly one variable and the two populations are keyed on complementary variable
    /// sets. A [`bioprism_world::WorldSource`] that reports one identifier under both is not a
    /// partition, and the duplicate survives to [`ProvenUnreachable::from_classified`] for the same
    /// reason it does within either list.
    region_carried: Vec<RegionCarried>,
    /// Why no compiled region was available, when none was.
    ///
    /// [`Self::region_carried`] being empty means two different things — the region carries no
    /// unclassified provider, or there was no region to ask — and only one of them supports a
    /// proof. Recording the refusal keeps the second from rendering as the first.
    region_unavailable: Option<String>,
}

/// Why this compile's omitted remainder carries no proof, when it carries none.
///
/// On [`CompileTrace`] rather than only in the manifest's reason string because a certificate with
/// no [`InfluenceClass::Zero`] group looks exactly like one whose corpus had nothing to prove, and
/// those are different facts about the compile. A caller replaying a compile needs to tell "every
/// omission was classified elsewhere" from "the proof was refused".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnprovenRemainder {
    /// A variable the compile reaches has two providers carrying one identifier, so no
    /// identifier-keyed accounting over this corpus is a partition of its facts.
    ///
    /// Reached by dependency or by scope alike: the two passes that meet this collision report it
    /// the same way, because it is a property of the corpus rather than of the pass that noticed.
    AmbiguousIdentifier { variables: Vec<String> },
    /// No region could be compiled, so the pass that classifies what the region carries never ran.
    ///
    /// Its population would otherwise be the remainder's, published with a bound of `0.0` — a proof
    /// resting on a scope check that never happened. [`crate::plan::evaluate`] publishes the same
    /// rejection as [`crate::plan::PortfolioOutcome::RegionRejected`], but a reader of the manifest
    /// is owed the consequence for the *omissions* and not only for the plan.
    RegionUnavailable { error: String },
    /// The classified populations did not partition the omitted corpus.
    ///
    /// The four populations are disjoint by construction on any world [`bioprism_world::World`] can
    /// load, so on that path this stays an invariant check rather than a reachable branch. It is
    /// reachable through [`bioprism_world::WorldSource`]: a source reporting one identifier for two
    /// displaced providers of two needed variables lands here as
    /// [`OmissionAccountingError::NamedTwice`], which is why the displaced population is handed to
    /// the constructor with its duplicates intact instead of collapsed into a set first.
    Accounting(OmissionAccountingError),
}

/// Groups omissions by structural reason and assigns each an influence class.
///
/// Facts the compiled region does not reach are classed [`InfluenceClass::Zero`] *conditional on
/// the declared factor graph being complete* — the reason string states that assumption, because
/// an incomplete factor graph would turn a zero-influence claim into an unknown-influence one.
/// That class is now minted from [`ProvenUnreachable`] rather than from a bare remainder, so the
/// population computed by [`reachable_but_unselected`] has to be named at the call site before a
/// zero-influence count can exist at all. Those facts get [`InfluenceClass::Unknown`], which voids
/// the sufficiency claim, and that is the correct verdict: a decision compiled while a competing
/// value for a needed variable sat unexamined in the corpus is not one the compiler can certify.
///
/// Every classified population is handed to [`ProvenUnreachable::from_classified`] by name and the
/// subtraction happens there. The arithmetic used to be four `saturating_sub` calls here, which
/// defeated the one guarantee that constructor was built to give: it refuses to underflow into a
/// count precisely because "everything is provably irrelevant" is the worst way for an accounting
/// error to render, and a caller that saturates first hands it a number that has already been
/// rounded up to that answer. Saturation can only shrink the proven group, so no certificate ever
/// overstated a proof through it, but it made the disagreement invisible — and an invariant that
/// cannot be observed failing is not being checked.
///
/// Whether a refusal keeps the remainder depends on which check declined, and the two arms differ.
/// [`UnprovenRemainder::AmbiguousIdentifier`] arrives with the subtraction already done, so those
/// facts are still pushed under [`InfluenceClass::Unknown`] with the refusal in the reason and
/// [`OmissionManifest::total_omitted`] stays equal to the corpus count. On
/// [`UnprovenRemainder::Accounting`] the subtraction itself is what declined: there is no remainder
/// count to publish, the group carries a count of zero, and `total_omitted` therefore does *not*
/// equal the corpus count — under [`OmissionAccountingError::ExceedsOmitted`] it falls short by
/// however many the remainder would have held, and under [`OmissionAccountingError::NamedTwice`]
/// the classified groups that caused the refusal are themselves naming a fact twice. What both arms
/// do guarantee is that the certificate says the proof was declined rather than quietly not
/// offering one; a count over books that do not balance is the thing this arm cannot also promise,
/// and inventing one is how the disagreement would become invisible again.
///
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
    reaching: &ReachingOmissions,
    exploratory: usize,
) -> (OmissionManifest, Option<UnprovenRemainder>) {
    let mut manifest = OmissionManifest::default();
    let classified = withheld
        .bounded
        .iter()
        .chain(withheld.deferred.iter())
        .chain(withheld_by_policy.iter())
        .chain(reaching.unselected.iter())
        .map(String::as_str)
        .chain(
            reaching
                .region_carried
                .iter()
                .map(|carried| carried.fact.as_str()),
        );

    let mut refused = None;
    match ProvenUnreachable::from_classified(omitted_total, classified) {
        Ok(proven) if proven.count() == 0 => {}
        Ok(proven) => match declined(reaching) {
            None => manifest.push(OmissionGroup::structurally_zero(
                "no backward dependency path to any target under the declared factor graph",
                proven,
                Vec::new(),
            )),
            Some((reason, remainder)) => {
                manifest.push(OmissionGroup {
                    reason,
                    influence: InfluenceClass::Unknown,
                    count: proven.count(),
                    bound: None,
                    examples: Vec::new(),
                });
                refused = Some(remainder);
            }
        },
        Err(error) => {
            manifest.push(OmissionGroup {
                reason: format!(
                    "the classified omissions do not partition the omitted corpus, so no \
                     zero-influence remainder follows from them: {error}"
                ),
                influence: InfluenceClass::Unknown,
                count: 0,
                bound: None,
                examples: Vec::new(),
            });
            refused = Some(UnprovenRemainder::Accounting(error));
        }
    }
    if !reaching.unselected.is_empty() {
        manifest.push(OmissionGroup {
            reason: "provides a variable the slice needs but was shadowed by a later fact \
                     providing the same variable; the omission has a backward dependency path to \
                     the target and no bound on it was computed"
                .into(),
            influence: InfluenceClass::Unknown,
            count: reaching.unselected.len(),
            bound: None,
            examples: reaching.unselected.iter().take(3).cloned().collect(),
        });
    }
    if !reaching.region_carried.is_empty() {
        let named = &reaching.region_carried[..reaching.region_carried.len().min(EXAMPLES_SHOWN)];
        manifest.push(OmissionGroup {
            reason: format!(
                "provides a variable no target needs but that a selected factor carries in its \
                 scope, so the compiled region has an image of the omission to perturb and no \
                 bound on it was computed; the fact(s) named here are carried at {}",
                named_scope_sites(named)
            ),
            influence: InfluenceClass::Unknown,
            count: reaching.region_carried.len(),
            bound: None,
            examples: named.iter().map(|carried| carried.fact.clone()).collect(),
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
    (manifest, refused)
}

/// Why the remainder carries no proof, when a condition the compiler checked says it may not.
///
/// The subtraction can succeed and the proof still be unavailable, and both conditions here are of
/// that shape: the arithmetic balances, but something the count would have to mean is not
/// established. `None` is the only value that lets a zero-influence group be minted.
///
/// A missing region is reported ahead of an ambiguous identifier, because without a region the
/// pass that reads factor scopes never ran and the ambiguity list it contributes to is therefore
/// partial. Naming the region says the larger thing, and both refusals carry the same remainder
/// count, so nothing is lost by ordering them.
fn declined(reaching: &ReachingOmissions) -> Option<(String, UnprovenRemainder)> {
    if let Some(error) = &reaching.region_unavailable {
        return Some((
            format!(
                "left over once every other omission was classified, but no proof follows: no \
                 region could be compiled, so nothing checked whether a selected factor's scope \
                 carries these omissions; the region was refused because {error}"
            ),
            UnprovenRemainder::RegionUnavailable {
                error: error.clone(),
            },
        ));
    }
    if reaching.ambiguous_variables.is_empty() {
        return None;
    }
    let variables = reaching.ambiguous_variables.clone();
    Some((
        format!(
            "left over once every other omission was classified, but no proof follows: {} \
             variable(s) the compile reaches have two providers under one identifier, so the \
             classification cannot tell a delivered fact from a displaced one; the collision is \
             on {}",
            variables.len(),
            named_variables(&variables)
        ),
        UnprovenRemainder::AmbiguousIdentifier { variables },
    ))
}

/// The first few of `variables`, rendered for a reason string.
///
/// The ambiguous-identifier group has no members it is allowed to name — that a fact cannot be
/// named is the refusal — so the variables that refused it go in the reason rather than in
/// [`OmissionGroup::examples`], which names members of the population the group counts, and on this
/// manifest that population is omitted facts: `bioprism_devx`'s `why_omitted` matches its subject
/// against `examples` after failing to find it in the selected fact set, so a variable name there
/// answers a question about a fact. [`UnprovenRemainder::AmbiguousIdentifier`] on the trace carries
/// the full list.
fn named_variables(variables: &[String]) -> String {
    let shown: Vec<&str> = variables.iter().take(3).map(String::as_str).collect();
    if variables.len() > shown.len() {
        format!(
            "{} and {} more",
            shown.join(", "),
            variables.len() - shown.len()
        )
    } else {
        shown.join(", ")
    }
}

/// The places the region carries the omissions in `named`, rendered for a reason string.
///
/// A site is a variable and the selected factor whose scope carries it, and it is the site rather
/// than the fact that answers the reader's question: the group's own `examples` already name facts,
/// and the certificate they are on names the selected factors, so a site is the pair that lets the
/// contradiction be checked against the certificate without re-running the compile.
///
/// `named` is the same slice the group's `examples` are built from, and that is the point of taking
/// a slice rather than the whole population. Truncating here independently — the first few sites of
/// the whole group under one ordering, beside the first few facts under another — let the reason
/// describe facts the examples do not name once the group grew past what either shows.
///
/// Deduplicated because several omitted facts can share one site — every displaced provider of a
/// carried variable does — and repeating it once per fact would misreport how many places the
/// region touches as the number of facts it touches there. First appearance wins, so the sites come
/// out in the order the facts beside them do.
fn named_scope_sites(named: &[RegionCarried]) -> String {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let sites: Vec<String> = named
        .iter()
        .map(|entry| format!("{} in {}", entry.variable, entry.factor))
        .filter(|site| seen.insert(site.clone()))
        .collect();
    sites.join(", ")
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

/// Refusals and renderings [`build_manifest`] decides, reached directly.
///
/// Neither case below is one a world can pose today, which is why they are posed to the function
/// that decides them. `QueryRegion::from_world_slice` under `CardinalityPolicy::default()` has no
/// failing branch reachable from here: every domain size it derives is at least one, every scope
/// name is declared before the region is built, scopes are deduplicated on the way in, and a
/// structurally derived factor carries no table to mismatch. So the missing-region arm is held
/// against an invariant that lives in `bioprism-backends` and that this crate cannot check — which
/// is the argument for keeping the arm rather than against it, because the fallback it replaces
/// fails by publishing a proof nobody computed. A region-carried group larger than the examples it
/// shows likewise needs a corpus wider than any fixture, and the pairing of the two renderings is
/// the property under test rather than the width.
#[cfg(test)]
mod tests {
    use super::*;

    fn carried(fact: &str, variable: &str, factor: &str) -> RegionCarried {
        RegionCarried {
            fact: fact.to_string(),
            variable: variable.to_string(),
            factor: factor.to_string(),
        }
    }

    /// With no region, the remainder is published unproven and the reason names the missing region.
    ///
    /// The silent alternative is the defect this pass was written to remove: with nothing to read
    /// factor scopes from, every sibling-output provider falls through to the remainder and is
    /// published as provably unable to matter, with no reader able to tell that from a corpus that
    /// genuinely had nothing carried.
    #[test]
    fn a_compile_with_no_region_declines_the_proof_and_names_the_region() {
        let reaching = ReachingOmissions {
            region_unavailable: Some("variable \"x\" has cardinality zero".to_string()),
            ..ReachingOmissions::default()
        };
        let (manifest, refused) = build_manifest(4, &WithheldSplit::default(), &[], &reaching, 0);

        assert_eq!(
            refused,
            Some(UnprovenRemainder::RegionUnavailable {
                error: "variable \"x\" has cardinality zero".to_string()
            })
        );
        assert_eq!(manifest.count_in(InfluenceClass::Zero), 0);
        assert_eq!(manifest.count_in(InfluenceClass::Unknown), 4);
        assert_eq!(
            manifest.total_omitted(),
            4,
            "declining the proof must not drop the omissions it would have covered"
        );
        let group = manifest
            .groups
            .first()
            .expect("the declined remainder is on the manifest");
        assert!(
            group.reason.contains("no region could be compiled")
                && group.reason.contains("variable \"x\" has cardinality zero"),
            "the reason must name what was missing and why: {}",
            group.reason
        );
        assert_eq!(group.bound, None);
    }

    /// The carried group's examples and its reason describe the same facts.
    ///
    /// Six facts at six sites, and only three of each are shown. Truncating the two lists
    /// independently — the facts in identifier order, the sites in their own — let a reader compare
    /// a named fact against a site belonging to a different one.
    #[test]
    fn the_carried_group_names_the_sites_of_the_facts_it_shows() {
        let reaching = ReachingOmissions {
            region_carried: vec![
                carried("fact.a", "var_z", "factor.one"),
                carried("fact.b", "var_y", "factor.one"),
                carried("fact.c", "var_x", "factor.two"),
                carried("fact.d", "var_a", "factor.two"),
                carried("fact.e", "var_b", "factor.three"),
                carried("fact.f", "var_c", "factor.three"),
            ],
            ..ReachingOmissions::default()
        };
        let (manifest, refused) = build_manifest(6, &WithheldSplit::default(), &[], &reaching, 0);

        assert_eq!(refused, None);
        let group = manifest
            .groups
            .iter()
            .find(|group| group.reason.contains("carries in its scope"))
            .expect("the carried group is on the manifest");
        assert_eq!(group.count, 6, "the count is the whole population");
        assert_eq!(
            group.examples,
            vec![
                "fact.a".to_string(),
                "fact.b".to_string(),
                "fact.c".to_string()
            ]
        );
        assert!(
            group
                .reason
                .ends_with("var_z in factor.one, var_y in factor.one, var_x in factor.two"),
            "the sites are those of the facts named, in their order: {}",
            group.reason
        );
        assert!(
            !group.reason.contains("var_a") && !group.reason.contains("factor.three"),
            "and no others: var_a is fact.d's site, and fact.d is not a fact this group names: {}",
            group.reason
        );
    }

    /// One site named once, however many of the shown facts share it.
    #[test]
    fn the_carried_group_names_a_shared_site_once() {
        let reaching = ReachingOmissions {
            region_carried: vec![
                carried("fact.a", "var_a", "factor.one"),
                carried("fact.b", "var_a", "factor.one"),
            ],
            ..ReachingOmissions::default()
        };
        let (manifest, _) = build_manifest(2, &WithheldSplit::default(), &[], &reaching, 0);

        let group = manifest
            .groups
            .iter()
            .find(|group| group.reason.contains("carries in its scope"))
            .expect("the carried group is on the manifest");
        assert!(group.reason.ends_with("carried at var_a in factor.one"));
        assert_eq!(group.examples.len(), 2);
    }
}
