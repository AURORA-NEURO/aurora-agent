//! The `bioprism` command-line interface.
//!
//! Implements the local-first slice of blueprint 40.13. Four invariants from that contract are
//! enforced here rather than left to convention:
//!
//! * `--json` emits exactly one document on stdout and nothing else, so it is pipeable;
//! * `--dry-run` performs no filesystem writes;
//! * every command prints a reproducible follow-up command in human mode;
//! * every failure maps to a documented exit code (see [`exit::ExitCode`]).
//!
//! # Not implemented
//!
//! * **`--domain` on `context compare`.** The comparison harness judges every strategy against
//!   the reference split-integrity oracle directly rather than through the injectable
//!   [`DecisionOracle`], so a pack oracle cannot yet be applied to the whole panel. The flag is
//!   refused with that reason (exit 2) rather than accepted and half-applied.
//! * **`world sweep` sweeps only the structural axes** (attachment, relay depth, tag style,
//!   distractor count, seed). The decision-defining knobs are deliberately fixed, as
//!   `bioprism_baseline::sweep` documents.

mod args;
mod exit;
mod explain;
mod io;

use args::{Command, CompileOptions, Family, GenerateOptions, Invocation, Parsed, Profile};
use bioprism_autopilot::{
    drive::instantiation_mission, drive_instantiation, preview_first_action,
    verify_autopilot_report, AutonomyGrant, AutonomyGrantDocument, FinalStatus, NextAction,
    RetryPolicyDocument, RetryScheduleDocument,
};
use bioprism_devplat::{
    audit_domain_decision_readiness, build_domain_workflow_catalogue,
    build_domain_workflow_portfolio, instantiate_domain_workflow, reconcile_domain_workflow,
    scaffold_domain_workflow, verify_domain_workflow_portfolio, verify_workbench, ArtifactRegistry,
    CiProviderEvidenceRegistry, WorkbenchReportRegistry, WorkbenchVerificationRequest,
};
use bioprism_devplat::{
    verify_mission_evidence_bundle, DomainWorkflowReconciliationRegistry, EvidenceBundleRegistry,
};
use bioprism_domain::DomainPack;
use bioprism_fiber::{compile, compile_with_oracle, DecisionOracle, Query};
use bioprism_mcp::{tool_definitions, workspace_capabilities, Server};
use bioprism_scope::DimensionRegistry;
use bioprism_section::{CertificateProfile, ContextCertificate, OracleStatus};
use bioprism_world::{validate, Severity};
use exit::{CliError, CliResult, ExitCode};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;

fn main() {
    std::thread::Builder::new()
        .name("bioprism-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(main_inner)
        .expect("bioprism CLI worker thread should start")
        .join()
        .expect("bioprism CLI worker thread should finish");
}

fn main_inner() {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    let parsed = match args::parse(raw) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("error [{}]: {}", error.code.slug(), error.message);
            eprintln!("\nRun `bioprism --help` for usage.");
            std::process::exit(error.code.as_i32());
        }
    };

    let invocation = match parsed {
        Parsed::Help => {
            print!("{}", args::help());
            std::process::exit(ExitCode::Ok.as_i32());
        }
        Parsed::Version => {
            println!("bioprism {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(ExitCode::Ok.as_i32());
        }
        Parsed::Run(invocation) => invocation,
    };

    let json_mode = invocation.json;
    match run(&invocation) {
        Ok(outcome) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&outcome.document).expect("serialisable")
                );
            } else {
                print!("{}", outcome.human);
            }
            std::process::exit(outcome.code.as_i32());
        }
        Err(error) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&error.to_json()).expect("serialisable")
                );
            } else {
                match &error.subject {
                    Some(subject) => {
                        eprintln!(
                            "error [{}]: {}: {}",
                            error.code.slug(),
                            subject,
                            error.message
                        )
                    }
                    None => eprintln!("error [{}]: {}", error.code.slug(), error.message),
                }
            }
            std::process::exit(error.code.as_i32());
        }
    }
}

struct Outcome {
    code: ExitCode,
    document: Value,
    human: String,
}

impl Outcome {
    fn ok(document: Value, human: String) -> Self {
        Outcome {
            code: ExitCode::Ok,
            document,
            human,
        }
    }

    fn failing_if(mut self, condition: bool) -> Self {
        if condition {
            self.code = ExitCode::AssertionFailed;
        }
        self
    }
}

fn run(invocation: &Invocation) -> CliResult<Outcome> {
    match &invocation.command {
        Command::WorldValidate { world, dimensions } => {
            world_validate(world, dimensions.as_deref())
        }
        Command::WorldShow { world } => world_show(world),
        Command::WorldSweep {
            distractors,
            seed,
            markdown,
        } => world_sweep(distractors.as_deref(), *seed, *markdown),
        Command::WorldGenerate(options) => world_generate(options),
        Command::WorldIndex {
            world,
            store,
            dry_run,
        } => world_index(world, store, *dry_run),
        Command::PrismFork {
            world,
            query,
            bundle_out,
            minimize,
        } => prism_fork(world, query, bundle_out.as_deref(), *minimize),
        Command::PrismMinimize { world } => prism_minimize(world),
        Command::MutateFamily { world, out_dir } => mutate_family(world, out_dir.as_deref()),
        Command::ContextExplain {
            world,
            query,
            domain,
        } => context_explain(world, query, domain.as_deref()),
        Command::ContextCompile(options) => context_compile(options),
        Command::ContextVerify { certificate } => context_verify(certificate),
        Command::ContextCompare {
            world,
            query,
            markdown,
        } => context_compare(world, query, *markdown),
        Command::EvidenceBundleVerify { bundle } => evidence_bundle_verify(bundle),
        Command::EvidenceBundleImport {
            bundle,
            store,
            dry_run,
        } => evidence_bundle_import(bundle, store, *dry_run),
        Command::EvidenceBundleQuery {
            store,
            mission_id,
            domain,
            after,
            limit,
            include_bundles,
        } => evidence_bundle_query(
            store,
            mission_id.as_deref(),
            domain.as_deref(),
            after.as_deref(),
            *limit,
            *include_bundles,
        ),
        Command::EvidenceDomainLineage {
            store,
            digest,
            group_id,
            domain,
            subject_id,
            source_tool,
            outcome,
            request_digest,
            response_digest,
            intake_digest,
            source_plan_digest,
            after,
            limit,
            include_children,
        } => evidence_domain_lineage(
            store,
            digest.as_deref(),
            group_id.as_deref(),
            domain.as_deref(),
            subject_id.as_deref(),
            source_tool.as_deref(),
            outcome.as_deref(),
            request_digest.as_deref(),
            response_digest.as_deref(),
            intake_digest.as_deref(),
            source_plan_digest.as_deref(),
            after.as_deref(),
            *limit,
            *include_children,
        ),
        Command::ReadinessAudit { request } => readiness_audit(request),
        Command::ReadinessQuery {
            store,
            subject_id,
            decision_state,
            policy_satisfied,
            after,
            limit,
            include_audits,
        } => readiness_query(
            store,
            subject_id.as_deref(),
            decision_state.as_deref(),
            *policy_satisfied,
            after.as_deref(),
            *limit,
            *include_audits,
        ),
        Command::WorkflowCatalogue => workflow_catalogue(),
        Command::WorkflowScaffold {
            workflow,
            mission_id,
            goal,
            tools,
            arguments,
        } => workflow_scaffold(
            workflow,
            mission_id,
            goal,
            tools.as_deref(),
            arguments.as_deref(),
        ),
        Command::WorkflowInstantiate {
            workflow,
            mission_id,
            goal,
            steps,
            policy,
            dry_run,
        } => workflow_instantiate(
            workflow,
            mission_id,
            goal,
            steps,
            policy.as_deref(),
            *dry_run,
        ),
        Command::WorkflowPortfolio {
            requests,
            policy,
            readiness_audit,
            allow_partial,
            require_complete_catalogue,
            require_readiness,
        } => workflow_portfolio(
            requests,
            policy.as_deref(),
            readiness_audit.as_deref(),
            *allow_partial,
            *require_complete_catalogue,
            *require_readiness,
        ),
        Command::WorkflowPortfolioVerify {
            portfolio,
            replay_requests,
            policy,
            readiness_audit,
            allow_partial,
            require_complete_catalogue,
            require_replay,
            require_readiness,
        } => workflow_portfolio_verify(
            portfolio,
            replay_requests.as_deref(),
            policy.as_deref(),
            readiness_audit.as_deref(),
            *allow_partial,
            *require_complete_catalogue,
            *require_replay,
            *require_readiness,
        ),
        Command::WorkbenchVerify {
            session,
            report,
            ci_replay,
            policy,
            expected_report_digest,
        } => workbench_verify(
            session,
            report,
            ci_replay.as_deref(),
            policy.as_deref(),
            expected_report_digest.as_deref(),
        ),
        Command::WorkbenchImport {
            report,
            store,
            dry_run,
        } => workbench_import(report, store, *dry_run),
        Command::WorkbenchQuery {
            store,
            session_digest,
            domain,
            capability,
            state,
            release_ready,
            after,
            limit,
            include_reports,
        } => workbench_query(
            store,
            session_digest.as_deref(),
            domain.as_deref(),
            capability.as_deref(),
            state.as_deref(),
            *release_ready,
            after.as_deref(),
            *limit,
            *include_reports,
        ),
        Command::WorkbenchGet { store, digest } => workbench_get(store, digest),
        Command::CiProviderEvidenceImport {
            request,
            store,
            dry_run,
        } => ci_provider_evidence_import(request, store, *dry_run),
        Command::CiProviderEvidenceQuery {
            store,
            provider,
            run_id,
            plan_digest,
            structurally_valid,
            conformance_ready,
            after,
            limit,
            include_records,
        } => ci_provider_evidence_query(
            store,
            provider.as_deref(),
            run_id.as_deref(),
            plan_digest.as_deref(),
            *structurally_valid,
            *conformance_ready,
            after.as_deref(),
            *limit,
            *include_records,
        ),
        Command::CiProviderEvidenceGet { store, digest } => ci_provider_evidence_get(store, digest),
        Command::WorkflowReconcile {
            instantiation,
            mission,
            evidence_bundle,
            policy,
            readiness_audit,
            require_readiness,
        } => workflow_reconcile(
            instantiation,
            mission.as_deref(),
            evidence_bundle.as_deref(),
            policy.as_deref(),
            readiness_audit.as_deref(),
            *require_readiness,
        ),
        Command::WorkflowReconciliationImport {
            record,
            store,
            dry_run,
        } => workflow_reconciliation_import(record, store, *dry_run),
        Command::WorkflowReconciliationQuery {
            store,
            mission_id,
            workflow_id,
            mission_plan_digest,
            completion_status,
            decision_readiness_state,
            decision_readiness_gate_satisfied,
            after,
            limit,
            include_records,
        } => workflow_reconciliation_query(
            store,
            mission_id.as_deref(),
            workflow_id.as_deref(),
            mission_plan_digest.as_deref(),
            completion_status.as_deref(),
            decision_readiness_state.as_deref(),
            *decision_readiness_gate_satisfied,
            after.as_deref(),
            *limit,
            *include_records,
        ),
        Command::AutopilotGrantTemplate => autopilot_grant_template(),
        Command::AutopilotRun {
            instantiation,
            grant,
            report_out,
            dry_run,
        } => autopilot_run(instantiation, grant, report_out.as_deref(), *dry_run),
        Command::AutopilotVerify { report } => autopilot_verify(report),
    }
}

fn workbench_verify(
    session_path: &Path,
    report_path: &Path,
    ci_replay_path: Option<&Path>,
    policy_path: Option<&Path>,
    expected_report_digest: Option<&str>,
) -> CliResult<Outcome> {
    let mut request = serde_json::json!({
        "session": io::read_json(session_path)?,
        "report": io::read_json(report_path)?,
    });
    if let Some(path) = ci_replay_path {
        request["ci_replay"] = io::read_json(path)?;
    }
    if let Some(path) = policy_path {
        request["policy"] = io::read_json(path)?;
    }
    if let Some(digest) = expected_report_digest {
        request["expected_report_digest"] = serde_json::json!(digest);
    }
    let typed: WorkbenchVerificationRequest = serde_json::from_value(request).map_err(|error| {
        CliError::invalid(format!("invalid workbench verification request: {error}"))
            .about(report_path.display().to_string())
    })?;
    let report = verify_workbench(&typed).map_err(|error| {
        CliError::invalid(error.to_string()).about(report_path.display().to_string())
    })?;
    let valid = report.valid;
    let status = report.status.clone();
    let mismatches = report.mismatches.len();
    let document =
        serde_json::to_value(report).map_err(|error| CliError::internal(error.to_string()))?;
    let human = format!(
        "developer workbench verification\n  status: {status}\n  valid: {valid}\n  mismatches: {mismatches}\n  dashboard replay: {}\n  CI replay: {}\n  execution: not started\n  network access: not started\n\nNext: inspect the verification digest and mismatch witnesses before any separate execution or CI handoff.\n",
        document["dashboard_verified"].as_bool().unwrap_or(false),
        document["ci_verified"].as_bool().unwrap_or(false),
    );
    Ok(Outcome::ok(document, human).failing_if(!valid))
}

fn load_workbench_registry(store_path: &Path) -> CliResult<WorkbenchReportRegistry> {
    if !store_path.exists() {
        return Ok(WorkbenchReportRegistry::new());
    }
    let snapshot = io::read_json(store_path)?;
    WorkbenchReportRegistry::from_snapshot(&snapshot).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })
}

fn workbench_import(report_path: &Path, store_path: &Path, dry_run: bool) -> CliResult<Outcome> {
    let report = io::read_json(report_path)?;
    let mut registry = load_workbench_registry(store_path)?;
    let result = registry.import(&report).map_err(|error| {
        CliError::invalid(error.to_string()).about(report_path.display().to_string())
    })?;
    let snapshot = registry.snapshot().map_err(|error| {
        CliError::internal(error.to_string()).about(store_path.display().to_string())
    })?;
    let artifact = if result.get("created").and_then(Value::as_bool) == Some(true) {
        Some(io::write_artifact(store_path, &snapshot, dry_run)?)
    } else {
        None
    };
    let mut document = result;
    document["store"] = json!(store_path.display().to_string());
    document["report"] = json!(report_path.display().to_string());
    document["dry_run"] = json!(dry_run);
    document["state_digest"] = snapshot.get("state_digest").cloned().unwrap_or(Value::Null);
    document["artifact"] = artifact
        .as_ref()
        .map(|value| {
            json!({
                "path": value.path.display().to_string(),
                "bytes": value.bytes,
                "written": value.written
            })
        })
        .unwrap_or(Value::Null);
    let digest = document
        .get("workbench_report_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let human = format!(
        "developer workbench report {}\n  digest: {}\n  registry: {} (generation {})\n  state: {}\n\nNext: bioprism workbench query --store {}\n",
        if document.get("created").and_then(Value::as_bool) == Some(true) {
            if dry_run { "planned for import" } else { "imported" }
        } else {
            "already present"
        },
        digest,
        store_path.display(),
        document
            .get("registry_generation")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        if dry_run { "not written (dry run)" } else { "checkpoint updated" },
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

#[allow(clippy::too_many_arguments)]
fn workbench_query(
    store_path: &Path,
    session_digest: Option<&str>,
    domain: Option<&str>,
    capability: Option<&str>,
    state: Option<&str>,
    release_ready: bool,
    after: Option<&str>,
    limit: usize,
    include_reports: bool,
) -> CliResult<Outcome> {
    let registry = load_workbench_registry(store_path)?;
    let report = registry
        .query(
            session_digest,
            domain,
            capability,
            state,
            release_ready.then_some(true),
            after,
            limit,
            include_reports,
        )
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let next_after = report
        .get("next_after")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    let human = format!(
        "developer workbench registry query\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n\nNext: bioprism workbench query --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        next_after,
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

fn workbench_get(store_path: &Path, digest: &str) -> CliResult<Outcome> {
    let registry = load_workbench_registry(store_path)?;
    let report = registry.get_response(digest).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })?;
    let human = format!(
        "developer workbench report\n  digest: {}\n  store: {}\n  execution: not started\n",
        digest,
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

fn load_ci_provider_evidence_registry(store_path: &Path) -> CliResult<CiProviderEvidenceRegistry> {
    if !store_path.exists() {
        return Ok(CiProviderEvidenceRegistry::new());
    }
    let snapshot = io::read_json(store_path)?;
    CiProviderEvidenceRegistry::from_snapshot(&snapshot).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })
}

fn load_artifact_registry(store_path: &Path) -> CliResult<ArtifactRegistry> {
    if !store_path.exists() {
        return Ok(ArtifactRegistry::new());
    }
    let snapshot = io::read_json(store_path)?;
    ArtifactRegistry::from_snapshot(&snapshot).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })
}

fn readiness_audit(request_path: &Path) -> CliResult<Outcome> {
    let request = io::read_json(request_path)?;
    let audit = audit_domain_decision_readiness(&request).map_err(|error| {
        CliError::invalid(error.to_string()).about(request_path.display().to_string())
    })?;
    let policy_satisfied = audit
        .get("policy_satisfied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let human = format!(
        "domain decision-readiness audit\n  subject: {}\n  state: {}\n  policy satisfied: {}\n  audit digest: {}\n  execution: not started\n\nCatalogue binding and artifact retention are transport responsibilities.\n",
        audit.get("subject_id").and_then(Value::as_str).unwrap_or("unknown"),
        audit
            .get("decision_state")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        policy_satisfied,
        audit.get("digest").and_then(Value::as_str).unwrap_or("<missing>"),
    );
    Ok(Outcome::ok(audit, human).failing_if(!policy_satisfied))
}

fn readiness_query(
    store_path: &Path,
    subject_id: Option<&str>,
    decision_state: Option<&str>,
    policy_satisfied: Option<bool>,
    after: Option<&str>,
    limit: usize,
    include_audits: bool,
) -> CliResult<Outcome> {
    let registry = load_artifact_registry(store_path)?;
    let report = registry
        .domain_decision_readiness_query(
            subject_id,
            decision_state,
            policy_satisfied,
            after,
            limit,
            include_audits,
        )
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let next_after = report
        .get("next_after")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    let human = format!(
        "domain decision-readiness registry query\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n\nNext: bioprism readiness query --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        next_after,
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

#[allow(clippy::too_many_arguments)]
fn evidence_domain_lineage(
    store_path: &Path,
    digest: Option<&str>,
    group_id: Option<&str>,
    domain: Option<&str>,
    subject_id: Option<&str>,
    source_tool: Option<&str>,
    outcome: Option<&str>,
    request_digest: Option<&str>,
    response_digest: Option<&str>,
    intake_digest: Option<&str>,
    source_plan_digest: Option<&str>,
    after: Option<&str>,
    limit: usize,
    include_children: bool,
) -> CliResult<Outcome> {
    let registry = load_artifact_registry(store_path)?;
    let mut request = json!({
        "max_items": limit,
        "include_children": include_children
    });
    let fields = [
        ("content_digest", digest),
        ("group_id", group_id),
        ("domain", domain),
        ("subject_id", subject_id),
        ("source_tool", source_tool),
        ("outcome", outcome),
        ("request_digest", request_digest),
        ("response_digest", response_digest),
        ("intake_digest", intake_digest),
        ("source_plan_digest", source_plan_digest),
        ("after", after),
    ];
    for (name, value) in fields {
        if let Some(value) = value {
            request[name] = json!(value);
        }
    }
    let report = registry
        .domain_evidence_lineage(&request)
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let human = format!(
        "domain evidence lineage\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n  execution: not started\n\nNext: bioprism evidence domain-lineage --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        report
            .get("next_after")
            .and_then(Value::as_str)
            .unwrap_or("<none>"),
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

fn ci_provider_evidence_import(
    request_path: &Path,
    store_path: &Path,
    dry_run: bool,
) -> CliResult<Outcome> {
    let request = io::read_json(request_path)?;
    let mut registry = load_ci_provider_evidence_registry(store_path)?;
    let result = registry.import(&request).map_err(|error| {
        CliError::invalid(error.to_string()).about(request_path.display().to_string())
    })?;
    let snapshot = registry.snapshot().map_err(|error| {
        CliError::internal(error.to_string()).about(store_path.display().to_string())
    })?;
    let artifact = if result.get("created").and_then(Value::as_bool) == Some(true) {
        Some(io::write_artifact(store_path, &snapshot, dry_run)?)
    } else {
        None
    };
    let mut document = result;
    document["store"] = json!(store_path.display().to_string());
    document["request"] = json!(request_path.display().to_string());
    document["dry_run"] = json!(dry_run);
    document["state_digest"] = snapshot.get("state_digest").cloned().unwrap_or(Value::Null);
    document["artifact"] = artifact
        .as_ref()
        .map(|value| {
            json!({
                "path": value.path.display().to_string(),
                "bytes": value.bytes,
                "written": value.written
            })
        })
        .unwrap_or(Value::Null);
    let digest = document
        .get("provider_evidence_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let human = format!(
        "CI provider evidence {}\n  digest: {}\n  provider: {}\n  run: {}\n  conformance ready: {}\n  lineage rows: artifacts={} logs={} attestations={}\n  registry: {} (generation {})\n  state: {}\n\nNext: bioprism ci provider-evidence-query --store {}\n",
        if document.get("created").and_then(Value::as_bool) == Some(true) {
            if dry_run { "planned for import" } else { "imported" }
        } else {
            "already present"
        },
        digest,
        document.get("provider").and_then(Value::as_str).unwrap_or("<unknown>"),
        document.get("run_id").and_then(Value::as_str).unwrap_or("<unknown>"),
        document
            .get("conformance_ready")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        document.get("artifact_count").and_then(Value::as_u64).unwrap_or(0),
        document.get("log_count").and_then(Value::as_u64).unwrap_or(0),
        document
            .get("attestation_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        store_path.display(),
        document
            .get("registry_generation")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        if dry_run { "not written (dry run)" } else { "checkpoint updated" },
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

#[allow(clippy::too_many_arguments)]
fn ci_provider_evidence_query(
    store_path: &Path,
    provider: Option<&str>,
    run_id: Option<&str>,
    plan_digest: Option<&str>,
    structurally_valid: bool,
    conformance_ready: bool,
    after: Option<&str>,
    limit: usize,
    include_records: bool,
) -> CliResult<Outcome> {
    let registry = load_ci_provider_evidence_registry(store_path)?;
    let report = registry
        .query(
            provider,
            run_id,
            plan_digest,
            structurally_valid.then_some(true),
            conformance_ready.then_some(true),
            None,
            None,
            None,
            after,
            limit,
            include_records,
        )
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let human = format!(
        "CI provider evidence registry query\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n\nNext: bioprism ci provider-evidence-query --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        report
            .get("next_after")
            .and_then(Value::as_str)
            .unwrap_or("<none>"),
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

fn ci_provider_evidence_get(store_path: &Path, digest: &str) -> CliResult<Outcome> {
    let registry = load_ci_provider_evidence_registry(store_path)?;
    let report = registry.get(digest).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })?;
    let human = format!(
        "CI provider evidence report\n  digest: {}\n  provider: {}\n  run: {}\n  store: {}\n  execution: not started\n",
        digest,
        report.get("provider").and_then(Value::as_str).unwrap_or("<unknown>"),
        report.get("run_id").and_then(Value::as_str).unwrap_or("<unknown>"),
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

fn workflow_catalogue() -> CliResult<Outcome> {
    let report = build_domain_workflow_catalogue(
        &workspace_capabilities(),
        &Value::Array(tool_definitions()),
    )
    .map_err(|error| CliError::internal(error.to_string()))?;
    let human = format!(
        "domain workflow catalogue\n  workflows: {}\n  groups with missing tools: {}\n  domain contracts: {}\n  execution: not started\n\nNext: bioprism workflow instantiate --workflow <id> --mission-id <id> --goal <text> --steps steps.json\n",
        report["workflow_count"].as_u64().unwrap_or_default(),
        report["coverage"]["groups_with_missing_tools"]
            .as_u64()
            .unwrap_or_default(),
        report["coverage"]["all_workflows_have_domain_contract"]
            .as_bool()
            .unwrap_or(false),
    );
    Ok(Outcome::ok(report, human))
}

fn workflow_scaffold(
    workflow: &str,
    mission_id: &str,
    goal: &str,
    tools_path: Option<&Path>,
    arguments_path: Option<&Path>,
) -> CliResult<Outcome> {
    let mut request = json!({
        "workflow_id": workflow,
        "mission_id": mission_id,
        "goal": goal,
    });
    if let Some(path) = tools_path {
        let raw = io::read_json(path)?;
        request["tools"] = raw
            .get("tools")
            .cloned()
            .filter(Value::is_array)
            .unwrap_or(raw);
    }
    if let Some(path) = arguments_path {
        request["arguments"] = io::read_json(path)?;
    }
    let mut report = scaffold_domain_workflow(
        &workspace_capabilities(),
        &Value::Array(tool_definitions()),
        &request,
    )
    .map_err(|error| CliError::invalid(error.to_string()))?;
    let server = Server::new(
        std::env::current_dir().map_err(|error| CliError::internal(error.to_string()))?,
    );
    match server.preflight_agent_mission(&report["mission"]) {
        Ok(preflight) => {
            report["preflight_status"] = json!("ready");
            report["preflight_report"] = preflight;
        }
        Err(error) => {
            report["preflight_status"] = json!("blocked");
            report["preflight_report"] = json!({
                "ok": false,
                "workflow": "agent_mission",
                "preflight": true,
                "dispatch": "not_started",
                "schema_valid": false,
                "error": error,
                "readiness_claimed": false,
            });
        }
    }
    report["cli"] = json!({"execution": "not_started", "writes": false});
    let selected = report["selection"]["selected_tools"]
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    let status = report["preflight_status"].as_str().unwrap_or("blocked");
    let human = format!(
        "domain workflow scaffold {}\n  mission: {}\n  selected tools: {}\n  preflight: {}\n  readiness claimed: false\n  execution: not started\n\nNext: review the scaffold, complete authoritative arguments, then run mission preflight before any explicit execution path.\n",
        workflow, mission_id, selected, status,
    );
    Ok(Outcome::ok(report, human))
}

fn workflow_instantiate(
    workflow: &str,
    mission_id: &str,
    goal: &str,
    steps_path: &Path,
    policy_path: Option<&Path>,
    dry_run: bool,
) -> CliResult<Outcome> {
    let raw_steps = io::read_json(steps_path)?;
    let steps = raw_steps
        .get("steps")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or(raw_steps);
    if !steps.is_array() {
        return Err(CliError::invalid(
            "--steps must contain a JSON array or an object with a steps array",
        )
        .about(steps_path.display().to_string()));
    }
    let policy = policy_path.map(io::read_json).transpose()?;
    let mut request = json!({
        "workflow_id": workflow,
        "mission_id": mission_id,
        "goal": goal,
        "steps": steps,
    });
    if let Some(policy) = policy {
        request["policy"] = policy;
    }
    let mut report = instantiate_domain_workflow(
        &workspace_capabilities(),
        &Value::Array(tool_definitions()),
        &request,
    )
    .map_err(|error| CliError::invalid(error.to_string()))?;
    let server = Server::new(
        std::env::current_dir().map_err(|error| CliError::internal(error.to_string()))?,
    );
    let preflight = server
        .preflight_agent_mission(&report["mission"])
        .map_err(|error| {
            CliError::invalid(format!("authoritative mission preflight refused: {error}"))
        })?;
    report["preflight_report"] = preflight;
    report["dry_run"] = json!(dry_run);
    let human = format!(
        "domain workflow {}\n  mission: {}\n  steps: {}\n  evidence plan: per-step\n  preflight: authoritative no-dispatch\n  execution: not started\n\nNext: POST /v1/missions/preflight or `bioprism workflow instantiate --workflow {} --mission-id <id> --goal <text> --steps <path>`\n",
        workflow,
        mission_id,
        report["selection"]["step_count"].as_u64().unwrap_or_default(),
        workflow,
    );
    Ok(Outcome::ok(report, human))
}

fn workflow_portfolio(
    requests_path: &Path,
    policy_path: Option<&Path>,
    readiness_audit_path: Option<&Path>,
    allow_partial: bool,
    require_complete_catalogue: bool,
    require_readiness: bool,
) -> CliResult<Outcome> {
    let raw = io::read_json(requests_path)?;
    let mut arguments = if raw.is_array() {
        json!({"requests": raw})
    } else {
        raw
    };
    if !arguments.is_object() {
        return Err(CliError::invalid(
            "--requests must contain a JSON array or an object with a requests array",
        )
        .about(requests_path.display().to_string()));
    }
    let mut policy = arguments
        .get("policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(path) = policy_path {
        policy = io::read_json(path)?;
    }
    if !policy.is_object() {
        return Err(
            CliError::invalid("--policy must contain a JSON object").about(
                policy_path
                    .map(Path::display)
                    .map(|display| display.to_string())
                    .unwrap_or_else(|| requests_path.display().to_string()),
            ),
        );
    }
    if allow_partial {
        policy["allow_partial"] = json!(true);
    }
    if require_complete_catalogue {
        policy["require_complete_catalogue"] = json!(true);
    }
    if require_readiness {
        policy["require_readiness"] = json!(true);
    }
    arguments["policy"] = policy;
    if let Some(path) = readiness_audit_path {
        arguments["readiness_audit"] = io::read_json(path)?;
    }

    let mut report = build_domain_workflow_portfolio(
        &workspace_capabilities(),
        &Value::Array(tool_definitions()),
        &arguments,
    )
    .map_err(|error| {
        CliError::invalid(error.to_string()).about(requests_path.display().to_string())
    })?;
    let server = Server::new(
        std::env::current_dir().map_err(|error| CliError::internal(error.to_string()))?,
    );
    let mut preflight_blocked_count = 0usize;
    if let Some(items) = report.get_mut("items").and_then(Value::as_array_mut) {
        for item in items.iter_mut() {
            if item.get("status").and_then(Value::as_str) != Some("instantiated") {
                continue;
            }
            let mission = item
                .pointer("/instantiation/mission")
                .cloned()
                .ok_or_else(|| CliError::internal("portfolio item omitted instantiated mission"))?;
            let preflight = match server.preflight_agent_mission(&mission) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "workflow": "agent_mission",
                    "preflight": true,
                    "dispatch": "not_started",
                    "schema_valid": false,
                    "error": error,
                    "fail_closed": true,
                    "readiness_claimed": false,
                }),
            };
            let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
            let observed_plan_digest = preflight
                .pointer("/plan/digest")
                .cloned()
                .unwrap_or(Value::Null);
            item["mission_preflight"] = json!({
                "status": if preflight_ok { "matched" } else { "blocked" },
                "matched": preflight_ok,
                "ok": preflight_ok,
                "observed_plan_digest": observed_plan_digest,
                "dispatch": "not_started",
            });
            item["instantiation"]["preflight_report"] = preflight.clone();
            if !preflight_ok {
                preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                item["status"] = json!("blocked_by_mission_preflight");
                item["issues"] = json!([{
                    "code": "mission_preflight_blocked",
                    "message": "authoritative mission schema preflight blocked this portfolio item",
                    "preflight": preflight,
                }]);
            }
        }
    }
    let kernel_valid = report
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let policy_allows_partial = report
        .pointer("/policy/allow_partial")
        .and_then(Value::as_bool)
        .unwrap_or(allow_partial);
    let valid = kernel_valid && preflight_blocked_count == 0;
    report["valid"] = json!(valid);
    report["portfolio_ready"] = json!(valid);
    report["summary"]["preflight_blocked_count"] = json!(preflight_blocked_count);
    report["summary"]["preflight_status"] = json!(if preflight_blocked_count == 0 {
        "matched"
    } else {
        "blocked"
    });
    report["preflight"] = json!({
        "required": true,
        "status": if preflight_blocked_count == 0 { "matched" } else { "blocked" },
        "matched": preflight_blocked_count == 0,
        "blocked_count": preflight_blocked_count,
        "dispatch": "not_started",
    });
    if preflight_blocked_count > 0 {
        report["portfolio_status"] = json!(if policy_allows_partial {
            "partial"
        } else {
            "blocked"
        });
    }
    report["dispatch"] = json!("not_started");
    report["execution"] = json!("not_started");
    if let Some(object) = report.as_object_mut() {
        object.remove("portfolio_digest");
    }
    let digest = bioprism_ids::ContentHash::of_value(&report)
        .map_err(|error| CliError::internal(error.to_string()))?;
    report["portfolio_digest"] = json!(digest.to_string());
    let item_count = report["items"].as_array().map(Vec::len).unwrap_or_default();
    let status = report["portfolio_status"].as_str().unwrap_or("blocked");
    let human = format!(
        "domain workflow portfolio\n  items: {}\n  catalogue complete: {}\n  preflight: {}\n  portfolio status: {}\n  portfolio ready: {}\n  dispatch: not started\n  execution: not started\n\nNext: complete blocked item arguments, review the portfolio digest, then run authoritative preflight before any explicit execution path.\n",
        item_count,
        report["coverage"]["complete_catalogue"].as_bool().unwrap_or(false),
        report["summary"]["preflight_status"].as_str().unwrap_or("blocked"),
        status,
        valid,
    );
    Ok(Outcome::ok(report, human).failing_if(!valid))
}

#[allow(clippy::too_many_arguments)]
fn workflow_portfolio_verify(
    portfolio_path: &Path,
    replay_requests_path: Option<&Path>,
    policy_path: Option<&Path>,
    readiness_audit_path: Option<&Path>,
    allow_partial: bool,
    require_complete_catalogue: bool,
    require_replay: bool,
    require_readiness: bool,
) -> CliResult<Outcome> {
    let raw_portfolio = io::read_json(portfolio_path)?;
    let mut portfolio =
        if raw_portfolio.get("portfolio").is_some() && raw_portfolio.get("workflow").is_none() {
            raw_portfolio
                .get("portfolio")
                .cloned()
                .ok_or_else(|| CliError::invalid("portfolio wrapper omitted portfolio"))?
        } else {
            raw_portfolio
        };
    if !portfolio.is_object() {
        return Err(
            CliError::invalid("--portfolio must contain a portfolio report object")
                .about(portfolio_path.display().to_string()),
        );
    }
    // REST adds request_id to response envelopes; it is transport metadata rather than part of
    // the content-addressed portfolio artifact and is safe to remove before digest verification.
    portfolio
        .as_object_mut()
        .expect("portfolio is an object")
        .remove("request_id");
    let mut request = json!({"portfolio": portfolio});
    if let Some(path) = replay_requests_path {
        let replay_requests = io::read_json(path)?;
        request["replay_requests"] = if replay_requests.is_array() {
            replay_requests
        } else {
            replay_requests
                .get("replay_requests")
                .cloned()
                .ok_or_else(|| {
                    CliError::invalid(
                        "--replay-requests must contain an array or an object with replay_requests",
                    )
                    .about(path.display().to_string())
                })?
        };
    }
    let mut policy = request["portfolio"]
        .get("policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(path) = policy_path {
        policy = io::read_json(path)?;
    }
    if !policy.is_object() {
        return Err(
            CliError::invalid("--policy must contain a JSON object").about(
                policy_path
                    .map(Path::display)
                    .map(|display| display.to_string())
                    .unwrap_or_else(|| portfolio_path.display().to_string()),
            ),
        );
    }
    if allow_partial {
        policy["allow_partial"] = json!(true);
    }
    if require_complete_catalogue {
        policy["require_complete_catalogue"] = json!(true);
    }
    if require_replay {
        policy["require_replay"] = json!(true);
    }
    if require_readiness {
        policy["require_readiness"] = json!(true);
    }
    request["policy"] = policy;
    if let Some(path) = readiness_audit_path {
        request["readiness_audit"] = io::read_json(path)?;
    }

    let mut report = verify_domain_workflow_portfolio(
        &workspace_capabilities(),
        &Value::Array(tool_definitions()),
        &request,
    )
    .map_err(|error| {
        CliError::invalid(error.to_string()).about(portfolio_path.display().to_string())
    })?;
    let server = Server::new(
        std::env::current_dir().map_err(|error| CliError::internal(error.to_string()))?,
    );
    let mut preflight_attempted_count = 0usize;
    let mut preflight_blocked_count = 0usize;
    if let Some(items) = report.get_mut("items").and_then(Value::as_array_mut) {
        for item in items.iter_mut() {
            if !matches!(
                item.get("status").and_then(Value::as_str),
                Some("verified") | Some("verified_without_replay")
            ) {
                continue;
            }
            let mission = item
                .pointer("/instantiation/mission")
                .cloned()
                .ok_or_else(|| CliError::internal("verified portfolio item omitted mission"))?;
            preflight_attempted_count = preflight_attempted_count.saturating_add(1);
            let preflight = match server.preflight_agent_mission(&mission) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "workflow": "agent_mission",
                    "preflight": true,
                    "dispatch": "not_started",
                    "schema_valid": false,
                    "error": error,
                    "fail_closed": true,
                    "readiness_claimed": false,
                }),
            };
            let preflight_ok = preflight.get("ok") == Some(&Value::Bool(true));
            let expected_plan_digest = item
                .pointer("/instantiation/preflight_report/plan/digest")
                .cloned()
                .unwrap_or(Value::Null);
            let observed_plan_digest = preflight
                .pointer("/plan/digest")
                .cloned()
                .unwrap_or(Value::Null);
            let mut item_mismatches = item
                .get("mismatches")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let preflight_status = if !preflight_ok {
                item_mismatches.push(json!({
                    "code": "mission_preflight_blocked",
                    "expected": expected_plan_digest,
                    "observed": observed_plan_digest,
                }));
                preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                item["status"] = json!("blocked_by_mission_preflight");
                "blocked"
            } else if expected_plan_digest.is_null() {
                item_mismatches.push(json!({
                    "code": "retained_preflight_missing",
                    "observed": observed_plan_digest,
                }));
                preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                item["status"] = json!("blocked_by_mission_preflight");
                "retained_projection_missing"
            } else if expected_plan_digest != observed_plan_digest {
                item_mismatches.push(json!({
                    "code": "mission_plan_digest_mismatch",
                    "expected": expected_plan_digest,
                    "observed": observed_plan_digest,
                }));
                preflight_blocked_count = preflight_blocked_count.saturating_add(1);
                item["status"] = json!("blocked_by_mission_preflight");
                "mismatched"
            } else {
                "matched"
            };
            item["mission_preflight"] = json!({
                "requested": true,
                "status": preflight_status,
                "matched": preflight_status == "matched",
                "ok": preflight_ok,
                "expected_plan_digest": expected_plan_digest,
                "observed_plan_digest": observed_plan_digest,
                "dispatch": "not_started",
            });
            item["verification"]["mission_preflight"] = item["mission_preflight"].clone();
            item["verification"]["preflight_report"] = preflight.clone();
            item["instantiation"]["preflight_report"] = preflight;
            item["mismatches"] = Value::Array(item_mismatches);
        }
    }
    let kernel_valid = report
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let valid = kernel_valid && preflight_blocked_count == 0;
    report["valid"] = json!(valid);
    report["portfolio_ready"] = json!(valid);
    let kernel_blocked_count = report
        .pointer("/summary/blocked_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    report["summary"]["blocked_count"] =
        json!(kernel_blocked_count.saturating_add(preflight_blocked_count));
    report["summary"]["preflight_attempted_count"] = json!(preflight_attempted_count);
    report["summary"]["preflight_blocked_count"] = json!(preflight_blocked_count);
    report["summary"]["preflight_status"] = json!(if preflight_blocked_count > 0 {
        "blocked"
    } else if preflight_attempted_count > 0 {
        "matched"
    } else {
        "deferred"
    });
    report["preflight"] = json!({
        "required": true,
        "status": if preflight_blocked_count > 0 {
            "blocked"
        } else if preflight_attempted_count > 0 {
            "matched"
        } else {
            "deferred"
        },
        "matched": preflight_blocked_count == 0 && preflight_attempted_count > 0,
        "attempted_count": preflight_attempted_count,
        "blocked_count": preflight_blocked_count,
        "dispatch": "not_started",
    });
    let mut mismatches = report
        .get("mismatches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if preflight_blocked_count > 0 {
        mismatches.push(json!({
            "code": "portfolio_mission_preflight_blocked",
            "blocked_count": preflight_blocked_count,
        }));
    }
    report["mismatches"] = Value::Array(mismatches);
    if !valid && preflight_blocked_count > 0 {
        let allow_partial = report
            .pointer("/policy/allow_partial")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        report["verification_status"] = json!(if allow_partial {
            "partial"
        } else {
            "blocked_by_mission_preflight"
        });
    }
    report["dispatch"] = json!("not_started");
    report["execution"] = json!("not_started");
    if let Some(object) = report.as_object_mut() {
        object.remove("portfolio_verify_digest");
    }
    let digest = bioprism_ids::ContentHash::of_value(&report)
        .map_err(|error| CliError::internal(error.to_string()))?;
    report["portfolio_verify_digest"] = json!(digest.to_string());
    let status = report["verification_status"].as_str().unwrap_or("blocked");
    let human = format!(
        "domain workflow portfolio verification\n  items: {}\n  verification status: {}\n  preflight: {}\n  portfolio ready: {}\n  dispatch: not started\n  execution: not started\n\nNext: review mismatches and replay digests, then rerun authoritative verification before any explicit execution path.\n",
        report["items"].as_array().map(Vec::len).unwrap_or_default(),
        status,
        report["summary"]["preflight_status"]
            .as_str()
            .unwrap_or("deferred"),
        valid,
    );
    Ok(Outcome::ok(report, human).failing_if(!valid))
}

fn workflow_reconcile(
    instantiation_path: &Path,
    mission_path: Option<&Path>,
    evidence_bundle_path: Option<&Path>,
    policy_path: Option<&Path>,
    readiness_audit_path: Option<&Path>,
    require_readiness: bool,
) -> CliResult<Outcome> {
    if mission_path.is_none() && evidence_bundle_path.is_none() {
        return Err(CliError::invalid(
            "workflow reconcile requires --mission or --evidence-bundle",
        ));
    }
    let mut request = json!({
        "instantiation": io::read_json(instantiation_path)?,
    });
    if let Some(path) = mission_path {
        request["mission_report"] = io::read_json(path)?;
    }
    if let Some(path) = evidence_bundle_path {
        request["evidence_bundle"] = io::read_json(path)?;
    }
    let mut policy = if let Some(path) = policy_path {
        io::read_json(path)?
    } else {
        json!({})
    };
    if !policy.is_object() {
        return Err(
            CliError::invalid("--policy must contain a JSON object").about(
                policy_path
                    .map(Path::display)
                    .map(|display| display.to_string())
                    .unwrap_or_else(|| instantiation_path.display().to_string()),
            ),
        );
    }
    if require_readiness {
        policy["require_readiness"] = json!(true);
    }
    if !policy.as_object().is_some_and(|value| value.is_empty()) {
        request["policy"] = policy;
    }
    if let Some(path) = readiness_audit_path {
        request["readiness_audit"] = io::read_json(path)?;
    }
    let report = reconcile_domain_workflow(&request)
        .map_err(|error| CliError::invalid(error.to_string()))?;
    let status = report["completion"]["status"]
        .as_str()
        .unwrap_or("unverified");
    let ready = report["completion"]["ready"].as_bool().unwrap_or(false);
    let readiness_required = report["decision_readiness"]["required"]
        .as_bool()
        .unwrap_or(false);
    let readiness_gate_satisfied = report["decision_review_gate_satisfied"]
        .as_bool()
        .unwrap_or(!readiness_required);
    let valid = ready && (!readiness_required || readiness_gate_satisfied);
    let human = format!(
        "domain workflow reconciliation\n  workflow: {}\n  mission: {}\n  completion: {}\n  evidence ready: {}\n  decision-readiness gate: {}\n  execution: not started\n",
        report["workflow_id"].as_str().unwrap_or("unknown"),
        report["mission_id"].as_str().unwrap_or("unknown"),
        status,
        ready,
        if readiness_required {
            if readiness_gate_satisfied { "satisfied" } else { "blocked" }
        } else {
            "not required"
        },
    );
    Ok(Outcome::ok(report, human).failing_if(!valid))
}

fn load_workflow_reconciliation_registry(
    store_path: &Path,
) -> CliResult<DomainWorkflowReconciliationRegistry> {
    if !store_path.exists() {
        return Ok(DomainWorkflowReconciliationRegistry::new());
    }
    let snapshot = io::read_json(store_path)?;
    DomainWorkflowReconciliationRegistry::from_snapshot(&snapshot).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })
}

fn workflow_reconciliation_import(
    record_path: &Path,
    store_path: &Path,
    dry_run: bool,
) -> CliResult<Outcome> {
    let record = io::read_json(record_path)?;
    let mut registry = load_workflow_reconciliation_registry(store_path)?;
    let report = registry.import(&record).map_err(|error| {
        CliError::invalid(error.to_string()).about(record_path.display().to_string())
    })?;
    let snapshot = registry.snapshot().map_err(|error| {
        CliError::internal(error.to_string()).about(store_path.display().to_string())
    })?;
    let artifact = if report.get("created").and_then(Value::as_bool) == Some(true) {
        Some(io::write_artifact(store_path, &snapshot, dry_run)?)
    } else {
        None
    };
    let mut document = report;
    document["store"] = json!(store_path.display().to_string());
    document["record"] = json!(record_path.display().to_string());
    document["dry_run"] = json!(dry_run);
    document["state_digest"] = snapshot.get("state_digest").cloned().unwrap_or(Value::Null);
    document["artifact"] = artifact
        .as_ref()
        .map(|value| {
            json!({
                "path": value.path.display().to_string(),
                "bytes": value.bytes,
                "written": value.written
            })
        })
        .unwrap_or(Value::Null);
    let digest = document
        .get("reconciliation_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let human = format!(
        "workflow reconciliation {}\n  digest: {}\n  registry: {} (generation {})\n  state: {}\n\nNext: bioprism workflow reconciliation-query --store {}\n",
        if document.get("created").and_then(Value::as_bool) == Some(true) {
            if dry_run { "planned for import" } else { "imported" }
        } else {
            "already present"
        },
        digest,
        store_path.display(),
        document
            .get("registry_generation")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        if dry_run { "not written (dry run)" } else { "checkpoint updated" },
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

#[allow(clippy::too_many_arguments)]
fn workflow_reconciliation_query(
    store_path: &Path,
    mission_id: Option<&str>,
    workflow_id: Option<&str>,
    mission_plan_digest: Option<&str>,
    completion_status: Option<&str>,
    decision_readiness_state: Option<&str>,
    decision_readiness_gate_satisfied: Option<bool>,
    after: Option<&str>,
    limit: usize,
    include_records: bool,
) -> CliResult<Outcome> {
    let registry = load_workflow_reconciliation_registry(store_path)?;
    let report = registry
        .query(
            mission_id,
            workflow_id,
            mission_plan_digest,
            completion_status,
            decision_readiness_state,
            decision_readiness_gate_satisfied,
            after,
            limit,
            include_records,
        )
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let next_after = report
        .get("next_after")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    let human = format!(
        "workflow reconciliation registry query\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n\nNext: bioprism workflow reconciliation-query --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        next_after,
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

/// The template autonomy grant, built from the typed document rather than a `json!` literal so a
/// field renamed in the grant schema fails to compile here instead of drifting into a template
/// that the validator would refuse.
fn autopilot_template_document() -> CliResult<Value> {
    let document = AutonomyGrantDocument {
        allowed_tools: vec!["replace_with_an_allowed_tool_name".into()],
        allow_side_effects: false,
        max_attempts: 3,
        retry: RetryPolicyDocument::default(),
        schedule: RetryScheduleDocument::default(),
        require_reconciliation_complete: true,
        stop_on_first_success: true,
    };
    serde_json::to_value(document).map_err(|error| CliError::internal(error.to_string()))
}

/// The commented rendering of the template for human mode. Comments make it non-JSON on
/// purpose: the machine-usable object comes from `--json`, and a document an operator can paste
/// without reading is exactly what an authority template must not be.
const GRANT_TEMPLATE_COMMENTED: &str = r#"{
  // Tools the drive may let missions execute: bare tool names, at least one, at most 512.
  // An absent or empty list grants nothing, and agent_mission is always refused.
  "allowed_tools": ["replace_with_an_allowed_tool_name"],

  // Permit caller-supplied confirmation flags to reach side-effecting tools.
  "allow_side_effects": false,

  // Total mission dispatches the drive may perform, full and repair combined (1..=16).
  "max_attempts": 3,

  // Which 40.36 retry classes may be re-dispatched. There is deliberately no knob for
  // `terminal`: a refusal is policy behaving correctly and is never re-sent.
  "retry": {
    "retry_retryable_as_is": true,
    "retry_retryable_after_change": false,
    // An outcome that declared no retry decision is never re-sent unless this is true.
    "retry_unknown": false
  },

  // Optional deterministic repair backoff in caller-defined logical clock ticks. Zero is
  // immediate; a non-zero base requires a maximum at least as large as the base.
  "schedule": {
    "retry_base_delay": 0,
    "retry_max_delay": 0
  },

  // Require a reconciliation record with `complete` status and valid integrity before the
  // drive may report success; success is never inferred from a mission report alone.
  "require_reconciliation_complete": true,

  // Only true is supported; the field exists so the unsupported option fails loudly.
  "stop_on_first_success": true
}"#;

fn autopilot_grant_template() -> CliResult<Outcome> {
    let template = autopilot_template_document()?;
    let mut human = String::from(
        "autonomy grant template\n\
         Authority for autonomous dispatch comes only from an explicit grant; there is no \
         default grant.\nThe commented form below is for reading; `--json` prints the bare \
         object, directly usable as --grant.\n\n",
    );
    human.push_str(GRANT_TEMPLATE_COMMENTED);
    human.push_str(
        "\n\nNext: bioprism --json autopilot grant-template > grant.json, edit allowed_tools, \
         then bioprism autopilot run --instantiation <instantiation.json> --grant grant.json \
         --dry-run\n",
    );
    Ok(Outcome::ok(template, human))
}

fn parse_autonomy_grant(grant_path: &Path) -> CliResult<AutonomyGrant> {
    let raw = io::read_json(grant_path)?;
    let document: AutonomyGrantDocument = serde_json::from_value(raw).map_err(|error| {
        CliError::invalid(format!("invalid autonomy grant document: {error}"))
            .about(grant_path.display().to_string())
    })?;
    AutonomyGrant::try_from(document)
        .map_err(|error| CliError::from_grant(error).about(grant_path.display().to_string()))
}

fn autopilot_run(
    instantiation_path: &Path,
    grant_path: &Path,
    report_out: Option<&Path>,
    dry_run: bool,
) -> CliResult<Outcome> {
    let instantiation = io::read_json(instantiation_path)?;
    let grant = parse_autonomy_grant(grant_path)?;
    let grant_digest = grant.digest().map_err(CliError::from_autopilot)?;

    if dry_run {
        let mission = instantiation_mission(&instantiation).map_err(|error| {
            CliError::from_autopilot(error).about(instantiation_path.display().to_string())
        })?;
        let action = preview_first_action(&grant, mission).map_err(|error| {
            CliError::from_autopilot(error).about(instantiation_path.display().to_string())
        })?;
        let NextAction::DispatchFull {
            mission: planned_mission,
            authorization,
        } = &action
        else {
            return Err(CliError::internal(format!(
                "the planner answered an empty history with {action:?} instead of a first full \
                 dispatch"
            )));
        };
        let planned_mission_digest = bioprism_ids::ContentHash::of_value(planned_mission)
            .map_err(|error| CliError::internal(error.to_string()))?
            .to_string();
        let step_count = planned_mission["steps"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        let mission_id = planned_mission["mission_id"].as_str().unwrap_or("unknown");
        let document = json!({
            "ok": true,
            "workflow": "autopilot_run",
            "dry_run": true,
            "no_dispatch": true,
            "dispatch": "not_started",
            "execution": "not_started",
            "writes": "none",
            "grant_digest": grant_digest,
            "max_attempts": grant.max_attempts(),
            "planned_first_action": {
                "action": "dispatch_full",
                "attempt_index": authorization.attempt_index(),
                "mission_id": mission_id,
                "step_count": step_count,
                "planned_mission_digest": planned_mission_digest,
                "allowed_tools": grant.allowed_tools(),
                "allow_side_effects": grant.allow_side_effects(),
                "mission": planned_mission,
            },
            "report_out": report_out
                .map(|path| json!(path.display().to_string()))
                .unwrap_or(Value::Null),
        });
        let human = format!(
            "autopilot dry run (no-dispatch)\n  grant digest: {grant_digest}\n  planned attempt \
             1: full mission dispatch\n  mission: {mission_id} ({step_count} steps)\n  attempt \
             budget: {}\n  dispatch: not started\n  writes: none\n\nNext: bioprism autopilot run \
             --instantiation {} --grant {} --report-out autopilot-report.json\n",
            grant.max_attempts(),
            instantiation_path.display(),
            grant_path.display(),
        );
        return Ok(Outcome::ok(document, human));
    }

    let server = Server::new(
        std::env::current_dir().map_err(|error| CliError::internal(error.to_string()))?,
    );
    let cancellation = AtomicBool::new(false);
    let mut dispatcher = |mission: &Value| -> Result<Value, String> {
        server.execute_agent_mission_with_cancellation(mission, &cancellation)
    };
    let outcome = drive_instantiation(&grant, &instantiation, &mut dispatcher).map_err(|error| {
        CliError::from_autopilot(error).about(instantiation_path.display().to_string())
    })?;
    let succeeded = outcome.final_status == FinalStatus::Succeeded;
    let report = outcome.report;
    let artifact = report_out
        .map(|path| io::write_artifact(path, &report, false))
        .transpose()?;
    let final_status = report["final_status"].as_str().unwrap_or("unknown").to_string();
    let attempts_used = report["totals"]["attempts_used"].as_u64().unwrap_or(0);
    let max_attempts = report["totals"]["max_attempts"].as_u64().unwrap_or(0);
    let report_sha256 = report["report_sha256"].as_str().unwrap_or("<missing>").to_string();
    let base_mission_id = report["base_mission_id"].as_str().unwrap_or("unknown").to_string();
    let document = json!({
        "ok": succeeded,
        "workflow": "autopilot_run",
        "dry_run": false,
        "final_status": final_status,
        "attempts_used": attempts_used,
        "max_attempts": max_attempts,
        "grant_digest": grant_digest,
        "report_sha256": report_sha256,
        "artifact": artifact
            .as_ref()
            .map(|value| {
                json!({
                    "path": value.path.display().to_string(),
                    "bytes": value.bytes,
                    "written": value.written,
                })
            })
            .unwrap_or(Value::Null),
        "report": report,
    });
    let mut human = format!(
        "autopilot drive: {final_status}\n  base mission: {base_mission_id}\n  grant digest: \
         {grant_digest}\n  attempts used: {attempts_used} of {max_attempts}\n  report sha256: \
         {report_sha256}\n",
    );
    if let Some(artifact) = &artifact {
        human.push_str(&format!(
            "  wrote {} ({} bytes)\n",
            artifact.path.display(),
            artifact.bytes
        ));
    }
    match report_out {
        Some(path) => human.push_str(&format!(
            "\nNext: bioprism autopilot verify --report {}\n",
            path.display()
        )),
        None => human.push_str(&format!(
            "\nNext: bioprism autopilot run --instantiation {} --grant {} --report-out \
             autopilot-report.json\n",
            instantiation_path.display(),
            grant_path.display()
        )),
    }
    Ok(Outcome::ok(document, human).failing_if(!succeeded))
}

fn autopilot_verify(report_path: &Path) -> CliResult<Outcome> {
    let report = io::read_json(report_path)?;
    let mut verification = verify_autopilot_report(&report).map_err(|error| {
        CliError::from_autopilot(error).about(report_path.display().to_string())
    })?;
    let valid = verification
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    verification["ok"] = json!(valid);
    verification["report"] = json!(report_path.display().to_string());
    let human = format!(
        "autopilot report: {}\n  claimed sha256: {}\n  recomputed sha256: {}\n  digest match: \
         {}\n  required limitations present: {}\n  final status known: {}\n\nNext: bioprism \
         autopilot verify --report {}\n",
        if valid { "verified" } else { "FAILED" },
        verification["claimed_report_sha256"].as_str().unwrap_or("<missing>"),
        verification["recomputed_report_sha256"].as_str().unwrap_or("<missing>"),
        verification["digest_match"].as_bool().unwrap_or(false),
        verification["limitations_present"].as_bool().unwrap_or(false),
        verification["final_status_known"].as_bool().unwrap_or(false),
        report_path.display(),
    );
    Ok(Outcome::ok(verification, human).failing_if(!valid))
}

fn evidence_bundle_verify(bundle_path: &Path) -> CliResult<Outcome> {
    let bundle = io::read_json(bundle_path)?;
    let report = verify_mission_evidence_bundle(&bundle).map_err(|error| {
        CliError::invalid(error.to_string()).about(bundle_path.display().to_string())
    })?;
    let valid = report
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let digest = report
        .get("bundle_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let recomputed = report
        .get("recomputed_bundle_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let failures = report
        .get("failures")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let human = format!(
        "mission evidence bundle: {}\n  declared digest: {}\n  recomputed digest: {}\n  failures: {}\n\nNext: bioprism evidence verify --bundle {}\n",
        if valid { "verified" } else { "FAILED" },
        digest,
        recomputed,
        if failures.is_empty() { "none" } else { &failures },
        bundle_path.display()
    );
    Ok(Outcome::ok(report, human).failing_if(!valid))
}

fn load_evidence_registry(store_path: &Path) -> CliResult<EvidenceBundleRegistry> {
    if !store_path.exists() {
        return Ok(EvidenceBundleRegistry::new());
    }
    let snapshot = io::read_json(store_path)?;
    EvidenceBundleRegistry::from_snapshot(&snapshot).map_err(|error| {
        CliError::invalid(error.to_string()).about(store_path.display().to_string())
    })
}

fn evidence_bundle_import(
    bundle_path: &Path,
    store_path: &Path,
    dry_run: bool,
) -> CliResult<Outcome> {
    let bundle = io::read_json(bundle_path)?;
    let mut registry = load_evidence_registry(store_path)?;
    let report = registry.import(&bundle).map_err(|error| {
        CliError::invalid(error.to_string()).about(bundle_path.display().to_string())
    })?;
    let snapshot = registry.snapshot().map_err(|error| {
        CliError::internal(error.to_string()).about(store_path.display().to_string())
    })?;
    let artifact = if report.get("created").and_then(Value::as_bool) == Some(true) {
        Some(io::write_artifact(store_path, &snapshot, dry_run)?)
    } else {
        None
    };
    let mut document = report;
    document["store"] = json!(store_path.display().to_string());
    document["dry_run"] = json!(dry_run);
    document["state_digest"] = snapshot.get("state_digest").cloned().unwrap_or(Value::Null);
    document["artifact"] = artifact
        .as_ref()
        .map(|value| {
            json!({
                "path": value.path.display().to_string(),
                "bytes": value.bytes,
                "written": value.written
            })
        })
        .unwrap_or(Value::Null);
    let digest = document
        .get("bundle_digest")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let human = format!(
        "mission evidence bundle {}\n  digest: {}\n  registry: {} (generation {})\n  state: {}\n\nNext: bioprism evidence query --store {}\n",
        if document.get("created").and_then(Value::as_bool) == Some(true) {
            if dry_run { "planned for import" } else { "imported" }
        } else {
            "already present"
        },
        digest,
        store_path.display(),
        document
            .get("registry_generation")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        if dry_run { "not written (dry run)" } else { "checkpoint updated" },
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

fn evidence_bundle_query(
    store_path: &Path,
    mission_id: Option<&str>,
    domain: Option<&str>,
    after: Option<&str>,
    limit: usize,
    include_bundles: bool,
) -> CliResult<Outcome> {
    let registry = load_evidence_registry(store_path)?;
    let report = registry
        .query(mission_id, domain, after, limit, include_bundles)
        .map_err(|error| {
            CliError::invalid(error.to_string()).about(store_path.display().to_string())
        })?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let next_after = report
        .get("next_after")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    let human = format!(
        "mission evidence registry query\n  store: {}\n  rows: {}\n  has more: {}\n  next after: {}\n\nNext: bioprism evidence query --store {} --after <digest>\n",
        store_path.display(),
        rows,
        report.get("has_more").and_then(Value::as_bool).unwrap_or(false),
        next_after,
        store_path.display()
    );
    Ok(Outcome::ok(report, human))
}

/// Validates a world, classifying scope dimensions from `--dimensions` when one is given.
///
/// The default registry knows only the reference dimensions, so a domain world's own dimensions
/// (`venue`, `account`, …) surface as `unclassified_scope_dimension` warnings — which is correct
/// when nobody declared them and noise when a `bioprism-scope-dimensions/0.1` document exists.
/// The registry source is echoed into both outputs, because a clean report means something
/// different under a caller-supplied classification than under the default one.
fn world_validate(world_path: &Path, dimensions_path: Option<&Path>) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;
    let registry = match dimensions_path {
        None => DimensionRegistry::default(),
        Some(path) => {
            let raw = io::read_json(path)?;
            DimensionRegistry::from_json(&raw)
                .map_err(|e| CliError::invalid(e).about(path.display().to_string()))?
        }
    };
    let report = validate(&world, &registry);
    let errors = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = report.diagnostics.len() - errors;

    let document = json!({
        "ok": errors == 0,
        "world_id": world.world_id.as_str(),
        "world_sha256": world.content_hash().as_str(),
        "counts": {
            "facts": world.facts.len(),
            "factors": world.factors.len(),
            "events": world.events.len(),
        },
        "errors": errors,
        "warnings": warnings,
        "dimensions_source": match dimensions_path {
            Some(path) => path.display().to_string(),
            None => "default".to_string(),
        },
        "diagnostics": report.diagnostics,
    });

    let mut human = format!(
        "world {} — {} facts, {} factors, {} events\n{} error(s), {} warning(s)\n",
        world.world_id,
        world.facts.len(),
        world.factors.len(),
        world.events.len(),
        errors,
        warnings
    );
    if let Some(path) = dimensions_path {
        human.push_str(&format!("  scope dimensions classified by {}\n", path.display()));
    }
    for diagnostic in &report.diagnostics {
        let label = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warn",
        };
        human.push_str(&format!(
            "  {label:<6} {:<34} {} — {}\n",
            diagnostic.code, diagnostic.subject, diagnostic.message
        ));
    }
    human.push_str(&format!(
        "\nNext: bioprism context explain --world {} --query <query.json>\n",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human).failing_if(errors > 0))
}

fn world_show(world_path: &Path) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;

    let mut tag_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for fact in &world.facts {
        for tag in &fact.tags {
            *tag_counts.entry(tag.as_str()).or_default() += 1;
        }
    }
    let mut kind_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for factor in &world.factors {
        *kind_counts.entry(factor.kind.as_str()).or_default() += 1;
    }

    let document = json!({
        "ok": true,
        "world_id": world.world_id.as_str(),
        "world_sha256": world.content_hash().as_str(),
        "description": world.description,
        "counts": {
            "facts": world.facts.len(),
            "factors": world.factors.len(),
            "events": world.events.len(),
        },
        "fact_tags": tag_counts,
        "factor_kinds": kind_counts,
        "event_managed_variables": world.event_managed_variables(),
    });

    let mut human = format!("world {}\n", world.world_id);
    if let Some(description) = &world.description {
        human.push_str(&format!("  {description}\n"));
    }
    human.push_str(&format!(
        "  {} facts, {} factors, {} events\n  sha256 {}\n\nfact tags\n",
        world.facts.len(),
        world.factors.len(),
        world.events.len(),
        world.content_hash()
    ));
    for (tag, count) in &tag_counts {
        human.push_str(&format!("  {tag:<22} {count}\n"));
    }
    human.push_str("\nfactor kinds\n");
    for (kind, count) in &kind_counts {
        human.push_str(&format!("  {kind:<22} {count}\n"));
    }
    human.push_str(&format!(
        "\nNext: bioprism world validate --world {}\n",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human))
}

fn world_generate(options: &GenerateOptions) -> CliResult<Outcome> {
    let spec = match options.family {
        Family::ReferenceLike => bioprism_worldgen::WorldSpec::reference_like(options.distractors),
        Family::Discriminating => bioprism_worldgen::WorldSpec::discriminating(options.distractors),
    };
    let generated = bioprism_worldgen::generate(&spec);

    let mut written = Vec::new();
    if let Some(path) = &options.world_out {
        written.push(io::write_artifact(path, &generated.world, options.dry_run)?);
    }
    if let Some(path) = &options.query_out {
        written.push(io::write_artifact(path, &generated.query, options.dry_run)?);
    }

    let facts = generated.world["facts"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let factors = generated.world["factors"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);

    let document = json!({
        "ok": true,
        "world_id": generated.world["world_id"],
        "family": match options.family {
            Family::ReferenceLike => "reference-like",
            Family::Discriminating => "discriminating",
        },
        "counts": { "facts": facts, "factors": factors },
        "dry_run": options.dry_run,
        "artifacts": written
            .iter()
            .map(|a| json!({
                "path": a.path.display().to_string(),
                "bytes": a.bytes,
                "written": a.written,
            }))
            .collect::<Vec<_>>(),
    });

    let mut human = format!(
        "generated {} — {} facts, {} factors
",
        generated.world["world_id"].as_str().unwrap_or_default(),
        facts,
        factors
    );
    for artifact in &written {
        human.push_str(&format!(
            "  {} {} ({} bytes)
",
            if artifact.written {
                "wrote"
            } else {
                "would write"
            },
            artifact.path.display(),
            artifact.bytes
        ));
    }
    human.push_str(
        "
Next: bioprism context compare --world <world.json> --query <query.json>
",
    );

    Ok(Outcome::ok(document, human))
}

/// Routes a sweep failure to the code carrying its 40.36 class.
///
/// A generated world or query the loader rejects is this binary's fault — the generator and the
/// loader ship together, so the operator supplied nothing that could be edited. A cell with no
/// reference verdict defers to [`CliError::from_compare`], so the paths that surface the same
/// oracle refusal cannot drift apart.
fn sweep_error(error: bioprism_baseline::SweepError) -> CliError {
    use bioprism_baseline::SweepError;
    let message = error.to_string();
    match error {
        SweepError::WorldRejected { .. } | SweepError::QueryRejected { .. } => {
            CliError::internal(message)
        }
        SweepError::NoReference { source, .. } => {
            CliError::new(CliError::from_compare(source).code, message)
        }
    }
}

/// Runs the structural family sweep (43.39 grid, full baseline panel per cell).
///
/// `--distractors` and `--seed` override the default grid's distractor axis and seed; the other
/// axes stay as declared, because varying the decision-defining knobs would compare strategies
/// across different questions. Rows the oracle refused serialise with `judged: false` and no
/// `sound` key at all, following `context compare`: an absent key cannot be read as a measured
/// zero. Exit 1 applies the 43.41 stop rule — FIBER inadmissible in any cell blocks advancement.
fn world_sweep(
    distractors: Option<&[usize]>,
    seed: Option<u64>,
    markdown: bool,
) -> CliResult<Outcome> {
    use bioprism_baseline::sweep::{run_sweep, SweepGrid};
    use bioprism_worldgen::{DistractorAttachment, TagStyle};

    let mut grid = SweepGrid::default_grid();
    if let Some(counts) = distractors {
        grid.distractor_counts = counts.to_vec();
    }
    if let Some(seed) = seed {
        grid.seed = seed;
    }
    let table = run_sweep(&grid).map_err(sweep_error)?;

    let strategies: Vec<&str> = table
        .cells
        .first()
        .map(|cell| cell.rows.iter().map(|row| row.strategy.as_str()).collect())
        .unwrap_or_default();
    let mut admissible_cells = serde_json::Map::new();
    for strategy in &strategies {
        admissible_cells.insert(
            strategy.to_string(),
            json!(table.admissible_cells(strategy)),
        );
    }
    let fiber_admissible_everywhere = table
        .cells
        .iter()
        .all(|cell| cell.row("fiber").is_some_and(|row| row.admissible));

    let attachment_label = |attachment: DistractorAttachment| match attachment {
        DistractorAttachment::Hub => "hub",
        DistractorAttachment::NearTarget => "near_target",
    };
    let tag_label = |style: TagStyle| match style {
        TagStyle::Distinct => "distinct",
        TagStyle::Camouflaged => "camouflaged",
    };

    let document = json!({
        "ok": fiber_admissible_everywhere,
        "seed": table.seed,
        "cells_total": table.cells.len(),
        "admissible_cells": admissible_cells,
        "cells": table.cells.iter().map(|cell| json!({
            "world_id": cell.world_id,
            "attachment": attachment_label(cell.attachment),
            "relay_depth": cell.relay_depth,
            "tag_style": tag_label(cell.tag_style),
            "distractors": cell.distractors,
            "total_facts": cell.total_facts,
            "rows": cell.rows.iter().map(|row| {
                let mut object = serde_json::Map::new();
                object.insert("strategy".into(), json!(row.strategy));
                object.insert("facts_selected".into(), json!(row.facts_selected));
                object.insert("judged".into(), json!(row.sound.is_some()));
                if let Some(sound) = row.sound {
                    object.insert("sound".into(), json!(sound));
                }
                object.insert("protected_closure".into(), json!(row.protected_closure));
                object.insert("admissible".into(), json!(row.admissible));
                Value::Object(object)
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });

    let mut human = table.to_markdown();
    if !markdown {
        human.push_str(
            "\nNext: bioprism world generate --family discriminating --world-out world.json \
             --query-out query.json\n",
        );
    }

    Ok(Outcome::ok(document, human).failing_if(!fiber_admissible_everywhere))
}

fn world_index(world_path: &Path, store_path: &Path, dry_run: bool) -> CliResult<Outcome> {
    let raw = io::read_json(world_path)?;
    io::refuse_to_rebind_store(store_path, &raw)?;

    if dry_run {
        return Ok(Outcome::ok(
            json!({ "ok": true, "dry_run": true, "store": store_path.display().to_string() }),
            format!(
                "would index {} into {}
",
                world_path.display(),
                store_path.display()
            ),
        ));
    }

    let manifest =
        bioprism_store::build(&raw, store_path).map_err(|e| CliError::from_store(store_path, e))?;

    let document = json!({
        "ok": true,
        "world_id": manifest.world_id,
        "world_sha256": manifest.world_sha256,
        "store": store_path.display().to_string(),
        "counts": {
            "facts": manifest.total_facts,
            "factors": manifest.total_factors,
            "events": manifest.events.len(),
        },
    });
    let human = format!(
        "indexed {} — {} facts, {} factors
  store {}
  sha256 {}

Next: bioprism context explain --world {} --query <query.json>
",
        manifest.world_id,
        manifest.total_facts,
        manifest.total_factors,
        store_path.display(),
        manifest.world_sha256,
        store_path.display()
    );
    Ok(Outcome::ok(document, human))
}

/// Loads and validates a domain pack, mapping a malformed document to `invalid_input`.
///
/// The pack is validated whole at this boundary (`DomainPack::from_json` consults nothing
/// lazily), so a compile that proceeds past this call is judged by exactly the oracle the pack
/// declared — a half-parsed pack cannot silently fall back to the reference oracle.
fn load_domain_pack(path: &Path) -> CliResult<DomainPack> {
    let raw = io::read_json(path)?;
    DomainPack::from_json(&raw)
        .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))
}

/// What the pack declares that this query does not honour.
///
/// Advisories, not errors, because a pack cannot amend a query: the certificate binds the query's
/// bytes by hash, so the query stays the sole author of its own contract and the pack can only
/// point at the gap. An unprotected pack tag is the dangerous one — facts carrying it never enter
/// protected closure, so nothing downstream will prove they were delivered.
fn domain_advisories(pack: &DomainPack, query: &Query) -> Vec<String> {
    let mut advisories = Vec::new();
    for tag in pack.protected_tags() {
        if !query.protected_tags.contains(tag) {
            advisories.push(format!(
                "the pack protects tag {tag:?} but the query does not, so facts tagged {tag:?} \
                 never enter protected closure"
            ));
        }
    }
    if query.goal.is_none() {
        match pack.goal() {
            Some(goal) => advisories.push(format!(
                "the query declares no goal; the pack declares the domain's: {goal:?}"
            )),
            None => advisories
                .push("the query declares no goal, and the pack declares none either".to_string()),
        }
    }
    advisories
}

/// The `domain` block both `--json` outputs carry when `--domain` was given.
fn domain_block(pack: &DomainPack, advisories: &[String]) -> Value {
    json!({
        "name": pack.name(),
        "oracle_kind": pack.oracle().kind(),
        "advisories": advisories,
    })
}

/// The human-mode rendering of the same block.
fn render_domain(pack: &DomainPack, advisories: &[String]) -> String {
    let mut text = format!(
        "domain {} (oracle {})\n",
        pack.name(),
        pack.oracle().kind()
    );
    for advisory in advisories {
        text.push_str(&format!("  advisory: {advisory}\n"));
    }
    text
}

fn context_compile(options: &CompileOptions) -> CliResult<Outcome> {
    let world = io::load_source(&options.world)?;
    let query = io::load_query(&options.query)?;
    let profile = match options.profile {
        Profile::Reference => CertificateProfile::Reference,
        Profile::Extended => CertificateProfile::Extended,
    };

    let pack = options.domain.as_deref().map(load_domain_pack).transpose()?;
    let out = match &pack {
        Some(pack) => compile_with_oracle(world.as_ref(), &query, pack.oracle()),
        None => compile(world.as_ref(), &query),
    }
    .map_err(CliError::from_compile)?;

    let certificate_document = out
        .certificate
        .to_json(profile)
        .map_err(|e| CliError::internal(e.to_string()))?;
    let section_document = out.section.to_json();

    let mut written = Vec::new();
    if let Some(path) = &options.certificate_out {
        written.push(io::write_artifact(
            path,
            &certificate_document,
            options.dry_run,
        )?);
    }
    if let Some(path) = &options.section_out {
        written.push(io::write_artifact(
            path,
            &section_document,
            options.dry_run,
        )?);
    }

    let digest = certificate_document["certificate_sha256"]
        .as_str()
        .unwrap_or_default();
    let invalid = out.certificate.oracle.status == OracleStatus::Invalid;

    let advisories = pack
        .as_ref()
        .map(|pack| domain_advisories(pack, &query))
        .unwrap_or_default();

    let mut document = json!({
        "ok": true,
        "world_id": out.certificate.world_id,
        "query_id": out.certificate.query_id,
        "oracle": {
            "kind": out.certificate.oracle.oracle_kind,
            "status": out.certificate.oracle.status.as_str(),
            "witnesses": out.certificate.oracle.witness_kinds(),
        },
        "selected_facts": out.certificate.selected_facts.len(),
        "selected_factors": out.certificate.selected_factors.len(),
        "omitted_facts": out.certificate.omissions.total_facts,
        "protected_closure": out.certificate.protected_closure.len(),
        "protected_closure_satisfied": out.protected_closure_satisfied(),
        "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
        "certificate_sha256": digest,
        "profile": match profile {
            CertificateProfile::Reference => "reference",
            CertificateProfile::Extended => "extended",
        },
        "dry_run": options.dry_run,
        "artifacts": written
            .iter()
            .map(|a| json!({
                "path": a.path.display().to_string(),
                "bytes": a.bytes,
                "written": a.written,
            }))
            .collect::<Vec<_>>(),
    });
    if let Some(pack) = &pack {
        document
            .as_object_mut()
            .expect("compile document is an object")
            .insert("domain".into(), domain_block(pack, &advisories));
    }

    let mut human = format!(
        "compiled {} facts and {} factors, omitted {} facts\noracle {} → {}\ncertificate sha256 {}\n",
        out.certificate.selected_facts.len(),
        out.certificate.selected_factors.len(),
        out.certificate.omissions.total_facts,
        out.certificate.oracle.oracle_kind,
        out.certificate.oracle.status.as_str(),
        digest
    );
    if let Some(pack) = &pack {
        human.push_str(&render_domain(pack, &advisories));
    }
    for witness in out.certificate.oracle.witness_kinds() {
        human.push_str(&format!("  witness {witness}\n"));
    }
    if !out.protected_closure_satisfied() {
        human.push_str(&format!(
            "  WARNING mandatory protected closure not delivered: {} facts withheld\n",
            out.trace.dropped_protected.len()
        ));
    }
    for artifact in &written {
        human.push_str(&format!(
            "  {} {} ({} bytes)\n",
            if artifact.written {
                "wrote"
            } else {
                "would write"
            },
            artifact.path.display(),
            artifact.bytes
        ));
    }
    human.push_str(&format!(
        "\nNext: bioprism context explain --world {} --query {}{}\n",
        options.world.display(),
        options.query.display(),
        match &options.domain {
            Some(path) => format!(" --domain {}", path.display()),
            None => String::new(),
        }
    ));

    Ok(Outcome::ok(document, human).failing_if(invalid && options.fail_on_invalid))
}

fn context_explain(
    world_path: &Path,
    query_path: &Path,
    domain_path: Option<&Path>,
) -> CliResult<Outcome> {
    let world = io::load_source(world_path)?;
    let query = io::load_query(query_path)?;
    let pack = domain_path.map(load_domain_pack).transpose()?;
    let out = match &pack {
        Some(pack) => compile_with_oracle(world.as_ref(), &query, pack.oracle()),
        None => compile(world.as_ref(), &query),
    }
    .map_err(CliError::from_compile)?;
    let advisories = pack
        .as_ref()
        .map(|pack| domain_advisories(pack, &query))
        .unwrap_or_default();

    let mut document = json!({
        "ok": true,
        "world_id": out.certificate.world_id,
        "query_id": out.certificate.query_id,
        "backend": out.certificate.plan.backend.as_str(),
        "passes": out.trace.passes
            .iter()
            .map(|p| json!({ "name": p.name, "retained": p.retained, "note": p.note }))
            .collect::<Vec<_>>(),
        "deferred_passes": out.trace.deferred_passes
            .iter()
            .map(|(name, reason)| json!({ "name": name, "reason": reason }))
            .collect::<Vec<_>>(),
        "selection": {
            "fact_ratio": out.certificate.plan.fact_selection_ratio(),
            "factor_ratio": out.certificate.plan.factor_selection_ratio(),
            "max_selected_factor_arity": out.certificate.plan.max_selected_factor_arity,
        },
        "omission_manifest": out.certificate.manifest,
        "supports_sufficiency_claim": out.certificate.manifest.supports_sufficiency_claim(),
        "unmatched_protected_tags": out.trace.unmatched_protected_tags,
        "dropped_protected": out.trace.dropped_protected,
    });
    if let Some(pack) = &pack {
        document
            .as_object_mut()
            .expect("explain document is an object")
            .insert("domain".into(), domain_block(pack, &advisories));
    }

    let mut human = explain::render(&out);
    if let Some(pack) = &pack {
        human.push_str(&render_domain(pack, &advisories));
    }
    human.push_str(&format!(
        "\nNext: bioprism context compile --world {} --query {} --certificate-out cert.json{}\n",
        world_path.display(),
        query_path.display(),
        match domain_path {
            Some(path) => format!(" --domain {}", path.display()),
            None => String::new(),
        }
    ));

    Ok(Outcome::ok(document, human))
}

fn context_compare(world_path: &Path, query_path: &Path, markdown: bool) -> CliResult<Outcome> {
    let world = io::load_world(world_path)?;
    let query = io::load_query(query_path)?;

    let panel = bioprism_baseline::default_panel();
    let borrowed: Vec<&dyn bioprism_baseline::ContextStrategy> =
        panel.iter().map(|boxed| boxed.as_ref()).collect();
    let comparison = bioprism_baseline::compare(&world, &query, &borrowed)
        .map_err(|error| CliError::from_compare(error).about(world_path.display().to_string()))?;

    let mut human = comparison.to_markdown();
    if !markdown {
        human.push_str(&format!(
            "
Next: bioprism context explain --world {} --query {}
",
            world_path.display(),
            query_path.display()
        ));
    }

    // 43.41 stop rule: FIBER dropping a decisive witness, failing to deliver the mandatory
    // protected closure, or offering a selection the oracle would not judge, blocks advancement.
    // A row that established nothing is not a row that passed.
    let fiber_inadmissible = comparison
        .results
        .iter()
        .any(|r| r.name == "fiber" && !r.admissible());

    Ok(Outcome::ok(comparison.to_json(), human).failing_if(fiber_inadmissible))
}

fn prism_fork(
    world_path: &Path,
    query_path: &Path,
    bundle_out: Option<&Path>,
    with_minimization: bool,
) -> CliResult<Outcome> {
    use bioprism_prism::{
        matched_fork, minimize_world, render_table, Architecture, DecisionCell, InputRef,
        ResultBundle,
    };

    let world_raw = io::read_json(world_path)?;
    let query_raw = io::read_json(query_path)?;
    let world = io::load_world(world_path)?;
    let query = io::load_query(query_path)?;

    // Freeze the cell from the full-context verdict, so the acceptance contract is derived from
    // evidence rather than asserted by the operator.
    let reference = compile(&world, &query).map_err(CliError::from_compile)?;
    let mut cell = DecisionCell::new(
        format!("dc_{}", query.query_id.as_str()),
        "context policy under a frozen world and query",
        InputRef::new(world_path.display().to_string(), &world_raw),
        InputRef::new(query_path.display().to_string(), &query_raw),
    )
    .accepting(reference.certificate.oracle.status);
    for kind in reference.certificate.oracle.witness_kinds() {
        cell = cell.requiring_witness(kind);
    }

    let fork = matched_fork(&cell, &world, &query, &Architecture::default_panel());
    let regression_free = fork.is_regression_free();
    let human = render_table(&fork);

    let mut bundle = ResultBundle::new(cell, fork.clone());
    if with_minimization {
        // A bundle carrying a minimization the oracle refused would attest a reduction that
        // preserved nothing, so the refusal ends the command rather than riding along in it.
        let minimization = minimize_world(&world).map_err(|error| {
            CliError::from_minimize(error).about(world_path.display().to_string())
        })?;
        bundle = bundle.with_minimization(minimization);
    }
    let attested = bundle.attest();

    let mut artifacts = Vec::new();
    if let Some(path) = bundle_out {
        artifacts.push(io::write_artifact(path, &attested, false)?);
    }

    let mut document = json!({
        "ok": true,
        "cell_id": fork.cell_id,
        "passing": fork.passing,
        "failing": fork.failing,
        "unjudged": fork.unjudged,
        "not_attempted": fork.not_attempted,
        "attribution": fork.attribution,
        "arms": fork.arms,
        "bundle_sha256": attested["bundle_sha256"],
    });
    if let Some(map) = document.as_object_mut() {
        map.insert(
            "artifacts".into(),
            json!(artifacts
                .iter()
                .map(|a| json!({ "path": a.path.display().to_string(), "bytes": a.bytes }))
                .collect::<Vec<_>>()),
        );
    }

    let mut human = human;
    for artifact in &artifacts {
        human.push_str(&format!(
            "
wrote {} ({} bytes)
",
            artifact.path.display(),
            artifact.bytes
        ));
    }
    human.push_str(&format!(
        "
Next: bioprism prism minimize --world {}
",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human).failing_if(!regression_free))
}

fn mutate_family(world_path: &Path, out_dir: Option<&Path>) -> CliResult<Outcome> {
    use bioprism_mutation::{generate, measure, standard_suite};

    let world_raw = io::read_json(world_path)?;
    let family = generate(&world_raw, &standard_suite()).map_err(CliError::from_mutation)?;
    let diversity = measure(std::slice::from_ref(&family));

    let mut written = Vec::new();
    if let Some(directory) = out_dir {
        for (id, world) in &family.worlds {
            let safe: String = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            written.push(io::write_artifact(
                &directory.join(format!("{safe}.json")),
                world,
                false,
            )?);
        }
    }

    let document = json!({
        "ok": true,
        "parent_id": family.parent_id,
        "parent_sha256": family.parent_sha256,
        "accepted": family.accepted,
        "rejected": family.rejected,
        "duplicates": family.duplicates,
        "yield_rate": family.yield_rate(),
        "diversity": diversity,
        "headline": diversity.headline(),
        "publishable": diversity.is_publishable(),
        "artifacts_written": written.len(),
    });

    let mut human = format!(
        "parent {} ({}...)

{} accepted, {} rejected, {} duplicate(s); yield {:.0}%

",
        family.parent_id,
        &family.parent_sha256[..12],
        family.accepted.len(),
        family.rejected.len(),
        family.duplicates.len(),
        family.yield_rate() * 100.0
    );
    human.push_str(
        "| Instance | Family | Verdict | Witnesses |
|---|---|---|---:|
",
    );
    for instance in &family.accepted {
        human.push_str(&format!(
            "| {} | {} | {} | {} |
",
            instance.mutation_id,
            instance.family,
            instance.status,
            instance.witnesses.len()
        ));
    }
    for rejection in &family.rejected {
        human.push_str(&format!(
            "
- rejected `{}`: {}
",
            rejection.mutation_id, rejection.reason
        ));
    }
    human.push_str(&format!(
        "
{}
",
        diversity.headline()
    ));
    human.push_str(&format!(
        "publishable as a benchmark family: {}
",
        diversity.is_publishable()
    ));
    if !written.is_empty() {
        human.push_str(&format!(
            "
wrote {} world(s)
",
            written.len()
        ));
    }
    human.push_str(&format!(
        "
Next: bioprism prism fork --world {} --query <query.json>
",
        world_path.display()
    ));

    Ok(Outcome::ok(document, human))
}

fn prism_minimize(world_path: &Path) -> CliResult<Outcome> {
    use bioprism_prism::{minimize_world, preserves};

    let world = io::load_world(world_path)?;
    let result = minimize_world(&world)
        .map_err(|error| CliError::from_minimize(error).about(world_path.display().to_string()))?;
    // Three re-verification outcomes reach the envelope as three, because `ok: false` on a check
    // that never ran would report the reduction refuted when nobody judged it.
    let preservation = preserves(&world, &result);
    let verified = preservation.is_preserved();

    let document = json!({
        "ok": verified,
        "started_from": result.started_from,
        "minimal": result.minimal,
        "removed": result.removed,
        "reduction_ratio": result.reduction_ratio(),
        "preserved_status": result.preserved_status,
        "preserved_witnesses": result.preserved_witnesses,
        "oracle_evaluations": result.evaluations,
        "unjudged": result.unjudged,
        "guarantee": result.guarantee,
        "reverified": preservation,
    });

    let human = format!(
        "minimized {} facts to {} ({:.2}% of the world), preserving {} with witnesses {}
           {} oracle evaluations
  re-verified: {}
  {}

{}

Next: bioprism prism fork --world {} --query <query.json>
",
        result.started_from,
        result.minimal.len(),
        result.reduction_ratio() * 100.0,
        result.preserved_status,
        result.preserved_witnesses.join(", "),
        result.evaluations,
        reverification_line(&preservation),
        result.guarantee,
        result.minimal.join(
            "
"
        ),
        world_path.display()
    );

    Ok(Outcome::ok(document, human).failing_if(!verified))
}

/// The re-verification sentence, which must not read as a pass or as a refutation when it is
/// neither.
fn reverification_line(preservation: &bioprism_prism::Preservation) -> String {
    use bioprism_prism::Preservation;
    match preservation {
        Preservation::Preserved => "yes".to_string(),
        Preservation::Diverged { status, witnesses } => format!(
            "no — the minimal set now reads {status} with witnesses [{}]",
            witnesses.join(", ")
        ),
        Preservation::Unverifiable { detail } => {
            format!("not checked — the oracle refused the minimal set: {detail}")
        }
    }
}

fn context_verify(path: &Path) -> CliResult<Outcome> {
    let document = io::read_json(path)?;
    let verification = ContextCertificate::verify(&document)
        .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))?;

    use bioprism_section::CertificateVerification::*;
    let (ok, detail) = match &verification {
        Valid => (true, "digest verifies".to_string()),
        DigestMismatch {
            claimed,
            recomputed,
        } => (
            false,
            format!("digest mismatch: claims {claimed}, recomputes to {recomputed}"),
        ),
        Malformed(reason) => (false, format!("malformed certificate: {reason}")),
    };

    let document = json!({
        "ok": ok,
        "certificate": path.display().to_string(),
        "verification": detail,
        "schema_version": document.get("schema_version"),
    });
    let human = format!(
        "{}: {}\n\nNext: bioprism world validate --world <world.json>\n",
        path.display(),
        detail
    );

    Ok(Outcome::ok(document, human).failing_if(!ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commented_grant_template_stays_in_lockstep_with_the_typed_template_document() {
        let stripped = GRANT_TEMPLATE_COMMENTED
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed: Value = serde_json::from_str(&stripped)
            .expect("the commented template minus its comment lines must be valid JSON");
        let typed = autopilot_template_document()
            .expect("the typed template document must serialize");
        assert_eq!(
            parsed, typed,
            "the human-mode commented template and the --json template document have drifted"
        );
    }
}
