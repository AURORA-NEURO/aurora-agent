//! The rule language's honesty properties, held as unit claims.

use bioprism_domain::{Predicate, RuleOracle};
use bioprism_fiber::DecisionOracle;
use bioprism_section::{LeakageWitness, OracleStatus};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn values(entries: &[(&str, Value)]) -> BTreeMap<String, Value> {
    entries
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

fn oracle(document: Value) -> RuleOracle {
    RuleOracle::from_json(&document).expect("oracle parses")
}

#[test]
fn an_absent_variable_makes_a_check_unrunnable_not_false_and_not_true() {
    let predicate = Predicate::from_json(&json!({
        "kind": "equals", "variable": "flag", "value": true
    }))
    .expect("parses");

    assert_eq!(predicate.evaluate(&values(&[("flag", json!(true))])), Ok(true));
    assert_eq!(predicate.evaluate(&values(&[("flag", json!(false))])), Ok(false));
    let obstruction = predicate
        .evaluate(&values(&[]))
        .expect_err("absence is unevaluable, not a truth value");
    assert_eq!(obstruction.variable, "flag");
}

#[test]
fn a_wrong_type_is_unevaluable_rather_than_coerced() {
    let predicate = Predicate::from_json(&json!({
        "kind": "number_at_least", "variable": "ratio", "minimum": 0.5
    }))
    .expect("parses");
    let obstruction = predicate
        .evaluate(&values(&[("ratio", json!("0.9"))]))
        .expect_err("a string is not a number");
    assert!(obstruction.reason.contains("expected a number"));
}

#[test]
fn three_valued_logic_forces_a_truth_value_only_when_the_evidence_does() {
    let conjunction = Predicate::from_json(&json!({
        "kind": "all_of",
        "predicates": [
            { "kind": "equals", "variable": "present", "value": true },
            { "kind": "equals", "variable": "absent", "value": true }
        ]
    }))
    .expect("parses");

    // false && unknown is determinately false; true && unknown is unknown.
    assert_eq!(
        conjunction.evaluate(&values(&[("present", json!(false))])),
        Ok(false)
    );
    conjunction
        .evaluate(&values(&[("present", json!(true))]))
        .expect_err("true && unknown has no truth value");

    let disjunction = Predicate::from_json(&json!({
        "kind": "any_of",
        "predicates": [
            { "kind": "equals", "variable": "present", "value": true },
            { "kind": "equals", "variable": "absent", "value": true }
        ]
    }))
    .expect("parses");
    assert_eq!(
        disjunction.evaluate(&values(&[("present", json!(true))])),
        Ok(true)
    );
    disjunction
        .evaluate(&values(&[("present", json!(false))]))
        .expect_err("false || unknown has no truth value");
}

#[test]
fn exists_and_missing_are_total_because_absence_is_what_they_ask_about() {
    let exists = Predicate::from_json(&json!({ "kind": "exists", "variable": "x" })).unwrap();
    let missing = Predicate::from_json(&json!({ "kind": "missing", "variable": "x" })).unwrap();
    assert_eq!(exists.evaluate(&values(&[])), Ok(false));
    assert_eq!(missing.evaluate(&values(&[])), Ok(true));
}

#[test]
fn an_unrun_check_abstains_the_verdict_rather_than_passing_the_world() {
    let oracle = oracle(json!({
        "kind": "rule/test-v1",
        "checks": [
            { "name": "needs_ratio", "description": "ratio too high",
              "when": { "kind": "number_at_least", "variable": "ratio", "minimum": 0.5 } }
        ]
    }));
    let verdict = oracle.evaluate(&values(&[])).expect("evaluates");
    assert_eq!(verdict.status, OracleStatus::Underdetermined);
    assert_eq!(verdict.witnesses.len(), 1);
    let LeakageWitness::DomainCheck { check, detail, .. } = &verdict.witnesses[0] else {
        panic!("expected a domain_check witness");
    };
    assert_eq!(check, "needs_ratio");
    assert!(detail.starts_with("check did not run:"), "detail: {detail}");
}

#[test]
fn a_proven_violation_outranks_an_unrun_check_and_both_are_reported() {
    let oracle = oracle(json!({
        "kind": "rule/test-v1",
        "checks": [
            { "name": "fires", "description": "flag is set",
              "when": { "kind": "equals", "variable": "flag", "value": true } },
            { "name": "blind", "description": "ratio too high",
              "when": { "kind": "number_at_least", "variable": "ratio", "minimum": 0.5 } }
        ]
    }));
    let verdict = oracle
        .evaluate(&values(&[("flag", json!(true))]))
        .expect("evaluates");
    assert_eq!(verdict.status, OracleStatus::Invalid);
    let kinds: Vec<&str> = verdict
        .witnesses
        .iter()
        .map(|witness| {
            let LeakageWitness::DomainCheck { check, .. } = witness else {
                panic!("expected domain_check witnesses");
            };
            check.as_str()
        })
        .collect();
    assert_eq!(kinds, vec!["fires", "blind"], "violations first, unrun checks after");
}

#[test]
fn required_evidence_abstains_before_any_check_runs() {
    let oracle = oracle(json!({
        "kind": "rule/test-v1",
        "require": ["ledger"],
        "checks": [
            { "name": "fires_on_anything", "description": "always true",
              "when": { "kind": "missing", "variable": "nothing_provides_this" } }
        ]
    }));
    let verdict = oracle.evaluate(&values(&[])).expect("evaluates");
    assert_eq!(verdict.status, OracleStatus::Underdetermined);
    let LeakageWitness::DomainCheck { check, .. } = &verdict.witnesses[0] else {
        panic!("expected a domain_check witness");
    };
    assert_eq!(check, "required_evidence");
}

#[test]
fn string_order_is_lexicographic_and_refused_for_non_strings() {
    let predicate = Predicate::from_json(&json!({
        "kind": "string_before", "variable": "stamp", "than": "2025-05-01T00:00:00Z"
    }))
    .expect("parses");
    assert_eq!(
        predicate.evaluate(&values(&[("stamp", json!("2025-04-30T23:59:59Z"))])),
        Ok(true)
    );
    predicate
        .evaluate(&values(&[("stamp", json!(20250430))]))
        .expect_err("a number has no lexicographic order against a string");
}

#[test]
fn an_unknown_predicate_kind_and_an_undeclared_field_are_refused_by_name() {
    let unknown = Predicate::from_json(&json!({ "kind": "equal", "variable": "x", "value": 1 }))
        .expect_err("misspelled kind");
    assert!(unknown.to_string().contains("equal"), "{unknown}");

    let undeclared = Predicate::from_json(&json!({
        "kind": "equals", "variable": "x", "value": 1, "vlaue": 2
    }))
    .expect_err("undeclared field");
    assert!(undeclared.to_string().contains("vlaue"), "{undeclared}");
}

#[test]
fn an_oracle_kind_outside_the_rule_namespace_is_refused() {
    RuleOracle::from_json(&json!({
        "kind": "deterministic_split_integrity_v1",
        "checks": [
            { "name": "x", "description": "d", "when": { "kind": "exists", "variable": "v" } }
        ]
    }))
    .expect_err("a rule oracle may not impersonate a native oracle kind");
}

#[test]
fn an_oracle_with_no_checks_or_duplicate_check_names_is_refused() {
    RuleOracle::from_json(&json!({ "kind": "rule/empty-v1", "checks": [] }))
        .expect_err("no checks means valid for every world");

    RuleOracle::from_json(&json!({
        "kind": "rule/dup-v1",
        "checks": [
            { "name": "same", "description": "a", "when": { "kind": "exists", "variable": "v" } },
            { "name": "same", "description": "b", "when": { "kind": "exists", "variable": "w" } }
        ]
    }))
    .expect_err("duplicate names make witnesses ambiguous");
}
