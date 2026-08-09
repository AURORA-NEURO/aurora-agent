//! The compiler pipeline.
//!
//! Blueprint 43.16 stages the compiler as `q → QIR → PCIR → SIR → LIR → AIR → PIR → render`, with
//! each pass replayable and each emitting receipts. The v0.1 engine implements the passes that
//! the wire schema can express — protected closure, dependency slice, temporal cut, plan
//! selection, render — and records the ones it cannot in [`CompileTrace::deferred_passes`]
//! rather than pretending they ran.
//!
//! Pass order is normative, not incidental: closure is computed *before* slicing so that
//! protected evidence enters the selection whether or not a dependency path reaches it.

use crate::closure::{dropped_protected, protected_closure, unmatched_tags};
use crate::error::FiberError;
use crate::oracle;
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
    pub dropped_protected: Vec<String>,
    pub temporal_cut: TemporalCut,
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
}

pub fn compile<S: WorldSource + ?Sized>(
    source: &S,
    query: &Query,
) -> Result<CompileOutput, FiberError> {
    let mut passes = Vec::new();

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

    let unresolved: Vec<UnresolvedObligation> = inaccessible
        .iter()
        .map(|id| UnresolvedObligation::InaccessibleAtCut { fact_id: id.clone() })
        .collect();
    let frontier = if inaccessible.is_empty() {
        Vec::new()
    } else {
        vec![RefinementOption {
            action: RETROSPECTIVE_ACTION.into(),
            facts: inaccessible.clone(),
        }]
    };

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

    let manifest = build_manifest(omitted_total, &inaccessible, omitted_exploratory);

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
fn build_manifest(
    omitted_total: usize,
    inaccessible: &[String],
    exploratory: usize,
) -> OmissionManifest {
    let mut manifest = OmissionManifest::default();
    let unreachable = omitted_total.saturating_sub(inaccessible.len());

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
