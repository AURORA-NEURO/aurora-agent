//! 27.11. A perfectly executed analysis of the wrong material.

use bioprism_scope::Timestamp;
use bioprism_worldfactory::error::IdentityProgramRefusal;
use bioprism_worldfactory::lineage::{
    apply, audit, Artifact, DetectionRoute, ExpectedDetection, Finding, FingerprintCheck,
    IdentityOperation, IdentityProgram, SpecimenId, SpecimenNode, SpecimenRegistry,
};
use serde_json::json;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("fixture timestamps are well formed")
}

/// Two donors, each with a tube and two artifacts, all identity evidence on file.
fn registry_with_fingerprints() -> SpecimenRegistry {
    SpecimenRegistry::new()
        .with_specimen(
            SpecimenNode::new("S1", "donor-a", 1_000)
                .with_content("sequence", json!("aaaa"))
                .fingerprinted("donor-a"),
        )
        .with_specimen(
            SpecimenNode::new("S2", "donor-b", 1_000)
                .with_content("sequence", json!("bbbb"))
                .fingerprinted("donor-b"),
        )
        .with_artifact(Artifact::new("A1", "S1", "sequencing"))
        .with_artifact(Artifact::new("A2", "S1", "imaging"))
        .with_artifact(Artifact::new("A3", "S2", "sequencing"))
        .with_artifact(Artifact::new("A4", "S2", "imaging"))
        .seal()
}

/// The same world with the identity evidence withheld — 27.11 workflow step 4's other branch.
fn registry_without_fingerprints() -> SpecimenRegistry {
    SpecimenRegistry::new()
        .with_specimen(SpecimenNode::new("S1", "donor-a", 1_000).with_content("sequence", json!("aaaa")))
        .with_specimen(SpecimenNode::new("S2", "donor-b", 1_000).with_content("sequence", json!("bbbb")))
        .with_artifact(Artifact::new("A1", "S1", "sequencing"))
        .with_artifact(Artifact::new("A2", "S1", "imaging"))
        .seal()
}

#[test]
fn renaming_a_specimen_does_not_move_its_content_digest_which_is_why_a_relabel_is_invisible_to_dedup(
) {
    let before = SpecimenNode::new("S1", "donor-a", 1_000).with_content("sequence", json!("aaaa"));
    let after = SpecimenNode::new("S1", "donor-z", 1_000).with_content("sequence", json!("aaaa"));
    assert_eq!(
        before.content_digest(),
        after.content_digest(),
        "the exclusion that stops renaming from manufacturing instances is the same exclusion \
         that stops a content digest from catching a mix-up"
    );
}

#[test]
fn a_relabel_with_the_identity_evidence_withheld_cannot_demand_detection() {
    let program = IdentityProgram::new(
        "p-relabel",
        IdentityOperation::Relabel {
            specimen: SpecimenId::new("S1"),
            to_label: "donor-z".to_string(),
        },
        ExpectedDetection::Detect,
    );
    assert!(matches!(
        apply(&registry_without_fingerprints(), &program)
            .expect_err("nothing in this world could reveal it"),
        IdentityProgramRefusal::UndetectableByConstruction { .. }
    ));
}

#[test]
fn the_same_relabel_is_a_legitimate_program_when_the_expected_response_is_abstention() {
    let program = IdentityProgram::new(
        "p-relabel",
        IdentityOperation::Relabel {
            specimen: SpecimenId::new("S1"),
            to_label: "donor-z".to_string(),
        },
        ExpectedDetection::Abstain,
    );
    let (_, delta) = apply(&registry_without_fingerprints(), &program)
        .expect("withholding evidence is legal; demanding detection of it is not");
    assert!(delta.routes.is_empty());
}

#[test]
fn a_relabel_is_caught_by_identity_evidence_when_the_world_exposes_it() {
    let program = IdentityProgram::new(
        "p-relabel",
        IdentityOperation::Relabel {
            specimen: SpecimenId::new("S1"),
            to_label: "donor-z".to_string(),
        },
        ExpectedDetection::Detect,
    );
    let (_, delta) = apply(&registry_with_fingerprints(), &program)
        .expect("the fingerprint contradicts the new label");
    assert!(delta.routes.contains(&DetectionRoute::GeneticFingerprint));
}

#[test]
fn a_partially_propagated_swap_leaves_artifacts_that_disagree_with_the_tube() {
    let program = IdentityProgram::new(
        "p-swap",
        IdentityOperation::Swap {
            a: SpecimenId::new("S1"),
            b: SpecimenId::new("S2"),
        },
        ExpectedDetection::Detect,
    )
    .propagating_to("A1");
    let (after, delta) =
        apply(&registry_with_fingerprints(), &program).expect("a findable mix-up");
    assert!(delta
        .routes
        .contains(&DetectionRoute::CrossArtifactDisagreement));
    assert!(audit(&after)
        .findings
        .iter()
        .any(|f| matches!(f, Finding::ArtifactsDisagree { .. })));
}

#[test]
fn a_swap_propagated_to_every_artifact_with_no_identity_evidence_leaves_nothing_to_find() {
    let program = IdentityProgram::new(
        "p-swap",
        IdentityOperation::Swap {
            a: SpecimenId::new("S1"),
            b: SpecimenId::new("S2"),
        },
        ExpectedDetection::Detect,
    )
    .propagating_to("A1")
    .propagating_to("A2");
    assert!(matches!(
        apply(&registry_without_fingerprints(), &program)
            .expect_err("the author erased the trace they seeded"),
        IdentityProgramRefusal::PropagatedEverywhere { .. }
    ));
}

#[test]
fn a_swap_across_the_access_boundary_is_a_governance_event_not_a_benchmark() {
    let split = SpecimenRegistry::new()
        .with_specimen(SpecimenNode::new("S1", "donor-a", 1_000).in_domain("public"))
        .with_specimen(SpecimenNode::new("S2", "donor-b", 1_000).in_domain("controlled"));
    let program = IdentityProgram::new(
        "p-swap",
        IdentityOperation::Swap {
            a: SpecimenId::new("S1"),
            b: SpecimenId::new("S2"),
        },
        ExpectedDetection::Detect,
    );
    assert!(matches!(
        apply(&split, &program).expect_err("material may not move between access domains"),
        IdentityProgramRefusal::CrossesAccessBoundary { .. }
    ));
}

#[test]
fn a_duplicate_under_a_second_identifier_is_findable_from_content_alone() {
    let program = IdentityProgram::new(
        "p-dup",
        IdentityOperation::Duplicate {
            source: SpecimenId::new("S1"),
            as_id: SpecimenId::new("S1-copy"),
        },
        ExpectedDetection::Detect,
    );
    let (after, delta) = apply(&registry_without_fingerprints(), &program)
        .expect("two identifiers holding the same material");
    assert!(delta.routes.contains(&DetectionRoute::DuplicateContent));
    assert!(audit(&after)
        .findings
        .iter()
        .any(|f| matches!(f, Finding::DuplicateContent { .. })));
}

#[test]
fn contamination_changes_the_host_so_its_unmoved_artifacts_stop_agreeing_with_it() {
    let program = IdentityProgram::new(
        "p-contam",
        IdentityOperation::Contamination {
            host: SpecimenId::new("S1"),
            contaminant: SpecimenId::new("S2"),
        },
        ExpectedDetection::Detect,
    );
    let (_, delta) = apply(&registry_without_fingerprints(), &program)
        .expect("the host's material is no longer what its slides were cut from");
    assert!(delta
        .routes
        .contains(&DetectionRoute::CrossArtifactDisagreement));
}

#[test]
fn aliquots_may_not_outweigh_the_parent_they_came_from() {
    let over_drawn = SpecimenRegistry::new()
        .with_specimen(SpecimenNode::new("P", "donor-a", 1_000))
        .with_specimen(SpecimenNode::new("C1", "donor-a", 700).derived_from("P"))
        .with_specimen(SpecimenNode::new("C2", "donor-a", 700).derived_from("P"));
    assert!(audit(&over_drawn)
        .findings
        .iter()
        .any(|f| matches!(f, Finding::MassNotConserved { .. })));
}

#[test]
fn a_specimen_that_is_its_own_ancestor_is_a_cycle_rather_than_a_deep_lineage() {
    let looped = SpecimenRegistry::new()
        .with_specimen(SpecimenNode::new("A", "donor-a", 10).derived_from("B"))
        .with_specimen(SpecimenNode::new("B", "donor-a", 10).derived_from("A"));
    assert!(audit(&looped)
        .findings
        .iter()
        .any(|f| matches!(f, Finding::LineageCycle { .. })));
}

#[test]
fn a_child_collected_before_its_parent_is_temporally_implausible() {
    let backwards = SpecimenRegistry::new()
        .with_specimen(
            SpecimenNode::new("P", "donor-a", 1_000).collected_at(at("2026-03-01T09:00:00Z")),
        )
        .with_specimen(
            SpecimenNode::new("C", "donor-a", 100)
                .derived_from("P")
                .collected_at(at("2026-02-01T09:00:00Z")),
        );
    assert!(audit(&backwards)
        .findings
        .iter()
        .any(|f| matches!(f, Finding::TemporalImplausibility { .. })));
}

#[test]
fn a_fingerprint_that_is_absent_is_not_a_fingerprint_that_matched() {
    let unchecked = SpecimenNode::new("S1", "donor-a", 10);
    let check = unchecked.fingerprint_check();
    assert!(matches!(
        check,
        FingerprintCheck::NoEvidenceAvailable { .. }
    ));
    assert!(
        !check.is_consistent(),
        "nobody ran it, so it has not passed"
    );

    let report = audit(&registry_without_fingerprints());
    assert_eq!(
        report.unchecked_specimens().len(),
        2,
        "the audit reports who was never checked, not only who failed"
    );
    assert!(
        report.is_clean(),
        "unchecked identity is not a finding; hiding that it is unchecked would be"
    );
}

#[test]
fn a_recorded_quantity_that_stops_matching_what_was_taken_is_findable_by_mass_balance() {
    let with_parent = SpecimenRegistry::new()
        .with_specimen(SpecimenNode::new("P", "donor-a", 1_000))
        .with_specimen(SpecimenNode::new("C", "donor-a", 100).derived_from("P"));
    let program = IdentityProgram::new(
        "p-mass",
        IdentityOperation::QuantityInconsistency {
            specimen: SpecimenId::new("C"),
            mass_ug: 5_000,
        },
        ExpectedDetection::Detect,
    );
    let (_, delta) = apply(&with_parent, &program).expect("the arithmetic no longer works");
    assert!(delta.routes.contains(&DetectionRoute::MassBalance));
}
