//! Well-formed documents for the verifiers the first battery did not reach.
//!
//! Every document here is produced by the code that writes it — `ResultBundle::attest`,
//! `BenchmarkPack::attest`, `ConformanceCertificate::to_json`, `CookbookReport::of`,
//! `SliceCatalog::run_all`, `SliceRegistry::run_all`, `RepairPlan::to_json`,
//! `instantiate_domain_workflow`, `build_domain_workflow_portfolio`, `run_workbench`,
//! `normalize_domain_evidence_provider` and `record_domain_evidence_provider_external_payload` —
//! over inputs the repository already ships or already generates. Nothing here hand-writes a
//! sealed document, because a hand-written one that its verifier happens to reject would turn
//! every mutation into a tautology, and one that it happens to accept would be evidence about the
//! literal rather than about the producer.
//!
//! Every document that costs real work to build is memoised: one generated structural world is
//! shared by the prism bundle and the registry pack, and the conformance certificate, the two
//! slice reports and the repair pair each run their producer once. Generating a world, running the
//! fiber conformance suite and running both slice catalogues are the expensive steps in this file,
//! and thirteen battery subjects would otherwise pay for them thirteen times.

use std::path::PathBuf;
use std::sync::OnceLock;

use bioprism_bioworlds::SliceCatalog;
use bioprism_conformance::{fiber_suite, shipped_baseline, FiberReference, FixtureStore};
use bioprism_cookbook::{standard_cookbook, CookbookReport};
use bioprism_devplat::{
    instantiate_domain_workflow, record_domain_evidence_provider_external_payload, run_workbench,
    ArtifactCard, ArtifactState, CellInput, CellKind, ChangeKind, CiCheck, CiRequest,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest, DomainEvidenceProviderNormalizationRequest,
    DomainEvidenceProviderReplayRequest, EvidencePosture, NotebookPolicy, StudioCell, StudioChange,
    StudioSession, WorkbenchRequest, WorkbenchVerificationPolicy, WorkbenchVerificationRequest,
};
use bioprism_domain::DomainPack;
use bioprism_examples::SliceRegistry;
use bioprism_fiber::{compile_with_oracle, Query};
use bioprism_ids::ContentHash;
use bioprism_prism::{matched_fork, Architecture, DecisionCell, InputRef, ResultBundle};
use bioprism_project::{AssemblyOptions, Issue, ProjectScan, ProjectWorld, ScanOptions};
use bioprism_registry::BenchmarkPack;
use bioprism_repair::{plan_for_issue, verify as verify_repair, PlanOptions, RepairPlan};
use bioprism_section::{ContextCertificate, OracleStatus};
use bioprism_world::World;
use bioprism_worldgen::{generate, WorldSpec};
use serde_json::{json, Value};

/// The canonical digest of a value, computed the way every producer in this workspace computes
/// one. Named here rather than imported because the crates that seal these documents keep their
/// own private one-line wrapper around `ContentHash::of_value` instead of exporting it.
fn canonical_digest(value: &Value) -> String {
    ContentHash::of_value(value)
        .expect("the value canonicalises")
        .to_string()
}

fn workspace_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

// -- prism result bundle ------------------------------------------------------------------------

struct GeneratedPair {
    world: Value,
    query: Value,
}

/// One discriminating structural world, generated once and shared by the prism and registry
/// documents. Both need a world whose leakage the panel actually separates on, and generating two
/// would double the cost for no additional coverage.
fn generated_pair() -> &'static GeneratedPair {
    static PAIR: OnceLock<GeneratedPair> = OnceLock::new();
    PAIR.get_or_init(|| {
        let generated = generate(&WorldSpec::discriminating(750));
        GeneratedPair {
            world: generated.world,
            query: generated.query,
        }
    })
}

fn decision_cell() -> DecisionCell {
    let pair = generated_pair();
    DecisionCell::new(
        "cell.split-integrity",
        "Is this train/test split contaminated, and by which mechanism?",
        InputRef::new("worldgen://discriminating-750", &pair.world),
        InputRef::new("worldgen://discriminating-750/query", &pair.query),
    )
    .accepting(OracleStatus::Invalid)
    .requiring_witness("identity_leakage")
}

pub fn prism_result_bundle() -> Value {
    static BUNDLE: OnceLock<Value> = OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            let pair = generated_pair();
            let world =
                World::from_json(pair.world.clone()).expect("the generated world validates");
            let query = Query::from_json(pair.query.clone()).expect("the generated query parses");
            let fork = matched_fork(
                &decision_cell(),
                &world,
                &query,
                &Architecture::default_panel(),
            );
            ResultBundle::new(decision_cell(), fork).attest()
        })
        .clone()
}

// -- registry benchmark pack --------------------------------------------------------------------

pub fn registry_pack() -> Value {
    static PACK: OnceLock<Value> = OnceLock::new();
    PACK.get_or_init(|| {
        let pair = generated_pair();
        let family = bioprism_mutation::generate(&pair.world, &bioprism_mutation::standard_suite())
            .expect("a worldgen world is mutable");
        BenchmarkPack::builder("oncoworld/split-integrity", "1.0.0")
            .intended_use(
                "Regression family for split-integrity decisions on synthetic structural worlds.",
            )
            .publisher("aurora-bioprism")
            .license("Apache-2.0")
            .limited_by(
                "Synthetic worlds only; establishes nothing about observed biological cohorts.",
            )
            .cell(decision_cell())
            .family(&family, "synthetic, bioprism-worldgen 43.39")
            .build()
            .expect("the family carries its own parents")
            .attest()
            .expect("the pack is digestible")
    })
    .clone()
}

// -- conformance certificate --------------------------------------------------------------------

pub fn conformance_certificate() -> Value {
    static CERTIFICATE: OnceLock<Value> = OnceLock::new();
    CERTIFICATE
        .get_or_init(|| {
            let suite = fiber_suite();
            let fixtures = FixtureStore::load(workspace_root().join("fixtures"), &suite.manifest)
                .expect("the shipped conformance fixtures load");
            suite
                .run(&FiberReference, &fixtures)
                .expect("the conformance suite runs")
                .declaring_baseline(shipped_baseline())
                .certify("2026-08-08T00:00:00Z", "2027-08-08T00:00:00Z")
                .expect("a fully conformant reference run certifies")
                .to_json()
                .expect("the certificate serialises")
        })
        .clone()
}

// -- cookbook report ----------------------------------------------------------------------------

pub fn cookbook_report() -> Value {
    static REPORT: OnceLock<Value> = OnceLock::new();
    REPORT
        .get_or_init(|| {
            let report = CookbookReport::of(&standard_cookbook().expect("the catalogue builds"))
                .expect("the report builds");
            serde_json::to_value(report).expect("the report serialises")
        })
        .clone()
}

// -- bioworlds catalogue report and examples registry report -------------------------------------

/// Two more documents of the cookbook report's exact shape: a serde struct carrying a `digest`
/// over its own body, checked by a `digest_is_intact() -> bool`. They are separate subjects rather
/// than one because they are separate artifacts with separate producers, and a battery that
/// covered one and assumed the other would be assuming the thing it exists to measure.
pub fn bioworlds_catalog_report() -> Value {
    static REPORT: OnceLock<Value> = OnceLock::new();
    REPORT
        .get_or_init(|| {
            let report = SliceCatalog::standard()
                .expect("the standard catalogue builds")
                .run_all()
                .expect("the standard catalogue runs");
            serde_json::to_value(report).expect("the catalogue report serialises")
        })
        .clone()
}

pub fn examples_registry_report() -> Value {
    static REPORT: OnceLock<Value> = OnceLock::new();
    REPORT
        .get_or_init(|| {
            let report = SliceRegistry::standard()
                .run_all()
                .expect("the standard registry runs");
            serde_json::to_value(report).expect("the registry report serialises")
        })
        .clone()
}

// -- repair plan and acceptance report ----------------------------------------------------------

fn demo_project() -> &'static ProjectWorld {
    static PROJECT: OnceLock<ProjectWorld> = OnceLock::new();
    PROJECT.get_or_init(|| {
        let root = workspace_root()
            .join("fixtures")
            .join("projects")
            .join("demo-app");
        let (scan, _) = ProjectScan::scan(&root, &ScanOptions::new("demo-app"))
            .expect("the demo project tree scans");
        let issues = Issue::load(&root.join("issues.json")).expect("the issue list loads");
        ProjectWorld::assemble(
            &scan,
            &AssemblyOptions {
                issues,
                ..AssemblyOptions::default()
            },
        )
        .expect("the demo project assembles")
    })
}

const DEMO_ISSUE: &str = "ISSUE-1";

fn compiled_issue() -> (World, DomainPack, ContextCertificate) {
    let assembled = demo_project();
    let world = World::from_json(assembled.world.clone()).expect("the project world validates");
    let pack = DomainPack::from_json(&assembled.pack).expect("the project pack parses");
    let query = Query::from_json(
        assembled
            .issue_queries
            .get(DEMO_ISSUE)
            .expect("the demo issue has a generated query")
            .clone(),
    )
    .expect("the issue query parses");
    let compiled = compile_with_oracle(&world, &query, pack.oracle()).expect("the query compiles");
    (world, pack, compiled.certificate)
}

fn repair_plan_struct() -> &'static RepairPlan {
    static PLAN: OnceLock<RepairPlan> = OnceLock::new();
    PLAN.get_or_init(|| {
        let (world, pack, certificate) = compiled_issue();
        plan_for_issue(
            &world,
            &pack,
            DEMO_ISSUE,
            &certificate,
            &PlanOptions::default(),
        )
        .expect("the demo issue yields a plan")
    })
}

pub fn repair_plan() -> Value {
    repair_plan_struct()
        .to_json()
        .expect("the repair plan serialises")
}

pub fn repair_acceptance_report() -> Value {
    static REPORT: OnceLock<Value> = OnceLock::new();
    REPORT
        .get_or_init(|| {
            let (world, _, _) = compiled_issue();
            verify_repair(repair_plan_struct(), &world).to_json()
        })
        .clone()
}

/// The offsets of the twelve hex characters `RepairPlan` truncates its body digest to.
///
/// The plan seals itself with `repair-<issue id>-<first twelve hex digits of the body digest>`, so
/// there is no 64-character field for `mutators::digest_pointers` to find by shape. Every battery
/// assertion about digest offsets on this document therefore names this range instead, and says
/// so: twelve hex characters is forty-eight bits, not two hundred and fifty-six.
pub fn repair_plan_id_digest_span() -> (usize, usize) {
    let plan_id = repair_plan()["plan_id"]
        .as_str()
        .expect("the plan carries its id")
        .to_string();
    let start = plan_id.len() - 12;
    (start, plan_id.len())
}

// -- domain workflow instantiation and portfolio ------------------------------------------------

pub fn workflow_catalogue() -> Value {
    json!([{
        "id": "oncology_workflows",
        "domains": ["oncology"],
        "crates": ["bioprism-onco"],
        "mcp_tools": ["onco_boundary_check"],
        "cli_entrypoints": [],
        "status": "available"
    }])
}

pub fn workflow_tool_definitions() -> Value {
    json!([{ "name": "onco_boundary_check", "inputSchema": { "type": "object" } }])
}

fn workflow_instantiate_request() -> Value {
    json!({
        "workflow_id": "oncology_workflows",
        "mission_id": "receipts-audit-1",
        "goal": "review the oncology boundary",
        "steps": [{ "id": "boundary", "tool": "onco_boundary_check", "arguments": {} }],
        "policy": { "execute": true }
    })
}

fn workflow_instantiation() -> &'static Value {
    static INSTANTIATION: OnceLock<Value> = OnceLock::new();
    INSTANTIATION.get_or_init(|| {
        instantiate_domain_workflow(
            &workflow_catalogue(),
            &workflow_tool_definitions(),
            &workflow_instantiate_request(),
        )
        .expect("the oncology workflow instantiates")
    })
}

/// The verification request a caller sends to re-check a retained instantiation.
///
/// The replay half is not optional here. Without `replay_request` the verifier says so in its own
/// limitations — current catalogue membership is not proof that the retained request is unchanged
/// — so a battery over a replay-free request would be measuring a check the verifier never claimed
/// to perform.
pub fn domain_workflow_verification() -> Value {
    json!({
        "instantiation": workflow_instantiation().clone(),
        "replay_request": workflow_instantiate_request(),
    })
}

pub fn domain_workflow_portfolio_verification() -> Value {
    static PORTFOLIO: OnceLock<Value> = OnceLock::new();
    PORTFOLIO
        .get_or_init(|| {
            let portfolio = bioprism_devplat::build_domain_workflow_portfolio(
                &workflow_catalogue(),
                &workflow_tool_definitions(),
                &json!({
                    "portfolio_id": "receipts-audit-portfolio",
                    "requests": [workflow_instantiate_request()],
                    "policy": { "allow_partial": false },
                }),
            )
            .expect("the portfolio plans");
            json!({
                "portfolio": portfolio,
                "replay_requests": [workflow_instantiate_request()],
            })
        })
        .clone()
}

// -- workbench verification ---------------------------------------------------------------------

fn hashed(label: &str) -> String {
    ContentHash::of_bytes(label.as_bytes()).to_string()
}

fn studio_session() -> StudioSession {
    StudioSession {
        session_id: "session-1".into(),
        owner: "agent-a".into(),
        goal: "author a verified oncology capability card".into(),
        environment_digest: Some(hashed("env")),
        artifacts: vec![ArtifactCard {
            id: "artifact-1".into(),
            title: "verification card".into(),
            path: "artifacts/verification.json".into(),
            domain: "oncology".into(),
            capability: "verification".into(),
            state: ArtifactState::Validated,
            evidence: EvidencePosture::Reproduced,
            digest: Some(hashed("artifact")),
            score: Some(0.8),
            tags: vec!["public-card".into()],
        }],
        cells: vec![StudioCell {
            id: "cell-1".into(),
            kind: CellKind::Query,
            source: "workspace.metrics_analytics_audit(...)".into(),
            inputs: vec![CellInput {
                artifact_id: "artifact-1".into(),
                digest: hashed("artifact"),
            }],
            depends_on: Vec::new(),
            executed: true,
            output_digest: Some(hashed("output")),
        }],
        changes: vec![StudioChange {
            id: "change-1".into(),
            artifact_id: "artifact-1".into(),
            kind: ChangeKind::Create,
            actor: "agent-a".into(),
            logical_time: 1,
            input_digest: None,
            output_digest: Some(hashed("artifact")),
            reason: "initial authored artifact".into(),
        }],
        policy: NotebookPolicy::default(),
    }
}

fn ci_request() -> CiRequest {
    CiRequest {
        workflow: "consumer contracts".into(),
        triggers: vec!["push".into(), "pull_request".into()],
        rust_toolchain: "1.85.0".into(),
        checks: vec![CiCheck {
            name: "workspace tests".into(),
            run: "cargo test --workspace --offline".into(),
            working_directory: Some("crates/devplat".into()),
            required: true,
        }],
        offline: true,
    }
}

/// A workbench verification request whose retained report is sealed by `expected_report_digest`.
///
/// The digest is what makes this a battery subject at all: without it the request declares no
/// identity for the report it carries, and every mutation of the report would be a mutation of an
/// unsealed value.
pub fn workbench_verification() -> Value {
    static REQUEST: OnceLock<Value> = OnceLock::new();
    REQUEST
        .get_or_init(|| {
            let report = run_workbench(&WorkbenchRequest {
                session: studio_session(),
                dashboard: None,
                ci: Some(ci_request()),
            })
            .expect("the workbench run completes");
            let retained = serde_json::to_value(&report).expect("the workbench report serialises");
            let request = WorkbenchVerificationRequest {
                session: studio_session(),
                expected_report_digest: Some(canonical_digest(&retained)),
                report,
                ci_replay: Some(ci_request()),
                policy: WorkbenchVerificationPolicy {
                    require_dashboard: false,
                    require_ci: true,
                    require_ci_replay: true,
                },
            };
            serde_json::to_value(request).expect("the verification request serialises")
        })
        .clone()
}

// -- domain evidence provider replay ------------------------------------------------------------

fn provider_observation() -> DomainEvidenceProviderNormalizationRequest {
    DomainEvidenceProviderNormalizationRequest {
        group_id: "biological_domains".into(),
        domains: vec!["oncology".into()],
        subject_id: "subject-1".into(),
        source_tool: "literature_bind_check".into(),
        connector_kind: "literature".into(),
        provider: "pubmed".into(),
        payload: json!({ "records": [{ "id": "pmid:1", "title": "opaque" }] }),
        request: Some(json!({ "query": "oncology" })),
        outcome: "observed".into(),
        claim_posture: json!({
            "status": "review_required",
            "does_not_claim": [
                "provider authenticity",
                "scientific or clinical validity",
                "provenance completeness",
                "execution or external effect"
            ]
        }),
        parent_digests: vec!["a".repeat(64)],
        source_plan_digest: Some("b".repeat(64)),
    }
}

pub fn provider_replay_request() -> Value {
    static REQUEST: OnceLock<Value> = OnceLock::new();
    REQUEST
        .get_or_init(|| {
            let observation = provider_observation();
            let normalized = bioprism_devplat::normalize_domain_evidence_provider(&observation)
                .expect("the observation normalizes");
            let normalization_value =
                serde_json::to_value(&normalized).expect("the normalization serialises");
            let intake = bioprism_devplat::intake_domain_evidence(&normalized.intake_arguments)
                .expect("the normalized arguments intake");
            let request = DomainEvidenceProviderReplayRequest {
                expected_payload_digest: normalized.payload_digest.clone(),
                expected_request_digest: normalized.request_digest.clone(),
                expected_shape_digest: normalized.shape_audit.shape_digest.clone(),
                expected_normalization_digest: canonical_digest(&normalization_value),
                expected_intake_digest: intake["intake_digest"]
                    .as_str()
                    .expect("the intake carries its digest")
                    .to_string(),
                observation,
            };
            serde_json::to_value(request).expect("the replay request serialises")
        })
        .clone()
}

// -- external payload replay --------------------------------------------------------------------

fn external_receipt_request() -> DomainEvidenceProviderExternalPayloadReceiptRequest {
    DomainEvidenceProviderExternalPayloadReceiptRequest {
        group_id: "biological_domains".into(),
        domains: vec!["genomics".into(), "oncology".into()],
        subject_id: "subject-1".into(),
        source_tool: "literature_bind_check".into(),
        provider: "pubmed".into(),
        connector_kind: "literature".into(),
        handoff_digest: "a".repeat(64),
        transfer_id: "transfer-1".into(),
        payload_digest: "b".repeat(64),
        byte_length: 4096,
        storage_backend: "object_store".into(),
        locator_kind: "opaque".into(),
        locator: "store://caller/pubmed/objects/1".into(),
        content_type: Some("application/json".into()),
        content_encoding: Some("gzip".into()),
        request_digest: Some("c".repeat(64)),
        parent_digests: vec!["d".repeat(64)],
        availability: "available".into(),
        retention: "durable".into(),
        attempt_id: Some("attempt-1".into()),
    }
}

pub fn external_payload_replay_request() -> Value {
    static REQUEST: OnceLock<Value> = OnceLock::new();
    REQUEST
        .get_or_init(|| {
            let receipt = external_receipt_request();
            let recorded = record_domain_evidence_provider_external_payload(&receipt)
                .expect("the external payload receipt records");
            let request = DomainEvidenceProviderExternalPayloadReplayRequest {
                expected_receipt_digest: recorded.receipt_digest.clone(),
                expected_handoff_digest: recorded.handoff_digest.clone(),
                expected_payload_digest: recorded.payload_digest.clone(),
                expected_byte_length: recorded.byte_length,
                receipt,
            };
            serde_json::to_value(request).expect("the replay request serialises")
        })
        .clone()
}

/// Every new document as a name and a value, for the cross-document confusion sweep.
pub fn library() -> Vec<(&'static str, Value)> {
    let mut all: Vec<(&'static str, Value)> = vec![
        ("prism_result_bundle", prism_result_bundle()),
        ("registry_pack", registry_pack()),
        ("conformance_certificate", conformance_certificate()),
        ("cookbook_report", cookbook_report()),
        ("bioworlds_catalog_report", bioworlds_catalog_report()),
        ("examples_registry_report", examples_registry_report()),
        ("repair_plan", repair_plan()),
        ("repair_acceptance_report", repair_acceptance_report()),
        (
            "domain_workflow_verification",
            domain_workflow_verification(),
        ),
        (
            "domain_workflow_portfolio_verification",
            domain_workflow_portfolio_verification(),
        ),
        ("workbench_verification", workbench_verification()),
        ("provider_replay_request", provider_replay_request()),
        (
            "external_payload_replay_request",
            external_payload_replay_request(),
        ),
    ];
    all.sort_by_key(|(name, _)| *name);
    all
}
