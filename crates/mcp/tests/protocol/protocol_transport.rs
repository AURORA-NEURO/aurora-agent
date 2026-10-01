//! MCP contract tests for protocol transport contracts.

use super::*;

#[test]
fn initialize_reports_the_protocol_version_and_instructions() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();

    assert_eq!(
        response["result"]["protocolVersion"],
        json!(PROTOCOL_VERSION)
    );
    assert_eq!(response["result"]["serverInfo"]["name"], json!("bioprism"));
    let instructions = response["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains("not a medical device") || instructions.contains("not a medical")
    );
    assert!(instructions.contains("fiber_compile"));
}

#[test]
fn tools_call_rejects_non_object_arguments_before_dispatch() {
    let mut server = server();
    ready(&mut server);
    for arguments in [json!([]), json!("not-an-object"), Value::Null] {
        let request = Request::parse(
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": { "name": "workspace_capabilities", "arguments": arguments }
            })
            .to_string(),
        )
        .unwrap();
        let response = server.handle(&request).unwrap().to_json();
        assert_eq!(response["error"]["code"], json!(code::INVALID_PARAMS));
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("arguments must be an object")
        );
    }
}

#[test]
fn notifications_are_not_answered() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
    assert!(server.handle(&request).is_none());
}

#[test]
fn unknown_methods_and_malformed_input_produce_typed_errors() {
    let mut server = server();
    let request = Request::parse(r#"{"jsonrpc":"2.0","id":7,"method":"nope"}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();
    assert_eq!(response["error"]["code"], json!(-32601));

    let failure = Request::parse("not json at all").unwrap_err().to_json();
    assert_eq!(failure["error"]["code"], json!(-32700));
}

#[test]
fn malformed_json_rpc_envelopes_are_refused_before_dispatch() {
    let missing_version = Request::parse(r#"{"id":1,"method":"ping"}"#)
        .unwrap_err()
        .to_json();
    assert_eq!(missing_version["error"]["code"], json!(-32600));

    let scalar = Request::parse(r#"[1,2,3]"#).unwrap_err().to_json();
    assert_eq!(scalar["error"]["code"], json!(-32600));

    let bad_params = Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":true}"#)
        .unwrap_err()
        .to_json();
    assert_eq!(bad_params["error"]["code"], json!(-32600));
}

#[test]
fn tools_are_not_available_before_the_session_is_ready() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();
    assert_eq!(response["error"]["code"], json!(-32600));

    ready(&mut server);
    let response = server.handle(&request).unwrap().to_json();
    assert!(response["result"]["tools"].is_array());
}

#[test]
fn schemas_are_exposed_as_read_only_resources() {
    let mut server = server();
    ready(&mut server);

    let list = Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}}"#)
        .unwrap();
    let listed = server.handle(&list).unwrap().to_json();
    assert_eq!(listed["result"]["resources"].as_array().unwrap().len(), 5);

    let adaptive = Request::parse(
        &json!({
            "jsonrpc":"2.0", "id":4, "method":"resources/read",
            "params": { "uri": ADAPTIVE_QUERY_SCHEMA_URI }
        })
        .to_string(),
    )
    .unwrap();
    let adaptive_document = server.handle(&adaptive).unwrap().to_json();
    let adaptive_text = adaptive_document["result"]["contents"][0]["text"]
        .as_str()
        .unwrap();
    let adaptive_schema: Value =
        serde_json::from_str(adaptive_text).expect("adaptive schema is valid JSON");
    assert_eq!(adaptive_schema["title"], json!("AURORA FIBER Query 0.5"));

    let read = Request::parse(
        &json!({
            "jsonrpc":"2.0", "id":2, "method":"resources/read",
            "params": { "uri": CERTIFICATE_SCHEMA_URI }
        })
        .to_string(),
    )
    .unwrap();
    let document = server.handle(&read).unwrap().to_json();
    let text = document["result"]["contents"][0]["text"].as_str().unwrap();
    let schema: Value = serde_json::from_str(text).expect("resource is valid JSON");
    assert_eq!(schema["title"], json!("AURORA FIBER Context Certificate"));

    let capabilities = Request::parse(
        &json!({
            "jsonrpc":"2.0", "id":3, "method":"resources/read",
            "params": { "uri": CAPABILITIES_URI }
        })
        .to_string(),
    )
    .unwrap();
    let document = server.handle(&capabilities).unwrap().to_json();
    let text = document["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("world_and_ingestion"));
}

#[test]
fn workspace_capabilities_are_explicit_about_every_major_domain_surface() {
    let mut server = server();
    let payload = call(&mut server, "workspace_capabilities", json!({}));
    let capabilities = payload.as_array().expect("catalog is an array");
    assert!(capabilities.len() >= 10);
    for domain in [
        "world_and_ingestion",
        "decision_context",
        "trajectory_and_decision_cells",
        "benchmark_pack_portfolio",
        "evaluation_and_baselines",
        "mutation_and_causal_discovery",
        "bioevaluation_reference_contracts",
        "biological_domains",
        "biological_ir_and_query",
        "oncoworlds_identity_and_transport",
        "oncoworlds_models_and_assays",
        "oncoworlds_clonal_evolution",
        "safety_privacy_and_policy",
        "agent_orchestration",
        "registry_operations_and_infrastructure",
        "inference_lab",
        "oracle_mesh",
        "runtime_execution_and_replay",
        "release_and_reproduction",
        "documentation_and_knowledge",
        "developer_and_release_contracts",
    ] {
        assert!(
            capabilities.iter().any(|item| item["id"] == json!(domain)),
            "missing domain {domain}"
        );
    }
}

#[test]
fn stdout_carries_only_json_rpc() {
    let mut server = server();
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "fiber_compile", "arguments": { "world": WORLD, "query": QUERY } }
        })
    );

    let mut output = Vec::new();
    serve(&mut server, input.as_bytes(), &mut output).expect("serves");
    let text = String::from_utf8(output).unwrap();

    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 2, "the notification must not be answered");
    for line in lines {
        let parsed: Value = serde_json::from_str(line).expect("every stdout line is JSON-RPC");
        assert_eq!(parsed["jsonrpc"], json!("2.0"));
    }
}
