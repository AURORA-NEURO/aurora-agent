//! This workspace, as a graph and as two deployments.
//!
//! [`ownership_table`] transcribes 40.04's nine rows; [`service_graph`] places the workspace's
//! crates in those domains and draws the calls the seven modules' own dependency lists imply;
//! [`alpha`] and [`hosted`] are 40.03's two diagrams.
//!
//! # Two things this file exists to make visible
//!
//! **40.04's "Does not own" column does not close.** Six of its entries name a concern that belongs
//! to another domain, and no row of the same table claims any of the six. The sharpest is *oracle
//! verdicts*: the Compiler disclaims them, the Oracle domain is credited with "evidence
//! establishment and uncertainty" — not verdicts — and in this workspace the value called
//! `OracleVerdict` is produced by `bioprism-fiber`, which sits in Context. Three domains touch it
//! and the table gives it to none of them.
//!
//! **The nine contracts do not close under their own dependency lists.** Six calls in
//! [`service_graph`] leave their domain with no contract to leave it under, and five of the six
//! land on Oracle or Runtime. 40.18, 40.20, 40.22 and 40.23 all name an oracle dependency; the
//! oracle runtime is 40.21, which is not one of the nine. A set of service contracts that cannot
//! type its own edges is a set with a hole in it, and the hole is named here rather than papered
//! over by inventing a contract nobody specified.

use crate::contract::ContractId;
use crate::graph::{
    Call, Concern, Disclaimer, Domain, Ownership, ServiceGraph, ServiceId, ServiceNode,
};
use crate::topology::{Deployment, Placement, Topology};

fn owns(domain: Domain, concerns: &[&str], disclaims: Vec<Disclaimer>) -> Ownership {
    Ownership {
        domain,
        owns: concerns.iter().map(|name| Concern::new(*name)).collect(),
        disclaims,
    }
}

fn elsewhere(concern: &str) -> Disclaimer {
    Disclaimer::OwnedElsewhere {
        concern: Concern::new(concern),
    }
}

fn prohibited(behaviour: &str) -> Disclaimer {
    Disclaimer::Prohibited {
        behaviour: behaviour.to_string(),
    }
}

/// 40.04's table, row for row.
pub fn ownership_table() -> Vec<Ownership> {
    vec![
        owns(
            Domain::BiologicalIr,
            &[
                "identity semantics",
                "specimen semantics",
                "assay semantics",
                "event semantics",
                "claim semantics",
            ],
            vec![elsewhere("storage backend"), elsewhere("UI")],
        ),
        owns(
            Domain::World,
            &[
                "visibility",
                "state transitions",
                "action catalog",
                "reveal",
            ],
            vec![elsewhere("model provider logic")],
        ),
        owns(
            Domain::Context,
            &["obligation graph", "projection", "omission/sufficiency"],
            vec![elsewhere("raw modality computations")],
        ),
        owns(
            Domain::Runtime,
            &["sandbox actions", "effects", "budgets", "replay"],
            vec![elsewhere("biological truth")],
        ),
        owns(
            Domain::Compiler,
            &["decision boundaries", "minimization", "mutation lineage"],
            vec![elsewhere("oracle verdicts")],
        ),
        owns(
            Domain::Oracle,
            &["evidence establishment and uncertainty"],
            vec![prohibited("hiding implementation failures")],
        ),
        owns(
            Domain::Evaluator,
            &["matched comparisons and statistics"],
            vec![prohibited("modifying worlds during scoring")],
        ),
        owns(
            Domain::Registry,
            &["immutable publication and discovery"],
            vec![prohibited("recomputing private data centrally")],
        ),
        owns(
            Domain::OncoWorld,
            &["neuro-oncology domain profiles"],
            vec![prohibited("core framework imports")],
        ),
    ]
}

fn node(
    name: &str,
    domain: Domain,
    implemented_by: &str,
    contract: Option<ContractId>,
) -> ServiceNode {
    ServiceNode {
        id: ServiceId::new(name),
        domain,
        implemented_by: Some(implemented_by.to_string()),
        contract,
    }
}

/// The eleven services the seven contracts and their dependency lists imply.
pub fn services() -> Vec<ServiceNode> {
    vec![
        node(
            "data-adapter",
            Domain::BiologicalIr,
            "bioprism-adapter",
            None,
        ),
        node(
            "world-builder",
            Domain::World,
            "bioprism-worldgen",
            Some(ContractId::WorldBuilder),
        ),
        node(
            "context-compiler",
            Domain::Context,
            "bioprism-fiber",
            Some(ContractId::ContextCompiler),
        ),
        node("sandbox-runtime", Domain::Runtime, "bioprism-runtime", None),
        node(
            "decision-compiler",
            Domain::Compiler,
            "bioprism-benchcompiler",
            Some(ContractId::DecisionCompiler),
        ),
        node(
            "mutation-runtime",
            Domain::Compiler,
            "bioprism-mutation",
            Some(ContractId::MutationRuntime),
        ),
        node("oracle-runtime", Domain::Oracle, "bioprism-oracle", None),
        node(
            "matched-evaluator",
            Domain::Evaluator,
            "bioprism-prism",
            Some(ContractId::MatchedEvaluator),
        ),
        node(
            "adaptive-scheduler",
            Domain::Evaluator,
            "bioprism-adaptive",
            Some(ContractId::AdaptiveScheduler),
        ),
        node(
            "registry-backend",
            Domain::Registry,
            "bioprism-registry",
            Some(ContractId::RegistryBackend),
        ),
        node("oncoworld-pack", Domain::OncoWorld, "bioprism-onco", None),
    ]
}

/// The workspace graph, with every edge the seven modules' dependency lists name.
pub fn service_graph() -> ServiceGraph {
    let adapter = ServiceId::new("data-adapter");
    let builder = ServiceId::new("world-builder");
    let context = ServiceId::new("context-compiler");
    let sandbox = ServiceId::new("sandbox-runtime");
    let compiler = ServiceId::new("decision-compiler");
    let mutation = ServiceId::new("mutation-runtime");
    let oracle = ServiceId::new("oracle-runtime");
    let evaluator = ServiceId::new("matched-evaluator");
    let scheduler = ServiceId::new("adaptive-scheduler");
    let registry = ServiceId::new("registry-backend");
    let onco = ServiceId::new("oncoworld-pack");

    let calls = vec![
        Call::internal(&builder, &adapter),
        Call::internal(&builder, &oracle),
        Call::under(&builder, &registry, ContractId::RegistryBackend),
        Call::under(&context, &builder, ContractId::WorldBuilder),
        Call::under(&compiler, &builder, ContractId::WorldBuilder),
        Call::internal(&compiler, &oracle),
        Call::internal(&compiler, &sandbox),
        Call::result(&sandbox, &compiler),
        Call::internal(&mutation, &compiler),
        Call::internal(&mutation, &oracle),
        Call::under(&evaluator, &compiler, ContractId::DecisionCompiler),
        Call::internal(&evaluator, &oracle),
        Call::under(&evaluator, &registry, ContractId::RegistryBackend),
        Call::internal(&scheduler, &evaluator),
        Call::under(&onco, &builder, ContractId::WorldBuilder),
    ];

    ServiceGraph::new(services(), calls, ownership_table())
}

fn placements(hosted: bool) -> Vec<(ServiceId, Placement)> {
    services()
        .into_iter()
        .map(|node| {
            let placement = match (hosted, node.id.as_str()) {
                (false, "sandbox-runtime") => Placement::Subprocess,
                (false, _) => Placement::InProcess,
                (true, "registry-backend") => Placement::ApiService,
                (true, _) => Placement::Worker,
            };
            (node.id, placement)
        })
        .collect()
}

/// 40.03's alpha topology: everything in process, the executor as a subprocess.
pub fn alpha() -> Deployment {
    Deployment::new(Topology::Alpha, placements(false))
}

/// 40.03's hosted topology: the same services behind an API and worker pools.
pub fn hosted() -> Deployment {
    Deployment::new(Topology::Hosted, placements(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::GraphError;
    use crate::topology::compare;
    use std::collections::BTreeSet;

    #[test]
    fn the_ownership_table_has_the_nine_domains_the_blueprint_lists() {
        let domains: Vec<Domain> = ownership_table().iter().map(|row| row.domain).collect();
        assert_eq!(domains, Domain::ALL.to_vec());
    }

    #[test]
    fn no_two_domains_claim_the_same_concern() {
        let conflicts: Vec<GraphError> = service_graph()
            .audit()
            .into_iter()
            .filter(|finding| matches!(finding, GraphError::OwnershipConflict { .. }))
            .collect();
        assert_eq!(conflicts, Vec::new());
    }

    #[test]
    fn every_concern_the_table_disclaims_as_owned_elsewhere_is_owned_by_nobody() {
        let orphans: Vec<String> = service_graph()
            .orphaned_concerns()
            .into_iter()
            .map(|concern| concern.as_str().to_string())
            .collect();
        assert_eq!(
            orphans,
            [
                "storage backend",
                "UI",
                "model provider logic",
                "raw modality computations",
                "biological truth",
                "oracle verdicts",
            ],
            "all six of 40.04's owned-elsewhere disclaimers point outside its own table"
        );
    }

    #[test]
    fn four_of_the_nine_disclaimers_are_prohibitions_no_structural_check_can_reach() {
        let prohibitions: Vec<String> = ownership_table()
            .iter()
            .flat_map(|row| row.disclaims.clone())
            .filter_map(|disclaimer| match disclaimer {
                Disclaimer::Prohibited { behaviour } => Some(behaviour),
                Disclaimer::OwnedElsewhere { .. } => None,
            })
            .collect();
        assert_eq!(
            prohibitions,
            [
                "hiding implementation failures",
                "modifying worlds during scoring",
                "recomputing private data centrally",
                "core framework imports",
            ]
        );
    }

    #[test]
    fn the_workspace_call_graph_has_no_cycle() {
        assert_eq!(service_graph().cycles(), Vec::new());
    }

    #[test]
    fn six_calls_leave_their_domain_with_no_contract_among_the_nine_to_leave_it_under() {
        let crossings: Vec<String> = service_graph()
            .undeclared_crossings()
            .into_iter()
            .map(|(caller, callee)| format!("{caller} -> {callee}"))
            .collect();
        assert_eq!(
            crossings,
            [
                "world-builder -> data-adapter",
                "world-builder -> oracle-runtime",
                "decision-compiler -> oracle-runtime",
                "decision-compiler -> sandbox-runtime",
                "mutation-runtime -> oracle-runtime",
                "matched-evaluator -> oracle-runtime",
            ]
        );
    }

    #[test]
    fn four_of_the_six_undeclared_crossings_land_on_the_oracle_runtime() {
        let to_oracle = service_graph()
            .undeclared_crossings()
            .into_iter()
            .filter(|(_, callee)| callee.as_str() == "oracle-runtime")
            .count();
        assert_eq!(
            to_oracle, 4,
            "40.18, 40.20, 40.22 and 40.23 each name an oracle dependency, and the oracle runtime \
             is 40.21, which is not one of the nine"
        );
    }

    #[test]
    fn every_declared_crossing_names_a_contract_its_callee_answers() {
        let wrong: Vec<GraphError> = service_graph()
            .audit()
            .into_iter()
            .filter(|finding| matches!(finding, GraphError::WrongContract { .. }))
            .collect();
        assert_eq!(wrong, Vec::new());
    }

    #[test]
    fn the_executor_returning_to_the_core_is_not_reported_as_a_cycle() {
        let graph = service_graph();
        assert!(graph
            .cycles()
            .iter()
            .all(|finding| !matches!(finding, GraphError::Cycle { .. })));
        assert!(!graph
            .audit()
            .iter()
            .any(|finding| matches!(finding, GraphError::UnpairedResult { .. })));
    }

    #[test]
    fn the_alpha_topology_requires_no_hosted_service() {
        assert_eq!(alpha().hosted_dependencies(), Vec::<&ServiceId>::new());
    }

    #[test]
    fn the_alpha_and_hosted_topologies_run_the_same_service_set() {
        assert_eq!(compare(&alpha(), &hosted()), Vec::new());
        let local = alpha();
        let placed: BTreeSet<&ServiceId> = local.services().into_iter().collect();
        assert_eq!(placed.len(), services().len());
    }

    #[test]
    fn every_service_names_the_crate_that_implements_it() {
        for node in services() {
            assert!(node.implemented_by.is_some(), "{}", node.id);
        }
    }

    #[test]
    fn exactly_four_services_answer_no_contract_among_the_nine() {
        let uncovered: Vec<String> = services()
            .into_iter()
            .filter(|node| node.contract.is_none())
            .map(|node| node.id.as_str().to_string())
            .collect();
        assert_eq!(
            uncovered,
            [
                "data-adapter",
                "sandbox-runtime",
                "oracle-runtime",
                "oncoworld-pack"
            ],
            "their contracts are 40.17, the runtime section, 40.21 and 40.26; none is one of the \
             nine, and four of the eleven services in the graph are therefore untyped"
        );
    }
}
