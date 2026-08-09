//! Invariants of the capability ontology, blueprint 03.09.
//!
//! The hierarchy is a DAG that a capability may not re-enter, `is_a` has exactly one
//! representation, and the non-hierarchical relations are read in the direction their semantics
//! demand rather than the direction they happen to be written in.

use bioprism_atlas::{
    AtlasError, CapabilityDimension, CapabilityFamily, CapabilityId, CapabilityNode,
    CapabilityOntology, RelationKind,
};

const VERSION: &str = "capability-ontology/2026-08-07";

fn cap(id: &str) -> CapabilityId {
    CapabilityId::parse(id).expect("valid capability identifier")
}

fn node(id: &str, family: CapabilityFamily) -> CapabilityNode {
    CapabilityNode::new(cap(id), id, family, CapabilityDimension::Competence)
}

#[test]
fn a_capability_that_is_its_own_ancestor_is_refused_rather_than_answered() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(node("a", CapabilityFamily::Verification).with_parent(cap("b")))
        .unwrap();
    ontology
        .insert(node("b", CapabilityFamily::Verification).with_parent(cap("c")))
        .unwrap();
    ontology
        .insert(node("c", CapabilityFamily::Verification).with_parent(cap("a")))
        .unwrap();

    assert!(ontology.is_own_ancestor(&cap("a")).unwrap());
    match ontology.validate() {
        Err(AtlasError::CyclicIsA { cycle, .. }) => {
            assert!(cycle.len() >= 4, "cycle should name the path: {cycle:?}");
            assert_eq!(cycle.first(), cycle.last());
        }
        other => panic!("expected a cycle refusal, got {other:?}"),
    }
}

#[test]
fn a_capability_that_declares_itself_its_own_parent_is_refused() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(node("a", CapabilityFamily::Memory).with_parent(cap("a")))
        .unwrap();
    assert!(matches!(
        ontology.validate(),
        Err(AtlasError::CyclicIsA { .. })
    ));
}

#[test]
fn an_ancestor_query_terminates_on_a_cyclic_hierarchy_instead_of_looping() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(node("a", CapabilityFamily::Memory).with_parent(cap("b")))
        .unwrap();
    ontology
        .insert(node("b", CapabilityFamily::Memory).with_parent(cap("a")))
        .unwrap();

    let ancestors = ontology.ancestors(&cap("a")).unwrap();
    assert!(ancestors.contains(&cap("a")));
    assert!(ancestors.contains(&cap("b")));
}

#[test]
fn ancestors_and_descendants_are_inverse_over_the_is_a_hierarchy() {
    let ontology = CapabilityOntology::from_nodes(
        VERSION,
        [
            node("root", CapabilityFamily::DomainReasoning),
            node("mid", CapabilityFamily::ToolUse).with_parent(cap("root")),
            node("leaf", CapabilityFamily::ToolUse).with_parent(cap("mid")),
        ],
    )
    .unwrap();

    assert!(ontology.ancestors(&cap("leaf")).unwrap().contains(&cap("root")));
    assert!(ontology.descendants(&cap("root")).unwrap().contains(&cap("leaf")));
    assert!(ontology.ancestors(&cap("root")).unwrap().is_empty());
    assert!(ontology.descendants(&cap("leaf")).unwrap().is_empty());
    assert_eq!(ontology.subtree(&cap("mid")).unwrap().len(), 2);
}

#[test]
fn a_capability_may_have_several_parents_without_forming_a_cycle() {
    let ontology = CapabilityOntology::from_nodes(
        VERSION,
        [
            node("verification", CapabilityFamily::Verification),
            node("tool_use", CapabilityFamily::ToolUse),
            node("verify_tool_result", CapabilityFamily::Verification)
                .with_parent(cap("verification"))
                .with_parent(cap("tool_use")),
        ],
    )
    .expect("a multi-parent DAG is legal");

    let ancestors = ontology.ancestors(&cap("verify_tool_result")).unwrap();
    assert_eq!(ancestors.len(), 2);
}

#[test]
fn a_dangling_is_a_parent_is_refused_before_any_query_can_use_it() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(node("child", CapabilityFamily::Recovery).with_parent(cap("absent")))
        .unwrap();
    assert!(matches!(
        ontology.validate(),
        Err(AtlasError::UnknownParent { .. })
    ));
}

#[test]
fn is_a_declared_as_a_loose_relation_is_refused_so_the_hierarchy_has_one_representation() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology.insert(node("parent", CapabilityFamily::Memory)).unwrap();
    ontology
        .insert(node("child", CapabilityFamily::Memory).with_relation(RelationKind::IsA, cap("parent")))
        .unwrap();
    assert!(matches!(
        ontology.validate(),
        Err(AtlasError::IsAOutsideHierarchy { .. })
    ));
}

#[test]
fn confounding_is_read_in_both_directions() {
    let ontology = CapabilityOntology::from_nodes(
        VERSION,
        [
            node("retrieval", CapabilityFamily::EvidenceAcquisition)
                .with_relation(RelationKind::ConfoundsWith, cap("context_compression")),
            node("context_compression", CapabilityFamily::ContextManagement),
        ],
    )
    .unwrap();

    assert!(ontology
        .confounded_with(&cap("retrieval"))
        .unwrap()
        .contains(&cap("context_compression")));
    assert!(ontology
        .confounded_with(&cap("context_compression"))
        .unwrap()
        .contains(&cap("retrieval")));
}

#[test]
fn a_safety_constraint_on_a_parent_binds_every_capability_beneath_it() {
    let ontology = CapabilityOntology::from_nodes(
        VERSION,
        [
            node("agent", CapabilityFamily::DomainReasoning),
            node("analysis", CapabilityFamily::ToolUse).with_parent(cap("agent")),
            node("boundary", CapabilityFamily::PrivacyAndSafety)
                .with_relation(RelationKind::SafetyConstraintOn, cap("agent")),
        ],
    )
    .unwrap();

    assert!(ontology
        .safety_constraints_on(&cap("analysis"))
        .unwrap()
        .contains(&cap("boundary")));
    assert!(ontology
        .safety_constraints_on(&cap("boundary"))
        .unwrap()
        .is_empty());
}

#[test]
fn a_relation_pointing_at_a_capability_that_does_not_exist_is_refused() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology
        .insert(
            node("a", CapabilityFamily::Coordination)
                .with_relation(RelationKind::Requires, cap("ghost")),
        )
        .unwrap();
    assert!(matches!(
        ontology.validate(),
        Err(AtlasError::UnknownRelationTarget { .. })
    ));
}

#[test]
fn an_empty_or_control_bearing_capability_identifier_is_refused() {
    assert!(matches!(
        CapabilityId::parse("   "),
        Err(AtlasError::EmptyCapabilityId)
    ));
    assert!(matches!(
        CapabilityId::parse("a\nb"),
        Err(AtlasError::ControlCharacterInCapabilityId(_))
    ));
}

#[test]
fn declaring_the_same_capability_twice_is_refused() {
    let mut ontology = CapabilityOntology::new(VERSION);
    ontology.insert(node("a", CapabilityFamily::Memory)).unwrap();
    assert!(matches!(
        ontology.insert(node("a", CapabilityFamily::ToolUse)),
        Err(AtlasError::DuplicateCapability { .. })
    ));
}
