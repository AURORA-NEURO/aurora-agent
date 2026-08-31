//! Argument parsing.
//!
//! Hand-rolled rather than derived. The 40.13 contract wants golden help output, a stable
//! exit-code matrix and a `--json` mode with no stray bytes on stdout; owning the parser keeps
//! all three exactly specified and keeps the binary dependency-free.

use crate::exit::{CliError, CliResult, ExitCode, Retryability};
use bioprism_figures::FigureKind;
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
  world validate    --world <path> [--dimensions <path>]
                    Report structural diagnostics. Exit 1 if any are errors. --dimensions loads
                    a bioprism-scope-dimensions/0.1 document so domain scope dimensions are
                    classified instead of reported as unclassified.
  world show        --world <path>
                    Summarise facts, factors, events, tags and factor kinds.
  world sweep       [--distractors <n,n,...>] [--seed <n>] [--markdown]
                    Generate the structural family grid (attachment x relay depth x tag style x
                    distractor count, 43.39) and run the full baseline panel over every cell.
                    Ranking is on admissibility only. Exit 1 if FIBER is inadmissible anywhere.
  world index       --world <path> --store <dir> [--dry-run]
                    Build a content-addressed index so compile cost tracks the compiled region
                    rather than the corpus. Afterwards --world accepts the store directory.
  world generate    --family reference-like|discriminating [--distractors <n>]
                    [--world-out <path>] [--query-out <path>] [--dry-run]
                    Generate a synthetic structural family (43.39) and its matching query.

  context explain   --world <path> --query <path> [--domain <path>]
                    Show the compile plan: passes run, passes deferred, selection ratios and
                    omissions grouped by influence class. Writes nothing.
  context compile   --world <path> --query <path> [--domain <path>]
                    [--certificate-out <path>] [--section-out <path>]
                    [--profile reference|extended] [--dry-run] [--fail-on-invalid]
                    Compile and optionally write artifacts. --domain loads a bioprism-domain/0.1
                    pack and judges the compile with its rule oracle instead of the reference
                    split-integrity oracle; the output then carries the pack's advisories.
  context verify    --certificate <path>
                    Recompute the certificate digest. Exit 1 if it does not verify.
  context compare   --world <path> --query <path> [--markdown]
                    Run the equal-engineering baseline panel (full-context, graph k-hop,
                    hypergraph component, query-graph, lexical top-k, embedding top-k, directed
                    walk, FIBER) and report which strategies preserve the reference verdict and
                    at what cost. --domain is not supported here: the harness judges every
                    strategy against the reference oracle directly, so a pack oracle cannot yet
                    be applied to the whole panel.

  prism fork        --world <path> --query <path> [--bundle-out <path>] [--minimize]
                    Run every architecture from one frozen Decision Cell and report which
                    context policy explains the difference. Exit 1 if any architecture fails.
  prism minimize    --world <path>
                    Reduce the world to a 1-minimal set of facts that preserves the oracle
                    signature, then re-verify the reduction.

  mutate family     --world <path> [--out-dir <dir>]
                    Apply the standard metamorphic suite, validate every postcondition against
                    the oracle, deduplicate by content, and report effective diversity.

  project ingest    --root <dir> [--issues <path>] [--decision-time <rfc3339>]
                    --world-out <path> --pack-out <path> --dimensions-out <path>
                    [--queries-out <dir or .json path>] [--dry-run]
                    Scan a software project tree into a fiber-world/0.1 document, its
                    bioprism-scope-dimensions/0.1 classification, the release-readiness pack
                    and the generated fiber-query/0.2 documents. Static scan only: nothing is
                    executed or resolved, and every skipped byte ships as declared loss.
                    --queries-out ending in .json writes one document carrying the release query
                    and every issue query; otherwise a directory of release.json plus
                    issue-<id>.json.
  project audit     --root <dir> [--issues <path>] [--decision-time <rfc3339>]
                    Scan, assemble and judge the project under the release-readiness pack's
                    rule oracle. Exit 1 when the verdict is invalid. With --issues, also
                    compiles and prints each issue's declared evidence region.
  project plan      --root <dir> --issues <path> --issue <id> [--decision-time <rfc3339>]
                    [--criteria <path>] --out <path> [--dry-run]
                    Derive a repair plan for one declared issue from its compiled evidence
                    region and write it as a bioprism-repair-plan/0.1 document. --criteria loads
                    a bioprism-repair-declarations/0.1 file whose criteria, obligations and
                    falsifiers are recorded as declared by their author; the generator's own
                    items are recorded as derived and the two never merge. Nothing is edited,
                    built or run: the plan is a declaration of what would count as evidence.
  project verify    --root <dir> --plan <path> [--issues <path>] [--decision-time <rfc3339>]
                    Re-scan the tree and report which of the plan's declared criteria held,
                    each with its own three-valued status and the obstruction that stopped it.
                    Never reports that the issue is fixed. Exit 1 when the outcome is not_met
                    or falsified, 8 when a criterion or falsifier could not be evaluated, and 9
                    when the plan is bound to a different world — a stale plan evaluates
                    nothing, so it is not a failed verification. Obligations are reported on
                    their own admissibility axis and never move the exit code.

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

  autopilot grant-template
                    Print a template autonomy grant to stdout (with --json, the bare grant
                    object, directly usable as --grant). Authority for autonomous dispatch
                    comes only from an explicit grant document; there is no default grant, and
                    nothing is written.
  autopilot run     --instantiation <path> --grant <path> [--report-out <path>] [--dry-run]
                    Drive an instantiated workflow's mission under an explicit autonomy grant:
                    dispatch, classify each failure by its 40.36 retry class, repair what the
                    grant authorises, and chain every mission report and reconciliation digest
                    into one autopilot report. --dry-run plans attempt 1 only, no-dispatch:
                    nothing runs and nothing is written. Exit 1 reports a completed drive whose
                    final status is exhausted or refused rather than succeeded.
  autopilot verify  --report <path>
                    Recompute an autopilot report's digest and require its stated limitations.
                    Exit 1 if the report does not verify.

  research template
                    Print a template research request to stdout (with --json, the bare request
                    object, directly usable as --request). The question field is recorded
                    verbatim and never interpreted: the runner executes the protocol the other
                    fields declare, over synthetic decision worlds only.
  research run      --request <path> --out-dir <dir> [--dry-run]
                    Plan and execute the research protocol: anchor to the pinned reference
                    certificate, then generate, compile and compare each declared distractor
                    point, with optional sweep, mutation and minimization steps. Writes
                    dossier.json, REPORT.md and figures/*.svg into --out-dir. --dry-run prints
                    the planned protocol, no-dispatch: nothing runs and nothing is written.
                    A completed run exits 0 even when every finding is negative; a tie is a
                    first-class result, not a failure. Exit 3 for an invalid request, 5 when
                    an artifact cannot be written.
  research verify   --dossier <path>
                    Recompute a research dossier's digest and check its structural contract
                    (request digest, required limitations, step outcomes, finding support).
                    Exit 1 if the dossier does not verify.

  figure list       --input <path>
                    Report what is drawable in a JSON artifact and where, without writing
                    anything. Recognition is structural — required key sets and declared schema
                    strings, never the filename — so the answer does not change when a file is
                    renamed. A document holding nothing this builder draws is reported as such
                    and still exits 0: listing succeeded, and an empty list is the answer.
  figure render     --input <path> [--out-dir <dir>] [--kind <kind>] [--pointer <json-pointer>]
                    [--dry-run]
                    Render every drawable region of one document to SVG, or just the ones
                    --kind and --pointer select. Each figure's footer carries the canonical
                    digest of the exact value it was drawn from: that hex identifies the
                    artifact, it does not attest that the artifact is correct. --out-dir
                    defaults to ./figures. --dry-run reports what would be written and writes
                    nothing. Exit 1 when the selection is empty — the document holds nothing
                    drawable, or --kind/--pointer matched none of what it holds. That is a
                    verdict about the input, not a failure of this command. Exit 3 for a
                    document that is not readable JSON or that no figure can be drawn from,
                    and exit 5 when a figure cannot be written.
  figure batch      --input-dir <dir> [--out-dir <dir>] [--dry-run]
                    Render every drawable region of every *.json file directly inside a
                    directory (non-recursive: subdirectories are not walked) and write a
                    manifest.json naming every figure produced and every input skipped, with
                    the reason. A skip never moves the exit code by itself: the code follows
                    whether any figure was produced at all. Figures land in
                    --out-dir/<input file stem>/, so two inputs cannot overwrite each other's
                    output. Exit 1 when nothing in the directory was drawable — the manifest is
                    still written, because it is the answer.

  figure kinds: baseline-panel, selection-ratio, omission-accounting, sweep-grid,
                mutation-diversity, autopilot-drive.

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
        dimensions: Option<PathBuf>,
    },
    WorldShow {
        world: PathBuf,
    },
    WorldSweep {
        distractors: Option<Vec<usize>>,
        seed: Option<u64>,
        markdown: bool,
    },
    ContextExplain {
        world: PathBuf,
        query: PathBuf,
        domain: Option<PathBuf>,
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
    ProjectIngest(ProjectIngestOptions),
    ProjectAudit {
        root: PathBuf,
        issues: Option<PathBuf>,
        decision_time: Option<String>,
    },
    ProjectPlan(ProjectPlanOptions),
    ProjectVerify {
        root: PathBuf,
        plan: PathBuf,
        issues: Option<PathBuf>,
        decision_time: Option<String>,
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
    AutopilotGrantTemplate,
    AutopilotRun {
        instantiation: PathBuf,
        grant: PathBuf,
        report_out: Option<PathBuf>,
        dry_run: bool,
    },
    AutopilotVerify {
        report: PathBuf,
    },
    ResearchTemplate,
    ResearchRun {
        request: PathBuf,
        out_dir: PathBuf,
        dry_run: bool,
    },
    ResearchVerify {
        dossier: PathBuf,
    },
    FigureList {
        input: PathBuf,
    },
    FigureRender {
        input: PathBuf,
        out_dir: PathBuf,
        kind: Option<FigureKind>,
        pointer: Option<String>,
        dry_run: bool,
    },
    FigureBatch {
        input_dir: PathBuf,
        out_dir: PathBuf,
        dry_run: bool,
    },
}

/// Where `figure render` and `figure batch` write when the caller names no directory.
///
/// A default rather than a required flag because the overwhelmingly common invocation is "draw
/// this file", and `research run` already writes its SVGs into a `figures/` directory beside the
/// dossier — a caller who has seen one layout should not have to learn a second. Stated in
/// `--help` rather than left to be discovered by finding the files.
const DEFAULT_FIGURE_OUT_DIR: &str = "figures";

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
pub struct ProjectIngestOptions {
    pub root: PathBuf,
    pub issues: Option<PathBuf>,
    /// Already validated as RFC 3339 by the parser; kept as the caller's exact string so the
    /// emitted world carries their bytes, not a re-rendering.
    pub decision_time: Option<String>,
    pub world_out: PathBuf,
    pub pack_out: PathBuf,
    pub dimensions_out: PathBuf,
    pub queries_out: Option<PathBuf>,
    pub dry_run: bool,
}

/// `project plan`'s parsed invocation.
///
/// `issues` is required here although `project ingest` and `project audit` both take it
/// optionally. Those two commands have something to say about a tree with no declared issues; a
/// plan is *for* one issue, so an invocation naming none has no subject, and defaulting to an
/// empty issue list would turn that into "issue not found in the world" — a diagnostic pointing
/// at the tree rather than at the missing flag.
#[derive(Debug, PartialEq, Eq)]
pub struct ProjectPlanOptions {
    pub root: PathBuf,
    pub issues: PathBuf,
    pub issue: String,
    /// Already validated as RFC 3339 by the parser; kept as the caller's exact string.
    pub decision_time: Option<String>,
    /// A `bioprism-repair-declarations/0.1` document, or nothing: a plan with no declared items
    /// carries only what the generator could derive, and says so in its own limitations.
    pub criteria: Option<PathBuf>,
    pub out: PathBuf,
    pub dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub world: PathBuf,
    pub query: PathBuf,
    pub domain: Option<PathBuf>,
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
            dimensions: options.take_optional_path("--dimensions"),
        },
        ("world", "show") => Command::WorldShow {
            world: options.take_path("--world")?,
        },
        ("world", "sweep") => Command::WorldSweep {
            distractors: match options.take_optional("--distractors") {
                None => None,
                Some(text) => Some(
                    text.split(',')
                        .map(|part| {
                            part.trim().parse::<usize>().map_err(|_| {
                                usage(format!(
                                    "--distractors must be a comma-separated list of numbers, \
                                     got {text:?}"
                                ))
                            })
                        })
                        .collect::<CliResult<Vec<usize>>>()?,
                ),
            },
            seed: match options.take_optional("--seed") {
                None => None,
                Some(text) => Some(
                    text.parse()
                        .map_err(|_| usage(format!("--seed must be a number, got {text:?}")))?,
                ),
            },
            markdown: options.take_switch("--markdown"),
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
            domain: options.take_optional_path("--domain"),
        },
        ("context", "compile") => Command::ContextCompile(CompileOptions {
            world: options.take_path("--world")?,
            query: options.take_path("--query")?,
            domain: options.take_optional_path("--domain"),
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
        ("project", "ingest") => Command::ProjectIngest(ProjectIngestOptions {
            root: options.take_path("--root")?,
            issues: options.take_optional_path("--issues"),
            decision_time: take_decision_time(&mut options)?,
            world_out: options.take_path("--world-out")?,
            pack_out: options.take_path("--pack-out")?,
            dimensions_out: options.take_path("--dimensions-out")?,
            queries_out: options.take_optional_path("--queries-out"),
            dry_run: options.take_switch("--dry-run"),
        }),
        ("project", "audit") => Command::ProjectAudit {
            root: options.take_path("--root")?,
            issues: options.take_optional_path("--issues"),
            decision_time: take_decision_time(&mut options)?,
        },
        ("project", "plan") => Command::ProjectPlan(ProjectPlanOptions {
            root: options.take_path("--root")?,
            issues: options.take_path("--issues")?,
            issue: options
                .take_optional("--issue")
                .ok_or_else(|| usage("--issue is required and takes a declared issue id"))?,
            decision_time: take_decision_time(&mut options)?,
            criteria: options.take_optional_path("--criteria"),
            out: options.take_path("--out")?,
            dry_run: options.take_switch("--dry-run"),
        }),
        ("project", "verify") => Command::ProjectVerify {
            root: options.take_path("--root")?,
            plan: options.take_path("--plan")?,
            issues: options.take_optional_path("--issues"),
            decision_time: take_decision_time(&mut options)?,
        },
        // `--domain` is refused here rather than silently accepted-and-ignored. The comparison
        // harness (`bioprism_baseline::compare`) judges every strategy's selection against the
        // reference split-integrity oracle directly, not through the injectable
        // `DecisionOracle`, so a pack oracle cannot yet be applied to the whole panel — and a
        // table whose reference verdict came from a different oracle than the flag named would
        // be a half-truth, not a comparison.
        ("context", "compare") => {
            if options.take_optional("--domain").is_some() {
                return Err(usage(
                    "--domain is not supported on context compare: the comparison harness judges \
                     every strategy against the reference split-integrity oracle directly, so a \
                     pack oracle cannot yet be applied to the whole panel; compile under the pack \
                     with `context compile --domain` instead",
                ));
            }
            Command::ContextCompare {
                world: options.take_path("--world")?,
                query: options.take_path("--query")?,
                markdown: options.take_switch("--markdown"),
            }
        }
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
        ("autopilot", "grant-template") => Command::AutopilotGrantTemplate,
        ("autopilot", "run") => Command::AutopilotRun {
            instantiation: options.take_path("--instantiation")?,
            grant: options.take_path("--grant")?,
            report_out: options.take_optional_path("--report-out"),
            dry_run: options.take_switch("--dry-run"),
        },
        ("autopilot", "verify") => Command::AutopilotVerify {
            report: options.take_path("--report")?,
        },
        ("research", "template") => Command::ResearchTemplate,
        ("research", "run") => Command::ResearchRun {
            request: options.take_path("--request")?,
            out_dir: options.take_path("--out-dir")?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("research", "verify") => Command::ResearchVerify {
            dossier: options.take_path("--dossier")?,
        },
        ("figure", "list") => Command::FigureList {
            input: options.take_path("--input")?,
        },
        ("figure", "render") => Command::FigureRender {
            input: options.take_path("--input")?,
            out_dir: take_figure_out_dir(&mut options),
            kind: take_figure_kind(&mut options)?,
            pointer: take_figure_pointer(&mut options)?,
            dry_run: options.take_switch("--dry-run"),
        },
        ("figure", "batch") => Command::FigureBatch {
            input_dir: options.take_path("--input-dir")?,
            out_dir: take_figure_out_dir(&mut options),
            dry_run: options.take_switch("--dry-run"),
        },
        _ => return Err(usage(format!("unknown command {group:?} {subcommand:?}"))),
    };

    options.reject_leftovers()?;
    Ok(Parsed::Run(Invocation { json, command }))
}

fn take_figure_out_dir(options: &mut Options) -> PathBuf {
    options
        .take_optional_path("--out-dir")
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FIGURE_OUT_DIR))
}

/// Takes `--kind`, refusing any name outside `bioprism-figures`' own registry.
///
/// Validated here rather than after the document is read, because an unrecognised kind is a
/// mistyped flag and not a defect in the artifact: accepted and matched later, it would surface
/// as "nothing drawable selected" (exit 1) and send the caller to inspect a file that is fine.
/// The registry is quantified over rather than restated, so a figure added to the crate becomes
/// typeable here without an edit.
fn take_figure_kind(options: &mut Options) -> CliResult<Option<FigureKind>> {
    match options.take_optional("--kind") {
        None => Ok(None),
        Some(text) => match FigureKind::from_slug(&text) {
            Some(kind) => Ok(Some(kind)),
            None => Err(usage(format!(
                "--kind must be one of {}, got {text:?}",
                FigureKind::ALL
                    .iter()
                    .map(|kind| kind.slug())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        },
    }
}

/// Takes `--pointer`, refusing anything that is not an RFC 6901 JSON pointer.
///
/// `--pointer report` names nothing under RFC 6901, and a pointer that names nothing selects no
/// figure — which the command would otherwise report as "the document holds nothing at that
/// pointer" when the real fault is the missing leading slash. The empty string is accepted and
/// means the document root, exactly as `figure list` prints it.
fn take_figure_pointer(options: &mut Options) -> CliResult<Option<String>> {
    match options.take_optional("--pointer") {
        None => Ok(None),
        Some(text) if text.is_empty() || text.starts_with('/') => Ok(Some(text)),
        Some(text) => Err(usage(format!(
            "--pointer must be an RFC 6901 JSON pointer: either empty for the document root, or \
             beginning with `/`; got {text:?}"
        ))),
    }
}

/// Takes `--decision-time`, refusing anything the workspace's own RFC 3339 parser refuses.
///
/// Validated here rather than deep inside assembly, because there the malformed string would
/// surface as the emitted world failing the reference validator — which reads as a bug in this
/// binary, when in fact the flag value is the thing that needs editing. The caller's exact
/// string is kept: the parse is a gate, not a normalisation.
fn take_decision_time(options: &mut Options) -> CliResult<Option<String>> {
    match options.take_optional("--decision-time") {
        None => Ok(None),
        Some(text) => {
            bioprism_scope::Timestamp::parse(&text)
                .map_err(|error| usage(format!("--decision-time must be RFC 3339: {error}")))?;
            Ok(Some(text))
        }
    }
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
                    if let Some(next) = cursor.next() {
                        values.push((token, Some(next)));
                    } else {
                        values.push((token, None));
                    }
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
    use super::{parse, Command, FigureKind, Parsed};
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
    fn help_documents_the_domain_flag_where_it_works_and_where_it_is_refused() {
        let text = super::help();
        assert!(text.contains("context compile   --world <path> --query <path> [--domain <path>]"));
        assert!(text.contains("context explain   --world <path> --query <path> [--domain <path>]"));
        assert!(
            text.contains("--domain is not supported here"),
            "compare's help must state the refusal rather than imply support"
        );
        assert!(text.contains("world validate    --world <path> [--dimensions <path>]"));
        assert!(
            text.contains("world sweep       [--distractors <n,n,...>] [--seed <n>] [--markdown]")
        );
    }

    #[test]
    fn help_documents_both_project_commands_and_every_flag_they_parse() {
        let text = super::help();
        assert!(text.contains("project ingest    --root <dir>"));
        assert!(text.contains("project audit     --root <dir>"));
        for flag in [
            "--issues",
            "--decision-time",
            "--world-out",
            "--pack-out",
            "--dimensions-out",
            "--queries-out",
        ] {
            assert!(
                text.contains(flag),
                "a flag the project parser accepts must be documented: {flag}"
            );
        }
        assert!(
            text.contains("Exit 1 when the verdict is invalid"),
            "the audit's exit contract is the reason to run it and must be stated"
        );
    }

    #[test]
    fn project_ingest_parses_every_declared_output_path_and_the_dry_run_switch() {
        let parsed = parse(
            [
                "project",
                "ingest",
                "--root",
                "tree",
                "--issues",
                "issues.json",
                "--decision-time",
                "2024-01-01T00:00:00Z",
                "--world-out",
                "world.json",
                "--pack-out",
                "pack.json",
                "--dimensions-out",
                "dimensions.json",
                "--queries-out",
                "queries",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse project ingest");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::ProjectIngest(super::ProjectIngestOptions {
                    root: PathBuf::from("tree"),
                    issues: Some(PathBuf::from("issues.json")),
                    decision_time: Some("2024-01-01T00:00:00Z".into()),
                    world_out: PathBuf::from("world.json"),
                    pack_out: PathBuf::from("pack.json"),
                    dimensions_out: PathBuf::from("dimensions.json"),
                    queries_out: Some(PathBuf::from("queries")),
                    dry_run: true,
                }),
            })
        );
    }

    #[test]
    fn help_documents_the_repair_commands_and_the_exit_code_a_stale_plan_reports() {
        let text = super::help();
        assert!(text.contains("project plan      --root <dir>"));
        assert!(text.contains("project verify    --root <dir>"));
        for flag in ["--issue ", "--criteria", "--out ", "--plan "] {
            assert!(
                text.contains(flag),
                "a flag the repair parser accepts must be documented: {flag:?}"
            );
        }
        assert!(
            text.contains("Exit 1 when the outcome is not_met")
                && text.contains("a stale plan evaluates"),
            "the three verdict-bearing exit codes are what a caller branches on and must be \
             stated, staleness included:\n{text}"
        );
        assert!(
            text.contains("Never reports that the issue is fixed"),
            "the help must state the refusal the whole command exists for:\n{text}"
        );
    }

    #[test]
    fn project_plan_requires_the_issue_it_plans_for_rather_than_defaulting_to_none() {
        let error = parse(
            [
                "project",
                "plan",
                "--root",
                "tree",
                "--issues",
                "issues.json",
                "--out",
                "plan.json",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("a plan with no subject must be refused");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
        assert!(
            error.message.contains("--issue is required"),
            "the message must name the flag the operator has to add: {}",
            error.message
        );

        let without_issues = parse(
            [
                "project",
                "plan",
                "--root",
                "tree",
                "--issue",
                "ISSUE-1",
                "--out",
                "plan.json",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("planning against a tree with no declared issues must be refused");
        assert!(
            without_issues.message.contains("--issues"),
            "an absent issues file must point at the flag, not at the tree: {}",
            without_issues.message
        );
    }

    #[test]
    fn project_plan_parses_its_declaration_file_output_path_and_dry_run_switch() {
        let parsed = parse(
            [
                "project",
                "plan",
                "--root",
                "tree",
                "--issues",
                "issues.json",
                "--issue",
                "ISSUE-1",
                "--decision-time",
                "2024-01-01T00:00:00Z",
                "--criteria",
                "declared.json",
                "--out",
                "plan.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse project plan");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::ProjectPlan(super::ProjectPlanOptions {
                    root: PathBuf::from("tree"),
                    issues: PathBuf::from("issues.json"),
                    issue: "ISSUE-1".into(),
                    decision_time: Some("2024-01-01T00:00:00Z".into()),
                    criteria: Some(PathBuf::from("declared.json")),
                    out: PathBuf::from("plan.json"),
                    dry_run: true,
                }),
            })
        );
    }

    #[test]
    fn project_verify_parses_the_plan_it_checks_and_the_tree_it_checks_it_against() {
        let parsed = parse(
            [
                "--json",
                "project",
                "verify",
                "--root",
                "tree",
                "--plan",
                "plan.json",
                "--issues",
                "issues.json",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse project verify");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::ProjectVerify {
                    root: PathBuf::from("tree"),
                    plan: PathBuf::from("plan.json"),
                    issues: Some(PathBuf::from("issues.json")),
                    decision_time: None,
                },
            })
        );
    }

    #[test]
    fn a_decision_time_that_is_not_rfc_3339_is_refused_at_the_flag_rather_than_inside_assembly() {
        let error = parse(
            [
                "project",
                "audit",
                "--root",
                "tree",
                "--decision-time",
                "yesterday",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("a malformed decision time must be a usage error");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
        assert!(
            error.message.contains("--decision-time must be RFC 3339"),
            "the message must name the flag the operator has to edit: {}",
            error.message
        );
    }

    #[test]
    fn context_compare_refuses_a_domain_pack_with_the_reason_in_the_message() {
        let error = parse(
            [
                "context",
                "compare",
                "--world",
                "w.json",
                "--query",
                "q.json",
                "--domain",
                "pack.json",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("--domain on compare must be refused, not ignored");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
        assert!(
            error.message.contains("not supported on context compare"),
            "the refusal must say why: {}",
            error.message
        );
    }

    #[test]
    fn world_sweep_parses_a_comma_separated_distractor_grid_and_rejects_a_non_numeric_one() {
        let parsed = parse(
            ["world", "sweep", "--distractors", "50,250", "--seed", "7"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse world sweep");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::WorldSweep {
                    distractors: Some(vec![50, 250]),
                    seed: Some(7),
                    markdown: false,
                },
            })
        );

        let error = parse(
            ["world", "sweep", "--distractors", "50,many"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("a non-numeric distractor count is a usage error");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
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
    fn autopilot_run_parses_instantiation_grant_report_out_and_dry_run() {
        let parsed = parse(
            [
                "--json",
                "autopilot",
                "run",
                "--instantiation",
                "instantiation.json",
                "--grant",
                "grant.json",
                "--report-out",
                "autopilot-report.json",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse autopilot run");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::AutopilotRun {
                    instantiation: PathBuf::from("instantiation.json"),
                    grant: PathBuf::from("grant.json"),
                    report_out: Some(PathBuf::from("autopilot-report.json")),
                    dry_run: true,
                },
            })
        );
    }

    #[test]
    fn autopilot_run_refuses_an_invocation_without_a_grant() {
        let refused = parse(
            ["autopilot", "run", "--instantiation", "instantiation.json"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("a run without a grant must be refused, never defaulted");
        assert!(refused.message.contains("--grant"), "{}", refused.message);
    }

    #[test]
    fn autopilot_grant_template_takes_no_options_and_verify_takes_a_report_path() {
        let template = parse(
            ["autopilot", "grant-template"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse autopilot grant-template");
        assert_eq!(
            template,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::AutopilotGrantTemplate,
            })
        );
        parse(
            ["autopilot", "grant-template", "--out", "grant.json"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("grant-template writes nothing and accepts no options");

        let verified = parse(
            ["autopilot", "verify", "--report", "autopilot-report.json"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse autopilot verify");
        assert_eq!(
            verified,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::AutopilotVerify {
                    report: PathBuf::from("autopilot-report.json"),
                },
            })
        );
    }

    #[test]
    fn help_documents_every_autopilot_subcommand() {
        let text = super::help();
        assert!(text.contains("autopilot grant-template"));
        assert!(text.contains(
            "autopilot run     --instantiation <path> --grant <path> [--report-out <path>] [--dry-run]"
        ));
        assert!(text.contains("autopilot verify  --report <path>"));
        assert!(
            text.contains("only from an explicit grant document"),
            "help must say where autonomous authority comes from"
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

    #[test]
    fn research_run_parses_request_out_dir_and_dry_run() {
        let parsed = parse(
            [
                "--json",
                "research",
                "run",
                "--request",
                "request.json",
                "--out-dir",
                "research-out",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse research run");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::ResearchRun {
                    request: PathBuf::from("request.json"),
                    out_dir: PathBuf::from("research-out"),
                    dry_run: true,
                },
            })
        );
    }

    #[test]
    fn research_run_requires_both_the_request_and_the_out_dir() {
        let without_request = parse(
            ["research", "run", "--out-dir", "research-out"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("a run without a request must be refused, never defaulted");
        assert_eq!(without_request.code, crate::exit::ExitCode::Usage);
        assert!(
            without_request.message.contains("--request"),
            "the message must name the flag the operator has to add: {}",
            without_request.message
        );

        let without_out_dir = parse(
            ["research", "run", "--request", "request.json"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("a run without an out-dir has nowhere to put the dossier it promises");
        assert!(
            without_out_dir.message.contains("--out-dir"),
            "{}",
            without_out_dir.message
        );
    }

    #[test]
    fn research_template_takes_no_options_and_verify_takes_a_dossier_path() {
        let template = parse(["research", "template"].into_iter().map(String::from))
            .expect("parse research template");
        assert_eq!(
            template,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::ResearchTemplate,
            })
        );
        parse(
            ["research", "template", "--out", "request.json"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("research template writes nothing and accepts no options");

        let verified = parse(
            ["research", "verify", "--dossier", "dossier.json"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse research verify");
        assert_eq!(
            verified,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::ResearchVerify {
                    dossier: PathBuf::from("dossier.json"),
                },
            })
        );
    }

    #[test]
    fn help_documents_every_research_subcommand() {
        let text = super::help();
        assert!(text.contains("research template"));
        assert!(text.contains("research run      --request <path> --out-dir <dir> [--dry-run]"));
        assert!(text.contains("research verify   --dossier <path>"));
        assert!(
            text.contains("verbatim and never interpreted"),
            "help must state that the question is recorded, not understood"
        );
        assert!(
            text.contains("exits 0 even when every finding is negative"),
            "help must state that a negative finding is a completed run, not a failure"
        );
    }

    #[test]
    fn figure_render_defaults_its_output_directory_rather_than_demanding_one() {
        let parsed = parse(
            ["figure", "render", "--input", "dossier.json"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse figure render");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::FigureRender {
                    input: PathBuf::from("dossier.json"),
                    out_dir: PathBuf::from("figures"),
                    kind: None,
                    pointer: None,
                    dry_run: false,
                },
            }),
            "the default is `figures/`, the layout `research run` already writes"
        );
    }

    #[test]
    fn figure_render_carries_its_kind_pointer_and_dry_run_through_the_parser() {
        let parsed = parse(
            [
                "--json",
                "figure",
                "render",
                "--input",
                "dossier.json",
                "--out-dir",
                "out",
                "--kind",
                "sweep-grid",
                "--pointer",
                "/steps/10/outputs/0/artifact",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse a filtered figure render");
        assert_eq!(
            parsed,
            Parsed::Run(super::Invocation {
                json: true,
                command: Command::FigureRender {
                    input: PathBuf::from("dossier.json"),
                    out_dir: PathBuf::from("out"),
                    kind: Some(FigureKind::SweepGrid),
                    pointer: Some("/steps/10/outputs/0/artifact".to_string()),
                    dry_run: true,
                },
            })
        );
    }

    #[test]
    fn a_kind_outside_the_figure_registry_is_refused_at_parse_time_naming_the_registry() {
        let error = parse(
            [
                "figure",
                "render",
                "--input",
                "x.json",
                "--kind",
                "pie-chart",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("an unknown figure name is a mistyped flag, not a defect in the document");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
        for slug in FigureKind::ALL.iter().map(|kind| kind.slug()) {
            assert!(
                error.message.contains(slug),
                "the refusal must name the whole registry, and is missing {slug}: {}",
                error.message
            );
        }
    }

    #[test]
    fn a_pointer_without_a_leading_slash_is_refused_but_the_empty_root_pointer_is_accepted() {
        let error = parse(
            [
                "figure",
                "render",
                "--input",
                "x.json",
                "--pointer",
                "report",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect_err("`report` names nothing under RFC 6901 and must not be silently accepted");
        assert_eq!(error.code, crate::exit::ExitCode::Usage);
        assert!(error.message.contains("RFC 6901"), "{}", error.message);

        let root = parse(
            ["figure", "render", "--input", "x.json", "--pointer="]
                .into_iter()
                .map(String::from),
        )
        .expect("the empty pointer is the document root and is a legitimate selection");
        assert_eq!(
            root,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::FigureRender {
                    input: PathBuf::from("x.json"),
                    out_dir: PathBuf::from("figures"),
                    kind: None,
                    pointer: Some(String::new()),
                    dry_run: false,
                },
            })
        );
    }

    #[test]
    fn figure_list_takes_only_an_input_and_batch_takes_an_input_directory() {
        let listed = parse(
            ["figure", "list", "--input", "cert.json"]
                .into_iter()
                .map(String::from),
        )
        .expect("parse figure list");
        assert_eq!(
            listed,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::FigureList {
                    input: PathBuf::from("cert.json"),
                },
            })
        );
        parse(
            ["figure", "list", "--input", "cert.json", "--out-dir", "out"]
                .into_iter()
                .map(String::from),
        )
        .expect_err("`figure list` writes nothing, so an output directory is meaningless");

        let batched = parse(
            [
                "figure",
                "batch",
                "--input-dir",
                "artifacts",
                "--out-dir",
                "out",
                "--dry-run",
            ]
            .into_iter()
            .map(String::from),
        )
        .expect("parse figure batch");
        assert_eq!(
            batched,
            Parsed::Run(super::Invocation {
                json: false,
                command: Command::FigureBatch {
                    input_dir: PathBuf::from("artifacts"),
                    out_dir: PathBuf::from("out"),
                    dry_run: true,
                },
            })
        );
        let missing = parse(["figure", "batch"].into_iter().map(String::from))
            .expect_err("a batch without a directory has nothing to walk");
        assert!(
            missing.message.contains("--input-dir"),
            "{}",
            missing.message
        );
    }

    #[test]
    fn help_documents_the_figure_group_and_what_its_digests_do_not_prove() {
        let text = super::help();
        assert!(text.contains("figure list       --input <path>"));
        assert!(text.contains("figure render     --input <path>"));
        assert!(text.contains("figure batch      --input-dir <dir>"));
        assert!(
            text.contains("it does not attest that the artifact is correct"),
            "help must say what the footer digest does not prove"
        );
        assert!(
            text.contains("That is a\n                    verdict about the input"),
            "help must state that an empty selection is a verdict rather than a failure"
        );
        assert!(
            text.contains("non-recursive: subdirectories are not walked"),
            "help must state the walk's boundary rather than leave it to be discovered"
        );
        for slug in FigureKind::ALL.iter().map(|kind| kind.slug()) {
            assert!(
                text.contains(slug),
                "help must list every figure a caller can name with --kind, and is missing \
                 {slug}"
            );
        }
    }
}
