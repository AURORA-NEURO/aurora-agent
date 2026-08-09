//! The seven §40 service contracts, written down.
//!
//! Each function transcribes one blueprint module: its Inputs become the request descriptor, its
//! Outputs the response descriptor, its "Non-negotiable invariants" the invariant list verbatim,
//! and its "Failure semantics" bullets the failure modes, each bound to exactly one
//! [`ErrorClass`]. Field names are the blueprint's own phrases in snake case, so a reader can put
//! the module beside the code and check the transcription line by line. That is the point: a
//! contract nobody can check against its source is a paraphrase.
//!
//! # Where the transcription had to make a decision
//!
//! Three places, all of them recorded here rather than buried.
//!
//! **The error class of each failure mode is not in the blueprint.** 40.36 defines a taxonomy and
//! the seven modules list failures as bare noun phrases — "oracle unstable", "stale graph",
//! "private metadata leak". Nothing connects the two. The binding below is this crate's reading,
//! and the reading a caller will disagree with first is `Indeterminate`: an unstable oracle and an
//! ambiguous decision boundary are classified as *the science is unknown*, not *the run failed*,
//! because 40.36's fourth invariant says exactly that and because the repair is different.
//!
//! **40.18 disagrees with itself about its inputs.** Its graph snapshot draws four; its Inputs list
//! names five, the extra being `oracle references`. It is transcribed as optional, which is the
//! only reading under which both statements are true.
//!
//! **40.25 lists an API among its outputs.** "expansion API" is not an artifact a response can
//! carry; it is a second operation. It is transcribed as a response field because dropping it
//! would hide the problem, and named in the audit as a place the contract is not build-ready.
//!
//! # Effects are transcribed from the Execution path, not the Persistence section
//!
//! Every module has a "Persistence and state" list, and every one of those lists includes tables,
//! logs and caches that are plainly the deployment's business rather than the operation's. Taking
//! them as declared effects would make every contract a writer and the least-authority clause
//! vacuous. So an effect is declared when a numbered step of the Execution path requires it — 40.25
//! step 6 is "validate/sign artifacts", so the context compiler declares a store write, and the
//! consequences of that for `bioprism-fiber` are in [`crate::audit`].

use crate::contract::{
    descriptor, optional, required, required_list, ContractId, Delivery, Effect, Idempotency,
    ServiceContract,
};
use crate::error::{ErrorClass, FailureMode, ServicesError};
use bioprism_governance::CompatibilityMode;

/// Contract documents are read, never re-emitted, so an unknown field must survive a round trip
/// through an older reader without moving the document's digest. That is preserve-and-forward, the
/// same mode the three shipped wire formats declare.
const MODE: CompatibilityMode = CompatibilityMode::PreserveAndForward;

/// 40.18 — compile manifests, fragments, artifacts, actions, policies and oracle hooks into an
/// immutable world release.
pub fn world_builder() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::WorldBuilder,
        descriptor(
            "bioprism-world-build-request/0.1",
            MODE,
            vec![
                required("world_manifest"),
                required_list("adapter_outputs"),
                required_list("action_definitions"),
                required("visibility_reveal_policy"),
                optional("oracle_references"),
            ],
        )?,
        descriptor(
            "bioprism-world-build-response/0.1",
            MODE,
            vec![
                required("versioned_bioworld"),
                required("world_graph_and_event_ledger"),
                required("build_report"),
                required("world_card"),
            ],
        )?,
    )
    .invariants(&[
        "Build is deterministic under pinned inputs.",
        "World release is immutable.",
        "Hidden/evaluator state is physically separated.",
        "Every action and observation has declared semantics.",
    ])
    .failing(vec![
        FailureMode::result("identity conflict", ErrorClass::InvalidInput),
        FailureMode::result("temporal inconsistency", ErrorClass::ContractViolation),
        FailureMode::result("hidden state reachable", ErrorClass::ContractViolation),
        FailureMode::result("unsupported action", ErrorClass::InvalidInput),
        FailureMode::result("dependency not pinned", ErrorClass::InvalidInput),
    ])
    .delivering(Delivery::Job, Idempotency::ContentAddressed)
    .touching(&[
        Effect::ReadsGraph,
        Effect::WritesArtifactStore,
        Effect::AppendsEventLedger,
    ])
    .build()
}

/// 40.20 — extract, minimise, version and package causally important decisions from trajectories.
pub fn decision_compiler() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::DecisionCompiler,
        descriptor(
            "bioprism-decision-compile-request/0.1",
            MODE,
            vec![
                required("world_run_trace"),
                required("candidate_decision_boundary"),
                required("outcome_and_oracle_evidence"),
                required("compiler_policy"),
            ],
        )?,
        descriptor(
            "bioprism-decision-compile-response/0.1",
            MODE,
            vec![
                required("biodecision_cell"),
                required_list("candidate_action_set"),
                required("minimal_context_state"),
                required("divergence_and_fidelity_report"),
            ],
        )?,
    )
    .invariants(&[
        "Cell preserves the tested defect.",
        "Acceptable actions are set-valued.",
        "Evaluator-only state remains hidden.",
        "Minimization records every removal intervention.",
    ])
    .failing(vec![
        FailureMode::result("boundary ambiguous", ErrorClass::Indeterminate),
        FailureMode::result("state not replayable", ErrorClass::ContractViolation),
        FailureMode::result("oracle unstable", ErrorClass::Indeterminate),
        FailureMode::result("minimization changes defect", ErrorClass::ContractViolation),
        FailureMode::result("action set incomplete", ErrorClass::ContractViolation),
    ])
    .delivering(Delivery::Job, Idempotency::ContentAddressed)
    .touching(&[
        Effect::ReadsGraph,
        Effect::ReadsEvaluatorState,
        Effect::ExecutesSandbox,
        Effect::WritesArtifactStore,
    ])
    .build()
}

/// 40.22 — apply validated mutations while preserving explicit lineage and semantics.
pub fn mutation_runtime() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::MutationRuntime,
        descriptor(
            "bioprism-mutate-request/0.1",
            MODE,
            vec![
                required("parent_world_or_cell"),
                required("mutation_program_and_seed"),
                required_list("preconditions"),
                required_list("oracle_postconditions"),
            ],
        )?,
        descriptor(
            "bioprism-mutate-response/0.1",
            MODE,
            vec![
                required("descendant_world_or_cell"),
                required_list("lineage_edges"),
                required("semantic_validation_report"),
                required("effective_diversity_features"),
            ],
        )?,
    )
    .invariants(&[
        "Every descendant has exactly identified parents and operator versions.",
        "Semantics-preserving claims are tested.",
        "Controlled semantic mutations change the expected result as declared.",
        "Failed descendants are not counted.",
    ])
    .failing(vec![
        FailureMode::result("precondition false", ErrorClass::ContractViolation),
        FailureMode::result(
            "operator touches undeclared state",
            ErrorClass::ContractViolation,
        ),
        FailureMode::result("postcondition ambiguous", ErrorClass::Indeterminate),
        FailureMode::result("duplicate/near-duplicate descendant", ErrorClass::Conflict),
    ])
    .delivering(Delivery::Immediate, Idempotency::ContentAddressed)
    .touching(&[
        Effect::ReadsGraph,
        Effect::WritesArtifactStore,
        Effect::AppendsEventLedger,
    ])
    .build()
}

/// 40.23 — run architecture variants from identical decision states and attribute the difference.
pub fn matched_evaluator() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::MatchedEvaluator,
        descriptor(
            "bioprism-evaluate-request/0.1",
            MODE,
            vec![
                required("biodecision_cell"),
                required_list("architecture_manifests"),
                required("randomization_repetition_plan"),
                required("oracle_and_metric_plan"),
            ],
        )?,
        descriptor(
            "bioprism-evaluate-response/0.1",
            MODE,
            vec![
                required_list("paired_executions"),
                required("component_effects"),
                required("cost_and_failure_traces"),
                required("signed_result_bundle"),
            ],
        )?,
    )
    .invariants(&[
        "World/cell state is identical across matched arms.",
        "Only declared architecture dimensions vary.",
        "Caches and model randomness are recorded.",
        "Parent clustering is preserved.",
    ])
    .failing(vec![
        FailureMode::result("state mismatch", ErrorClass::ContractViolation),
        FailureMode::result("provider drift", ErrorClass::ContractViolation),
        FailureMode::result("arm-specific hidden cache", ErrorClass::ContractViolation),
        FailureMode::result("partial arm failure", ErrorClass::Unavailable),
        FailureMode::projection("statistical plan violation", ErrorClass::ContractViolation),
    ])
    .delivering(Delivery::Job, Idempotency::KeyedWrite)
    .touching(&[
        Effect::ReadsGraph,
        Effect::ReadsEvaluatorState,
        Effect::ExecutesSandbox,
        Effect::WritesArtifactStore,
        Effect::AppendsEventLedger,
    ])
    .build()
}

/// 40.24 — select a high-information, coverage-safe subset under cost and release constraints.
pub fn adaptive_scheduler() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::AdaptiveScheduler,
        descriptor(
            "bioprism-schedule-request/0.1",
            MODE,
            vec![
                required_list("candidate_cells_and_lineage"),
                required("architecture_change_fingerprint"),
                required("historical_results_posterior"),
                required("budget_and_mandatory_coverage"),
            ],
        )?,
        descriptor(
            "bioprism-schedule-response/0.1",
            MODE,
            vec![
                required_list("ordered_evaluation_plan"),
                required_list("selection_explanations"),
                required("stopping_decision"),
                required("coverage_and_uncertainty_report"),
            ],
        )?,
    )
    .invariants(&[
        "Safety and protected coverage cannot be optimized away.",
        "Dependent descendants do not masquerade as independent evidence.",
        "Selection policy is pinned before outcomes.",
        "Stopping rules are predeclared.",
    ])
    .failing(vec![
        FailureMode::projection("posterior misspecification", ErrorClass::Indeterminate),
        FailureMode::result("lineage missing", ErrorClass::InvalidInput),
        FailureMode::result("starvation of rare strata", ErrorClass::ContractViolation),
        FailureMode::projection("cost estimate drift", ErrorClass::Indeterminate),
        FailureMode::result("adaptive overfitting", ErrorClass::ContractViolation),
    ])
    .delivering(Delivery::Immediate, Idempotency::Pure)
    .touching(&[Effect::ReadsGraph])
    .build()
}

/// 40.25 — the Section 39 compiler as a deterministic, inspectable local service.
///
/// The most load-bearing of the seven: its sufficiency certificate is the artifact three
/// implementations agree on, and that agreement is the platform's claim to being checkable by
/// somebody who does not trust the publisher.
pub fn context_compiler() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::ContextCompiler,
        descriptor(
            "bioprism-context-compile-request/0.1",
            MODE,
            vec![
                required("world_cell_role_references"),
                required("goal_and_obligation_contract"),
                required("policy"),
                required("token_tool_privacy_budgets"),
            ],
        )?,
        descriptor(
            "bioprism-context-compile-response/0.1",
            MODE,
            vec![
                required("plan"),
                required("capsule"),
                required("omission_ledger"),
                required("sufficiency_certificate"),
                required("candidate_decision_trace"),
                required("expansion_api"),
            ],
        )?,
    )
    .invariants(&[
        "Mandatory closure is deterministic.",
        "Learned ranking cannot bypass policy or closure.",
        "All summaries resolve to sources.",
        "Full-context control remains reproducible.",
    ])
    .failing(vec![
        FailureMode::result("budget below closure", ErrorClass::ContractViolation),
        FailureMode::result("unresolvable source", ErrorClass::Unavailable),
        FailureMode::projection("summarizer inconsistency", ErrorClass::ContractViolation),
        FailureMode::projection("stale graph", ErrorClass::Stale),
        FailureMode::result("policy conflict", ErrorClass::PolicyDenied),
    ])
    .delivering(Delivery::Immediate, Idempotency::ContentAddressed)
    .touching(&[Effect::ReadsGraph, Effect::WritesArtifactStore])
    .build()
}

/// 40.27 — publish immutable packs, worlds, cells, architectures, results and attestations.
pub fn registry_backend() -> Result<ServiceContract, ServicesError> {
    ServiceContract::describing(
        ContractId::RegistryBackend,
        descriptor(
            "bioprism-publish-request/0.1",
            MODE,
            vec![
                required("signed_publication_bundle"),
                required_list("maintainer_review_decisions"),
                required_list("result_attestations"),
                required("visibility_policy"),
            ],
        )?,
        descriptor(
            "bioprism-publish-response/0.1",
            MODE,
            vec![
                required("versioned_registry_objects"),
                required_list("search_index_records"),
                required("health_and_trust_status"),
                required("public_private_graph_projections"),
            ],
        )?,
    )
    .invariants(&[
        "History is append-only.",
        "Publication status is distinct from scientific validity.",
        "Revocation does not erase history.",
        "Private objects never leak through search or counts.",
    ])
    .failing(vec![
        FailureMode::result("signature invalid", ErrorClass::InvalidInput),
        FailureMode::result("license/consent missing", ErrorClass::PolicyDenied),
        FailureMode::result("object dependency unavailable", ErrorClass::Unavailable),
        FailureMode::result("private metadata leak", ErrorClass::ContractViolation),
        FailureMode::result("revoked plugin", ErrorClass::PolicyDenied),
    ])
    .delivering(Delivery::Immediate, Idempotency::AppendOnly)
    .touching(&[
        Effect::ReadsGraph,
        Effect::WritesArtifactStore,
        Effect::AppendsEventLedger,
        Effect::WritesSearchIndex,
    ])
    .build()
}

/// The seven, in blueprint module order.
pub fn all() -> Vec<ServiceContract> {
    [
        world_builder(),
        decision_compiler(),
        mutation_runtime(),
        matched_evaluator(),
        adaptive_scheduler(),
        context_compiler(),
        registry_backend(),
    ]
    .into_iter()
    .map(|contract| contract.expect("a transcribed contract is well formed"))
    .collect()
}

/// The contract for one id, where the id names an operation.
pub fn contract_for(id: ContractId) -> Option<ServiceContract> {
    all().into_iter().find(|contract| contract.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Retryability;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn every_operation_module_has_a_transcribed_contract() {
        let transcribed: BTreeSet<ContractId> = all().iter().map(|contract| contract.id).collect();
        let expected: BTreeSet<ContractId> = ContractId::ALL
            .into_iter()
            .filter(|id| id.is_operation())
            .collect();
        assert_eq!(transcribed, expected);
        assert_eq!(all().len(), 7);
    }

    #[test]
    fn every_transcribed_failure_mode_carries_exactly_one_error_class() {
        for contract in all() {
            let mut classes: BTreeMap<&str, ErrorClass> = BTreeMap::new();
            for failure in &contract.failures {
                if let Some(previous) = classes.insert(failure.label.as_str(), failure.class) {
                    assert_eq!(
                        previous, failure.class,
                        "{} binds {:?} to two classes",
                        contract.id, failure.label
                    );
                }
            }
            assert!(
                !contract.failures.is_empty(),
                "{} declares no failure modes",
                contract.id
            );
        }
    }

    #[test]
    fn the_taxonomy_has_no_class_that_no_contract_can_raise() {
        let raised: BTreeSet<ErrorClass> = all()
            .iter()
            .flat_map(|contract| contract.error_classes())
            .collect();
        let unraised: Vec<ErrorClass> = ErrorClass::ALL
            .into_iter()
            .filter(|class| !raised.contains(class))
            .collect();
        assert_eq!(
            unraised,
            [ErrorClass::Usage, ErrorClass::Internal],
            "usage and internal belong to the invocation layer and to the unknown; no §40 module \
             lists a failure mode for either, and inventing one would be transcription fiction"
        );
    }

    #[test]
    fn every_transcribed_invariant_list_has_the_four_the_blueprint_states() {
        for contract in all() {
            assert_eq!(
                contract.invariants.len(),
                4,
                "{} should transcribe four non-negotiable invariants",
                contract.id
            );
        }
    }

    #[test]
    fn every_contract_declares_at_least_one_input_and_one_output() {
        for contract in all() {
            assert!(!contract.required_inputs().is_empty(), "{}", contract.id);
            assert!(!contract.declared_outputs().is_empty(), "{}", contract.id);
        }
    }

    #[test]
    fn the_world_builder_treats_oracle_references_as_optional_because_the_module_disagrees_with_itself(
    ) {
        let contract = world_builder().expect("transcribes");
        let field = contract
            .request
            .field("oracle_references")
            .expect("the fifth input is transcribed");
        assert!(
            !field.presence.is_required(),
            "40.18's graph snapshot draws four inputs and its Inputs list names five; optional is \
             the only reading under which both are true"
        );
        assert_eq!(contract.request.paths().len(), 6);
    }

    #[test]
    fn a_contract_that_writes_never_claims_purity() {
        for contract in all() {
            if contract.effects.iter().any(|effect| effect.is_write()) {
                assert!(
                    !contract.idempotency.is_pure(),
                    "{} writes and claims purity",
                    contract.id
                );
            }
        }
    }

    #[test]
    fn only_the_scheduler_is_a_pure_read() {
        let pure: Vec<ContractId> = all()
            .iter()
            .filter(|contract| contract.idempotency.is_pure())
            .map(|contract| contract.id)
            .collect();
        assert_eq!(pure, [ContractId::AdaptiveScheduler]);
    }

    #[test]
    fn the_two_contracts_that_may_read_evaluator_state_are_the_compiler_and_the_evaluator() {
        let readers: Vec<ContractId> = all()
            .iter()
            .filter(|contract| contract.effects.contains(&Effect::ReadsEvaluatorState))
            .map(|contract| contract.id)
            .collect();
        assert_eq!(
            readers,
            [ContractId::DecisionCompiler, ContractId::MatchedEvaluator],
            "both declare an invariant about keeping evaluator-only state hidden, which is only \
             meaningful for a service that can see it"
        );
    }

    #[test]
    fn an_unstable_oracle_is_indeterminate_in_every_contract_that_names_one() {
        for contract in all() {
            for failure in &contract.failures {
                if failure.label.contains("oracle unstable")
                    || failure.label.contains("postcondition ambiguous")
                    || failure.label.contains("boundary ambiguous")
                {
                    assert_eq!(
                        failure.class,
                        ErrorClass::Indeterminate,
                        "{}: {:?}",
                        contract.id,
                        failure.label
                    );
                    assert_eq!(failure.retryability(), Retryability::OnlyAfterCallerChange);
                }
            }
        }
    }

    #[test]
    fn only_one_transcribed_failure_leaves_the_underlying_result_intact_in_every_contract_but_two()
    {
        let projection_only: Vec<ContractId> = all()
            .iter()
            .filter(|contract| {
                contract
                    .failures
                    .iter()
                    .any(|failure| failure.invalidates == crate::error::Invalidates::Projection)
            })
            .map(|contract| contract.id)
            .collect();
        assert_eq!(
            projection_only,
            [
                ContractId::MatchedEvaluator,
                ContractId::AdaptiveScheduler,
                ContractId::ContextCompiler
            ],
            "the three contracts whose output is an aggregate over intact evidence are the only \
             ones that can fail without voiding it"
        );
    }

    #[test]
    fn every_contract_document_declares_preserve_and_forward() {
        for contract in all() {
            assert_eq!(contract.request.mode, CompatibilityMode::PreserveAndForward);
            assert_eq!(
                contract.response.mode,
                CompatibilityMode::PreserveAndForward
            );
        }
    }

    #[test]
    fn contract_for_finds_operations_and_nothing_else() {
        assert!(contract_for(ContractId::ContextCompiler).is_some());
        assert!(contract_for(ContractId::ServiceGraph).is_none());
    }
}
