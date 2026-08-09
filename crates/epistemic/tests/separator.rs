//! Blueprint 43.29: separators, leak refusal, and exactness checked against a centralised answer.

use bioprism_epistemic::separator::{
    centralised, collect, local_message, separator, structure, transport_cost, Exactness,
    FactorGraph, FactorTable, LocalOutcome, ObstructionKind, Partition, SeparatorMessage,
};
use bioprism_epistemic::theorem::{separator_exact, Applicability};
use bioprism_epistemic::EpistemicError;
use std::collections::{BTreeMap, BTreeSet};

fn table(id: &str, scope: &[&str], values: &[f64]) -> FactorTable {
    FactorTable::new(
        id,
        scope.iter().map(|s| (*s).to_string()).collect(),
        values.to_vec(),
    )
    .expect("well-formed factor")
}

fn agents(pairs: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
    pairs
        .iter()
        .map(|(agent, factors)| {
            (
                (*agent).to_string(),
                factors.iter().map(|f| (*f).to_string()).collect(),
            )
        })
        .collect()
}

/// A chain: imaging(lesion, mapping) — lineage(mapping, specimen) — molecular(specimen, call).
fn tree_graph() -> FactorGraph {
    FactorGraph::new(vec![
        table("lesion_prior", &["lesion"], &[0.3, 0.7]),
        table("lesion_mapping", &["lesion", "mapping"], &[0.8, 0.2, 0.35, 0.65]),
        table(
            "mapping_specimen",
            &["mapping", "specimen"],
            &[0.6, 0.4, 0.25, 0.75],
        ),
        table("specimen_call", &["specimen", "call"], &[0.9, 0.1, 0.2, 0.8]),
    ])
    .expect("unique factor ids")
}

fn tree_partition(graph: &FactorGraph) -> Partition {
    Partition::new(
        graph,
        agents(&[
            ("imaging", &["lesion_prior", "lesion_mapping"]),
            ("lineage", &["mapping_specimen"]),
            ("molecular", &["specimen_call"]),
        ]),
    )
    .expect("disjoint assignment")
}

#[test]
fn the_separator_of_two_agents_is_exactly_their_shared_variables() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);

    let shared = separator(&graph, &partition, "imaging", "lineage").expect("computable");
    assert_eq!(shared, BTreeSet::from(["mapping".to_string()]));

    let none = separator(&graph, &partition, "imaging", "molecular").expect("computable");
    assert!(
        none.is_empty(),
        "the imaging and molecular agents share nothing and must exchange nothing"
    );
}

#[test]
fn a_message_carrying_a_variable_outside_the_separator_is_rejected_as_a_leak() {
    let allowed = BTreeSet::from(["mapping".to_string()]);
    let outcome = SeparatorMessage::new(
        "imaging",
        "lineage",
        &allowed,
        vec!["mapping".to_string(), "lesion".to_string()],
        vec![0.1, 0.2, 0.3, 0.4],
        vec!["lesion_mapping".to_string()],
        1.0,
    );
    assert!(
        matches!(
            outcome,
            Err(EpistemicError::VariableOutsideSeparator { .. })
        ),
        "a private local variable cannot ride along as a harmless extra field"
    );
}

#[test]
fn separator_message_composition_matches_the_centralised_answer_on_a_tree() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);
    let report = structure(&graph, &partition).expect("checkable");
    assert_eq!(report.exactness, Exactness::ExactOnTree);

    let query = vec!["call".to_string()];
    let collected = collect(&graph, &partition, "molecular", &query).expect("collectable");
    let truth = centralised(&graph, &query).expect("enumerable");

    assert!(collected.obstructions.is_empty());
    for (got, want) in collected.marginal.iter().zip(&truth) {
        assert!(
            (got - want).abs() < 1e-12,
            "one collection pass on a junction structure must reproduce the exact aggregate: \
             {collected:?} against {truth:?}"
        );
    }
}

#[test]
fn exactness_is_claimed_only_through_the_theorem_gate() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);
    let report = structure(&graph, &partition).expect("checkable");
    assert!(matches!(
        separator_exact(&report),
        Applicability::Applies { .. }
    ));
}

#[test]
fn a_cyclic_partition_is_labelled_approximate_and_its_answer_differs_from_centralised() {
    let graph = FactorGraph::new(vec![
        table("ab", &["a", "b"], &[0.6, 0.4, 0.3, 0.7]),
        table("bc", &["b", "c"], &[0.55, 0.45, 0.2, 0.8]),
        table("ca", &["c", "a"], &[0.7, 0.3, 0.35, 0.65]),
    ])
    .expect("unique ids");
    let partition = Partition::new(
        &graph,
        agents(&[("x", &["ab"]), ("y", &["bc"]), ("z", &["ca"])]),
    )
    .expect("disjoint");

    let report = structure(&graph, &partition).expect("checkable");
    assert_eq!(report.exactness, Exactness::LoopyApproximate);
    assert!(report.cycle.is_some());
    assert!(matches!(
        separator_exact(&report),
        Applicability::DoesNotApply { .. }
    ));

    let query = vec!["a".to_string()];
    let collected = collect(&graph, &partition, "x", &query).expect("collectable");
    let truth = centralised(&graph, &query).expect("enumerable");
    let diverges = collected
        .marginal
        .iter()
        .zip(&truth)
        .any(|(got, want)| (got - want).abs() > 1e-9);
    assert!(
        diverges,
        "if one pass on a cycle happened to be exact, the fixture would not discriminate the two \
         regimes and the tree result above would prove nothing"
    );
}

#[test]
fn a_factor_assigned_to_two_agents_is_refused() {
    let graph = tree_graph();
    let outcome = Partition::new(
        &graph,
        agents(&[
            ("imaging", &["lesion_prior", "lesion_mapping"]),
            ("lineage", &["lesion_mapping", "mapping_specimen"]),
            ("molecular", &["specimen_call"]),
        ]),
    );
    assert!(
        matches!(outcome, Err(EpistemicError::FactorAssignedTwice { .. })),
        "a factor counted twice is multiplied into the product twice and every number downstream \
         is wrong with nothing to indicate it"
    );
}

#[test]
fn an_agent_with_no_factors_is_refused() {
    let graph = tree_graph();
    let outcome = Partition::new(
        &graph,
        agents(&[
            ("imaging", &["lesion_prior", "lesion_mapping"]),
            ("idle", &[]),
            ("lineage", &["mapping_specimen"]),
            ("molecular", &["specimen_call"]),
        ]),
    );
    assert!(matches!(outcome, Err(EpistemicError::EmptyAgent { .. })));
}

#[test]
fn a_locally_inconsistent_agent_raises_an_obstruction_instead_of_a_table_of_zeros() {
    let graph = FactorGraph::new(vec![
        table("contradiction", &["p"], &[0.0, 0.0]),
        table("downstream", &["p", "q"], &[0.5, 0.5, 0.5, 0.5]),
    ])
    .expect("unique ids");
    let partition = Partition::new(
        &graph,
        agents(&[("local", &["contradiction"]), ("other", &["downstream"])]),
    )
    .expect("disjoint");

    let outcome =
        local_message(&graph, &partition, "local", "other", &[]).expect("message or obstruction");
    match outcome {
        LocalOutcome::Obstruction(obstruction) => {
            assert_eq!(obstruction.kind, ObstructionKind::LocallyInconsistent);
            assert_eq!(obstruction.provenance, vec!["contradiction".to_string()]);
        }
        LocalOutcome::Message(message) => panic!(
            "a table of zeros reads downstream as an absence of evidence, not a contradiction: \
             {message:?}"
        ),
    }
}

#[test]
fn separator_traffic_is_smaller_than_sharing_every_factor_table() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);
    let query = vec!["call".to_string()];
    let collected = collect(&graph, &partition, "molecular", &query).expect("collectable");
    let cost = transport_cost(&graph, &partition, &collected).expect("measurable");

    assert_eq!(cost.agents, 3);
    assert_eq!(cost.messages, 2);
    assert!(
        cost.ratio < 1.0,
        "the separator protocol must beat transcript sharing on this partition: {} bytes of \
         messages against {} bytes of transcript",
        cost.message_bytes,
        cost.transcript_bytes
    );
}

#[test]
fn a_factor_table_of_the_wrong_width_is_refused() {
    let outcome = FactorTable::new(
        "wrong",
        vec!["a".to_string(), "b".to_string()],
        vec![0.1, 0.2, 0.3],
    );
    assert!(matches!(
        outcome,
        Err(EpistemicError::FactorTableShape { .. })
    ));
}

#[test]
fn a_factor_repeating_a_variable_in_its_scope_is_refused() {
    let outcome = FactorTable::new(
        "repeated",
        vec!["a".to_string(), "a".to_string()],
        vec![0.1, 0.2, 0.3, 0.4],
    );
    assert!(matches!(
        outcome,
        Err(EpistemicError::RepeatedVariableInScope { .. })
    ));
}

#[test]
fn a_query_variable_the_root_does_not_hold_is_refused_rather_than_widening_the_root() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);
    let outcome = collect(
        &graph,
        &partition,
        "molecular",
        &["lesion".to_string()],
    );
    assert!(matches!(
        outcome,
        Err(EpistemicError::UnknownIdentifier { .. })
    ));
}

#[test]
fn every_message_carries_a_validity_key_that_two_identical_messages_share() {
    let graph = tree_graph();
    let partition = tree_partition(&graph);
    let query = vec!["call".to_string()];
    let first = collect(&graph, &partition, "molecular", &query).expect("collectable");
    let second = collect(&graph, &partition, "molecular", &query).expect("collectable");

    assert_eq!(first.messages.len(), second.messages.len());
    for (a, b) in first.messages.iter().zip(&second.messages) {
        assert_eq!(
            a.validity_key, b.validity_key,
            "two runs of the same partition must produce the same content address or a fusion \
             cannot be replayed"
        );
        assert!(!a.validity_key.is_empty());
    }
}
