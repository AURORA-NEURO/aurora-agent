//! Lowering invariants (blueprint 23.03 phases 2-4).
//!
//! The claim under test throughout is the one stated in `lower`'s module docs: a compiled program
//! never declares more authority than its source did.

use bioprism_weavelang::diagnostic::Diagnostic;
use bioprism_weavelang::ir::WeaveIr;
use bioprism_weavelang::lower::{lower_program, LowerError};
use bioprism_weavelang::parser::parse;
use bioprism_weavelang::reference::COMPLETE_PROGRAM;
use bioprism_weave::Resource;

fn compile(source: &str) -> Result<WeaveIr, LowerError> {
    let program = parse(source).expect("the fixture must parse");
    lower_program(&program, source)
}

/// A program with a commitment whose quality predicate every role declares.
const COMMITTING_PROGRAM: &str = r#"
policy repair {
  allow effects [repo.read, tests.pass, branch.write]
  budget tokens <= 40000
  budget tool-calls <= 32
}

role Patcher {
  provides [patch@1]
  requires [branch.write, tests.pass]
}

role Lead {
  provides [plan@1]
  requires [repo.read]
}

weave repair-issue(issue: Issue) -> Report using repair {
  bind patcher to role Patcher
  bind lead to role Lead

  commit patcher to lead when task.accepted {
    deliver Patch before 15m
    satisfy with tests.pass
    compensate revert-branch on violation
  }
}
"#;

/// The same program with `tests.pass` removed from every role's declared requirement.
const UNDECLARED_DISCHARGE_EFFECT: &str = r#"
policy repair {
  allow effects [repo.read, tests.pass, branch.write]
  budget tokens <= 40000
}

role Patcher {
  provides [patch@1]
  requires [branch.write]
}

role Lead {
  provides [plan@1]
  requires [repo.read]
}

weave repair-issue(issue: Issue) -> Report using repair {
  bind patcher to role Patcher
  bind lead to role Lead

  commit patcher to lead when task.accepted {
    deliver Patch
    satisfy with tests.pass
  }
}
"#;

const FORKING_PROGRAM: &str = r#"
policy exploration {
  allow effects [search.read]
  budget tokens <= 30000
  budget tool-calls <= 16
}

role Worker {
  provides [search@1]
  requires [search.read]
}

weave explore(q: Query) -> Report using exploration {
  bind worker to role Worker
  checkpoint c = current
  fork from c {
    branch h1 with budget tokens(10000) { let one = ask worker.search(q) }
    branch h2 with budget tokens(10000) { let two = ask worker.search(q) }
  }
  join using verified-best
}
"#;

#[test]
fn the_reference_program_lowers_and_its_canonical_form_round_trips() {
    let ir = compile(COMPLETE_PROGRAM).expect("the reference program must compile");

    assert_eq!(ir.weave_ir_version, "0.1.0");
    assert!(ir.program_id.starts_with("urn:weave:program:run@sha256:"));
    assert_eq!(ir.ledgers, vec!["commitment", "epistemic"]);

    let bytes = ir.canonical_bytes().expect("canonicalises");
    let text = String::from_utf8(bytes.clone()).expect("canonical JSON is UTF-8");
    let restored: WeaveIr = serde_json::from_str(&text).expect("round trips");
    assert_eq!(restored, ir);
    assert_eq!(
        restored.canonical_bytes().expect("canonicalises"),
        bytes,
        "a round trip must not change a single byte"
    );
}

#[test]
fn the_canonical_bytes_are_the_workspace_canonical_json_and_nothing_else() {
    let ir = compile(COMPLETE_PROGRAM).expect("compiles");
    let value = serde_json::to_value(&ir).expect("serialises");
    let expected = bioprism_ids::to_canonical_bytes(&value).expect("canonicalises");
    assert_eq!(ir.canonical_bytes().expect("canonicalises"), expected);
}

#[test]
fn compiling_the_same_source_twice_produces_the_same_program_identity() {
    let first = compile(COMPLETE_PROGRAM).expect("compiles");
    let second = compile(COMPLETE_PROGRAM).expect("compiles");
    assert_eq!(first.program_id, second.program_id);
    assert_eq!(first.digest().expect("digests"), second.digest().expect("digests"));
}

#[test]
fn program_identity_ignores_comments_and_whitespace_but_not_semantics() {
    let baseline = compile(COMPLETE_PROGRAM).expect("compiles");

    let reformatted = COMPLETE_PROGRAM.replace(
        "weave run(issue: Issue, repo: Repository) -> Report using safe-repair {",
        "// a comment the compiler must ignore\n\n  weave run(issue: Issue, repo: Repository) -> Report using safe-repair {",
    );
    let reformatted = compile(&reformatted).expect("compiles");
    assert_eq!(
        baseline.program_id, reformatted.program_id,
        "23.03: identity comes from semantic IR, not source formatting"
    );

    let changed = COMPLETE_PROGRAM.replace("budget tokens <= 120000", "budget tokens <= 60000");
    let changed = compile(&changed).expect("compiles");
    assert_ne!(baseline.program_id, changed.program_id);
}

#[test]
fn provenance_is_recorded_but_excluded_from_program_identity() {
    let baseline = compile(COMPLETE_PROGRAM).expect("compiles");
    assert_eq!(baseline.provenance.compiler, "bioprism-weavelang");
    assert!(!baseline.provenance.source_sha256.is_empty());

    let mut altered = baseline.clone();
    altered.provenance.compiler_version = "99.99.99".to_string();
    assert_eq!(
        baseline.semantic_digest().expect("digests"),
        altered.semantic_digest().expect("digests")
    );
    assert_ne!(
        baseline.digest().expect("digests"),
        altered.digest().expect("digests"),
        "the whole-document digest must still notice the change"
    );
}

#[test]
fn a_lowering_that_introduces_an_undeclared_effect_is_a_compiler_error() {
    let error = compile(UNDECLARED_DISCHARGE_EFFECT)
        .expect_err("discharging this commitment runs tests no role declared");
    let LowerError::EffectIntroducedByLowering {
        effect,
        transition,
        declared,
        ..
    } = &error
    else {
        panic!("expected an introduced-effect error, got {error:?}");
    };
    assert_eq!(effect, "tests.pass");
    assert_eq!(transition, "commit-discharge-patcher");
    assert!(!declared.contains(&"tests.pass".to_string()));
    assert_eq!(error.code(), "WEAVE-E3203");
}

#[test]
fn the_same_program_with_the_effect_declared_compiles() {
    let ir = compile(COMMITTING_PROGRAM).expect("every discharged effect is declared");
    let discharge = ir
        .state_graph
        .transitions
        .iter()
        .find(|transition| transition.id == "commit-discharge-patcher")
        .expect("the commitment lowers to a discharge");
    assert_eq!(discharge.act, "discharge");
    assert_eq!(discharge.effects.world, vec!["tests.pass"]);
}

#[test]
fn a_program_whose_role_requires_a_denied_effect_is_rejected() {
    let source = r#"
policy locked {
  allow effects [repo.read]
  deny effects [main.write]
  budget tokens <= 100
}
role Rogue { provides [x@1] requires [main.write] }
weave w() -> R using locked {
  bind rogue to role Rogue
  let a = ask rogue.go()
}
"#;
    let error = compile(source).expect_err("`main.write` is denied");
    let LowerError::DeniedEffect { effect, role, .. } = &error else {
        panic!("expected a denied-effect error, got {error:?}");
    };
    assert_eq!(effect, "main.write");
    assert_eq!(role, "Rogue");
    assert_eq!(error.code(), "WEAVE-E3202");
}

#[test]
fn a_program_whose_role_requires_an_effect_outside_the_allow_list_is_rejected() {
    let source = r#"
policy narrow {
  allow effects [repo.read]
  budget tokens <= 100
}
role Wide { provides [x@1] requires [repo.read, network.crossref] }
weave w() -> R using narrow {
  bind wide to role Wide
  let a = ask wide.go()
}
"#;
    let error = compile(source).expect_err("`network.crossref` is not allowed");
    assert_eq!(error.code(), "WEAVE-E3201");
    assert!(error.to_string().contains("network.crossref"));
}

#[test]
fn branch_leases_that_exceed_the_policy_ceiling_are_rejected_by_the_kernels_own_budget() {
    let source = FORKING_PROGRAM.replace("budget tokens <= 30000", "budget tokens <= 15000");
    let error = compile(&source).expect_err("two 10000 leases do not fit under 15000");
    let LowerError::BudgetCeilingExceeded {
        branch,
        resource,
        requested,
        available,
        ..
    } = &error
    else {
        panic!("expected a ceiling error, got {error:?}");
    };
    assert_eq!(branch, "h2");
    assert_eq!(*resource, Resource::Tokens);
    assert_eq!(*requested, 10000);
    assert_eq!(*available, 5000);
    assert_eq!(error.code(), "WEAVE-E3301");
}

#[test]
fn branch_leases_within_the_ceiling_compile_and_are_recorded_on_the_branch_transition() {
    let ir = compile(FORKING_PROGRAM).expect("20000 fits under 30000");
    let branch = ir
        .state_graph
        .transitions
        .iter()
        .find(|transition| transition.id == "branch-h1")
        .expect("a branch transition");
    assert_eq!(branch.act, "delegate");
    assert_eq!(branch.effects.budget, vec!["reserve:tokens"]);
    assert_eq!(branch.effects.authority, vec!["attenuate"]);
}

#[test]
fn a_branch_leasing_a_resource_the_kernel_cannot_account_is_rejected() {
    let source = FORKING_PROGRAM.replace("tokens(10000)", "money(10000)");
    let error = compile(&source).expect_err("money is not a kernel resource");
    assert_eq!(error.code(), "WEAVE-E3302");
}

#[test]
fn a_money_budget_is_recorded_as_unenforceable_rather_than_silently_dropped() {
    let source = COMPLETE_PROGRAM.replace(
        "budget tokens <= 120000",
        "budget tokens <= 120000\n  budget money <= usd(5)",
    );
    let ir = compile(&source).expect("compiles");
    let policy = ir.policies.get("safe-repair").expect("the policy");

    assert!(policy
        .budgets
        .iter()
        .any(|budget| budget.resource == Resource::Tokens));
    let unenforced = policy
        .unenforceable_budgets
        .iter()
        .find(|budget| budget.declared_resource == "money")
        .expect("the money ceiling must survive into the IR");
    assert_eq!(unenforced.declared_limit, "usd(5.00)");
    assert!(unenforced.reason.contains("23.16"));
}

#[test]
fn a_weave_without_a_policy_is_rejected_because_there_is_no_ceiling_to_preserve() {
    let error = compile("weave w() -> R { let a = ask x.y() }")
        .expect_err("no policy means no declared budget or allowance");
    assert_eq!(error.code(), "WEAVE-E3103");
}

#[test]
fn binding_an_undeclared_role_names_the_roles_that_were_declared() {
    let source = r#"
policy p { allow effects [a.read] budget tokens <= 10 }
role Known { provides [x@1] requires [a.read] }
weave w() -> R using p {
  bind who to role Unknown
  let a = ask who.go()
}
"#;
    let error = compile(source).expect_err("`Unknown` is not declared");
    let LowerError::UnknownRole { name, known, .. } = &error else {
        panic!("expected an unknown-role error, got {error:?}");
    };
    assert_eq!(name, "Unknown");
    assert_eq!(known, &vec!["Known".to_string()]);
}

#[test]
fn a_fork_from_an_undeclared_checkpoint_is_rejected() {
    let source = FORKING_PROGRAM.replace("checkpoint c = current\n", "");
    let error = compile(&source).expect_err("`c` was never checkpointed");
    assert_eq!(error.code(), "WEAVE-E3107");
}

#[test]
fn a_send_to_a_participant_that_was_never_bound_is_rejected() {
    let source = r#"
policy p { allow effects [a.read] budget tokens <= 10 }
role R { provides [x@1] requires [a.read] }
weave w() -> Report using p {
  bind one to role R
  send propose(x) from one to two
}
"#;
    let error = compile(source).expect_err("`two` is unbound");
    assert_eq!(error.code(), "WEAVE-E3106");
}

#[test]
fn a_forking_program_demands_a_higher_abi_grade_than_one_that_never_resumes() {
    let forking = compile(FORKING_PROGRAM).expect("compiles");
    let linear = compile(COMPLETE_PROGRAM).expect("compiles");
    assert_eq!(forking.participants[0].required_abi_grade, 3);
    assert_eq!(linear.participants[0].required_abi_grade, 1);
    assert!(
        !linear.participants[0].bound,
        "23.02 binds participants at run time, so the compiler must leave the slot open"
    );
}

#[test]
fn an_effect_gated_on_human_approval_marks_the_transition_that_carries_it() {
    let source = r#"
policy gated {
  allow effects [main.merge]
  require human for [main.merge]
  budget tokens <= 100
}
role Merger { provides [merge@1] requires [main.merge] }
weave w() -> R using gated {
  bind merger to role Merger
  let done = ask merger.merge()
}
"#;
    let ir = compile(source).expect("compiles");
    let transition = ir
        .state_graph
        .transitions
        .iter()
        .find(|transition| transition.id == "ask-done")
        .expect("the ask transition");
    assert!(transition.requires_human_approval);
    assert_eq!(transition.effects.world, vec!["main.merge"]);
}

#[test]
fn the_declared_choreography_is_preserved_in_the_ir_rather_than_projected() {
    let ir = compile(COMPLETE_PROGRAM).expect("compiles");
    assert_eq!(ir.choreography.id, "review");
    assert_eq!(ir.choreography.roles, vec!["Lead", "Reviewer"]);

    let labels: Vec<Option<String>> = ir
        .choreography
        .declared_steps
        .iter()
        .map(|step| step.choice_label.clone())
        .collect();
    assert_eq!(
        labels,
        vec![
            None,
            Some("accept".to_string()),
            Some("challenge".to_string())
        ]
    );
    assert_eq!(
        ir.choreography.declared_steps[1].decided_by.as_deref(),
        Some("Reviewer")
    );
}

#[test]
fn an_evaluation_hook_attribute_becomes_a_prism_decision_boundary() {
    let source = r#"
policy p { allow effects [evidence.read] budget tokens <= 10 }
role R { provides [x@1] requires [evidence.read] }
weave w() -> Report using p {
  bind r to role R
  @decision-cell(capability="context.information-value")
  let next = choose evidence by information-value
  let seen = ask r.look()
}
"#;
    let ir = compile(source).expect("compiles");
    assert_eq!(ir.evaluation_hooks.len(), 1);
    let hook = &ir.evaluation_hooks[0];
    assert_eq!(hook.id, "hook-next");
    assert_eq!(hook.capability, "context.information-value");
    assert!(hook.snapshot_include.contains(&"world".to_string()));
    assert!(hook
        .counterfactual_dimensions
        .contains(&"model".to_string()));
}

#[test]
fn an_empty_branch_body_is_rejected_rather_than_given_an_invented_meaning() {
    let source = FORKING_PROGRAM.replace("{ let one = ask worker.search(q) }", "{ }");
    let error = compile(&source).expect_err("an empty branch reaches no state");
    assert_eq!(error.code(), "WEAVE-E3108");
}

#[test]
fn a_source_with_no_weave_program_has_nothing_to_compile() {
    let error = compile("policy p { budget tokens <= 1 }").expect_err("no entry point");
    assert_eq!(error.code(), "WEAVE-E3101");
}
