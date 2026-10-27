use super::*;
use crate::{ApiConfig, ApiRouter};
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::AtomicU64;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);
const TOKEN: &str = "application-test-token-with-32-plus-bytes";

struct Fixture {
    root: PathBuf,
    config: WorkflowApplicationConfig,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aurora-workflow-api-{}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let python = ["python", "python3"]
            .into_iter()
            .find_map(|name| {
                let output = Command::new(name)
                    .args(["-c", "import sys;print(sys.executable)"])
                    .output()
                    .ok()?;
                if !output.status.success() {
                    return None;
                }
                Some(PathBuf::from(String::from_utf8(output.stdout).ok()?.trim()))
            })
            .expect("the executable-workflow bridge tests require the installed Python runtime");
        let python_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../python")
            .canonicalize()
            .unwrap();
        let config = WorkflowApplicationConfig {
            python_executable: python,
            python_path,
            state_root: root.clone(),
            authority: "127.0.0.1:8787".into(),
            model: None,
            base_url: "http://127.0.0.1:11435/v1".into(),
            research_sources: false,
            molecular_glioma: false,
            glioma_mutations: false,
            gene_references: false,
            protein_pathways: false,
            denominator_coverage: false,
            capabilities: Vec::new(),
        };
        Self { root, config }
    }
    fn application(&self) -> WorkflowApplication {
        WorkflowApplication::new(self.config.clone()).unwrap()
    }
    fn router(&self) -> ApiRouter {
        ApiRouter::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
            ApiConfig {
                bearer_token: Some(TOKEN.into()),
                workflow_application: Some(self.config.clone()),
                ..ApiConfig::default()
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn request(method: &str, path: &str, body: Value) -> HttpRequest {
    HttpRequest {
        method: method.into(),
        target: path.into(),
        version: "HTTP/1.1".into(),
        headers: BTreeMap::from([
            ("host".into(), "127.0.0.1:8787".into()),
            ("origin".into(), "http://127.0.0.1:8787".into()),
            ("content-type".into(), "application/json".into()),
            ("authorization".into(), format!("Bearer {TOKEN}")),
        ]),
        body: serde_json::to_vec(&body).unwrap(),
    }
}
fn data(envelope: Value) -> Value {
    assert_eq!(envelope["ok"], true, "{envelope}");
    envelope["data"].clone()
}
fn body(response: HttpResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}

#[test]
fn application_configuration_requires_loopback_existing_operator_paths_and_long_bearer() {
    let fixture = Fixture::new();
    for authority in [
        "0.0.0.0:8787",
        "localhost:8787",
        "127.0.0.1:0",
        "example.test:8787",
    ] {
        let mut config = fixture.config.clone();
        config.authority = authority.into();
        assert!(config.validate().is_err());
    }
    for token in [None, Some("sixteen-byte-token".into())] {
        assert!(ApiConfig {
            bearer_token: token,
            workflow_application: Some(fixture.config.clone()),
            ..ApiConfig::default()
        }
        .validate()
        .is_err());
    }
    let debug = format!(
        "{:?}",
        ApiConfig {
            bearer_token: Some(TOKEN.into()),
            ..ApiConfig::default()
        }
    );
    assert!(!debug.contains(TOKEN));
    assert!(debug.contains("bearer_token_configured"));
    let mut config = fixture.config.clone();
    config.python_executable = PathBuf::from("python");
    assert!(config.validate().is_err());
    for endpoint in [
        "http://127.0.0.1:0/v1",
        "http://127.0.0.1:bad/v1",
        "http://127.0.0.1:11435/other",
    ] {
        let mut config = fixture.config.clone();
        config.base_url = endpoint.into();
        assert!(config.validate().is_err());
    }
    let mut config = fixture.config.clone();
    config.model = Some("m".repeat(201));
    assert!(config.validate().is_err());
}

#[test]
fn hostile_origin_host_content_type_and_control_fields_are_refused_before_worker_start() {
    let fixture = Fixture::new();
    let router = fixture.router();
    for method in ["POST", "OPTIONS"] {
        let mut request = request(
            method,
            "/v1/executable-workflows/interpret",
            json!({"task":"Review glioma evidence."}),
        );
        request
            .headers
            .insert("origin".into(), "https://attacker.invalid".into());
        let response = router.handle(request);
        assert_eq!(response.status, 403);
        assert!(!response.headers.contains_key("access-control-allow-origin"));
    }
    let mut wrong_host = request("GET", "/workflows/", json!({}));
    wrong_host
        .headers
        .insert("host".into(), "localhost:8787".into());
    assert_eq!(router.handle(wrong_host).status, 403);
    let mut cross_site = request(
        "POST",
        "/v1/executable-workflows/interpret",
        json!({"task":"Review glioma evidence."}),
    );
    cross_site
        .headers
        .insert("sec-fetch-site".into(), "cross-site".into());
    assert_eq!(router.handle(cross_site).status, 403);
    let mut no_auth = request("GET", "/v1/executable-workflows/catalogue", json!({}));
    no_auth.headers.remove("authorization");
    assert_eq!(router.handle(no_auth).status, 401);
    let mut no_mime = request(
        "POST",
        "/v1/executable-workflows/interpret",
        json!({"task":"Review glioma evidence."}),
    );
    no_mime.headers.remove("content-type");
    assert_eq!(router.handle(no_mime).status, 415);
    assert_eq!(router.handle(request("POST", "/v1/executable-workflows/plans", json!({"task":"Review glioma evidence.","planner":"glioma-intent","argv":["forbidden"]}))).status,422);
    for field in [
        "glioma_mutations",
        "gene_references",
        "protein_pathways",
        "denominator_coverage",
    ] {
        let mut body = json!({"task":"A fixed public research label","planner":"glioma-mutations"});
        body[field] = json!(true);
        assert_eq!(
            router
                .handle(request("POST", "/v1/executable-workflows/plans", body))
                .status,
            422
        );
    }
    assert_eq!(router.handle(request("POST", "/v1/executable-workflows/runs", json!({"plan_id":"a".repeat(32),"expected_plan_digest":"b".repeat(64),"approve_source_dispatch":1}))).status,422);
    assert_eq!(
        std::fs::read_dir(&fixture.root).unwrap().count(),
        0,
        "no refused transport request may start the worker or create its directories"
    );
    for raw in [
        br#"{"approve_source_dispatch":false,"approve_source_dispatch":true}"#.as_slice(),
        br#"{"proposal":{"steps":[{"arguments":{"limit":1,"limit":2}}]}}"#.as_slice(),
        br#"{"requirements":{"max_steps":1,"max_steps":32}}"#.as_slice(),
    ] {
        let mut ambiguous = request("POST", "/v1/executable-workflows/plans/validate", json!({}));
        ambiguous.body = raw.to_vec();
        assert_eq!(router.handle(ambiguous).status, 400);
    }
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
    let mut asset = request("GET", "/workflows/", json!({}));
    asset.headers.remove("authorization");
    let asset = router.handle(asset);
    assert_eq!(asset.status, 200);
    assert!(asset.headers["content-security-policy"].contains("frame-ancestors 'none'"));
    assert_eq!(asset.headers["cache-control"], "no-store");
    assert_eq!(asset.headers["x-content-type-options"], "nosniff");
    let preflight = router.handle(request(
        "OPTIONS",
        "/v1/executable-workflows/plans",
        json!({}),
    ));
    assert_eq!(preflight.status, 204);
    assert!(!preflight
        .headers
        .contains_key("access-control-allow-origin"));
}

#[test]
fn optional_public_template_catalogues_are_operator_owned_and_plan_without_a_model() {
    for (mutations, references) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut fixture = Fixture::new();
        fixture.config.glioma_mutations = mutations;
        fixture.config.gene_references = references;
        let application = fixture.application();
        let catalogue = data(
            application
                .invoke(&json!({"action":"catalogue"}), Duration::from_secs(30))
                .unwrap(),
        );
        let operations: std::collections::BTreeSet<&str> = catalogue["operations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|operation| operation["operation_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            operations.len(),
            10 + usize::from(mutations) * 3 + usize::from(references) * 3
        );
        for (enabled, planner, source, analysis, review, steps) in [
            (
                mutations,
                "glioma-mutations",
                "gdc.gene_mutation_counts",
                "glioma.mutation.analyze",
                "glioma.mutation.review_handoff",
                3,
            ),
            (
                references,
                "gene-references",
                "ensembl.gene_references",
                "glioma.gene_references.analyze",
                "glioma.gene_references.review_handoff",
                4,
            ),
        ] {
            for operation in [source, analysis, review] {
                assert_eq!(operations.contains(operation), enabled);
            }
            let task = "A retained public research label; selectors remain fixed";
            let planned = application
                .invoke(
                    &json!({"action":"plan","planner":planner,"task":task}),
                    Duration::from_secs(30),
                )
                .unwrap();
            if !enabled {
                assert_eq!(planned["ok"], false);
                continue;
            }
            let planned = data(planned);
            assert_eq!(planned["task"], task);
            assert_eq!(planned["source_dispatch"], false);
            assert_eq!(planned["interpretation"]["provider_calls"], 0);
            assert_eq!(planned["interpretation"]["question_used_as_query"], false);
            assert_eq!(planned["plan"]["steps"].as_array().unwrap().len(), steps);
            assert_eq!(planned["plan"]["steps"][0]["operation"], source);
            assert_eq!(
                planned["plan"]["steps"][0]["arguments"],
                json!({"timeout_ms":30000})
            );
            if references && planner == "gene-references" {
                assert_eq!(
                    planned["plan"]["steps"][1]["operation"],
                    "ncbi_gene.summary"
                );
                assert_eq!(
                    planned["plan"]["steps"][1]["arguments"],
                    json!({"gene_symbols":["TP53"],"timeout_ms":30000})
                );
            }
            assert_eq!(
                planned["plan"]["requirements"]["max_calls_by_operation"][source],
                1
            );
            assert!(planned["plan"]["requirements"]["required_operations"]
                .as_array()
                .unwrap()
                .contains(&json!(review)));
            let bounded = application
                .invoke(
                    &json!({"action":"plan","planner":planner,"task":task,"max_steps":1}),
                    Duration::from_secs(30),
                )
                .unwrap();
            assert_eq!(bounded["ok"], false);
        }
    }
}

#[test]
fn provider_inspection_http_route_requires_authentication_and_matching_origin_without_source_authority(
) {
    let fixture = Fixture::new();
    let router = fixture.router();
    let path = "/v1/executable-workflows/providers";
    let mut unauthenticated = request("GET", path, json!({}));
    unauthenticated.headers.remove("authorization");
    assert_eq!(router.handle(unauthenticated).status, 401);
    let mut foreign_origin = request("GET", path, json!({}));
    foreign_origin
        .headers
        .insert("origin".into(), "https://unrelated.invalid".into());
    assert_eq!(router.handle(foreign_origin).status, 403);
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
    let response = router.handle(request("GET", path, json!({})));
    assert_eq!(response.status, 200);
    let inspected = data(body(response));
    assert_eq!(inspected["counts"]["implemented_reader_paths"], 18);
    assert_eq!(
        inspected["counts"]["registered_reviewed_default_readers"],
        8
    );
    assert!(inspected["counts"]["independently_live_verified_providers"]["value"].is_null());
    assert_eq!(inspected["network_requests_on_inspection"], 0);
    for directory in ["plans", "runs"] {
        assert_eq!(
            std::fs::read_dir(fixture.root.join(directory))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn denominator_flag_and_provider_route_use_actual_worker_without_source_or_model_authority() {
    for enabled in [false, true] {
        let mut fixture = Fixture::new();
        fixture.config.denominator_coverage = enabled;
        let application = fixture.application();
        let providers = data(
            application
                .invoke(&json!({"action":"providers"}), Duration::from_secs(30))
                .unwrap(),
        );
        assert_eq!(providers["counts"]["implemented_reader_paths"], 18);
        assert_eq!(providers["counts"]["declared_reviewed_service_groups"], 15);
        assert_eq!(providers["network_requests_on_inspection"], 0);
        assert!(providers["counts"]["independently_live_verified_providers"]["value"].is_null());
        let response = application.invoke(&json!({"action":"plan", "task":"Review fixed TP53 denominator evidence", "planner":"denominator-coverage"}), Duration::from_secs(30)).unwrap();
        if !enabled {
            assert_eq!(response["ok"], false);
            continue;
        }
        let planned = data(response);
        assert_eq!(planned["source_dispatch"], false);
        assert_eq!(planned["interpretation"]["provider_calls"], 0);
        assert_eq!(planned["plan"]["steps"].as_array().unwrap().len(), 4);
        assert_eq!(
            planned["plan"]["steps"][1]["operation"],
            "gdc.ssm_project_coverage"
        );
        assert_eq!(
            planned["plan"]["steps"][2]["arguments"]["coverage_bundle"]["$ref"],
            "coverage#"
        );
        assert_eq!(
            planned["plan"]["requirements"]["data_links"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}

#[test]
fn reviewed_capability_profiles_reach_actual_worker_without_browser_configuration_or_dispatch() {
    let mut fixture = Fixture::new();
    fixture.config.capabilities = vec!["gene-identity".into(), "glioma-ontology".into()];
    let application = fixture.application();
    let catalogue = data(
        application
            .invoke(&json!({"action":"catalogue"}), Duration::from_secs(30))
            .unwrap(),
    );
    assert_eq!(
        catalogue["capabilities"]["selected_profiles"],
        json!(["gene-identity", "glioma-ontology"])
    );
    for (profile, count) in [("gene-identity", 5), ("glioma-ontology", 1)] {
        let planned = data(application.invoke(&json!({"action":"plan", "task":"Inspect fixed source metadata", "planner":format!("capability:{profile}")}), Duration::from_secs(30)).unwrap());
        assert_eq!(planned["source_dispatch"], false);
        assert_eq!(planned["interpretation"]["provider_calls"], 0);
        assert_eq!(planned["plan"]["steps"].as_array().unwrap().len(), count);
        let refused = application.invoke(&json!({"action":"plan", "task":"Inspect", "planner":format!("capability:{profile}"), "capabilities":[profile]}), Duration::from_secs(30)).unwrap();
        assert_eq!(refused["ok"], false);
    }
    let disabled = Fixture::new();
    let refused = disabled
        .application()
        .invoke(
            &json!({"action":"plan", "task":"Inspect", "planner":"capability:glioma-ontology"}),
            Duration::from_secs(30),
        )
        .unwrap();
    assert_eq!(refused["ok"], false);
    assert!(!fixture
        .root
        .join("runs")
        .read_dir()
        .unwrap()
        .any(|entry| entry.is_ok()));
}

#[test]
fn capability_configuration_bounds_names_and_refuses_unknown_profiles_in_fixed_worker() {
    let fixture = Fixture::new();
    for values in [
        vec![""],
        vec!["Bad"],
        vec!["../gene-identity"],
        vec!["--model"],
        vec!["gene-identity", "gene-identity"],
        vec!["x"; 33],
    ] {
        let mut config = fixture.config.clone();
        config.capabilities = values.into_iter().map(String::from).collect();
        assert!(config.validate().is_err());
    }
    let mut config = fixture.config.clone();
    config.capabilities = vec!["unknown-profile".into()];
    let application = WorkflowApplication::new(config).unwrap();
    let refused = application
        .invoke(&json!({"action":"catalogue"}), Duration::from_secs(30))
        .unwrap();
    assert_eq!(refused["ok"], false);
    assert!(!fixture.root.join("plans").exists());
    assert!(!fixture.root.join("runs").exists());
}

#[test]
fn protein_pathway_flag_reaches_the_actual_worker_without_granting_source_or_model_calls() {
    for enabled in [false, true] {
        let mut fixture = Fixture::new();
        fixture.config.protein_pathways = enabled;
        let application = fixture.application();
        let catalogue = data(
            application
                .invoke(&json!({"action":"catalogue"}), Duration::from_secs(30))
                .unwrap(),
        );
        assert_eq!(catalogue["operation_count"], if enabled { 14 } else { 10 });
        let response = application.invoke(&json!({"action":"plan", "task":"Inspect fixed TP53 protein/pathway annotations", "planner":"protein-pathways"}), Duration::from_secs(30)).unwrap();
        if !enabled {
            assert_eq!(response["ok"], false);
            continue;
        }
        let planned = data(response);
        assert_eq!(planned["source_dispatch"], false);
        assert_eq!(planned["interpretation"]["provider_calls"], 0);
        assert_eq!(planned["plan"]["steps"].as_array().unwrap().len(), 4);
        assert_eq!(
            planned["plan"]["steps"][0]["operation"],
            "uniprot.protein_annotation"
        );
        assert_eq!(
            planned["plan"]["steps"][1]["operation"],
            "reactome.pathway_annotations"
        );
        assert_eq!(
            planned["plan"]["requirements"]["schema"],
            "aurora-workflow-requirements/0.2"
        );
        assert_eq!(planned["plan"]["requirements"]["max_steps"], 4);
        assert_eq!(
            planned["plan"]["requirements"]["data_links"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            planned["plan"]["requirements"]["data_links"][0]["source_pointer"],
            ""
        );
    }
}

#[test]
fn incremental_controls_pass_the_http_allowlist_but_never_bypass_worker_approval_or_integer_checks()
{
    let fixture = Fixture::new();
    let router = fixture.router();
    for (model_calls, expected) in [
        (json!(3), "approval_required"),
        (json!(1.0), "invalid_request"),
        (json!(true), "invalid_request"),
    ] {
        let response = router.handle(request("POST", "/v1/executable-workflows/plans", json!({
            "task":"Inspect public metadata", "planner":"local-incremental", "max_model_calls":model_calls,
            "decision_attempts":1, "planning_attempts":1, "operation_ids":["pubmed.search"],
            "approve_provider_call":false
        })));
        let envelope: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["error"]["code"], expected);
    }
    assert_eq!(
        std::fs::read_dir(fixture.root.join("plans"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(fixture.root.join("runs"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn actual_worker_retains_task_contract_and_changed_revisions_refuse_stale_approval() {
    let fixture = Fixture::new();
    let application = fixture.application();
    let catalogue = data(
        application
            .invoke(&json!({"action":"catalogue"}), Duration::from_secs(30))
            .unwrap(),
    );
    assert!(catalogue["operation_count"].as_u64().unwrap() >= 10);
    let task = "Review glioma evidence using PubMed only; exclude trials and TCGA.";
    let plan = data(
        application
            .invoke(
                &json!({"action":"plan","task":task,"planner":"glioma-intent"}),
                Duration::from_secs(30),
            )
            .unwrap(),
    );
    let id = plan["plan_id"].as_str().unwrap();
    let digest = plan["plan_digest"].as_str().unwrap();
    let mut proposal = json!({"name":plan["plan"]["name"],"steps":plan["plan"]["steps"],"outputs":plan["plan"]["outputs"]});
    proposal["steps"][0]["arguments"]["per_specialty_limit"] = json!(25);
    let refused = application.invoke(&json!({"action":"validate","plan_id":id,"expected_plan_digest":digest,"proposal":proposal}),Duration::from_secs(30)).unwrap();
    assert_eq!(refused["ok"], false);
    proposal["steps"][0]["arguments"] = plan["plan"]["steps"][0]["arguments"].clone();
    proposal["name"] = json!("Reviewed name <img src=x onerror=alert(1)>");
    let revision = data(application.invoke(&json!({"action":"validate","plan_id":id,"expected_plan_digest":digest,"proposal":proposal}),Duration::from_secs(30)).unwrap());
    assert_eq!(revision["task"], task);
    assert_ne!(revision["plan_digest"], digest);
    assert_eq!(
        revision["plan"]["requirements"],
        plan["plan"]["requirements"]
    );
    let stale = application
        .invoke(
            &json!({"action":"create_run","plan_id":id,"expected_plan_digest":digest}),
            Duration::from_secs(30),
        )
        .unwrap();
    assert_eq!(stale["ok"], false);
    assert_eq!(stale["error"]["code"], "revision_conflict");
    assert_eq!(
        std::fs::read_dir(fixture.root.join("runs"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn cancellation_survives_real_process_restarts_and_background_resume_does_not_release_it() {
    let fixture = Fixture::new();
    let application = fixture.application();
    let plan = data(application.invoke(&json!({"action":"plan","task":"Review glioma evidence using PubMed only; exclude trials and TCGA.","planner":"glioma-intent"}),Duration::from_secs(30)).unwrap());
    let run = data(application.invoke(&json!({"action":"create_run","plan_id":plan["plan_id"],"expected_plan_digest":plan["plan_digest"]}),Duration::from_secs(30)).unwrap());
    let id = run["run_id"].as_str().unwrap();
    data(
        application
            .invoke(
                &json!({"action":"cancel","run_id":id}),
                Duration::from_secs(30),
            )
            .unwrap(),
    );
    let cancelled = data(application.invoke(&json!({"action":"execute","run_id":id,"expected_plan_digest":plan["plan_digest"],"approve_source_dispatch":true}),Duration::from_secs(30)).unwrap());
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["metadata_only"], true);
    let router = fixture.router();
    let resumed = router.handle(request(
        "POST",
        &format!("{PREFIX}/runs/{id}/resume"),
        json!({"expected_plan_digest":plan["plan_digest"],"approve_source_dispatch":true}),
    ));
    assert_eq!(resumed.status, 202);
    let started = Instant::now();
    loop {
        let inspected = data(body(router.handle(request(
            "GET",
            &format!("{PREFIX}/runs/{id}"),
            json!({}),
        ))));
        if inspected
            .pointer("/worker_process/state")
            .and_then(Value::as_str)
            == Some("finished")
            && inspected["active"] == false
        {
            assert_eq!(inspected["status"], "cancelled");
            assert_eq!(inspected["cancel_requested"], true);
            assert!(!stringify(&inspected).contains("\"value\""));
            assert!(!stringify(&inspected).contains("\"receipt\""));
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(50));
    }
    let exported = router.handle(request(
        "GET",
        &format!("{PREFIX}/runs/{id}/checkpoint"),
        json!({}),
    ));
    assert_eq!(exported.status, 200);
    assert!(exported.headers["content-disposition"].contains("attachment"));
    let checkpoint = body(exported);
    assert_eq!(checkpoint["status"], "cancelled");
    assert_eq!(checkpoint["attempts"], json!({}));
    let mut proposal = json!({"name":plan["plan"]["name"],"steps":plan["plan"]["steps"],"outputs":plan["plan"]["outputs"]});
    proposal["name"] = json!("A separately reviewed revision");
    data(application.invoke(&json!({"action":"validate","plan_id":plan["plan_id"],"expected_plan_digest":plan["plan_digest"],"proposal":proposal}),Duration::from_secs(30)).unwrap());
    assert_eq!(
        router
            .handle(request(
                "POST",
                &format!("{PREFIX}/runs/{id}/resume"),
                json!({"expected_plan_digest":plan["plan_digest"],"approve_source_dispatch":true})
            ))
            .status,
        202
    );
    let started = Instant::now();
    loop {
        let observed = data(body(router.handle(request(
            "GET",
            &format!("{PREFIX}/runs/{id}"),
            json!({}),
        ))));
        if observed
            .pointer("/worker_process/state")
            .and_then(Value::as_str)
            == Some("finished")
            && observed["active"] == false
        {
            assert_eq!(observed["worker_process"]["response_ok"], false);
            assert_eq!(
                observed["worker_process"]["error"]["code"],
                "revision_conflict"
            );
            assert_eq!(observed["status"], "cancelled");
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn stringify(value: &Value) -> String {
    serde_json::to_string(value).unwrap()
}

#[test]
fn real_http_connection_enforces_origin_before_dispatch_and_serves_inert_assets() {
    let mut fixture = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let authority = listener.local_addr().unwrap().to_string();
    fixture.config.authority = authority.clone();
    let router = Arc::new(fixture.router());
    let serving = std::thread::spawn(move || {
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            crate::serve_connection(stream, Arc::clone(&router)).unwrap();
        }
    });
    for (origin, expected) in [
        ("http://attacker.invalid", "403 Forbidden"),
        (&format!("http://{authority}"), "200 OK"),
    ] {
        let mut stream = TcpStream::connect(&authority).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        write!(
            stream,
            "GET /workflows/ HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\n\r\n"
        )
        .unwrap();
        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        let mut content_length = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(!line.is_empty(), "response ended before its HTTP headers");
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            response.push_str(&line);
            if line == "\r\n" {
                break;
            }
            assert!(
                response.len() <= 32 * 1024,
                "response headers exceed the boundary"
            );
        }
        let mut payload =
            vec![0; content_length.expect("bounded server responses declare their body length")];
        reader.read_exact(&mut payload).unwrap();
        response.push_str(std::str::from_utf8(&payload).unwrap());
        assert!(response.contains(expected));
        assert!(!response.contains("Access-Control-Allow-Origin: *"));
    }
    serving.join().unwrap();
    assert_eq!(std::fs::read_dir(&fixture.root).unwrap().count(), 0);
    let javascript = include_str!("../../assets/workflows/app.js");
    assert!(!javascript.contains("innerHTML"));
    assert!(!javascript.contains("localStorage"));
    assert!(!javascript.contains("sessionStorage"));
    assert!(javascript.contains("textContent"));
}

#[test]
fn bridge_bounds_actual_process_output_and_kills_timed_out_workers_without_restarting() {
    let mut fixture = Fixture::new();
    let sdk = fixture.root.join("fixture-sdk");
    let package = sdk.join("prism_sdk");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("__init__.py"), "").unwrap();
    std::fs::write(package.join("workflow_api_worker.py"), "import json,sys,time,os\nr=json.load(sys.stdin)\nif r['action']=='slow':\n time.sleep(10)\nelif r['action']=='overflow':\n sys.stdout.write('x'*17000000)\nelif r['action']=='stderr':\n sys.stderr.write('x'*70000)\nelif r['action']=='large':\n print(json.dumps({'schema':'aurora-workflow-api-response/0.1','ok':True,'data':{'large_integer':2**80}}))\nelif r['action']=='validate':\n print(json.dumps({'schema':'aurora-workflow-api-response/0.1','ok':True,'data':{'large_integer':r['proposal']['integer']}}))\nelse:\n print(json.dumps({'schema':'aurora-workflow-api-response/0.1','ok':True,'data':{'api_token_inherited':'AURORA_API_TOKEN' in os.environ}}))\n").unwrap();
    fixture.config.python_path = sdk;
    let application = fixture.application();
    let large = application.call(json!({"action":"large"}), Duration::from_secs(5));
    assert_eq!(large.status, 200);
    assert!(String::from_utf8(large.body)
        .unwrap()
        .contains("1208925819614629174706176"));
    let mut exact = request("POST", &format!("{PREFIX}/plans/validate"), json!({}));
    exact.body=br#"{"plan_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","expected_plan_digest":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","proposal":{"integer":1208925819614629174706176}}"#.to_vec();
    let echoed = application.handle(&exact);
    assert_eq!(echoed.status, 200);
    assert!(String::from_utf8(echoed.body)
        .unwrap()
        .contains("1208925819614629174706176"));
    let environment = data(
        application
            .invoke(&json!({"action":"environment"}), Duration::from_secs(5))
            .unwrap(),
    );
    assert_eq!(environment["api_token_inherited"], false);
    let started = Instant::now();
    assert!(application
        .invoke(&json!({"action":"slow"}), Duration::from_millis(100))
        .unwrap_err()
        .contains("time bound"));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(application
        .invoke(&json!({"action":"overflow"}), Duration::from_secs(10))
        .is_err());
    assert!(application
        .invoke(&json!({"action":"stderr"}), Duration::from_secs(5))
        .is_err());
    assert_eq!(
        data(
            application
                .invoke(&json!({"action":"environment"}), Duration::from_secs(5))
                .unwrap()
        )["api_token_inherited"],
        false
    );
}
