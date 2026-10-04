//! MCP contract tests for context repository contracts.

use super::*;

#[test]
fn repository_impact_reports_a_scanned_module_and_typed_closure() {
    let mut server = server();
    let catalogue = call(
        &mut server,
        "repository_catalog",
        json!({ "prefix": "docs/", "limit": 1 }),
    );
    let changed = catalogue["modules"][0]["id"]
        .as_str()
        .expect("repository catalog returns a module id")
        .to_string();
    let payload = call(
        &mut server,
        "repository_impact",
        json!({ "changed": changed }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert!(payload["closure_count"].as_u64().unwrap() >= 1);
    assert!(payload["impact"]["affected"].is_array());
    assert!(payload["impact"]["stopped_at"].is_array());
    assert!(payload["guarantees"].as_array().unwrap().len() >= 4);
}

#[test]
fn world_generation_is_digest_stable_and_validates_both_documents() {
    let mut server = server();
    let spec = bioprism_worldgen::WorldSpec::discriminating(2);
    let arguments = json!({
        "spec": serde_json::to_value(spec).unwrap(),
        "include_world": true,
        "include_query": true,
    });
    let first = call(&mut server, "world_generate", arguments.clone());
    let second = call(&mut server, "world_generate", arguments);
    assert_eq!(first["ok"], json!(true));
    assert_eq!(first["validation"]["errors"], json!(0));
    assert_eq!(first["world_digest"], second["world_digest"]);
    assert_eq!(first["query_digest"], second["query_digest"]);
    assert!(first["world"]["facts"].is_array());
    assert!(first["query"]["targets"].is_array());
    assert!(first["guarantees"].as_array().unwrap().len() >= 4);
}

#[test]
fn world_validation_is_available_before_context_compilation() {
    let mut server = server();
    let payload = call(&mut server, "world_validate", json!({ "world": WORLD }));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["counts"]["facts"], json!(761));
    assert!(payload["world_sha256"].as_str().unwrap().len() >= 32);
    assert!(payload["diagnostics"].is_array());
}

#[test]
fn context_comparison_exposes_the_equal_engineering_panel() {
    let mut server = server();
    let payload = call(
        &mut server,
        "context_compare",
        json!({ "world": WORLD, "query": QUERY }),
    );
    assert!(payload["results"].is_array());
    let fiber = payload["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == json!("fiber"))
        .expect("baseline panel includes fiber");
    assert!(fiber["facts_exposed"].is_number());
    assert!(fiber["admissible"].is_boolean());
}

#[test]
fn repository_bundle_fails_instead_of_truncating_oversized_markdown() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_bundle",
        json!({
            "route": {
                "id": "orientation-too-small",
                "intent": "understand the repository",
                "must_read": ["README.md"]
            },
            "include_markdown": true,
            "max_markdown_chars": 1
        }),
    );
    assert_eq!(payload["__isError"], json!(true));
    assert!(
        payload["error"]
            .as_str()
            .unwrap()
            .contains("max_markdown_chars")
    );
}

#[test]
fn a_repository_bundle_under_budget_refuses_and_names_the_shortfall() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_bundle",
        json!({
            "route": {
                "id": "orientation-underfunded",
                "intent": "understand the repository before choosing a domain",
                "must_read": ["README.md"],
                "budget": 1
            }
        }),
    );
    assert_eq!(payload["__isError"], json!(true));
    let error = payload["error"].as_str().unwrap();
    assert!(
        error.contains("cannot close its mandatory set within budget"),
        "the refusal must say the mandatory set did not fit; got {error}"
    );
    assert!(
        error.contains("short by"),
        "the refusal must quantify the shortfall so a caller can raise the budget once; got {error}"
    );
}

#[test]
fn repository_catalog_is_bounded_and_reports_graph_health() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_catalog",
        json!({
            "prefix": "docs/",
            "limit": 3,
            "include_briefs": true
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["returned_modules"], json!(3));
    assert!(payload["matching_modules"].as_u64().unwrap() >= 3);
    assert_eq!(payload["truncated"], json!(true));
    assert!(payload["module_count"].as_u64().unwrap() > 0);
    assert!(payload["edge_count"].as_u64().unwrap() > 0);
    assert!(payload["modules"][0]["brief"].is_string());
    assert!(payload["lint"]["counts"].is_object());
}

#[test]
fn repository_bundle_compiles_a_route_with_progressive_disclosure() {
    let mut server = server();
    let payload = call(
        &mut server,
        "repository_bundle",
        json!({
            "route": {
                "id": "orientation",
                "intent": "understand the repository before choosing a domain",
                "must_read": ["README.md"],
                "budget": 60000
            },
            "policy": "normative",
            "include_markdown": true,
            "max_markdown_chars": 400000
        }),
    );
    assert_eq!(payload["ok"], json!(true), "{payload}");
    assert_eq!(payload["bundle"]["route"], json!("orientation"));
    assert!(!payload["bundle"]["entries"].as_array().unwrap().is_empty());
    assert!(payload["bundle"]["traversal"].is_object());
    assert!(
        payload["markdown"]
            .as_str()
            .unwrap()
            .contains("context bundle")
    );
}

#[test]
fn compile_returns_the_contract_not_the_evidence() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({ "world": WORLD, "query": QUERY }),
    );

    assert_eq!(payload["layer"], json!("l0"));
    assert_eq!(payload["verdict"]["status"], json!("invalid"));
    assert_eq!(
        payload["certificate_sha256"],
        json!("c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4")
    );

    assert!(
        payload.get("evidence").is_none(),
        "L0 must not carry values"
    );
    assert!(payload.get("evidence_inventory").is_none());
    assert!(payload.get("factors").is_none());

    assert_eq!(payload["omissions"]["omitted_facts"], json!(750));
    assert_eq!(
        payload["omissions"]["supports_sufficiency_claim"],
        json!(true)
    );
    assert_eq!(payload["refine"]["next_layer"], json!("l1"));
    assert_eq!(payload["refine"]["handle"]["version"], json!(1));
    assert_eq!(
        payload["refine"]["handle"]["certificate_sha256"],
        payload["certificate_sha256"]
    );
}

#[test]
fn compile_projects_the_wire_decision_quotient_without_claiming_rate_distortion() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.3/decision_contract_query.json"
        }),
    );

    assert_eq!(payload["layer"], json!("l0"));
    let quotient = &payload["decision_quotient"];
    assert_eq!(
        quotient["schema"],
        json!("bioprism-mcp/epistemic-decision-quotient/0.1")
    );
    assert_eq!(
        quotient["permitted_actions"],
        json!(["accept", "defer", "reject"])
    );
    assert_eq!(quotient["original_model_count"], json!(3));
    assert_eq!(quotient["quotient_model_count"], json!(2));
    assert_eq!(quotient["merged_model_count"], json!(1));
    assert_eq!(
        quotient["certificate_binding"]["query_sha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        quotient["certificate_binding"]["certificate_sha256"],
        payload["certificate_sha256"]
    );
    assert!(
        quotient["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("rate-distortion"))
    );

    let explained = call(
        &mut server,
        "fiber_explain",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.3/decision_contract_query.json"
        }),
    );
    assert_eq!(
        explained["decision_quotient"]["quotient_model_count"],
        json!(2)
    );
    assert!(
        !explained["passes_not_run"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pass| pass["name"] == "decision_quotient")
    );
}

#[test]
fn compile_projects_the_wire_rate_distortion_audit() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.4/rate_distortion_query.json"
        }),
    );

    let report = &payload["rate_distortion"];
    assert_eq!(
        report["schema"],
        json!("bioprism-mcp/epistemic-context-audit/0.2")
    );
    assert_eq!(report["criterion"], json!("bayes_regret"));
    assert_eq!(report["evidence_count"], json!(2));
    assert_eq!(report["frontier"]["evaluated"], json!(4));
    assert!(report["identification"].is_object());
    assert!(report["sufficiency"].is_object());
    assert_eq!(
        report["certificate_binding"]["query_sha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        report["certificate_binding"]["certificate_sha256"],
        payload["certificate_sha256"]
    );

    let explained = call(
        &mut server,
        "fiber_explain",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.4/rate_distortion_query.json"
        }),
    );
    assert_eq!(
        explained["rate_distortion"]["frontier"]["evaluated"],
        json!(4)
    );
    assert!(
        explained["passes_not_run"]
            .as_array()
            .unwrap()
            .iter()
            .all(|pass| pass["name"] != "rate_distortion")
    );
}

#[test]
fn compile_projects_the_wire_adaptive_policy() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.5/adaptive_acquisition_query.json"
        }),
    );

    let report = &payload["adaptive_acquisition"];
    assert_eq!(
        report["schema"],
        json!("bioprism-mcp/fiber-adaptive-acquisition/0.1")
    );
    assert_eq!(report["budget"], json!(1.0));
    assert_eq!(report["max_steps"], json!(2));
    assert_eq!(report["prior"], json!([0.5, 0.25, 0.25]));
    assert_eq!(report["acquisitions"].as_array().unwrap().len(), 2);
    assert!(report["policy"]["nodes_evaluated"].as_u64().unwrap() > 0);
    assert!(report["policy"]["root"].is_object());
    assert_eq!(report["execution"], json!("not_started"));
    assert_eq!(report["authorization"], json!("not_granted"));
    assert_eq!(
        report["certificate_binding"]["query_sha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        report["certificate_binding"]["certificate_sha256"],
        payload["certificate_sha256"]
    );

    let explained = call(
        &mut server,
        "fiber_explain",
        json!({
            "world": WORLD,
            "query": "fixtures/fiber-v0.5/adaptive_acquisition_query.json"
        }),
    );
    assert_eq!(
        explained["adaptive_acquisition"]["schema"],
        json!("bioprism-mcp/fiber-adaptive-acquisition/0.1")
    );
    assert!(
        explained["passes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pass| pass["name"] == "adaptive_acquisition")
    );
    assert!(
        explained["passes_not_run"]
            .as_array()
            .unwrap()
            .iter()
            .all(|pass| pass["name"] != "adaptive_acquisition")
    );
}

#[test]
fn a_refinement_handle_is_verified_and_stale_handles_are_refused() {
    let mut server = server();
    let compiled = call(
        &mut server,
        "fiber_compile",
        json!({ "world": WORLD, "query": QUERY }),
    );
    let handle = compiled["refine"]["handle"].clone();

    let refined = call(
        &mut server,
        "fiber_refine",
        json!({ "handle": handle, "layer": "l2" }),
    );
    assert_eq!(refined["layer"], json!("l2"));

    let mut stale = compiled["refine"]["handle"].clone();
    stale["certificate_sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    let refused = call(
        &mut server,
        "fiber_refine",
        json!({ "handle": stale, "layer": "l2" }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("stale"));
}

#[test]
fn omissions_are_reported_at_every_layer() {
    let mut server = server();
    for layer in ["l0", "l1", "l2", "l3", "l4"] {
        let payload = call(
            &mut server,
            "fiber_refine",
            json!({ "world": WORLD, "query": QUERY, "layer": layer }),
        );
        assert_eq!(
            payload["omissions"]["omitted_facts"],
            json!(750),
            "{layer} hid the omission count"
        );
        assert_eq!(
            payload["omissions"]["protected_closure_satisfied"],
            json!(true)
        );
    }
}

#[test]
fn layers_are_cumulative_and_grow_monotonically() {
    let mut server = server();
    let mut previous = 0usize;
    for layer in ["l0", "l1", "l2", "l3", "l4"] {
        let payload = call(
            &mut server,
            "fiber_refine",
            json!({ "world": WORLD, "query": QUERY, "layer": layer }),
        );
        let size = serde_json::to_string(&payload).unwrap().len();
        assert!(
            size > previous,
            "{layer} was not larger than the layer before it"
        );
        previous = size;
    }

    let l1 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l1" }),
    );
    assert!(l1["evidence_inventory"].is_array());
    assert!(l1.get("evidence").is_none(), "l1 lists names, not values");

    let l2 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l2" }),
    );
    assert_eq!(l2["evidence"].as_array().unwrap().len(), 11);
    assert_eq!(l2["witnesses"].as_array().unwrap().len(), 4);

    let l3 = call(
        &mut server,
        "fiber_refine",
        json!({ "world": WORLD, "query": QUERY, "layer": "l3" }),
    );
    assert_eq!(l3["factors"].as_array().unwrap().len(), 6);
}

#[test]
fn explain_reports_the_passes_that_did_not_run() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_explain",
        json!({ "world": WORLD, "query": QUERY }),
    );
    let deferred: Vec<&str> = payload["passes_not_run"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert!(deferred.contains(&"obstruction_tests"));
    assert!(deferred.contains(&"rate_distortion"));
    assert_eq!(payload["selection"]["facts"], json!(11));
}

#[test]
fn absolute_paths_and_traversal_are_refused() {
    let server = server();
    assert!(
        server
            .resolve("fixtures/fiber-v0.1/leakage_query.json")
            .is_ok()
    );
    assert!(
        server
            .resolve("./fixtures/fiber-v0.1/leakage_query.json")
            .is_ok()
    );

    for hostile in [
        "../../../etc/passwd",
        "fixtures/../../secrets.json",
        "/etc/passwd",
        "C:/Windows/System32/config/SAM",
        "C:\\Windows\\System32\\config\\SAM",
        "..\\outside.json",
    ] {
        assert!(
            server.resolve(hostile).is_err(),
            "{hostile} should have been refused"
        );
    }
}

#[test]
fn a_refused_path_surfaces_as_a_tool_error_not_a_crash() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({ "world": "../../../etc/passwd", "query": QUERY }),
    );
    assert_eq!(payload["__isError"], json!(true));
    assert!(payload["error"].as_str().unwrap().contains("refused"));
}

#[test]
fn a_domain_compile_judges_the_trade_world_invalid_with_the_self_cross_witness() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": TRADE_WORLD,
            "query": TRADE_QUERY,
            "domain": TRADE_DOMAIN,
            "layer": "l2"
        }),
    );

    assert_eq!(payload["verdict"]["status"], json!("invalid"));
    assert_eq!(
        payload["verdict"]["oracle"],
        json!("rule/trade-surveillance-v1")
    );
    let witnesses = payload["witnesses"].as_array().unwrap();
    assert!(
        witnesses
            .iter()
            .any(|witness| witness["type"] == json!("domain_check")
                && witness["check"] == json!("self_cross")),
        "expected the self_cross domain_check witness, got {witnesses:?}"
    );

    assert_eq!(payload["domain"]["name"], json!("trade-surveillance"));
    assert_eq!(
        payload["domain"]["oracle_kind"],
        json!("rule/trade-surveillance-v1")
    );
    assert_eq!(
        payload["domain"]["protected_tags"],
        json!(["identity", "time", "protected"])
    );
    assert_eq!(
        payload["domain"]["advisories"],
        json!([]),
        "the fixture query declares the pack's goal and tags, so nothing is advisory"
    );
}

#[test]
fn a_domain_compile_reports_the_privilege_review_abstention_rather_than_valid() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": "fixtures/domains/privilege-review/world.json",
            "query": "fixtures/domains/privilege-review/query.json",
            "domain": "fixtures/domains/privilege-review/domain.json"
        }),
    );

    assert_eq!(payload["verdict"]["status"], json!("underdetermined"));
    assert_eq!(
        payload["verdict"]["oracle"],
        json!("rule/privilege-review-v1")
    );
    assert_eq!(payload["verdict"]["witnesses"], json!(["domain_check"]));
    assert_eq!(payload["domain"]["name"], json!("privilege-review"));

    let explained = call(
        &mut server,
        "fiber_explain",
        json!({
            "world": "fixtures/domains/privilege-review/world.json",
            "query": "fixtures/domains/privilege-review/query.json",
            "domain": "fixtures/domains/privilege-review/domain.json"
        }),
    );
    assert_eq!(explained["domain"]["name"], json!("privilege-review"));
    assert_eq!(
        explained["domain"]["oracle_kind"],
        json!("rule/privilege-review-v1")
    );
    assert_eq!(explained["domain"]["advisories"], json!([]));
}

#[test]
fn a_compile_without_a_domain_parameter_keeps_the_reference_digest_and_no_domain_object() {
    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({ "world": WORLD, "query": QUERY }),
    );
    assert_eq!(
        payload["certificate_sha256"],
        json!("c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4")
    );
    assert!(payload.get("domain").is_none());
    assert!(payload["refine"]["handle"].get("domain").is_none());
}

#[test]
fn a_domain_refinement_handle_recompiles_under_the_pack_oracle() {
    let mut server = server();
    let compiled = call(
        &mut server,
        "fiber_compile",
        json!({ "world": TRADE_WORLD, "query": TRADE_QUERY, "domain": TRADE_DOMAIN }),
    );
    let handle = compiled["refine"]["handle"].clone();
    assert_eq!(handle["domain"], json!(TRADE_DOMAIN));

    let refined = call(
        &mut server,
        "fiber_refine",
        json!({ "handle": handle, "layer": "l2" }),
    );
    assert_eq!(refined["layer"], json!("l2"));
    assert_eq!(refined["verdict"]["status"], json!("invalid"));
    assert!(
        refined["witnesses"]
            .as_array()
            .unwrap()
            .iter()
            .any(|witness| witness["check"] == json!("self_cross"))
    );
}

#[test]
fn a_query_missing_the_packs_tags_and_goal_earns_advisories_not_silent_repair() {
    let staged = repo_root().join("target/mcp-domain-fixtures");
    std::fs::create_dir_all(&staged).unwrap();
    std::fs::write(
        staged.join("goalless_query.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "fiber-query/0.2",
            "query_id": "audit-wash-trade-goalless",
            "targets": ["wash_trade_status"],
            "protected_tags": ["identity", "protected"],
            "decision_time": "2025-03-15T00:00:00Z",
            "budgets": { "max_facts": 16 }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut server = server();
    let payload = call(
        &mut server,
        "fiber_compile",
        json!({
            "world": TRADE_WORLD,
            "query": "target/mcp-domain-fixtures/goalless_query.json",
            "domain": TRADE_DOMAIN
        }),
    );

    let advisories = payload["domain"]["advisories"].as_array().unwrap();
    assert_eq!(advisories.len(), 2, "got {advisories:?}");
    assert!(
        advisories
            .iter()
            .any(|advisory| advisory.as_str().unwrap().contains("\"time\""))
    );
    assert!(
        advisories
            .iter()
            .any(|advisory| advisory.as_str().unwrap().contains("declares no goal"))
    );
}

#[test]
fn a_malformed_domain_pack_is_refused_with_the_parse_error_not_a_crash() {
    let staged = repo_root().join("target/mcp-domain-fixtures");
    std::fs::create_dir_all(&staged).unwrap();
    std::fs::write(
        staged.join("malformed_pack.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "bioprism-domain/0.1",
            "name": "broken",
            "description": "a pack whose oracle declares no checks",
            "oracle": { "kind": "rule/broken-v1", "checks": [] }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut server = server();
    let payload = call(
        &mut server,
        "domain_validate",
        json!({ "domain": "target/mcp-domain-fixtures/malformed_pack.json" }),
    );
    assert_eq!(payload["__isError"], json!(true));
    assert!(payload["error"].as_str().unwrap().contains("no checks"));
}

#[test]
fn a_domain_path_escaping_the_root_is_refused() {
    let mut server = server();
    let compile = call(
        &mut server,
        "fiber_compile",
        json!({ "world": TRADE_WORLD, "query": TRADE_QUERY, "domain": "../../../etc/passwd" }),
    );
    assert_eq!(compile["__isError"], json!(true));
    assert!(compile["error"].as_str().unwrap().contains("refused"));

    let validate = call(
        &mut server,
        "domain_validate",
        json!({ "domain": "..\\outside-pack.json" }),
    );
    assert_eq!(validate["__isError"], json!(true));
    assert!(validate["error"].as_str().unwrap().contains("refused"));
}

#[test]
fn a_project_audit_of_the_demo_app_is_invalid_with_a_witness_naming_the_unpinned_dependency() {
    let mut server = server();
    let payload = call(
        &mut server,
        "project_audit",
        json!({ "root": DEMO_PROJECT }),
    );

    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["verdict"]["status"], json!("invalid"));
    assert_eq!(
        payload["verdict"]["oracle_kind"],
        json!("rule/project-release-readiness-v1")
    );

    let witnesses = payload["verdict"]["witnesses"].as_array().unwrap();
    let unpinned = witnesses
        .iter()
        .find(|witness| {
            witness["type"] == json!("domain_check")
                && witness["check"] == json!("unpinned_dependency")
        })
        .unwrap_or_else(|| panic!("expected the unpinned_dependency witness, got {witnesses:?}"));
    let observed = unpinned["observed"]["unpinned_dependencies"]
        .as_str()
        .expect("the witness carries the bindings the rule read");
    assert!(
        observed.contains("loose-gadget"),
        "the witness must name the unpinned dependency; got {observed}"
    );
    assert!(
        !observed.contains("exact-widget"),
        "the exactly pinned dependency must not appear in the unpinned set; got {observed}"
    );

    assert!(
        payload["loss"]["total"].as_u64().unwrap() > 0,
        "a scan reporting zero loss would be claiming it understood every byte of the tree"
    );
    assert!(!payload["limitations"].as_array().unwrap().is_empty());
}

#[test]
fn a_project_audit_reports_the_compiled_region_of_each_declared_issue() {
    let mut server = server();
    let payload = call(
        &mut server,
        "project_audit",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES }),
    );

    let issues = payload["issues"].as_object().expect("issues object");
    assert_eq!(
        issues.keys().collect::<Vec<_>>(),
        vec!["ISSUE-1", "ISSUE-2"],
        "both declared issues must be reported, in a stable order"
    );

    let region = |issue: &str| -> Vec<String> {
        issues[issue]["selected_facts"]
            .as_array()
            .unwrap_or_else(|| panic!("{issue} has no selected_facts"))
            .iter()
            .map(|fact| fact.as_str().unwrap().to_string())
            .collect()
    };

    let naming_a_component = region("ISSUE-1");
    assert!(
        issues["ISSUE-1"]["query_id"]
            .as_str()
            .unwrap()
            .starts_with("issue-ISSUE-1-"),
        "the region must be traceable to the query that produced it"
    );
    assert!(
        naming_a_component
            .iter()
            .any(|id| id == "fact.component.src"),
        "ISSUE-1 names src/lib.rs, so the src inventory belongs to its region; got {naming_a_component:?}"
    );
    assert!(
        !naming_a_component
            .iter()
            .any(|id| id == "fact.component.assets"),
        "the assets component is named by no issue and must be excluded; got {naming_a_component:?}"
    );
    assert!(
        naming_a_component
            .iter()
            .any(|id| id == "fact.issue.ISSUE-1"),
        "the issue's own record belongs to its region; got {naming_a_component:?}"
    );

    let naming_nothing = region("ISSUE-2");
    assert!(
        !naming_nothing
            .iter()
            .any(|id| id.starts_with("fact.component.")),
        "ISSUE-2 declares no components, so no inventory may be guessed into its region; got {naming_nothing:?}"
    );
    assert!(
        naming_nothing
            .iter()
            .any(|id| id == "fact.aggregate.dependency_declarations"),
        "the aggregate decision inputs are the whole of an undeclared issue's region; got {naming_nothing:?}"
    );
}

#[test]
fn project_ingest_writes_the_assembled_documents_into_a_root_confined_out_dir_and_names_them() {
    let out_dir = "target/mcp-project-ingest";
    let _ = std::fs::remove_dir_all(repo_root().join(out_dir));

    let mut server = server();
    let payload = call(
        &mut server,
        "project_ingest",
        json!({ "root": DEMO_PROJECT, "out_dir": out_dir, "confirm": true }),
    );

    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["performed"], json!(true));
    assert_eq!(
        payload["written"],
        json!([
            "target/mcp-project-ingest/world.json",
            "target/mcp-project-ingest/pack.json",
            "target/mcp-project-ingest/dimensions.json",
            "target/mcp-project-ingest/query.release.json",
        ])
    );
    for reported in payload["written"].as_array().unwrap() {
        let path = repo_root().join(reported.as_str().unwrap());
        assert!(
            path.is_file(),
            "{} was reported written but is not on disk",
            path.display()
        );
    }

    let world: Value = serde_json::from_slice(
        &std::fs::read(repo_root().join(out_dir).join("world.json")).unwrap(),
    )
    .expect("the written world is JSON");
    assert_eq!(
        world["world_id"], payload["world_id"],
        "the reported world id must be the one in the written document"
    );
    assert!(payload["facts"].as_u64().unwrap() > 0);

    let _ = std::fs::remove_dir_all(repo_root().join(out_dir));
}

#[test]
fn project_ingest_previews_exactly_the_paths_confirming_writes_and_creates_none_of_them() {
    let out_dir = "target/mcp-project-ingest-preview";
    let _ = std::fs::remove_dir_all(repo_root().join(out_dir));

    let mut server = server();
    let arguments = json!({
        "root": DEMO_PROJECT,
        "issues": DEMO_PROJECT_ISSUES,
        "out_dir": out_dir,
    });

    let preview = call(&mut server, "project_ingest", arguments.clone());
    assert_eq!(preview["performed"], json!(false));
    assert_eq!(
        preview["written"],
        json!([]),
        "an unconfirmed call has written nothing, so it may claim nothing"
    );
    assert!(
        preview["preview"]["effect"]
            .as_str()
            .unwrap()
            .contains("would write")
    );
    assert!(
        !repo_root().join(out_dir).exists(),
        "the preview created {}, so it was not a preview",
        repo_root().join(out_dir).display()
    );

    let previewed = preview["preview"]["writes"].as_array().unwrap().clone();
    assert!(
        previewed
            .iter()
            .any(|path| path == "target/mcp-project-ingest-preview/query.issue.ISSUE-1.json"),
        "a per-issue query is one of the writes and must appear in the preview; got {previewed:?}"
    );

    let mut confirmed_arguments = arguments;
    confirmed_arguments["confirm"] = json!(true);
    let performed = call(&mut server, "project_ingest", confirmed_arguments);
    assert_eq!(performed["performed"], json!(true));
    assert_eq!(
        performed["written"],
        Value::Array(previewed.clone()),
        "confirming wrote a different set of files than the preview promised"
    );
    for reported in &previewed {
        let path = repo_root().join(reported.as_str().unwrap());
        assert!(
            path.is_file(),
            "{} was previewed and confirmed but is not on disk",
            path.display()
        );
    }

    let _ = std::fs::remove_dir_all(repo_root().join(out_dir));
}

#[test]
fn a_root_escaping_path_is_refused_by_both_project_tools() {
    let mut server = server();
    let escape_attempts = [
        ("project_audit", json!({ "root": "fixtures/../../etc" })),
        ("project_audit", json!({ "root": "fixtures\\..\\..\\etc" })),
        ("project_audit", json!({ "root": "/etc" })),
        ("project_audit", json!({ "root": "C:\\Windows" })),
        (
            "project_audit",
            json!({ "root": DEMO_PROJECT, "issues": "../outside-issues.json" }),
        ),
        (
            "project_audit",
            json!({ "root": DEMO_PROJECT, "issues": "\\etc\\outside-issues.json" }),
        ),
        ("project_ingest", json!({ "root": "..\\..\\etc" })),
        ("project_ingest", json!({ "root": "/etc" })),
        (
            "project_ingest",
            json!({ "root": DEMO_PROJECT, "out_dir": "../mcp-outside-out", "confirm": true }),
        ),
        (
            "project_ingest",
            json!({ "root": DEMO_PROJECT, "out_dir": "C:/mcp-outside-out", "confirm": true }),
        ),
    ];

    for (tool, arguments) in escape_attempts {
        let payload = call(&mut server, tool, arguments.clone());
        assert_eq!(
            payload["__isError"],
            json!(true),
            "{tool} accepted the escaping arguments {arguments}"
        );
        assert!(
            payload["error"].as_str().unwrap().contains("refused"),
            "{tool} refused {arguments} without saying so: {}",
            payload["error"]
        );
    }

    for outside in [
        repo_root().join("..").join("mcp-outside-out"),
        PathBuf::from("C:/mcp-outside-out"),
    ] {
        assert!(
            !outside.exists(),
            "the refused out_dir {} was created anyway",
            outside.display()
        );
    }
}

#[test]
fn project_ingest_run_twice_writes_byte_identical_world_documents() {
    let first_dir = "target/mcp-project-ingest-first";
    let second_dir = "target/mcp-project-ingest-second";
    for dir in [first_dir, second_dir] {
        let _ = std::fs::remove_dir_all(repo_root().join(dir));
    }

    let mut server = server();
    let ingest = |server: &mut Server, out_dir: &str| {
        call(
            server,
            "project_ingest",
            json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "out_dir": out_dir, "confirm": true }),
        )
    };

    let first = ingest(&mut server, first_dir);
    let second = ingest(&mut server, second_dir);
    assert_eq!(first["performed"], json!(true));
    assert_eq!(second["performed"], json!(true));
    assert_eq!(first["world_id"], second["world_id"]);

    let first_bytes = std::fs::read(repo_root().join(first_dir).join("world.json")).unwrap();
    let second_bytes = std::fs::read(repo_root().join(second_dir).join("world.json")).unwrap();
    assert!(
        !first_bytes.is_empty(),
        "an empty world would compare equal to an empty world"
    );
    assert_eq!(
        first_bytes, second_bytes,
        "two ingests of the same tree must write identical world bytes"
    );

    for dir in [first_dir, second_dir] {
        let _ = std::fs::remove_dir_all(repo_root().join(dir));
    }
}

#[test]
fn a_malformed_decision_time_is_refused_by_name_rather_than_as_a_world_validation_failure() {
    let mut server = server();
    for tool in ["project_audit", "project_ingest"] {
        let payload = call(
            &mut server,
            tool,
            json!({ "root": DEMO_PROJECT, "decision_time": "yesterday" }),
        );
        assert_eq!(payload["__isError"], json!(true), "{tool} accepted it");
        let error = payload["error"].as_str().unwrap();
        assert!(
            error.contains("decision_time must be RFC 3339"),
            "{tool} must name the parameter the caller has to edit; got {error}"
        );
        assert!(
            !error.contains("reference validator"),
            "{tool} blamed the emitter for the caller's value; got {error}"
        );
    }
}

#[test]
fn a_region_built_from_an_unresolvable_declaration_is_distinguishable_from_one_declaring_nothing() {
    let issues_dir = "target/mcp-project-unresolvable";
    let issues_path = format!("{issues_dir}/issues.json");
    std::fs::create_dir_all(repo_root().join(issues_dir)).unwrap();
    std::fs::write(
        repo_root().join(&issues_path),
        serde_json::to_vec(&json!([
            { "id": "TYPO", "title": "names a component that is not there", "components": ["srcc"] },
            { "id": "SILENT", "title": "names nothing at all" }
        ]))
        .unwrap(),
    )
    .unwrap();

    let mut server = server();
    let payload = call(
        &mut server,
        "project_audit",
        json!({ "root": DEMO_PROJECT, "issues": issues_path }),
    );

    let issues = payload["issues"].as_object().expect("issues object");
    assert_eq!(
        issues["TYPO"]["unresolved_components"],
        json!(["srcc"]),
        "the declaration that resolved to nothing must be reported verbatim, not dropped"
    );
    assert_eq!(issues["TYPO"]["resolved_components"], json!([]));
    assert_eq!(
        issues["SILENT"]["unresolved_components"],
        json!([]),
        "an issue that declared nothing has nothing unresolved, which is a different claim"
    );
    let region = |issue: &str| -> Vec<String> {
        issues[issue]["selected_facts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fact| fact.as_str().unwrap().to_string())
            .collect()
    };
    let typo = region("TYPO");
    let silent = region("SILENT");
    assert_eq!(
        typo.len(),
        silent.len(),
        "the two regions are the same size and neither carries a component inventory, which is \
         exactly why the declarations have to be reported: {typo:?} vs {silent:?}"
    );
    for selected in [&typo, &silent] {
        assert!(
            !selected.iter().any(|id| id.starts_with("fact.component.")),
            "no component inventory may be guessed into either region; got {selected:?}"
        );
    }

    let _ = std::fs::remove_dir_all(repo_root().join(issues_dir));
}

#[test]
fn repair_verify_reports_not_met_on_an_unrepaired_tree_and_never_that_the_issue_is_resolved() {
    let directory = "target/mcp-repair-unchanged";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let out = format!("{directory}/plan.json");

    let mut server = server();
    let planned = plan_demo_issue_one(&mut server, &out, &[]);
    assert!(
        planned["plan_id"]
            .as_str()
            .unwrap()
            .starts_with("repair-ISSUE-1-")
    );

    let payload = call(
        &mut server,
        "repair_verify",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "plan": out }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["stale"], json!(false));
    assert_eq!(payload["outcome"], json!("not_met"));
    assert_eq!(
        payload["admissibility"],
        json!("undeclared"),
        "a plan declaring no prerequisite has declared none, which is not the same as one holding"
    );

    let items = payload["report"]["items"].as_array().unwrap();
    let status_of = |name: &str| -> String {
        items
            .iter()
            .find(|item| item["name"] == json!(name))
            .unwrap_or_else(|| panic!("{name} is not on the report: {items:?}"))["status"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(status_of("check_cleared:unpinned_dependency"), "unmet");
    assert_eq!(status_of("component_present:src"), "met");
    assert_eq!(status_of("region_evidence_removed"), "unmet");
    assert!(
        payload["report"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line
                .as_str()
                .unwrap()
                .contains("does not state that the issue is resolved")),
        "the report must refuse the claim the tool could be mistaken for: {payload}"
    );

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn repair_verify_reports_staleness_against_a_different_world_without_evaluating_anything() {
    let directory = "target/mcp-repair-stale";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let out = format!("{directory}/plan.json");

    let mut server = server();
    let planned = plan_demo_issue_one(&mut server, &out, &[]);

    let payload = call(
        &mut server,
        "repair_verify",
        json!({ "root": "fixtures/projects/bare-script", "plan": out }),
    );
    assert_ne!(
        payload["__isError"],
        json!(true),
        "staleness is a finding and arrives as a successful call, or a caller discards it with \
         the transport errors: {payload}"
    );
    assert_eq!(payload["stale"], json!(true));
    assert_eq!(payload["report"]["verdict"], json!("stale"));
    assert_eq!(
        payload["report"]["expected_world_id"], planned["world_id"],
        "the report must name the world the plan was planned from"
    );
    assert_ne!(
        payload["report"]["found_world_id"], payload["report"]["expected_world_id"],
        "if the two worlds were the same this test would prove nothing"
    );
    assert_eq!(
        payload["outcome"],
        Value::Null,
        "a stale report has no verdict rather than a neutral one: {payload}"
    );
    assert_eq!(payload["admissibility"], Value::Null);
    assert!(
        payload["report"]["items"].is_null(),
        "nothing was evaluated, so there is no item list to report: {payload}"
    );
    assert!(
        payload["report"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line
                .as_str()
                .unwrap()
                .contains("not a verdict about this plan")),
        "the stale report must say why nothing was evaluated: {payload}"
    );

    let declared = call(
        &mut server,
        "repair_verify",
        json!({
            "root": "fixtures/projects/bare-script",
            "plan": out,
            "succession": {
                "declared_by": "release engineer",
                "statement": "The checked tree is the successor produced by this repair."
            }
        }),
    );
    assert_ne!(
        declared["__isError"],
        json!(true),
        "an explicit named assertion permits evaluating the changed world: {declared}"
    );
    assert_eq!(declared["stale"], json!(false));
    assert_eq!(declared["report"]["verdict"], json!("evaluated"));
    assert_eq!(declared["report"]["binding_matches"], json!(false));
    assert_eq!(
        declared["report"]["succession"],
        json!({
            "declared_by": "release engineer",
            "statement": "The checked tree is the successor produced by this repair."
        }),
        "the named assertion must travel in the report verbatim: {declared}"
    );
    assert!(
        declared["report"]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line
                .as_str()
                .unwrap_or_default()
                .contains("asserted by the caller and is never verified")),
        "the report must not overstate what the succession establishes: {declared}"
    );

    let malformed = call(
        &mut server,
        "repair_verify",
        json!({
            "root": "fixtures/projects/bare-script",
            "plan": out,
            "succession": {
                "declared_by": "release engineer",
                "statement": "The checked tree is the repaired successor.",
                "verified": true
            }
        }),
    );
    assert_eq!(
        malformed["__isError"],
        json!(true),
        "unknown succession fields must fail closed instead of disappearing: {malformed}"
    );

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn repair_plan_previews_exactly_the_path_confirming_writes_and_creates_none_of_it() {
    let directory = "target/mcp-repair-preview";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let out = format!("{directory}/plan.json");

    let mut server = server();
    let arguments = json!({
        "root": DEMO_PROJECT,
        "issues": DEMO_PROJECT_ISSUES,
        "issue": "ISSUE-1",
        "out": out,
    });

    let unasked = call(
        &mut server,
        "repair_plan",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1" }),
    );
    assert_eq!(
        unasked["performed"],
        Value::Null,
        "nobody asked for a write, which is not the same as a write being declined: {unasked}"
    );

    let preview = call(&mut server, "repair_plan", arguments.clone());
    assert_eq!(preview["performed"], json!(false));
    assert_eq!(
        preview["written"],
        json!([]),
        "an unconfirmed call has written nothing, so it may claim nothing"
    );
    assert!(
        preview["preview"]["effect"]
            .as_str()
            .unwrap()
            .contains("would write")
    );
    assert!(
        !repo_root().join(&out).exists(),
        "the preview created {out}, so it was not a preview"
    );
    let previewed = preview["preview"]["writes"].as_array().unwrap().clone();
    assert_eq!(previewed, vec![json!(out)]);

    let mut confirmed_arguments = arguments;
    confirmed_arguments["confirm"] = json!(true);
    let performed = call(&mut server, "repair_plan", confirmed_arguments);
    assert_eq!(performed["performed"], json!(true));
    assert_eq!(
        performed["written"],
        Value::Array(previewed),
        "confirming wrote a different set of files than the preview promised"
    );
    assert!(repo_root().join(&out).is_file());

    let written: Value =
        serde_json::from_slice(&std::fs::read(repo_root().join(&out)).unwrap()).unwrap();
    assert_eq!(
        written, preview["plan"],
        "the document on disk must be the one the preview already showed the caller"
    );
    assert_eq!(written["plan_id"], performed["plan_id"]);

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn a_root_escaping_path_is_refused_by_both_repair_tools() {
    let directory = "target/mcp-repair-confinement";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let out = format!("{directory}/plan.json");
    let mut server = server();
    plan_demo_issue_one(&mut server, &out, &[]);

    let escape_attempts = [
        (
            "repair_plan",
            json!({ "root": "fixtures/../../etc", "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1" }),
        ),
        (
            "repair_plan",
            json!({ "root": "C:\\Windows", "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1" }),
        ),
        (
            "repair_plan",
            json!({ "root": DEMO_PROJECT, "issues": "..\\outside-issues.json", "issue": "ISSUE-1" }),
        ),
        (
            "repair_plan",
            json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1", "criteria": "../outside-criteria.json" }),
        ),
        (
            "repair_plan",
            json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1", "criteria": "/etc/criteria.json" }),
        ),
        (
            "repair_plan",
            json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1", "out": "../mcp-outside-plan.json", "confirm": true }),
        ),
        (
            "repair_plan",
            json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1", "out": "C:/mcp-outside-plan.json", "confirm": true }),
        ),
        (
            "repair_verify",
            json!({ "root": "..\\..\\etc", "plan": out }),
        ),
        (
            "repair_verify",
            json!({ "root": DEMO_PROJECT, "plan": "../outside-plan.json" }),
        ),
        (
            "repair_verify",
            json!({ "root": DEMO_PROJECT, "plan": "\\etc\\outside-plan.json" }),
        ),
    ];

    for (tool, arguments) in escape_attempts {
        let payload = call(&mut server, tool, arguments.clone());
        assert_eq!(
            payload["__isError"],
            json!(true),
            "{tool} accepted the escaping arguments {arguments}"
        );
        assert!(
            payload["error"].as_str().unwrap().contains("refused"),
            "{tool} refused {arguments} without saying so: {}",
            payload["error"]
        );
    }

    for outside in [
        repo_root().join("..").join("mcp-outside-plan.json"),
        PathBuf::from("C:/mcp-outside-plan.json"),
    ] {
        assert!(
            !outside.exists(),
            "the refused out {} was created anyway",
            outside.display()
        );
    }

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn repair_verify_reports_an_unevaluable_declared_criterion_as_neither_met_nor_unmet() {
    let directory = "target/mcp-repair-declared";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let out = format!("{directory}/plan.json");
    let criteria = write_repair_declarations(
        directory,
        json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "criteria": [{
                "name": "ghost_component_inventory_nonempty",
                "statement": "A component the tree does not carry reports a non-empty inventory.",
                "predicate": { "kind": "nonempty", "variable": "component_ghost_inventory" },
                "rationale": "Declared to exercise a criterion no scan of this tree can evaluate."
            }]
        }),
    );

    let mut server = server();
    let planned = plan_demo_issue_one(&mut server, &out, &[("criteria", json!(criteria))]);
    let origin_of = |name: &str| -> String {
        planned["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == json!(name))
            .unwrap_or_else(|| panic!("{name} is not among the plan's items: {planned}"))["origin"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        origin_of("ghost_component_inventory_nonempty"),
        "declared",
        "the caller's criterion must never borrow the authority of an inference"
    );
    assert_eq!(
        origin_of("check_cleared:unpinned_dependency"),
        "derived",
        "and the generator's inference must never be reported as somebody's claim"
    );

    let payload = call(
        &mut server,
        "repair_verify",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "plan": out }),
    );
    let items = payload["report"]["items"].as_array().unwrap();
    let blocked = items
        .iter()
        .find(|item| item["name"] == json!("ghost_component_inventory_nonempty"))
        .expect("the declared criterion is reported");
    assert_eq!(blocked["status"], json!("not_evaluable"));
    assert_eq!(
        blocked["obstruction"]["variable"],
        json!("component_ghost_inventory"),
        "the third status exists to name what stopped the check: {blocked}"
    );
    assert!(
        items.iter().any(|item| item["status"] == json!("unmet")),
        "a determinate failure must also be present, or this test does not exercise the ordering: \
         {items:?}"
    );
    assert_eq!(
        payload["outcome"],
        json!("underdetermined"),
        "not_met would presuppose the criteria were all checked, and one was not: {payload}"
    );

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn repair_plan_refuses_an_undeclared_issue_and_an_undeclared_criteria_key_by_name() {
    let directory = "target/mcp-repair-refusals";
    let _ = std::fs::remove_dir_all(repo_root().join(directory));
    let mut server = server();

    let unknown_issue = call(
        &mut server,
        "repair_plan",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-404" }),
    );
    assert_eq!(unknown_issue["__isError"], json!(true));
    let message = unknown_issue["error"].as_str().unwrap();
    assert!(
        message.contains("ISSUE-404") && message.contains("ISSUE-1"),
        "the refusal must name both what was asked for and what is actually declared: {message}"
    );

    let no_issues = call(
        &mut server,
        "repair_plan",
        json!({ "root": DEMO_PROJECT, "issue": "ISSUE-1" }),
    );
    assert_eq!(no_issues["__isError"], json!(true));
    assert!(
        no_issues["error"]
            .as_str()
            .unwrap()
            .contains("required property `issues` is missing"),
        "authoritative schema validation must name the missing declaration input: {}",
        no_issues["error"]
    );

    let criteria = write_repair_declarations(
        directory,
        json!({
            "schema_version": "bioprism-repair-declarations/0.1",
            "falsifier": [{
                "name": "typo_in_the_key",
                "statement": "The author meant falsifiers and wrote falsifier.",
                "predicate": { "kind": "missing", "variable": "component_src_inventory" }
            }]
        }),
    );
    let misspelled = call(
        &mut server,
        "repair_plan",
        json!({ "root": DEMO_PROJECT, "issues": DEMO_PROJECT_ISSUES, "issue": "ISSUE-1", "criteria": criteria }),
    );
    assert_eq!(misspelled["__isError"], json!(true));
    assert!(
        misspelled["error"].as_str().unwrap().contains("falsifier"),
        "the refusal must name the key the author has to fix: {}",
        misspelled["error"]
    );

    let _ = std::fs::remove_dir_all(repo_root().join(directory));
}

#[test]
fn writing_tools_preview_before_they_act() {
    let mut server = server();
    let store = "target/mcp-preview-store";
    let _ = std::fs::remove_dir_all(repo_root().join(store));

    let preview = call(
        &mut server,
        "world_index",
        json!({ "world": WORLD, "store": store }),
    );
    assert_eq!(preview["performed"], json!(false));
    assert!(
        preview["preview"]["effect"]
            .as_str()
            .unwrap()
            .contains("would write")
    );
    assert!(
        !repo_root().join(store).exists(),
        "preview must not create the store"
    );

    let performed = call(
        &mut server,
        "world_index",
        json!({ "world": WORLD, "store": store, "confirm": true }),
    );
    assert_eq!(performed["performed"], json!(true));
    assert_eq!(performed["facts"], json!(761));
    assert!(repo_root().join(store).join("manifest.json").exists());

    let _ = std::fs::remove_dir_all(repo_root().join(store));
}

#[test]
fn a_tampered_certificate_fails_verification() {
    let mut server = server();
    let good = call(
        &mut server,
        "fiber_verify",
        json!({ "certificate": "fixtures/fiber-v0.1/golden/reference_certificate.json" }),
    );
    assert_eq!(good["verified"], json!(true));

    let mut document: Value = serde_json::from_str(
        &std::fs::read_to_string(
            repo_root().join("fixtures/fiber-v0.1/golden/reference_certificate.json"),
        )
        .unwrap(),
    )
    .unwrap();
    document["selected_facts"]
        .as_array_mut()
        .unwrap()
        .push(json!("fact.smuggled"));
    let tampered = repo_root().join("target/mcp-tampered.json");
    std::fs::create_dir_all(tampered.parent().unwrap()).unwrap();
    std::fs::write(&tampered, serde_json::to_string_pretty(&document).unwrap()).unwrap();

    let bad = call(
        &mut server,
        "fiber_verify",
        json!({ "certificate": "target/mcp-tampered.json" }),
    );
    assert_eq!(bad["verified"], json!(false));
    assert!(bad["detail"].as_str().unwrap().contains("digest mismatch"));

    let _ = std::fs::remove_file(tampered);
}
