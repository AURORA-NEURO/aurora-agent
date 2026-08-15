//! The fifty-five, transcribed.
//!
//! Every module `docs/BACKLOG.md` lists, with the verdict a classifying crate reached about it and
//! the file that verdict was written in. Nothing here is a new judgement about a module a crate
//! already judged: where `crates/stewardship` said process, this says process, in its words, with a
//! fragment of its sentence that must still be present for the entry to stand.
//!
//! # Where the judgements came from
//!
//! Eight crates classified their section's remainder explicitly, and their tables are transcribed
//! row for row: `crates/devplat` (§11 and §19), `crates/ops` (§40), `crates/stewardship` (§14),
//! `crates/atlashub` (§34), `crates/atlasx` (§33), `crates/sweep` (§04 and §10), `crates/bioevalx`
//! (§26) and `crates/bioethics` (§36). `bioprism-metrics` and `bioprism-trace` supply a second
//! reading each. Where two crates decide one module differently — `crates/atlasx` and
//! `bioprism-metrics` on the whole of §33 — both readings are recorded and neither is adjudicated.
//!
//! **Every verdict here but one is now transcribed rather than inferred**, and that is a fact about
//! the current backlog rather than about the design. The register once carried thirty verdicts this
//! crate had read across from a neighbouring section or drawn from a crate's not-implemented list,
//! because §12, §23, §35 and most of §36 had no crate that had read them. Four crates then did, and
//! those modules left the backlog. The one survivor is recorded below.
//!
//! # How to regenerate this file
//!
//! This has now been done once, when the backlog went from eighty-four modules to fifty-seven, and
//! the procedure held: three section functions were deleted whole, one was rewritten, and no entry
//! outside those sections was touched.
//!
//! 1. Run `tools/backlog.sh` and diff `docs/BACKLOG.md` against
//!    [`reconcile::reconcile`](crate::reconcile::reconcile). Trust the diff over any arithmetic
//!    done by hand, and do not try to find the entries with a regular expression — the register is
//!    Rust, its entries are nested constructor calls, and a section's modules are not textually
//!    adjacent to its section number.
//! 2. For a module that has **left** the backlog, delete its entry, or call
//!    [`Register::without`](crate::Register::without) if the caller is a script. Nothing else
//!    moves: entries hold no cross-references to each other. When a whole section goes, delete its
//!    function and its `entries.extend` line together.
//! 3. For a module that has **arrived**, read the crate that owns its section, find the sentence
//!    that classified it, and add an entry whose anchor is a fragment of that sentence. If there is
//!    no such sentence, the verdict is
//!    [`GenuinelyUncovered`](crate::Classification::GenuinelyUncovered) and the survey is the list
//!    of crates that were read while looking. It is not a placeholder and it must not be treated as
//!    one. **A survey that found nothing is still a finding, and it is recorded with the crates
//!    that were read.**
//! 4. When a crate *newly* classifies a module the register already explains, replace the source
//!    rather than the entry: a verdict this crate inferred should become a transcription the moment
//!    somebody writes the sentence down. That is what happened to both §36 entries.
//! 5. Never write a dotted id. [`crate::citations`] fails the build if one appears.
//!
//! One consequence of step 2 worth knowing before it surprises somebody: the last entry built by a
//! given helper takes the helper with it. The private `unread` constructor that built a
//! nobody-has-read verdict was deleted in this pass, because leaving an unused function behind is a
//! warning and this crate finishes at zero. Re-adding it is four lines and
//! [`UncoveredStanding::nobody_has_read`](crate::UncoveredStanding::nobody_has_read) is still public
//! and still tested.

use crate::entry::{Entry, Register};
use crate::error::RegisterError;
use crate::module::ModuleKey;
use crate::verdict::{Classification, ForeignSurface, Source, UncoveredStanding, Verdict};

const STEWARDSHIP: &str = "crates/stewardship/src/lib.rs";
const ATLASHUB: &str = "crates/atlashub/src/lib.rs";
const ATLASX: &str = "crates/atlasx/src/lib.rs";
const METRICS: &str = "crates/metrics/src/lib.rs";
const DEVPLAT: &str = "crates/devplat/src/classify.rs";
const OPS: &str = "crates/ops/src/lib.rs";
const SWEEP: &str = "crates/sweep/src/lib.rs";
const BIOEVALX: &str = "crates/bioevalx/src/lib.rs";
const BIOETHICS: &str = "crates/bioethics/src/lib.rs";

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

/// §36, two modules, both declined by `crates/bioethics` on one shared ground.
///
/// This section is the register's worked example of the regeneration procedure. Five of its seven
/// modules left the backlog when `crates/bioethics` implemented them, and the two that stayed
/// stopped being *unread* and became *classified* in the same commit: their entries used to be this
/// register's own reading of `bioprism-safety`'s not-implemented list, and are now transcribed from
/// the crate that read the modules and took a position.
///
/// `crates/bioethics` calls the shared ground *perimeter infrastructure a sibling already
/// positioned*. Both modules ask for controls at a boundary — a process boundary, a network stack, a
/// scanner, an independent team — and in both cases `bioprism-safety` already states the
/// workspace's position under a section-13 id.
///
/// The sandboxing module carries a second verdict and the red-team module does not, and the
/// difference is real rather than editorial. What is left of the red-team module after the discharge
/// is a clause the blueprint never defines, so nobody could build it from the specification. What is
/// left of the sandboxing module is a control that would work if somebody built it, and nobody has.
fn biology_governance() -> Result<Vec<Entry>, RegisterError> {
    let bioethics = |needle: &str, reasoning: &str, classification: Classification| {
        transcribed(
            "bioprism-bioethics",
            BIOETHICS,
            needle,
            reasoning,
            classification,
        )
    };

    Ok(vec![
        entry(
            36,
            7,
            "Sandboxing Untrusted Code And Research Artifacts",
            vec![
                bioethics(
                    "Implementing it here would produce a second threat model",
                    "`crates/bioethics` read the module and declined it: `bioprism-safety` states \
                     the workspace's position on every one of its controls under four section-13 \
                     ids, and `bioprism-sdk` holds the isolation-request ladder with its single \
                     declared-only variant. Implementing it again would produce a second threat \
                     model with its own opinion about what isolation means.",
                    Classification::discharged_by(["bioprism-safety", "bioprism-sdk"])?,
                )?,
                inferred(
                    "bioprism-bioethics",
                    BIOETHICS,
                    "All thirteen need a process boundary, a network stack or a scanner.",
                    "`crates/bioethics` classified this module as positioned by a sibling and did \
                     not classify it as work remaining. This register reads its own sentence — that \
                     all thirteen required controls need a process boundary, a network stack or a \
                     scanner — as saying the control exists nowhere, and records that separately so \
                     a reader does not come away thinking a sandbox exists. The same crate reports \
                     six enforced safeguards and thirty-six declared, and states that not one of \
                     the six defends a perimeter.",
                    Classification::GenuinelyUncovered {
                        standing: UncoveredStanding::real_work_not_done(
                            "isolation, egress control and artifact scanning need a process \
                             boundary, a network stack or a scanner, and this workspace has none \
                             of the three; every safeguard covering them is declared rather than \
                             enforced",
                        )?,
                    },
                )?,
            ],
        )?,
        entry(
            36,
            19,
            "Security Privacy Safety Red Team Program",
            vec![bioethics(
                "`disclosure` module owns the red-team corpus and the",
                "`crates/bioethics` read the module and declined it: `bioprism-safety` owns the \
                 red-team corpus and the epoch-driven vulnerability ladder under two section-13 \
                 ids, and its integrity module owns the witnesses. The one clause that looks \
                 checkable — canary assets — is a gap the neighbouring section already records, \
                 because nothing in the blueprint says what makes a canary detectable and a canary \
                 type with an invented detectability rule would be fiction with a struct around it.",
                Classification::discharged_by(["bioprism-safety"])?,
            )?],
        )?,
    ])
}

/// One module each from §04, §10 and §26 — the tails of sections that are otherwise complete.
fn small_remainders() -> Result<Vec<Entry>, RegisterError> {
    Ok(vec![
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
