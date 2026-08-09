//! Budget, resource and cost controller (blueprint 05.09).
//!
//! The claim these defend is that exhaustion *aborts*. A meter that applied what fit and reported
//! what completed would turn "ran out of tokens" into "scored poorly", and a benchmark that cannot
//! tell those apart is measuring its own funding.

use bioprism_ids::RunId;
use bioprism_runtime::{
    BudgetController, BudgetPlan, ChargeStatus, EffectKind, EffectPolicy, Host, InProcessWorld,
    Limit, RecordingHost, RuntimeError, RuntimeResource, Sandbox,
};

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

#[test]
fn budget_exhaustion_aborts_rather_than_truncating() {
    let plan = BudgetPlan::new().with(RuntimeResource::ModelTokens, Limit::hard(100));
    let mut budget = BudgetController::from_plan(&plan);

    budget
        .charge(RuntimeResource::ModelTokens, 90)
        .expect("within the ceiling");

    let error = budget
        .charge(RuntimeResource::ModelTokens, 20)
        .expect_err("110 does not fit under 100");
    match error {
        RuntimeError::BudgetExhausted {
            hard,
            used,
            requested,
            ..
        } => {
            assert_eq!(hard, 100);
            assert_eq!(used, 90);
            assert_eq!(requested, 20);
        }
        other => panic!("expected exhaustion, got {other}"),
    }

    assert_eq!(
        budget.used(RuntimeResource::ModelTokens),
        90,
        "a refused charge must not be partially applied; the remaining 10 are not silently spent"
    );
    assert_eq!(budget.aborted_on(), Some(RuntimeResource::ModelTokens));
}

#[test]
fn an_aborted_trial_cannot_resume_spending_on_any_resource() {
    let plan = BudgetPlan::new()
        .with(RuntimeResource::ModelTokens, Limit::hard(10))
        .with(RuntimeResource::ToolCalls, Limit::hard(10));
    let mut budget = BudgetController::from_plan(&plan);

    budget
        .charge(RuntimeResource::ModelTokens, 11)
        .expect_err("over the ceiling");

    let error = budget
        .charge(RuntimeResource::ToolCalls, 1)
        .expect_err("an aborted trial is over, not merely out of one resource");
    assert!(matches!(
        error,
        RuntimeError::AlreadyAborted {
            resource: RuntimeResource::ModelTokens
        }
    ));
}

#[test]
fn a_soft_limit_warns_and_lets_the_charge_through() {
    let plan = BudgetPlan::new()
        .with(RuntimeResource::ToolCalls, Limit::soft_then_hard(2, 5));
    let mut budget = BudgetController::from_plan(&plan);

    assert_eq!(
        budget.charge(RuntimeResource::ToolCalls, 2).expect("fits"),
        ChargeStatus::Within
    );
    assert_eq!(
        budget.charge(RuntimeResource::ToolCalls, 1).expect("fits"),
        ChargeStatus::SoftLimitCrossed
    );
    assert_eq!(
        budget.charge(RuntimeResource::ToolCalls, 1).expect("fits"),
        ChargeStatus::OverSoftLimit
    );

    assert_eq!(budget.warnings().len(), 1, "the crossing is warned about once");
    assert_eq!(budget.warnings()[0].soft, 2);
    assert_eq!(budget.used(RuntimeResource::ToolCalls), 4);
}

#[test]
fn an_undeclared_resource_cannot_be_charged() {
    let plan = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(5));
    let mut budget = BudgetController::from_plan(&plan);

    let error = budget
        .charge(RuntimeResource::CostMicros, 1)
        .expect_err("nothing budgeted money for this trial");
    assert!(matches!(
        error,
        RuntimeError::UndeclaredResource {
            resource: RuntimeResource::CostMicros
        }
    ));
    assert_eq!(
        budget.aborted_on(),
        None,
        "an undeclared charge is a plan error, not an exhausted trial"
    );
}

#[test]
fn a_child_budget_cannot_exceed_its_parents_remaining_allocation() {
    let parent_plan = BudgetPlan::new().with(RuntimeResource::ModelTokens, Limit::hard(100));
    let mut parent = BudgetController::from_plan(&parent_plan);
    parent
        .charge(RuntimeResource::ModelTokens, 60)
        .expect("within");

    let greedy = BudgetPlan::new().with(RuntimeResource::ModelTokens, Limit::hard(50));
    let error = parent
        .derive_child(&greedy)
        .expect_err("only 40 remain to give away");
    match error {
        RuntimeError::OverAllocatedChild {
            requested,
            available,
            ..
        } => {
            assert_eq!(requested, 50);
            assert_eq!(available, 40);
        }
        other => panic!("expected an over-allocated child, got {other}"),
    }
    assert_eq!(
        parent.used(RuntimeResource::ModelTokens),
        60,
        "a refused delegation changes nothing"
    );
}

#[test]
fn a_child_budget_is_deducted_so_two_children_cannot_share_the_same_headroom() {
    let parent_plan = BudgetPlan::new().with(RuntimeResource::ModelTokens, Limit::hard(100));
    let mut parent = BudgetController::from_plan(&parent_plan);
    let child_plan = BudgetPlan::new().with(RuntimeResource::ModelTokens, Limit::hard(60));

    let first = parent.derive_child(&child_plan).expect("60 of 100");
    assert_eq!(first.remaining(RuntimeResource::ModelTokens), 60);
    assert_eq!(parent.remaining(RuntimeResource::ModelTokens), 40);

    parent
        .derive_child(&child_plan)
        .expect_err("the same 60 cannot be handed out twice");
}

#[test]
fn a_child_cannot_be_given_a_resource_its_parent_never_had() {
    let parent_plan = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(10));
    let mut parent = BudgetController::from_plan(&parent_plan);
    let child_plan = BudgetPlan::new().with(RuntimeResource::CostMicros, Limit::hard(1));

    let error = parent
        .derive_child(&child_plan)
        .expect_err("a parent cannot delegate what it was never allocated");
    assert!(matches!(
        error,
        RuntimeError::UndeclaredResource {
            resource: RuntimeResource::CostMicros
        }
    ));
}

#[test]
fn accounting_reports_what_was_allowed_and_what_was_used() {
    let plan = BudgetPlan::new()
        .with(RuntimeResource::ToolCalls, Limit::soft_then_hard(2, 5))
        .with(RuntimeResource::ModelTokens, Limit::hard(1_000));
    let mut budget = BudgetController::from_plan(&plan);
    budget.charge(RuntimeResource::ToolCalls, 3).expect("fits");

    let accounting = budget.accounting();
    assert_eq!(accounting.len(), 2);
    assert_eq!(accounting[&RuntimeResource::ToolCalls].used, 3);
    assert_eq!(accounting[&RuntimeResource::ToolCalls].limit.hard, 5);
    assert_eq!(
        accounting[&RuntimeResource::ModelTokens].used,
        0,
        "a resource that was budgeted and never used still appears, so a comparison is complete"
    );
}

#[test]
fn an_exhausted_host_refuses_the_effect_instead_of_performing_it() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileWrite])
        .allowing_path("/work/");
    let plan = BudgetPlan::new().with(RuntimeResource::ToolCalls, Limit::hard(2));
    let mut host = RecordingHost::new(run("run-budget"), InProcessWorld::new(), policy)
        .with_budget(BudgetController::from_plan(&plan));

    host.write_file("/work/a.txt", "a").expect("first of two");
    host.write_file("/work/b.txt", "b").expect("second of two");

    let error = host
        .write_file("/work/c.txt", "c")
        .expect_err("the third exceeds the ceiling");
    assert!(matches!(error, RuntimeError::BudgetExhausted { .. }));

    assert_eq!(
        host.source().calls(),
        2,
        "the world must not perform an effect the budget refused to pay for"
    );
    assert_eq!(
        host.tape().len(),
        2,
        "and the tape must not record one either"
    );
}

#[test]
fn a_host_without_a_meter_is_not_charged_for_anything() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileWrite])
        .allowing_path("/work/");
    let mut host = RecordingHost::new(run("run-unmetered"), InProcessWorld::new(), policy);

    for index in 0..10 {
        host.write_file(&format!("/work/{index}.txt"), "x")
            .expect("metering is the orchestrator's job when no meter is attached");
    }
    assert!(host.budget().is_none());
    assert_eq!(host.tape().len(), 10);
}

#[test]
fn an_unfunded_meter_permits_nothing() {
    let mut budget = BudgetController::unfunded();
    let error = budget
        .charge(RuntimeResource::ToolCalls, 1)
        .expect_err("nothing was declared, so nothing may be spent");
    assert!(matches!(error, RuntimeError::UndeclaredResource { .. }));
}
