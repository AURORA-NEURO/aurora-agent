//! Argument parsing.
//!
//! Hand-rolled rather than derived. The 40.13 contract wants golden help output, a stable
//! exit-code matrix and a `--json` mode with no stray bytes on stdout; owning the parser keeps
//! all three exactly specified and keeps the binary dependency-free.

use crate::exit::{CliError, CliResult, ExitCode, Retryability};
use std::fmt::Write;
use std::path::PathBuf;

/// The help text, with the exit-code table rendered from the registry itself.
///
/// 40.13 asks for golden help output and a stable exit-code matrix, and the two pull against each
/// other the moment the matrix is typed out by hand: a code added to [`ExitCode`] without a
/// matching line here leaves the binary documenting a registry it no longer has.
/// `bioprism-devx`'s exit-code audit exists because exactly that kind of hand copy drifts, so this
/// one is generated instead — over [`ExitCode::ALL`], so a new code appears here whether or not
/// anybody remembers it should.
pub fn help() -> String {
    let mut text = String::from(HELP_HEAD);
    for code in ExitCode::ALL {
        let decision = code
            .retryability()
            .map(Retryability::as_str)
            .unwrap_or_default();
        let row = format!(
            "  {}  {:<17} {:<51} {decision}",
            code.as_i32(),
            code.slug(),
            code.summary()
        );
        let _ = writeln!(text, "{}", row.trim_end());
    }
    text.push_str(HELP_TAIL);
    text
}

const HELP_HEAD: &str = "\
bioprism — query-compiled inference for executable biology

Compiles a typed decision query against a FIBER world into the smallest decision-sufficient
Decision Section, together with a Context Certificate stating exactly what was omitted and
whether the omission could have changed the decision.

USAGE
  bioprism [--json] <command> <subcommand> [options]

COMMANDS
  world validate    --world <path>
                    Report structural diagnostics. Exit 1 if any are errors.
  world show        --world <path>
                    Summarise facts, factors, events, tags and factor kinds.
  world index       --world <path> --store <dir> [--dry-run]
                    Build a content-addressed index so compile cost tracks the compiled region
                    rather than the corpus. Afterwards --world accepts the store directory.
  world generate    --family reference-like|discriminating [--distractors <n>]
                    [--world-out <path>] [--query-out <path>] [--dry-run]
                    Generate a synthetic structural family (43.39) and its matching query.

  context explain   --world <path> --query <path>
                    Show the compile plan: passes run, passes deferred, selection ratios and
                    omissions grouped by influence class. Writes nothing.
  context compile   --world <path> --query <path>
                    [--certificate-out <path>] [--section-out <path>]
                    [--profile reference|extended] [--dry-run] [--fail-on-invalid]
                    Compile and optionally write artifacts.
  context verify    --certificate <path>
                    Recompute the certificate digest. Exit 1 if it does not verify.
  context compare   --world <path> --query <path> [--markdown]
                    Run the equal-engineering baseline panel (full-context, graph k-hop,
                    hypergraph component, query-graph, lexical top-k, FIBER) and report which
                    strategies preserve the reference verdict and at what cost.

  prism fork        --world <path> --query <path> [--bundle-out <path>] [--minimize]
                    Run every architecture from one frozen Decision Cell and report which
                    context policy explains the difference. Exit 1 if any architecture fails.
  prism minimize    --world <path>
                    Reduce the world to a 1-minimal set of facts that preserves the oracle
                    signature, then re-verify the reduction.

  mutate family     --world <path> [--out-dir <dir>]
                    Apply the standard metamorphic suite, validate every postcondition against
                    the oracle, deduplicate by content, and report effective diversity.

  evidence verify   --bundle <path>
                    Verify a portable mission evidence bundle's schema, retention claims and
                    content digests. Exit 1 when the bundle is well-formed but unverifiable.
  evidence import   --bundle <path> --store <path> [--dry-run]
                    Verify and idempotently add a bundle to a bounded digest-protected local
                    registry checkpoint. --dry-run reports the checkpoint without writing it.
  evidence query    --store <path> [--mission-id <id>] [--domain <name>] [--after <digest>]
                    [--limit <n>] [--include-bundles]
                    Query a local registry checkpoint without executing any mission or tool.
  evidence domain-lineage --store <path> [--digest <digest>] [--group-id <id>]
                    [--domain <name>] [--subject-id <id>] [--source-tool <tool>]
                    [--outcome observed|partial|refused|error|unknown]
                    [--request-digest <digest>] [--response-digest <digest>]
                    [--intake-digest <digest>] [--source-plan-digest <digest>]
                    [--after <digest>] [--limit <n>] [--no-children]
                    Trace retained domain-evidence intake digests and explicit registry lineage.

  knowledge interop-verify --request <path> [--receipt-out <path>] [--dry-run]
                    Verify a multimodal knowledge-representation interoperability request;
                    retain a typed assurance receipt without executing retrieval or external effects.

  protocol simulate-verify --request <path> [--receipt-out <path>] [--dry-run]
                    Verify a federated continual protocol-simulation report;
                    retain release evidence without executing a protocol runner or instrument.

  readiness audit --request <path>
                    Run the offline structural decision-readiness audit in a JSON request.
                    Catalogue binding and artifact retention remain transport responsibilities.
  readiness query --store <path> [--subject-id <id>] [--decision-state <state>]
                    [--policy-satisfied|--policy-unsatisfied] [--after <digest>]
                    [--limit <n>] [--include-audits]
                    Query retained decision-readiness artifacts without re-running an audit.

  workflow catalogue
                    Build one deterministic, digest-bound workflow template for every capability
                    group. No tool is selected or executed.
  workflow scaffold --workflow <id> --mission-id <id> --goal <text>
                    [--tools <path>] [--arguments <path>]
                    Select available tools by advisory stage (or use an explicit JSON string array)
                    and run no-dispatch preflight. Missing arguments remain explicitly blocked.
  workflow instantiate --workflow <id> --mission-id <id> --goal <text> --steps <path>
                    [--policy <path>] [--dry-run]
                    Instantiate a group-scoped mission and attach authoritative no-dispatch
                    preflight. The steps file is a JSON array or an object containing `steps`.
  workflow portfolio --requests <path> [--policy <path>] [--readiness-audit <path>]
                    [--allow-partial] [--require-complete-catalogue] [--require-readiness]
                    Plan multiple explicit group workflows from a JSON array (or an object with
                    `requests`), retaining independent no-dispatch preflight outcomes and an
                    optional caller-supplied decision-readiness gate.
  workflow portfolio-verify --portfolio <path> [--replay-requests <path>] [--policy <path>]
                    [--readiness-audit <path>] [--allow-partial] [--require-complete-catalogue]
                    [--require-replay] [--require-readiness]
                    Verify a retained portfolio digest, coverage, replay, and optional bound
                    readiness posture; authoritative mission preflight remains no-dispatch.
  workbench verify  --session <path> --report <path> [--ci-replay <path>] [--policy <path>]
                    [--expected-report-digest <digest>]
                    Verify a retained authoring/notebook report and optional CI projection;
                    no notebook cells, YAML, GitHub, or CI run is executed.
  workbench import  --report <path> --store <path> [--dry-run]
                    Retain a structurally valid workbench report in a bounded local checkpoint.
  workbench query   --store <path> [--session-digest <digest>] [--domain <name>]
                    [--capability <name>] [--state <state>] [--release-ready]
                    [--after <digest>] [--limit <n>] [--include-reports]
                    Query retained report posture without executing or re-evaluating a workbench.
  workbench get     --store <path> --digest <digest>
                    Fetch one retained report by canonical content digest.
  ci provider-evidence-import --request <path> --store <path> [--dry-run]
                    Re-audit and retain provider-shaped CI evidence with bounded artifact/log/
                    attestation lineage joins; no provider is contacted.
  ci provider-evidence-query --store <path> [--provider <name>] [--run-id <id>]
                    [--plan-digest <digest>] [--structurally-valid] [--conformance-ready]
                    [--after <digest>] [--limit <n>] [--include-records]
                    Query retained provider evidence without executing checks.
  ci provider-evidence-get --store <path> --digest <digest>
                    Fetch one retained provider evidence report by canonical content digest.
  workflow reconcile --instantiation <path> [--mission <path>] [--evidence-bundle <path>]
                    [--policy <path>] [--readiness-audit <path>] [--require-readiness]
                    Reconcile a retained agent_mission report or evidence bundle against the
                    instantiated workflow. Exit 1 when completion evidence or a required
                    readiness gate is not satisfied.
  workflow reconciliation-import --record <path> --store <path> [--dry-run]
                    Import a digest-valid workflow reconciliation report into a bounded local
                    registry checkpoint. --dry-run performs no write.
  workflow reconciliation-query --store <path> [--mission-id <id>] [--workflow-id <id>]
                    [--plan-digest <digest>] [--status <status>] [--readiness-state <state>]
                    [--readiness-gate-satisfied|--readiness-gate-unsatisfied]
                    [--after <digest>] [--limit <n>] [--include-records]
                    Query a local reconciliation registry without executing a mission.

GLOBAL OPTIONS
  --json            Emit exactly one JSON document on stdout and nothing else.
  -h, --help        Show this help.
  -V, --version     Show the version.

EXIT CODES
  Every failure code carries exactly one 40.36 retry decision, so a script can recover the
  decision from the status alone. Codes 0 and 1 report a verdict rather than a failure and
  publish no retry decision.

";

const HELP_TAIL: &str = "
Research and developer infrastructure. Not a medical device: it does not diagnose, recommend
treatment, or triage care.
";

// The parser intentionally keeps the invocation value owned and pattern-matchable so the
// command contract remains straightforward. The rich cross-domain lineage command adds enough
// typed filters to cross Clippy's representation-size heuristic; boxing every parser fixture
// would obscure that stable contract without changing runtime behavior.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Help,
    Version,
    Run(Invocation),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub json: bool,
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    WorldValidate {
        world: PathBuf,
    },
    WorldShow {
        world: PathBuf,
    },
    ContextExplain {
        world: PathBuf,
        query: PathBuf,
    },
    ContextCompile(CompileOptions),
    ContextVerify {
        certificate: PathBuf,
    },
    ContextCompare {
        world: PathBuf,
        query: PathBuf,
        markdown: bool,
    },
    WorldGenerate(GenerateOptions),
    WorldIndex {
        world: PathBuf,
        store: PathBuf,
        dry_run: bool,
    },
    PrismFork {
        world: PathBuf,
        query: PathBuf,
        bundle_out: Option<PathBuf>,
        minimize: bool,
    },
    PrismMinimize {
        world: PathBuf,
    },
    MutateFamily {
        world: PathBuf,
        out_dir: Option<PathBuf>,
    },
    EvidenceBundleVerify {
        bundle: PathBuf,
    },
    EvidenceBundleImport {
        bundle: PathBuf,
        store: PathBuf,
        dry_run: bool,
    },
    EvidenceBundleQuery {
        store: PathBuf,
        mission_id: Option<String>,
        domain: Option<String>,
        after: Option<String>,
        limit: usize,
        include_bundles: bool,
    },
    EvidenceDomainLineage {
        store: PathBuf,
        digest: Option<String>,
        group_id: Option<String>,
        domain: Option<String>,
        subject_id: Option<String>,
        source_tool: Option<String>,
        outcome: Option<String>,
        request_digest: Option<String>,
        response_digest: Option<String>,
        intake_digest: Option<String>,
        source_plan_digest: Option<String>,
        after: Option<String>,
        limit: usize,
        include_children: bool,
    },
    KnowledgeInteropVerify {
        request: PathBuf,
        receipt_out: Option<PathBuf>,
        dry_run: bool,
    },
    ProtocolSimulationVerify {
        request: PathBuf,
        receipt_out: Option<PathBuf>,
        dry_run: bool,
    },
    ReadinessAudit {
        request: PathBuf,
    },
    ReadinessQuery {
        store: PathBuf,
        subject_id: Option<String>,
        decision_state: Option<String>,
        policy_satisfied: Option<bool>,
        after: Option<String>,
        limit: usize,
        include_audits: bool,
    },
    WorkflowCatalogue,
    WorkflowScaffold {
        workflow: String,
        mission_id: String,
        goal: String,
        tools: Option<PathBuf>,
        arguments: Option<PathBuf>,
    },
    WorkflowInstantiate {
        workflow: String,
        mission_id: String,
        goal: String,
        steps: PathBuf,
        policy: Option<PathBuf>,
        dry_run: bool,
    },
    WorkflowPortfolio {
        requests: PathBuf,
        policy: Option<PathBuf>,
        readiness_audit: Option<PathBuf>,
        allow_partial: bool,
        require_complete_catalogue: bool,
        require_readiness: bool,
    },
    WorkflowPortfolioVerify {
        portfolio: PathBuf,
        replay_requests: Option<PathBuf>,
        policy: Option<PathBuf>,
        readiness_audit: Option<PathBuf>,
        allow_partial: bool,
        require_complete_catalogue: bool,
        require_replay: bool,
        require_readiness: bool,
    },
    WorkbenchVerify {
        session: PathBuf,
        report: PathBuf,
        ci_replay: Option<PathBuf>,
        policy: Option<PathBuf>,
        expected_report_digest: Option<String>,
    },
    WorkbenchImport {
        report: PathBuf,
        store: PathBuf,
        dry_run: bool,
    },
    WorkbenchQuery {
        store: PathBuf,
        session_digest: Option<String>,
        domain: Option<String>,
        capability: Option<String>,
        state: Option<String>,
        release_ready: bool,
        after: Option<String>,
        limit: usize,
        include_reports: bool,
    },
    WorkbenchGet {
        store: PathBuf,
        digest: String,
    },
    CiProviderEvidenceImport {
        request: PathBuf,
        store: PathBuf,
        dry_run: bool,
    },
    CiProviderEvidenceQuery {
        store: PathBuf,
        provider: Option<String>,
        run_id: Option<String>,
        plan_digest: Option<String>,
        structurally_valid: bool,
        conformance_ready: bool,
        after: Option<String>,
        limit: usize,
        include_records: bool,
    },
    CiProviderEvidenceGet {
        store: PathBuf,
        digest: String,
    },
    WorkflowReconcile {
        instantiation: PathBuf,
        mission: Option<PathBuf>,
        evidence_bundle: Option<PathBuf>,
        policy: Option<PathBuf>,
        readiness_audit: Option<PathBuf>,
        require_readiness: bool,
    },
    WorkflowReconciliationImport {
        record: PathBuf,
        store: PathBuf,
        dry_run: bool,
    },
    WorkflowReconciliationQuery {
        store: PathBuf,
        mission_id: Option<String>,
        workflow_id: Option<String>,
        mission_plan_digest: Option<String>,
        completion_status: Option<String>,
        decision_readiness_state: Option<String>,
        decision_readiness_gate_satisfied: Option<bool>,
        after: Option<String>,
        limit: usize,
        include_records: bool,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct GenerateOptions {
    pub family: Family,
    pub distractors: usize,
    pub world_out: Option<PathBuf>,
    pub query_out: Option<PathBuf>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    ReferenceLike,
    Discriminating,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub world: PathBuf,
    pub query: PathBuf,
    pub certificate_out: Option<PathBuf>,
    pub section_out: Option<PathBuf>,
    pub profile: Profile,
    pub dry_run: bool,
    pub fail_on_invalid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Reference,
    Extended,
}

pub fn parse<I: IntoIterator<Item = String>>(arguments: I) -> CliResult<Parsed> {
    let mut tokens: Vec<String> = arguments.into_iter().collect();

    if tokens.iter().any(|t| t == "-h" || t == "--help") {
        return Ok(Parsed::Help);
    }
    if tokens.iter().any(|t| t == "-V" || t == "--version") {
        return Ok(Parsed::Version);
    }

    let json = extract_flag(&mut tokens, "--json");
    let mut cursor = tokens.into_iter();

    let group = cursor.next().ok_or_else(|| usage("no command given"))?;
    let subcommand = cursor
        .next()
        .ok_or_else(|| usage(format!("{group:?} needs a subcommand")))?;
    let mut options = Options::collect(cursor)?;

    let command = match (group.as_str(), subcommand.as_str()) {
        ("world", "validate") => Command::WorldValidate {
            world: options.take_path("--world")?,
        },
        ("world", "show") => Command::WorldShow {
            world: options.take_path("--world")?,
        },
        ("world", "index") => Command::WorldIndex {
            world: options.take_path("--world")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("world", "generate") => Command::WorldGenerate(GenerateOptions {
            family: match options.take_optional("--family").as_deref() {
                None | Some("discriminating") => Family::Discriminating,
                Some("reference-like") => Family::ReferenceLike,
                Some(other) => {
                    return Err(usage(format!(
                        "--family must be reference-like or discriminating, got {other:?}"
                    )))
                }
            },
            distractors: match options.take_optional("--distractors") {
                None => 750,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--distractors must be a number, got {text:?}")))?,
            },
            world_out: options.take_optional_path("--world-out"),
            query_out: options.take_optional_path("--query-out"),
            dry_run: options.take_switch("--dry-run"),
        }),
        ("context", "explain") => Command::ContextExplain {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
        },
        ("context", "compile") => Command::ContextCompile(CompileOptions {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            certificate_out: options.take_optional_path("--certificate-out"),
            section_out: options.take_optional_path("--section-out"),
            profile: match options.take_optional("--profile").as_deref() {
                None | Some("reference") => Profile::Reference,
                Some("extended") => Profile::Extended,
                Some(other) => {
                    return Err(usage(format!(
                        "--profile must be reference or extended, got {other:?}"
                    )))
                }
            },
            dry_run: options.take_switch("--dry-run"),
            fail_on_invalid: options.take_switch("--fail-on-invalid"),
        }),
        ("context", "verify") => Command::ContextVerify {
            certificate: options.take_path("--certificate")?,
        },
        ("prism", "fork") => Command::PrismFork {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            bundle_out: options.take_optional_path("--bundle-out"),
            minimize: options.take_switch("--minimize"),
        },
        ("mutate", "family") => Command::MutateFamily {
            world: options.take_path("--world")?,
            out_dir: options.take_optional_path("--out-dir"),
        },
        ("prism", "minimize") => Command::PrismMinimize {
            world: options.take_path("--world")?,
        },
        ("context", "compare") => Command::ContextCompare {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            markdown: options.take_switch("--markdown"),
        },
        ("evidence", "verify") => Command::EvidenceBundleVerify {
            bundle: options.take_path("--bundle")?,
        },
        ("evidence", "import") => Command::EvidenceBundleImport {
            bundle: options.take_path("--bundle")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("evidence", "query") => Command::EvidenceBundleQuery {
            store: options.take_path("--store")?,
            mission_id: options.take_optional("--mission-id"),
            domain: options.take_optional("--domain"),
            after: options.take_optional("--after"),
            limit: match options.take_optional("--limit") {
                None => 100,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
            },
            include_bundles: options.take_switch("--include-bundles"),
        },
        ("evidence", "domain-lineage") => Command::EvidenceDomainLineage {
            store: options.take_path("--store")?,
            digest: options.take_optional("--digest"),
            group_id: options.take_optional("--group-id"),
            domain: options.take_optional("--domain"),
            subject_id: options.take_optional("--subject-id"),
            source_tool: options.take_optional("--source-tool"),
            outcome: options.take_optional("--outcome"),
            request_digest: options.take_optional("--request-digest"),
            response_digest: options.take_optional("--response-digest"),
            intake_digest: options.take_optional("--intake-digest"),
            source_plan_digest: options.take_optional("--source-plan-digest"),
            after: options.take_optional("--after"),
            limit: match options.take_optional("--limit") {
                None => 100,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
            },
            include_children: !options.take_switch("--no-children"),
        },
        ("knowledge", "interop-verify") => Command::KnowledgeInteropVerify {
            request: options.take_path("--request")?,
            receipt_out: options.take_optional_path("--receipt-out"),
            dry_run: options.take_switch("--dry-run"),
        },
        ("protocol", "simulate-verify") => Command::ProtocolSimulationVerify {
            request: options.take_path("--request")?,
            receipt_out: options.take_optional_path("--receipt-out"),
            dry_run: options.take_switch("--dry-run"),
        },
        ("readiness", "audit") => Command::ReadinessAudit {
            request: options.take_path("--request")?,
        },
        ("readiness", "query") => {
            let policy_satisfied = options.take_switch("--policy-satisfied");
            let policy_unsatisfied = options.take_switch("--policy-unsatisfied");
            if policy_satisfied && policy_unsatisfied {
                return Err(usage(
                    "--policy-satisfied and --policy-unsatisfied are mutually exclusive",
                ));
            }
            Command::ReadinessQuery {
                store: options.take_path("--store")?,
                subject_id: options.take_optional("--subject-id"),
                decision_state: options.take_optional("--decision-state"),
                policy_satisfied: if policy_satisfied {
                    Some(true)
                } else if policy_unsatisfied {
                    Some(false)
                } else {
                    None
                },
                after: options.take_optional("--after"),
                limit: match options.take_optional("--limit") {
                    None => 100,
                    Some(text) => text
                        .parse()
                        .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
                },
                include_audits: options.take_switch("--include-audits"),
            }
        }
        ("workflow", "catalogue") => Command::WorkflowCatalogue,
        ("workflow", "scaffold") => Command::WorkflowScaffold {
            workflow: options
                .take_optional("--workflow")
                .ok_or_else(|| usage("--workflow is required"))?,
            mission_id: options
                .take_optional("--mission-id")
                .ok_or_else(|| usage("--mission-id is required"))?,
            goal: options
                .take_optional("--goal")
                .ok_or_else(|| usage("--goal is required"))?,
            tools: options.take_optional_path("--tools"),
            arguments: options.take_optional_path("--arguments"),
        },
        ("workflow", "instantiate") => Command::WorkflowInstantiate {
            workflow: options
                .take_optional("--workflow")
                .ok_or_else(|| usage("--workflow is required"))?,
            mission_id: options
                .take_optional("--mission-id")
                .ok_or_else(|| usage("--mission-id is required"))?,
            goal: options
                .take_optional("--goal")
                .ok_or_else(|| usage("--goal is required"))?,
            steps: options.take_path("--steps")?,
            policy: options.take_optional_path("--policy"),
            dry_run: options.take_switch("--dry-run"),
        },
        ("workflow", "portfolio") => Command::WorkflowPortfolio {
            requests: options.take_path("--requests")?,
            policy: options.take_optional_path("--policy"),
            readiness_audit: options.take_optional_path("--readiness-audit"),
            allow_partial: options.take_switch("--allow-partial"),
            require_complete_catalogue: options.take_switch("--require-complete-catalogue"),
            require_readiness: options.take_switch("--require-readiness"),
        },
        ("workflow", "portfolio-verify") => Command::WorkflowPortfolioVerify {
            portfolio: options.take_path("--portfolio")?,
            replay_requests: options.take_optional_path("--replay-requests"),
            policy: options.take_optional_path("--policy"),
            readiness_audit: options.take_optional_path("--readiness-audit"),
            allow_partial: options.take_switch("--allow-partial"),
            require_complete_catalogue: options.take_switch("--require-complete-catalogue"),
            require_replay: options.take_switch("--require-replay"),
            require_readiness: options.take_switch("--require-readiness"),
        },
        ("workbench", "verify") => Command::WorkbenchVerify {
            session: options.take_path("--session")?,
            report: options.take_path("--report")?,
            ci_replay: options.take_optional_path("--ci-replay"),
            policy: options.take_optional_path("--policy"),
            expected_report_digest: options.take_optional("--expected-report-digest"),
        },
        ("workbench", "import") => Command::WorkbenchImport {
            report: options.take_path("--report")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("workbench", "query") => Command::WorkbenchQuery {
            store: options.take_path("--store")?,
            session_digest: options.take_optional("--session-digest"),
            domain: options.take_optional("--domain"),
            capability: options.take_optional("--capability"),
            state: options.take_optional("--state"),
            release_ready: options.take_switch("--release-ready"),
            after: options.take_optional("--after"),
            limit: match options.take_optional("--limit") {
                None => 100,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
            },
            include_reports: options.take_switch("--include-reports"),
        },
        ("workbench", "get") => Command::WorkbenchGet {
            store: options.take_path("--store")?,
            digest: options
                .take_optional("--digest")
                .ok_or_else(|| usage("--digest is required"))?,
        },
        ("ci", "provider-evidence-import") => Command::CiProviderEvidenceImport {
            request: options.take_path("--request")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("ci", "provider-evidence-query") => Command::CiProviderEvidenceQuery {
            store: options.take_path("--store")?,
            provider: options.take_optional("--provider"),
            run_id: options.take_optional("--run-id"),
            plan_digest: options.take_optional("--plan-digest"),
            structurally_valid: options.take_switch("--structurally-valid"),
            conformance_ready: options.take_switch("--conformance-ready"),
            after: options.take_optional("--after"),
            limit: match options.take_optional("--limit") {
                None => 100,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
            },
            include_records: options.take_switch("--include-records"),
        },
        ("ci", "provider-evidence-get") => Command::CiProviderEvidenceGet {
            store: options.take_path("--store")?,
            digest: options
                .take_optional("--digest")
                .ok_or_else(|| usage("--digest is required"))?,
        },
        ("workflow", "reconcile") => Command::WorkflowReconcile {
            instantiation: options.take_path("--instantiation")?,
            mission: options.take_optional_path("--mission"),
            evidence_bundle: options.take_optional_path("--evidence-bundle"),
            policy: options.take_optional_path("--policy"),
            readiness_audit: options.take_optional_path("--readiness-audit"),
            require_readiness: options.take_switch("--require-readiness"),
        },
        ("workflow", "reconciliation-import") => Command::WorkflowReconciliationImport {
            record: options.take_path("--record")?,
            store: options.take_path("--store")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("workflow", "reconciliation-query") => Command::WorkflowReconciliationQuery {
            store: options.take_path("--store")?,
            mission_id: options.take_optional("--mission-id"),
            workflow_id: options.take_optional("--workflow-id"),
            mission_plan_digest: options.take_optional("--plan-digest"),
            completion_status: options.take_optional("--status"),
            decision_readiness_state: options.take_optional("--readiness-state"),
            decision_readiness_gate_satisfied: {
                let satisfied = options.take_switch("--readiness-gate-satisfied");
                let unsatisfied = options.take_switch("--readiness-gate-unsatisfied");
                if satisfied && unsatisfied {
                    return Err(usage(
                        "--readiness-gate-satisfied and --readiness-gate-unsatisfied are mutually exclusive",
                    ));
                }
                if satisfied {
                    Some(true)
                } else if unsatisfied {
                    Some(false)
                } else {
                    None
                }
            },
            after: options.take_optional("--after"),
            limit: match options.take_optional("--limit") {
                None => 100,
                Some(text) => text
                    .parse()
                    .map_err(|_| usage(format!("--limit must be a number, got {text:?}")))?,
            },
            include_records: options.take_switch("--include-records"),
        },
        _ => return Err(usage(format!("unknown command {group:?} {subcommand:?}"))),
    };

    options.reject_leftovers()?;
    Ok(Parsed::Run(Invocation { json, command }))
}

fn extract_flag(tokens: &mut Vec<String>, flag: &str) -> bool {
    let present = tokens.iter().any(|t| t == flag);
    tokens.retain(|t| t != flag);
    present
}

fn usage(message: impl Into<String>) -> CliError {
    CliError::new(ExitCode::Usage, message)
}

struct Options {
    values: Vec<(String, Option<String>)>,
}

impl Options {
    fn collect<I: Iterator<Item = String>>(cursor: I) -> CliResult<Self> {
        let mut cursor = cursor.peekable();
        let mut values = Vec::new();
        while let Some(token) = cursor.next() {
            if !token.starts_with("--") {
                return Err(usage(format!("unexpected positional argument {token:?}")));
            }
            if let Some((name, inline)) = token.split_once('=') {
                values.push((name.to_string(), Some(inline.to_string())));
                continue;
            }
            match cursor.peek() {
                Some(next) if !next.starts_with("--") => {
                    let next = cursor.next().expect("peeked CLI option value");
                    values.push((token, Some(next)));
                }
                _ => values.push((token, None)),
            }
        }
        Ok(Options { values })
    }

    fn take_optional(&mut self, name: &str) -> Option<String> {
        let position = self.values.iter().position(|(key, _)| key == name)?;
        let (_, value) = self.values.remove(position);
        value
    }

    fn take_optional_path(&mut self, name: &str) -> Option<PathBuf> {
        self.take_optional(name).map(PathBuf::from)
    }

    fn take_path(&mut self, name: &str) -> CliResult<PathBuf> {
        self.take_optional_path(name)
            .ok_or_else(|| usage(format!("{name} is required and takes a path")))
    }

    fn take_switch(&mut self, name: &str) -> bool {
        match self.values.iter().position(|(key, _)| key == name) {
            Some(position) => {
                self.values.remove(position);
                true
            }
            None => false,
        }
    }

    fn reject_leftovers(&self) -> CliResult<()> {
        match self.values.first() {
            None => Ok(()),
            Some((name, _)) => Err(usage(format!("unrecognised option {name:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Command, Parsed};
    use std::path::PathBuf;

    #[test]
    fn evidence_bundle_verify_is_a_json_capable_command() {
        let parsed = parse(
            ["--json", "evidence", "verify", "--bundle", "bundle.json"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse evidence verifier");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::EvidenceBundleVerify {
                    bundle: PathBuf::from("bundle.json"),
                },
            })
        );
    }

    #[test]
    fn help_documents_evidence_bundle_verification() {
        assert!(super::help().contains("evidence verify   --bundle <path>"));
    }

    #[test]
    fn evidence_registry_commands_parse_bounded_options() {
        let parsed = parse(
            [
                "evidence",
                "query",
                "--store",
                "registry.json",
                "--mission-id",
                "m-1",
                "--domain",
                "oncology",
                "--limit",
                "7",
                "--include-bundles",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse evidence query");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::EvidenceBundleQuery {
                    store: PathBuf::from("registry.json"),
                    mission_id: Some("m-1".into()),
                    domain: Some("oncology".into()),
                    after: None,
                    limit: 7,
                    include_bundles: true,
                },
            })
        );
    }

    #[test]
    fn knowledge_interoperability_verifier_parses_retention_and_dry_run() {
        let parsed = parse(
            [
                "--json",
                "knowledge",
                "interop-verify",
                "--request",
                "retrieval.json",
                "--receipt-out",
                "receipt.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse knowledge interoperability verifier");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::KnowledgeInteropVerify {
                    request: PathBuf::from("retrieval.json"),
                    receipt_out: Some(PathBuf::from("receipt.json")),
                    dry_run: true,
                },
            })
        );
    }

    #[test]
    fn help_documents_knowledge_interoperability_verification() {
        assert!(super::help().contains("knowledge interop-verify --request <path>"));
    }

    #[test]
    fn workflow_instantiation_parses_explicit_steps_and_policy_paths() {
        let parsed = parse(
            [
                "workflow",
                "instantiate",
                "--workflow",
                "documentation_and_knowledge",
                "--mission-id",
                "m-1",
                "--goal",
                "discover capabilities",
                "--steps",
                "steps.json",
                "--policy",
                "policy.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workflow instantiate");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowInstantiate {
                    workflow: "documentation_and_knowledge".into(),
                    mission_id: "m-1".into(),
                    goal: "discover capabilities".into(),
                    steps: PathBuf::from("steps.json"),
                    policy: Some(PathBuf::from("policy.json")),
                    dry_run: true,
                },
            })
        );
    }

    #[test]
    fn workflow_scaffold_parses_optional_tool_and_argument_files() {
        let parsed = parse(
            [
                "workflow",
                "scaffold",
                "--workflow",
                "documentation_and_knowledge",
                "--mission-id",
                "m-1",
                "--goal",
                "discover capabilities",
                "--tools",
                "tools.json",
                "--arguments",
                "arguments.json",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workflow scaffold");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowScaffold {
                    workflow: "documentation_and_knowledge".into(),
                    mission_id: "m-1".into(),
                    goal: "discover capabilities".into(),
                    tools: Some(PathBuf::from("tools.json")),
                    arguments: Some(PathBuf::from("arguments.json")),
                },
            })
        );
    }

    #[test]
    fn workflow_portfolio_parses_bounded_scope_controls() {
        let parsed = parse(
            [
                "workflow",
                "portfolio",
                "--requests",
                "portfolio.json",
                "--policy",
                "policy.json",
                "--readiness-audit",
                "readiness.json",
                "--allow-partial",
                "--require-complete-catalogue",
                "--require-readiness",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workflow portfolio");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowPortfolio {
                    requests: PathBuf::from("portfolio.json"),
                    policy: Some(PathBuf::from("policy.json")),
                    readiness_audit: Some(PathBuf::from("readiness.json")),
                    allow_partial: true,
                    require_complete_catalogue: true,
                    require_readiness: true,
                },
            })
        );
    }

    #[test]
    fn workflow_portfolio_verify_parses_replay_and_integrity_controls() {
        let parsed = parse(
            [
                "workflow",
                "portfolio-verify",
                "--portfolio",
                "portfolio-report.json",
                "--replay-requests",
                "requests.json",
                "--policy",
                "policy.json",
                "--readiness-audit",
                "readiness.json",
                "--allow-partial",
                "--require-complete-catalogue",
                "--require-replay",
                "--require-readiness",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workflow portfolio verify");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowPortfolioVerify {
                    portfolio: PathBuf::from("portfolio-report.json"),
                    replay_requests: Some(PathBuf::from("requests.json")),
                    policy: Some(PathBuf::from("policy.json")),
                    readiness_audit: Some(PathBuf::from("readiness.json")),
                    allow_partial: true,
                    require_complete_catalogue: true,
                    require_replay: true,
                    require_readiness: true,
                },
            })
        );
    }

    #[test]
    fn workbench_verify_parses_retained_report_and_replay_controls() {
        let parsed = parse(
            [
                "workbench",
                "verify",
                "--session",
                "session.json",
                "--report",
                "report.json",
                "--ci-replay",
                "ci.json",
                "--policy",
                "policy.json",
                "--expected-report-digest",
                "a".repeat(64).as_str(),
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workbench verify");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkbenchVerify {
                    session: PathBuf::from("session.json"),
                    report: PathBuf::from("report.json"),
                    ci_replay: Some(PathBuf::from("ci.json")),
                    policy: Some(PathBuf::from("policy.json")),
                    expected_report_digest: Some("a".repeat(64)),
                },
            })
        );
    }

    #[test]
    fn workbench_registry_commands_parse_storage_and_query_controls() {
        let imported = parse(
            [
                "workbench",
                "import",
                "--report",
                "report.json",
                "--store",
                "state.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workbench import");
        assert_eq!(
            imported,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkbenchImport {
                    report: PathBuf::from("report.json"),
                    store: PathBuf::from("state.json"),
                    dry_run: true,
                },
            })
        );

        let queried = parse(
            [
                "workbench",
                "query",
                "--store",
                "state.json",
                "--session-digest",
                &"a".repeat(64),
                "--domain",
                "oncology",
                "--capability",
                "evidence",
                "--state",
                "release_ready",
                "--release-ready",
                "--after",
                &"b".repeat(64),
                "--limit",
                "17",
                "--include-reports",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workbench query");
        assert_eq!(
            queried,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkbenchQuery {
                    store: PathBuf::from("state.json"),
                    session_digest: Some("a".repeat(64)),
                    domain: Some("oncology".into()),
                    capability: Some("evidence".into()),
                    state: Some("release_ready".into()),
                    release_ready: true,
                    after: Some("b".repeat(64)),
                    limit: 17,
                    include_reports: true,
                },
            })
        );

        let fetched = parse(
            [
                "workbench",
                "get",
                "--store",
                "state.json",
                "--digest",
                &"c".repeat(64),
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workbench get");
        assert_eq!(
            fetched,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkbenchGet {
                    store: PathBuf::from("state.json"),
                    digest: "c".repeat(64),
                },
            })
        );
    }

    #[test]
    fn ci_provider_evidence_registry_commands_parse_lineage_controls() {
        let imported = parse(
            [
                "ci",
                "provider-evidence-import",
                "--request",
                "provider.json",
                "--store",
                "state.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse provider evidence import");
        assert_eq!(
            imported,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::CiProviderEvidenceImport {
                    request: PathBuf::from("provider.json"),
                    store: PathBuf::from("state.json"),
                    dry_run: true,
                },
            })
        );

        let queried = parse(
            [
                "ci",
                "provider-evidence-query",
                "--store",
                "state.json",
                "--provider",
                "github_actions",
                "--run-id",
                "9030",
                "--plan-digest",
                &"a".repeat(64),
                "--structurally-valid",
                "--conformance-ready",
                "--after",
                &"b".repeat(64),
                "--limit",
                "12",
                "--include-records",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse provider evidence query");
        assert_eq!(
            queried,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::CiProviderEvidenceQuery {
                    store: PathBuf::from("state.json"),
                    provider: Some("github_actions".into()),
                    run_id: Some("9030".into()),
                    plan_digest: Some("a".repeat(64)),
                    structurally_valid: true,
                    conformance_ready: true,
                    after: Some("b".repeat(64)),
                    limit: 12,
                    include_records: true,
                },
            })
        );

        let fetched = parse(
            [
                "ci",
                "provider-evidence-get",
                "--store",
                "state.json",
                "--digest",
                &"c".repeat(64),
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse provider evidence get");
        assert_eq!(
            fetched,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::CiProviderEvidenceGet {
                    store: PathBuf::from("state.json"),
                    digest: "c".repeat(64),
                },
            })
        );
    }

    #[test]
    fn evidence_domain_lineage_parses_digest_filters_and_child_control() {
        let parsed = parse(
            [
                "evidence",
                "domain-lineage",
                "--store",
                "artifacts.json",
                "--digest",
                &"a".repeat(64),
                "--group-id",
                "biological_domains",
                "--domain",
                "modalities",
                "--subject-id",
                "subject-1",
                "--source-tool",
                "modality_catalog",
                "--outcome",
                "partial",
                "--request-digest",
                &"b".repeat(64),
                "--response-digest",
                &"c".repeat(64),
                "--intake-digest",
                &"d".repeat(64),
                "--source-plan-digest",
                &"e".repeat(64),
                "--after",
                &"f".repeat(64),
                "--limit",
                "7",
                "--no-children",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse domain evidence lineage");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::EvidenceDomainLineage {
                    store: PathBuf::from("artifacts.json"),
                    digest: Some("a".repeat(64)),
                    group_id: Some("biological_domains".into()),
                    domain: Some("modalities".into()),
                    subject_id: Some("subject-1".into()),
                    source_tool: Some("modality_catalog".into()),
                    outcome: Some("partial".into()),
                    request_digest: Some("b".repeat(64)),
                    response_digest: Some("c".repeat(64)),
                    intake_digest: Some("d".repeat(64)),
                    source_plan_digest: Some("e".repeat(64)),
                    after: Some("f".repeat(64)),
                    limit: 7,
                    include_children: false,
                },
            })
        );
    }

    #[test]
    fn workflow_reconciliation_parses_retained_evidence_paths() {
        let parsed = parse(
            [
                "workflow",
                "reconcile",
                "--instantiation",
                "instantiation.json",
                "--mission",
                "mission.json",
                "--policy",
                "policy.json",
                "--readiness-audit",
                "readiness.json",
                "--require-readiness",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse workflow reconcile");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowReconcile {
                    instantiation: PathBuf::from("instantiation.json"),
                    mission: Some(PathBuf::from("mission.json")),
                    evidence_bundle: None,
                    policy: Some(PathBuf::from("policy.json")),
                    readiness_audit: Some(PathBuf::from("readiness.json")),
                    require_readiness: true,
                },
            })
        );
    }

    #[test]
    fn workflow_reconciliation_registry_commands_parse_filters_and_dry_run() {
        let imported = parse(
            [
                "workflow",
                "reconciliation-import",
                "--record",
                "record.json",
                "--store",
                "reconciliations.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse reconciliation import");
        assert_eq!(
            imported,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowReconciliationImport {
                    record: PathBuf::from("record.json"),
                    store: PathBuf::from("reconciliations.json"),
                    dry_run: true,
                },
            })
        );

        let queried = parse(
            [
                "workflow",
                "reconciliation-query",
                "--store",
                "reconciliations.json",
                "--mission-id",
                "m-1",
                "--workflow-id",
                "oncology",
                "--plan-digest",
                "d".repeat(64).as_str(),
                "--status",
                "complete",
                "--readiness-state",
                "ready_for_human_review",
                "--readiness-gate-satisfied",
                "--limit",
                "7",
                "--include-records",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse reconciliation query");
        assert_eq!(
            queried,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorkflowReconciliationQuery {
                    store: PathBuf::from("reconciliations.json"),
                    mission_id: Some("m-1".into()),
                    workflow_id: Some("oncology".into()),
                    mission_plan_digest: Some("d".repeat(64)),
                    completion_status: Some("complete".into()),
                    decision_readiness_state: Some("ready_for_human_review".into()),
                    decision_readiness_gate_satisfied: Some(true),
                    after: None,
                    limit: 7,
                    include_records: true,
                },
            })
        );
    }
}
