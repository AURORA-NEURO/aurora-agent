use bioprism_mcp::{Lifecycle, Request, Server};
use serde_json::{json, Value};
use std::path::PathBuf;

// Keep this integration target relinkable on Windows hosts with transient PDB contention.
// Portfolio coverage intentionally exercises every supported specialty lane.
// Case-aware intake coverage exercises the real observation handoff.
// Keep the target deterministic and read-only for local contract verification.

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn ready(server: &mut Server) {
    let initialize =
        Request::parse(r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#)
            .expect("initialize parses");
    server.handle(&initialize).expect("initialize is answered");
    assert_eq!(server.lifecycle(), Lifecycle::Initialized);
    let notification = Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .expect("initialized notification parses");
    assert!(server.handle(&notification).is_none());
    assert_eq!(server.lifecycle(), Lifecycle::Ready);
}

fn call(server: &mut Server, arguments: Value) -> Value {
    ready(server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "neurosurgery_plan", "arguments": { "request": arguments } }
        })
        .to_string(),
    )
    .expect("call parses");
    let response = server.handle(&request).expect("call is answered").to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("tool returns text content");
    serde_json::from_str(text).expect("tool content is JSON")
}

fn call_with_real_data(server: &mut Server, request: Value, real_data: Value) -> Value {
    ready(server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_plan",
                "arguments": { "request": request, "real_glioma_data": real_data }
            }
        })
        .to_string(),
    )
    .expect("real-data call parses");
    let response = server
        .handle(&rpc)
        .expect("real-data call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("real-data tool returns text content");
    serde_json::from_str(text).expect("real-data tool content is JSON")
}

fn call_with_public_literature(
    server: &mut Server,
    request: Value,
    public_literature: Value,
) -> Value {
    ready(server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_plan",
                "arguments": { "request": request, "public_literature": public_literature }
            }
        })
        .to_string(),
    )
    .expect("public-literature call parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature tool returns text content");
    serde_json::from_str(text).expect("public-literature tool content is JSON")
}

fn intake_call(server: &mut Server, arguments: Value) -> Value {
    ready(server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": "neurosurgery_intake_plan", "arguments": arguments }
        })
        .to_string(),
    )
    .expect("intake call parses");
    let response = server
        .handle(&rpc)
        .expect("intake call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("intake returns text content");
    serde_json::from_str(text).expect("intake content is JSON")
}

fn intake_mission_call(server: &mut Server, arguments: Value) -> Value {
    ready(server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": { "name": "neurosurgery_intake_mission", "arguments": arguments }
        })
        .to_string(),
    )
    .expect("intake mission call parses");
    let response = server
        .handle(&rpc)
        .expect("intake mission call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("intake mission returns text content");
    serde_json::from_str(text).expect("intake mission content is JSON")
}

fn intake_portfolio_call(server: &mut Server, arguments: Value) -> Value {
    ready(server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": { "name": "neurosurgery_intake_portfolio", "arguments": arguments }
        })
        .to_string(),
    )
    .expect("intake portfolio call parses");
    let response = server
        .handle(&rpc)
        .expect("intake portfolio call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("intake portfolio returns text content");
    serde_json::from_str(text).expect("intake portfolio content is JSON")
}

fn session_call(
    server: &mut Server,
    operation: &str,
    request: Value,
    session: Option<Value>,
) -> Value {
    let mut arguments = json!({ "operation": operation, "request": request });
    if let Some(session) = session {
        arguments["session"] = session;
    }
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": { "name": "neurosurgery_session", "arguments": arguments }
        })
        .to_string(),
    )
    .expect("session call parses");
    let response = server
        .handle(&rpc)
        .expect("session call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("session tool returns text content");
    serde_json::from_str(text).expect("session tool content is JSON")
}

fn public_session_call(
    server: &mut Server,
    operation: &str,
    request: Value,
    session: Option<Value>,
    public_literature: Value,
) -> Value {
    let mut arguments = json!({
        "operation": operation,
        "request": request,
        "public_literature": public_literature,
    });
    if let Some(session) = session {
        arguments["session"] = session;
    }
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": { "name": "neurosurgery_session", "arguments": arguments }
        })
        .to_string(),
    )
    .expect("public session call parses");
    let response = server
        .handle(&rpc)
        .expect("public session call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public session returns text content");
    serde_json::from_str(text).expect("public session content is JSON")
}

#[test]
fn mcp_routes_the_local_neurosurgical_agent_without_a_provider() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("synthetic fixture parses");
    let mut server = Server::new(repo_root());
    let response = call(&mut server, request);
    assert_eq!(response["specialty"], json!("glioma"));
    assert_eq!(response["status"], json!("ready_for_human_review"));
    assert_eq!(response["evidence_gaps"], json!([]));
    assert_eq!(response["plan"][0]["capability"], json!("safety_gate"));
    assert_eq!(response["plan"][0]["effect"], json!("read_only"));
    assert_eq!(
        response["plan"].as_array().unwrap().last().unwrap()["capability"],
        json!("human_review_hold")
    );
}

#[test]
fn mcp_advertises_the_specialty_tool() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
        .expect("tools/list parses");
    let response = server
        .handle(&request)
        .expect("tools/list is answered")
        .to_json();
    let tools = &response["result"]["tools"];
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_plan"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_mission"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_evidence_audit"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_specialty_evidence_map"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_evidence_graph"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_coverage"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_cohort_landscape"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_reconciliation"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_diff"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_review_queue"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_review_disposition"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_evidence_packet"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_autonomous_workflow"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_reasoning_context"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_real_data_draft_audit"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_research_plan"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_evidence_acquisition"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_intake_plan"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_intake_mission"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "neurosurgery_intake_portfolio"));
    let mission = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "neurosurgery_mission")
        .expect("mission tool is advertised");
    assert!(mission["inputSchema"]["properties"]["case_dicom_import"].is_object());
    assert!(mission["inputSchema"]["properties"]["case_fhir_import"].is_object());
    assert_eq!(
        mission["inputSchema"]["properties"]["case_asset_manifest_query"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        mission["inputSchema"]["properties"]["case_asset_manifest_query"]["properties"]
            ["max_review_items"]["maximum"],
        json!(512)
    );
    let intake_mission = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "neurosurgery_intake_mission")
        .expect("intake mission tool is advertised");
    assert!(intake_mission["inputSchema"]["properties"]["case_dicom_import"].is_object());
    assert!(intake_mission["inputSchema"]["properties"]["case_fhir_import"].is_object());
    assert_eq!(
        intake_mission["inputSchema"]["properties"]["case_asset_manifest_query"]
            ["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        intake_mission["inputSchema"]["properties"]["freshness"]["properties"]["as_of"]["pattern"],
        json!("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$")
    );
    let intake_portfolio = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "neurosurgery_intake_portfolio")
        .expect("intake portfolio tool is advertised");
    assert_eq!(
        intake_portfolio["inputSchema"]["properties"]["case_asset_manifest_query"]
            ["additionalProperties"],
        json!(false)
    );
    assert_eq!(
        intake_portfolio["inputSchema"]["properties"]["freshness"]["properties"]["max_age_days"]
            ["maximum"],
        json!(3650)
    );
    assert!(
        intake_portfolio["inputSchema"]["properties"]["case_asset_review_disposition"].is_object()
    );
}

#[test]
fn mcp_routes_natural_language_intake_without_retaining_the_question() {
    let mut server = Server::new(repo_root());
    let selected = intake_call(
        &mut server,
        json!({
            "question": "Review MGMT promoter methylation and IDH evidence for a glioblastoma research handoff"
        }),
    );
    assert_eq!(
        selected["schema_version"],
        json!("bioprism-neurosurgery-intake-plan/0.1")
    );
    assert_eq!(selected["selected_specialty"], json!("glioma"));
    assert_eq!(selected["abstained"], json!(false));
    assert_eq!(selected["provider"], json!("none"));
    assert_eq!(selected["network"], json!(false));
    assert_eq!(selected["effect"], json!("read_only"));
    assert_eq!(selected["human_review_required"], json!(true));
    assert!(selected["route"]
        .as_array()
        .unwrap()
        .iter()
        .any(|capability| capability == "safety_gate"));
    assert_eq!(
        selected["evidence_sources"],
        json!(["real_glioma_snapshot", "pubmed_snapshot"])
    );
    assert!(selected.get("question").is_none());
    assert_eq!(selected["question_digest"].as_str().unwrap().len(), 64);

    let mut ambiguous_server = Server::new(repo_root());
    let ambiguous = intake_call(
        &mut ambiguous_server,
        json!({ "question": "What evidence exists for a neural tube defect?" }),
    );
    assert_eq!(ambiguous["selected_specialty"], Value::Null);
    assert_eq!(ambiguous["abstained"], json!(true));
    assert_eq!(ambiguous["reason"], json!("insufficient_margin"));
}

#[test]
fn mcp_composes_real_data_intake_into_a_digest_only_mission() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let mut server = Server::new(repo_root());
    let ready_report = intake_mission_call(
        &mut server,
        json!({
            "question": "Review MGMT promoter methylation and IDH evidence for a glioblastoma research handoff",
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "max_session_steps": 32,
            "freshness": { "as_of": "2027-08-31T00:00:00Z", "max_age_days": 30 }
        }),
    );
    assert_eq!(
        ready_report["schema_version"],
        json!("bioprism-neurosurgery-intake-mission/0.1")
    );
    // The validated snapshot permits execution; route-level observation gaps remain visible in
    // the nested mission for reviewer follow-up.
    assert_eq!(ready_report["status"], json!("ready_for_human_review"));
    assert_eq!(ready_report["provider"], json!("none"));
    assert_eq!(ready_report["network"], json!(false));
    assert_eq!(ready_report["human_review_required"], json!(true));
    assert_eq!(ready_report["effect"], json!("read_only"));
    assert!(ready_report["mission"].is_object());
    assert_eq!(ready_report["mission"]["status"], json!("needs_evidence"));
    assert_eq!(ready_report["request_digest"].as_str().unwrap().len(), 64);
    assert!(ready_report.get("question").is_none());
    assert!(ready_report.get("request").is_none());
    assert_eq!(
        ready_report["mission"]["real_data_freshness"]["query"]["max_age_days"],
        json!(30)
    );
    let filtered_query = ready_report["mission"]["real_data_query"]["query"]["text"]
        .as_str()
        .expect("matched intake terms become a bounded local query");
    assert!(filtered_query.contains("mgmt"));
    assert!(filtered_query.contains("idh"));
    assert!(!filtered_query.contains("research handoff"));

    let case_question = "caller case question stays transient";
    let mut case_server = Server::new(repo_root());
    let case_report = intake_mission_call(
        &mut case_server,
        json!({
            "question": "Route this glioma IDH case through the molecular evidence workflow",
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "max_session_steps": 32,
            "freshness": { "as_of": "2027-08-31T00:00:00Z" },
            "case_request": {
                "schema_version": "bioprism-neurosurgery/0.1",
                "case_id": "case-deidentified-001",
                "specialty": "glioma",
                "request_use": "research_synthesis",
                "question": case_question,
                "observations": [{
                    "kind": "molecular",
                    "label": "IDH1 assay status",
                    "value": "caller-declared result",
                    "status": "observed",
                    "source_id": "pathology-record-1",
                    "observed_at": "2025-01-15T00:00:00Z",
                    "timepoint": "baseline"
                }]
            },
            "case_asset_manifest": {
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": false,
                "assets": [{
                    "asset_id": "deidentified-mri-001",
                    "kind": "imaging_series",
                    "status": "observed",
                    "source_kind": "dicom_archive",
                    "source_id": "archive-study-001",
                    "content_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                }]
            },
            "case_asset_manifest_query": {
                "requested_kinds": ["imaging_series"],
                "max_review_items": 8
            }
        }),
    );
    assert_eq!(case_report["status"], json!("ready_for_human_review"));
    assert_eq!(case_report["mission"]["specialty"], json!("glioma"));
    assert!(
        case_report["mission"]["run"]["response"]["report"]["observed_finding_count"]
            .as_u64()
            .unwrap_or_default()
            >= 1
    );
    assert_eq!(
        case_report["mission"]["case_asset_manifest"]["asset_count"],
        json!(1)
    );
    assert_eq!(
        case_report["mission"]["evidence_synthesis"]["case_asset_report_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        case_report["mission"]["evidence_synthesis"]["case_asset_summary"]["asset_count"],
        json!(1)
    );
    assert_eq!(
        case_report["mission"]["real_data_freshness"]["query"]["max_age_days"],
        json!(365)
    );
    assert!(!serde_json::to_string(&case_report)
        .expect("case report serialises")
        .contains(case_question));

    let mut missing_server = Server::new(repo_root());
    let missing = intake_mission_call(
        &mut missing_server,
        json!({ "question": "Review glioblastoma molecular evidence" }),
    );
    assert_eq!(missing["status"], json!("needs_evidence"));
    assert_eq!(
        missing["required_evidence"],
        json!(["real_glioma_snapshot"])
    );
    assert!(missing["mission"].is_null());

    let mut chiari_server = Server::new(repo_root());
    let chiari_missing = intake_mission_call(
        &mut chiari_server,
        json!({
            "question": "Review Chiari malformation and craniocervical junction evidence"
        }),
    );
    assert_eq!(chiari_missing["status"], json!("needs_evidence"));
    assert_eq!(
        chiari_missing["required_evidence"],
        json!(["pubmed_snapshot"])
    );
    assert!(chiari_missing["mission"].is_null());
}

#[test]
fn mcp_routes_natural_language_intake_with_real_dicom_and_fhir_imports() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_extended_snapshot.json"
    ))
    .expect("extended real glioma snapshot parses");
    let dicom: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM metadata fixture parses");
    let fhir: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/fhir_metadata.json"
    ))
    .expect("FHIR metadata fixture parses");
    let mut server = Server::new(repo_root());
    let report = intake_mission_call(
        &mut server,
        json!({
            "question": "Review glioma imaging and molecular evidence provenance",
            "specialty": "glioma",
            "real_glioma_data": real_data,
            "case_dicom_import": dicom,
            "case_fhir_import": fhir,
            "max_session_steps": 32
        }),
    );
    assert_eq!(report["status"], json!("ready_for_human_review"));
    assert_eq!(report["provider"], json!("none"));
    assert_eq!(report["network"], json!(false));
    assert_eq!(
        report["mission"]["case_asset_manifest"]["asset_count"],
        json!(3)
    );
    assert_eq!(
        report["mission"]["case_dicom_import"]["dataset_count"],
        json!(2)
    );
    assert_eq!(
        report["mission"]["case_fhir_import"]["resource_count"],
        json!(2)
    );
    assert!(
        report["mission"]["run"]["response"]["real_data"]["genomic_project_case_counts"]
            .as_array()
            .expect("project coverage is emitted")
            .iter()
            .any(|row| row["project_id"] == json!("TCGA-LGG"))
    );
}

#[test]
fn mcp_fans_out_an_explicit_all_specialty_portfolio_without_merging_lanes() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let mut server = Server::new(repo_root());
    let report = intake_portfolio_call(
        &mut server,
        json!({
            "question": "Compare evidence gaps across glioma, cranial base, encephalocele, spina bifida, and Chiari research",
            "include_all_specialties": true,
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "max_hits_per_lane": 4,
            "max_review_items_per_lane": 4,
            "max_issues_per_lane": 8,
            "max_session_steps": 16
        }),
    );
    assert_eq!(
        report["schema_version"],
        json!("bioprism-neurosurgery-intake-portfolio/0.1")
    );
    assert_eq!(report["status"], json!("ready_for_human_review"));
    assert_eq!(report["selected_specialties"].as_array().unwrap().len(), 6);
    assert_eq!(report["portfolio"]["specialty_count"], json!(6));
    assert_eq!(report["portfolio"]["provider"], json!("none"));
    assert_eq!(report["portfolio"]["network"], json!(false));
    assert_eq!(report["portfolio"]["synthetic_data"], json!(false));
    assert!(report["mission"].is_null());
    assert!(report.get("question").is_none());

    let mut missing_server = Server::new(repo_root());
    let missing = intake_portfolio_call(
        &mut missing_server,
        json!({ "question": "Review all six neurosurgical evidence lanes", "include_all_specialties": true }),
    );
    assert_eq!(missing["status"], json!("needs_evidence"));
    assert_eq!(
        missing["required_evidence"],
        json!(["pubmed_snapshot", "real_glioma_snapshot"])
    );
}

#[test]
fn mcp_attaches_assets_only_to_a_selected_intake_portfolio_lane() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let manifest = json!({
        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
        "specialty": "glioma",
        "synthetic_data": false,
        "assets": [{
            "asset_id": "deidentified-pathology-portfolio-001",
            "kind": "pathology_report",
            "status": "not_collected",
            "source_kind": "pathology_laboratory"
        }]
    });
    let mut server = Server::new(repo_root());
    let selected = intake_portfolio_call(
        &mut server,
        json!({
            "question": "Review glioma molecular evidence",
            "specialty": "glioma",
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "max_session_steps": 32,
            "case_asset_manifest": manifest.clone(),
            "case_asset_manifest_query": {"requested_kinds": ["pathology_report"]},
            "freshness": { "as_of": "2027-08-31T00:00:00Z", "max_age_days": 14 }
        }),
    );
    assert_eq!(selected["status"], json!("ready_for_human_review"));
    assert_eq!(
        selected["mission"]["case_asset_manifest"]["asset_count"],
        json!(1)
    );
    assert_eq!(
        selected["mission"]["case_asset_manifest"]["non_observed_asset_count"],
        json!(1)
    );
    assert_eq!(
        selected["portfolio"]["freshness"]["query"]["max_age_days"],
        json!(14)
    );

    let mut disposition_server = Server::new(repo_root());
    ready(&mut disposition_server);
    let disposition_rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 142,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_asset_review_disposition",
                "arguments": {
                    "report": selected["mission"]["case_asset_manifest"].clone(),
                    "decisions": []
                }
            }
        })
        .to_string(),
    )
    .expect("portfolio disposition call parses");
    let disposition_response = disposition_server
        .handle(&disposition_rpc)
        .expect("portfolio disposition call is answered")
        .to_json();
    let disposition_text = disposition_response["result"]["content"][0]["text"]
        .as_str()
        .expect("portfolio disposition returns text content");
    let disposition: Value =
        serde_json::from_str(disposition_text).expect("portfolio disposition content is JSON");
    let mut replay_server = Server::new(repo_root());
    let selected_with_disposition = intake_portfolio_call(
        &mut replay_server,
        json!({
            "question": "Review glioma molecular evidence",
            "specialty": "glioma",
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "max_session_steps": 32,
            "case_asset_manifest": manifest.clone(),
            "case_asset_manifest_query": {"requested_kinds": ["pathology_report"]},
            "case_asset_review_disposition": disposition.clone()
        }),
    );
    assert_eq!(
        selected_with_disposition["mission"]["case_asset_review_disposition"]["disposition_digest"],
        disposition["disposition_digest"]
    );

    let mut all_server = Server::new(repo_root());
    let all = intake_portfolio_call(
        &mut all_server,
        json!({
            "question": "Review all neurosurgical evidence lanes",
            "include_all_specialties": true,
            "real_glioma_data": real_data.clone(),
            "public_literature": public_literature.clone(),
            "case_asset_manifest": manifest
        }),
    );
    assert!(
        all.get("error").is_some(),
        "ambiguous asset attachment must refuse"
    );
}

#[test]
fn mcp_audits_granular_specialty_evidence_without_provider_access() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("synthetic fixture parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 14,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_evidence_audit",
                "arguments": { "request": request }
            }
        })
        .to_string(),
    )
    .expect("evidence-audit call parses");
    let response = server
        .handle(&rpc)
        .expect("evidence-audit call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("evidence audit returns text content");
    let value: Value = serde_json::from_str(text).expect("evidence audit content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-evidence-audit/0.1")
    );
    assert_eq!(value["specialty"], json!("glioma"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(!value["missing_required_kinds"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        value["temporal_alignment"]["schema_version"],
        json!("bioprism-neurosurgery-temporal-alignment/0.1")
    );
    assert_eq!(value["temporal_alignment"]["status"], json!("partial"));
}

#[test]
fn mcp_projects_the_specialty_evidence_map_without_provider_access() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("synthetic fixture parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 141,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_specialty_evidence_map",
                "arguments": { "request": request }
            }
        })
        .to_string(),
    )
    .expect("specialty-map call parses");
    let response = server
        .handle(&rpc)
        .expect("specialty-map call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("specialty map returns text content");
    let value: Value = serde_json::from_str(text).expect("specialty map content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-specialty-evidence-map/0.1")
    );
    assert_eq!(value["specialty"], json!("glioma"));
    assert!(value["dimensions"]
        .as_array()
        .is_some_and(|rows| !rows.is_empty()));
    assert!(value["map_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["effect"], json!("read_only"));
}

#[test]
fn mcp_projects_a_deidentified_case_asset_manifest_without_opening_assets() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 140,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_asset_manifest",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-asset-contract",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Inventory real multimodal asset provenance",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "manifest": {
                        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                        "specialty": "glioma",
                        "synthetic_data": false,
                        "direct_identifier_fields": [],
                        "assets": [{
                            "asset_id": "local-dicom-export-1",
                            "kind": "imaging_series",
                            "status": "observed",
                            "source_kind": "dicom_archive",
                            "source_id": "deidentified-archive",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                            "modality": "MR",
                            "body_region": "brain",
                            "observed_at": "2026-01-01T00:00:00Z",
                            "timepoint": "baseline"
                        }]
                    },
                    "query": {
                        "requested_kinds": ["imaging_series", "molecular_assay"],
                        "max_review_items": 16
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("case-asset manifest call parses");
    let response = server
        .handle(&rpc)
        .expect("case-asset manifest call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("case-asset manifest returns text content");
    let value: Value = serde_json::from_str(text).expect("case-asset manifest is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-case-asset-manifest/0.1")
    );
    assert_eq!(value["asset_count"], json!(1));
    assert_eq!(value["missing_requested_kinds"], json!(["molecular_assay"]));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["raw_values_retained"], json!(false));
    assert!(!value["assets"][0]["asset_ref"]
        .as_str()
        .expect("asset digest")
        .is_empty());
    assert!(!text.contains("local-dicom-export-1"));
    assert!(!text.contains("deidentified-archive"));

    let sequence = value["review_items"][0]["sequence"]
        .as_u64()
        .expect("review item sequence is numeric");
    let disposition_rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 141,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_asset_review_disposition",
                "arguments": {
                    "report": value.clone(),
                    "decisions": [{
                        "sequence": sequence,
                        "disposition": "reviewed",
                        "reviewer_id": "clinician-mcp"
                    }]
                }
            }
        })
        .to_string(),
    )
    .expect("case-asset disposition call parses");
    let disposition_response = server
        .handle(&disposition_rpc)
        .expect("case-asset disposition call is answered")
        .to_json();
    let disposition_text = disposition_response["result"]["content"][0]["text"]
        .as_str()
        .expect("case-asset disposition returns text content");
    let disposition: Value =
        serde_json::from_str(disposition_text).expect("case-asset disposition content is JSON");
    assert_eq!(disposition["submitted_decision_count"], json!(1));
    assert_eq!(disposition["resolved_decision_count"], json!(1));
    assert_eq!(disposition["report_digest"], value["report_digest"]);
    assert!(!disposition_text.contains("local-dicom-export-1"));
}

#[test]
fn mcp_imports_sanitized_fhir_metadata_without_echoing_bundle_values() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 142,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_fhir_import",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-fhir-contract",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Inventory sanitized FHIR asset metadata",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "import": {
                        "schema_version": "bioprism-neurosurgery-case-fhir-import/0.1",
                        "specialty": "glioma",
                        "deidentified": true,
                        "synthetic_data": false,
                        "source_id": "mcp-fhir-export",
                        "bundle": {
                            "resourceType": "Bundle",
                            "type": "collection",
                            "entry": [{"resource": {"resourceType": "ImagingStudy", "id": "img-mcp"}}]
                        },
                        "resource_hints": [{
                            "resource_id": "img-mcp",
                            "asset_kind": "imaging_series",
                            "status": "observed",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        }]
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("FHIR import call parses");
    let response = server
        .handle(&rpc)
        .expect("FHIR import call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("FHIR import returns text content");
    let value: Value = serde_json::from_str(text).expect("FHIR import content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-case-fhir-import/0.1")
    );
    assert_eq!(value["resource_count"], json!(1));
    assert_eq!(value["projected_asset_count"], json!(1));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(!text.contains("mcp-fhir-export"));
    assert!(!text.contains("img-mcp"));
}

#[test]
fn mcp_imports_deidentified_dicom_metadata_without_echoing_source_values() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let import: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM fixture parses");
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 143,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_dicom_import",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-dicom-contract",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Inventory DICOM series metadata",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "import": import
                }
            }
        })
        .to_string(),
    )
    .expect("DICOM import call parses");
    let response = server
        .handle(&rpc)
        .expect("DICOM import call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("DICOM import returns text content");
    let value: Value = serde_json::from_str(text).expect("DICOM import content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-case-dicom-import/0.1")
    );
    assert_eq!(value["dataset_count"], json!(2));
    assert_eq!(value["projected_series_count"], json!(2));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
    assert!(!text.contains("deidentified-dicom-metadata-export-001"));
    assert!(!text.contains("2.25.300000000000000000000000000000001"));
}

#[test]
fn mcp_composes_dicom_metadata_with_real_evidence_workers() {
    let import: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM fixture parses");
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 144,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_case_dicom_evidence_workflow",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-dicom-workflow",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Bind DICOM metadata to real glioma evidence",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "import": import,
                    "real_glioma_data": real_glioma_data,
                    "query": {
                        "max_synthesis_references": 16,
                        "real_data_reasoning_context": { "max_chars": 10000 }
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("DICOM workflow call parses");
    let response = server
        .handle(&rpc)
        .expect("DICOM workflow call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("DICOM workflow returns text content");
    let value: Value = serde_json::from_str(text).expect("DICOM workflow content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-case-dicom-evidence-workflow/0.1")
    );
    assert_eq!(value["specialty"], json!("glioma"));
    assert_eq!(value["dicom_import"]["projected_series_count"], json!(2));
    assert_eq!(
        value["evidence_synthesis"]["case_asset_summary"]["report_digest"],
        value["dicom_import"]["manifest_report"]["report_digest"]
    );
    assert_eq!(value["evidence_acquisition"]["provider"], json!("none"));
    assert_eq!(
        value["evidence_acquisition_session"]["plan_digest"],
        value["evidence_acquisition"]["plan_digest"]
    );
    assert_eq!(
        value["real_data_reasoning_context"]["bundle_digest"],
        value["real_data_digest"]
    );
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
    assert!(!text.contains("deidentified-dicom-metadata-export-001"));
}

#[test]
fn mcp_composes_dicom_metadata_inside_the_research_mission_envelope() {
    let import: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM fixture parses");
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 145,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-dicom-mission",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Bind DICOM metadata into the autonomous glioma mission",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "real_glioma_data": real_glioma_data,
                    "case_dicom_import": import,
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("mission DICOM call parses");
    let response = server
        .handle(&rpc)
        .expect("mission DICOM call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("mission DICOM returns text content");
    let value: Value = serde_json::from_str(text).expect("mission DICOM content is JSON");
    assert_eq!(
        value["case_dicom_import"]["projected_series_count"],
        json!(2)
    );
    assert_eq!(
        value["case_dicom_import"]["manifest_report"],
        value["case_asset_manifest"]
    );
    assert_eq!(
        value["evidence_synthesis"]["case_asset_summary"]["report_digest"],
        value["case_asset_manifest"]["report_digest"]
    );
    assert_eq!(value["mission_audit"]["fail_count"], json!(0));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
    assert!(!text.contains("deidentified-dicom-metadata-export-001"));
}

#[test]
fn mcp_composes_fhir_metadata_inside_the_research_mission_envelope() {
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 146,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-fhir-mission",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Bind FHIR metadata into the autonomous glioma mission",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "real_glioma_data": real_glioma_data,
                    "case_fhir_import": {
                        "schema_version": "bioprism-neurosurgery-case-fhir-import/0.1",
                        "specialty": "glioma",
                        "deidentified": true,
                        "synthetic_data": false,
                        "source_id": "mcp-fhir-export",
                        "bundle": {
                            "resourceType": "Bundle",
                            "type": "collection",
                            "entry": [
                                { "resource": {
                                    "resourceType": "ImagingStudy",
                                    "id": "img-1",
                                    "extension": [{
                                        "url": "https://aurora-neuro.dev/fhir/StructureDefinition/case-asset-kind",
                                        "valueCode": "imaging_series"
                                    }],
                                    "status": "available"
                                }},
                                { "resource": { "resourceType": "Observation", "id": "obs-1" } }
                            ]
                        },
                        "resource_hints": [{
                            "resource_id": "img-1",
                            "asset_kind": "imaging_series",
                            "status": "observed",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                            "modality": "MR",
                            "body_region": "brain",
                            "observed_at": "2026-01-01T00:00:00Z",
                            "timepoint": "baseline"
                        }],
                        "query": { "requested_kinds": ["imaging_series"], "max_review_items": 32 }
                    },
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("mission FHIR call parses");
    let response = server
        .handle(&rpc)
        .expect("mission FHIR call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("mission FHIR returns text content");
    let value: Value = serde_json::from_str(text).expect("mission FHIR content is JSON");
    assert_eq!(value["case_fhir_import"]["resource_count"], json!(2));
    assert_eq!(
        value["case_fhir_import"]["manifest_report"],
        value["case_asset_manifest"]
    );
    assert_eq!(
        value["evidence_synthesis"]["case_asset_summary"]["report_digest"],
        value["case_asset_manifest"]["report_digest"]
    );
    assert_eq!(value["mission_audit"]["fail_count"], json!(0));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
    assert!(!text.contains("mcp-fhir-export"));
}

#[test]
fn mcp_composes_dicom_and_fhir_metadata_into_one_multimodal_mission() {
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let dicom_import: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/dicom_metadata.json"
    ))
    .expect("DICOM metadata fixture parses");
    let fhir_import: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/fhir_metadata.json"
    ))
    .expect("FHIR metadata fixture parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 147,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": {
                        "schema_version": "bioprism-neurosurgery/0.1",
                        "case_id": "deidentified-mcp-multimodal-mission",
                        "specialty": "glioma",
                        "request_use": "research_synthesis",
                        "question": "Compose DICOM and FHIR metadata without opening case bytes",
                        "direct_identifier_fields": [],
                        "observations": [],
                        "evidence": [],
                        "requested_tools": []
                    },
                    "real_glioma_data": real_glioma_data,
                    "case_dicom_import": dicom_import,
                    "case_fhir_import": fhir_import,
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("multimodal mission call parses");
    let response = server
        .handle(&rpc)
        .expect("multimodal mission call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("multimodal mission returns text content");
    let value: Value = serde_json::from_str(text).expect("multimodal mission content is JSON");
    assert_eq!(value["case_asset_manifest"]["asset_count"], json!(3));
    assert_eq!(
        value["case_dicom_import"]["projected_series_count"],
        json!(2)
    );
    assert_eq!(value["case_fhir_import"]["projected_asset_count"], json!(1));
    assert_eq!(
        value["evidence_synthesis"]["case_asset_summary"]["report_digest"],
        value["case_asset_manifest"]["report_digest"]
    );
    assert_eq!(value["mission_audit"]["fail_count"], json!(0));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_composes_case_aware_evidence_synthesis_without_echoing_case_text() {
    let request = json!({
        "case_id": "mcp-evidence-synthesis-glioma",
        "specialty": "glioma",
        "request_use": "research_synthesis",
        "question": "Align real glioma population evidence with this case",
        "observations": [{
            "kind": "imaging",
            "label": "private synthesis label",
            "value": "private synthesis value",
            "source_id": "caller-source",
            "observed_at": "2026-01-01T00:00:00Z"
        }]
    });
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 141,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_evidence_synthesis",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_glioma_data,
                    "public_literature": public_literature,
                    "case_asset_manifest": {
                        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                        "specialty": "glioma",
                        "synthetic_data": false,
                        "direct_identifier_fields": [],
                        "assets": [{
                            "asset_id": "synthesis-local-mri",
                            "kind": "imaging_series",
                            "status": "observed",
                            "source_kind": "dicom_archive",
                            "source_id": "synthesis-dicom-source",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        }]
                    },
                    "case_asset_manifest_query": { "requested_kinds": ["imaging_series"], "max_review_items": 8 },
                    "query": { "max_references": 128 }
                }
            }
        })
        .to_string(),
    )
    .expect("synthesis call parses");
    let response = server
        .handle(&rpc)
        .expect("synthesis call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("synthesis returns text content");
    let value: Value = serde_json::from_str(text).expect("synthesis content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-evidence-synthesis/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["literature_link_audit"].is_object());
    assert!(value["case_asset_report_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
    assert_eq!(value["case_asset_summary"]["asset_count"], json!(1));
    assert_eq!(
        value["case_asset_summary"]["missing_requested_kinds"],
        json!([])
    );
    assert_eq!(
        value["case_asset_review_items"].as_array().unwrap().len(),
        value["case_asset_summary"]["review_item_count"]
            .as_u64()
            .unwrap() as usize
    );
    assert!(value["references"]
        .as_array()
        .expect("references are an array")
        .iter()
        .any(|reference| reference["plane"] == json!("real_glioma_population")));
    let encoded = value.to_string();
    assert!(!encoded.contains("private synthesis label"));
    assert!(!encoded.contains("private synthesis value"));
    assert!(!encoded.contains("synthesis-local-mri"));
    assert!(!encoded.contains("synthesis-dicom-source"));
}

#[test]
fn mcp_maps_typed_glioma_markers_to_real_snapshot_references() {
    let request = json!({
        "case_id": "mcp-molecular-map-glioma",
        "specialty": "glioma",
        "request_use": "research_synthesis",
        "question": "Map typed marker coverage to public records",
        "glioma_molecular": {
            "schema_version": "bioprism-neurosurgery-glioma-molecular/0.1",
            "observations": [{
                "marker": "idh1_mutation",
                "state": "present",
                "assay": "research-panel",
                "specimen": "tumour tissue",
                "source_id": "caller-source",
                "observed_at": "2026-01-01T00:00:00Z"
            }, {
                "marker": "mgmt_promoter_methylation",
                "state": "not_collected"
            }]
        }
    });
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 142,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_glioma_molecular_map",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_glioma_data,
                    "public_literature": public_literature,
                    "query": {
                        "markers": ["idh1_mutation", "mgmt_promoter_methylation"],
                        "real_data_query": { "limit": 2 },
                        "public_literature_query": { "specialty": "glioma", "limit": 2 },
                        "max_hits_per_marker": 2,
                        "max_references": 16
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("molecular map call parses");
    let response = server
        .handle(&rpc)
        .expect("molecular map call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("molecular map returns text content");
    let value: Value = serde_json::from_str(text).expect("molecular map content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-glioma-molecular-map/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(value["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reference| { reference["plane"] == json!("real_glioma_population") }));
    assert!(value["references"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reference| { reference["plane"] == json!("public_literature") }));
    assert!(value["review_items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| { item["code"] == json!("marker_not_collected") }));
}

#[test]
fn mcp_compiles_a_source_linked_research_plan_without_provider_access() {
    let request = json!({
        "case_id": "mcp-research-plan-encephalocele",
        "specialty": "encephalocele",
        "request_use": "research_synthesis",
        "question": "Which evidence should a reviewer inspect next?"
    });
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 15,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_research_plan",
                "arguments": {
                    "request": request,
                    "public_literature": public_literature,
                    "max_tasks": 8,
                    "max_references_per_task": 2
                }
            }
        })
        .to_string(),
    )
    .expect("research-plan call parses");
    let response = server
        .handle(&rpc)
        .expect("research-plan call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("research plan returns text content");
    let value: Value = serde_json::from_str(text).expect("research plan content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-research-plan/0.1")
    );
    assert_eq!(value["specialty"], json!("encephalocele"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(value["source_query_count"].as_u64().unwrap() > 0);
    assert!(value["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|task| task["source_references"].as_array())
        .flatten()
        .all(|reference| reference["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("https://pubmed.ncbi.nlm.nih.gov/"))));
}

#[test]
fn mcp_compiles_a_bounded_real_data_acquisition_wave_without_provider_access() {
    let request = json!({
        "case_id": "mcp-acquisition-glioma",
        "specialty": "glioma",
        "request_use": "research_synthesis",
        "question": "Which real evidence should be reviewed next?",
        "observations": [{
            "kind": "imaging",
            "label": "MRI interpretation",
            "status": "uninterpretable",
            "value": "MRI description requires specialist review"
        }]
    });
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 16,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_evidence_acquisition",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_glioma_data,
                    "public_literature": public_literature,
                    "case_asset_manifest": {
                        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                        "specialty": "glioma",
                        "synthetic_data": false,
                        "direct_identifier_fields": [],
                        "assets": [{
                            "asset_id": "mcp-acquisition-mri",
                            "kind": "imaging_series",
                            "status": "observed",
                            "source_kind": "dicom_archive",
                            "source_id": "mcp-dicom",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                            "modality": "MR",
                            "body_region": "brain"
                        }]
                    },
                    "case_asset_manifest_query": { "requested_kinds": ["imaging_series"] },
                    "query": {
                        "max_steps": 8,
                        "max_references_per_step": 2,
                        "freshness": {
                            "as_of": "2026-08-30T00:00:00Z",
                            "max_age_days": 365
                        }
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("acquisition call parses");
    let response = server
        .handle(&rpc)
        .expect("acquisition call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("acquisition returns text content");
    let value: Value = serde_json::from_str(text).expect("acquisition content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-evidence-acquisition/0.1")
    );
    assert_eq!(value["specialty"], json!("glioma"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(value["case_asset_report_digest"].as_str().is_some());
    assert!(value["case_asset_review_items"].is_array());
    assert!(value["source_query_count"].as_u64().unwrap() > 0);
    assert!(value["steps"].as_array().unwrap().iter().all(|step| {
        step["references"]
            .as_array()
            .unwrap()
            .iter()
            .all(|reference| reference["uri"].as_str().is_some())
    }));
}

#[test]
fn mcp_runs_digest_bound_acquisition_start_advance_finish_without_provider_access() {
    let request = json!({
        "case_id": "mcp-acquisition-session-glioma",
        "specialty": "glioma",
        "request_use": "research_synthesis",
        "question": "Which real evidence should be reviewed next?",
        "observations": [{
            "kind": "imaging",
            "label": "MRI interpretation",
            "status": "uninterpretable",
            "value": "MRI description requires specialist review"
        }]
    });
    let real_glioma_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("public literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);

    let call = |server: &mut Server, id: u64, operation: &str, session: Option<Value>| {
        let mut arguments = json!({
            "operation": operation,
            "request": request,
            "real_glioma_data": real_glioma_data,
            "public_literature": public_literature,
            "query": { "max_steps": 2, "max_references_per_step": 1 }
        });
        if let Some(session) = session {
            arguments["session"] = session;
        }
        let rpc = Request::parse(
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": "neurosurgery_evidence_acquisition",
                    "arguments": arguments
                }
            })
            .to_string(),
        )
        .expect("acquisition lifecycle call parses");
        let response = server
            .handle(&rpc)
            .expect("acquisition lifecycle call is answered")
            .to_json();
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("acquisition lifecycle returns text content");
        serde_json::from_str::<Value>(text).expect("acquisition lifecycle content is JSON")
    };

    let started = call(&mut server, 17, "start", None);
    assert_eq!(
        started["schema_version"],
        json!("bioprism-neurosurgery-evidence-acquisition-session/0.1")
    );
    assert_eq!(started["session"]["status"], json!("planned"));
    let first_session = started["session"].clone();
    let advanced = call(&mut server, 18, "advance", Some(first_session));
    assert_eq!(advanced["steps_executed"], json!(1));
    assert_eq!(advanced["network"], json!(false));
    let advanced = call(
        &mut server,
        19,
        "advance",
        Some(advanced["session"].clone()),
    );
    assert_eq!(advanced["complete"], json!(true));
    let finished = call(&mut server, 20, "finish", Some(advanced["session"].clone()));
    assert_eq!(
        finished["schema_version"],
        json!("bioprism-neurosurgery-evidence-acquisition-execution/0.1")
    );
    assert_eq!(finished["provider"], json!("none"));
}

#[test]
fn mcp_exposes_specialty_profiles_before_execution() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": { "name": "neurosurgery_catalogue", "arguments": {} }
        })
        .to_string(),
    )
    .expect("catalogue call parses");
    let response = server
        .handle(&request)
        .expect("catalogue call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("catalogue returns text content");
    let value: Value = serde_json::from_str(text).expect("catalogue content is JSON");
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["specialties"].as_array().unwrap().len(), 6);
    assert_eq!(value["tools"].as_array().unwrap().len(), 16);
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_evidence_graph"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_specialty_evidence_map"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_case_dicom_import"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_coverage"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_cohort_landscape"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_reconciliation"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_diff"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_review_queue"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_review_disposition"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_evidence_packet"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_autonomous_workflow"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_draft_audit"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_trial_landscape"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_real_data_molecular_coverage"));
    assert!(value["standalone_tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool == "neurosurgery_case_asset_review_disposition"));
}

#[test]
fn mcp_can_run_a_real_public_glioma_bundle_without_a_provider() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    let response = call_with_real_data(&mut server, request, real_data);
    assert_eq!(response["specialty"], json!("glioma"));
    assert_eq!(response["real_data"]["provenance_bound"], json!(true));
    assert_eq!(response["real_data"]["synthetic_data"], json!(false));
    assert_eq!(response["real_data"]["clinical_trial_count"], json!(5));
    assert!(response["tool_runs"].as_array().unwrap().iter().any(|run| {
        run["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == json!("real_data_provenance"))
    }));
}

#[test]
fn mcp_queries_real_public_records_without_fetching_or_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_query",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {"text": "enzastaurin", "status": "completed", "limit": 4}
                }
            }
        })
        .to_string(),
    )
    .expect("real-data query parses");
    let response = server
        .handle(&request)
        .expect("real-data query is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("query returns text content");
    let value: Value = serde_json::from_str(text).expect("query content is JSON");
    assert_eq!(value["total_matches"], json!(1));
    assert_eq!(value["hits"][0]["record_id"], json!("NCT00402116"));
    assert!(value["hits"][0]["source_uri"]
        .as_str()
        .unwrap()
        .starts_with("https://clinicaltrials.gov/"));

    let facet_request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_query",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "record_kind": "clinical_trial",
                        "trial_phase": "phase2",
                        "trial_study_type": "interventional",
                        "trial_updated_from": "2023-01-01",
                        "trial_updated_to": "2024-12-31",
                        "limit": 4
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("registry facet query parses");
    let facet_response = server
        .handle(&facet_request)
        .expect("registry facet query is answered")
        .to_json();
    let facet_text = facet_response["result"]["content"][0]["text"]
        .as_str()
        .expect("facet query returns text content");
    let facet_value: Value = serde_json::from_str(facet_text).expect("facet content is JSON");
    assert_eq!(facet_value["total_matches"], json!(2));
    assert_eq!(
        facet_value["hits"]
            .as_array()
            .expect("facet hits are an array")
            .iter()
            .map(|hit| hit["record_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["NCT01933815", "NCT04915404"]
    );
}

#[test]
fn mcp_queries_real_gdc_data_type_facets_without_fetching() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_extended_snapshot.json"
    ))
    .expect("extended real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 61,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_query",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {"genomic_data_type": "annotated somatic mutation", "limit": 16}
                }
            }
        })
        .to_string(),
    )
    .expect("GDC facet query parses");
    let response = server
        .handle(&request)
        .expect("GDC facet query is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("GDC facet query returns text content");
    let value: Value = serde_json::from_str(text).expect("GDC facet query content is JSON");
    assert_eq!(value["total_matches"], json!(2));
    assert!(value["hits"].as_array().unwrap().iter().all(|hit| {
        hit["record_kind"] == json!("genomic_project")
            && hit["genomic_data_type_counts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|facet| facet["data_type"] == json!("Annotated Somatic Mutation"))
    }));
}

#[test]
fn mcp_projects_a_real_trial_landscape_without_fetching_or_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_trial_landscape",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "query": {
                            "trial_phase": "phase2",
                            "trial_updated_from": "2023-01-01",
                            "trial_updated_to": "2024-12-31"
                        },
                        "max_interventions": 8
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("trial landscape call parses");
    let response = server
        .handle(&request)
        .expect("trial landscape call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("trial landscape returns text content");
    let value: Value = serde_json::from_str(text).expect("trial landscape content is JSON");
    assert_eq!(value["total_matching_trials"], json!(2));
    assert_eq!(value["returned_trial_count"], json!(2));
    assert_eq!(value["phase_annotated_trial_count"], json!(2));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_projects_a_real_molecular_coverage_ledger_without_fetching_or_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_molecular_coverage",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "query": {
                            "molecular_alteration_type": "mutation_extended",
                            "molecular_datatype": "maf",
                            "limit": 128
                        },
                        "max_studies": 8
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("molecular coverage call parses");
    let response = server
        .handle(&request)
        .expect("molecular coverage call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("molecular coverage returns text content");
    let value: Value = serde_json::from_str(text).expect("molecular coverage content is JSON");
    assert_eq!(value["total_matching_profile_count"], json!(6));
    assert_eq!(value["returned_profile_count"], json!(6));
    assert_eq!(value["emitted_profile_count"], json!(6));
    assert_eq!(value["patient_level_profile_count"], json!(0));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_projects_an_auditable_real_data_evidence_graph_without_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 16,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_evidence_graph",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "root_record_id": "24120142",
                        "root_record_kind": "literature_article",
                        "max_nodes": 16,
                        "max_edges": 32
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("evidence-graph call parses");
    let response = server
        .handle(&rpc)
        .expect("evidence-graph call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("evidence graph returns text content");
    let value: Value = serde_json::from_str(text).expect("evidence graph content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-evidence-graph/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["root_count"], json!(1));
    assert!(value["nodes"].as_array().unwrap().iter().any(|node| {
        node["record_kind"] == json!("portal_study")
            && node["record_id"] == json!("gbm_tcga_pub2013")
    }));
    assert!(value["graph_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_audits_real_data_coverage_without_scoring_or_fetching() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 17,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_coverage",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": { "record_kind": "clinical_trial", "from_year": 2020, "to_year": 2025 }
                }
            }
        })
        .to_string(),
    )
    .expect("real-data coverage call parses");
    let response = server
        .handle(&rpc)
        .expect("real-data coverage call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("real-data coverage returns text content");
    let value: Value = serde_json::from_str(text).expect("real-data coverage content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-coverage/0.1")
    );
    assert_eq!(value["matched_record_count"], json!(4));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["gaps"]
        .as_array()
        .is_some_and(|gaps| !gaps.is_empty()));
    assert!(value["coverage_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_compares_real_genomic_projects_without_opening_files() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_extended_snapshot.json"
    ))
    .expect("extended real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 171,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_cohort_landscape",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": { "max_projects": 8, "query": { "genomic_data_type": "Aligned Reads", "limit": 8 } }
                }
            }
        })
        .to_string(),
    )
    .expect("cohort landscape call parses");
    let response = server
        .handle(&rpc)
        .expect("cohort landscape call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("cohort landscape returns text content");
    let value: Value = serde_json::from_str(text).expect("cohort landscape content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-cohort-landscape/0.1")
    );
    assert_eq!(value["returned_project_count"], json!(2));
    assert_eq!(value["total_released_case_inventory"], json!(1133));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["landscape_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_reconciles_real_data_identifiers_without_fetching_or_merging() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 172,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_reconciliation",
                "arguments": { "real_glioma_data": real_data, "query": { "max_issues": 8 } }
            }
        })
        .to_string(),
    )
    .expect("reconciliation call parses");
    let response = server
        .handle(&rpc)
        .expect("reconciliation call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("reconciliation returns text content");
    let value: Value = serde_json::from_str(text).expect("reconciliation content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-reconciliation/0.1")
    );
    assert_eq!(value["candidate_issue_count"], json!(0));
    assert_eq!(value["requires_review"], json!(false));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
}

#[test]
fn mcp_audits_real_data_freshness_with_an_explicit_clock() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 171,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_freshness",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": { "as_of": "2027-08-31T00:00:00Z", "max_age_days": 30 }
                }
            }
        })
        .to_string(),
    )
    .expect("real-data freshness call parses");
    let response = server
        .handle(&rpc)
        .expect("real-data freshness call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("real-data freshness returns text content");
    let value: Value = serde_json::from_str(text).expect("real-data freshness content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-freshness/0.1")
    );
    assert_eq!(value["status"], json!("stale"));
    assert_eq!(value["source_count"], json!(5));
    assert_eq!(value["stale_source_count"], json!(5));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert!(value["freshness_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_compares_two_validated_real_snapshots_without_a_provider() {
    let snapshot: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 18,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_diff",
                "arguments": {
                    "before_real_glioma_data": snapshot,
                    "after_real_glioma_data": snapshot,
                    "query": { "max_changes": 8 }
                }
            }
        })
        .to_string(),
    )
    .expect("real-data diff call parses");
    let response = server
        .handle(&rpc)
        .expect("real-data diff call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("real-data diff returns text content");
    let value: Value = serde_json::from_str(text).expect("real-data diff content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-diff/0.1")
    );
    assert_eq!(value["before_record_count"], json!(88));
    assert_eq!(value["after_record_count"], json!(88));
    assert_eq!(value["total_change_count"], json!(0));
    assert_eq!(value["returned_change_count"], json!(0));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["diff_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_reconciles_two_real_snapshots_without_accepting_a_refresh() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let snapshot: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 19,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_refresh_audit",
                "arguments": {
                    "request": request,
                    "before_real_glioma_data": snapshot,
                    "after_real_glioma_data": snapshot,
                    "query": {
                        "brief": {
                            "focus_terms": ["MGMT"],
                            "include_abstracts": true,
                            "freshness": {"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30}
                        }
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("refresh-audit call parses");
    let response = server
        .handle(&rpc)
        .expect("refresh-audit call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("refresh-audit returns text content");
    let value: Value = serde_json::from_str(text).expect("refresh-audit content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-refresh-audit/0.1")
    );
    assert_eq!(value["structural_change_detected"], json!(false));
    assert_eq!(value["source_identity_stable"], json!(true));
    assert_eq!(value["record_identity_stable"], json!(true));
    assert_eq!(value["research_brief"]["source"], json!("real_glioma"));
    assert_eq!(value["freshness"]["status"], json!("stale"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["audit_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_derives_a_real_data_review_queue_without_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 19,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_review_queue",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": { "max_items": 2 }
                }
            }
        })
        .to_string(),
    )
    .expect("review queue call parses");
    let response = server
        .handle(&rpc)
        .expect("review queue call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("review queue returns text content");
    let value: Value = serde_json::from_str(text).expect("review queue content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-review-queue/0.1")
    );
    assert_eq!(value["candidate_item_count"], json!(15));
    assert_eq!(value["returned_item_count"], json!(2));
    assert_eq!(value["omitted_item_count"], json!(13));
    assert_eq!(value["truncated"], json!(true));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["queue_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_applies_digest_bound_real_data_review_dispositions() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let queue_rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_review_queue",
                "arguments": { "real_glioma_data": real_data, "query": { "max_items": 2 } }
            }
        })
        .to_string(),
    )
    .expect("review queue call parses");
    let queue_response = server
        .handle(&queue_rpc)
        .expect("review queue call is answered")
        .to_json();
    let queue_text = queue_response["result"]["content"][0]["text"]
        .as_str()
        .expect("queue returns text content");
    let queue: Value = serde_json::from_str(queue_text).expect("queue content is JSON");
    let task_id = queue["items"][0]["task_id"].clone();
    let disposition_rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_review_disposition",
                "arguments": {
                    "queue": queue,
                    "decisions": [{ "task_id": task_id, "disposition": "reviewed", "reviewer_id": "mcp-test" }]
                }
            }
        })
        .to_string(),
    )
    .expect("review disposition call parses");
    let response = server
        .handle(&disposition_rpc)
        .expect("review disposition call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("review disposition returns text content");
    let value: Value = serde_json::from_str(text).expect("review disposition content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-review-disposition/0.1")
    );
    assert_eq!(value["submitted_decision_count"], json!(1));
    assert_eq!(value["accepted_decision_count"], json!(1));
    assert_eq!(value["resolved_decision_count"], json!(1));
    assert_eq!(value["pending_item_count"], json!(14));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["disposition_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_composes_a_real_data_evidence_packet_without_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_evidence_packet",
                "arguments": {
                    "real_glioma_data": real_data,
                        "query": {
                            "query": { "text": "glioblastoma", "limit": 4 },
                            "graph": { "max_nodes": 8, "max_edges": 12 },
                            "review_queue": { "max_items": 3 },
                            "freshness": { "as_of": "2027-08-31T00:00:00Z", "max_age_days": 30 }
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("evidence packet call parses");
    let response = server
        .handle(&rpc)
        .expect("evidence packet call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("evidence packet returns text content");
    let value: Value = serde_json::from_str(text).expect("evidence packet content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-evidence-packet/0.4")
    );
    assert_eq!(value["source_count"], json!(5));
    assert_eq!(value["record_count"], json!(88));
    assert_eq!(value["data_query"]["query"]["limit"], json!(4));
    assert_eq!(value["review_queue"]["returned_item_count"], json!(3));
    assert_eq!(value["review_queue"]["omitted_item_count"], json!(12));
    assert_eq!(
        value["cohort_landscape"]["returned_project_count"],
        json!(1)
    );
    assert_eq!(
        value["cohort_landscape"]["project_rows"][0]["project_id"],
        json!("TCGA-GBM")
    );
    assert_eq!(value["freshness"]["status"], json!("stale"));
    assert_eq!(value["graph"]["nodes"].as_array().map(Vec::len), Some(8));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["packet_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_composes_a_real_data_autonomous_review_wave_without_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 223,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_autonomous_workflow",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "packet": { "review_queue": { "max_items": 8 } },
                        "max_actions": 12
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("autonomous-workflow call parses");
    let response = server
        .handle(&rpc)
        .expect("autonomous-workflow call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("autonomous workflow returns text content");
    let value: Value = serde_json::from_str(text).expect("autonomous workflow content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-autonomous-workflow/0.1")
    );
    assert_eq!(value["bundle_digest"], value["packet"]["bundle_digest"]);
    assert_eq!(
        value["packet"]["review_queue"]["omitted_item_count"],
        json!(7)
    );
    assert_eq!(value["state"], json!("needs_snapshot_expansion"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["workflow_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_renders_real_glioma_context_without_invoking_a_provider() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 221,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_reasoning_context",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "packet": { "query": { "text": "glioblastoma", "limit": 2 } },
                        "max_chars": 6000,
                        "include_abstracts": true
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("reasoning-context call parses");
    let response = server
        .handle(&rpc)
        .expect("reasoning-context call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("reasoning context returns text content");
    let value: Value = serde_json::from_str(text).expect("reasoning context content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-reasoning-context/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["context_text"].as_str().is_some_and(|context| context
        .contains("SAFETY_BOUNDARY:")
        && context.contains("<public_record>")));
    let citations = value["citations"]
        .as_array()
        .expect("reasoning context carries a citation index");
    assert_eq!(value["included_citation_count"], json!(citations.len()));
    assert_eq!(citations.len(), 3);
    assert!(citations.iter().any(|citation| {
        citation["record_kind"] == json!("genomic_project")
            && citation["record_id"] == json!("TCGA-GBM")
    }));
    assert_eq!(text.matches("<public_record>").count(), 2);
    assert_eq!(value["omitted_citation_count"], json!(32));
    assert!(value["context_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_audits_local_draft_claims_against_real_packet_records() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_real_data_draft_audit",
                "arguments": {
                    "real_glioma_data": real_data,
                    "query": {
                        "query": { "text": "glioblastoma", "limit": 4 },
                        "graph": { "max_nodes": 8, "max_edges": 12 }
                    },
                    "claims": [
                        {
                            "claim_id": "trial-metadata",
                            "kind": "source_observation",
                            "scope": "public_record_metadata",
                            "text": "The packet contains a public registry record.",
                            "citations": [{ "record_kind": "clinical_trial", "record_id": "NCT00005955" }]
                        },
                        {
                            "claim_id": "unsafe-action",
                            "kind": "clinical_action",
                            "scope": "public_record_metadata",
                            "text": "This action posture must be blocked.",
                            "citations": [{ "record_kind": "clinical_trial", "record_id": "NCT00005955" }]
                        },
                        {
                            "claim_id": "outside-packet",
                            "kind": "source_observation",
                            "scope": "public_record_metadata",
                            "text": "This citation is outside the bounded packet.",
                            "citations": [{ "record_kind": "literature_article", "record_id": "not-emitted" }]
                        }
                    ]
                }
            }
        })
        .to_string(),
    )
    .expect("draft audit call parses");
    let response = server
        .handle(&rpc)
        .expect("draft audit call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("draft audit returns text content");
    let value: Value = serde_json::from_str(text).expect("draft audit content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-real-data-draft-audit/0.1")
    );
    assert_eq!(value["claim_count"], json!(3));
    assert_eq!(value["grounded_claim_count"], json!(1));
    assert_eq!(value["blocked_claim_count"], json!(2));
    assert_eq!(value["status"], json!("blocked"));
    assert_eq!(value["packet"]["data_query"]["query"]["limit"], json!(4));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["draft_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_composes_cross_specialty_literature_packet_and_audits_pmids() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_draft_audit",
                "arguments": {
                    "public_literature": public_literature,
                    "query": { "query": { "specialty": "chiari_malformation", "text": "downbeat nystagmus", "limit": 1 } },
                    "claims": [
                        {
                            "claim_id": "chiari-pmid",
                            "kind": "source_observation",
                            "scope": "citation_metadata",
                            "text": "The bounded packet emits one Chiari PMID.",
                            "citations": [{ "record_kind": "literature_article", "record_id": "42594882" }]
                        },
                        {
                            "claim_id": "unsafe-action",
                            "kind": "clinical_action",
                            "scope": "citation_metadata",
                            "text": "This clinical posture is blocked.",
                            "citations": [{ "record_kind": "literature_article", "record_id": "42594882" }]
                        }
                    ]
                }
            }
        })
        .to_string(),
    )
    .expect("public-literature draft audit parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature draft audit is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature draft audit returns text content");
    let value: Value = serde_json::from_str(text).expect("public-literature draft audit is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-draft-audit/0.1")
    );
    assert_eq!(
        value["packet"]["query_result"]["returned_matches"],
        json!(1)
    );
    assert_eq!(value["grounded_claim_count"], json!(1));
    assert_eq!(value["blocked_claim_count"], json!(1));
    assert_eq!(value["status"], json!("blocked"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
}

#[test]
fn mcp_renders_public_literature_context_without_invoking_a_provider() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 241,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_reasoning_context",
                "arguments": {
                    "public_literature": public_literature,
                    "query": {
                        "packet": { "query": { "specialty": "chiari_malformation", "text": "chiari", "limit": 2 } },
                        "max_chars": 6000,
                        "include_abstracts": true
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("public-literature reasoning-context call parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature reasoning-context call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature reasoning context returns text content");
    let value: Value =
        serde_json::from_str(text).expect("public-literature reasoning context content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-reasoning-context/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["included_citation_count"].as_u64().unwrap() > 0);
    assert!(value["context_text"].as_str().is_some_and(|context| context
        .contains("# AURORA PUBLIC-NEUROSURGICAL LITERATURE REASONING CONTEXT")
        && context.contains("<pubmed_record>")
        && context.contains("pmid:")));
    assert!(value["context_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_fans_out_a_real_literature_matrix_without_cross_lane_inference() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 26,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_matrix",
                "arguments": {
                    "public_literature": public_literature,
                    "query": {
                        "specialties": ["glioma", "chiari_malformation"],
                        "query": { "text": "glioma", "limit": 2 }
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("public-literature matrix parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature matrix is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature matrix returns text content");
    let value: Value = serde_json::from_str(text).expect("public-literature matrix is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-matrix/0.1")
    );
    assert_eq!(value["specialty_count"], json!(2));
    assert_eq!(value["lanes"].as_array().expect("lanes").len(), 2);
    assert_eq!(value["total_returned_count"], json!(2));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["matrix_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_reconciles_identical_real_literature_snapshots_without_a_provider() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 27,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_refresh_audit",
                "arguments": {
                    "before_public_literature": public_literature,
                    "after_public_literature": public_literature,
                    "query": { "max_source_changes": 8, "max_record_changes": 16 }
                }
            }
        })
        .to_string(),
    )
    .expect("public-literature refresh audit parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature refresh audit is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature refresh audit returns text content");
    let value: Value = serde_json::from_str(text).expect("public-literature refresh audit is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-refresh-audit/0.1")
    );
    assert_eq!(value["diff"]["source_counts"]["added"], json!(0));
    assert_eq!(value["diff"]["record_counts"]["changed"], json!(0));
    assert_eq!(value["matrix"]["specialty_count"], json!(6));
    assert_eq!(value["source_identity_stable"], json!(true));
    assert_eq!(value["record_identity_stable"], json!(true));
    assert_eq!(value["requires_refresh_review"], json!(false));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["audit_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_links_real_glioma_and_public_literature_by_exact_identifiers() {
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 28,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_literature_link_audit",
                "arguments": {
                    "real_glioma_data": real_data,
                    "public_literature": public_literature,
                    "query": { "max_links": 16, "max_unmatched_ids": 32 }
                }
            }
        })
        .to_string(),
    )
    .expect("literature link audit parses");
    let response = server
        .handle(&rpc)
        .expect("literature link audit is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("literature link audit returns text content");
    let value: Value = serde_json::from_str(text).expect("literature link audit is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-literature-link-audit/0.1")
    );
    assert_eq!(value["counts"]["real_literature_records"], json!(20));
    assert_eq!(
        value["counts"]["selected_public_literature_records"],
        json!(25)
    );
    assert_eq!(value["counts"]["linked_real_records"], json!(12));
    assert_eq!(value["counts"]["linked_public_records"], json!(12));
    assert_eq!(value["counts"]["pmid_match_count"], json!(12));
    assert_eq!(value["counts"]["doi_match_count"], json!(12));
    assert_eq!(value["requires_link_review"], json!(true));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert!(value["audit_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_audits_public_literature_missingness_without_quality_inference() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 29,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_integrity_audit",
                "arguments": {
                    "public_literature": public_literature,
                    "query": { "specialties": ["glioma"], "max_issues": 8 }
                }
            }
        })
        .to_string(),
    )
    .expect("literature integrity audit parses");
    let response = server
        .handle(&rpc)
        .expect("literature integrity audit is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("literature integrity audit returns text content");
    let value: Value = serde_json::from_str(text).expect("literature integrity audit is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-integrity-audit/0.1")
    );
    assert_eq!(value["counts"]["selected_record_count"], json!(25));
    assert_eq!(value["counts"]["missing_abstract_count"], json!(0));
    assert_eq!(value["counts"]["empty_mesh_term_count"], json!(3));
    assert_eq!(value["issues"].as_array().expect("issues").len(), 3);
    assert_eq!(value["requires_integrity_review"], json!(true));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
}

#[test]
fn mcp_projects_public_literature_integrity_findings_into_review_tasks() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_review_queue",
                "arguments": {
                    "public_literature": public_literature,
                    "query": { "specialties": ["glioma"], "max_items": 2 }
                }
            }
        })
        .to_string(),
    )
    .expect("literature review queue parses");
    let response = server
        .handle(&rpc)
        .expect("literature review queue is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("literature review queue returns text content");
    let value: Value = serde_json::from_str(text).expect("literature review queue is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-review-queue/0.1")
    );
    assert_eq!(value["query"]["specialties"], json!(["glioma"]));
    assert_eq!(value["candidate_item_count"], json!(3));
    assert_eq!(value["returned_item_count"], json!(2));
    assert_eq!(value["omitted_item_count"], json!(1));
    assert_eq!(value["items"][0]["status"], json!("needs_human_review"));
    assert_eq!(value["items"][0]["source_id"], json!("pubmed_glioma"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
}

#[test]
fn mcp_builds_a_specialty_workbench_from_real_public_literature() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_workbench",
                "arguments": {
                    "public_literature": public_literature,
                    "query": { "specialties": ["glioma", "chiari_malformation"] }
                }
            }
        })
        .to_string(),
    )
    .expect("literature workbench parses");
    let response = server
        .handle(&rpc)
        .expect("literature workbench is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("literature workbench returns text content");
    let value: Value = serde_json::from_str(text).expect("literature workbench is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-workbench/0.1")
    );
    assert_eq!(value["specialty_count"], json!(2));
    assert_eq!(value["non_empty_lane_count"], json!(2));
    assert_eq!(value["total_record_count"], json!(47));
    assert_eq!(value["lanes"][0]["specialty"], json!("glioma"));
    assert_eq!(value["lanes"][0]["record_count"], json!(25));
    assert_eq!(value["lanes"][0]["profile"]["specialty"], json!("glioma"));
    assert!(!value["lanes"][0]["design_strata"]
        .as_array()
        .expect("glioma design strata array")
        .is_empty());
    assert!(value["lanes"][0]["unclassified_design_count"].is_number());
    assert!(value["lanes"][0]["overlapping_design_count"].is_number());
    assert_eq!(value["lanes"][1]["specialty"], json!("chiari_malformation"));
    assert_eq!(value["lanes"][1]["record_count"], json!(22));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_builds_source_grounded_evidence_program_tracks() {
    let request = json!({
        "schema_version": "bioprism-neurosurgery/0.1",
        "case_id": "mcp-evidence-program-glioma",
        "specialty": "glioma",
        "request_use": "research_synthesis",
        "question": "Build a source-grounded glioma review agenda",
        "direct_identifier_fields": [],
        "observations": [],
        "evidence": [],
        "requested_tools": []
    });
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_evidence_program",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "public_literature": public_literature,
                    "case_asset_manifest": {
                        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                        "specialty": "glioma",
                        "synthetic_data": false,
                        "direct_identifier_fields": [],
                        "assets": [{
                            "asset_id": "mcp-real-mri",
                            "kind": "imaging_series",
                            "status": "observed",
                            "source_kind": "dicom_archive",
                            "source_id": "mcp-dicom-archive",
                            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                            "modality": "MR",
                            "body_region": "brain"
                        }]
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("evidence program parses");
    let response = server
        .handle(&rpc)
        .expect("evidence program is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("evidence program returns text content");
    let value: Value = serde_json::from_str(text).expect("evidence program is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-evidence-program/0.1")
    );
    assert_eq!(value["lanes"][0]["specialty"], json!("glioma"));
    assert_eq!(value["lanes"][0]["track_count"], json!(6));
    assert!(value["lanes"][0]["tracks"][0]["observation_coverage"].is_array());
    assert!(value["lanes"][0]["tracks"][0]["missing_observation_kinds"].is_array());
    assert!(value["lanes"][0]["tracks"][0]["asset_coverage"].is_array());
    assert_eq!(
        value["lanes"][0]["tracks"][0]["asset_coverage_complete"],
        json!(false)
    );
    assert!(value["lanes"][0]["tracks"][0]["review_worklist"].is_array());
    assert!(value["reference_count"].as_u64().unwrap() > 0);
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_runs_a_bounded_multi_lane_literature_portfolio_without_a_provider() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_portfolio",
                "arguments": {
                    "public_literature": public_literature,
                    "query": {
                        "specialties": ["glioma", "chiari_malformation"],
                        "max_hits_per_lane": 2,
                        "max_review_items_per_lane": 2,
                        "max_issues_per_lane": 8
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("literature portfolio parses");
    let response = server
        .handle(&rpc)
        .expect("literature portfolio is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("literature portfolio returns text content");
    let value: Value = serde_json::from_str(text).expect("literature portfolio is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-public-literature-portfolio/0.1")
    );
    assert_eq!(value["specialty_count"], json!(2));
    assert_eq!(value["lanes"].as_array().unwrap().len(), 2);
    assert_eq!(value["total_match_count"], json!(47));
    assert_eq!(value["total_returned_count"], json!(4));
    assert_eq!(
        value["lanes"][0]["query_result"]["returned_matches"],
        json!(2)
    );
    assert_eq!(value["lanes"][0]["workbench"]["specialty"], json!("glioma"));
    assert_eq!(
        value["lanes"][0]["review_queue"]["returned_item_count"],
        json!(2)
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
}

#[test]
fn mcp_advertises_cross_specialty_literature_handoff_tools() {
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let request = Request::parse(r#"{"jsonrpc":"2.0","id":25,"method":"tools/list","params":{}}"#)
        .expect("tools/list parses");
    let value = server
        .handle(&request)
        .expect("tools/list is answered")
        .to_json();
    let tools = value["result"]["tools"].as_array().expect("tools array");
    for name in [
        "neurosurgery_public_literature_evidence_packet",
        "neurosurgery_public_literature_reasoning_context",
        "neurosurgery_public_literature_draft_audit",
        "neurosurgery_public_literature_matrix",
        "neurosurgery_public_literature_freshness",
        "neurosurgery_public_literature_refresh_audit",
        "neurosurgery_literature_link_audit",
        "neurosurgery_public_literature_integrity_audit",
        "neurosurgery_public_literature_review_queue",
        "neurosurgery_public_literature_workbench",
        "neurosurgery_public_literature_portfolio",
        "neurosurgery_research_brief",
    ] {
        assert!(tools.iter().any(|tool| tool["name"] == json!(name)));
    }
}

#[test]
fn mcp_builds_a_real_data_research_brief_without_a_provider() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_research_brief",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "query": {
                        "focus_terms": ["MGMT"],
                        "max_topics": 12,
                        "max_records_per_topic": 3,
                        "include_abstracts": true,
                        "freshness": {"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("research brief call parses");
    let response = server
        .handle(&rpc)
        .expect("research brief call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("research brief returns text content");
    let value: Value = serde_json::from_str(text).expect("research brief content is JSON");
    assert_eq!(
        value["schema_version"],
        json!("bioprism-neurosurgery-research-brief/0.1")
    );
    assert_eq!(value["source"], json!("real_glioma"));
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["synthetic_data"], json!(false));
    assert_eq!(value["freshness"]["status"], json!("stale"));
    assert!(value["topics"].as_array().unwrap().iter().any(|topic| {
        topic["topic_id"] == json!("caller_focus")
            && topic["matched_record_count"].as_u64().unwrap_or(0) > 0
    }));
    assert!(value["brief_digest"]
        .as_str()
        .is_some_and(|digest| digest.len() == 64));
}

#[test]
fn mcp_queries_cross_specialty_pubmed_metadata_without_a_provider() {
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_public_literature_query",
                "arguments": {
                    "public_literature": public_literature,
                    "query": { "specialty": "chiari_malformation", "text": "chiari", "limit": 3 }
                }
            }
        })
        .to_string(),
    )
    .expect("public-literature query parses");
    let response = server
        .handle(&rpc)
        .expect("public-literature query is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("public-literature query returns text content");
    let value: Value = serde_json::from_str(text).expect("public-literature query content is JSON");
    assert!(value["total_matches"].as_u64().unwrap() > 0);
    assert!(value["hits"].as_array().unwrap().iter().all(|hit| {
        hit["specialty"] == json!("chiari_malformation")
            && hit["source_uri"]
                .as_str()
                .unwrap()
                .starts_with("https://eutils.ncbi.nlm.nih.gov/")
    }));
}

#[test]
fn mcp_runs_a_non_glioma_route_from_cross_specialty_pubmed_metadata() {
    let mut request: Value = serde_json::json!({
        "case_id": "chiari-public-research",
        "specialty": "chiari_malformation",
        "request_use": "research_synthesis",
        "question": "Which public evidence gaps should a reviewer inspect?"
    });
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    let response = call_with_public_literature(&mut server, request.take(), public_literature);
    assert_eq!(response["specialty"], json!("chiari_malformation"));
    assert_eq!(
        response["public_literature"]["provenance_bound"],
        json!(true)
    );
    assert_eq!(
        response["public_literature"]["synthetic_data"],
        json!(false)
    );
    assert!(response["tool_runs"].as_array().unwrap().iter().any(|run| {
        run["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == json!("public_literature_provenance"))
    }));
}

#[test]
fn mcp_refuses_ambiguous_dual_evidence_bundles() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_plan",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "public_literature": public_literature
                }
            }
        })
        .to_string(),
    )
    .expect("ambiguous call parses");
    let response = server
        .handle(&rpc)
        .expect("ambiguous call is answered")
        .to_json();
    assert_eq!(response["result"]["isError"], json!(true));
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("one evidence bundle"));
}

#[test]
fn mcp_session_is_stateless_and_replayable() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("synthetic fixture parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let mut session = session_call(&mut server, "start", request.clone(), None);
    assert_eq!(session["status"], json!("planned"));
    while session["next_ordinal"].as_u64().unwrap()
        <= session["route"].as_array().unwrap().len() as u64
    {
        session = session_call(&mut server, "advance", request.clone(), Some(session));
    }
    assert_eq!(session["status"], json!("awaiting_human_review"));
    let response = session_call(&mut server, "finish", request, Some(session));
    assert_eq!(response["status"], json!("ready_for_human_review"));
    assert_eq!(response["plan"][0]["capability"], json!("safety_gate"));
}

#[test]
fn mcp_can_run_a_bounded_session_to_the_review_hold_in_one_call() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_session",
                "arguments": {
                    "operation": "run",
                    "request": request,
                    "real_glioma_data": real_data,
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("session run parses");
    let response = server
        .handle(&rpc)
        .expect("session run is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("session run returns text content");
    let value: Value = serde_json::from_str(text).expect("session run content is JSON");
    assert_eq!(value["session"]["status"], json!("awaiting_human_review"));
    assert_eq!(
        value["steps_executed"],
        json!(value["session"]["route"].as_array().unwrap().len())
    );
    assert_eq!(value["response"]["real_data"]["record_count"], json!(88));
}

#[test]
fn mcp_composes_a_provenance_first_neurosurgical_mission() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "query": {"text": "enzastaurin", "limit": 2},
                    "freshness": {"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("mission call parses");
    let response = server
        .handle(&rpc)
        .expect("mission call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("mission returns text content");
    let value: Value = serde_json::from_str(text).expect("mission content is JSON");
    assert_eq!(
        value["schema"],
        json!("bioprism-neurosurgical-research-mission/0.1")
    );
    assert_eq!(value["provider"], json!("none"));
    assert_eq!(value["network"], json!(false));
    assert_eq!(value["human_review_required"], json!(true));
    assert_eq!(value["real_data_query"]["total_matches"], json!(1));
    assert_eq!(
        value["real_data_query"]["hits"][0]["study_type"],
        json!("INTERVENTIONAL")
    );
    assert_eq!(
        value["real_data_query"]["hits"][0]["enrollment_count"],
        json!(72)
    );
    assert_eq!(
        value["run"]["response"]["real_data"]["trial_intervention_count"],
        json!(5)
    );
    assert_eq!(value["real_data_coverage"]["total_record_count"], json!(88));
    assert_eq!(value["real_data_coverage"]["synthetic_data"], json!(false));
    assert_eq!(value["real_data_coverage"]["provider"], json!("none"));
    assert_eq!(
        value["real_data_trial_landscape"]["total_matching_trials"],
        json!(5)
    );
    assert_eq!(
        value["real_data_trial_landscape"]["bundle_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(
        value["real_data_trial_landscape"]["provider"],
        json!("none")
    );
    assert_eq!(
        value["real_data_molecular_coverage"]["total_matching_profile_count"],
        json!(54)
    );
    assert_eq!(
        value["real_data_molecular_coverage"]["bundle_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(
        value["real_data_molecular_coverage"]["provider"],
        json!("none")
    );
    assert_eq!(
        value["real_data_review_queue"]["bundle_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(value["real_data_review_queue"]["provider"], json!("none"));
    assert_eq!(
        value["real_data_evidence_packet"]["bundle_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(
        value["real_data_evidence_packet"]["provider"],
        json!("none")
    );
    assert_eq!(
        value["real_data_autonomous_workflow"]["bundle_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(
        value["real_data_autonomous_workflow"]["provider"],
        json!("none")
    );
    assert_eq!(value["real_data_freshness"]["status"], json!("stale"));
    assert_eq!(
        value["real_data_evidence_graph"]["total_node_count"],
        json!(88)
    );
    assert_eq!(
        value["real_data_evidence_graph"]["specialty"],
        json!("glioma")
    );
    assert_eq!(value["real_data_evidence_graph"]["provider"], json!("none"));
    assert_eq!(value["real_data_evidence_graph"]["network"], json!(false));
    assert_eq!(
        value["real_data_reasoning_context"]["synthetic_data"],
        json!(false)
    );
    assert_eq!(
        value["real_data_reasoning_context"]["network"],
        json!(false)
    );
    assert_eq!(
        value["real_data_reasoning_context"]["human_review_required"],
        json!(true)
    );
    assert!(value["real_data_reasoning_context"]["context_text"]
        .as_str()
        .is_some_and(|text| text.contains("AURORA REAL-GLIOMA REASONING CONTEXT")));
    assert_eq!(
        value["research_plan"]["schema_version"],
        json!("bioprism-neurosurgery-research-plan/0.1")
    );
    assert_eq!(
        value["research_plan"]["real_data_digest"],
        value["real_data_coverage"]["bundle_digest"]
    );
    assert_eq!(value["research_plan"]["provider"], json!("none"));
    assert_eq!(
        value["evidence_program"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-program/0.1")
    );
    assert_eq!(
        value["evidence_program"]["lanes"][0]["track_count"],
        json!(6)
    );
    assert_eq!(
        value["mission_audit"]["schema_version"],
        json!("bioprism-neurosurgery-mission-audit/0.1")
    );
    assert_eq!(value["mission_audit"]["integrity_ok"], json!(true));
    assert_eq!(value["mission_audit"]["fail_count"], json!(0));
    assert_eq!(
        value["research_brief"]["schema_version"],
        json!("bioprism-neurosurgery-research-brief/0.1")
    );
    assert_eq!(value["research_brief"]["source"], json!("real_glioma"));
    assert_eq!(value["research_brief"]["provider"], json!("none"));
    assert_eq!(
        value["run"]["session"]["status"],
        json!("awaiting_human_review")
    );

    let validate_rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "operation": "validate",
                    "request": serde_json::from_str::<Value>(include_str!(
                        "../../../data/neurosurgery/glioma_real_request.json"
                    )).expect("real request reparses"),
                    "mission": value,
                    "real_glioma_data": serde_json::from_str::<Value>(include_str!(
                        "../../../data/neurosurgery/glioma_public_snapshot.json"
                    )).expect("real snapshot reparses")
                }
            }
        })
        .to_string(),
    )
    .expect("mission validation call parses");
    let validate_response = server
        .handle(&validate_rpc)
        .expect("mission validation call is answered")
        .to_json();
    let validate_text = validate_response["result"]["content"][0]["text"]
        .as_str()
        .expect("mission validation returns text content");
    let validate_value: Value =
        serde_json::from_str(validate_text).expect("mission validation content is JSON");
    assert_eq!(validate_value["valid"], json!(true));
}

#[test]
fn mcp_runs_cross_specialty_public_session_and_mission() {
    let request = json!({
        "case_id": "encephalocele-public-research",
        "specialty": "encephalocele",
        "request_use": "research_synthesis",
        "question": "Which public evidence gaps should a reviewer inspect?"
    });
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let mut session = public_session_call(
        &mut server,
        "start",
        request.clone(),
        None,
        public_literature.clone(),
    );
    while session["next_ordinal"].as_u64().unwrap()
        <= session["route"].as_array().unwrap().len() as u64
    {
        session = public_session_call(
            &mut server,
            "advance",
            request.clone(),
            Some(session),
            public_literature.clone(),
        );
    }
    assert_eq!(session["status"], json!("awaiting_human_review"));
    let response = public_session_call(
        &mut server,
        "finish",
        request.clone(),
        Some(session),
        public_literature.clone(),
    );
    assert_eq!(
        response["public_literature"]["provenance_bound"],
        json!(true)
    );

    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": request,
                    "public_literature": public_literature,
                    "query": { "specialty": "encephalocele", "text": "encephalocele", "limit": 2 },
                    "portfolio_query": {
                        "specialties": ["encephalocele", "glioma"],
                        "max_hits_per_lane": 1,
                        "max_review_items_per_lane": 1,
                        "max_issues_per_lane": 1
                    },
                    "freshness": { "as_of": "2027-08-31T00:00:00Z", "max_age_days": 30 },
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("public mission call parses");
    let value = server
        .handle(&rpc)
        .expect("public mission call is answered")
        .to_json();
    let text = value["result"]["content"][0]["text"]
        .as_str()
        .expect("public mission returns text content");
    let mission: Value = serde_json::from_str(text).expect("public mission content is JSON");
    assert_eq!(
        mission["public_literature_query"]["total_matches"],
        json!(21)
    );
    assert_eq!(
        mission["public_literature_portfolio"]["schema_version"],
        json!("bioprism-neurosurgery-public-literature-portfolio/0.1")
    );
    assert_eq!(
        mission["public_literature_portfolio"]["specialty_count"],
        json!(2)
    );
    assert_eq!(
        mission["public_literature_portfolio"]["total_match_count"],
        json!(48)
    );
    assert_eq!(
        mission["public_literature_portfolio"]["provider"],
        json!("none")
    );
    assert_eq!(
        mission["run"]["session"]["status"],
        json!("awaiting_human_review")
    );
    assert_eq!(
        mission["research_plan"]["schema_version"],
        json!("bioprism-neurosurgery-research-plan/0.1")
    );
    assert_eq!(mission["research_plan"]["provider"], json!("none"));
    assert_eq!(
        mission["evidence_program"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-program/0.1")
    );
    assert_eq!(
        mission["mission_audit"]["schema_version"],
        json!("bioprism-neurosurgery-mission-audit/0.1")
    );
    assert_eq!(mission["mission_audit"]["integrity_ok"], json!(true));
    assert_eq!(
        mission["evidence_acquisition"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-acquisition/0.1")
    );
    assert_eq!(mission["evidence_acquisition"]["provider"], json!("none"));
    assert_eq!(mission["evidence_acquisition"]["network"], json!(false));
    assert_eq!(
        mission["evidence_acquisition_session"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-acquisition-session/0.1")
    );
    assert_eq!(
        mission["evidence_acquisition_session"]["next_sequence"],
        json!(1)
    );
    assert_eq!(
        mission["public_literature_evidence_packet"]["bundle_digest"],
        mission["public_literature_freshness"]["bundle_digest"]
    );
    assert_eq!(
        mission["public_literature_evidence_packet"]["provider"],
        json!("none")
    );
    assert_eq!(
        mission["public_literature_integrity_audit"]["schema_version"],
        json!("bioprism-neurosurgery-public-literature-integrity-audit/0.1")
    );
    assert_eq!(
        mission["public_literature_integrity_audit"]["counts"]["selected_record_count"],
        json!(23)
    );
    assert_eq!(
        mission["public_literature_integrity_audit"]["provider"],
        json!("none")
    );
    assert_eq!(
        mission["public_literature_integrity_audit"]["network"],
        json!(false)
    );
    assert_eq!(
        mission["public_literature_review_queue"]["schema_version"],
        json!("bioprism-neurosurgery-public-literature-review-queue/0.1")
    );
    assert_eq!(
        mission["public_literature_review_queue"]["candidate_item_count"],
        json!(13)
    );
    assert_eq!(
        mission["public_literature_review_queue"]["provider"],
        json!("none")
    );
    assert_eq!(
        mission["public_literature_review_queue"]["network"],
        json!(false)
    );
    assert_eq!(
        mission["research_brief"]["schema_version"],
        json!("bioprism-neurosurgery-research-brief/0.1")
    );
    assert_eq!(
        mission["research_brief"]["source"],
        json!("public_literature")
    );
    assert_eq!(
        mission["public_literature_freshness"]["status"],
        json!("stale")
    );
    assert_eq!(
        mission["evidence_synthesis"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-synthesis/0.1")
    );
    assert_eq!(mission["evidence_synthesis"]["provider"], json!("none"));
}

#[test]
fn mcp_fuses_real_glioma_and_pubmed_bundles_in_one_mission() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let public_literature: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/neurosurgical_public_literature_snapshot.json"
    ))
    .expect("cross-specialty literature snapshot parses");
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 14,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "public_literature": public_literature,
                    "query": {"text": "enzastaurin", "limit": 1},
                    "public_literature_query": {"specialty": "glioma", "text": "glioma", "limit": 1},
                    "portfolio_query": {"specialties": ["glioma"], "max_hits_per_lane": 1, "max_review_items_per_lane": 1, "max_issues_per_lane": 1},
                    "freshness": {"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("dual-bundle mission call parses");
    let response = server
        .handle(&rpc)
        .expect("dual-bundle mission call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("dual-bundle mission returns text content");
    let mission: Value = serde_json::from_str(text).expect("dual-bundle mission content is JSON");
    assert_eq!(mission["provider"], json!("none"));
    assert_eq!(mission["network"], json!(false));
    assert_eq!(
        mission["real_data_coverage"]["synthetic_data"],
        json!(false)
    );
    assert_eq!(
        mission["public_literature_evidence_packet"]["synthetic_data"],
        json!(false)
    );
    assert_eq!(mission["real_data_query"]["total_matches"], json!(1));
    assert_eq!(
        mission["public_literature_query"]["total_matches"],
        json!(16)
    );
    assert_eq!(
        mission["literature_link_audit"]["schema_version"],
        json!("bioprism-neurosurgery-literature-link-audit/0.1")
    );
    assert_eq!(mission["literature_link_audit"]["provider"], json!("none"));
    assert_eq!(mission["literature_link_audit"]["network"], json!(false));
    assert_eq!(
        mission["literature_link_audit"]["synthetic_data"],
        json!(false)
    );
    assert_eq!(
        mission["evidence_synthesis"]["schema_version"],
        json!("bioprism-neurosurgery-evidence-synthesis/0.1")
    );
    assert!(
        !mission["public_literature_workbench"]["lanes"][0]["design_strata"]
            .as_array()
            .expect("mission design strata array")
            .is_empty()
    );
    assert!(
        mission["public_literature_workbench"]["lanes"][0]["overlapping_design_count"].is_number()
    );
    assert_eq!(
        mission["evidence_synthesis"]["links"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(mission["evidence_synthesis"]["provider"], json!("none"));
    assert_eq!(
        mission["run"]["session"]["status"],
        json!("awaiting_human_review")
    );
}

#[test]
fn mcp_attaches_real_case_asset_provenance_to_a_mission_without_opening_assets() {
    let request: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real request parses");
    let real_data: Value = serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real snapshot parses");
    let manifest = json!({
        "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
        "specialty": "glioma",
        "synthetic_data": false,
        "direct_identifier_fields": [],
        "assets": [{
            "asset_id": "caller-local-mri-1",
            "kind": "imaging_series",
            "status": "observed",
            "source_kind": "dicom_archive",
            "source_id": "caller-dicom-archive",
            "content_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "modality": "MR",
            "body_region": "brain",
            "observed_at": "2026-01-01T00:00:00Z",
            "timepoint": "baseline"
        }]
    });
    let mut server = Server::new(repo_root());
    ready(&mut server);
    let rpc = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 15,
            "method": "tools/call",
            "params": {
                "name": "neurosurgery_mission",
                "arguments": {
                    "request": request,
                    "real_glioma_data": real_data,
                    "case_asset_manifest": manifest,
                    "case_asset_manifest_query": {
                        "requested_kinds": ["imaging_series", "molecular_assay"],
                        "max_review_items": 16
                    },
                    "max_steps": 32
                }
            }
        })
        .to_string(),
    )
    .expect("case-asset mission call parses");
    let response = server
        .handle(&rpc)
        .expect("case-asset mission call is answered")
        .to_json();
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("case-asset mission returns text content");
    let mission: Value = serde_json::from_str(text).expect("case-asset mission content is JSON");
    assert_eq!(mission["provider"], json!("none"));
    assert_eq!(mission["network"], json!(false));
    assert_eq!(
        mission["case_asset_manifest"]["schema_version"],
        json!("bioprism-neurosurgery-case-asset-manifest/0.1")
    );
    assert_eq!(mission["case_asset_manifest"]["asset_count"], json!(1));
    assert_eq!(
        mission["case_asset_manifest"]["missing_requested_kinds"],
        json!(["molecular_assay"])
    );
    assert_eq!(
        mission["case_asset_manifest"]["synthetic_data"],
        json!(false)
    );
    assert_eq!(
        mission["case_asset_manifest"]["human_review_required"],
        json!(true)
    );
    assert_eq!(
        mission["evidence_synthesis"]["case_asset_report_digest"],
        mission["case_asset_manifest"]["report_digest"]
    );
    assert!(!mission.to_string().contains("caller-local-mri-1"));
    assert!(!mission.to_string().contains("caller-dicom-archive"));
    assert_eq!(
        mission["run"]["session"]["status"],
        json!("awaiting_human_review")
    );
}
