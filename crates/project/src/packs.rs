//! The engineering pack: release readiness as a `bioprism-domain/0.1` document.
//!
//! Every check here is a **static-scan proxy** and its description says so on the wire, so a
//! witness quoted out of context still carries the caveat. A counted test is not a passing
//! test; a present workflow is not a working one; a requirement string is not a resolved
//! version. The oracle judges what the scan can honestly claim — declarations and inventories —
//! and nothing it would need execution to know.
//!
//! The `no_ci` check uses `not` over `nonempty` on `ci_workflow_inventory`: the predicate
//! language as it exists can state "empty", so nothing was extended and no shadow count is
//! needed for the check (the `ci_workflow_count` fact still exists in the world as colour).

use crate::assemble::DEFAULT_DECISION_TIME;
use serde_json::{json, Value};

/// The pack's declared name.
pub const PACK_NAME: &str = "project-release-readiness";

/// The oracle kind every certificate from this pack names.
pub const RELEASE_ORACLE_KIND: &str = "rule/project-release-readiness-v1";

/// The single protected tag project queries declare.
pub const PROTECTED_TAG: &str = "protected";

/// The goal sentence shared by the pack and the release query.
pub const RELEASE_GOAL: &str =
    "Decide whether the scanned project is ready to release, on static inventory evidence alone.";

/// The dimension classification for project worlds.
///
/// `project` is an identity (which thing the facts are about), `component` is a region (where
/// in the tree a claim holds), `scan` is an ontology (which scanner vocabulary produced the
/// numbers), and `issue` is an identity (which work item a record belongs to). The task sketch
/// suggested `manifest: specimen`, but no fact in the assembled world is scoped by a manifest —
/// manifest paths ride inside the dependency records as data — and declaring a dimension no
/// scope binds would be coverage theatre.
pub fn dimension_document() -> Value {
    json!({
        "schema_version": "bioprism-scope-dimensions/0.1",
        "dimensions": {
            "project": "identity",
            "component": "region",
            "scan": "ontology",
            "issue": "identity"
        }
    })
}

/// Emits the release-readiness pack with the declared `todo_burden` threshold.
pub fn release_readiness_pack(todo_burden_at_least: u64) -> Value {
    json!({
        "schema_version": "bioprism-domain/0.1",
        "name": PACK_NAME,
        "description": "Software release-readiness review over a static project scan: decides \
            whether the tree declares unpinned dependencies, counts no tests, carries no CI \
            workflow, or exceeds the declared TODO-marker budget. Every check is a proxy over \
            declarations and inventories; nothing is executed or resolved.",
        "goal": RELEASE_GOAL,
        "protected_tags": [PROTECTED_TAG],
        "scope_dimensions": dimension_document(),
        "oracle": {
            "kind": RELEASE_ORACLE_KIND,
            "require": [
                "dependency_declarations",
                "test_function_total",
                "ci_workflow_inventory",
                "scan_loss_summary"
            ],
            "checks": [
                {
                    "name": "unpinned_dependency",
                    "description": "a dependency is declared without an exact version pin \
                        (static manifest scan: '=' prefix for Cargo, exact numeric semver for \
                        package.json, '==' for pyproject; a declared requirement is not a \
                        resolved version, and a lockfile is not consulted)",
                    "when": { "kind": "nonempty", "variable": "unpinned_dependencies" }
                },
                {
                    "name": "tests_absent",
                    "description": "the scan counted zero test functions ('#[test]' substring \
                        occurrences in .rs files — a static proxy that never runs anything; a \
                        counted test is not a passing test, and zero counted means zero found \
                        by that proxy, not proof no test exists in another language)",
                    "when": { "kind": "number_below", "variable": "test_function_total", "maximum": 1 }
                },
                {
                    "name": "no_ci",
                    "description": "no workflow file exists under .github/workflows (file \
                        presence by static scan; workflow content is never interpreted, so a \
                        present workflow is not evidence of a working one — this check only \
                        fires on total absence)",
                    "when": {
                        "kind": "not",
                        "predicate": { "kind": "nonempty", "variable": "ci_workflow_inventory" }
                    }
                },
                {
                    "name": "todo_burden",
                    "description": format!(
                        "the tree carries at least {todo_burden_at_least} TODO markers \
                         (case-sensitive substring count across UTF-8 files; over-counts \
                         markers quoted in strings and under-counts lowercase todo!(); the \
                         threshold is a declared editorial default, not a measurement)"
                    ),
                    "when": {
                        "kind": "number_at_least",
                        "variable": "todo_marker_total",
                        "minimum": todo_burden_at_least
                    }
                }
            ]
        }
    })
}

/// The release query: compile the minimal decision-sufficient region for
/// `release_integrity_status` at `decision_time` (defaulted to the fixed epoch when empty —
/// no clock is ever read).
pub fn release_query(world_id: &str, decision_time: &str, max_facts: usize) -> Value {
    query(
        format!("release-{world_id}"),
        RELEASE_GOAL.to_string(),
        "release_integrity_status".to_string(),
        decision_time,
        max_facts,
    )
}

/// The per-issue query: compile the minimal evidence region for working one issue — the
/// components it declares plus the aggregate decision inputs, and nothing else.
pub fn issue_query(issue_id: &str, world_id: &str, decision_time: &str, max_facts: usize) -> Value {
    query(
        format!("issue-{issue_id}-{world_id}"),
        format!(
            "Compile the minimal declared-evidence region for working issue {issue_id}: its \
             named components' inventories plus the aggregate decision inputs."
        ),
        format!("issue_{issue_id}_context_status"),
        decision_time,
        max_facts,
    )
}

fn query(
    query_id: String,
    goal: String,
    target: String,
    decision_time: &str,
    max_facts: usize,
) -> Value {
    let decision_time = if decision_time.is_empty() {
        DEFAULT_DECISION_TIME
    } else {
        decision_time
    };
    json!({
        "schema_version": "fiber-query/0.2",
        "query_id": query_id,
        "goal": goal,
        "targets": [target],
        "protected_tags": [PROTECTED_TAG],
        "decision_time": decision_time,
        "budgets": { "max_facts": max_facts }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_domain::DomainPack;

    #[test]
    fn the_emitted_pack_parses_under_the_strict_domain_reader_at_any_threshold() {
        for threshold in [1, 50, 500] {
            let pack = DomainPack::from_json(&release_readiness_pack(threshold))
                .expect("emitted pack must satisfy the strict bioprism-domain/0.1 parser");
            assert_eq!(pack.name(), PACK_NAME);
            assert_eq!(pack.protected_tags(), [PROTECTED_TAG.to_string()]);
        }
    }

    #[test]
    fn every_check_description_declares_itself_a_static_proxy_in_some_words() {
        let pack = release_readiness_pack(50);
        for check in pack["oracle"]["checks"].as_array().unwrap() {
            let description = check["description"].as_str().unwrap();
            assert!(
                description.contains("static")
                    || description.contains("proxy")
                    || description.contains("count"),
                "check {} does not say it is a static proxy: {description}",
                check["name"]
            );
        }
    }

    #[test]
    fn an_empty_decision_time_falls_back_to_the_documented_epoch_never_a_clock() {
        let q = release_query("project-abc", "", 10);
        assert_eq!(q["decision_time"], DEFAULT_DECISION_TIME);
    }
}
