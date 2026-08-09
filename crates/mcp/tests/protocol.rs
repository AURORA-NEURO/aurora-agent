//! Protocol conformance and the security properties of 11.11.

use bioprism_mcp::{serve, tool_definitions, Request, Server, PROTOCOL_VERSION};
use serde_json::{json, Value};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn server() -> Server {
    Server::new(repo_root())
}

fn call(server: &mut Server, name: &str, arguments: Value) -> Value {
    let request = Request::parse(
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })
        .to_string(),
    )
    .expect("request parses");

    let response = server.handle(&request).expect("call is answered");
    let json = response.to_json();
    let text = json["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let mut parsed: Value = serde_json::from_str(text).expect("payload is JSON");
    if let Some(map) = parsed.as_object_mut() {
        map.insert(
            "__isError".into(),
            json["result"]["isError"].clone(),
        );
    }
    parsed
}

const WORLD: &str = "fixtures/fiber-v0.1/radiogenomic_world.json";
const QUERY: &str = "fixtures/fiber-v0.1/leakage_query.json";

#[test]
fn initialize_reports_the_protocol_version_and_instructions() {
    let mut server = server();
    let request =
        Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
    let response = server.handle(&request).unwrap().to_json();

    assert_eq!(response["result"]["protocolVersion"], json!(PROTOCOL_VERSION));
    assert_eq!(response["result"]["serverInfo"]["name"], json!("bioprism"));
    let instructions = response["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("not a medical device") || instructions.contains("not a medical"));
    assert!(instructions.contains("fiber_compile"));
}

#[test]
fn every_tool_declares_an_input_schema_with_required_fields() {
    let tools = tool_definitions();
    assert_eq!(tools.len(), 5);
    for tool in &tools {
        assert!(tool["name"].is_string());
        assert!(tool["description"].as_str().unwrap().len() > 40);
        assert_eq!(tool["inputSchema"]["type"], json!("object"));
        assert!(tool["inputSchema"]["required"].is_array());
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

/// The disclosure contract: L0 carries the decision and the omissions, never the evidence.
#[test]
fn compile_returns_the_contract_not_the_evidence() {
    let mut server = server();
    let payload = call(&mut server, "fiber_compile", json!({ "world": WORLD, "query": QUERY }));

    assert_eq!(payload["layer"], json!("l0"));
    assert_eq!(payload["verdict"]["status"], json!("invalid"));
    assert_eq!(
        payload["certificate_sha256"],
        json!("c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4")
    );

    assert!(payload.get("evidence").is_none(), "L0 must not carry values");
    assert!(payload.get("evidence_inventory").is_none());
    assert!(payload.get("factors").is_none());

    assert_eq!(payload["omissions"]["omitted_facts"], json!(750));
    assert_eq!(payload["omissions"]["supports_sufficiency_claim"], json!(true));
    assert_eq!(payload["refine"]["next_layer"], json!("l1"));
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
        assert_eq!(payload["omissions"]["protected_closure_satisfied"], json!(true));
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
    let payload = call(&mut server, "fiber_explain", json!({ "world": WORLD, "query": QUERY }));
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
    assert!(server.resolve("fixtures/fiber-v0.1/leakage_query.json").is_ok());
    assert!(server.resolve("./fixtures/fiber-v0.1/leakage_query.json").is_ok());

    for hostile in [
        "../../../etc/passwd",
        "fixtures/../../secrets.json",
        "/etc/passwd",
        "C:/Windows/System32/config/SAM",
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
    assert!(preview["preview"]["effect"].as_str().unwrap().contains("would write"));
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
