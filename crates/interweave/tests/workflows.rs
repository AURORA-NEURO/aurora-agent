//! 23.33: the six workflows, the deliverables they owe, and the one stated effect envelope.

use bioprism_fabric::effect::{Effect, EffectKind, Scope};
use bioprism_interweave::adapter::{AdapterProfile, CarriedSurface, Protocol};
use bioprism_interweave::workflow::{
    catalogue, outstanding_deliverables, Deliverable, ReferenceWorkflow, WorkflowId, WorkflowRole,
    CLAIM_REPRODUCTION_SHARED_STATE, SOFTWARE_REPAIR_EVALUATION_BOUNDARIES,
};
use std::collections::BTreeSet;

#[test]
fn the_catalogue_holds_the_six_workflows_the_blueprint_names() {
    let catalogue = catalogue();
    assert_eq!(catalogue.len(), 6);
    let ids: BTreeSet<WorkflowId> = catalogue.iter().map(|w| w.id).collect();
    assert_eq!(ids, WorkflowId::ALL.into_iter().collect());
}

#[test]
fn every_reference_workflow_in_this_workspace_owes_all_nine_deliverables() {
    let catalogue = catalogue();
    for workflow in &catalogue {
        assert_eq!(
            workflow.owed().len(),
            9,
            "{:?} unexpectedly delivers something",
            workflow.id
        );
        assert!(!workflow.complete());
    }
    let total: usize = outstanding_deliverables(&catalogue).values().sum();
    assert_eq!(total, 54);
}

#[test]
fn workflow_numbers_are_one_through_six_without_a_gap() {
    let numbers: BTreeSet<u8> = WorkflowId::ALL.into_iter().map(WorkflowId::number).collect();
    assert_eq!(numbers, (1u8..=6).collect());
}

#[test]
fn every_workflow_names_at_least_five_roles() {
    for id in WorkflowId::ALL {
        assert!(id.roles().len() >= 5, "{id:?} names too few roles");
    }
}

#[test]
fn two_workflows_place_a_human_in_a_named_role() {
    let with_humans: Vec<WorkflowId> = WorkflowId::ALL
        .into_iter()
        .filter(|id| id.roles().iter().any(|role| role.contains("human")))
        .collect();
    assert_eq!(
        with_humans,
        vec![
            WorkflowId::IncidentResponse,
            WorkflowId::EvidenceGroundedPolicyComparison,
        ]
    );
}

#[test]
fn the_biomedical_audit_is_the_only_workflow_with_a_stated_effect_envelope() {
    let constrained: Vec<WorkflowId> = WorkflowId::ALL
        .into_iter()
        .filter(|id| !id.forbidden_effects().is_empty())
        .collect();
    assert_eq!(constrained, vec![WorkflowId::BiomedicalResearchDataAudit]);
}

#[test]
fn a_biomedical_audit_role_declaring_external_publish_contradicts_its_own_envelope() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::BiomedicalResearchDataAudit)
        .with_role(
            WorkflowRole::new("cohort auditor")
                .performing(Effect::new(EffectKind::ExternalPublish, Scope::Undeclared)),
        );
    let violations = workflow.envelope_violations();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].role, "cohort auditor");
    assert_eq!(violations[0].kind, EffectKind::ExternalPublish);
}

#[test]
fn a_biomedical_audit_role_reading_artifacts_is_within_the_envelope() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::BiomedicalResearchDataAudit)
        .with_role(
            WorkflowRole::new("format reader")
                .performing(Effect::new(EffectKind::ArtifactRead, Scope::Undeclared))
                .performing(Effect::new(EffectKind::FilesystemRead, Scope::Undeclared)),
        );
    assert!(workflow.envelope_violations().is_empty());
}

#[test]
fn the_same_effect_in_a_workflow_with_no_stated_envelope_is_not_a_violation() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::ReliableSoftwareRepair).with_role(
        WorkflowRole::new("patcher")
            .performing(Effect::new(EffectKind::ExternalPublish, Scope::Undeclared)),
    );
    assert!(workflow.envelope_violations().is_empty());
}

#[test]
fn multiple_envelope_violations_are_reported_in_a_stable_order() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::BiomedicalResearchDataAudit)
        .with_role(
            WorkflowRole::new("privacy monitor")
                .performing(Effect::new(EffectKind::NetworkWrite, Scope::Undeclared)),
        )
        .with_role(
            WorkflowRole::new("cohort auditor")
                .performing(Effect::new(EffectKind::ExternalPublish, Scope::Undeclared)),
        );
    let first = workflow.envelope_violations();
    let second = workflow.envelope_violations();
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
}

fn adapter_at(name: &str, top: CarriedSurface) -> AdapterProfile {
    AdapterProfile::new(Protocol::A2A, name)
        .carrying(CarriedSurface::StructuredMessages)
        .carrying(CarriedSurface::Artifacts)
        .carrying(top)
}

#[test]
fn two_opaque_adapters_do_not_satisfy_the_two_adapter_deliverable() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::DatasetTransformationMolecule)
        .with_adapter(AdapterProfile::new(Protocol::Cli, "pty-a"))
        .with_adapter(AdapterProfile::new(Protocol::Cli, "pty-b"));
    assert!(!workflow.adapter_deliverable());
    assert!(workflow.owed().contains(&Deliverable::TwoParticipantAdapters));
}

#[test]
fn two_graded_adapters_satisfy_the_two_adapter_deliverable() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::DatasetTransformationMolecule)
        .with_adapter(adapter_at("a2a", CarriedSurface::TaskLifecycle))
        .with_adapter(adapter_at("mcp", CarriedSurface::TaskLifecycle));
    assert!(workflow.adapter_deliverable());
    assert!(!workflow.owed().contains(&Deliverable::TwoParticipantAdapters));
}

#[test]
fn one_graded_adapter_is_not_two() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::DatasetTransformationMolecule)
        .with_adapter(adapter_at("a2a", CarriedSurface::TaskLifecycle));
    assert!(!workflow.adapter_deliverable());
}

#[test]
fn a_workflow_delivering_everything_owes_nothing() {
    let workflow = Deliverable::ALL
        .into_iter()
        .fold(
            ReferenceWorkflow::from_blueprint(WorkflowId::IncidentResponse),
            ReferenceWorkflow::delivering,
        )
        .with_adapter(adapter_at("a2a", CarriedSurface::TaskLifecycle))
        .with_adapter(adapter_at("mcp", CarriedSurface::TaskLifecycle));
    assert!(workflow.complete());
    assert!(workflow.owed().is_empty());
}

#[test]
fn declaring_the_two_adapter_deliverable_without_adapters_does_not_satisfy_it() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::IncidentResponse)
        .delivering(Deliverable::TwoParticipantAdapters);
    assert!(workflow.owed().contains(&Deliverable::TwoParticipantAdapters));
}

#[test]
fn replacing_a_role_keeps_the_workflows_role_count_from_the_blueprint() {
    let workflow = ReferenceWorkflow::from_blueprint(WorkflowId::ReliableSoftwareRepair).with_role(
        WorkflowRole::new("patcher")
            .performing(Effect::new(EffectKind::ArtifactWrite, Scope::Undeclared)),
    );
    assert_eq!(
        workflow.roles.len(),
        WorkflowId::ReliableSoftwareRepair.roles().len()
    );
    let patcher = workflow
        .roles
        .iter()
        .find(|role| role.name == "patcher")
        .expect("patcher present");
    assert_eq!(patcher.effects.len(), 1);
}

#[test]
fn every_workflow_lists_its_distinctive_behaviours() {
    for id in WorkflowId::ALL {
        assert!(
            id.distinctive_behaviours().len() >= 5,
            "{id:?} lists too few behaviours"
        );
    }
}

#[test]
fn the_recorded_prose_lists_carry_the_counts_the_blueprint_gives() {
    assert_eq!(CLAIM_REPRODUCTION_SHARED_STATE.len(), 5);
    assert_eq!(SOFTWARE_REPAIR_EVALUATION_BOUNDARIES.len(), 6);
    assert_eq!(Deliverable::ALL.len(), 9);
}
