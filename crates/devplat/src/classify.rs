//! The headline: what the remaining developer-platform and reference-example modules actually are.
//!
//! Twenty modules were left after `bioprism-sdk` took plugin registration and capability
//! declaration, `bioprism-devx` took diagnostics and the retryability taxonomy, `bioprism-cookbook`
//! took the verifiable recipes and `bioprism-examples` took the vertical slices. Applying one test
//! to each — *is the detailed design a set of predicates over an artifact, or a description of what
//! people do?* — splits them four ways, and only four of the twenty are things this crate could
//! honestly implement.
//!
//! | verdict | count |
//! |---|---|
//! | [`Verdict::Process`] — the design describes what a person does at an interface | 3 |
//! | [`Verdict::ForeignArtifact`] — code-bearing, but not Rust and not in this repository | 3 |
//! | [`Verdict::CoveredElsewhere`] — an existing crate already owns the substance | 10 |
//! | [`Verdict::ImplementedHere`] — predicates over an artifact this crate defines | 4 |
//!
//! # The citation rule, in the type system
//!
//! Only [`Verdict::ImplementedHere`] carries a `module_id`. The other three variants have a
//! `title` and no id, so a contributor cannot record "this is process" and simultaneously write
//! the token that `tools/coverage.sh` counts as coverage. The workspace's coverage script matches
//! any `NN.MM` token anywhere under `crates/`, which means citing a module you did not implement
//! moves the number without moving the platform — the one thing this workspace must not do.
//! [`crate::citations`] turns that from a convention into a test over this crate's own source.
//!
//! # On the third bucket
//!
//! [`Verdict::CoveredElsewhere`] is not a synonym for "someone else's problem". It is a claim with
//! a named referent: the crate that holds the substance, and the sentence saying what would be
//! duplicated if this crate implemented it anyway. Six of the twenty land here, which is the
//! reason a section can read as two-thirds unimplemented while the platform underneath it is not.

use serde::{Deserialize, Serialize};

use crate::surface::{foreign_subjects, Surface};

/// What one blueprint module turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    /// The detailed design describes what a person does, not what an artifact guarantees.
    Process {
        /// The strongest sentence in the module, and why it is not a predicate.
        because: &'static str,
    },
    /// Code-bearing, in a language and a repository that are not this one.
    ForeignArtifact {
        surface: Surface,
        because: &'static str,
    },
    /// Another crate of this workspace already owns the substance under a different module id.
    CoveredElsewhere {
        /// Workspace package names, most specific first.
        crates: Vec<&'static str>,
        because: &'static str,
    },
    /// Predicates over an artifact this crate defines. The only variant that carries an id.
    ImplementedHere {
        /// The blueprint module id, which this crate is entitled to cite.
        module_id: &'static str,
        /// The module of this crate that holds it.
        rust_module: &'static str,
    },
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Process { .. } => "process",
            Verdict::ForeignArtifact { .. } => "foreign artifact",
            Verdict::CoveredElsewhere { .. } => "covered elsewhere",
            Verdict::ImplementedHere { .. } => "implemented here",
        }
    }

    /// The id, when there is one. `None` for every verdict but the last, structurally.
    pub fn module_id(&self) -> Option<&'static str> {
        match self {
            Verdict::ImplementedHere { module_id, .. } => Some(module_id),
            _ => None,
        }
    }

    /// Whether this crate is entitled to name the module by id.
    pub fn is_citable_here(&self) -> bool {
        self.module_id().is_some()
    }
}

/// One classified module. Identified by title, because most of them may not be identified by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleVerdict {
    /// The blueprint section this module belongs to, as a number, so a reader can group them
    /// without a token that the coverage tool would read as a citation.
    pub section: u8,
    /// The module title, as the blueprint spells it.
    pub title: &'static str,
    pub verdict: Verdict,
}

impl ModuleVerdict {
    /// A one-line row for a report.
    pub fn describe(&self) -> String {
        match &self.verdict {
            Verdict::ImplementedHere {
                module_id,
                rust_module,
            } => format!("{} [{module_id}] -> {rust_module}", self.title),
            other => format!("{} -> {}", self.title, other.as_str()),
        }
    }
}

const DEVELOPER_PLATFORM: u8 = 11;
const REFERENCE_EXAMPLES: u8 = 19;

/// The twenty modules, classified.
///
/// The order is section then blueprint order. The three [`Verdict::ForeignArtifact`] rows take
/// their surfaces from [`foreign_subjects`], so the census and the classification cannot drift
/// apart — a test asserts the two agree.
pub fn classification() -> Vec<ModuleVerdict> {
    let foreign = foreign_subjects();
    let foreign_row = |title: &'static str| -> Verdict {
        let subject = foreign
            .iter()
            .find(|subject| subject.title == title)
            .expect("every foreign row has a census entry");
        Verdict::ForeignArtifact {
            surface: subject.surface.clone(),
            because: subject.why_not_here,
        }
    };

    vec![
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Python SDK",
            verdict: foreign_row("Python SDK"),
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Python Benchmark Authoring SDK",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-packs", "bioprism-benchcompiler", "bioprism-mutation"],
                because: "the repository now ships a dependency-free Python authoring layer that builds digest-bound PackIr, DecisionCell, and deterministic MutationPlan documents, validates their cross-field invariants, and forwards final health and mutation checks to the authoritative Rust tools. The Python ergonomics are now in-tree while the Rust crates remain the owners of scientific semantics.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "TypeScript SDK",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-api"],
                because: "the repository now ships a dependency-free TypeScript Fetch client over the bounded API gateway, with strict request/response limits, structured errors, SSE cursor parsing, webhook lifecycle helpers, and typed facades for the cross-domain workflows. It remains an integration SDK rather than a generated clone of every Rust domain type.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "REST, gRPC and Event APIs",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-api"],
                because: "the workspace now owns the bounded REST, JSON-RPC, cursor event, and signed webhook outbox boundary. The gRPC half remains an explicit deployment gap rather than being inferred from an HTTP route.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Event Stream and Webhooks",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-api"],
                because: "the API gateway owns event cursors, retention-gap reporting, signed outbox envelopes, bounded retry, and idempotent acknowledgement; an external delivery worker remains a deployment concern.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "MCP Server",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-mcp"],
                because: "`bioprism-mcp` serves the protocol, advertises the tool definitions and \
                          states which blueprint module it implements. This module is the same \
                          subject at lower detail; a second server would be a second answer to \
                          `what tools does the platform expose`.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Evaluator, Oracle, and Mutation SDK",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-oracle", "bioprism-mutation", "bioprism-sdk"],
                because: "the repository now ships dependency-free Python authoring and \
                          oracle/evaluation request contracts over the typed claims, evidence \
                          references, declared preconditions and deterministic-first priority \
                          those crates enforce. Rust still owns combination and scientific \
                          semantics; the remaining gap is the full multi-distribution SDK \
                          ergonomics described by the blueprint.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Environment and Pack Authoring SDK",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-packs", "bioprism-factory"],
                because: "pack structure, manifests, fixture provenance and local validation gates \
                          live in those crates. The remainder is four scaffolding subcommands and \
                          six starting templates, which are files a generator writes rather than \
                          properties a type holds.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Benchmark and Decision-Cell Authoring Studio",
            verdict: Verdict::Process {
                because: "the detailed design is five interface descriptions — a trace workspace, \
                          a cell editor with a draggable boundary, a minimisation panel, an oracle \
                          lab, a release checklist. Every sentence is an action a person takes at \
                          a screen. The one checkable rule in the vicinity, that the studio is \
                          never the source of truth, belongs to the notebook-workflow module.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Authoring Studio and Notebook Workflow",
            verdict: Verdict::Process {
                because: "an eight-arrow workflow diagram plus reviewer collaboration and notebook \
                          etiquette. Its file-backed rule — every edit produces a deterministic \
                          artifact in a working tree — is real, and it is a property of the \
                          exporter, which does not exist in any language here.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Capability Dashboard and Query Layer",
            verdict: Verdict::Process {
                because: "charts, filters, keyboard navigation and colour-independent status. The \
                          one clause with teeth, that adaptive results must display inclusion \
                          probabilities and stopping rule, is a requirement on the sampler that \
                          produced them, not on the page that draws them.",
            },
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "GitHub Action for Consumer Repositories",
            verdict: foreign_row("GitHub Action for Consumer Repositories"),
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "GitHub Action and CI Integration",
            verdict: foreign_row("GitHub Action and CI Integration"),
        },
        ModuleVerdict {
            section: DEVELOPER_PLATFORM,
            title: "Reporting and Export Formats",
            verdict: Verdict::ImplementedHere {
                module_id: "11.23",
                rust_module: "bioprism_devplat::report",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Reference Decision Cell",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-prism", "bioprism-benchcompiler"],
                because: "the module is one YAML document instantiating the Decision Cell IR. \
                          `bioprism-prism` owns that IR and `bioprism-benchcompiler` mines and \
                          minimises cells; transcribing the example into a third type would \
                          create a decision cell that no compiler produces and no oracle scores.",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Reference Case - Scientific Figure Reproduction and Claim Trace",
            verdict: Verdict::ImplementedHere {
                module_id: "19.12",
                rust_module: "bioprism_devplat::repro",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Worked Example - Evaluation-Conditioned Routing",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-routing", "bioprism-baseline"],
                because: "`bioprism-routing` is the evaluation-conditioned router, and its five \
                          guardrails — approved immutable architectures only, no benchmark-id \
                          shortcut, fallback under uncertainty, validation before learning, \
                          rollback on a poor prospective result — are invariants of that crate's \
                          selector. Restating them over a `Route` type defined here would produce \
                          predicates that guard nothing anybody calls.",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Reference Case - Evaluator Exploit and Security Cell",
            verdict: Verdict::ImplementedHere {
                module_id: "19.17",
                rust_module: "bioprism_devplat::exploit",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Reference Example - Reliable Repair Weave Program",
            verdict: Verdict::CoveredElsewhere {
                crates: vec!["bioprism-weavelang", "bioprism-weave"],
                because: "the module is a 110-line program in WeaveLang. `bioprism-weavelang` is \
                          the lexer, parser, type system and lowering for exactly that language, \
                          and `bioprism-weave` is the kernel its events run on. A reference \
                          program belongs in the compiler's fixture set, where it is parsed, not \
                          in a documentation crate, where it would be a string.",
            },
        },
        ModuleVerdict {
            section: REFERENCE_EXAMPLES,
            title: "Reference Example - Scientific Reproduction Capability Molecule",
            verdict: Verdict::ImplementedHere {
                module_id: "19.22",
                rust_module: "bioprism_devplat::repro",
            },
        },
    ]
}

/// The module ids this crate implemented, and is therefore entitled to write down.
///
/// Four. This is the declared set that [`crate::citations::audit`] checks the source against.
pub fn implemented_module_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = classification()
        .iter()
        .filter_map(|row| row.verdict.module_id())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Modules deliberately not implemented, by title, with the reason.
///
/// Sixteen rows. Returned as data rather than left in a doc comment so that a caller assembling a
/// status report cannot summarise the section as "in progress".
pub fn not_implemented() -> Vec<(&'static str, &'static str, &'static str)> {
    classification()
        .into_iter()
        .filter(|row| !row.verdict.is_citable_here())
        .filter_map(|row| match row.verdict {
            Verdict::Process { because } => Some((row.title, "process", because)),
            Verdict::ForeignArtifact { because, .. } => {
                Some((row.title, "foreign artifact", because))
            }
            Verdict::CoveredElsewhere { because, .. } => {
                Some((row.title, "covered elsewhere", because))
            }
            Verdict::ImplementedHere { .. } => None,
        })
        .collect()
}

/// How many modules fell into each bucket: process, foreign, covered elsewhere, implemented.
pub fn verdict_counts() -> [usize; 4] {
    let mut counts = [0usize; 4];
    for row in classification() {
        let index = match row.verdict {
            Verdict::Process { .. } => 0,
            Verdict::ForeignArtifact { .. } => 1,
            Verdict::CoveredElsewhere { .. } => 2,
            Verdict::ImplementedHere { .. } => 3,
        };
        counts[index] += 1;
    }
    counts
}
