//! MCP contract tests for agent workflows contracts.

use super::*;

#[test]
fn domain_workflow_catalogue_covers_every_capability_group() {
    let mut server = server();
    let report = call(&mut server, "domain_workflow_catalogue", json!({}));
    assert_eq!(report["workflow"], json!("domain_workflow_catalogue"));
    assert_eq!(report["workflow_count"], json!(CAPABILITY_GROUP_COUNT));
    assert_eq!(
        report["coverage"]["group_count"],
        json!(CAPABILITY_GROUP_COUNT)
    );
    assert_eq!(report["coverage"]["all_groups_have_workflow"], json!(true));
    assert_eq!(
        report["coverage"]["all_declared_tools_advertised"],
        json!(true)
    );
    assert_eq!(
        report["coverage"]["all_workflows_have_domain_contract"],
        json!(true)
    );
    assert_eq!(report["execution"], json!("not_started"));
    assert!(
        report["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|workflow| {
                workflow["workflow_id"].is_string()
                    && workflow["workflow_digest"].is_string()
                    && workflow["domain_contract"].is_object()
                    && workflow["tool_contracts"].is_array()
                    && workflow["recommended_stages"].is_array()
                    && workflow["tool_contracts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|contract| contract["argument_contract"].is_object())
            })
    );
}

#[test]
fn domain_workflow_scaffolds_are_actionable_and_execution_disabled_for_every_group() {
    let mut server = server();
    let catalogue = call(&mut server, "domain_workflow_catalogue", json!({}));
    let workflows = catalogue["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), CAPABILITY_GROUP_COUNT);

    for workflow in workflows {
        let workflow_id = workflow["workflow_id"].as_str().unwrap();
        let report = call(
            &mut server,
            "domain_workflow_scaffold",
            json!({
                "workflow_id": workflow_id,
                "mission_id": format!("scaffold-{workflow_id}"),
                "goal": format!("prepare a reviewed starting plan for {workflow_id}")
            }),
        );
        assert_eq!(
            report["ok"], true,
            "scaffold failed for {workflow_id}: {report}"
        );
        assert_eq!(report["workflow"], "domain_workflow_scaffold");
        assert_eq!(report["execution"], "not_started");
        assert_eq!(report["mission"]["policy"]["execute"], false);
        assert_eq!(
            report["mission"]["workflow_binding"]["workflow_id"],
            workflow_id
        );
        assert!(
            !report["selection"]["selected_tools"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            report["preflight_status"].as_str(),
            Some("ready") | Some("blocked")
        ));
        assert_eq!(report["preflight_report"]["dispatch"], "not_started");
        assert_eq!(report["preflight_report"]["preflight"], true);
        assert_eq!(report["readiness_claimed"], false);
        assert!(report["next_actions"].as_array().unwrap().len() >= 2);
    }
}

#[test]
fn domain_workflow_bindings_cover_every_available_capability_group() {
    let capabilities = bioprism_mcp::workspace_capabilities();
    let definitions = Value::Array(on_a_dispatch_sized_stack(tool_definitions));
    let catalogue = build_domain_workflow_catalogue(&capabilities, &definitions).unwrap();
    let workflows = catalogue["workflows"].as_array().unwrap();
    assert_eq!(workflows.len(), CAPABILITY_GROUP_COUNT);

    for workflow in workflows {
        let workflow_id = workflow["workflow_id"].as_str().unwrap();
        let tool = workflow["tools"]["available"]
            .as_array()
            .and_then(|tools| tools.first())
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("workflow {workflow_id} has no available tool"));
        let mission_id = format!("all-domain-binding-{workflow_id}");
        let report = instantiate_domain_workflow(
            &capabilities,
            &definitions,
            &json!({
                "workflow_id": workflow_id,
                "mission_id": mission_id,
                "goal": format!("exercise the {workflow_id} contract"),
                "steps": [{
                    "id": "contract-probe",
                    "tool": tool,
                    "arguments": {}
                }],
                "policy": {"execute": false}
            }),
        )
        .unwrap_or_else(|error| panic!("workflow {workflow_id} failed to instantiate: {error}"));
        let binding = &report["mission"]["workflow_binding"];
        assert_eq!(binding["workflow_id"], workflow["workflow_id"]);
        assert_eq!(binding["workflow_digest"], workflow["workflow_digest"]);
        assert_eq!(binding["catalog_digest"], workflow["catalog_digest"]);
        assert_eq!(
            binding["domain_contract_digest"],
            workflow["domain_contract_digest"]
        );
        assert_eq!(binding["domain_contract"], workflow["domain_contract"]);
        assert_eq!(binding["evidence_plan"], report["evidence_plan"]);
        assert_eq!(
            binding["evidence_plan_digest"],
            ContentHash::of_value(&report["evidence_plan"])
                .unwrap()
                .to_string()
        );
        let verification = bioprism_devplat::verify_domain_workflow(
            &capabilities,
            &definitions,
            &json!({
                "instantiation": report,
                "replay_request": {
                    "workflow_id": workflow_id,
                    "mission_id": mission_id,
                    "goal": format!("exercise the {workflow_id} contract"),
                    "steps": [{
                        "id": "contract-probe",
                        "tool": tool,
                        "arguments": {}
                    }],
                    "policy": {"execute": false}
                }
            }),
        )
        .unwrap_or_else(|error| panic!("workflow {workflow_id} failed verification: {error}"));
        assert_eq!(verification["structural_valid"], true);
        assert_eq!(verification["replay"]["status"], "matched");
        assert_eq!(verification["mismatches"], json!([]));
    }
}

#[test]
fn domain_workflow_portfolio_preflights_every_capability_group_without_dispatch() {
    let mut server = server();
    let catalogue = call(&mut server, "domain_workflow_catalogue", json!({}));
    let requests = catalogue["workflows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|workflow| {
            let workflow_id = workflow["workflow_id"].as_str().unwrap();
            let tool = workflow["tools"]["available"]
                .as_array()
                .and_then(|tools| tools.first())
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("workflow {workflow_id} has no available tool"));
            json!({
                "workflow_id": workflow_id,
                "mission_id": format!("portfolio-{workflow_id}"),
                "goal": format!("prepare a bounded portfolio plan for {workflow_id}"),
                "steps": [{"id": "portfolio-probe", "tool": tool, "arguments": {}}],
                "policy": {"execute": true}
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), CAPABILITY_GROUP_COUNT);

    let portfolio = call(
        &mut server,
        "domain_workflow_portfolio",
        json!({
            "requests": requests,
            "policy": {"allow_partial": true, "require_complete_catalogue": true}
        }),
    );
    assert_eq!(portfolio["workflow"], "domain_workflow_portfolio");
    assert_eq!(portfolio["valid"], false);
    assert_eq!(portfolio["portfolio_ready"], false);
    assert_eq!(portfolio["portfolio_status"], "partial");
    assert_eq!(portfolio["coverage"]["complete_catalogue"], true);
    assert_eq!(
        portfolio["summary"]["instantiated_count"],
        CAPABILITY_GROUP_COUNT
    );
    assert_eq!(portfolio["summary"]["blocked_count"], 0);
    assert!(
        portfolio["summary"]["preflight_blocked_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(portfolio["summary"]["preflight_status"], "blocked");
    assert_eq!(
        portfolio["items"].as_array().unwrap().len(),
        CAPABILITY_GROUP_COUNT
    );
    for item in portfolio["items"].as_array().unwrap() {
        assert!(matches!(
            item["status"].as_str(),
            Some("instantiated") | Some("blocked_by_mission_preflight")
        ));
        assert!(item["mission_preflight"]["matched"].is_boolean());
        assert_eq!(item["mission_preflight"]["dispatch"], "not_started");
        assert_eq!(item["instantiation"]["execution"], "not_started");
    }
    assert_eq!(portfolio["dispatch"], "not_started");
    assert_eq!(portfolio["execution"], "not_started");
}

#[test]
fn domain_workflow_reconciliation_preserves_outcomes_for_every_capability_group() {
    on_a_dispatch_sized_stack(|| {
        let capabilities = bioprism_mcp::workspace_capabilities();
        let definitions = Value::Array(tool_definitions());
        let catalogue = build_domain_workflow_catalogue(&capabilities, &definitions).unwrap();
        let workflows = catalogue["workflows"].as_array().unwrap();
        assert_eq!(workflows.len(), CAPABILITY_GROUP_COUNT);

        for workflow in workflows {
            let workflow_id = workflow["workflow_id"].as_str().unwrap();
            let tool = workflow["tools"]["available"]
                .as_array()
                .and_then(|tools| tools.first())
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("workflow {workflow_id} has no available tool"));
            let instantiation = instantiate_domain_workflow(
                &capabilities,
                &definitions,
                &json!({
                    "workflow_id": workflow_id,
                    "mission_id": format!("all-domain-reconcile-{workflow_id}"),
                    "goal": format!("exercise retained evidence states for {workflow_id}"),
                    "steps": [{"id": "outcome", "tool": tool, "arguments": {}}],
                    "policy": {"execute": true}
                }),
            )
            .unwrap_or_else(|error| {
                panic!("workflow {workflow_id} failed to instantiate: {error}")
            });
            let request: MissionRequest =
                serde_json::from_value(instantiation["mission"].clone()).unwrap();
            let plan = plan_mission(&request).unwrap();
            let step = &plan.steps[0];
            let report = |status: &str, wire: Option<Value>| {
                json!({
                    "ok": true,
                    "workflow": "agent_mission",
                    "schema_version": "bioprism-devplat-mission/0.1",
                    "plan": serde_json::to_value(&plan).unwrap(),
                    "execution": "executed",
                    "mission_status": if status == "succeeded" { "succeeded" } else { "failed" },
                    "succeeded": usize::from(status == "succeeded"),
                    "refused": usize::from(status == "refused"),
                    "blocked": usize::from(status == "blocked"),
                    "cancelled": usize::from(status == "cancelled"),
                    "required_failures": usize::from(status != "succeeded"),
                    "returned_bytes": if wire.is_some() { 12 } else { 0 },
                    "results": [{
                        "id": step.id,
                        "tool": step.tool,
                        "status": status,
                        "required": step.required,
                        "arguments_digest": "a".repeat(64),
                        "bytes": if wire.is_some() { 12 } else { 0 },
                        "wire": wire,
                        "error": if status == "succeeded" { Value::Null } else { json!("explicit refusal") }
                    }],
                    "execution_trace_schema_version": "bioprism-devplat-mission-trace/0.1",
                    "execution_trace": [
                        {"sequence": 0, "event": "mission.started", "wave": null, "step_id": null, "tool": null, "status": "running", "arguments_digest": null, "bytes": 0, "detail": null},
                        {"sequence": 1, "event": "mission.completed", "wave": null, "step_id": null, "tool": null, "status": if status == "succeeded" { "succeeded" } else { "failed" }, "arguments_digest": null, "bytes": if wire.is_some() { 12 } else { 0 }, "detail": null}
                    ],
                    "claim_requests": [],
                    "claim_lineage": {},
                    "guarantees": [],
                    "limitations": []
                })
            };

            let success = reconcile_domain_workflow(&json!({
                "instantiation": instantiation,
                "mission_report": report("succeeded", Some(json!({"result": {"ok": true}})))
            }))
            .unwrap_or_else(|error| {
                panic!("workflow {workflow_id} success reconciliation failed: {error}")
            });
            assert_eq!(success["completion"]["status"], "complete");
            assert_eq!(success["completion"]["ready"], true);

            let refused = reconcile_domain_workflow(&json!({
                "instantiation": instantiation,
                "mission_report": report("refused", None)
            }))
            .unwrap_or_else(|error| {
                panic!("workflow {workflow_id} refusal reconciliation failed: {error}")
            });
            assert_eq!(refused["completion"]["status"], "failed");
            assert_eq!(refused["completion"]["ready"], false);
            assert_eq!(
                refused["evidence"]["rows"][0]["evidence_state"],
                "explicit_refusal"
            );

            let omitted = reconcile_domain_workflow(&json!({
                "instantiation": instantiation,
                "mission_report": report("succeeded", None)
            }))
            .unwrap_or_else(|error| {
                panic!("workflow {workflow_id} omission reconciliation failed: {error}")
            });
            assert_eq!(
                omitted["completion"]["status"],
                "complete_with_output_omissions"
            );
            assert_eq!(omitted["completion"]["ready"], false);
            assert_eq!(
                omitted["evidence"]["rows"][0]["evidence_state"],
                "completed_output_omitted"
            );

            let mut mismatched_report = report("succeeded", Some(json!({"result": {"ok": true}})));
            mismatched_report["plan"]["digest"] = json!("b".repeat(64));
            let mismatched = reconcile_domain_workflow(&json!({
                "instantiation": instantiation,
                "mission_report": mismatched_report
            }))
            .unwrap_or_else(|error| {
                panic!("workflow {workflow_id} mismatch reconciliation failed: {error}")
            });
            assert_eq!(mismatched["integrity"]["valid"], false);
            assert_eq!(mismatched["completion"]["ready"], false);
            assert!(
                mismatched["integrity"]["findings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|finding| finding["code"] == "mission_plan_digest_mismatch")
            );
        }
    })
}

#[test]
fn domain_workflow_instantiation_is_scoped_and_preflighted_without_dispatch() {
    let mut server = server();
    let report = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-test",
            "goal": "discover the repository capability surface",
            "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}]
        }),
    );
    assert_eq!(report["workflow"], json!("domain_workflow_instantiate"));
    assert_eq!(
        report["mission"]["steps"][0]["tool"],
        json!("workspace_capabilities")
    );
    assert_eq!(
        report["selection"]["all_selected_tools_declared"],
        json!(true)
    );
    assert_eq!(
        report["selection"]["all_selected_tools_available"],
        json!(true)
    );
    assert_eq!(
        report["evidence_plan"]["steps"][0]["step_id"],
        json!("catalog")
    );
    assert_eq!(
        report["domain_contract"]["posture"],
        json!("advisory_review_gated")
    );
    assert_eq!(report["execution"], json!("not_started"));
    assert_eq!(
        report["preflight_report"]["workflow"],
        json!("agent_mission")
    );

    let verified = call(
        &mut server,
        "domain_workflow_verify",
        json!({
            "instantiation": report.clone(),
            "replay_request": {
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "workflow-test",
                "goal": "discover the repository capability surface",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}]
            }
        }),
    );
    assert_eq!(verified["workflow"], json!("domain_workflow_verify"));
    assert_eq!(verified["valid"], json!(true));
    assert_eq!(verified["verification_status"], json!("verified"));
    assert_eq!(verified["replay"]["matched"], json!(true));
    assert_eq!(verified["mission_preflight"]["matched"], json!(true));
    assert_eq!(verified["dispatch"], json!("not_started"));
    assert_eq!(verified["execution"], json!("not_started"));

    let portfolio = call(
        &mut server,
        "domain_workflow_portfolio",
        json!({
            "requests": [{
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "workflow-portfolio-single",
                "goal": "prepare the repository capability surface",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}]
            }]
        }),
    );
    assert_eq!(portfolio["workflow"], json!("domain_workflow_portfolio"));
    assert_eq!(portfolio["valid"], json!(true));
    assert_eq!(portfolio["summary"]["preflight_status"], json!("matched"));
    assert_eq!(
        portfolio["items"][0]["mission_preflight"]["matched"],
        json!(true)
    );
    assert_eq!(portfolio["dispatch"], json!("not_started"));

    let mut retained_portfolio = portfolio.clone();
    retained_portfolio
        .as_object_mut()
        .unwrap()
        .remove("__isError");
    let portfolio_verified = call(
        &mut server,
        "domain_workflow_portfolio_verify",
        json!({
            "portfolio": retained_portfolio,
            "replay_requests": [{
                "workflow_id": "documentation_and_knowledge",
                "mission_id": "workflow-portfolio-single",
                "goal": "prepare the repository capability surface",
                "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}]
            }],
            "policy": {"require_replay": true}
        }),
    );
    assert_eq!(
        portfolio_verified["workflow"],
        json!("domain_workflow_portfolio_verify")
    );
    assert_eq!(portfolio_verified["valid"], json!(true));
    assert_eq!(portfolio_verified["verification_status"], json!("verified"));
    assert_eq!(portfolio_verified["summary"]["verified_count"], json!(1));
    assert_eq!(
        portfolio_verified["summary"]["replay_matched_count"],
        json!(1)
    );
    assert_eq!(portfolio_verified["items"][0]["status"], json!("verified"));
    assert_eq!(
        portfolio_verified["items"][0]["mission_preflight"]["matched"],
        json!(true)
    );
    assert_eq!(portfolio_verified["dispatch"], json!("not_started"));
    assert_eq!(portfolio_verified["execution"], json!("not_started"));

    let shape_only = call(
        &mut server,
        "domain_workflow_verify",
        json!({"instantiation": report.clone()}),
    );
    assert_eq!(shape_only["valid"], json!(true));
    assert_eq!(
        shape_only["verification_status"],
        json!("verified_without_replay")
    );

    let mut tampered = report.clone();
    tampered["mission"]["goal"] = json!("tampered workflow goal");
    let refused_tamper = call(
        &mut server,
        "domain_workflow_verify",
        json!({"instantiation": tampered}),
    );
    assert_eq!(refused_tamper["valid"], json!(false));
    assert_eq!(refused_tamper["verification_status"], json!("mismatch"));
    assert!(
        refused_tamper["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["code"] == "mission_plan_digest_mismatch")
    );

    let refused = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-refused",
            "goal": "must refuse cross-group selection",
            "steps": [{"id": "compile", "tool": "bioql_compile"}]
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("outside workflow")
    );
}

#[test]
fn domain_workflow_reconciliation_binds_execution_results_to_the_instantiated_contract() {
    let mut server = server();
    let instantiation = call(
        &mut server,
        "domain_workflow_instantiate",
        json!({
            "workflow_id": "documentation_and_knowledge",
            "mission_id": "workflow-reconcile",
            "goal": "reconcile repository capability evidence",
            "steps": [{"id": "catalog", "tool": "workspace_capabilities", "arguments": {}}],
            "policy": {"execute": true}
        }),
    );
    let mission = call(
        &mut server,
        "agent_mission",
        instantiation["mission"].clone(),
    );
    assert_eq!(mission["mission_status"], json!("succeeded"));
    let reconciled = call(
        &mut server,
        "domain_workflow_reconcile",
        json!({"instantiation": instantiation, "mission_report": mission}),
    );
    assert_eq!(reconciled["workflow"], json!("domain_workflow_reconcile"));
    assert_eq!(reconciled["integrity"]["valid"], json!(true));
    assert_eq!(reconciled["completion"]["status"], json!("complete"));
    assert_eq!(reconciled["completion"]["ready"], json!(true));
    assert_eq!(reconciled["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        reconciled["artifact_registry"]["kind"],
        json!("workflow_reconciliation")
    );
    assert_eq!(
        reconciled["evidence"]["rows"][0]["result_retained"],
        json!(true)
    );

    let imported = call(
        &mut server,
        "domain_workflow_reconciliation_import",
        json!({"record": reconciled}),
    );
    assert_eq!(
        imported["workflow"],
        json!("domain_workflow_reconciliation_import")
    );
    // Executed workflow-bound missions are reconciled and indexed automatically; the explicit
    // import below must therefore exercise the registry's idempotent re-import path.
    assert_eq!(imported["created"], json!(false));
    assert_eq!(imported["already_present"], json!(true));
    assert_eq!(imported["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        imported["artifact_registry"]["content_digest"],
        reconciled["artifact_registry"]["content_digest"]
    );
    let digest = imported["reconciliation_digest"].as_str().unwrap();
    let queried = call(
        &mut server,
        "domain_workflow_reconciliation_query",
        json!({"mission_id": "workflow-reconcile", "completion_status": "complete"}),
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(queried["rows"][0]["reconciliation_digest"], json!(digest));
    let fetched = call(
        &mut server,
        "domain_workflow_reconciliation_get",
        json!({"reconciliation_digest": digest}),
    );
    assert_eq!(
        fetched["workflow"],
        json!("domain_workflow_reconciliation_get")
    );
    assert_eq!(fetched["record"]["reconciliation_digest"], json!(digest));
}

#[test]
fn mission_evaluator_discovery_covers_domains_without_executing_tools() {
    let mut server = server();
    let all = call(&mut server, "mission_evaluator_discover", json!({}));
    assert_eq!(all["ok"], json!(true));
    assert_eq!(all["workflow"], json!("mission_evaluator_discover"));
    assert_eq!(all["selection_posture"], json!("candidate_only"));
    assert_eq!(all["total_adapters"], json!(29));
    assert_eq!(all["result_count"], json!(29));
    assert_eq!(
        all["coverage"]["capability_group_count"],
        json!(CAPABILITY_GROUP_COUNT)
    );
    assert_eq!(all["coverage"]["evaluator_group_count"], json!(29));
    assert_eq!(all["coverage"]["complete"], json!(false));
    assert_eq!(
        all["matches"][0]["adapter"]["status"],
        json!("candidate_only")
    );
    assert!(
        all["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "candidate tools are suggestions and were not executed")
    );

    let oncology = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "level": "evaluation", "max_items": 4}),
    );
    assert_eq!(oncology["result_count"], json!(1));
    assert_eq!(
        oncology["matches"][0]["adapter"]["id"],
        json!("oncoworlds.assay_fidelity")
    );
    assert_eq!(
        oncology["matches"][0]["adapter"]["group_id"],
        json!("oncoworlds_models_and_assays")
    );
}

#[test]
fn mission_evaluator_review_builds_claim_bindings_and_blocks_adversarial_rows() {
    let mut server = server();
    let discovery = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "level": "evaluation", "max_items": 4}),
    );
    let ready = call(
        &mut server,
        "mission_evaluator_review",
        json!({
            "discovery": discovery,
            "selections": [{
                "id": "assay-evaluator",
                "claim_id": "fidelity-claim",
                "adapter_id": "oncoworlds.assay_fidelity",
                "domain": "oncology",
                "step_id": "assay",
                "output_pointer": "/fidelity",
                "required": true
            }]
        }),
    );
    assert_eq!(ready["ok"], json!(true));
    assert_eq!(ready["workflow"], json!("mission_evaluator_review"));
    assert_eq!(ready["review_status"], json!("ready"));
    assert_eq!(
        ready["binding_posture"],
        json!("ready_for_mission_claim_bindings")
    );
    assert_eq!(ready["bindings"][0]["binding_posture"], json!("ready"));
    assert_eq!(
        ready["bindings"][0]["proposed_binding"]["step_id"],
        json!("assay")
    );
    assert!(
        ready["bindings"][0]["proposed_binding"]
            .get("claim_id")
            .is_none()
    );
    assert_eq!(ready["execution"], json!("not_started"));

    let mission = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "reviewed-fidelity",
            "goal": "retain a reviewed evaluator output",
            "steps": [{
                "id": "assay",
                "domain": "oncology",
                "capability": "assay",
                "objective": "retain assay evidence",
                "tool": "workspace_capabilities"
            }],
            "claim_requests": [{
                "id": "fidelity-claim",
                "claim": "The assay output was retained for review.",
                "domains": ["oncology"],
                "requires_steps": ["assay"],
                "level": "evaluation",
                "evidence_mode": "successful_tool_result",
                "evaluator_bindings": [ready["bindings"][0]["proposed_binding"].clone()]
            }],
            "evaluator_review": ready
        }),
    );
    assert_eq!(
        mission["workflow"],
        json!("agent_mission"),
        "unexpected mission response: {mission}"
    );
    assert_eq!(mission["execution"], json!("planned"));
    assert_eq!(
        mission["claim_lineage"]["evaluator_review"]["present"],
        json!(true)
    );
    assert_eq!(
        mission["claim_lineage"]["claims"][0]["evaluator_review"]["review_status"],
        json!("ready")
    );

    let replay = call(
        &mut server,
        "mission_evaluator_replay",
        json!({"mission": mission, "include_fixtures": true, "max_items": 64}),
    );
    assert_eq!(replay["workflow"], json!("mission_evaluator_replay"));
    assert_eq!(replay["execution"], json!("not_started"));
    assert_eq!(replay["coverage"]["catalogue_adapter_count"], json!(29));
    assert_eq!(replay["fixtures"].as_array().unwrap().len(), 29);
    assert_eq!(
        replay["fixtures"][0]["variants"].as_array().unwrap().len(),
        4
    );
    assert_eq!(replay["coverage"]["complete"], json!(false));

    let comparison = call(
        &mut server,
        "mission_evaluator_replay_compare",
        json!({"mission": mission.clone(), "include_fixtures": false, "max_items": 64}),
    );
    assert_eq!(
        comparison["workflow"],
        json!("mission_evaluator_replay_compare")
    );
    assert_eq!(comparison["catalog_drift"]["status"], json!("unchanged"));
    assert_eq!(comparison["catalog_drift"]["digest_match"], json!(true));
    let mut drifted_mission = mission.clone();
    drifted_mission["claim_lineage"]["evaluator_review"]["catalog_digest"] = json!("a".repeat(64));
    let drifted_comparison = call(
        &mut server,
        "mission_evaluator_replay_compare",
        json!({"mission": drifted_mission, "include_fixtures": false, "max_items": 64}),
    );
    assert_eq!(
        drifted_comparison["catalog_drift"]["status"],
        json!("drifted")
    );

    let mut bundle = json!({
        "schema": "bioprism-api/mission-evidence-bundle/0.1",
        "workflow": "mission_evidence_bundle_export",
        "mission_id": "mission-protocol",
        "retention": {"mode": "summary_only", "result_retained": false, "result_included": false, "summary_retained": true},
        "result": Value::Null,
        "result_digest": "d".repeat(64),
        "evaluator_replay": {"workflow": "mission_evaluator_replay_summary"},
        "catalog_drift": {"status": "not_recorded"},
        "trace": [{"sequence": 1, "event": "mission_succeeded"}],
        "export": {"format": "json", "include_result": false, "include_trace": true, "trace_included": true, "digest_algorithm": "sha256", "execution": "not_started"}
    });
    bundle["bundle_digest"] = json!(ContentHash::of_value(&bundle).unwrap().to_string());
    let verified = call(
        &mut server,
        "mission_evidence_bundle_verify",
        json!({"bundle": bundle.clone()}),
    );
    assert_eq!(
        verified["workflow"],
        json!("mission_evidence_bundle_verify")
    );
    assert_eq!(verified["valid"], json!(true));
    let imported_bundle = call(
        &mut server,
        "mission_evidence_bundle_import",
        json!({"bundle": bundle.clone()}),
    );
    assert_eq!(imported_bundle["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        imported_bundle["artifact_registry"]["kind"],
        json!("mission_evidence_bundle")
    );
    bundle["catalog_drift"]["status"] = json!("drifted");
    let tampered = call(
        &mut server,
        "mission_evidence_bundle_verify",
        json!({"bundle": bundle}),
    );
    assert_eq!(tampered["valid"], json!(false));

    let replayed_inconsistent = call(
        &mut server,
        "mission_evaluator_replay",
        json!({"mission": {"workflow": "agent_mission", "plan": {"mission_id": "replay-inconsistent"}, "mission_status": "planned", "claim_lineage": {"claims": [{"id": "fidelity-claim", "evaluator_bindings": [{"id": "assay-evaluator", "adapter_id": "oncoworlds.assay_fidelity", "domain": "oncology", "step_id": "assay", "output_pointer": "/fidelity", "required": true, "outcome_state": "retained", "output_digest": "x".repeat(64)}], "evaluator_coverage": {"outcome_counts": {}, "distinct_output_digests": 1, "disagreement_posture": "single_observation"}}]}}}),
    );
    assert_eq!(replayed_inconsistent["replay_status"], json!("blocked"));
    assert!(
        replayed_inconsistent["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| { finding["code"] == json!("outcome_count_mismatch") })
    );

    let mut mismatched_review = ready.clone();
    mismatched_review["bindings"][0]["domain"] = json!("unrelated");
    let rejected = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "reviewed-fidelity-rejected",
            "goal": "reject a stale binding",
            "steps": [{"id": "assay", "domain": "oncology", "capability": "assay", "objective": "retain", "tool": "workspace_capabilities"}],
            "claim_requests": [{
                "id": "fidelity-claim",
                "claim": "The assay output was retained for review.",
                "domains": ["oncology"],
                "requires_steps": ["assay"],
                "evaluator_bindings": [ready["bindings"][0]["proposed_binding"].clone()]
            }],
            "evaluator_review": mismatched_review
        }),
    );
    assert_eq!(rejected["__isError"], json!(true));

    let discovery_for_blocked = call(
        &mut server,
        "mission_evaluator_discover",
        json!({"query": "oncology fidelity", "max_items": 4}),
    );
    let blocked = call(
        &mut server,
        "mission_evaluator_review",
        json!({
            "discovery": discovery_for_blocked,
            "selections": [
                {
                    "id": "duplicate",
                    "claim_id": "fidelity-claim",
                    "adapter_id": "oncoworlds.assay_fidelity",
                    "domain": "unrelated-domain",
                    "step_id": "assay",
                    "output_pointer": "/bad~2pointer"
                },
                {
                    "id": "duplicate",
                    "claim_id": "fidelity-claim",
                    "adapter_id": "not-in-discovery",
                    "domain": "oncology",
                    "step_id": "assay-2",
                    "output_pointer": ""
                }
            ]
        }),
    );
    assert_eq!(blocked["review_status"], json!("blocked"));
    assert_eq!(
        blocked["binding_posture"],
        json!("requires_caller_correction")
    );
    assert!(blocked["findings"].as_array().unwrap().len() >= 4);
    assert!(
        blocked["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["message"] == json!("selection.id must be unique within the review")
            })
    );
    assert!(
        blocked["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["message"]
                    == json!("selection.output_pointer must be a valid RFC 6901 pointer")
            })
    );
}

#[test]
fn agent_mission_plans_and_executes_allow_listed_cross_domain_steps() {
    let mut server = server();
    let planned = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-plan-1",
            "goal": "prepare a cross-domain evidence review",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "metrics", "domain": "metrics", "capability": "analytics", "objective": "prepare measurements", "tool": "metrics_analytics_audit", "arguments": {"observations": []}, "depends_on": ["catalog"]}
            ]
        }),
    );
    assert_eq!(planned["__isError"], json!(false));
    assert_eq!(planned["workflow"], json!("agent_mission"));
    assert_eq!(planned["execution"], json!("planned"));
    assert_eq!(planned["plan"]["critical_path_length"], json!(2));
    assert_eq!(
        planned["plan"]["ordered_steps"],
        json!(["catalog", "metrics"])
    );
    assert_eq!(planned["results"].as_array().unwrap().len(), 0);
    assert_eq!(planned["plan"]["digest"].as_str().unwrap().len(), 64);
    assert_eq!(planned["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        planned["artifact_registry"]["kind"],
        json!("mission_report")
    );
    assert_eq!(
        planned["artifact_registry"]["subject_id"],
        json!("mission-plan-1")
    );

    let executed = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-execute-1",
            "goal": "execute safe local discovery",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "dashboard", "domain": "workspace", "capability": "report", "objective": "summarize the discovered capability group", "tool": "capability_dashboard", "arguments": {"group_id": null}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "/0/id", "target_pointer": "/group_id"}]}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities", "capability_dashboard"], "max_total_output_bytes": 2000000}
        }),
    );
    assert_eq!(executed["__isError"], json!(false));
    assert_eq!(executed["execution"], json!("executed"));
    assert_eq!(executed["mission_status"], json!("succeeded"));
    assert_eq!(executed["succeeded"], json!(2));
    assert_eq!(executed["refused"], json!(0));
    assert_eq!(executed["blocked"], json!(0));
    assert_eq!(executed["results"][0]["status"], json!("succeeded"));
    assert!(executed["results"][0]["wire"]["result"].is_object());
    assert_eq!(
        executed["execution_trace"][0]["event"],
        json!("mission.started")
    );
    assert_eq!(
        executed["execution_trace"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["event"],
        json!("mission.completed")
    );
    assert!(
        executed["execution_trace"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event"] == "wave.completed")
    );
    let source_payload: Value = serde_json::from_str(
        executed["results"][0]["wire"]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let expected_arguments_digest =
        ContentHash::of_value(&json!({"group_id": source_payload[0]["id"]}))
            .unwrap()
            .to_string();
    assert_eq!(
        executed["results"][1]["arguments_digest"],
        json!(expected_arguments_digest)
    );
    assert_eq!(executed["artifact_registry"]["indexed"], json!(true));
    assert_eq!(
        executed["artifact_registry"]["content_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
}

#[test]
fn agent_mission_schema_preflight_refuses_materialized_binding_before_dispatch() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-schema-serial",
            "goal": "prove authoritative argument validation",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "compile", "domain": "fiber", "capability": "compile", "objective": "must be refused before dispatch", "tool": "fiber_compile", "arguments": {"world": "fixture.json", "query": "fixture.query.json"}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "", "target_pointer": "/query"}]}
            ],
            "policy": {"execute": true, "allowed_tools": ["workspace_capabilities", "fiber_compile"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["succeeded"], json!(1));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["results"][1]["status"], json!("refused"));
    assert!(
        result["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("authoritative schema validation refused")
    );
    assert!(
        result["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("schema_digest=")
    );
    assert_eq!(
        result["execution_trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["event"] == "step.started" && event["step_id"] == "compile")
            .count(),
        0
    );
}

#[test]
fn agent_mission_parallel_schema_preflight_refuses_before_batch_launch() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-schema-parallel",
            "goal": "prove parallel schema refusal",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "compile", "domain": "fiber", "capability": "compile", "objective": "must be refused before dispatch", "tool": "fiber_compile", "arguments": {"world": "fixture.json", "query": "fixture.query.json"}, "depends_on": ["catalog"], "bindings": [{"from_step": "catalog", "source_pointer": "", "target_pointer": "/query"}]}
            ],
            "policy": {"execute": true, "execution_mode": "parallel_waves", "max_parallelism": 2, "allowed_tools": ["workspace_capabilities", "fiber_compile"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["succeeded"], json!(1));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["results"][1]["status"], json!("refused"));
    assert!(
        result["results"][1]["error"]
            .as_str()
            .unwrap()
            .contains("authoritative schema validation refused")
    );
    assert_eq!(
        result["execution_trace"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["event"] == "step.started" && event["step_id"] == "compile")
            .count(),
        0
    );
}

#[test]
fn agent_mission_cancellation_preserves_a_closed_trace_and_unlaunched_steps() {
    let mut server = server();
    ready(&mut server);
    let cancellation = AtomicBool::new(true);
    let report = server
        .execute_agent_mission_with_cancellation(
            &json!({
                "mission_id": "mission-cancelled-1",
                "goal": "cancel before dispatch",
                "steps": [
                    {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"}
                ],
                "policy": {"execute": true, "allowed_tools": ["workspace_capabilities"]}
            }),
            &cancellation,
        )
        .expect("cancelled mission should still return a report");
    assert_eq!(report["mission_status"], json!("cancelled"));
    assert_eq!(report["cancelled"], json!(1));
    assert_eq!(report["results"][0]["status"], json!("cancelled"));
    assert_eq!(
        report["execution_trace"][0]["event"],
        json!("mission.started")
    );
    assert_eq!(
        report["execution_trace"][1]["event"],
        json!("step.cancelled")
    );
    assert_eq!(
        report["execution_trace"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["event"],
        json!("mission.completed")
    );
}

#[test]
fn agent_mission_executes_independent_parallel_waves_with_deterministic_reporting() {
    let mut server = server();
    let executed = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-parallel-1",
            "goal": "run independent discovery and protocol inspections",
            "steps": [
                {"id": "catalog", "domain": "workspace", "capability": "discovery", "objective": "discover routes", "tool": "workspace_capabilities"},
                {"id": "protocol", "domain": "orchestration", "capability": "catalogue", "objective": "inspect protocol", "tool": "weave_protocol_catalog"},
                {"id": "protocol-extra", "domain": "orchestration", "capability": "catalogue", "objective": "inspect protocol again", "tool": "weave_protocol_catalog"}
            ],
            "policy": {
                "execute": true,
                "execution_mode": "parallel_waves",
                "max_parallelism": 2,
                "allowed_tools": ["workspace_capabilities", "weave_protocol_catalog"],
                "max_step_output_bytes": 3000000,
                "max_total_output_bytes": 10000000
            }
        }),
    );
    assert_eq!(executed["__isError"], json!(false));
    assert_eq!(executed["execution"], json!("executed"));
    assert_eq!(executed["plan"]["execution_mode"], json!("parallel_waves"));
    assert_eq!(executed["plan"]["max_parallelism"], json!(2));
    assert_eq!(executed["plan"]["waves"].as_array().unwrap().len(), 1);
    assert_eq!(
        executed["plan"]["waves"][0],
        json!(["catalog", "protocol", "protocol-extra"])
    );
    assert_eq!(executed["mission_status"], json!("succeeded"));
    assert_eq!(executed["succeeded"], json!(3));
    assert_eq!(executed["refused"], json!(0));
    assert_eq!(executed["blocked"], json!(0));
    assert_eq!(executed["results"].as_array().unwrap().len(), 3);
    assert_eq!(executed["results"][0]["id"], json!("catalog"));
    assert_eq!(executed["results"][0]["status"], json!("succeeded"));
    assert_eq!(executed["results"][1]["id"], json!("protocol"));
    assert_eq!(executed["results"][1]["status"], json!("succeeded"));
    assert_eq!(executed["results"][2]["id"], json!("protocol-extra"));
    assert_eq!(executed["results"][2]["status"], json!("succeeded"));
    let trace = executed["execution_trace"].as_array().unwrap();
    assert_eq!(trace.len(), 10);
    for (sequence, event) in trace.iter().enumerate() {
        assert_eq!(event["sequence"], json!(sequence));
    }
    assert_eq!(trace[0]["event"], json!("mission.started"));
    assert_eq!(trace[1]["event"], json!("wave.started"));
    assert_eq!(trace[trace.len() - 2]["event"], json!("wave.completed"));
    assert_eq!(trace[trace.len() - 1]["event"], json!("mission.completed"));
    assert_eq!(
        trace
            .iter()
            .filter(|event| event["event"] == "step.started")
            .count(),
        3
    );
    assert_eq!(
        trace
            .iter()
            .filter(|event| event["event"] == "step.completed")
            .count(),
        3
    );
    assert_eq!(trace[trace.len() - 1]["bytes"], executed["returned_bytes"]);
}

#[test]
fn agent_mission_parallel_waves_preserve_refusals_and_block_dependents() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-parallel-refusal",
            "goal": "prove parallel refusal propagation",
            "steps": [
                {"id": "bad", "domain": "test", "capability": "refusal", "objective": "invoke an unknown tool", "tool": "not_a_real_tool"},
                {"id": "dependent", "domain": "test", "capability": "blocked", "objective": "must not run", "tool": "workspace_capabilities", "depends_on": ["bad"]}
            ],
            "policy": {
                "execute": true,
                "execution_mode": "parallel_waves",
                "allowed_tools": ["not_a_real_tool", "workspace_capabilities"]
            }
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["plan"]["execution_mode"], json!("parallel_waves"));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["blocked"], json!(1));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["results"][0]["status"], json!("refused"));
    assert_eq!(result["results"][1]["status"], json!("blocked"));
    let trace = result["execution_trace"].as_array().unwrap();
    assert!(trace.iter().any(|event| event["event"] == "step.refused"));
    assert!(trace.iter().any(|event| event["event"] == "step.blocked"));
    assert_eq!(trace.last().unwrap()["event"], json!("mission.completed"));
    assert_eq!(trace.last().unwrap()["status"], json!("failed"));
}

#[test]
fn capability_discovery_routes_across_domains_and_attaches_authoritative_schemas() {
    let mut server = server();
    let result = call(
        &mut server,
        "capability_discover",
        json!({
            "query": "oncology",
            "max_items": 2,
            "include_tools": true
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["workflow"], json!("capability_discover"));
    assert_eq!(result["result_count"], json!(1));
    assert_eq!(
        result["matches"][0]["group"]["id"],
        json!("biological_domains")
    );
    assert!(
        result["matches"][0]["matched_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "onco_response_assess")
    );
    assert!(
        result["matches"][0]["tool_schemas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|schema| schema["name"] == "onco_response_assess")
    );
    assert_eq!(result["schema_attachment"]["requested"], json!(true));
    assert_eq!(result["catalog_digest"].as_str().unwrap().len(), 64);

    let filtered = call(
        &mut server,
        "capability_discover",
        json!({"domain": "release", "tool": "bundle_verify"}),
    );
    assert_eq!(filtered["__isError"], json!(false));
    assert_eq!(filtered["result_count"], json!(1));
    assert_eq!(
        filtered["matches"][0]["matched_tools"],
        json!(["bundle_verify"])
    );
}

#[test]
fn adapter_plan_routes_biological_formats_without_sniffing_or_execution() {
    let mut server = server();
    let unknown = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "scan-1",
            "source_kind": "bytes",
            "declared_format": "application/dicom"
        }),
    );
    assert_eq!(unknown["workflow"], json!("adapter_plan"));
    assert_eq!(unknown["executable"], json!(false));
    assert_eq!(unknown["execution"], json!("not_started"));
    assert_eq!(unknown["plan_id"].as_str().unwrap().len(), 64);
    assert_eq!(
        unknown["plan"]["candidates"][0]["status"],
        json!("dependency_unknown")
    );
    assert_eq!(unknown["selected_adapter"], Value::Null);

    let ready = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "scan-1",
            "source_kind": "bytes",
            "declared_format": "application/dicom",
            "available_dependencies": ["pydicom"]
        }),
    );
    assert_eq!(ready["executable"], json!(true));
    assert_eq!(
        ready["selected_adapter"]["id"],
        json!("bioprism.python.dicom")
    );
    assert_eq!(
        ready["selected_adapter"]["execution"],
        json!("python_delegated")
    );

    let refused = call(
        &mut server,
        "adapter_plan",
        json!({
            "source_id": "opaque",
            "source_kind": "bytes",
            "declared_format": "application/octet-stream"
        }),
    );
    assert_eq!(refused["executable"], json!(false));
    assert_eq!(refused["selected_adapter"], Value::Null);
    assert!(
        refused["guarantees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item == "format matching is explicit and content sniffing is refused")
    );
}

#[test]
fn adapter_execution_evidence_binds_declared_adapter_scope_and_loss_posture() {
    let mut server = server();
    let evidence = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "output_digest": "b".repeat(64),
            "execution_status": "succeeded",
            "conformance_status": "verified",
            "semantic_loss_status": "lossless",
            "item_count": 4,
            "byte_length": 128,
            "parent_digests": ["c".repeat(64)]
        }),
    );
    assert_eq!(evidence["ok"], json!(true));
    assert_eq!(
        evidence["evidence"]["adapter_id"],
        json!("bioprism.python.vcf_text")
    );
    assert_eq!(
        evidence["evidence"]["attestation_posture"],
        json!("caller_asserted")
    );
    assert_eq!(evidence["adapter"]["execution"], json!("python_delegated"));
    assert_eq!(evidence["artifact_registry"]["indexed"], json!(true));
    assert_eq!(evidence["execution"], json!("not_started"));
    assert_eq!(evidence["readiness_claimed"], json!(false));

    let queried = call(
        &mut server,
        "adapter_execution_evidence_query",
        json!({"subject_id": "adapter-subject-1", "include_artifacts": true}),
    );
    assert_eq!(queried["ok"], json!(true));
    assert_eq!(
        queried["workflow"],
        json!("adapter_execution_evidence_query")
    );
    assert_eq!(queried["rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        queried["rows"][0]["join_status"],
        json!("bound_with_missing_parents")
    );
    assert_eq!(
        queried["rows"][0]["evidence_artifact"]["evidence_digest"],
        evidence["evidence_digest"]
    );
    assert_eq!(queried["page_summary"]["page_row_count"], json!(1));
    assert_eq!(
        queried["page_summary"]["rows_with_missing_parents"],
        json!(1)
    );
    assert_eq!(queried["readiness_claimed"], json!(false));

    let inconsistent = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "execution_status": "refused",
            "conformance_status": "refused",
            "semantic_loss_status": "lossless"
        }),
    );
    assert_eq!(inconsistent["__isError"], json!(true));

    let out_of_scope = call(
        &mut server,
        "adapter_execution_evidence",
        json!({
            "group_id": "biological_domains",
            "domains": ["not-a-declared-domain"],
            "subject_id": "adapter-subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-source-1",
            "input_digest": "a".repeat(64),
            "execution_status": "unknown",
            "conformance_status": "unknown",
            "semantic_loss_status": "unknown"
        }),
    );
    assert_eq!(out_of_scope["__isError"], json!(true));
}

#[test]
fn domain_acquisition_catalogue_covers_every_declared_domain_in_two_planes() {
    let mut server = server();
    let full = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"include_adapters": true}),
    );
    assert_eq!(full["ok"], json!(true));
    assert_eq!(full["workflow"], json!("domain_acquisition_catalogue"));
    let catalogue = &full["catalogue"];
    assert_eq!(
        catalogue["total_group_count"],
        json!(CAPABILITY_GROUP_COUNT)
    );
    assert_eq!(
        catalogue["selected_group_count"],
        json!(CAPABILITY_GROUP_COUNT)
    );
    assert_eq!(catalogue["complete"], json!(true));
    assert_eq!(catalogue["truncated"], json!(false));
    assert_eq!(
        catalogue["selected_domain_count"],
        catalogue["total_domain_count"]
    );
    assert_eq!(
        catalogue["groups"].as_array().unwrap().len(),
        CAPABILITY_GROUP_COUNT
    );
    assert_eq!(
        catalogue["routes"].as_array().unwrap().len(),
        catalogue["total_domain_count"].as_u64().unwrap() as usize
    );
    assert!(catalogue["routes"].as_array().unwrap().iter().all(|route| {
        route["transport"]["status"] == "bounded_file_http"
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_normalize")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_replay_verify")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_connector_handoff")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_receipt")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_replay_verify")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_normalize")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_lineage_audit")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_execution_evidence")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "domain_evidence_provider_external_payload_evidence_query")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "adapter_execution_evidence")
            && route["transport"]["caller_managed_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool == "adapter_execution_evidence_query")
            && route["interpretation"]["status"].is_string()
            && route["limitations"].as_array().is_some()
    }));
    assert!(catalogue["routes"].as_array().unwrap().iter().any(|route| {
        route["adapters"]
            .as_array()
            .is_some_and(|adapters| !adapters.is_empty())
    }));
    assert_eq!(catalogue["digest"].as_str().unwrap().len(), 64);

    let filtered = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"max_domains": 2}),
    );
    assert_eq!(filtered["ok"], json!(true));
    assert_eq!(filtered["catalogue"]["truncated"], json!(true));
    assert_eq!(filtered["catalogue"]["routes"].as_array().unwrap().len(), 2);

    let refused = call(
        &mut server,
        "domain_acquisition_catalogue",
        json!({"max_domains": 0}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(refused["error"].as_str().unwrap().contains("max_domains"));
}

#[test]
fn capability_route_batches_ranked_and_explicit_needs_without_execution() {
    let mut server = server();
    let result = call(
        &mut server,
        "capability_route",
        json!({
            "goal": "compose a cross-domain evidence route",
            "needs": [
                {"id": "oncology", "query": "oncology"},
                {"id": "release", "tool": "bundle_verify"}
            ],
            "max_candidates_per_need": 2,
            "max_tools": 4,
            "include_tools": true
        }),
    );
    assert_eq!(result["workflow"], json!("capability_route"));
    assert_eq!(result["execution"], json!("not_started"));
    assert_eq!(result["unresolved_needs"], json!([]));
    assert_eq!(result["needs"][0]["resolution"], json!("ranked_candidates"));
    assert_eq!(result["needs"][1]["resolution"], json!("explicit"));
    assert!(
        result["recommended_tools"]
            .as_array()
            .unwrap()
            .contains(&json!("bundle_verify"))
    );
    assert_eq!(result["recommended_tools"].as_array().unwrap().len(), 4);
    assert_eq!(result["schema_attachment"]["requested"], json!(true));
    assert_eq!(result["schema_attachment"]["returned"], json!(4));
    assert_eq!(result["route_coverage"]["needs_total"], json!(2));
    assert_eq!(result["route_coverage"]["needs_resolved"], json!(2));
    assert_eq!(result["route_coverage"]["needs_unresolved"], json!(0));
    assert_eq!(
        result["route_coverage"]["candidate_group_evidence_count"],
        result["route_coverage"]["candidate_group_count"]
    );
    assert_eq!(result["evidence"]["readiness_claimed"], json!(false));
    assert_eq!(
        result["evidence"]["groups_with_artifact_evidence"],
        json!(0)
    );
    assert_eq!(
        result["evidence"]["groups_with_workflow_reconciliation"],
        json!(0)
    );
    assert_eq!(result["evidence_digest"].as_str().unwrap().len(), 64);
    assert_eq!(
        result["evidence_digest"],
        result["evidence"]["evidence_digest"]
    );
    assert!(
        !result["needs"][0]["candidate_group_evidence"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        result["needs"][0]["candidate_group_evidence"][0]["artifact_evidence"]["state"],
        json!("missing")
    );
    assert!(
        result["route_coverage"]["candidate_domain_count"]
            .as_u64()
            .unwrap()
            >= 2
    );
    assert!(
        result["needs"][0]["candidate_domains"]
            .as_array()
            .unwrap()
            .iter()
            .any(|domain| domain == "oncology")
    );
    assert_eq!(result["route_id"].as_str().unwrap().len(), 64);

    let refused = call(
        &mut server,
        "capability_route",
        json!({"goal": "bad", "needs": [{"id": "nested", "include_tools": true}]}),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert_eq!(refused["ok"], json!(false));
}

#[test]
fn capability_route_review_builds_non_executing_handoff_and_reports_bad_selection() {
    let mut server = server();
    let route = call(
        &mut server,
        "capability_route",
        json!({
            "goal": "compose a reviewed handoff",
            "needs": [
                {"id": "oncology", "tool": "workspace_capabilities"},
                {"id": "release", "tool": "weave_protocol_catalog"}
            ],
            "max_candidates_per_need": 2,
            "max_tools": 4
        }),
    );
    let oncology_tool = route["needs"][0]["candidate_tools"][0]
        .as_str()
        .unwrap()
        .to_string();
    let selections = json!([
        {
            "need_id": "oncology",
            "tool": oncology_tool,
            "domain": "oncology",
            "capability": "evidence",
            "objective": "review oncology evidence",
            "arguments": {}
        },
        {
            "need_id": "release",
            "tool": "weave_protocol_catalog",
            "domain": "release",
            "capability": "verification",
            "objective": "verify the release bundle",
            "arguments": {},
            "depends_on": ["oncology"]
        }
    ]);
    let review = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "selections": selections.clone()
        }),
    );
    assert_eq!(review["workflow"], json!("capability_route_review"));
    assert_eq!(review["review_id"].as_str().unwrap().len(), 64);
    assert_eq!(review["review_status"], json!("ready"));
    assert_eq!(
        review["handoff_status"],
        json!("mission_preflight_required")
    );
    assert_eq!(review["execution"], json!("not_started"));
    assert_eq!(review["evidence_binding"]["present"], json!(true));
    assert_eq!(
        review["evidence_binding"]["evidence_digest"],
        route["evidence_digest"]
    );
    assert_eq!(
        review["mission_draft"]["route_evidence_digest"],
        route["evidence_digest"]
    );
    assert_eq!(
        review["mission_draft"]["route_evidence_scope"],
        route["evidence_scope"]
    );
    let mut tampered_route = route.clone();
    tampered_route["evidence_scope"] = json!("tampered_scope");
    let refused_tampering = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": tampered_route,
            "selections": selections.clone()
        }),
    );
    assert_eq!(refused_tampering["__isError"], json!(true));
    assert!(
        refused_tampering["error"]
            .as_str()
            .unwrap()
            .contains("route evidence summary does not match")
    );

    let mut legacy_route = route.clone();
    legacy_route.as_object_mut().unwrap().remove("evidence");
    legacy_route
        .as_object_mut()
        .unwrap()
        .remove("evidence_digest");
    legacy_route
        .as_object_mut()
        .unwrap()
        .remove("evidence_scope");
    let legacy_review = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": legacy_route,
            "selections": selections
        }),
    );
    assert_eq!(legacy_review["review_status"], json!("ready"));
    assert_eq!(
        legacy_review["handoff_status"],
        json!("mission_preflight_required")
    );
    assert_eq!(legacy_review["evidence_binding"]["present"], json!(false));
    assert_eq!(
        legacy_review["evidence_binding"]["posture"],
        json!("not_supplied")
    );
    assert_eq!(
        review["dependency_waves"],
        json!([["oncology"], ["release"]])
    );
    assert_eq!(
        review["mission_draft"]["steps"].as_array().unwrap().len(),
        2
    );

    let mission = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "route-review-mission",
            "goal": review["goal"].clone(),
            "steps": review["mission_draft"]["steps"].clone(),
            "route_review": review.clone()
        }),
    );
    assert_eq!(mission["__isError"], json!(false));
    assert_eq!(mission["execution"], json!("planned"));
    assert_eq!(
        mission["plan"]["route_review_provenance"]["present"],
        json!(true)
    );
    assert_eq!(
        mission["plan"]["route_review_provenance"]["evidence_present"],
        json!(true)
    );
    assert_eq!(
        mission["plan"]["route_review_provenance"]["readiness_claimed"],
        json!(false)
    );

    let mut tampered_review = review.clone();
    tampered_review["mission_draft"]["steps"][0]["objective"] = json!("tampered after review");
    let refused_handoff = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "route-review-tampered",
            "goal": review["goal"].clone(),
            "steps": review["mission_draft"]["steps"].clone(),
            "route_review": tampered_review
        }),
    );
    assert_eq!(refused_handoff["__isError"], json!(true));
    assert!(
        refused_handoff["error"]
            .as_str()
            .unwrap()
            .contains("route_review")
    );

    let blocked = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": review["route_coverage"].clone(),
            "selections": []
        }),
    );
    assert_eq!(blocked["__isError"], json!(true));

    let route = call(
        &mut server,
        "capability_route",
        json!({"goal": "review one", "needs": [{"id": "release", "tool": "bundle_verify"}]}),
    );
    let blocked = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "selections": [{
                "need_id": "release",
                "tool": "not_a_candidate",
                "domain": "release",
                "capability": "verification",
                "objective": "bad selection",
                "arguments": {}
            }]
        }),
    );
    assert_eq!(blocked["review_status"], json!("blocked"));
    assert!(
        blocked["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "candidate_mismatch")
    );

    let route = call(
        &mut server,
        "capability_route",
        json!({"goal": "validate schemas", "needs": [{"id": "catalog", "tool": "workspace_capabilities"}]}),
    );
    let schema_review = call(
        &mut server,
        "capability_route_review",
        json!({
            "route": route,
            "validate_schemas": true,
            "selections": [{
                "need_id": "catalog",
                "tool": "workspace_capabilities",
                "domain": "workspace",
                "capability": "discovery",
                "objective": "validate the catalogue schema",
                "arguments": {}
            }]
        }),
    );
    assert_eq!(schema_review["review_status"], json!("ready"));
    assert_eq!(schema_review["schema_review"]["requested"], json!(true));
    assert_eq!(schema_review["schema_review"]["valid"], json!(true));
    assert_eq!(schema_review["schema_review"]["checked"], json!(1));
}

#[test]
fn capability_route_plan_verifier_replays_inputs_and_preserves_no_dispatch() {
    let mut server = server();
    let route = call(
        &mut server,
        "capability_route",
        json!({
            "goal": "verify a reviewed plan",
            "needs": [{"id": "audit", "tool": "capability_audit"}],
            "max_candidates_per_need": 2,
            "max_tools": 2
        }),
    );
    let selections = json!([{
        "need_id": "audit",
        "tool": "capability_audit",
        "domain": "developer_platform",
        "capability": "capability_audit",
        "objective": "audit the capability catalogue",
        "arguments": {}
    }]);
    let plan = call(
        &mut server,
        "capability_route_plan",
        json!({
            "mission_id": "route-plan-verifier",
            "route": route,
            "selections": selections
        }),
    );
    assert_eq!(plan["__isError"], json!(false));
    assert_eq!(plan["plan_status"], json!("ready_for_caller_inspection"));
    assert_eq!(plan["route_input_digest"].as_str().unwrap().len(), 64);
    assert_eq!(plan["selection_digest"].as_str().unwrap().len(), 64);
    assert_eq!(plan["selection_count"], json!(1));
    let original_plan = plan.clone();

    let verified = call(
        &mut server,
        "capability_route_plan_verify",
        json!({
            "plan": original_plan.clone(),
            "route": route,
            "selections": selections
        }),
    );
    assert_eq!(verified["__isError"], json!(false));
    assert_eq!(verified["workflow"], json!("capability_route_plan_verify"));
    assert_eq!(verified["valid"], json!(true));
    assert_eq!(verified["verification_status"], json!("verified"));
    assert_eq!(verified["route_replay"]["status"], json!("matched"));
    assert_eq!(verified["mission_preflight"]["status"], json!("matched"));
    assert_eq!(verified["dispatch"], json!("not_started"));

    let mut tampered_plan = original_plan;
    tampered_plan["plan_digest"] = json!("f");
    let invalid = call(
        &mut server,
        "capability_route_plan_verify",
        json!({"plan": tampered_plan}),
    );
    assert_eq!(invalid["__isError"], json!(false));
    assert_eq!(invalid["valid"], json!(false));
    assert_eq!(invalid["verification_status"], json!("mismatch"));
}

#[test]
fn agent_mission_preserves_refusal_and_blocks_dependents() {
    let mut server = server();
    let result = call(
        &mut server,
        "agent_mission",
        json!({
            "mission_id": "mission-refusal-1",
            "goal": "prove refusal propagation",
            "steps": [
                {"id": "bad", "domain": "test", "capability": "refusal", "objective": "invoke an unknown tool", "tool": "not_a_real_tool"},
                {"id": "dependent", "domain": "test", "capability": "blocked", "objective": "must not run", "tool": "workspace_capabilities", "depends_on": ["bad"]}
            ],
            "policy": {"execute": true, "allowed_tools": ["not_a_real_tool", "workspace_capabilities"]}
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["refused"], json!(1));
    assert_eq!(result["blocked"], json!(1));
    assert_eq!(result["mission_status"], json!("failed"));
    assert_eq!(result["results"][0]["status"], json!("refused"));
    assert_eq!(result["results"][1]["status"], json!("blocked"));
    assert!(
        result["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("unknown tool")
    );
}
