//! The deliverable: does the workspace satisfy the nine contracts as written?
//!
//! No. All nine diverge, and the divergences are worth more than a pass would have been. They fall
//! into four groups, and only the first is a defect in the code.
//!
//! **The code is missing something the contract asks for.** `bioprism-worldgen` produces no world
//! version, build report or world card; `bioprism-adaptive` has no architecture change fingerprint
//! and cannot score regression relevance without one; `bioprism-mutation` identifies parents
//! exactly and operator versions not at all; `bioprism-prism` cannot distinguish an arm that failed
//! from an arm that never ran. These are tickets.
//!
//! **The code is stricter than the contract, and the contract should move.**
//! `bioprism-benchcompiler` derives candidate decision boundaries instead of accepting one, so a
//! caller cannot smuggle in a boundary the trace does not support. 40.20 lists the boundary as an
//! input. The code is right.
//!
//! **The contract puts an effect in the wrong place.** Six of the seven modules place an artifact
//! write inside the service — "emit world release", "package and validate cell", "bundle",
//! "validate/sign artifacts", "publish". Not one crate in this workspace does it. Every one returns
//! its artifacts as values and leaves persistence to its caller, which is what makes them usable as
//! functions, testable without a store, and runnable in the alpha topology 40.03 insists on. The
//! contracts are wrong about where the boundary is, uniformly, and this is the largest single
//! finding of the audit.
//!
//! **The contract cannot be satisfied because it does not say enough.** 40.25 lists an "expansion
//! API" among its *outputs*, which is a second operation rather than an artifact. 40.04's "does not
//! own" column names six concerns no row of its own table owns. 40.03 draws the executor returning
//! to the core without distinguishing a call from its answer, so read literally its alpha topology
//! contains a cycle. Nobody can build to these as written.
//!
//! # Where a build-ready contract turned out not to be build-ready
//!
//! Recorded once here, since the point of §40 being marked build-ready is that this list should be
//! empty.
//!
//! | Module | What is missing |
//! |---|---|
//! | 40.03 | No distinction between a request edge and the result returning along it, so the alpha diagram is a cycle |
//! | 40.03 | No statement of which services may be co-located, only that they can be |
//! | 40.04 | Six "does not own" entries name concerns the table gives to nobody, `oracle verdicts` among them |
//! | 40.04 | Four more are prohibitions on behaviour, not ownership, and no structural check can observe them |
//! | 40.18 | The graph snapshot draws four inputs; the Inputs list names five |
//! | 40.25 | An API is listed as an output |
//! | all seven | No failure mode carries a retry classification, though the module template demands one of every emitted failure |
//! | all seven | No request or response field types, cardinalities or identity rules — only bullet names |
//! | the nine | Four of the eleven services their own dependency lists imply answer no contract among them; 40.21 in particular is named by four of the seven and is not one of the nine |

use crate::catalog;
use crate::conformance::{Conformance, ConformanceCheck};
use crate::contract::ContractId;
use crate::implementations;
use crate::workspace;
use serde::{Deserialize, Serialize};

/// One contract's verdict, with the divergences named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub contract: ContractId,
    pub module_id: String,
    pub title: String,
    /// The crates that between them answer the contract.
    pub crates: Vec<String>,
    pub verdict: Conformance,
    /// Each divergence, named. Empty exactly when the verdict is [`Conformance::Satisfied`].
    pub divergences: Vec<String>,
    /// Citations, so a reviewer can disagree with a verdict by disagreeing with a fact.
    pub evidence: Vec<String>,
}

impl AuditEntry {
    pub fn divergence_count(&self) -> usize {
        self.divergences.len()
    }
}

fn named(check: &ConformanceCheck) -> Vec<String> {
    let mut named = Vec::new();
    for field in &check.missing_inputs {
        named.push(format!("input not accepted: {field}"));
    }
    for field in &check.missing_outputs {
        named.push(format!("output not produced: {field}"));
    }
    for label in &check.untyped_failures {
        named.push(format!("failure mode with no typed error: {label}"));
    }
    for effect in &check.undeclared_effects {
        named.push(format!("undeclared effect: {effect:?}"));
    }
    for effect in &check.unperformed_effects {
        named.push(format!("declared effect not performed: {effect:?}"));
    }
    for invariant in &check.unenforced_invariants {
        named.push(format!("invariant not enforced: {invariant}"));
    }
    if let Some((contract, actual)) = check.delivery_mismatch {
        named.push(format!(
            "delivery: contract says {contract:?}, implementation is {actual:?}"
        ));
    }
    if let Some((contract, actual)) = check.idempotency_mismatch {
        named.push(format!(
            "idempotency: contract says {contract:?}, implementation is {actual:?}"
        ));
    }
    named
}

/// 40.03, judged against the workspace's two topologies.
fn service_graph_entry() -> AuditEntry {
    let graph = workspace::service_graph();
    let mut divergences = vec![
        "the hosted topology has no implementation: every service in the workspace is an \
         in-process library, and no crate places one behind an API or a job ledger"
            .to_string(),
        "40.03 draws the executor returning to the core without distinguishing a call from its \
         answer; read literally the alpha topology contains a two-cycle, and this crate had to \
         add an edge kind the module does not have"
            .to_string(),
    ];
    if !graph.cycles().is_empty() {
        divergences.push(format!(
            "{} cycle(s) in the workspace call graph",
            graph.cycles().len()
        ));
    }
    AuditEntry {
        contract: ContractId::ServiceGraph,
        module_id: ContractId::ServiceGraph.module_id().to_string(),
        title: ContractId::ServiceGraph.title().to_string(),
        crates: Vec::new(),
        verdict: Conformance::Diverges,
        divergences,
        evidence: vec![
            "The alpha half is satisfied: crate::workspace::alpha places every service in process \
             or as a subprocess, and hosted_dependencies is empty."
                .to_string(),
            "crate::topology::compare finds no difference between the two service sets, so the \
             half of 40.03 that can be checked without a deployment holds."
                .to_string(),
        ],
    }
}

/// 40.04, judged against the workspace's ownership table and call graph.
fn domain_boundaries_entry() -> AuditEntry {
    let graph = workspace::service_graph();
    let orphans = graph.orphaned_concerns();
    let crossings = graph.undeclared_crossings();
    let uncovered = workspace::services()
        .into_iter()
        .filter(|node| node.contract.is_none())
        .count();

    let mut divergences = vec![
        format!(
            "{} concern(s) disclaimed as owned elsewhere that no domain in the table owns: {}",
            orphans.len(),
            orphans
                .iter()
                .map(|concern| concern.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "{} cross-domain call(s) with no contract among the nine to make them under",
            crossings.len()
        ),
        format!(
            "{uncovered} of {} services in the graph answer no contract among the nine",
            workspace::services().len()
        ),
    ];
    divergences.push(
        "four of the nine \"does not own\" entries are prohibitions on behaviour rather than \
         ownership statements, and no structural check can observe any of them"
            .to_string(),
    );

    AuditEntry {
        contract: ContractId::DomainBoundaries,
        module_id: ContractId::DomainBoundaries.module_id().to_string(),
        title: ContractId::DomainBoundaries.title().to_string(),
        crates: Vec::new(),
        verdict: Conformance::Diverges,
        divergences,
        evidence: vec![
            "No concern in 40.04's table is claimed by two domains, so the owns column is at least \
             consistent."
                .to_string(),
            "Every crossing that does carry a contract names one its callee answers; the \
             divergences are all absences, none is a mislabelling."
                .to_string(),
        ],
    }
}

/// The nine, in blueprint module order.
pub fn audit() -> Vec<AuditEntry> {
    let mut entries = vec![service_graph_entry(), domain_boundaries_entry()];
    for contract in catalog::all() {
        let report = implementations::report_for(contract.id)
            .expect("every transcribed contract has a report");
        let check = contract.check(&report);
        entries.push(AuditEntry {
            contract: contract.id,
            module_id: contract.module_id().to_string(),
            title: contract.id.title().to_string(),
            crates: report.crates.clone(),
            verdict: check.verdict(),
            divergences: named(&check),
            evidence: report.evidence.clone(),
        });
    }
    entries.sort_by_key(|entry| entry.module_id.clone());
    entries
}

/// The counts, for a headline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSummary {
    pub satisfied: usize,
    pub diverges: usize,
    pub not_implemented: usize,
    pub divergences: usize,
}

impl AuditSummary {
    pub fn of(entries: &[AuditEntry]) -> Self {
        AuditSummary {
            satisfied: count(entries, Conformance::Satisfied),
            diverges: count(entries, Conformance::Diverges),
            not_implemented: count(entries, Conformance::NotImplemented),
            divergences: entries.iter().map(AuditEntry::divergence_count).sum(),
        }
    }

    pub fn total(&self) -> usize {
        self.satisfied + self.diverges + self.not_implemented
    }

    pub fn headline(&self) -> String {
        format!(
            "{} contract(s): {} satisfied, {} diverging, {} not implemented; {} named divergences",
            self.total(),
            self.satisfied,
            self.diverges,
            self.not_implemented,
            self.divergences
        )
    }
}

fn count(entries: &[AuditEntry], verdict: Conformance) -> usize {
    entries
        .iter()
        .filter(|entry| entry.verdict == verdict)
        .count()
}

/// The audit as a table, for a doc or a report.
pub fn markdown_table() -> String {
    let mut out =
        String::from("| § | contract | crates | verdict | divergences |\n|---|---|---|---|---:|\n");
    for entry in audit() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            entry.module_id,
            entry.title,
            if entry.crates.is_empty() {
                "—".to_string()
            } else {
                entry.crates.join(", ")
            },
            entry.verdict,
            entry.divergence_count()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_audit_covers_all_nine_contracts() {
        let entries = audit();
        assert_eq!(entries.len(), 9);
        let covered: Vec<ContractId> = entries.iter().map(|entry| entry.contract).collect();
        for id in ContractId::ALL {
            assert!(covered.contains(&id), "{id} is not audited");
        }
    }

    #[test]
    fn every_one_of_the_nine_contracts_diverges_and_none_is_unimplemented() {
        let summary = AuditSummary::of(&audit());
        assert_eq!(summary.satisfied, 0);
        assert_eq!(summary.not_implemented, 0);
        assert_eq!(summary.diverges, 9);
        assert_eq!(
            summary.total(),
            9,
            "the finding is that all nine diverge; it ships as it is"
        );
    }

    #[test]
    fn a_diverging_entry_always_names_at_least_one_divergence() {
        for entry in audit() {
            match entry.verdict {
                Conformance::Satisfied => assert!(entry.divergences.is_empty()),
                _ => assert!(
                    !entry.divergences.is_empty(),
                    "{} diverges without saying how",
                    entry.contract
                ),
            }
        }
    }

    #[test]
    fn every_entry_carries_evidence_a_reviewer_can_contradict() {
        for entry in audit() {
            assert!(
                entry.evidence.len() >= 2,
                "{} carries {} note(s)",
                entry.contract,
                entry.evidence.len()
            );
        }
    }

    #[test]
    fn the_registry_is_the_closest_of_the_seven_operations_to_its_contract_and_the_builder_the_furthest(
    ) {
        let mut operations: Vec<(ContractId, usize)> = audit()
            .into_iter()
            .filter(|entry| entry.contract.is_operation())
            .map(|entry| (entry.contract, entry.divergence_count()))
            .collect();
        operations.sort_by_key(|(_, count)| *count);
        assert_eq!(
            operations.first().copied(),
            Some((ContractId::RegistryBackend, 3)),
            "bioprism-registry answers 40.27 in all but the visibility layer its own docs say is \
             not built"
        );
        assert_eq!(
            operations.last().copied(),
            Some((ContractId::WorldBuilder, 16)),
            "40.18 is the furthest by a factor of five: bioprism-worldgen is a fixture generator, \
             not a build service"
        );
    }

    #[test]
    fn six_of_the_seven_operation_contracts_place_an_artifact_write_the_workspace_does_not_perform()
    {
        let with_write = audit()
            .into_iter()
            .filter(|entry| {
                entry.divergences.iter().any(|divergence| {
                    divergence == "declared effect not performed: WritesArtifactStore"
                })
            })
            .count();
        assert_eq!(
            with_write, 6,
            "every operation contract but the scheduler puts the write inside the service, and no \
             crate in the workspace does; the contracts are uniformly wrong about the boundary"
        );
    }

    #[test]
    fn the_context_compiler_cannot_raise_three_of_its_five_declared_failures() {
        let entry = audit()
            .into_iter()
            .find(|entry| entry.contract == ContractId::ContextCompiler)
            .expect("audited");
        let untyped: Vec<&String> = entry
            .divergences
            .iter()
            .filter(|divergence| divergence.starts_with("failure mode with no typed error"))
            .collect();
        assert_eq!(untyped.len(), 3, "{:#?}", entry.divergences);
        assert!(entry
            .divergences
            .iter()
            .any(|divergence| divergence.contains("policy conflict")));
    }

    #[test]
    fn the_mutation_runtime_does_not_enforce_the_invariant_about_operator_versions() {
        let entry = audit()
            .into_iter()
            .find(|entry| entry.contract == ContractId::MutationRuntime)
            .expect("audited");
        assert!(
            entry.divergences.iter().any(|divergence| divergence
                .starts_with("invariant not enforced:")
                && divergence.contains("operator versions")),
            "{:#?}",
            entry.divergences
        );
    }

    #[test]
    fn the_summary_headline_states_the_result_without_rounding_it_up() {
        let headline = AuditSummary::of(&audit()).headline();
        assert!(
            headline.starts_with("9 contract(s): 0 satisfied, 9 diverging"),
            "{headline}"
        );
    }

    #[test]
    fn the_markdown_table_has_a_row_per_contract() {
        let table = markdown_table();
        assert_eq!(table.lines().count(), 11, "two header lines and nine rows");
        assert!(table.contains("40.25"));
        assert!(table.contains("bioprism-fiber"));
    }

    #[test]
    fn an_audit_entry_round_trips_through_json() {
        let entries = audit();
        let value = serde_json::to_value(&entries).expect("serialises");
        let back: Vec<AuditEntry> = serde_json::from_value(value).expect("deserialises");
        assert_eq!(back, entries);
    }
}
