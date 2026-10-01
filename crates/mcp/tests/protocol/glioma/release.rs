//! MCP contract tests for the release area.

use super::*;

#[test]
fn glioma_replay_campaign_executes_dependency_aware_release_check_in_sandbox() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let campaign = call(
        &mut server,
        "glioma_replay_campaign_execute",
        json!({
            "request": {
                "release": {
                    "research_id": "mcp-replay",
                    "study_id": "mcp-study",
                    "objective": "replay a glioma mechanism result",
                    "plan_digest": hash,
                    "execution_digest": hash,
                    "replay_identity": hash,
                    "program_order": ["p05-mechanism"],
                    "artifacts": [{"artifact_id": "artifact-main", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}],
                    "negative_evidence": ["null-result-preserved"],
                    "limitations": ["single-model-system"],
                    "raw_data_local": true,
                    "aggregate_only": true
                },
                "tasks": [{
                    "task_id": "mechanism-replay",
                    "program_id": "p05-mechanism",
                    "artifact_id": "artifact-main",
                    "expected_content_hash": hash,
                    "cost_units": 1,
                    "required": true,
                    "deterministic": true,
                    "depends_on": []
                }],
                "budget_units": 1,
                "max_rounds": 2,
                "max_retries": 1,
                "min_coverage_milli": 1000,
                "require_exact_hash": true
            }
        }),
    );
    assert_eq!(campaign["dispatch"], json!("dry_run"));
    assert_eq!(campaign["simulation_only"], json!(true));
    assert_eq!(campaign["campaign"]["disposition"], json!("reproducible"));
    assert_eq!(campaign["campaign"]["exact_match"], json!(true));
}

#[test]
fn glioma_research_object_release_gate_requires_replay_and_independent_review() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let campaign = call(
        &mut server,
        "glioma_replay_campaign_execute",
        json!({
            "request": {
                "release": {
                    "research_id": "gate-mcp-research",
                    "study_id": "gate-mcp-study",
                    "objective": "release a reproducible preclinical glioma result",
                    "plan_digest": hash,
                    "execution_digest": hash,
                    "replay_identity": hash,
                    "program_order": ["p05-mechanism"],
                    "artifacts": [{"artifact_id": "gate-artifact", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}],
                    "negative_evidence": ["null-result-preserved"],
                    "limitations": ["single-model-system"],
                    "raw_data_local": true,
                    "aggregate_only": true
                },
                "tasks": [{"task_id": "gate-task", "program_id": "p05-mechanism", "artifact_id": "gate-artifact", "expected_content_hash": hash, "cost_units": 1, "required": true, "deterministic": true, "depends_on": []}],
                "budget_units": 1,
                "max_rounds": 2,
                "max_retries": 1,
                "min_coverage_milli": 1000,
                "require_exact_hash": true
            }
        }),
    );
    let gate = call(
        &mut server,
        "glioma_research_object_release_gate",
        json!({
            "request": {
                "required_coverage_milli": 1000,
                "require_exact_hash": true,
                "require_reproducible": true,
                "require_accountable_review": true,
                "min_independent_approvals": 1,
                "max_uncertainty_items": 8,
                "reviews": [{"reviewer_id": "reviewer-a", "role": "independent-reproducibility-reviewer", "decision": "approve", "evidence_digest": hash, "independent": true}]
            },
            "campaign": campaign["campaign"].clone()
        }),
    );
    assert_eq!(gate["dispatch"], json!("not_started"));
    assert_eq!(gate["simulation_only"], json!(true));
    assert_eq!(gate["gate"]["status"], json!("publishable"));
    assert!(
        gate["gate"]["blocking_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn glioma_release_operating_cycle_composes_replay_and_signing_handoff() {
    let mut server = server();
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let cycle = call(
        &mut server,
        "glioma_release_operating_cycle",
        json!({
            "request": {
                "replay": {
                    "release": {
                        "research_id": "cycle-mcp-research",
                        "study_id": "cycle-mcp-study",
                        "objective": "release a reproducible preclinical glioma result",
                        "plan_digest": hash,
                        "execution_digest": hash,
                        "replay_identity": hash,
                        "program_order": ["p09-computation"],
                        "artifacts": [{"artifact_id": "cycle-artifact", "content_hash": hash, "content_type": "application/json", "local_only": true, "contains_human_data": false, "contains_direct_identifiers": false}],
                        "negative_evidence": ["null-result-preserved"],
                        "limitations": ["synthetic-fixture"],
                        "raw_data_local": true,
                        "aggregate_only": true
                    },
                    "tasks": [{"task_id": "cycle-task", "program_id": "p09-computation", "artifact_id": "cycle-artifact", "expected_content_hash": hash, "cost_units": 1, "required": true, "deterministic": true, "depends_on": []}],
                    "budget_units": 1,
                    "max_rounds": 2,
                    "max_retries": 1,
                    "min_coverage_milli": 1000,
                    "require_exact_hash": true
                },
                "gate": {
                    "required_coverage_milli": 1000,
                    "require_exact_hash": true,
                    "require_reproducible": true,
                    "require_accountable_review": true,
                    "min_independent_approvals": 1,
                    "max_uncertainty_items": 8,
                    "reviews": [{"reviewer_id": "reviewer-cycle", "role": "independent-reproducibility-reviewer", "decision": "approve", "evidence_digest": hash, "independent": true}]
                },
                "execution_mode": "local_simulation"
            }
        }),
    );
    assert_eq!(cycle["dispatch"], json!("dry_run"));
    assert_eq!(cycle["simulation_only"], json!(true));
    assert_eq!(cycle["cycle"]["disposition"], json!("publishable"));
    assert!(
        cycle["cycle"]["next_operator_action"]
            .as_str()
            .unwrap()
            .contains("accountable signature")
    );
}

#[test]
fn glioma_protocol_transport_gate_releases_supported_target_model_endpoint() {
    let mut server = server();
    let protocol = json!({
        "objective": "transport invasion evidence",
        "model_system": "organoid",
        "tasks": [{"task_id":"assay","label":"run invasion assay","resource_kind":"imaging","resource_units":1,"duration_ticks":2,"depends_on":[],"model_system":"organoid","output_schema":"Assay1@1","risk_milli":100,"requires_instrument":false}],
        "resources": [{"resource_id":"imaging","kind":"imaging","capacity_units":1}],
        "max_ticks": 10,
        "max_risk_milli": 500,
        "allow_instrument_execution": false,
        "approval_reference": null,
        "randomization_seed": ContentHash::of_bytes(b"transport-gate-mcp")
    });
    let execution = call(
        &mut server,
        "glioma_protocol_execute",
        json!({"request":{"protocol":protocol.clone(),"max_retries":0,"require_artifacts":true}}),
    );
    assert_eq!(execution["execution"]["disposition"], json!("completed"));
    let surface = |server: &mut Server, first: i32, second: i32, prefix: &str| {
        call(
            server,
            "glioma_protocol_evidence_surface",
            json!({
                "request": {
                    "objective": "transport invasion evidence",
                    "protocol": protocol.clone(),
                    "execution": execution["execution"].clone(),
                    "measurements": [
                        {"measurement_id":format!("{prefix}-m1"),"task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":first,"uncertainty_milli":50,"quality_milli":900,"replicate_index":1},
                        {"measurement_id":format!("{prefix}-m2"),"task_id":"assay","output_schema":"Assay1@1","endpoint_id":"invasion","modality":"imaging","value_milli":second,"uncertainty_milli":50,"quality_milli":900,"replicate_index":2}
                    ],
                    "min_replicates":2,
                    "min_quality_milli":700,
                    "max_uncertainty_milli":200,
                    "contradiction_threshold_milli":100
                }
            }),
        )
    };
    let surface_a = surface(&mut server, 400, 420, "site-a");
    let surface_b = surface(&mut server, 430, 440, "site-b");
    let fusion = call(
        &mut server,
        "glioma_protocol_multistudy_fusion",
        json!({
            "request": {
                "objective": "transport invasion evidence",
                "studies": [
                    {"study_id":"study-a","site_id":"site-a","model_system":"organoid","surface":surface_a["surface"].clone()},
                    {"study_id":"study-b","site_id":"site-b","model_system":"mouse_model","surface":surface_b["surface"].clone()}
                ],
                "min_studies":2,
                "min_quality_milli":700,
                "max_heterogeneity_milli":100,
                "contradiction_threshold_milli":100
            }
        }),
    );
    let result = call(
        &mut server,
        "glioma_protocol_transport_gate",
        json!({
            "request": {
                "objective": "transport invasion evidence",
                "fusion": fusion["fusion"].clone(),
                "target_model_system": "mouse_model",
                "min_studies":2,
                "min_sites":2,
                "min_model_systems":2,
                "min_information_milli":700,
                "max_heterogeneity_milli":100,
                "require_positive_signal":true
            }
        }),
    );
    assert_eq!(result["dispatch"], json!("not_started"));
    assert_eq!(result["simulation_only"], json!(true));
    assert_eq!(result["gate"]["disposition"], json!("ready"));
    assert_eq!(result["gate"]["ready_endpoint_order"], json!(["invasion"]));
}

#[test]
fn glioma_release_metadata_normalizer_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_release_metadata_normalize",
        json!({
            "request": {
                "target_schema": "RO-Crate",
                "target_schema_version": "1.1",
                "target_fields": [
                    {"key":"assay","required":true,"vocabulary_id":"assay-v1"},
                    {"key":"organism_model","required":true,"vocabulary_id":null}
                ],
                "sources": [{
                    "source_id":"manifest",
                    "fields":[
                        {"source_field_id":"manifest-assay","key":"assay_name","value":" imaging "},
                        {"source_field_id":"manifest-model","key":"model","value":"mouse"}
                    ]
                }],
                "vocabularies": [{
                    "vocabulary_id":"assay-v1",
                    "terms":[{"source_value":"imaging","canonical_value":"microscopy"}]
                }],
                "mapping_rules": [
                    {"rule_id":"rule-assay","source_key":"assay_name","target_key":"assay","transform":"lowercase","vocabulary_id":"assay-v1","approved":true},
                    {"rule_id":"rule-model","source_key":"model","target_key":"organism_model","transform":"trim","vocabulary_id":null,"approved":true}
                ],
                "confirmed_inference_rule_ids": []
            }
        }),
    );
    assert_eq!(
        output["normalization"]["feature_id"],
        json!("GAF-GLIOMA-P11-F09")
    );
    assert_eq!(output["normalization"]["disposition"], json!("ready"));
    assert_eq!(
        output["normalization"]["fields"][0]["normalized_value"],
        json!("microscopy")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_release_attestation_issue_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let manifest = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"attestation-mcp-research","study_id":"attestation-mcp-study","objective":"release a preclinical glioma result","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05_mechanism"],"artifacts":[{"artifact_id":"attestation-artifact","content_hash":zero,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null-result"],"limitations":["single-model"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let output = call(
        &mut server,
        "glioma_release_attestation_issue",
        json!({
            "request": {
                "manifest": manifest,
                "build_provenance_digest": zero,
                "release_gate_digest": zero,
                "release_gate_status": "publishable",
                "signer_id": "mcp-release-authority",
                "policy_scope": "preclinical-research-release",
                "authority": {
                    "authority_id": "mcp-release-authority",
                    "key_id": "mcp-key-2026",
                    "algorithm": "institution-signature-seam-v1",
                    "active": true,
                    "revoked": false,
                    "allowed_policy_scope": "preclinical-research-release",
                    "revocation_epoch": null
                },
                "verification_results": [{
                    "verifier_id": "independent-replay",
                    "verification_schema": "replay-verifier-1",
                    "passed": true,
                    "evidence_digest": zero
                }],
                "issued_at_epoch": 20260923
            }
        }),
    );
    assert_eq!(
        output["attestation"]["feature_id"],
        json!("GAF-GLIOMA-P11-F08")
    );
    assert_eq!(output["attestation"]["status"], json!("signed"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_release_signature_verifier_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let manifest = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"verify-mcp-research","study_id":"verify-mcp-study","objective":"verify a preclinical glioma release","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05_mechanism"],"artifacts":[{"artifact_id":"verify-artifact","content_hash":zero,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null-result"],"limitations":["single-model"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let issued = call(
        &mut server,
        "glioma_release_attestation_issue",
        json!({"request":{"manifest":manifest,"build_provenance_digest":zero,"release_gate_digest":zero,"release_gate_status":"publishable","signer_id":"mcp-release-authority","policy_scope":"preclinical-research-release","authority":{"authority_id":"mcp-release-authority","key_id":"mcp-key-2026","algorithm":"institution-signature-seam-v1","active":true,"revoked":false,"allowed_policy_scope":"preclinical-research-release","revocation_epoch":null},"verification_results":[{"verifier_id":"independent-replay","verification_schema":"replay-verifier-1","passed":true,"evidence_digest":zero}],"issued_at_epoch":20260923}}),
    );
    let attestation = issued["attestation"].clone();
    let output = call(
        &mut server,
        "glioma_release_signature_verify",
        json!({"request":{"attestation":attestation,"expected_manifest_digest":issued["attestation"]["manifest_digest"],"expected_build_provenance_digest":zero,"expected_release_gate_digest":zero,"expected_policy_scope":"preclinical-research-release","verification_epoch":20260923,"max_attestation_age_epochs":Some(1u64),"trust_roots":[{"authority_id":"mcp-release-authority","key_id":"mcp-key-2026","algorithm":"institution-signature-seam-v1","active":true,"revoked":false,"revocation_epoch":null,"policy_scope_order":["preclinical-research-release"]}]}}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P11-F25"));
    assert_eq!(output["report"]["disposition"], json!("verified"));
    assert_eq!(output["report"]["cryptographic_valid"], json!(true));
}

#[test]
fn glioma_research_object_conformance_suite_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_research_object_conformance_check",
        json!({"request": {
            "object": {
                "object_id": "conformance-object",
                "schema_id": "aurora.glioma.result",
                "schema_version": "2.0",
                "fields": {"effect":"0.42"},
                "artifacts": [{"artifact_id":"result","content_hash":zero,"required":true,"local_only":true}],
                "provenance_digest": zero,
                "uncertainty_order": ["tail"],
                "negative_evidence_order": ["null"]
            },
            "profile": {
                "profile_id":"ro-crate-glioma",
                "schema_id":"aurora.glioma.result",
                "schema_version":"2.0",
                "required_field_order":["effect"],
                "allowed_field_order":["effect"],
                "forbidden_field_order":["protected"],
                "required_artifact_order":["result"],
                "extension_allowlist_order":["local_extension"],
                "require_provenance":false,
                "require_uncertainty":true,
                "require_negative_evidence":true,
                "require_verified_signature":false
            },
            "signature": null,
            "migration_route": null
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P11-F26"));
    assert_eq!(output["report"]["disposition"], json!("conformant"));
}

#[test]
fn glioma_release_preview_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let prepared = call(
        &mut server,
        "glioma_research_object_prepare",
        json!({"request":{"research_id":"preview-mcp-research","study_id":"preview-mcp-study","objective":"audience preview","plan_digest":zero,"execution_digest":zero,"replay_identity":zero,"program_order":["p05"],"artifacts":[{"artifact_id":"artifact-a","content_hash":zero,"content_type":"application/json","local_only":true,"contains_human_data":false,"contains_direct_identifiers":false}],"negative_evidence":["null"],"limitations":["preclinical"],"raw_data_local":true,"aggregate_only":true}}),
    );
    let output = call(
        &mut server,
        "glioma_release_preview",
        json!({
            "request": {
                "manifest": prepared,
                "audience_id": "consortium-review",
                "export_profile": "aggregate-review-v1",
                "allowed_section_order": ["limitations", "methods", "results"],
                "redact_section_order": ["results"],
                "allowed_artifact_order": ["artifact-a"],
                "prior_preview_digest": null,
                "prior_section_order": [],
                "prior_artifact_order": []
            }
        }),
    );
    assert_eq!(output["preview"]["feature_id"], json!("GAF-GLIOMA-P11-F17"));
    assert_eq!(output["preview"]["comparison"], json!("new_release"));
    assert_eq!(
        output["preview"]["redacted_section_order"],
        json!(["results"])
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_release_queue_snapshot_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let candidate = |id: &str, state: &str| {
        json!({
            "candidate_id": id,
            "version": "v1",
            "state": state,
            "required_check_order": ["replay", "review"],
            "passed_check_order": ["replay"],
            "failed_check_order": [],
            "reviewer_id": null,
            "priority": 5,
            "deadline_epoch": 120,
            "lineage_digest": zero
        })
    };
    let output = call(
        &mut server,
        "glioma_release_queue_snapshot",
        json!({
            "request": {
                "candidates": [candidate("candidate-blocked", "blocked"), candidate("candidate-ready", "ready")],
                "events": [{"event_id":"event-ready","candidate_id":"candidate-ready","kind":"state_observed","check_id":null,"reviewer_id":null,"observed_state":"ready","event_epoch":99,"telemetry_epoch":99}],
                "reviewers": [{"reviewer_id":"reviewer-a","assigned_count":1,"capacity":1,"active":true}],
                "now_epoch": 100,
                "stale_after_epochs": 10
            }
        }),
    );
    assert_eq!(
        output["snapshot"]["feature_id"],
        json!("GAF-GLIOMA-P11-F19")
    );
    assert_eq!(output["snapshot"]["ledger_candidate_count"], json!(2));
    assert_eq!(output["snapshot"]["reconciled_candidate_count"], json!(2));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_release_shareability_check_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let field = |id: &str| {
        json!({
            "field_id": id,
            "classification": "aggregate_result",
            "requested_export": true,
            "source_digest": zero
        })
    };
    let output = call(
        &mut server,
        "glioma_release_shareability_check",
        json!({
            "request": {
                "candidate_manifest_digest": zero,
                "root_order": ["root"],
                "dependencies": [
                    {"artifact_id":"root","dependency_order":["upstream"],"license_id":"CC-BY-4.0","fields":[field("summary")],"local_only":false,"embargo_until_epoch":null,"rights_confirmed":true,"contains_human_data":false,"intended_audience":"consortium"},
                    {"artifact_id":"upstream","dependency_order":[],"license_id":"MIT","fields":[{"field_id":"methods","classification":"public_metadata","requested_export":true,"source_digest":zero}],"local_only":false,"embargo_until_epoch":null,"rights_confirmed":true,"contains_human_data":false,"intended_audience":"consortium"}
                ],
                "policy": {"allowed_license_order":["CC-BY-4.0","MIT"],"forbidden_license_order":["PROPRIETARY"],"audience":"consortium","now_epoch":20260923,"permit_aggregate_export":true,"permit_local_only_export":false,"permit_human_data":false}
            }
        }),
    );
    assert_eq!(
        output["decision"]["feature_id"],
        json!("GAF-GLIOMA-P11-F12")
    );
    assert_eq!(output["decision"]["disposition"], json!("shareable"));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_archive_migration_adapter_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_archive_migration_execute",
        json!({
            "request": {
                "source": {
                    "object_id": "archive-object",
                    "schema_id": "aurora.glioma.result",
                    "schema_version": "1.0",
                    "fields": {"effect":"0.42", "study":"study-1"},
                    "artifacts": [{"artifact_id":"result","content_hash":zero,"required":true,"local_only":true}],
                    "provenance_digest": zero,
                    "uncertainty_order": ["tail"],
                    "negative_evidence_order": ["null"]
                },
                "target": {
                    "schema_id": "aurora.glioma.result",
                    "version": "2.0",
                    "required_field_order": ["effect", "study_id"],
                    "allowed_field_order": ["effect", "study_id"]
                },
                "rules": [
                    {"source_field":"effect","target_field":"effect","transform":"identity","reversible":true,"semantic_loss_milli":0,"rule_version":"1.0"},
                    {"source_field":"study","target_field":"study_id","transform":"rename","reversible":true,"semantic_loss_milli":0,"rule_version":"1.0"}
                ],
                "max_semantic_loss_milli": 0,
                "allow_optional_drop": false,
                "require_rollback": true
            }
        }),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P11-F22"));
    assert_eq!(output["report"]["disposition"], json!("ready"));
    assert_eq!(
        output["report"]["migrated"]["fields"]["study_id"],
        json!("study-1")
    );
}

#[test]
fn glioma_multistudy_release_compose_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let study = |id: &str| {
        json!({
            "study_id": id,
            "model_system": format!("organoid-{id}"),
            "methods_digest": zero,
            "provenance_digest": zero,
            "fields": [{"field_id": format!("{id}-invasion"), "concept":"invasion", "value":"0.5", "source_digest":zero}],
            "limitations": ["preclinical"]
        })
    };
    let output = call(
        &mut server,
        "glioma_multistudy_release_compose",
        json!({"request": {
            "studies": [study("study-a"), study("study-b")],
            "mappings": [],
            "required_concept_order": ["invasion"],
            "allow_comparable_pool": false
        }}),
    );
    assert_eq!(
        output["comparative"]["feature_id"],
        json!("GAF-GLIOMA-P11-F14")
    );
    assert_eq!(
        output["comparative"]["pooled_concept_order"],
        json!(["invasion"])
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_comparative_release_explore_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let study = |id: &str| {
        json!({
            "study_id": id,
            "model_system": format!("organoid-{id}"),
            "methods_digest": zero,
            "provenance_digest": zero,
            "fields": [{"field_id": format!("{id}-invasion"), "concept":"invasion", "value":"0.5", "source_digest":zero}],
            "limitations": ["preclinical"]
        })
    };
    let studies = vec![study("study-a"), study("study-b")];
    let composed = call(
        &mut server,
        "glioma_multistudy_release_compose",
        json!({"request": {
            "studies": studies,
            "mappings": [],
            "required_concept_order": ["invasion"],
            "allow_comparable_pool": false
        }}),
    );
    let output = call(
        &mut server,
        "glioma_comparative_release_explore",
        json!({"request": {
            "comparative_object": composed["comparative"],
            "studies": [study("study-a"), study("study-b")],
            "audience_id": "reviewer",
            "access_scope": "consortium",
            "target_concept_order": ["invasion"],
            "requested_study_order": ["study-a", "study-b"],
            "access_epoch": 100
        }}),
    );
    assert_eq!(output["view"]["feature_id"], json!("GAF-GLIOMA-P11-F18"));
    assert_eq!(output["view"]["cell_order"].as_array().unwrap().len(), 2);
    assert_eq!(output["view"]["protected_cache_evicted"], json!(true));
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_continuous_release_compile_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let output = call(
        &mut server,
        "glioma_continuous_release_compile",
        json!({"request": {
            "candidate_id": "candidate-mcp",
            "now_epoch": 100,
            "events": [{
                "sequence": 1,
                "event_id": "event-1",
                "candidate_id": "candidate-mcp",
                "version": "v1",
                "event_epoch": 100,
                "kind": "candidate_snapshot",
                "program_order": ["p01", "p07"],
                "artifact_order": ["result"],
                "negative_evidence": ["null-effect"],
                "schema_version": "schema-1",
                "policy_digest": zero,
                "content_digest": zero,
                "omission_reason": null
            }],
            "rules": {
                "required_program_order": ["p01", "p07"],
                "required_artifact_order": ["result"],
                "schema_version": "schema-1",
                "policy_digest": zero,
                "max_event_age": 10,
                "require_negative_evidence_field": true
            },
            "prior": null
        }}),
    );
    assert_eq!(
        output["candidate"]["feature_id"],
        json!("GAF-GLIOMA-P11-F15")
    );
    assert_eq!(
        output["candidate"]["disposition"],
        json!("ready_for_review")
    );
    assert_eq!(output["dispatch"], json!("not_started"));
}

#[test]
fn glioma_release_event_protocol_replay_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let seal = |event_id: &str, sequence: u64, kind: &str, predecessor_digest: Option<String>| {
        let payload = json!({
            "event_id": event_id,
            "object_id": "event-object",
            "sequence": sequence,
            "kind": kind,
            "object_digest": zero.clone(),
            "predecessor_digest": predecessor_digest,
            "authority_id": "authority",
            "key_id": "key-1",
            "policy_scope": "consortium",
            "authority_active": true,
            "authority_revoked": false,
            "issued_epoch": sequence
        });
        let digest = ContentHash::of_value(&payload).unwrap().to_string();
        let mut event = payload.as_object().unwrap().clone();
        event.insert("payload_digest".into(), json!(digest));
        json!(event)
    };
    let candidate = seal("candidate", 1, "candidate", None);
    let candidate_digest = candidate["payload_digest"].as_str().unwrap().to_string();
    let review = seal("review", 2, "review", Some(candidate_digest));
    let review_digest = review["payload_digest"].as_str().unwrap().to_string();
    let published = seal("published", 3, "published", Some(review_digest));
    let output = call(
        &mut server,
        "glioma_release_event_protocol_replay",
        json!({"request": {
            "object_id": "event-object",
            "policy_scope": "consortium",
            "events": [candidate, review, published],
            "max_events": 32
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["report"]["feature_id"], json!("GAF-GLIOMA-P11-F23"));
    assert_eq!(output["report"]["disposition"], json!("replayed"));
    assert_eq!(output["report"]["terminal_state"], json!("published"));
}

#[test]
fn glioma_version_retention_plan_is_reachable_through_mcp() {
    let mut server = server();
    let digest = ContentHash::of_value(&json!({"version": "v1"}))
        .unwrap()
        .to_string();
    let output = call(
        &mut server,
        "glioma_version_retention_plan",
        json!({"request": {
            "versions": [{"version_id":"v1","object_id":"object","content_digest":digest,"predecessor_version_id":null,"issued_epoch":1,"immutable":true,"pinned":true,"legal_hold":false,"superseded_by":null,"storage_tier":"hot"}],
            "storage_health": [{"version_id":"v1","verified_replica_count":2,"last_verified_epoch":100,"digest_matches_source":true,"approved_region_order":["eu","us"]}],
            "policy": {"now_epoch":100,"archive_after_epochs":20,"minimum_retain_epochs":10,"minimum_verified_replicas":2,"allow_delete_plan":true,"require_immutable_versions":true}
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["plan"]["feature_id"], json!("GAF-GLIOMA-P11-F29"));
    assert_eq!(output["plan"]["disposition"], json!("planned"));
    assert_eq!(output["plan"]["protected_version_order"], json!(["v1"]));
}

#[test]
fn glioma_distributed_archive_mirror_is_reachable_through_mcp() {
    let mut server = server();
    let digest = ContentHash::of_value(&json!({"version": "v1"}))
        .unwrap()
        .to_string();
    let output = call(
        &mut server,
        "glioma_distributed_archive_mirror",
        json!({"request": {
            "sources": [{"version_id":"v1","content_digest":digest,"approved_region_order":["eu","us"]}],
            "replicas": [
                {"replica_id":"r1","version_id":"v1","region":"eu","content_digest":digest,"verified_epoch":100,"available":true,"policy_allowed":true},
                {"replica_id":"r2","version_id":"v1","region":"us","content_digest":digest,"verified_epoch":100,"available":true,"policy_allowed":true}
            ],
            "policy": {"now_epoch":100,"stale_after_epochs":10,"required_replica_count":2,"require_locality":true,"max_repair_tasks":8}
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["status"]["feature_id"], json!("GAF-GLIOMA-P11-F30"));
    assert_eq!(output["status"]["disposition"], json!("synchronized"));
}

#[test]
fn glioma_release_queue_schedule_is_reachable_through_mcp() {
    let mut server = server();
    let output = call(
        &mut server,
        "glioma_release_queue_schedule",
        json!({"request": {
            "candidates": [
                {"candidate_id":"candidate-a","object_id":"object-a","gate_ready":true,"reviewer_ready":true,"compute_units":2,"reviewer_units":1,"risk_milli":200,"deadline_epoch":105,"fairness_credit":3},
                {"candidate_id":"candidate-b","object_id":"object-b","gate_ready":false,"reviewer_ready":false,"compute_units":2,"reviewer_units":1,"risk_milli":100,"deadline_epoch":105,"fairness_credit":4}
            ],
            "capacity": {"now_epoch":100,"planning_horizon_epochs":10,"compute_units":4,"reviewer_units":2,"max_scheduled":2}
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(
        output["schedule"]["feature_id"],
        json!("GAF-GLIOMA-P11-F31")
    );
    assert_eq!(
        output["schedule"]["scheduled_order"],
        json!(["candidate-a"])
    );
    assert_eq!(output["schedule"]["disposition"], json!("blocked"));
}

#[test]
fn glioma_research_object_exchange_plan_is_reachable_through_mcp() {
    let mut server = server();
    let object_digest = ContentHash::of_value(&json!({"object":"exchange"}))
        .unwrap()
        .to_string();
    let chunk_digest = |index: u32| {
        ContentHash::of_value(&json!({"chunk":index}))
            .unwrap()
            .to_string()
    };
    let output = call(
        &mut server,
        "glioma_research_object_exchange_plan",
        json!({"request": {
            "manifest":{"object_id":"object","version_id":"v1","content_digest":object_digest,"total_bytes":8,"chunk_size":4,"audience_scope":"consortium","locality_region":"us","signed":true},
            "chunks":[{"index":0,"content_digest":chunk_digest(0),"byte_len":4,"acknowledged":true},{"index":1,"content_digest":chunk_digest(1),"byte_len":4,"acknowledged":true}],
            "policy":{"recipient_id":"archive","permitted_audience_scope":"consortium","permitted_region":"us","max_bytes":16,"grant_active":true,"require_signed_manifest":true},
            "resume_cursor":0
        }}),
    );
    assert_eq!(output["dispatch"], json!("not_started"));
    assert_eq!(output["record"]["feature_id"], json!("GAF-GLIOMA-P11-F21"));
    assert_eq!(output["record"]["disposition"], json!("ready"));
}

#[test]
fn glioma_release_dependency_leakage_audit_is_reachable_through_mcp() {
    let mut server = server();
    let zero = "0".repeat(64);
    let node = |node_id: &str, dependency_order: Vec<&str>| {
        json!({
            "node_id": node_id,
            "path": format!("/release/{node_id}.json"),
            "content_type": "application/json",
            "content_hash": zero,
            "dependency_order": dependency_order,
            "export_scope": "public",
            "requested_export": true,
            "contains_human_data": false,
            "contains_direct_identifiers": false,
            "embedded_secret": false,
            "symlink_escape": false
        })
    };
    let output = call(
        &mut server,
        "glioma_research_object_dependency_leakage_audit",
        json!({
            "request": {
                "candidate_manifest_digest": zero,
                "root_order": ["root"],
                "nodes": [node("root", vec!["upstream"]), node("upstream", vec![])],
                "allowed_export_prefixes": ["/release/"],
                "max_depth": 8
            }
        }),
    );
    assert_eq!(output["audit"]["feature_id"], json!("GAF-GLIOMA-P11-F03"));
    assert_eq!(output["audit"]["disposition"], json!("clear"));
    assert_eq!(output["audit"]["export_blocked"], json!(false));
    assert_eq!(output["dispatch"], json!("not_started"));
}
