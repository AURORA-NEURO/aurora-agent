//! What the workspace actually does, per contract, with citations.
//!
//! Each function returns a [`ServiceReport`] for one §40 contract, read off the crate that
//! implements it. These are facts, and the facts are separated from the checker in
//! [`crate::conformance`] on purpose: disagreeing with a verdict should mean editing a claim about
//! the code, not editing the logic that consumes claims.
//!
//! # How a field was decided
//!
//! `accepts` and `produces` are named by the *contract's* field names, and a field counts as
//! present when the entry point takes or returns something that plays that role — not when a
//! similarly-named type exists somewhere in the crate. `bioprism-benchcompiler` exports
//! `CandidateActionSet`, and `pipeline::compile` does not return one, so the output is recorded as
//! absent.
//!
//! `error_classes` is the set of classes the implementation can express *as a type at its entry
//! point*. `bioprism-mutation::lineage::generate` returned `Result<Family, String>` until this audit named it; the typed
//! `ApplyError` underneath it is real, so the classes are recorded, and the erasure is recorded in
//! the evidence instead of being silently forgiven. It is the single clearest violation of 40.36's
//! first invariant — *errors are not strings only* — in the workspace.
//!
//! `enforces` is the strictest field. An invariant counts as enforced when something refuses when
//! it is broken. An invariant that is true because the code has no feature that could break it is
//! *not* enforced, because the moment the feature arrives nothing will notice. 40.25's second
//! invariant — learned ranking cannot bypass policy or closure — is the example: `bioprism-fiber`
//! has no learned ranking, so it cannot be bypassed, and adding one would go unchecked.
//!
//! # A caveat about class-level failure checking
//!
//! [`crate::conformance`] asks whether a failure mode's *class* is expressible, not whether that
//! specific mode is detected. So 40.25's "summarizer inconsistency" counts as typed against
//! `bioprism-fiber` because `FiberError` can express `ContractViolation`, even though `fiber`
//! writes no summaries at all. The check is deliberately coarse there: a finer one would require
//! this crate to assert, per failure mode, that a specific code path exists, which is a claim it
//! cannot substantiate by reading. The coarse check still catches every class a crate cannot
//! express, which is where the interesting gaps turned out to be.

use crate::conformance::ServiceReport;
use crate::contract::{ContractId, Delivery, Effect, Idempotency};
use crate::error::ErrorClass;

/// 40.25 against `bioprism-fiber`.
pub fn context_compiler() -> ServiceReport {
    ServiceReport::new(
        ContractId::ContextCompiler,
        "bioprism_fiber::compile(&impl WorldSource, &Query) -> Result<CompileOutput, FiberError>",
    )
    .in_crates(&["bioprism-fiber", "bioprism-section"])
    .accepting(&[
        "world_cell_role_references",
        "goal_and_obligation_contract",
        "policy",
        "token_tool_privacy_budgets",
    ])
    .producing(&[
        "plan",
        "capsule",
        "omission_ledger",
        "sufficiency_certificate",
    ])
    .raising(&[ErrorClass::InvalidInput, ErrorClass::ContractViolation])
    .touching(&[Effect::ReadsGraph])
    .delivered(Delivery::Immediate, Idempotency::Pure)
    .enforcing(&[1, 3])
    .noting(
        "Query carries `policy: Vec<String>` and `role: Option<String>`, so the input is accepted; \
         qir.rs parses `policy` and no pass in compile.rs or closure.rs reads it. The contract's \
         `policy conflict` failure therefore cannot fire, and invariant 2 has nothing to enforce.",
    )
    .noting(
        "`candidate_decision_trace` is not produced. CompileTrace records one PassReceipt per \
         compiler pass, which is a pass trace; the reference backend has no scoring frontier and \
         so no candidates to trace.",
    )
    .noting(
        "`expansion_api` is not produced. DecisionSection::refinement_frontier publishes the \
         options an expansion would offer, and no operation consumes them.",
    )
    .noting(
        "Invariant 1 is enforced: protected_closure runs before the slice, dropped_protected is \
         reported, and CompileOutput::protected_closure_satisfied is a refusal a caller can act on.",
    )
    .noting(
        "Invariant 4 is not enforced by this entry point; the full-context control arm lives \
         outside `compile` and nothing here checks it reproduces.",
    )
    .noting(
        "An id that fails to resolve is dropped from the selection rather than raised. It cannot \
         happen for a self-consistent WorldSource, so this is a missing contract surface rather \
         than a live defect, but `unresolvable source` has no typed representation.",
    )
}

/// 40.18 against `bioprism-worldgen`.
pub fn world_builder() -> ServiceReport {
    ServiceReport::new(
        ContractId::WorldBuilder,
        "bioprism_worldgen::generate(&WorldSpec) -> Generated",
    )
    .in_crates(&["bioprism-worldgen", "bioprism-world"])
    .accepting(&["world_manifest"])
    .producing(&["world_graph_and_event_ledger"])
    .raising(&[ErrorClass::InvalidInput])
    .delivered(Delivery::Immediate, Idempotency::Pure)
    .enforcing(&[1])
    .noting(
        "`generate` is infallible: it returns `Generated { world, query }`, two JSON values. None \
         of 40.18's five failure modes can be raised from the entry point.",
    )
    .noting(
        "InvalidInput is expressible through bioprism-world's WorldError, whose DuplicateFactId is \
         exactly 40.18's `identity conflict` — but it is raised when a world is *parsed*, not when \
         one is built.",
    )
    .noting(
        "There is no version, no build report and no world card. WorldSpec is a fixture generator's \
         spec (subjects, tag style, leakage mechanism), not a manifest of adapter outputs, action \
         definitions and a reveal policy.",
    )
    .noting(
        "Invariant 1 is enforced: generation is seeded and deterministic. Invariants 2 to 4 have \
         nothing in the code that could refuse.",
    )
}

/// 40.20 against `bioprism-benchcompiler`.
pub fn decision_compiler() -> ServiceReport {
    ServiceReport::new(
        ContractId::DecisionCompiler,
        "bioprism_benchcompiler::pipeline::compile(..) -> Result<Compilation, CompileError>",
    )
    .in_crates(&["bioprism-benchcompiler"])
    .accepting(&[
        "world_run_trace",
        "outcome_and_oracle_evidence",
        "compiler_policy",
    ])
    .producing(&[
        "biodecision_cell",
        "minimal_context_state",
        "divergence_and_fidelity_report",
    ])
    .raising(&[
        ErrorClass::InvalidInput,
        ErrorClass::ContractViolation,
        ErrorClass::Indeterminate,
    ])
    .touching(&[Effect::ReadsGraph, Effect::ReadsEvaluatorState])
    .delivered(Delivery::Immediate, Idempotency::ContentAddressed)
    .enforcing(&[1, 3, 4])
    .noting(
        "The closest match of the seven. Six typed error enums cover every declared failure: \
         MinimizeError::PropertyLost is `minimization changes defect`, NondeterministicProbe is \
         `oracle unstable`, CompileError::NotCompilable is `boundary ambiguous`, ActionError is \
         `action set incomplete`.",
    )
    .noting(
        "`candidate_decision_boundary` is not an input. boundary::boundaries derives candidates \
         from the trace instead of accepting one, which is stricter than the contract and better; \
         the contract is what should move.",
    )
    .noting(
        "`candidate_action_set` is not returned. Compilation carries trace_id, episodes, \
         boundaries, analysis, card, minimization, oracle, class, confidence and provenance; \
         CandidateActionSet is public but not part of the pipeline's output.",
    )
    .noting(
        "Invariant 2 (acceptable actions are set-valued) has no representation in the pipeline \
         output, which follows from the missing action set.",
    )
    .noting(
        "Invariant 1 is enforced by MinimizeError::PropertyLost and NotOneMinimal; invariant 3 by \
         dedup::Holdout and ExposureLedger; invariant 4 by Minimization recording each removal.",
    )
    .noting(
        "ExecutesSandbox is inverted: the fork is the caller's `InterestProbe`, so the compiler \
         never executes anything itself.",
    )
}

/// 40.22 against `bioprism-mutation`.
pub fn mutation_runtime() -> ServiceReport {
    ServiceReport::new(
        ContractId::MutationRuntime,
        "bioprism_mutation::lineage::generate(&Value, &[Mutation]) -> Result<Family, MutationError>",
    )
    .in_crates(&["bioprism-mutation"])
    .accepting(&[
        "parent_world_or_cell",
        "mutation_program_and_seed",
        "oracle_postconditions",
    ])
    .producing(&[
        "descendant_world_or_cell",
        "lineage_edges",
        "semantic_validation_report",
        "effective_diversity_features",
    ])
    .raising(&[ErrorClass::InvalidInput, ErrorClass::ContractViolation])
    .touching(&[Effect::ReadsGraph])
    .delivered(Delivery::Immediate, Idempotency::ContentAddressed)
    .enforcing(&[2, 3, 4])
    .noting(
        "FIXED since this audit named it. The entry point returned `Result<Family, String>` and          erased the typed ApplyError underneath — the workspace's clearest breach of 40.36's first          invariant, at the boundary a caller actually uses. It now returns `MutationError`, whose          variants each name the world they are about and carry the underlying typed error as a          source, and `Rejection.reason` is a typed `RejectionReason` rather than prose.",
    )
    .noting(
        "Invariant 1 is half enforced. Instance carries parent_id and mutation_id, so parents are \
         identified exactly; Mutation carries id, kind and relation and no version, so operator \
         versions are not identified at all. A repaired operator produces descendants \
         indistinguishable from those of the operator it replaced.",
    )
    .noting(
        "`preconditions` is not an input. ApplyError::NotApplicable is raised from inside the \
         operator rather than checked against a supplied precondition set.",
    )
    .noting(
        "Duplicates are not an error: Family::duplicates lists them as data. That is better than \
         the contract's reading, which classes a duplicate descendant as a conflict; nothing is \
         lost by reporting it.",
    )
    .noting(
        "Invariant 4 is enforced by Family::yield_rate counting accepted against attempted, so a \
         rejected descendant cannot inflate the family size.",
    )
}

/// 40.23 against `bioprism-prism` and `bioprism-evalengine`.
pub fn matched_evaluator() -> ServiceReport {
    ServiceReport::new(
        ContractId::MatchedEvaluator,
        "bioprism_prism::fork::matched_fork(&DecisionCell, &World, &Query, &[Architecture]) -> ForkResult",
    )
    .in_crates(&["bioprism-prism", "bioprism-evalengine"])
    .accepting(&["biodecision_cell", "architecture_manifests", "oracle_and_metric_plan"])
    .producing(&[
        "paired_executions",
        "component_effects",
        "cost_and_failure_traces",
        "signed_result_bundle",
    ])
    .raising(&[ErrorClass::InvalidInput, ErrorClass::ContractViolation])
    .touching(&[Effect::ReadsGraph, Effect::ReadsEvaluatorState])
    .delivered(Delivery::Immediate, Idempotency::ContentAddressed)
    .enforcing(&[1, 2, 4])
    .noting(
        "Invariant 1 is enforced structurally and this is the strongest single result of the \
         audit: matched_fork takes one world and one query and runs every architecture against \
         them, so identical state across arms is not checked, it is unrepresentable otherwise.",
    )
    .noting(
        "`randomization_repetition_plan` is not an input. Each architecture runs once, \
         deterministically. Invariant 3 (caches and model randomness are recorded) has nothing to \
         record because there is no randomness.",
    )
    .noting(
        "FIXED since this audit named it, and not the way the finding implied. matched_fork was          infallible, so a trial that could not run was indistinguishable from one that ran and          failed. The remedy is a state rather than a Result: an arm is Judged, Unjudged with a          typed failure, or NotAttempted with a reason — an Err would have answered an arm-level          question by discarding the arms that did run. The bug underneath was an oracle refusal          swallowed into an abstention. `provider drift` still has no representation.",
    )
    .noting(
        "The bundle is attested, not signed. bundle::Attestation is Valid/Mismatch/Malformed over \
         a recomputed content digest; there is no key material in the workspace, so `signed result \
         bundle` is delivered as `content-addressed result bundle`.",
    )
    .noting(
        "Invariant 4 is enforced in bioprism-evalengine and bioprism-adaptive rather than here: \
         cluster.rs carries the ICC, design effect and effective sample size.",
    )
}

/// 40.24 against `bioprism-adaptive`.
pub fn adaptive_scheduler() -> ServiceReport {
    ServiceReport::new(
        ContractId::AdaptiveScheduler,
        "bioprism_adaptive::panel::AdaptivePanel::{select_batch, stopping_verdict, coverage}",
    )
    .in_crates(&["bioprism-adaptive"])
    .accepting(&[
        "candidate_cells_and_lineage",
        "historical_results_posterior",
        "budget_and_mandatory_coverage",
    ])
    .producing(&[
        "ordered_evaluation_plan",
        "selection_explanations",
        "stopping_decision",
        "coverage_and_uncertainty_report",
    ])
    .raising(&[ErrorClass::InvalidInput, ErrorClass::ContractViolation])
    .touching(&[Effect::ReadsGraph])
    .delivered(Delivery::Immediate, Idempotency::Pure)
    .enforcing(&[1, 2])
    .noting(
        "`architecture_change_fingerprint` is not an input, and nothing in the crate mentions one. \
         40.24's execution path step 3 is \"score information gain and regression relevance\"; \
         with no fingerprint of what changed, only the first half is computable, and the contract \
         reads as if both were.",
    )
    .noting(
        "Invariant 1 is enforced hard: CoveragePolicy floors are refusals, and \
         AdaptiveError::CoverageFloorNotMet withholds the estimate rather than caveating it.",
    )
    .noting(
        "Invariant 2 is the crate's thesis: every estimate reports the clustered interval beside \
         the naive one and the inflation between them, and DuplicateTrial refuses a second scored \
         trial on one instance rather than counting it as independent.",
    )
    .noting(
        "Invariants 3 and 4 are not enforced. SelectionConfig and StoppingRule are parameters, so \
         pinning a policy before outcomes and predeclaring a stopping rule are disciplines a \
         caller may keep or not; CoverageGate::digest and PanelAudit::digest exist and nothing \
         requires either to be recorded first.",
    )
    .noting(
        "`posterior misspecification` and `cost estimate drift` have no typed representation. The \
         crate's own docs name both as known limits, so this is a disclosed gap rather than an \
         unexamined one.",
    )
}

/// 40.27 against `bioprism-registry` and `bioprism-hub`.
pub fn registry_backend() -> ServiceReport {
    ServiceReport::new(
        ContractId::RegistryBackend,
        "bioprism_registry::index::RegistryIndex::publish and bioprism_registry::promote",
    )
    .in_crates(&["bioprism-registry", "bioprism-hub"])
    .accepting(&[
        "signed_publication_bundle",
        "maintainer_review_decisions",
        "result_attestations",
        "visibility_policy",
    ])
    .producing(&[
        "versioned_registry_objects",
        "search_index_records",
        "health_and_trust_status",
    ])
    .raising(&[
        ErrorClass::InvalidInput,
        ErrorClass::Conflict,
        ErrorClass::ContractViolation,
        ErrorClass::PolicyDenied,
        ErrorClass::Unavailable,
    ])
    .touching(&[
        Effect::ReadsGraph,
        Effect::AppendsEventLedger,
        Effect::WritesSearchIndex,
    ])
    .delivered(Delivery::Immediate, Idempotency::AppendOnly)
    .enforcing(&[1, 2, 3])
    .noting(
        "Invariant 1 is enforced: the publication log is the source of truth and the indices are \
         projections of it. VersionAlreadyBound refuses an edit to published content in so many \
         words — a correction is a new version, never an edit.",
    )
    .noting(
        "Invariant 2 is enforced structurally: a trust tier is computed from checkable properties \
         and there is no API that sets one. Invariant 3 by PackStatus::Withdrawn being a status on \
         a retained record.",
    )
    .noting(
        "Invariant 4 is not enforced. bioprism-registry's own docs state that the policy layer of \
         10.02 is not built and there is no multi-tenancy, visibility or embargo. Private objects \
         cannot leak through search because nothing is private.",
    )
    .noting("`public_private_graph_projections` is not produced, which follows from the same gap.")
    .noting(
        "The bundle is not signed and no signature is verified. AttestationFailed is a digest \
         mismatch. 40.27's execution path step 1 is \"verify bundle/signatures\" and the first \
         half is implemented.",
    )
    .noting(
        "PolicyDenied comes from bioprism-hub — SubmitterSuspended, ConflictsUndeclared, \
         SelfReview — rather than from the registry. A caller using bioprism-registry alone gets \
         none of it.",
    )
}

/// Every report, in blueprint module order.
pub fn all() -> Vec<ServiceReport> {
    vec![
        world_builder(),
        decision_compiler(),
        mutation_runtime(),
        matched_evaluator(),
        adaptive_scheduler(),
        context_compiler(),
        registry_backend(),
    ]
}

/// The report for one contract, where the contract names an operation.
pub fn report_for(id: ContractId) -> Option<ServiceReport> {
    all().into_iter().find(|report| report.contract == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use std::collections::BTreeSet;

    #[test]
    fn every_operation_contract_has_a_report() {
        let reported: BTreeSet<ContractId> = all().iter().map(|report| report.contract).collect();
        let expected: BTreeSet<ContractId> = ContractId::ALL
            .into_iter()
            .filter(|id| id.is_operation())
            .collect();
        assert_eq!(reported, expected);
    }

    #[test]
    fn every_report_names_an_entry_point_and_at_least_one_crate() {
        for report in all() {
            assert!(!report.entry_point.is_empty(), "{}", report.contract);
            assert!(!report.crates.is_empty(), "{}", report.contract);
        }
    }

    #[test]
    fn every_report_cites_its_evidence() {
        for report in all() {
            assert!(
                report.evidence.len() >= 3,
                "{} carries {} note(s); a verdict a reviewer cannot argue with is not an audit",
                report.contract,
                report.evidence.len()
            );
        }
    }

    #[test]
    fn every_reported_field_name_belongs_to_its_contract() {
        for report in all() {
            let contract = catalog::contract_for(report.contract).expect("a transcribed contract");
            let inputs: BTreeSet<&str> = contract.request.paths().into_iter().collect();
            let outputs: BTreeSet<&str> = contract.response.paths().into_iter().collect();
            for field in &report.accepts {
                assert!(
                    inputs.contains(field.as_str()),
                    "{}: {field:?} is not an input of the contract",
                    report.contract
                );
            }
            for field in &report.produces {
                assert!(
                    outputs.contains(field.as_str()),
                    "{}: {field:?} is not an output of the contract",
                    report.contract
                );
            }
        }
    }

    #[test]
    fn every_enforced_invariant_index_exists_in_its_contract() {
        for report in all() {
            let contract = catalog::contract_for(report.contract).expect("a transcribed contract");
            for index in &report.enforces {
                assert!(
                    (1..=contract.invariants.len()).contains(index),
                    "{}: invariant {index} is out of range",
                    report.contract
                );
            }
        }
    }

    #[test]
    fn nothing_in_the_workspace_writes_to_an_artifact_store_from_inside_a_service() {
        for report in all() {
            assert!(
                !report.effects.contains(&Effect::WritesArtifactStore),
                "{} was recorded as writing its own output; the systemic finding is that none of \
                 them do",
                report.contract
            );
        }
    }

    #[test]
    fn report_for_finds_operations_and_nothing_else() {
        assert!(report_for(ContractId::AdaptiveScheduler).is_some());
        assert!(report_for(ContractId::DomainBoundaries).is_none());
    }
}
