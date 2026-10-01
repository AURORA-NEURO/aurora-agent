//! Check blueprint 11.11's advertised-tool contract in a small target, so schema/dispatch drift
//! is caught without compiling the large protocol behavior suite.

use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use bioprism_ids::ContentHash;
use bioprism_mcp::{Request, Server, tool_definitions};
use bioprism_research::glioma::evidence::{EvidenceSourceKind, EvidenceState};
use bioprism_research::{
    DecisionContextArtifactCompatibility, DecisionContextArtifactConsumer,
    DecisionContextArtifactRequest, DecisionContextRequest, EvidenceRecord,
    ExpectedDisclosurePanel, ExpectedDisclosureStudy, GliomaModality, GliomaModelSystem,
    KnowledgeRequest, LocalArtifactRef, MultiStudyContextEpochReplayRequest,
    MultiStudyContextInput, MultiStudyContextRequest, MultiStudyDecisionContextArtifact,
    OutcomeAvailability, OutcomeSensitivityStudy, RegisteredOutcomeEvidencePanelRequest,
    RegisteredOutcomeRecord, ReleaseDisclosureBatchInput, ReleaseDisclosureBatchRequest,
    ReleaseDisclosurePanelRequest, ReleaseDisclosureStudyInput, ResearchObjectManifest,
    ResearchObjectRequest, align_glioma_multi_study_context_artifacts,
    build_glioma_registered_outcome_record, build_research_object_manifest,
    compile_decision_context, compile_glioma_release_disclosure_register, compile_typed_knowledge,
    materialize_glioma_decision_context_artifact, reconcile_glioma_release_disclosure_batch,
    reconcile_glioma_release_disclosure_panel,
};
use serde_json::{Value, json};

fn rust_sources(root: &Path, found: &mut Vec<(PathBuf, String)>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                continue;
            }
            rust_sources(&path, found)?;
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "rs")
        {
            found.push((path.clone(), fs::read_to_string(path)?));
        }
    }
    Ok(())
}

fn static_tool_routes(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter_map(|line| {
            let tail = line.trim_start().strip_prefix('"')?;
            let (name, rest) = tail.split_once('"')?;
            rest.trim_start().starts_with("=>").then_some(name)
        })
        .collect()
}

fn initialized_server() -> Server {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists");
    let mut server = Server::new(root);
    let initialize =
        Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
            .expect("initialize request parses");
    server.handle(&initialize).expect("initialize is answered");
    let initialized = Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .expect("initialized notification parses");
    assert!(server.handle(&initialized).is_none());
    server
}

fn call_tool(server: &mut Server, name: &str, arguments: Value) -> Value {
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        })
        .to_string(),
    )
    .expect("tool request parses");
    server
        .handle(&request)
        .expect("tool call is answered")
        .to_json()
}

fn registered_outcome_record(study_id: &str) -> RegisteredOutcomeRecord {
    let content_hash = |label: &str| ContentHash::of_bytes(label.as_bytes());
    let artifact = |label: &str| LocalArtifactRef {
        artifact_id: label.into(),
        content_hash: content_hash(label),
        content_type: "application/vnd.aurora.local-study-outcome+json".into(),
        local_only: true,
        contains_human_data: false,
        contains_direct_identifiers: false,
    };
    let study = OutcomeSensitivityStudy {
        study_id: study_id.into(),
        independence_group: format!("group-{study_id}"),
        registry_report_digest: content_hash(&format!("registry-{study_id}")),
        registry_artifact: artifact(&format!("registry-artifact-{study_id}")),
        result_report_digest: Some(content_hash(&format!("result-{study_id}"))),
        result_artifact: Some(artifact(&format!("result-artifact-{study_id}"))),
        outcome_id: "primary-viability".into(),
        estimand_id: "viability-change-at-24h".into(),
        effect_unit: "normalized-signal-milli".into(),
        availability: OutcomeAvailability::Estimate,
        effect_milli: Some(200),
        uncertainty_milli: Some(20),
        quality_milli: Some(950),
    };
    build_glioma_registered_outcome_record(&study).expect("fixture record validates")
}

fn release_manifest(study_id: &str) -> ResearchObjectManifest {
    let hash = |label: &str| ContentHash::of_value(&json!({ "label": label })).unwrap();
    build_research_object_manifest(&ResearchObjectRequest {
        research_id: "disclosure-route-test".into(),
        study_id: study_id.into(),
        objective: "verify preclinical release disclosures".into(),
        plan_digest: hash(&format!("plan-{study_id}")),
        execution_digest: hash(&format!("execution-{study_id}")),
        replay_identity: hash(&format!("replay-{study_id}")),
        program_order: vec!["p10-interpretation".into()],
        artifacts: vec![LocalArtifactRef {
            artifact_id: format!("analysis-artifact-{study_id}"),
            content_hash: hash(&format!("artifact-{study_id}")),
            content_type: "application/vnd.aurora.glioma-result+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }],
        negative_evidence: vec!["null effect in independent validation".into()],
        limitations: vec!["single model system".into(), "short follow-up".into()],
        raw_data_local: true,
        aggregate_only: true,
    })
    .expect("release manifest validates")
}

fn multi_study_input(study_id: &str, group: &str, claim: &str) -> MultiStudyContextInput {
    let objective = "temporal decision-frontier route test";
    let evidence = EvidenceRecord {
        evidence_id: format!("evidence-{study_id}"),
        source_artifact: LocalArtifactRef {
            artifact_id: format!("artifact-{study_id}"),
            content_hash: ContentHash::of_value(&json!({"study": study_id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        },
        source_kind: EvidenceSourceKind::Dataset,
        claim: claim.into(),
        scope: "preclinical glioma".into(),
        modality: GliomaModality::Genomics,
        model_system: Some(GliomaModelSystem::Organoid),
        state: EvidenceState::Supported,
        relevance_milli: 900,
        quality_milli: 900,
        reproducibility_milli: 900,
        release_epoch: 1,
    };
    let knowledge = compile_typed_knowledge(
        &KnowledgeRequest {
            objective: objective.into(),
            required_modalities: BTreeSet::from([GliomaModality::Genomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            min_support_milli: 700,
            min_sources_per_claim: 1,
            max_claims: 8,
        },
        &[evidence],
    )
    .unwrap();
    let context = compile_decision_context(
        &DecisionContextRequest {
            objective: objective.into(),
            max_actions: 8,
            default_cost_units: 5,
        },
        &knowledge,
    )
    .unwrap();
    let compatibility = DecisionContextArtifactCompatibility {
        contract_version: "glioma-context-compat/1".into(),
        consumer_order: vec![
            DecisionContextArtifactConsumer::LocalAgent,
            DecisionContextArtifactConsumer::ResearcherWorkbench,
            DecisionContextArtifactConsumer::RustSdk,
            DecisionContextArtifactConsumer::PythonSdk,
            DecisionContextArtifactConsumer::TypeScriptSdk,
            DecisionContextArtifactConsumer::McpClient,
        ],
        semantic_loss_order: Vec::new(),
        local_raw_data_required: false,
        clinical_decision_capable: false,
    };
    let artifact = materialize_glioma_decision_context_artifact(&DecisionContextArtifactRequest {
        objective: objective.into(),
        study_id: study_id.into(),
        epoch: 1,
        compatibility,
        context,
    })
    .unwrap();
    MultiStudyContextInput {
        study_id: study_id.into(),
        independent_group: group.into(),
        quality_milli: 900,
        policy_allowed: true,
        artifact,
        outcome_order: Vec::new(),
    }
}

fn multi_study_epoch(epoch: u32) -> MultiStudyDecisionContextArtifact {
    align_glioma_multi_study_context_artifacts(&MultiStudyContextRequest {
        objective: "temporal decision-frontier route test".into(),
        epoch,
        minimum_studies: 2,
        minimum_independent_groups: 2,
        minimum_action_support_milli: 700,
        minimum_quality_milli: 700,
        maximum_actions: 8,
        compatibility: DecisionContextArtifactCompatibility {
            contract_version: "glioma-context-compat/1".into(),
            consumer_order: vec![
                DecisionContextArtifactConsumer::LocalAgent,
                DecisionContextArtifactConsumer::ResearcherWorkbench,
                DecisionContextArtifactConsumer::RustSdk,
                DecisionContextArtifactConsumer::PythonSdk,
                DecisionContextArtifactConsumer::TypeScriptSdk,
                DecisionContextArtifactConsumer::McpClient,
            ],
            semantic_loss_order: Vec::new(),
            local_raw_data_required: false,
            clinical_decision_capable: false,
        },
        studies: vec![
            multi_study_input("study-a", "group-a", "EGFR signaling increases invasion"),
            multi_study_input("study-b", "group-b", "EGFR signaling increases invasion"),
        ],
    })
    .unwrap()
}

fn assert_closed_fixed_object_schemas(value: &Value, path: &str) {
    match value {
        Value::Object(object) => {
            if object.get("type") == Some(&json!("object")) && object.contains_key("properties") {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "fixed object schema at {path} must reject undeclared fields"
                );
            }
            for (key, child) in object {
                assert_closed_fixed_object_schemas(child, &format!("{path}/{key}"));
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_closed_fixed_object_schemas(child, &format!("{path}/{index}"));
            }
        }
        _ => {}
    }
}

#[test]
fn all_embedded_tools_have_unique_names_and_closed_required_field_schemas() {
    let tools = tool_definitions();
    assert_eq!(tools.len(), 938);
    assert_eq!(
        ContentHash::of_value(&Value::Array(tools.clone()))
            .unwrap()
            .to_string(),
        "98d4d62dce6401212bfe339d0c65344155fe5de22a2159d3169ae1c95a31f5bb"
    );

    let mut names = BTreeSet::new();
    for tool in &tools {
        let name = tool["name"].as_str().expect("tool has a string name");
        assert!(!name.is_empty(), "tool names are non-empty");
        assert!(names.insert(name), "duplicate MCP tool name: {name}");
        assert!(tool["description"].as_str().unwrap().len() > 40);

        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert_closed_fixed_object_schemas(schema, name);
        assert_eq!(
            schema["additionalProperties"], false,
            "{name} must reject unknown root fields"
        );
        let properties = schema["properties"]
            .as_object()
            .expect("tool schema has an object properties map");
        let required = schema["required"]
            .as_array()
            .expect("tool schema has an array of required fields");
        for field in required {
            let field = field.as_str().expect("required field name is a string");
            assert!(
                properties.contains_key(field),
                "{name} requires {field} but does not declare it"
            );
        }
    }

    let bioeval = tools
        .iter()
        .find(|tool| tool["name"] == "bioeval_reveal_audit")
        .expect("the bioeval audit tool is present");
    for field in ["rubric", "score_rubric"] {
        assert_eq!(
            bioeval["inputSchema"]["properties"][field]
                .as_object()
                .map(serde_json::Map::len),
            Some(1),
            "{field} remains an intentionally free-form JSON payload"
        );
    }
}

#[test]
fn direct_tool_dispatch_rejects_unknown_root_arguments_before_execution() {
    let mut server = initialized_server();
    let rejected = call_tool(
        &mut server,
        "workspace_capabilities",
        json!({"unexpected": true}),
    );
    assert_eq!(rejected["result"]["isError"], true);
    let refusal = rejected["result"]["content"][0]["text"]
        .as_str()
        .expect("schema refusal is text");
    assert!(refusal.contains("additional_property"), "{refusal}");

    let accepted = call_tool(&mut server, "workspace_capabilities", json!({}));
    assert_eq!(accepted["result"]["isError"], false);

    let rejected_nested = call_tool(
        &mut server,
        "brain_model_select_contextual",
        json!({
            "context": {
                "domain": "software_engineering",
                "capability": "implementation",
                "risk_class": "low",
                "misspelled_field": true,
            },
            "base": {},
        }),
    );
    assert_eq!(rejected_nested["result"]["isError"], true);
    let refusal = rejected_nested["result"]["content"][0]["text"]
        .as_str()
        .expect("nested schema refusal is text");
    assert!(refusal.contains("/context/misspelled_field"), "{refusal}");
}

#[test]
fn federated_continual_promotion_dispatches_and_requires_a_stable_epoch_window() {
    let mut server = initialized_server();
    let mut source_reports = Vec::new();
    for epoch in 1..=2 {
        let sites = ["site-a", "site-b"]
            .into_iter()
            .enumerate()
            .map(|(index, site)| {
                json!({
                    "site_id": site,
                    "independent_group": format!("group-{index}"),
                    "plan_digest": ContentHash::of_bytes(format!("{site}-{epoch}").as_bytes()).to_string(),
                    "quality_milli": 900,
                    "local_only": true,
                    "aggregate_only": true,
                    "policy_allowed": true,
                    "observations": [{
                        "observation_id": format!("{site}-branch-a-epoch-{epoch}"),
                        "branch_id": "branch-a",
                        "outcome": "qualified",
                        "expected_value_milli": 800,
                        "worst_case_value_milli": 700,
                        "uncertainty_milli": 100,
                        "failure_risk_milli": 100,
                        "support_milli": 900,
                        "cost_units": 2
                    }]
                })
            })
            .collect::<Vec<_>>();
        let source = call_tool(
            &mut server,
            "glioma_federated_decision_context",
            json!({"request": {
                "objective": "prioritize invasion research",
                "epoch": epoch,
                "minimum_sites": 2,
                "minimum_independent_groups": 2,
                "minimum_quality_milli": 700,
                "minimum_branch_support_milli": 700,
                "maximum_heterogeneity_milli": 250,
                "maximum_influence_milli": 300,
                "maximum_branches": 8,
                "sites": sites
            }}),
        );
        assert_eq!(source["result"]["isError"], false);
        let text = source["result"]["content"][0]["text"]
            .as_str()
            .expect("source report is JSON text");
        source_reports.push(serde_json::from_str::<Value>(text).unwrap()["report"].clone());
    }

    let promoted = call_tool(
        &mut server,
        "glioma_federated_continual_context_promotion",
        json!({"request": {
            "objective": "prioritize invasion research",
            "minimum_consecutive_epochs": 2,
            "minimum_sites": 2,
            "minimum_independent_groups": 2,
            "minimum_support_milli": 700,
            "maximum_uncertainty_milli": 250,
            "maximum_failure_risk_milli": 250,
            "maximum_heterogeneity_milli": 250,
            "maximum_influence_milli": 300,
            "source_reports": source_reports
        }}),
    );
    assert_eq!(promoted["result"]["isError"], false);
    let text = promoted["result"]["content"][0]["text"]
        .as_str()
        .expect("promotion report is JSON text");
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["report"]["disposition"], "promote");
    assert_eq!(payload["report"]["promoted_branch_id"], "branch-a");
    assert_eq!(payload["report"]["stable_epoch_order"], json!([1, 2]));
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn multi_study_context_epoch_replay_dispatches_a_stable_frontier_without_execution() {
    let mut server = initialized_server();
    let request = MultiStudyContextEpochReplayRequest {
        objective: "temporal decision-frontier route test".into(),
        minimum_consecutive_qualified_epochs: 2,
        minimum_studies: 2,
        minimum_independent_groups: 2,
        minimum_support_milli: 700,
        maximum_disagreement_milli: 250,
        source_artifacts: vec![multi_study_epoch(1), multi_study_epoch(2)],
    };
    let result = call_tool(
        &mut server,
        "glioma_multi_study_context_epoch_replay",
        json!({"request": request}),
    );

    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("epoch replay report is JSON text");
    let payload: Value = serde_json::from_str(text).unwrap();
    assert_eq!(payload["report"]["disposition"], "ready");
    assert_eq!(
        payload["report"]["stable_frontier_order"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(payload["dispatch"], "not_started");
    assert_eq!(payload["next_route"], "glioma_decision_operating_cycle");
}

#[test]
fn advertised_tool_names_match_the_server_dispatch_table() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut sources = Vec::new();
    rust_sources(&crate_root, &mut sources).expect("the MCP crate source tree is readable");
    assert!(
        sources.len() >= 80,
        "the scanner opened only {} Rust files; the source walk must cover the MCP crate",
        sources.len()
    );
    let dispatch_path = crate_root.join("src/server/dispatch.rs");
    let dispatch_source = sources
        .iter()
        .find(|(path, _)| path == &dispatch_path)
        .map(|(_, source)| source)
        .expect("the source walk must include the authoritative Server tool dispatcher");

    let advertised = tool_definitions()
        .into_iter()
        .map(|definition| {
            definition["name"]
                .as_str()
                .expect("tool has a name")
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let route_rows = static_tool_routes(dispatch_source);
    let routed = route_rows.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        route_rows.len(),
        routed.len(),
        "the MCP dispatcher must not declare duplicate literal routes"
    );
    assert_eq!(
        advertised,
        routed.into_iter().map(str::to_owned).collect(),
        "every advertised MCP tool must have exactly one dispatch route and every static route must be listed"
    );
}

#[test]
fn longitudinal_transport_tool_dispatches_empty_sources_as_unresolved() {
    let mut server = initialized_server();
    let result = call_tool(
        &mut server,
        "glioma_longitudinal_transport_analyze",
        json!({
            "request": {
                "objective": "route contract smoke test",
                "estimand_id": "viability-change-per-timepoint",
                "effect_unit": "normalized-signal-milli",
                "target_model_system": "organoid",
                "target_signature": [100, 200],
                "timepoint_order": [0, 10, 20],
                "min_studies": 2,
                "min_replicates_per_timepoint": 3,
                "min_quality_milli": 800,
                "distance_scale_milli": 500,
                "max_transport_gap_milli": 500,
                "max_heterogeneity_milli": 500,
                "min_direction_concordance_milli": 1000,
                "max_leave_one_out_shift_milli": 100,
                "effect_threshold_milli_per_tick": 5
            },
            "observations": []
        }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("analysis result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("analysis result parses");
    assert_eq!(payload["analysis"]["disposition"], "unresolved");
    assert_eq!(
        payload["analysis"]["pooled_trend_milli_per_tick"],
        Value::Null
    );
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn prospective_contradiction_tool_dispatches_insufficient_evidence_as_a_plan() {
    let mut server = initialized_server();
    let result = call_tool(
        &mut server,
        "glioma_prospective_contradiction_plan",
        json!({
            "request": {
                "objective": "route contract smoke test",
                "estimand_id": "viability-change-at-24h",
                "effect_unit": "normalized-signal-milli",
                "source_evidence_report_order": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                "resolver_design_report_order": [],
                "rival_hypotheses": [
                    {"hypothesis_id": "A", "statement_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
                    {"hypothesis_id": "B", "statement_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}
                ],
                "min_independent_support_groups": 2,
                "min_quality_milli": 800,
                "min_prediction_separation_milli": 100,
                "min_feasibility_milli": 600,
                "max_risk_milli": 500,
                "budget_units": 10,
                "max_selected_candidates": 2
            },
            "evidence": [],
            "candidates": []
        }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("plan result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("plan result parses");
    assert_eq!(payload["plan"]["disposition"], "insufficient_evidence");
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn registered_outcome_reporting_tool_dispatches_empty_sources_as_insufficient() {
    let mut server = initialized_server();
    let result = call_tool(
        &mut server,
        "glioma_registered_outcome_reporting_audit",
        json!({
            "request": {
                "objective": "route contract smoke test",
                "registry_report_order": [],
                "result_report_order": [],
                "evaluation_day": 900,
                "max_reporting_lag_days": 90,
                "min_studies": 2,
                "min_independent_groups": 2,
                "min_completed_studies": 1,
                "min_primary_coverage_milli": 1000,
                "max_unreported_primary_groups": 0
            },
            "studies": []
        }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("audit result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("audit result parses");
    assert_eq!(payload["audit"]["disposition"], "insufficient_evidence");
    assert_eq!(payload["audit"]["dispatch"], "not_dispatched_audit_only");
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn registered_outcome_record_tool_seals_one_study_record() {
    let mut server = initialized_server();
    let sample = registered_outcome_record("study-a");
    let result = call_tool(
        &mut server,
        "glioma_registered_outcome_record_build",
        json!({"study": sample.study}),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("record result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("record result parses");
    assert_eq!(payload["record"]["feature_id"], "GAF-GLIOMA-P10-F05");
    assert_eq!(payload["record"]["digest"].as_str().unwrap().len(), 64);
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn release_disclosure_register_tool_covers_manifest_statements_without_returning_them() {
    let mut server = initialized_server();
    let manifest = release_manifest("study-a");
    let expected = compile_glioma_release_disclosure_register(&manifest).unwrap();
    let result = call_tool(
        &mut server,
        "glioma_release_disclosure_register_build",
        json!({ "manifest": manifest }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("register result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("register result parses");
    assert_eq!(payload["register"]["feature_id"], "GAF-GLIOMA-P11-F02");
    assert_eq!(payload["register"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(
        payload["register"]["register_digest"],
        expected.register_digest.to_string()
    );
    assert_eq!(payload["register"]["scientific_truth_verified"], false);
    assert_eq!(payload["register"]["statement_digests_are_unkeyed"], true);
    assert_eq!(payload["register"]["release_authorized"], false);
    assert!(!text.contains("single model system"));
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn release_disclosure_panel_tool_preserves_missing_expected_studies() {
    let mut server = initialized_server();
    let manifest_a = release_manifest("study-a");
    let manifest_b = release_manifest("study-b");
    let register_a = compile_glioma_release_disclosure_register(&manifest_a).unwrap();
    let studies = vec![ReleaseDisclosureStudyInput {
        manifest: manifest_a.clone(),
        register: register_a,
    }];
    let request = ReleaseDisclosurePanelRequest {
        research_id: "disclosure-route-test".into(),
        expected_studies: vec![
            ExpectedDisclosureStudy {
                study_id: "study-a".into(),
                independence_group: "group-a".into(),
                manifest_digest: manifest_a.manifest_digest.clone(),
            },
            ExpectedDisclosureStudy {
                study_id: "study-b".into(),
                independence_group: "group-b".into(),
                manifest_digest: manifest_b.manifest_digest.clone(),
            },
        ],
    };
    let expected = reconcile_glioma_release_disclosure_panel(&request, &studies).unwrap();
    let result = call_tool(
        &mut server,
        "glioma_release_disclosure_panel_reconcile",
        json!({ "request": request, "studies": studies }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("panel result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("panel result parses");
    assert_eq!(payload["panel"]["feature_id"], "GAF-GLIOMA-P11-F03");
    assert_eq!(
        payload["panel"]["panel_digest"],
        expected.panel_digest.to_string()
    );
    assert_eq!(payload["panel"]["disposition"], "partial");
    assert_eq!(payload["panel"]["counts"]["registered_studies"], 1);
    assert_eq!(payload["panel"]["counts"]["missing_studies"], 1);
    assert_eq!(payload["panel"]["studies"][1]["status"], "missing");
    assert!(!text.contains("single model system"));
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn release_disclosure_batch_tool_reconciles_prioritized_panels_without_execution() {
    let mut server = initialized_server();
    let manifest_a = release_manifest("study-a");
    let manifest_b = release_manifest("study-b");
    let panel_request = ReleaseDisclosurePanelRequest {
        research_id: "disclosure-route-test".into(),
        expected_studies: vec![
            ExpectedDisclosureStudy {
                study_id: "study-a".into(),
                independence_group: "group-a".into(),
                manifest_digest: manifest_a.manifest_digest.clone(),
            },
            ExpectedDisclosureStudy {
                study_id: "study-b".into(),
                independence_group: "group-b".into(),
                manifest_digest: manifest_b.manifest_digest.clone(),
            },
        ],
    };
    let panel_request_digest = reconcile_glioma_release_disclosure_panel(&panel_request, &[])
        .unwrap()
        .request_digest;
    let batch_request = ReleaseDisclosureBatchRequest {
        batch_id: "disclosure-batch-route-test".into(),
        expected_panels: vec![
            ExpectedDisclosurePanel {
                item_id: "panel-a".into(),
                priority: 5,
                research_id: panel_request.research_id.clone(),
                request_digest: panel_request_digest,
            },
            ExpectedDisclosurePanel {
                item_id: "panel-b".into(),
                priority: 10,
                research_id: "pending-research".into(),
                request_digest: ContentHash::of_value(&json!({ "pending": true })).unwrap(),
            },
        ],
    };
    let studies = vec![ReleaseDisclosureStudyInput {
        manifest: manifest_a.clone(),
        register: compile_glioma_release_disclosure_register(&manifest_a).unwrap(),
    }];
    let panels = vec![ReleaseDisclosureBatchInput {
        item_id: "panel-a".into(),
        request: panel_request,
        studies,
    }];
    let expected = reconcile_glioma_release_disclosure_batch(&batch_request, &panels).unwrap();
    let result = call_tool(
        &mut server,
        "glioma_release_disclosure_batch_reconcile",
        json!({ "request": batch_request, "panels": panels }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("batch result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("batch result parses");
    assert_eq!(payload["batch"]["feature_id"], "GAF-GLIOMA-P11-F04");
    assert_eq!(
        payload["batch"]["batch_digest"],
        expected.batch_digest.to_string()
    );
    assert_eq!(payload["batch"]["counts"]["expected_panels"], 2);
    assert_eq!(payload["batch"]["counts"]["awaiting_submission"], 1);
    assert_eq!(payload["batch"]["items"][0]["status"], "panel_partial");
    assert_eq!(
        payload["batch"]["items"][1]["status"],
        "awaiting_submission"
    );
    assert_eq!(payload["batch"]["dispatch_started"], false);
    assert_eq!(payload["dispatch"], "not_started");
    assert!(!text.contains("single model system"));
}

#[test]
fn registered_outcome_panel_tool_combines_digest_validated_independent_records() {
    let mut server = initialized_server();
    let records = vec![
        registered_outcome_record("study-a"),
        registered_outcome_record("study-b"),
    ];
    let mut registry_report_order = records
        .iter()
        .map(|record| record.study.registry_report_digest.clone())
        .collect::<Vec<_>>();
    registry_report_order.sort();
    let mut result_report_order = records
        .iter()
        .filter_map(|record| record.study.result_report_digest.clone())
        .collect::<Vec<_>>();
    result_report_order.sort();
    let request = RegisteredOutcomeEvidencePanelRequest {
        objective: "route contract test".into(),
        outcome_id: "primary-viability".into(),
        estimand_id: "viability-change-at-24h".into(),
        effect_unit: "normalized-signal-milli".into(),
        registry_report_order,
        result_report_order,
    };
    let result = call_tool(
        &mut server,
        "glioma_registered_outcome_evidence_panel",
        json!({"request": request, "records": records}),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("panel result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("panel result parses");
    assert_eq!(payload["panel"]["feature_id"], "GAF-GLIOMA-P10-F06");
    assert_eq!(
        payload["panel"]["study_order"],
        json!(["study-a", "study-b"])
    );
    assert_eq!(
        payload["panel"]["estimate_order"],
        json!(["study-a", "study-b"])
    );
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn registered_outcome_sensitivity_tool_dispatches_empty_sources_as_insufficient() {
    let mut server = initialized_server();
    let result = call_tool(
        &mut server,
        "glioma_registered_outcome_sensitivity_analyze",
        json!({
            "request": {
                "objective": "route contract smoke test",
                "outcome_id": "primary-viability",
                "estimand_id": "viability-change-at-24h",
                "effect_unit": "normalized-signal-milli",
                "expected_direction": "positive",
                "registry_report_order": [],
                "result_report_order": [],
                "min_studies": 2,
                "min_reported_independent_groups": 2,
                "min_supporting_groups": 1,
                "min_quality_milli": 800,
                "effect_threshold_milli": 50,
                "missing_effect_min_milli": -300,
                "missing_effect_max_milli": 300,
                "missing_uncertainty_max_milli": 20,
                "missing_scenario_quality_milli": 900,
                "min_direction_concordance_milli": 800
            },
            "studies": []
        }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("sensitivity result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("sensitivity result parses");
    assert_eq!(
        payload["sensitivity"]["disposition"],
        "insufficient_evidence"
    );
    assert_eq!(
        payload["sensitivity"]["dispatch"],
        "not_dispatched_sensitivity_only"
    );
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn multistudy_concordance_tool_dispatches_empty_sources_as_insufficient() {
    let mut server = initialized_server();
    let result = call_tool(
        &mut server,
        "glioma_multistudy_concordance_analyze",
        json!({
            "request": {
                "objective": "route contract smoke test",
                "estimand_id": "viability-change-at-24h",
                "effect_unit": "normalized-signal-milli",
                "expected_direction": "positive",
                "source_report_order": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
                "min_studies": 2,
                "min_independent_groups": 2,
                "min_supporting_groups": 2,
                "min_quality_milli": 800,
                "effect_threshold_milli": 50,
                "max_interval_gap_milli": 20,
                "min_direction_concordance_milli": 900
            },
            "studies": []
        }),
    );
    assert_eq!(result["result"]["isError"], false);
    let text = result["result"]["content"][0]["text"]
        .as_str()
        .expect("concordance result is JSON text");
    let payload: Value = serde_json::from_str(text).expect("concordance result parses");
    assert_eq!(payload["analysis"]["disposition"], "insufficient_evidence");
    assert_eq!(
        payload["analysis"]["dispatch"],
        "not_dispatched_analysis_only"
    );
    assert_eq!(payload["dispatch"], "not_started");
}

#[test]
fn mcp_dispatch_scanner_can_actually_see_a_planted_route() {
    assert_eq!(
        static_tool_routes("\"scanner_planted_route\" => self.handler(arguments),"),
        ["scanner_planted_route"],
        "a dispatch scanner that cannot see a real arm must not pass"
    );
    assert!(static_tool_routes(
        "let label = \"not_a_route\";\n// \"comment_only\" => ignored,\n\"split_operator\" = > ignored,"
    )
    .is_empty());
}
