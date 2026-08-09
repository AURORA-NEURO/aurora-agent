//! 36.10: the physical-action boundary.

use bioprism_bioethics::action::{
    refer, ActionKind, ActionPlan, Authorisation, Effect, PlannedStep,
};
use bioprism_bioethics::BioethicsError;
use bioprism_onco::{OncoError, OutputUse, ResearchBoundary};

fn mixed_plan(declared_use: OutputUse) -> ActionPlan {
    ActionPlan::new("resistance-screen", declared_use)
        .with_step(PlannedStep::new(
            ActionKind::Analysis,
            "reanalyse the cohort",
        ))
        .with_step(PlannedStep::new(
            ActionKind::Simulation,
            "simulate the perturbation",
        ))
        .with_step(PlannedStep::new(
            ActionKind::LiquidHandler,
            "plate the dilution series",
        ))
}

fn authorised() -> Authorisation {
    Authorisation::new()
        .approved_by("named principal investigator")
        .safety_reviewed_by("institutional biosafety committee")
}

#[test]
fn the_six_scope_kinds_act_on_the_world_and_the_three_permitted_activities_do_not() {
    let physical = [
        ActionKind::RemoteLabApi,
        ActionKind::LiquidHandler,
        ActionKind::SampleConsumption,
        ActionKind::CellCultureOrAnimalWork,
        ActionKind::ChemicalHandling,
        ActionKind::InstrumentControl,
    ];
    for kind in physical {
        assert_eq!(
            kind.effect(),
            Effect::OnTheWorld,
            "{kind} is in 36.10's scope"
        );
        assert!(kind.is_physical());
    }
    for kind in [
        ActionKind::Simulation,
        ActionKind::Analysis,
        ActionKind::ProtocolPlanning,
    ] {
        assert_eq!(kind.effect(), Effect::InSilico);
        assert!(!kind.is_physical());
    }
    assert_eq!(ActionKind::ALL.len(), 9);
}

#[test]
fn a_physical_step_is_split_out_rather_than_taking_the_safe_steps_down_with_it() {
    let plan = mixed_plan(OutputUse::MethodDevelopment);
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("method development is inside the research boundary");

    assert_eq!(
        disposition.in_silico_steps().len(),
        2,
        "36.10 keeps simulation and analysis; a whole-plan refusal would destroy them"
    );
    assert_eq!(disposition.physical_steps().len(), 1);
    assert!(disposition.requires_physical_authorisation());
}

#[test]
fn a_plan_for_individual_clinical_use_is_refused_before_a_single_step_is_read() {
    let plan = mixed_plan(OutputUse::TreatmentRecommendation);
    let error = plan
        .partition(&ResearchBoundary::research_only())
        .expect_err("a physical action taken to direct one person's care is the compound failure");
    assert!(
        matches!(
            error,
            BioethicsError::Onco(OncoError::OutsideResearchBoundary { .. })
        ),
        "the refusal must be bioprism-onco's own rather than a paraphrase: {error}"
    );
}

#[test]
fn a_referral_requires_both_of_the_human_acts_and_names_the_one_that_is_missing() {
    let plan = mixed_plan(OutputUse::MethodDevelopment);
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("inside the boundary");

    let only_approved = Authorisation::new().approved_by("named principal investigator");
    let error = refer(&disposition, &only_approved).expect_err("one of the two is not both");
    match error {
        BioethicsError::PhysicalStepUnauthorised { missing, .. } => {
            assert_eq!(missing, "institutional safety review");
        }
        other => panic!("expected the missing act to be named: {other}"),
    }

    let neither = Authorisation::new();
    let error = refer(&disposition, &neither).expect_err("neither act was performed");
    match error {
        BioethicsError::PhysicalStepUnauthorised { missing, .. } => {
            assert_eq!(missing, "human approval and institutional safety review");
        }
        other => panic!("expected both acts to be named: {other}"),
    }
}

#[test]
fn an_unattributed_approval_is_not_an_approval() {
    let plan = mixed_plan(OutputUse::MethodDevelopment);
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("inside the boundary");
    let blank = Authorisation::new()
        .approved_by("   ")
        .safety_reviewed_by("institutional biosafety committee");
    let error = refer(&disposition, &blank).expect_err("a blank name records nothing");
    assert!(matches!(
        error,
        BioethicsError::UnattributedAuthorisation { .. }
    ));
}

#[test]
fn a_plan_with_no_physical_step_cannot_produce_a_referral() {
    let plan = ActionPlan::new("in-silico-only", OutputUse::CohortAnalysis).with_step(
        PlannedStep::new(ActionKind::Analysis, "reanalyse the cohort"),
    );
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("inside the boundary");
    assert!(!disposition.requires_physical_authorisation());
    let error = refer(&disposition, &authorised())
        .expect_err("a referral for nothing implies something was authorised");
    assert!(matches!(
        error,
        BioethicsError::PhysicalStepUnauthorised {
            physical_steps: 0,
            ..
        }
    ));
}

#[test]
fn a_referral_carries_the_physical_steps_and_nothing_that_ran() {
    let plan = mixed_plan(OutputUse::MethodDevelopment);
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("inside the boundary");
    let referral = refer(&disposition, &authorised()).expect("both human acts are recorded");

    assert_eq!(referral.steps().len(), 1);
    assert_eq!(referral.steps()[0].kind, ActionKind::LiquidHandler);
    assert_eq!(referral.human_approver(), "named principal investigator");
    assert_eq!(
        referral.institutional_safety_review_body(),
        "institutional biosafety committee"
    );
    assert!(
        bioprism_bioethics::action::PhysicalReferral::STATEMENT.contains("does not execute"),
        "the statement travelling with a referral must say the workspace did not act"
    );
}

#[test]
fn a_partition_always_reports_both_lists_so_absence_cannot_be_read_as_emptiness() {
    let plan = ActionPlan::new("empty", OutputUse::QualityControl);
    let disposition = plan
        .partition(&ResearchBoundary::research_only())
        .expect("inside the boundary");
    assert!(disposition.in_silico_steps().is_empty());
    assert!(disposition.physical_steps().is_empty());
    assert_eq!(disposition.subject(), "empty");
}
