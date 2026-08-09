//! The eighty-four, transcribed.
//!
//! Every module `docs/BACKLOG.md` lists, with the verdict a classifying crate reached about it and
//! the file that verdict was written in. Nothing here is a new judgement about a module a crate
//! already judged: where `crates/stewardship` said process, this says process, in its words, with a
//! fragment of its sentence that must still be present for the entry to stand.
//!
//! # Where the judgements came from
//!
//! Verdicts are attributed to fourteen crates, and they arrived three ways.
//!
//! **Seven crates had classified their section's remainder explicitly**, and their tables are
//! transcribed row for row: `crates/devplat` (§11 and §19), `crates/ops` (§40),
//! `crates/stewardship` (§14), `crates/atlashub` (§34), `crates/atlasx` (§33), `crates/sweep` (§04
//! and §10) and `crates/bioevalx` (§26). Between them they decide fifty-one modules, and where two
//! of them decide the same one differently — `crates/atlasx` and `bioprism-metrics` on the whole of
//! §33 — both readings are recorded and neither is adjudicated.
//!
//! **Seven more had recorded a position without naming the module this register is explaining**,
//! because the content lives under a different section's id or because the capability the module
//! needs does not exist: `crates/worldfactory`, `crates/scale` and `crates/factory` between them
//! decide all six of §35, and `crates/infra`, `crates/ops`, `bioprism-safety` and `bioprism-trace`
//! supply the stated blockers under §12 and §36. Every one of those is marked
//! [`Standing::InferredHere`](crate::Standing::InferredHere), so a reader may disagree with this
//! register about them without disagreeing with those crates.
//!
//! **Nineteen modules had nobody at all**: §23's nine, four of §12's and six of §36's. **A survey
//! that found nothing is still a finding, and it is recorded with the crates that were read.**
//! Those entries name what was searched, so the next reader starts from a list rather than from
//! scratch.
//!
//! # How to regenerate this file
//!
//! 1. Run `tools/backlog.sh` and diff `docs/BACKLOG.md` against
//!    [`reconcile::reconcile`](crate::reconcile::reconcile).
//! 2. For a module that has **left** the backlog — the normal case, because four crates are being
//!    written against these sections right now — delete its entry, or call
//!    [`Register::without`](crate::Register::without) if the caller is a script. Nothing else moves:
//!    entries hold no cross-references to each other.
//! 3. For a module that has **arrived**, read the crate that owns its section, find the sentence
//!    that classified it, and add an entry whose anchor is a fragment of that sentence. If there is
//!    no such sentence, the verdict is
//!    [`GenuinelyUncovered`](crate::Classification::GenuinelyUncovered) and the survey is the list
//!    of crates that were read while looking. It is not a placeholder and it must not be treated as
//!    one.
//! 4. Never write a dotted id. [`crate::citations`] fails the build if one appears.

use crate::entry::{Entry, Register};
use crate::error::RegisterError;
use crate::module::ModuleKey;
use crate::verdict::{
    Classification, ForeignSurface, Source, UncoveredStanding, Verdict, THIS_REGISTER,
};

const STEWARDSHIP: &str = "crates/stewardship/src/lib.rs";
const ATLASHUB: &str = "crates/atlashub/src/lib.rs";
const ATLASX: &str = "crates/atlasx/src/lib.rs";
const METRICS: &str = "crates/metrics/src/lib.rs";
const DEVPLAT: &str = "crates/devplat/src/classify.rs";
const OPS: &str = "crates/ops/src/lib.rs";
const SWEEP: &str = "crates/sweep/src/lib.rs";
const TRACE: &str = "crates/trace/src/lib.rs";
const BIOEVALX: &str = "crates/bioevalx/src/lib.rs";
const WORLDFACTORY: &str = "crates/worldfactory/src/coverage.rs";
const SCALE: &str = "crates/scale/src/lib.rs";
const FACTORY: &str = "crates/factory/src/lib.rs";
const INFRA: &str = "crates/infra/src/lib.rs";
const SAFETY: &str = "crates/safety/src/lib.rs";
const HERE: &str = "crates/residue/src/register.rs";

/// The sentence every "nobody has read it" verdict anchors on, in this file's own documentation.
///
/// A survey is the one verdict whose evidence is an absence, so it cannot point at another crate's
/// sentence. It points at this one, and the accompanying survey names where the search ran.
const SURVEY_ANCHOR: &str = "A survey that found nothing is still a finding";

fn transcribed(
    krate: &str,
    locus: &str,
    needle: &str,
    reasoning: &str,
    classification: Classification,
) -> Result<Verdict, RegisterError> {
    let source = Source::transcribed(krate, locus, needle, reasoning)?;
    Ok(Verdict::record(classification, source)?)
}

fn inferred(
    krate: &str,
    locus: &str,
    needle: &str,
    reasoning: &str,
    classification: Classification,
) -> Result<Verdict, RegisterError> {
    let source = Source::inferred(krate, locus, needle, reasoning)?;
    Ok(Verdict::record(classification, source)?)
}

fn unread(reasoning: &str, surveyed: &[&str]) -> Result<Verdict, RegisterError> {
    let classification = Classification::GenuinelyUncovered {
        standing: UncoveredStanding::nobody_has_read(surveyed)?,
    };
    inferred(
        THIS_REGISTER,
        HERE,
        SURVEY_ANCHOR,
        reasoning,
        classification,
    )
}

fn entry(
    section: u8,
    index: u8,
    title: &str,
    verdicts: Vec<Verdict>,
) -> Result<Entry, RegisterError> {
    Entry::new(ModuleKey::new(section, index)?, title, verdicts)
}

fn foreign(surface: ForeignSurface) -> Classification {
    Classification::ForeignArtifact { surface }
}

/// The whole register.
///
/// Returns a `Result` rather than panicking because every entry passes the same construction gate a
/// caller building one by hand would pass, and a register that silently degraded when an anchor was
/// too thin would be the failure this crate exists to prevent.
pub fn residue() -> Result<Register, RegisterError> {
    let mut entries = Vec::new();
    entries.extend(developer_platform()?);
    entries.extend(reference_examples()?);
    entries.extend(governance_and_quality()?);
    entries.extend(capability_metrics()?);
    entries.extend(public_hub()?);
    entries.extend(engineering_contracts()?);
    entries.extend(million_scale()?);
    entries.extend(agent_interweave()?);
    entries.extend(data_and_infrastructure()?);
    entries.extend(biology_governance()?);
    entries.extend(small_remainders()?);
    Register::new(entries)
}

/// §11, thirteen modules, all classified by `crates/devplat`.
///
/// The largest single bucket of foreign artifacts in the workspace and the reason the vocabulary
/// needs the category at all: seven of these specify a real, precise, testable artifact that is
/// simply not a Rust crate. Calling them process would be as wrong as implementing them.
fn developer_platform() -> Result<Vec<Entry>, RegisterError> {
    let foreign_row = |needle: &str, reasoning: &str, surface: ForeignSurface| {
        transcribed(
            "bioprism-devplat",
            DEVPLAT,
            needle,
            reasoning,
            foreign(surface),
        )
    };

    Ok(vec![
        entry(
            11,
            4,
            "Python Sdk",
            vec![foreign_row(
                "title: \"Python SDK\",",
                "A Python distribution with nine importable packages. Precise, code-bearing and \
                 unwritable here: no Rust type can be the module, and nothing in this repository \
                 can look for `prism.compiler.mine` now or ever.",
                ForeignSurface::PythonPackage,
            )?],
        )?,
        entry(
            11,
            5,
            "Python Benchmark Authoring Sdk",
            vec![foreign_row(
                "title: \"Python Benchmark Authoring SDK\",",
                "The authoring ergonomics of the same Python distribution. The contract underneath \
                 is already enforced in Rust; what this module adds is decorator syntax, which is \
                 a property of a language this workspace does not ship.",
                ForeignSurface::PythonPackage,
            )?],
        )?,
        entry(
            11,
            6,
            "Typescript Sdk",
            vec![foreign_row(
                "title: \"TypeScript SDK\",",
                "A TypeScript client generated from OpenAPI. The workspace emits no OpenAPI \
                 artifact, so there is nothing for the generator to read and nothing for a \
                 generated client to be pinned to.",
                ForeignSurface::TypeScriptPackage,
            )?],
        )?,
        entry(
            11,
            8,
            "Rest Grpc And Event Apis",
            vec![foreign_row(
                "title: \"REST, gRPC and Event APIs\",",
                "A network surface for a server this workspace does not run. The service graph \
                 requires the same services to run in-process, so the transport the module \
                 specifies has no host here.",
                ForeignSurface::NetworkApi,
            )?],
        )?,
        entry(
            11,
            9,
            "Event Stream And Webhooks",
            vec![foreign_row(
                "title: \"Event Stream and Webhooks\",",
                "Delivery semantics for a stream nobody publishes. The event vocabulary exists in \
                 Rust; the subscription, retry and signature machinery is a property of a running \
                 endpoint.",
                ForeignSurface::NetworkApi,
            )?],
        )?,
        entry(
            11,
            10,
            "Mcp Server",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"MCP Server\",",
                "`bioprism-mcp` serves the protocol and advertises the tool definitions, citing a \
                 different module for the same subject. A second server would be a second answer \
                 to what tools the platform exposes.",
                Classification::discharged_by(["bioprism-mcp"])?,
            )?],
        )?,
        entry(
            11,
            15,
            "Evaluator Oracle And Mutation Sdk",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Evaluator, Oracle, and Mutation SDK\",",
                "Typed claims with evidence references, declared preconditions, expected semantic \
                 relations and deterministic-first priority are already enforced by three crates. \
                 What is left over is Python decorator ergonomics.",
                Classification::discharged_by([
                    "bioprism-oracle",
                    "bioprism-mutation",
                    "bioprism-sdk",
                ])?,
            )?],
        )?,
        entry(
            11,
            16,
            "Environment And Pack Authoring Sdk",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Environment and Pack Authoring SDK\",",
                "Pack structure, manifests, fixture provenance and local validation gates already \
                 live in two crates. The remainder is scaffolding subcommands and starting \
                 templates: files a generator writes, not properties a type holds.",
                Classification::discharged_by(["bioprism-packs", "bioprism-factory"])?,
            )?],
        )?,
        entry(
            11,
            17,
            "Authoring Studio",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Benchmark and Decision-Cell Authoring Studio\",",
                "Five interface descriptions — a trace workspace, a cell editor with a draggable \
                 boundary, a minimisation panel, an oracle lab, a release checklist. Every \
                 sentence is an action a person takes at a screen.",
                Classification::Process,
            )?],
        )?,
        entry(
            11,
            18,
            "Authoring Studio And Notebook Workflow",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Authoring Studio and Notebook Workflow\",",
                "An eight-arrow workflow diagram plus reviewer collaboration and notebook \
                 etiquette. Its one real rule — every edit produces a deterministic artifact in a \
                 working tree — is a property of an exporter that exists in no language here.",
                Classification::Process,
            )?],
        )?,
        entry(
            11,
            20,
            "Capability Dashboard And Query",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Capability Dashboard and Query Layer\",",
                "Charts, filters, keyboard navigation and colour-independent status. Its one \
                 clause with teeth constrains the sampler that produced the results, not the page \
                 that draws them.",
                Classification::Process,
            )?],
        )?,
        entry(
            11,
            21,
            "Github Action For Consumer Repositories",
            vec![foreign_row(
                "title: \"GitHub Action for Consumer Repositories\",",
                "A composite action evaluated by a runner in somebody else's repository. Its \
                 inputs, outputs and failure modes are precise; none of them is observable from a \
                 library compiled into its caller.",
                ForeignSurface::GitHubAction,
            )?],
        )?,
        entry(
            11,
            22,
            "Github Action And Ci Integration",
            vec![foreign_row(
                "title: \"GitHub Action and CI Integration\",",
                "The same action seen from the pipeline side. `crates/devplat` measured that this \
                 module and the Python example block are the only two onboarding documents the \
                 section writes out, and both are entirely outside this repository.",
                ForeignSurface::GitHubAction,
            )?],
        )?,
    ])
}

/// §19, three modules, all discharged by the crate that owns the artifact each one exemplifies.
///
/// The asymmetry `crates/devplat` recorded: a section can read two-thirds unimplemented while the
/// platform underneath it is not.
fn reference_examples() -> Result<Vec<Entry>, RegisterError> {
    Ok(vec![
        entry(
            19,
            1,
            "Decision Cell Example",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Reference Decision Cell\",",
                "One YAML document instantiating the Decision Cell IR. Transcribing it into a \
                 third type would create a decision cell that no compiler produces and no oracle \
                 scores.",
                Classification::discharged_by(["bioprism-prism", "bioprism-benchcompiler"])?,
            )?],
        )?,
        entry(
            19,
            15,
            "Evaluation Conditioned Routing Example",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Worked Example - Evaluation-Conditioned Routing\",",
                "Its five guardrails are already invariants of the router's selector. Restating \
                 them over a route type defined in a documentation crate would produce predicates \
                 that guard nothing anybody calls.",
                Classification::discharged_by(["bioprism-routing", "bioprism-baseline"])?,
            )?],
        )?,
        entry(
            19,
            21,
            "Reliable Repair Weave Program",
            vec![transcribed(
                "bioprism-devplat",
                DEVPLAT,
                "title: \"Reference Example - Reliable Repair Weave Program\",",
                "A 110-line program in WeaveLang. It belongs in the compiler's fixture set, where \
                 it is parsed, rather than in a documentation crate, where it would be a string.",
                Classification::discharged_by(["bioprism-weavelang", "bioprism-weave"])?,
            )?],
        )?,
    ])
}

/// §14, nine modules, all process, all from `crates/stewardship`.
///
/// The section that established the test every later crate reused, and the cleanest bucket in the
/// register: nine descriptions of what people do, none of them a predicate over an artifact.
fn governance_and_quality() -> Result<Vec<Entry>, RegisterError> {
    let row = |index: u8, title: &str, needle: &str, reasoning: &str| {
        entry(
            14,
            index,
            title,
            vec![transcribed(
                "bioprism-stewardship",
                STEWARDSHIP,
                needle,
                reasoning,
                Classification::Process,
            )?],
        )
    };

    Ok(vec![
        row(
            1,
            "Project Governance",
            "The modules are named instead: project",
            "Councils, seats and terms. A council type with a vote method would assert that a \
             council met, which is precisely what software cannot know.",
        )?,
        row(
            2,
            "Open Governance And Rfc Process",
            "open governance and the RFC process",
            "RFC stages, comment periods and a supermajority whose threshold the module never \
             states. A stage is a place in a workflow, not a property of an artifact.",
        )?,
        row(
            4,
            "Contributor Model And Code Ownership",
            "contributor model and code ownership;",
            "Maintainer promotion and ownership assignment. The one rule underneath — that authors \
             do not solely certify their own systems — is separation of duties and two crates \
             already hold it.",
        )?,
        row(
            5,
            "Rfc Adr And Technical Decision Process",
            "code ownership; RFC/ADR",
            "Minimum fields, review dates and supersession links. `crates/ops` reached the same \
             verdict on the engineering register in another section and found the same single \
             predicate underneath it, owned elsewhere.",
        )?,
        row(
            18,
            "Conflicts Of Interest And Sponsorship",
            "conflicts of interest and sponsorship",
            "Disclosure and recusal. Whether a disclosure was made is a fact about a person, and \
             the artifact-level residue — that a sponsor may not grade its own submission — is \
             already the reviewer-independence rule.",
        )?,
        row(
            22,
            "Documentation Information Architecture And Review",
            "documentation information architecture and",
            "Where documentation lives and who reviews it. A review cadence is a schedule, and the \
             module supplies no interval that a staleness check could be written against.",
        )?,
        row(
            23,
            "Community Conduct Inclusion And Appeals",
            "community conduct, inclusion and appeals",
            "Conduct reporting, enforcement and appeal. The module asks for proportional \
             enforcement and gives no scale, which is one of nine mechanisms `crates/stewardship` \
             records the section naming and never specifying.",
        )?,
        row(
            24,
            "Sustainability Finance And Public Benefit",
            "sustainability, finance and public benefit",
            "Budget publication and funding structure. Every clause is an obligation of an \
             organisation; none of them is decidable from any object this workspace holds.",
        )?,
        row(
            25,
            "Periodic Program Review",
            "and periodic programme review.",
            "A review cadence with no stated period. `crates/stewardship` records that the section \
             asks for periodic review of access grants without saying how periodic, which is the \
             same gap one level up.",
        )?,
    ])
}

/// §33, ten modules, and the register's largest disagreement.
///
/// `crates/atlasx` read all ten and reports that zero are code-bearing. `bioprism-metrics` reports
/// that it already implements the arithmetic governing all of them. Both are recorded. They are not
/// the same claim: one says nothing is there to build, the other says the buildable part is built
/// somewhere else, and a reader deciding whether to open the section needs to know the workspace
/// holds two answers.
fn capability_metrics() -> Result<Vec<Entry>, RegisterError> {
    let atlasx_headline = "Zero of the ten capability-metrics modules is code-bearing.";
    let metrics_needle = "This crate covers the remaining fifteen modules";

    let row = |index: u8, title: &str, atlasx_needle: &str, reasoning: &str| {
        entry(
            33,
            index,
            title,
            vec![
                transcribed(
                    "bioprism-atlasx",
                    ATLASX,
                    atlasx_needle,
                    reasoning,
                    Classification::Process,
                )?,
                transcribed(
                    "bioprism-metrics",
                    METRICS,
                    metrics_needle,
                    "`bioprism-metrics` states that it covers this module — not by implementing an \
                     estimator the section never defines, but by implementing the stratification, \
                     interval, worst-domain and predeclared-weight discipline that governs every \
                     metric in it. On that reading the substance is discharged rather than absent.",
                    Classification::discharged_by(["bioprism-metrics"])?,
                )?,
            ],
        )
    };

    Ok(vec![
        row(
            3,
            "Evidence Grounding Provenance And Claim Support",
            atlasx_headline,
            "Six measurement-dimension nouns and five metric names above four blocks that are \
             byte-identical across the section. Strip the shared blocks and what is left defines \
             nothing.",
        )?,
        row(
            5,
            "Information Acquisition And Context Value",
            atlasx_headline,
            "Names information gain per token and regret against an oracle acquisition policy, \
             with no posterior and no named policy. A metric name is not a metric.",
        )?,
        row(
            6,
            "Value Of Experiment Assay Selection And Active Discovery",
            atlasx_headline,
            "The same template again. The one place the workspace does compute experiment value \
             refuses a scalar without a caller-supplied exchange rate, which is the gap this \
             module leaves open rather than closes.",
        )?,
        row(
            7,
            "Tissue Sample Time And Resource Efficiency",
            atlasx_headline,
            "Avoidable consumption and resource regret against an optimal policy, where both \
             `avoidable` and `optimal` need a counterfactual policy the section never supplies.",
        )?,
        row(
            8,
            "Temporal Validity And Evidence Firewall Metrics",
            atlasx_headline,
            "Six nouns and five names above the shared blocks. The firewall itself — evidence \
             recorded after a decision leaks even when it describes earlier biology — is a rule \
             two other crates already carry as types.",
        )?,
        row(
            9,
            "Cross Modal Consistency And Contradiction Metrics",
            atlasx_headline,
            "Contradiction as a number, where the workspace's finding is that a contradiction is \
             an object with three resolution states. A rate over that object needs a denominator \
             this module does not give.",
        )?,
        row(
            10,
            "Causal Identification Intervention And Mechanism Metrics",
            atlasx_headline,
            "Identification validity and decision regret with no graph format and no \
             identification criterion, so nothing here can be checked against anything.",
        )?,
        row(
            12,
            "Reproducibility Reexecution And Claim Stability",
            atlasx_headline,
            "Claim-flip rate and reproduction rate as bare noun phrases. The certificate that \
             would carry them exists elsewhere and deliberately refuses to be read as a validity \
             claim.",
        )?,
        row(
            13,
            "Translation Spine And Evidence Maturity Metrics",
            "*Translation Spine and Evidence Maturity Metrics* names",
            "The closest of the ten to code: `weakest-link maturity` implies a minimum over a \
             path. `crates/atlasx` declined it on the merits — the spine is `bioprism-foundation`'s, \
             which already refuses a path with a missing edge and an edge whose evidence is \
             ungraded, and a second spine would be a second spine.",
        )?,
        row(
            14,
            "Multi Agent Coordination And Molecule Value",
            "*Multi-Agent Coordination and Molecule Value* defines",
            "The only module in the section with a stated baseline: team gain is value beyond the \
             best single participant. `crates/atlasx` declined it anyway, because that is an \
             aggregation rule and the workspace has one.",
        )?,
    ])
}

/// §34, ten modules, all interface or process, from `crates/atlashub`.
///
/// Seven of the ten carry a second verdict from the same crate: `crates/atlashub` classified the
/// module as surface *and* named the crate holding the one object underneath it. That is the
/// block-level shape one module at a time, and it is why the entries are compound rather than
/// contested.
fn public_hub() -> Result<Vec<Entry>, RegisterError> {
    let surface = |needle: &str, reasoning: &str| {
        transcribed(
            "bioprism-atlashub",
            ATLASHUB,
            needle,
            reasoning,
            Classification::Process,
        )
    };
    let beneath = |needle: &str, reasoning: &str, crates: &[&str]| {
        transcribed(
            "bioprism-atlashub",
            ATLASHUB,
            needle,
            reasoning,
            Classification::discharged_by(crates.iter().copied())?,
        )
    };

    Ok(vec![
        entry(
            34,
            1,
            "Users Personas And Jobs To Be Done",
            vec![surface(
                "*Users, Personas and Jobs to be Done*",
                "Seven user intentions and a set of adoption rates. Nothing in it is a predicate \
                 over an artifact, which is the test the section was read with.",
            )?],
        )?,
        entry(
            34,
            2,
            "Information Architecture And Navigation",
            vec![
                surface(
                    "*Information Architecture and Navigation*",
                    "A list of twelve object names. Rich only because the capability list is \
                     twelve proper nouns, which is a vocabulary rather than a specification.",
                )?,
                beneath(
                    "The one checkable residue",
                    "Its one checkable residue — that a card's outbound links all resolve — is \
                     enforced as a construction error on the world card, under the neighbouring \
                     module rather than claimed separately.",
                    &["bioprism-atlashub"],
                )?,
            ],
        )?,
        entry(
            34,
            4,
            "Worldline Timeline And State Explorer",
            vec![
                surface(
                    "*Worldline Timeline and State Explorer*",
                    "A visibility-cutoff slider and layered tracks. What is left after the \
                     non-visual half is removed is a viewport.",
                )?,
                beneath(
                    "belongs to `bioprism-world` and to `bioprism-lens`'s leakage module",
                    "Valid time versus record time, and the future-evidence firewall, are already \
                     owned: one crate holds the temporal axes as unconvertible newtypes and \
                     another holds the leakage check.",
                    &["bioprism-world", "bioprism-lens"],
                )?,
            ],
        )?,
        entry(
            34,
            5,
            "Biodecision Cell Inference Microscope",
            vec![
                surface(
                    "*BioDecision Cell Inference Microscope*",
                    "The module is the act of looking at an object that already exists and already \
                     carries the state, the candidate actions and the acceptance criteria.",
                )?,
                beneath(
                    "The object it inspects is `bioprism_prism::DecisionCell`",
                    "The decision cell is a frozen state two architectures can resume from \
                     identically, and it is already a type with a human-approval gate as its only \
                     constructor.",
                    &["bioprism-prism"],
                )?,
            ],
        )?,
        entry(
            34,
            6,
            "Fork Compare And Counterfactual Lab",
            vec![
                surface(
                    "*Fork Compare and Counterfactual Lab*",
                    "Factor pickers and interaction plots over a mechanism that already exists. \
                     The surface is presentation; the paired execution underneath it is not.",
                )?,
                beneath(
                    "`bioprism_prism::matched_fork` is the mechanism",
                    "Matched forking, including seed control and paired execution, is already \
                     implemented in the crate that owns decision cells.",
                    &["bioprism-prism"],
                )?,
            ],
        )?,
        entry(
            34,
            7,
            "Oracle Evidence And Disagreement Explorer",
            vec![
                surface(
                    "*Oracle Evidence and Disagreement Explorer*",
                    "An appeal queue with a turnaround metric is a workflow, and the queue is what \
                     is left once the oracle machinery is attributed to the crate that holds it.",
                )?,
                beneath(
                    "adjudication and circularity are `bioprism-oracle`'s",
                    "Oracle planes, reader distributions, adjudication and circularity are already \
                     implemented, together with the rule that an oracle which cannot decide must \
                     abstain rather than answer.",
                    &["bioprism-oracle"],
                )?,
            ],
        )?,
        entry(
            34,
            11,
            "Architecture And Agent Molecule Registry",
            vec![
                surface(
                    "*Architecture and Agent Molecule Registry*",
                    "`crates/atlashub` calls this the closest near-miss of the twelve it declined. \
                     The declarative immutable composition it describes already exists as a type.",
                )?,
                beneath(
                    "is `bioprism-sdk`'s rule that a plugin without conformance evidence",
                    "The one predicate the module adds is substitution safety, and that is already \
                     the rule that a plugin without conformance evidence is not selectable for \
                     load-bearing work. Implementing it here would be a second capability \
                     declaration.",
                    &["bioprism-sdk"],
                )?,
            ],
        )?,
        entry(
            34,
            19,
            "Notebook Ide Mcp And Agent Integrations",
            vec![
                surface(
                    "*Notebook, IDE, MCP and Agent Integrations*",
                    "Seven integration targets, which is a list of destinations rather than a \
                     property any of them must hold.",
                )?,
                beneath(
                    "`bioprism-mcp` owns the MCP resource shape and `bioprism-sdk` the client contract",
                    "Round-trip fidelity is a property of those objects' serialisation and is \
                     tested where they live; a third crate asserting it would be asserting \
                     somebody else's test.",
                    &["bioprism-mcp", "bioprism-sdk"],
                )?,
            ],
        )?,
        entry(
            34,
            21,
            "No Key Demos And Onboarding",
            vec![surface(
                "*No-Key Demos and Onboarding*",
                "An MVP acceptance test written as a first-run experience. `crates/atlashub` calls \
                 it genuinely valuable and genuinely not a type, and both halves of that are the \
                 verdict.",
            )?],
        )?,
        entry(
            34,
            22,
            "Open Source Community And Star Flywheel",
            vec![surface(
                "*Open Source Community and Star Flywheel*",
                "Scaffold commands, working groups, bounties and a stars-to-active-use ratio. A \
                 programme, and measured by adoption rather than by any artifact.",
            )?],
        )?,
    ])
}

/// §40, seven modules, from `crates/ops`.
///
/// The section labelled build-ready, and the one whose residue splits three ways. Four entries are
/// compound because `crates/ops` classified the module *and* named where its single checkable
/// sentence already lives.
fn engineering_contracts() -> Result<Vec<Entry>, RegisterError> {
    let ops = |needle: &str, reasoning: &str, classification: Classification| {
        transcribed("bioprism-ops", OPS, needle, reasoning, classification)
    };

    Ok(vec![
        entry(
            40,
            1,
            "Technology Baseline",
            vec![ops(
                "Reference Technology Baseline | process",
                "A table of choices with reasons — Python, FastAPI, Typer, Pydantic, DuckDB, \
                 OpenTelemetry. There is no predicate in it, and `crates/ops` calls it the sharpest \
                 single finding about the section: a build-ready technology module describing an \
                 implementation nobody built.",
                Classification::Process,
            )?],
        )?,
        entry(
            40,
            2,
            "Monorepo And Package Layout",
            vec![
                ops(
                    "Monorepo and Package Layout | process",
                    "A directory tree for a Python workspace, against a Rust Cargo workspace. The \
                     layout is a description of a repository that does not exist in this shape.",
                    Classification::Process,
                )?,
                ops(
                    "it here would produce a second layering register.",
                    "Its one genuinely checkable sentence — packages may depend only downstream \
                     according to the graph — is the domain-boundary rule in a second voice, and \
                     `bioprism-services` has already audited this workspace against it.",
                    Classification::discharged_by(["bioprism-services"])?,
                )?,
            ],
        )?,
        entry(
            40,
            15,
            "Typescript Sdk Contract",
            vec![ops(
                "TypeScript SDK Contract | code, **in another language",
                "Genuinely code-bearing, and the code is TypeScript. Its first invariant pins \
                 generated code to an API schema hash, and this workspace emits no such artifact, \
                 so there is nothing to pin to. A Rust type named after a browser client would be \
                 fiction.",
                foreign(ForeignSurface::TypeScriptPackage),
            )?],
        )?,
        entry(
            40,
            40,
            "Ci Cd And Release Automation",
            vec![
                ops(
                    "CI/CD and Release Automation | code, **in a workflow file",
                    "Code that lives in a workflow file, and a workflow runner is not a library.",
                    foreign(ForeignSurface::CiWorkflow),
                )?,
                ops(
                    "Its four invariants decompose into work three crates already do",
                    "Schema and migration checks before publication, quality gates, and release \
                     gating with signing are each already owned — where signing comes out as a \
                     single not-checked variant, because no key material exists.",
                    Classification::discharged_by([
                        "bioprism-governance",
                        "bioprism-infra",
                        "bioprism-safety",
                    ])?,
                )?,
            ],
        )?,
        entry(
            40,
            41,
            "First 100 Implementation Tickets",
            vec![ops(
                "First 100 Implementation Tickets | process",
                "A ticket table, and the least machine-readable document in the section: its \
                 primary-contract column mixes dotted ids from two sections with bare directory \
                 names, so no tool can resolve the mapping a ticket-to-contract table exists for.",
                Classification::Process,
            )?],
        )?,
        entry(
            40,
            43,
            "Engineering Adr Register",
            vec![
                ops(
                    "Engineering ADR Register and Decision Process | process",
                    "`crates/ops` calls this the closest call of the seven, because its trigger \
                     list reads like a predicate over a change. Everything else is workflow — \
                     minimum fields, review dates, supersession links — and the module defers its \
                     own authority to another corpus.",
                    Classification::Process,
                )?,
                ops(
                    "that crate refuses an author's contrary claim",
                    "Its one predicate is already held: a change that moves an artifact's digest \
                     is breaking, and the crate holding that rule refuses an author's contrary \
                     claim rather than asking for a document. A type here would assert that an ADR \
                     was written.",
                    Classification::discharged_by(["bioprism-governance"])?,
                )?,
            ],
        )?,
        entry(
            40,
            45,
            "Ownership Raci And Maintainer Boundaries",
            vec![
                ops(
                    "Ownership, RACI and Maintainer Boundaries | process",
                    "A table of who reviews what. Responsibility assignment is a fact about an \
                     organisation and decidable from no artifact this workspace holds.",
                    Classification::Process,
                )?,
                ops(
                    "That is separation of duties, and",
                    "Its one rule that is not a table — no single maintainer may both author \
                     hidden evaluator state and approve release claims for the same benchmark \
                     family without independent review — is separation of duties, and two crates \
                     already hold it.",
                    Classification::discharged_by(["bioprism-stewardship", "bioprism-registry"])?,
                )?,
            ],
        )?,
    ])
}

/// §35, six modules, and the only section here decided entirely by crates that never name it.
///
/// Five of the six are read across from a neighbouring section whose modules carry the same
/// content under different ids. Every one of those is marked as this register's inference, because
/// `crates/worldfactory` tabulated the neighbouring section and not this one.
fn million_scale() -> Result<Vec<Entry>, RegisterError> {
    let mined = "it turns real model, pipeline and agent executions into decision units";

    Ok(vec![
        entry(
            35,
            2,
            "Observed Data World Authoring",
            vec![inferred(
                "bioprism-worldfactory",
                WORLDFACTORY,
                "title: \"Observed Worlds from Real Data and Workflows\",",
                "`crates/worldfactory` implements observed worlds from real data under a \
                 neighbouring section's id and records in its own coverage table that the two \
                 sections restate each other. It does not name this module, so the correspondence \
                 is this register's reading of that table rather than its stated verdict.",
                Classification::discharged_by(["bioprism-worldfactory"])?,
            )?],
        )?,
        entry(
            35,
            3,
            "Semi Synthetic World Construction",
            vec![inferred(
                "bioprism-worldfactory",
                WORLDFACTORY,
                "title: \"Semi-Synthetic Biological Worlds\",",
                "Semi-synthetic construction is implemented, together with the provenance ladder \
                 that refuses a claim naming a quantity the construction itself fixed. Recorded \
                 under a neighbouring section's id, which is why the coverage script still reads \
                 this one as untouched.",
                Classification::discharged_by(["bioprism-worldfactory"])?,
            )?],
        )?,
        entry(
            35,
            4,
            "Mechanistic Simulation And Assay Twin Factory",
            vec![inferred(
                "bioprism-worldfactory",
                WORLDFACTORY,
                "title: \"Mechanistic and Simulated BioWorlds\",",
                "Mechanistic and simulated worlds are implemented, and the rule that matters — a \
                 benchmark built from a simulation cannot make a claim about biology the \
                 simulation assumed — is a type there. The assay-twin half is `crates/oraclex`'s \
                 disjoint calibration and test sets.",
                Classification::discharged_by(["bioprism-worldfactory", "bioprism-oraclex"])?,
            )?],
        )?,
        entry(
            35,
            6,
            "Trajectory Capture And Research Workflow Mining",
            vec![inferred(
                "bioprism-worldfactory",
                WORLDFACTORY,
                mined,
                "`crates/worldfactory` marks the neighbouring section's trajectory-mining module \
                 unclaimed, with the reason: it turns real model, pipeline and agent executions \
                 into decision units, and there are no real executions in this workspace to mine. \
                 That reason applies unchanged here, so the module is uncovered for a stated cause \
                 rather than for want of attention.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "there are no real model, pipeline or agent executions in this workspace \
                         to mine, so the input the module consumes does not exist yet",
                    )?,
                },
            )?],
        )?,
        entry(
            35,
            7,
            "Biodecision Compiler And Boundary Detection",
            vec![inferred(
                "bioprism-worldfactory",
                WORLDFACTORY,
                mined,
                "The compilation half of the same unclaimed module. `crates/worldfactory` \
                 separately records that the neighbouring section specifies no unit boundary for \
                 the compilation, so even with trajectories in hand there is no stated rule for \
                 where one decision unit ends.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "no trajectories exist to compile, and the specification supplies no unit \
                         boundary for deciding where one decision unit ends",
                    )?,
                },
            )?],
        )?,
        entry(
            35,
            13,
            "Distributed Execution Scheduling And Fault Tolerance",
            vec![
                inferred(
                    "bioprism-scale",
                    SCALE,
                    "The scheduling half of 35 — jobs, workers, leases, idempotency-aware recovery",
                    "`bioprism-scale` states that the scheduling half of this section belongs to \
                     `bioprism-factory` and is deliberately not duplicated. That crate implements \
                     the lease and recovery lifecycle, including the rule that lease expiry does \
                     not imply safe retry for non-idempotent effects.",
                    Classification::discharged_by(["bioprism-factory"])?,
                )?,
                inferred(
                    "bioprism-factory",
                    FACTORY,
                    "no distributed lease fencing",
                    "The crate the work was handed to disagrees about how much of it landed: it \
                     records no backpressure model, no fair-share scheduling across tenants and no \
                     distributed lease fencing, and says the section's million-scale concerns \
                     beyond enqueue and recovery are absent rather than stubbed. The distributed \
                     half of this module is therefore still work.",
                    Classification::GenuinelyUncovered {
                        standing: UncoveredStanding::real_work_not_done(
                            "the lease and recovery lifecycle exists in-memory and single-process; \
                             distributed fencing, backpressure and cross-tenant fair share are \
                             recorded as absent",
                        )?,
                    },
                )?,
            ],
        )?,
    ])
}

/// §23, nine modules, and nobody has read them.
///
/// `crates/fabric` implements eleven of the section's modules and classifies four more as prose,
/// naming them by id. These nine are in neither list. `crates/interweave` is being written against
/// this section right now, so the expected next state of these entries is deletion.
fn agent_interweave() -> Result<Vec<Entry>, RegisterError> {
    let surveyed = [
        "bioprism-fabric",
        "bioprism-weave",
        "bioprism-weavelang",
        "bioprism-choreography",
    ];

    let row = |index: u8, title: &str, reasoning: &str| {
        entry(23, index, title, vec![unread(reasoning, &surveyed)?])
    };

    Ok(vec![
        row(
            24,
            "Protocol Adapters A2A Mcp Otel And Cloudevents",
            "No crate records a position. The nearest neighbours are the layered protocol stack in \
             `crates/fabric` and the same refusal `crates/sweep` and `bioprism-trace` record about \
             an OpenTelemetry adapter in another section, but neither is a judgement on this \
             module.",
        )?,
        row(
            25,
            "Component Runtime Wasm Wit And Sandbox Composition",
            "No crate records a position. `bioprism-safety` states that the workspace has no \
             WebAssembly, no sandbox and no capability drop, which would block the runtime half; \
             nobody has said whether the composition half is code.",
        )?,
        row(
            27,
            "Interweave Evaluation And Microbenchmark Generation",
            "No crate records a position. The section's microbenchmark generation would sit \
             between the fabric's composition algebra and the evaluation crates, and neither side \
             has claimed it.",
        )?,
        row(
            28,
            "Orchestration Learning And Credit Assignment",
            "No crate records a position. Credit assignment over a composition is the one §23 \
             subject with no counterpart anywhere in the workspace, which makes it the likeliest \
             of these nine to be real work rather than a duplicate.",
        )?,
        row(
            29,
            "Security Threat Model And Trust Boundaries",
            "No crate records a position on the fabric's own threat model. `bioprism-safety` owns \
             the platform threat model under a different section and states that a library of \
             plain Rust types may model a control and may not claim one.",
        )?,
        row(
            31,
            "Governance Versioning And Conformance",
            "No crate records a position. `bioprism-governance` owns schema evolution and \
             `crates/fabric` owns the molecule version bump, but nobody has said whether this \
             module is those two seen from the fabric side or a third thing.",
        )?,
        row(
            33,
            "Reference Interweave Workflows",
            "No crate records a position. The reference programs of this section are the same kind \
             of artifact `crates/devplat` classified as discharged by the compiler's fixture set \
             in another section, and nobody has checked whether that reading transfers.",
        )?,
        row(
            39,
            "Weavebench Packs And Microbenchmark Taxonomy",
            "No crate records a position. A benchmark taxonomy for the fabric would be a pack, and \
             the pack crates cite neither this module nor this section.",
        )?,
        row(
            47,
            "Human And Organizational Participants",
            "No crate records a position, and this is the one of the nine most likely to be \
             process: `crates/stewardship`'s test would ask whether human participation is a \
             predicate over an artifact or a description of what people do. Nobody has applied it.",
        )?,
    ])
}

/// §12, seven modules: two with a stated blocker, five with nobody.
///
/// `crates/infra` and `bioprism-ledger` between them hold the section's caching, invalidation,
/// quality, tiering, lifecycle, quota, index, backup and event-model modules. What is left is the
/// deployment and topology half, and the workspace has no infrastructure at all.
fn data_and_infrastructure() -> Result<Vec<Entry>, RegisterError> {
    let surveyed = [
        "bioprism-infra",
        "bioprism-ledger",
        "bioprism-store",
        "bioprism-services",
    ];

    Ok(vec![
        entry(
            12,
            2,
            "Storage Architecture",
            vec![inferred(
                "bioprism-infra",
                INFRA,
                "There is no storage backend",
                "`crates/infra` states plainly that everything it holds is in-memory and \
                 single-process, that its lifecycle manages records about objects and never their \
                 bytes, and that there is no storage backend. It does not classify this module; it \
                 states the condition that stops anybody implementing it here.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "the workspace has no storage backend, no database and no filesystem \
                         access, so a storage architecture has nothing to be an architecture of",
                    )?,
                },
            )?],
        )?,
        entry(
            12,
            3,
            "Relational Catalog Schema",
            vec![unread(
                "No crate records a position. A relational schema presupposes the database \
                 `crates/infra` says does not exist, but nobody has decided whether the module is \
                 therefore a foreign artifact — DDL in another language — or genuinely absent work.",
                &surveyed,
            )?],
        )?,
        entry(
            12,
            12,
            "Observability And Slos",
            vec![unread(
                "No crate records a position. `crates/ops` implements the observability contract \
                 of another section — signals, redaction policy, export batches — and never claims \
                 this module, so whether the two are the same content under two ids is unchecked.",
                &surveyed,
            )?],
        )?,
        entry(
            12,
            13,
            "Compute Provider And Kubernetes",
            vec![inferred(
                "bioprism-ops",
                OPS,
                "**No infrastructure.** No deployment, no provider binding",
                "`crates/ops` records that the workspace has no deployment, no provider binding, \
                 no health check, no container, no image, no cluster and no CI runner. That is a \
                 statement about the workspace rather than about this module, and it is why this \
                 module cannot be implemented here.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "there is no deployment, provider binding, container, image or cluster \
                         anywhere in the workspace for a provider abstraction to abstract over",
                    )?,
                },
            )?],
        )?,
        entry(
            12,
            14,
            "Distributed Compute And Placement",
            vec![unread(
                "No crate records a position. `bioprism-factory` records that it has no \
                 distributed lease fencing and `crates/infra` that it has no concurrency, which \
                 together say the capability is absent — but neither says what this module is.",
                &surveyed,
            )?],
        )?,
        entry(
            12,
            15,
            "Local First Deployment",
            vec![inferred(
                "bioprism-ops",
                OPS,
                "are statements about a running system, and a type",
                "`crates/ops` classified the deployment module of another section and found that \
                 three of its four invariants — local mode needs no external service, protected \
                 mode denies egress, hosted metadata does not imply artifact access — are \
                 statements about a running system, and that a type asserting any of them would be \
                 claiming a control. The same holds for this module's local half.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "the invariants are properties of a running deployment, and a library of \
                         plain types can model such a control but must never claim one",
                    )?,
                },
            )?],
        )?,
        entry(
            12,
            16,
            "Cloud And Federated Deployment",
            vec![unread(
                "No crate records a position. `bioprism-hubapi` owns federation and the rule that \
                 trust does not transit, and `crates/atlashub` owns federated evaluation, but the \
                 deployment half — what runs where — is claimed by nobody.",
                &surveyed,
            )?],
        )?,
    ])
}

/// §36, seven modules: one with a stated blocker, six with nobody.
///
/// `bioprism-policy` holds the section's classification, consent, federation and retention
/// modules and `bioprism-safety` the adversarial half of its neighbour. The seven left are the
/// ethics, biosecurity and quality-management half, and `crates/bioethics` is being written against
/// them now.
fn biology_governance() -> Result<Vec<Entry>, RegisterError> {
    let surveyed = [
        "bioprism-policy",
        "bioprism-safety",
        "bioprism-stewardship",
        "bioprism-foundation",
    ];

    Ok(vec![
        entry(
            36,
            7,
            "Sandboxing Untrusted Code And Research Artifacts",
            vec![inferred(
                "bioprism-safety",
                SAFETY,
                "seccomp, no capability drop, no user namespace",
                "`bioprism-safety` states that it has no process spawning, no container, no \
                 microVM, no WebAssembly, no seccomp, no capability drop and no user namespace, and \
                 that the neighbouring section's sandbox modules are absent in full. It does not \
                 classify this module; it records that the control it asks for does not exist.",
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::real_work_not_done(
                        "no isolation mechanism of any kind exists in the workspace, and a type \
                         asserting containment would be claiming a control rather than modelling \
                         one",
                    )?,
                },
            )?],
        )?,
        entry(
            36,
            10,
            "Physical Experiment And Wetlab Action Boundaries",
            vec![unread(
                "No crate records a position. `bioprism-foundation` enumerates executing wet \
                 laboratory protocols and autonomously ordering assays among the things the \
                 platform does not do, under another section's id, and `bioprism-onco` carries the \
                 typed research boundary — but neither says this module is thereby discharged.",
                &surveyed,
            )?],
        )?,
        entry(
            36,
            11,
            "Dual Use Biosecurity And Capability Release",
            vec![unread(
                "No crate records a position. `bioprism-safety` implements dual-use release gates \
                 under the neighbouring security section and `bioprism-packs` carries a dual-use \
                 portfolio axis, so this may be a duplicate; nobody has checked.",
                &surveyed,
            )?],
        )?,
        entry(
            36,
            13,
            "Fairness Representation And Global Resource Context",
            vec![unread(
                "No crate records a position. `crates/stewardship` classified the neighbouring \
                 governance section's benchmark-ethics-and-fairness module as process, which is \
                 evidence about a sibling module rather than about this one.",
                &surveyed,
            )?],
        )?,
        entry(
            36,
            19,
            "Security Privacy Safety Red Team Program",
            vec![unread(
                "No crate records a position. `bioprism-safety` implements security testing and \
                 red team under the neighbouring section, and separately records that it has no \
                 fuzzer, no scanner and no detector — so whether this module is a duplicate or the \
                 programme around one is undecided.",
                &surveyed,
            )?],
        )?,
        entry(
            36,
            21,
            "Quality Management Validation And Release Gates",
            vec![unread(
                "No crate records a position. Release-gate arithmetic with a three-valued outcome \
                 exists in `bioprism-metrics` and quality gates in `crates/infra`; the \
                 quality-management system around them is the part `crates/oraclex` called \
                 programme when it met the same content in another section.",
                &surveyed,
            )?],
        )?,
        entry(
            36,
            22,
            "Research Ethics Irb And Human Subject Boundaries",
            vec![unread(
                "No crate records a position. `crates/stewardship` owns the medical and \
                 neuroscience boundary and `bioprism-foundation` the research-use-only wrapper, \
                 both under other sections' ids; the ethics-review half is claimed by nobody.",
                &surveyed,
            )?],
        )?,
    ])
}

/// One module each from §04, §10 and §26 — the tails of sections that are otherwise complete.
fn small_remainders() -> Result<Vec<Entry>, RegisterError> {
    Ok(vec![
        entry(
            4,
            2,
            "Opentelemetry Adapter",
            vec![
                transcribed(
                    "bioprism-sweep",
                    SWEEP,
                    "**The OpenTelemetry adapter** is an integration surface",
                    "An integration surface: a mapping from OTLP and Jaeger exports and the GenAI \
                     semantic conventions onto the event IR. The workspace is offline against \
                     pinned dependencies and carries no OTel crate, and hand-rolling a second \
                     reading of another organisation's evolving conventions would put its \
                     disagreements into the evidence record as properties of the run.",
                    foreign(ForeignSurface::ForeignSpecification),
                )?,
                transcribed(
                    "bioprism-trace",
                    TRACE,
                    "No OpenTelemetry adapter — the MVP cut line asks for one",
                    "`bioprism-trace` records the same refusal independently and for the same \
                     reason: the adapter needs a dependency the offline build cannot take. Two \
                     crates reaching one verdict is worth recording, because it is the only module \
                     in the register where that happened without one crate citing the other.",
                    foreign(ForeignSurface::ForeignSpecification),
                )?,
            ],
        )?,
        entry(
            10,
            1,
            "Registry Overview",
            vec![transcribed(
                "bioprism-sweep",
                SWEEP,
                "**The registry overview** is an architecture statement whose every predicate is discharged",
                "Immutable resolution, signed manifests, digest and revocation verification before \
                 materialisation, and the local-first contract are already implemented across \
                 three crates; leaderboard rows pointing at result bundles rather than at a \
                 database are a fourth. Implementing the overview would mean building a fourth \
                 registry.",
                Classification::discharged_by([
                    "bioprism-registry",
                    "bioprism-hubapi",
                    "bioprism-hub",
                ])?,
            )?],
        )?,
        entry(
            26,
            19,
            "Biocapability Atlas",
            vec![
                transcribed(
                    "bioprism-bioevalx",
                    BIOEVALX,
                    "**BioCapability Atlas and Posterior Profiles** (§26, module 19)",
                    "Every clause of its protocol already exists: the capability-by-evidence \
                     object, the coverage report and the holes list in one crate; clustering, \
                     intervals, worst-domain reporting and the rule that an aggregate over a grid \
                     with an unmeasured cell is not an aggregate over the grid in another; the \
                     capability posterior and its coverage floors in a third. `crates/bioevalx` \
                     says a second atlas with its own opinion about what a hole is would be worse \
                     than an uncovered id.",
                    Classification::discharged_by([
                        "bioprism-atlas",
                        "bioprism-metrics",
                        "bioprism-evalengine",
                    ])?,
                )?,
                transcribed(
                    "bioprism-bioevalx",
                    BIOEVALX,
                    "So the honest classification is per *block*, not per",
                    "The same crate's section-wide finding applies to this module too: every one \
                     of the section's files ends with four blocks describing what a study author \
                     must do, above a purpose, an evaluation target and a numbered protocol that \
                     are checkable. The discharge above covers the protocol; the four trailing \
                     blocks are process and stay process.",
                    Classification::block_level_split(
                        ["bioprism-metrics", "bioprism-evalengine"],
                        "the four trailing blocks — diagnostic outputs, required baselines, \
                         statistical analysis and release gates — describe what a study author \
                         does and are byte-identical across all twenty-four of the section's \
                         modules",
                    )?,
                )?,
            ],
        )?,
    ])
}
