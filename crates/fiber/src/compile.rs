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
use crate::oracle;
use crate::policy::{self, PolicyEnvelope, PolicyOutcome, PolicyScreen, POLICY_REFINEMENT_ACTION};
use crate::qir::Query;
use crate::slice::{backward_slice, max_selected_arity};
use crate::temporal::{temporal_cut, TemporalCut};
use bioprism_section::{
    Backend, ContextCertificate, DecisionSection, EvidenceCapsule, InfluenceClass, OmissionGroup,
    OmissionManifest, PlanDescriptor, ReferenceOmissions, RefinementOption, SourceHashes,
    UnresolvedObligation,
};
use bioprism_ids::ContentHash;
use bioprism_world::{Fact, WorldSource};
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

pub fn compile<S: WorldSource + ?Sized>(
    source: &S,
    query: &Query,
) -> Result<CompileOutput, FiberError> {
    let mut passes = Vec::new();

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
        note: format!("{} variables reachable from targets", slice.needed_variables.len()),
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
                screen.missing_for(id).expect("withheld ids carry their clauses"),
            ),
        })
        .collect();
    unresolved.extend(
        inaccessible
            .iter()
            .map(|id| UnresolvedObligation::InaccessibleAtCut { fact_id: id.clone() }),
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

    let plan = PlanDescriptor {
        backend: Backend::BackwardFactorSliceReference,
        compiled_factor_count: slice.selected_factors.len(),
        compiled_fact_count: selected_facts.len(),
        total_factor_count: source.total_factors(),
        total_fact_count: source.total_facts(),
        max_selected_factor_arity: max_selected_arity(source, &slice.selected_factors),
        fallback: None,
    };
    passes.push(PassReceipt {
        name: "plan_selection",
        retained: plan.compiled_fact_count,
        note: format!(
            "backend {} retained {:.4} of facts",
            plan.backend.as_str(),
            plan.fact_selection_ratio()
        ),
    });

    let manifest = build_manifest(
        omitted_total,
        &inaccessible,
        &withheld_by_policy,
        omitted_exploratory,
    );

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
fn build_manifest(
    omitted_total: usize,
    inaccessible: &[String],
    withheld_by_policy: &[String],
    exploratory: usize,
) -> OmissionManifest {
    let mut manifest = OmissionManifest::default();
    let unreachable = omitted_total
        .saturating_sub(inaccessible.len())
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
    if !inaccessible.is_empty() {
        manifest.push(OmissionGroup {
            reason: "governed by an event not yet available at the decision cut".into(),
            influence: InfluenceClass::DeferredAcquisition,
            count: inaccessible.len(),
            bound: None,
            examples: inaccessible.iter().take(3).cloned().collect(),
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
            "43.33 binds role and purpose to the query before selection; fiber-query/0.1 carries a role string and no purpose, and no pass reads either",
        ),
        (
            "information_flow_export",
            "43.33 orders outputs by policy label; fiber-world/0.1 declares no labels and no rules attached at scopes, so only read access is decided",
        ),
    ];
    if query.missing_contract_fields().contains(&"decision_loss") {
        deferred.push((
            "decision_quotient",
            "43.10 is defined relative to permitted actions and decision loss, neither of which fiber-query/0.1 carries",
        ));
        deferred.push((
            "rate_distortion",
            "43.12 optimises against a decision loss the query does not declare",
        ));
    }
    deferred
}
