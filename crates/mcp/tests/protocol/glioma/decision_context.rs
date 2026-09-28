//! MCP contract tests for the decision context area.

use super::*;

#[test]
fn glioma_decision_branch_plan_ranks_scenario_robust_portfolios() {
    let mut server = server();
    let hash = "0".repeat(64);
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {"objective":"rank invasion mechanisms","required_modalities":["genomics"],"required_model_systems":["organoid"],"min_support_milli":700,"min_sources_per_claim":1,"max_claims":8},
            "records": [{"evidence_id":"branch-e1","source_artifact":{"artifact_id":"branch-a1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}]
        }),
    );
    let context_response = call(
        &mut server,
        "glioma_decision_context",
        json!({"request":{"objective":"rank invasion mechanisms","max_actions":8,"default_cost_units":4},"knowledge":knowledge["knowledge"].clone()}),
    );
    let context_candidate = context_response["context"]["actions"][0]["candidate"].clone();
    let context_id = context_candidate["action_id"].as_str().unwrap().to_string();
    let branch = call(
        &mut server,
        "glioma_decision_branch_plan",
        json!({
            "request": {
                "objective":"rank invasion mechanisms",
                "candidates":[context_candidate, {"action_id":"branch-independent","stage_kind":"mechanism_exploration","modality":"genomics","model_system":"organoid","depends_on":[],"cost_units":3,"information_gain_milli":800,"frontier_novelty_milli":800,"workflow_leverage_milli":800,"cross_stage_unlock_milli":800,"reproducibility_safety_milli":900,"federation_value_milli":500,"feasibility_milli":900,"autonomy_tier":"a1","effects":["read_local_data","execute_local_computation"]}],
                "completed_action_order":[],
                "scenarios":[
                    {"scenario_id":"invasion_high","probability_milli":600,"outcomes":[{"action_id":context_id,"value_milli":700,"uncertainty_milli":100,"failure_probability_milli":50},{"action_id":"branch-independent","value_milli":600,"uncertainty_milli":150,"failure_probability_milli":50}]},
                    {"scenario_id":"invasion_low","probability_milli":400,"outcomes":[{"action_id":context_id,"value_milli":300,"uncertainty_milli":200,"failure_probability_milli":100},{"action_id":"branch-independent","value_milli":450,"uncertainty_milli":150,"failure_probability_milli":100}]}
                ],
                "budget_units":8,"max_actions_per_branch":2,"max_branches":8,"beam_width":16,"minimum_robustness_milli":0,"uncertainty_penalty_milli":300,"failure_penalty_milli":300,
                "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            },
            "context":context_response["context"].clone()
        }),
    );
    assert_eq!(branch["dispatch"], json!("not_started"));
    assert!(branch["plan"]["selected_branch_id"].is_string());
    assert!(
        !branch["plan"]["frontier_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        branch["plan"]["portfolios"]
            .as_array()
            .unwrap()
            .iter()
            .all(|portfolio| portfolio["scenario_scores"].as_array().unwrap().len() == 2)
    );
}

#[test]
fn glioma_decision_branch_campaign_executes_and_scores_local_evidence() {
    let mut server = server();
    let hash = "0".repeat(64);
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {"objective":"rank invasion mechanisms","required_modalities":["genomics"],"required_model_systems":["organoid"],"min_support_milli":700,"min_sources_per_claim":1,"max_claims":8},
            "records": [{"evidence_id":"branch-exec-e1","source_artifact":{"artifact_id":"branch-exec-a1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}]
        }),
    );
    let context = call(
        &mut server,
        "glioma_decision_context",
        json!({"request":{"objective":"rank invasion mechanisms","max_actions":8,"default_cost_units":1},"knowledge":knowledge["knowledge"].clone()}),
    );
    let candidate = context["context"]["actions"][0]["candidate"].clone();
    let action_id = candidate["action_id"].as_str().unwrap().to_string();
    let branch = call(
        &mut server,
        "glioma_decision_branch_plan",
        json!({
            "request": {
                "objective":"rank invasion mechanisms",
                "candidates":[candidate],
                "completed_action_order":[],
                "scenarios":[
                    {"scenario_id":"high","probability_milli":600,"outcomes":[{"action_id":action_id,"value_milli":900,"uncertainty_milli":100,"failure_probability_milli":50}]},
                    {"scenario_id":"low","probability_milli":400,"outcomes":[{"action_id":action_id,"value_milli":700,"uncertainty_milli":200,"failure_probability_milli":100}]}
                ],
                "budget_units":2,"max_actions_per_branch":2,"max_branches":4,"beam_width":8,"minimum_robustness_milli":0,"uncertainty_penalty_milli":100,"failure_penalty_milli":100,
                "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
            },
            "context":context["context"].clone()
        }),
    );
    let campaign = call(
        &mut server,
        "glioma_decision_branch_campaign_execute",
        json!({"request":{"objective":"rank invasion mechanisms","knowledge":knowledge["knowledge"].clone(),"context":context["context"].clone(),"branch_plan":branch["plan"].clone(),"completed_action_order":[],"budget_units":2,"max_branches":2,"max_retries":1,"stop_on_completed":true}}),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("completed"));
    assert!(campaign["campaign"]["completed_branch_id"].is_string());
    assert_eq!(campaign["campaign"]["records"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_adaptive_decision_branch_campaign_recompiles_after_evidence() {
    let mut server = server();
    let hash = "0".repeat(64);
    let knowledge_request = json!({
        "objective":"rank invasion mechanisms",
        "required_modalities":["genomics"],
        "required_model_systems":["organoid"],
        "min_support_milli":700,
        "min_sources_per_claim":1,
        "max_claims":8
    });
    let records = json!([{
        "evidence_id":"adaptive-branch-e1",
        "source_artifact":{"artifact_id":"adaptive-branch-a1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
        "source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1
    }]);
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({"request":knowledge_request,"records":records}),
    );
    let context_request = json!({
        "objective":"rank invasion mechanisms",
        "max_actions":8,
        "default_cost_units":1
    });
    let context = call(
        &mut server,
        "glioma_decision_context",
        json!({"request":context_request,"knowledge":knowledge["knowledge"].clone()}),
    );
    let candidate = context["context"]["actions"][0]["candidate"].clone();
    let action_id = candidate["action_id"].as_str().unwrap().to_string();
    let campaign = call(
        &mut server,
        "glioma_adaptive_decision_branch_campaign_execute",
        json!({
            "request": {
                "knowledge": knowledge_request,
                "context": context_request,
                "branches": {
                    "objective":"rank invasion mechanisms",
                    "candidates":[candidate],
                    "completed_action_order":[],
                    "scenarios":[
                        {"scenario_id":"high","probability_milli":600,"outcomes":[{"action_id":action_id,"value_milli":900,"uncertainty_milli":100,"failure_probability_milli":50}]},
                        {"scenario_id":"low","probability_milli":400,"outcomes":[{"action_id":action_id,"value_milli":700,"uncertainty_milli":200,"failure_probability_milli":100}]}
                    ],
                    "budget_units":2,"max_actions_per_branch":2,"max_branches":4,"beam_width":8,"minimum_robustness_milli":0,"uncertainty_penalty_milli":100,"failure_penalty_milli":100,
                    "selection_weights":{"information_gain":25,"frontier_novelty":20,"workflow_leverage":15,"cross_stage_unlock":15,"reproducibility_safety":10,"federation_value":10,"feasibility":5}
                },
                "records":records,
                "budget_units":2,
                "max_rounds":3,
                "max_retries":1,
                "stop_on_completed":true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
    assert!(
        !campaign["campaign"]["records"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(campaign["campaign"]["final_knowledge"].is_object());
    assert!(campaign["campaign"]["final_context"].is_object());
}

#[test]
fn glioma_decision_context_campaign_dispatches_claim_scoped_action_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let campaign = call(
        &mut server,
        "glioma_decision_context_campaign_execute",
        json!({
            "request": {
                "knowledge": {
                    "objective": "rank glioma invasion actions",
                    "required_modalities": ["genomics"],
                    "required_model_systems": ["organoid"],
                    "min_support_milli": 700,
                    "min_sources_per_claim": 1,
                    "max_claims": 8
                },
                "context": {
                    "objective": "rank glioma invasion actions",
                    "max_actions": 8,
                    "default_cost_units": 2
                },
                "action_plan": {
                    "objective": "rank glioma invasion actions",
                    "completed_action_order": [],
                    "selection": {
                        "budget_units": 2,
                        "max_actions": 1,
                        "approval_granted": true,
                        "allow_instrument_execution": false,
                        "allow_federation": false,
                        "weights": {
                            "information_gain": 20,
                            "frontier_novelty": 15,
                            "workflow_leverage": 15,
                            "cross_stage_unlock": 15,
                            "reproducibility_safety": 15,
                            "federation_value": 10,
                            "feasibility": 10
                        }
                    }
                },
                "records": [{
                    "evidence_id": "decision-campaign-seed",
                    "source_artifact": {"artifact_id":"decision-campaign-artifact","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
                    "source_kind": "dataset",
                    "claim": "EGFR signaling increases invasion",
                    "scope": "preclinical glioma",
                    "modality": "genomics",
                    "model_system": "organoid",
                    "state": "supported",
                    "relevance_milli": 900,
                    "quality_milli": 900,
                    "reproducibility_milli": 900,
                    "release_epoch": 1
                }],
                "budget_units": 2,
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("qualified"));
    assert_eq!(
        campaign["campaign"]["final_action_plan"]["disposition"],
        json!("no_runnable_actions")
    );
    assert_eq!(campaign["campaign"]["rounds"].as_array().unwrap().len(), 1);
}

#[test]
fn glioma_decision_operating_cycle_runs_full_p04_stack_in_sandbox() {
    let mut server = server();
    let hash = "0".repeat(64);
    let objective = "resolve glioma invasion mechanism";
    let record = json!({
        "evidence_id": "p04-cycle-seed",
        "source_artifact": {"artifact_id":"p04-cycle-artifact","content_hash":hash,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},
        "source_kind": "literature",
        "claim": "EGFR signaling increases invasion",
        "scope": "organoid invasion",
        "modality": "genomics",
        "model_system": "organoid",
        "state": "supported",
        "relevance_milli": 900,
        "quality_milli": 900,
        "reproducibility_milli": 900,
        "release_epoch": 1
    });
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {
                "objective": objective,
                "required_modalities": ["genomics"],
                "required_model_systems": ["organoid"],
                "min_support_milli": 100,
                "min_sources_per_claim": 1,
                "max_claims": 8
            },
            "records": [record.clone()]
        }),
    );
    let composition = call(
        &mut server,
        "glioma_knowledge_compose",
        json!({
            "request": {
                "objective": objective,
                "min_path_length": 2,
                "max_paths": 8,
                "min_strength_milli": 0,
                "max_contradiction_milli": 1000,
                "require_supported_root": false
            },
            "knowledge": knowledge["knowledge"].clone(),
            "relations": []
        }),
    );
    let cycle = call(
        &mut server,
        "glioma_decision_operating_cycle",
        json!({
            "request": {
                "knowledge": {
                    "objective": objective,
                    "required_modalities": ["genomics"],
                    "required_model_systems": ["organoid"],
                    "min_support_milli": 100,
                    "min_sources_per_claim": 1,
                    "max_claims": 8
                },
                "context": {"objective": objective, "max_actions": 8, "default_cost_units": 2},
                "graph": {"objective": objective, "max_nodes": 8, "max_waves": 8, "budget_units": 8, "require_qualified_composition": false},
                "branches": {
                    "objective": objective,
                    "candidates": [],
                    "completed_action_order": [],
                    "scenarios": [],
                    "budget_units": 8,
                    "max_actions_per_branch": 2,
                    "max_branches": 4,
                    "beam_width": 8,
                    "minimum_robustness_milli": -1000000,
                    "uncertainty_penalty_milli": 100,
                    "failure_penalty_milli": 100,
                    "selection_weights": {
                        "information_gain": 25,
                        "frontier_novelty": 20,
                        "workflow_leverage": 15,
                        "cross_stage_unlock": 15,
                        "reproducibility_safety": 10,
                        "federation_value": 10,
                        "feasibility": 5
                    }
                },
                "composition": composition["composition"].clone(),
                "records": [record],
                "action_plan": {
                    "objective": objective,
                    "completed_action_order": [],
                    "selection": {
                        "budget_units": 8,
                        "max_actions": 2,
                        "approval_granted": true,
                        "allow_instrument_execution": false,
                        "allow_federation": false,
                        "weights": {
                            "information_gain": 25,
                            "frontier_novelty": 20,
                            "workflow_leverage": 15,
                            "cross_stage_unlock": 15,
                            "reproducibility_safety": 10,
                            "federation_value": 10,
                            "feasibility": 5
                        }
                    }
                },
                "budget_units": 8,
                "max_rounds": 2,
                "max_retries": 1,
                "stop_on_qualified": true
            }
        }),
    );
    assert_eq!(cycle["dispatch"], json!("dry_run"));
    assert_eq!(cycle["simulation_only"], json!(true));
    assert_eq!(cycle["cycle"]["phase_order"].as_array().unwrap().len(), 5);
    assert_eq!(
        cycle["cycle"]["action_graph"]["context_digest"],
        cycle["cycle"]["context"]["digest"]
    );
}

#[test]
fn glioma_decision_context_query_is_reachable_through_mcp() {
    let mut server = server();
    let zero_hash = "0".repeat(64);
    let capability_body = json!({
        "capability_id": "cap-mcp",
        "allowed_scope_order": ["claims"],
        "allow_omissions": true,
        "allow_uncertainty": true,
        "max_result_budget": 4,
        "expires_at_tick": 100,
        "revoked": false
    });
    let capability_digest = bioprism_ids::ContentHash::of_value(&capability_body)
        .unwrap()
        .to_string();
    let record = |record_id: &str, field: &str, state: &str, reason: Option<&str>| {
        json!({
            "context_id": "ctx-mcp",
            "schema_version": "decision-context/1",
            "context_digest": zero_hash.clone(),
            "scope": "claims/glioma",
            "field": field,
            "record_id": record_id,
            "value_digest": zero_hash.clone(),
            "provenance_digest": zero_hash.clone(),
            "state": state,
            "reason": reason,
            "updated_tick": 1
        })
    };
    let output = call(
        &mut server,
        "glioma_decision_context_query",
        json!({
            "request": {
                "context_id": "ctx-mcp",
                "schema_version": "decision-context/1",
                "context_digest": zero_hash,
                "scope_prefix": "claims/glioma",
                "field_order": ["claim", "omission"],
                "after": null,
                "page_size": 4,
                "result_budget": 4,
                "capability": {
                    "capability_id": "cap-mcp",
                    "allowed_scope_order": ["claims"],
                    "allow_omissions": true,
                    "allow_uncertainty": true,
                    "max_result_budget": 4,
                    "expires_at_tick": 100,
                    "revoked": false,
                    "capability_digest": capability_digest
                },
                "current_tick": 5,
                "records": [
                    record("claim-a", "claim", "measured", None),
                    record("omission-a", "omission", "omitted", Some("protected assay unavailable"))
                ]
            }
        }),
    );
    assert_eq!(output["result"]["feature_id"], json!("GAF-GLIOMA-P04-F21"));
    assert_eq!(output["result"]["completeness"], json!("complete"));
    assert_eq!(output["result"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_decision_context_update_is_reachable_through_mcp() {
    let mut server = server();
    let hash = |label: &str| {
        bioprism_ids::ContentHash::of_value(&json!({"label": label}))
            .unwrap()
            .to_string()
    };
    let candidate = json!({
        "action_id": "action-a",
        "stage_kind": "experiment_design",
        "modality": "organoid_assay",
        "model_system": "organoid",
        "depends_on": [],
        "cost_units": 1,
        "information_gain_milli": 800,
        "frontier_novelty_milli": 700,
        "workflow_leverage_milli": 700,
        "cross_stage_unlock_milli": 600,
        "reproducibility_safety_milli": 800,
        "federation_value_milli": 300,
        "feasibility_milli": 900,
        "autonomy_tier": "a0",
        "effects": ["execute_local_computation"]
    });
    let action = json!({
        "action_id": "action-a",
        "claim_id": "claim-a",
        "kind": "validate_mechanism",
        "rationale": "validate the typed glioma mechanism",
        "target_modality": "organoid_assay",
        "target_model_system": "organoid",
        "priority_milli": 900,
        "candidate": candidate
    });
    let context_body = json!({
        "feature_id": "GAF-GLIOMA-P04-F01",
        "output_schema": "GliomaDecisionContext1@2",
        "objective": "update glioma context",
        "claim_order": ["claim-a"],
        "actions": [action.clone()],
        "action_order": ["action-a"],
        "deferred_action_order": [],
        "omission_order": [],
        "negative_evidence_order": [],
        "uncertainty_order": [],
        "disposition": "qualified"
    });
    let context_digest = bioprism_ids::ContentHash::of_value(&context_body)
        .unwrap()
        .to_string();
    let event_body = json!({
        "event_id": "negative-action-a",
        "sequence": 1,
        "observed_tick": 2,
        "context_anchor_digest": context_digest,
        "kind": "action_negative",
        "subject_order": ["action-a"],
        "payload_digest": hash("negative-payload"),
        "provenance_digest": hash("negative-provenance"),
        "reason": "organoid assay did not reproduce the predicted invasion effect"
    });
    let event_digest = bioprism_ids::ContentHash::of_value(&event_body)
        .unwrap()
        .to_string();
    let output = call(
        &mut server,
        "glioma_decision_context_update",
        json!({
            "request": {
                "base_context": {
                    "feature_id": "GAF-GLIOMA-P04-F01",
                    "output_schema": "GliomaDecisionContext1@2",
                    "objective": "update glioma context",
                    "claim_order": ["claim-a"],
                    "actions": [action],
                    "action_order": ["action-a"],
                    "deferred_action_order": [],
                    "omission_order": [],
                    "negative_evidence_order": [],
                    "uncertainty_order": [],
                    "disposition": "qualified",
                    "digest": context_digest
                },
                "event_order": [{
                    "event_id": "negative-action-a",
                    "sequence": 1,
                    "observed_tick": 2,
                    "context_anchor_digest": context_digest,
                    "kind": "action_negative",
                    "subject_order": ["action-a"],
                    "payload_digest": hash("negative-payload"),
                    "provenance_digest": hash("negative-provenance"),
                    "reason": "organoid assay did not reproduce the predicted invasion effect",
                    "event_digest": event_digest
                }],
                "last_applied_sequence": 0,
                "max_events": 4,
                "current_tick": 5
            }
        }),
    );
    assert_eq!(output["result"]["feature_id"], json!("GAF-GLIOMA-P04-F23"));
    assert_eq!(output["result"]["disposition"], json!("applied"));
    assert_eq!(
        output["result"]["invalidated_action_order"],
        json!(["action-a"])
    );
    assert_eq!(output["result"]["final_context"]["action_order"], json!([]));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_decision_context_snapshot_store_is_reachable_through_mcp() {
    let mut server = server();
    let hash = "0".repeat(64);
    let knowledge = call(
        &mut server,
        "glioma_knowledge_compile",
        json!({
            "request": {"objective":"snapshot a glioma invasion context","required_modalities":["genomics"],"required_model_systems":["organoid"],"min_support_milli":700,"min_sources_per_claim":1,"max_claims":4},
            "records": [{"evidence_id":"snapshot-e1","source_artifact":{"artifact_id":"snapshot-a1","content_hash":hash,"content_type":"application/vnd.aurora.glioma-evidence+json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false},"source_kind":"dataset","claim":"EGFR signaling increases invasion","scope":"preclinical glioma","modality":"genomics","model_system":"organoid","state":"supported","relevance_milli":900,"quality_milli":900,"reproducibility_milli":900,"release_epoch":1}]
        }),
    );
    let context = call(
        &mut server,
        "glioma_decision_context",
        json!({"request":{"objective":"snapshot a glioma invasion context","max_actions":4,"default_cost_units":1},"knowledge":knowledge["knowledge"].clone()}),
    );
    let output = call(
        &mut server,
        "glioma_decision_context_snapshot_store",
        json!({"request":{"study_id":"snapshot-study","max_retained_snapshots":2,"snapshots":[{"snapshot_id":"snapshot-1","study_id":"snapshot-study","epoch":1,"parent_snapshot_id":null,"context":context["context"].clone(),"event_order":["event-1"],"pinned":true,"referenced":false}],"restore_snapshot_id":"snapshot-1"}}),
    );
    assert_eq!(output["index"]["feature_id"], json!("GAF-GLIOMA-P04-F29"));
    assert_eq!(output["index"]["recovery_snapshot_id"], json!("snapshot-1"));
    assert_eq!(output["dispatch"], json!("not_started"));
}
