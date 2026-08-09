//! A fixture graph transcribed from this repository's own documentation (41.16).
//!
//! 41.16 asks for "executable checks proving that task routing and token-bounded retrieval work
//! on a clean extraction", plus "broken-edge fixtures". This module supplies both, and it does it
//! from the corpus this crate is meant to serve: `AGENTS.md`, `CLAUDE.md`, `docs/`, and
//! `.agents/skills/`. A harness that cannot navigate its own documentation is not making its case.
//!
//! # It is a transcription of structure, not of bytes
//!
//! The nodes carry abridged stand-in text. Their *structure* — which files exist, which H1 each
//! has, which file links to which — is transcribed from the repository as it stands, so the
//! findings this fixture produces are findings about the real corpus. The text is a stand-in so
//! the fixture stays hermetic: a test that read the real files would fail when someone edits a
//! sentence, and would then be deleted rather than fixed. [`crate::scan`] is the path that reads
//! the real bytes.
//!
//! # What it reports about us
//!
//! Three findings here are findings about this repository, reproduced from a
//! [`crate::scan`] run over the working tree: `CLAUDE.md` has no H1;
//! `docs/BASELINE_COMPARISON.md` and `docs/DISCRIMINATING_COMPARISON.md` share one; and
//! `docs/ARCHITECTURE.md` is an orphan, because neither `README.md` nor `AGENTS.md` links to
//! anything in `docs/`. The absent edges are absent here too — a fixture that quietly added the
//! link the corpus ought to have would be a fixture that proves nothing.
//!
//! # The contradiction in the fixture is real
//!
//! Blueprint sections 41 and 42 were written when the graph *was* the inference substrate, and
//! 43.01 states that it is not — graphs are "generated projections". Both texts are in the
//! distribution. That is a genuine `contradicts` pair, and it is resolved: 41.00 carries the v0.6
//! clarification. The fixture records it that way, with `resolved_by` set, which is what stops
//! [`crate::lint`] from reporting it and what forces any bundle touching either side to carry all
//! three modules.

use crate::registry::{
    ContextCard, DocGraph, ModuleId, ModuleNode, NodeStatus, ProtectedClass,
};
use crate::route::{RouteId, TaskRoute};
use crate::tokens::ProfileLevel;
use crate::vocabulary::{DocEdge, DocEdgeType};

fn id(value: &str) -> ModuleId {
    ModuleId::parse(value).expect("fixture ids are well formed")
}

fn node(path: &str, title: &str, status: NodeStatus, decision: &str) -> ModuleNode {
    ModuleNode::new(id(path), path, title, status)
        .with_profile(ProfileLevel::Brief)
        .with_card(ContextCard {
            decision: decision.to_string(),
            ..ContextCard::default()
        })
        .with_brief(format!("{title}. {decision}"))
}

/// The repository's documentation as a graph.
///
/// Every node and every edge here corresponds to something in the working tree. Nothing was added
/// to make the linter look good, and nothing was removed to make it look bad.
pub fn repository_doc_graph() -> DocGraph {
    let mut graph = DocGraph::new();

    let agents = node(
        "AGENTS.md",
        "AURORA Agent",
        NodeStatus::Guide,
        "Context assembly is a compiler pass, not a retrieval heuristic; honest labelling of what \
         was omitted is the product.",
    )
    .with_card(ContextCard {
        decision: "Context assembly is a compiler pass; honest omission labelling is the product."
            .to_string(),
        protected_invariants: vec![
            "Zero influence is not unknown influence.".to_string(),
            "Unmeasured is not zero.".to_string(),
        ],
        ..ContextCard::default()
    })
    .with_brief(
        "AURORA Agent is a context harness for biological research agents. A typed decision query \
         compiles into the smallest decision-sufficient evidence region, delivered as a Decision \
         Section with a Context Certificate stating what was omitted.",
    )
    .with_protected(ProtectedClass::ClaimKind)
    .with_protected(ProtectedClass::UncertaintyAndQuality);

    graph.insert_node(agents).expect("fresh graph");

    graph
        .insert_node(
            ModuleNode::new(id("CLAUDE.md"), "CLAUDE.md", "", NodeStatus::Guide)
                .with_brief("Working agreements for agents operating in this workspace."),
        )
        .expect("fresh graph");

    graph
        .insert_node(node(
            "README.md",
            "bioprism",
            NodeStatus::Guide,
            "Repository front door; links to every crate's source but to no document in docs/.",
        ))
        .expect("fresh graph");

    for (path, title, status, decision) in [
        (
            "docs/ARCHITECTURE.md",
            "Architecture",
            NodeStatus::Implementation,
            "How the pieces fit, and why the boundaries fall where they do.",
        ),
        (
            "docs/ADR-001-language-strategy.md",
            "ADR-001 — Language strategy",
            NodeStatus::Specification,
            "Rust is the normative implementation; other languages must reach bit-parity.",
        ),
        (
            "docs/FINDINGS.md",
            "Findings",
            NodeStatus::Implementation,
            "Measured results from context comparison, including the ones that do not favour FIBER.",
        ),
        (
            "docs/COVERAGE.md",
            "Blueprint coverage",
            NodeStatus::Generated,
            "Which of the blueprint's 973 content modules the workspace actually cites.",
        ),
        (
            "docs/BASELINE_COMPARISON.md",
            "Equal-engineering context comparison",
            NodeStatus::Generated,
            "Baseline world comparison, 761 facts.",
        ),
        (
            "docs/DISCRIMINATING_COMPARISON.md",
            "Equal-engineering context comparison",
            NodeStatus::Generated,
            "Discriminating world comparison, 762 facts.",
        ),
        (
            ".agents/skills/add-module/SKILL.md",
            "Add a module",
            NodeStatus::Tool,
            "How to add a blueprint module to a crate.",
        ),
        (
            ".agents/skills/check-parity/SKILL.md",
            "Check cross-language parity",
            NodeStatus::Tool,
            "How to verify bit-parity between implementations.",
        ),
        (
            ".agents/skills/verify-crate/SKILL.md",
            "Verify a crate",
            NodeStatus::Tool,
            "How to test and lint one crate without touching the workspace.",
        ),
    ] {
        graph
            .insert_node(node(path, title, status, decision))
            .expect("fixture paths are unique");
    }

    for (path, title, decision) in [
        (
            "blueprint/41.00",
            "Graph-First Knowledge and Navigation — index",
            "v0.6 clarification: graphs in this section are compiled projections, not the substrate.",
        ),
        (
            "blueprint/41.01",
            "Documentation Graph Model",
            "Represent the blueprint as addressable modules and typed dependency edges.",
        ),
        (
            "blueprint/41.03",
            "Documentation Edge Vocabulary",
            "Use explicit edge semantics; unknown edge types fail validation.",
        ),
        (
            "blueprint/41.09",
            "Documentation Context Bundle Compiler",
            "Materialize a route-specific bundle with source boundaries and a budget report.",
        ),
        (
            "blueprint/43.01",
            "Query-Compiled Fibered Factor Calculus",
            "The calculus is the substrate; graphs and timelines are generated projections.",
        ),
    ] {
        graph
            .insert_node(node(path, title, NodeStatus::Specification, decision))
            .expect("fixture paths are unique");
    }

    graph
        .insert_node(
            node(
                "crate/bioprism-docgraph",
                "bioprism-docgraph",
                NodeStatus::Implementation,
                "Documentation graph, bundle compiler and change-impact analysis over 41.x.",
            )
            .with_hashed_body(
                "# bioprism-docgraph\n\n## What it implements\n\nBlueprint 41.01 through 41.16, \
                 minus the projection modules `bioprism-graph` already discharges.\n",
            ),
        )
        .expect("fixture paths are unique");

    graph
        .insert_node(node(
            "crate/bioprism-graph",
            "bioprism-graph",
            NodeStatus::Implementation,
            "Generated projections of compiled decision regions, under the constraint of 43.01.",
        ))
        .expect("fixture paths are unique");

    for (from, kind, to) in [
        ("docs/ARCHITECTURE.md", DocEdgeType::References, "docs/ADR-001-language-strategy.md"),
        ("docs/ARCHITECTURE.md", DocEdgeType::References, "docs/FINDINGS.md"),
        ("docs/FINDINGS.md", DocEdgeType::References, "docs/BASELINE_COMPARISON.md"),
        ("docs/FINDINGS.md", DocEdgeType::References, "docs/DISCRIMINATING_COMPARISON.md"),
        ("docs/FINDINGS.md", DocEdgeType::References, "crates/worldgen"),
        ("docs/FINDINGS.md", DocEdgeType::References, "crates/baseline/tests/equal_engineering.rs"),
        ("docs/FINDINGS.md", DocEdgeType::References, "crates/worldgen/tests/structural_families.rs"),
        ("blueprint/41.01", DocEdgeType::PartOf, "blueprint/41.00"),
        ("blueprint/41.03", DocEdgeType::PartOf, "blueprint/41.00"),
        ("blueprint/41.09", DocEdgeType::PartOf, "blueprint/41.00"),
        ("blueprint/41.00", DocEdgeType::Contains, "blueprint/41.01"),
        ("blueprint/41.00", DocEdgeType::Contains, "blueprint/41.03"),
        ("blueprint/41.00", DocEdgeType::Contains, "blueprint/41.09"),
        ("blueprint/41.09", DocEdgeType::Requires, "blueprint/41.03"),
        ("blueprint/41.01", DocEdgeType::Governs, "blueprint/43.01"),
        ("blueprint/41.09", DocEdgeType::Governs, "blueprint/43.01"),
        ("crate/bioprism-docgraph", DocEdgeType::Implements, "blueprint/41.09"),
        ("crate/bioprism-docgraph", DocEdgeType::Implements, "blueprint/41.03"),
        ("crate/bioprism-graph", DocEdgeType::Implements, "blueprint/41.01"),
        ("crate/bioprism-docgraph", DocEdgeType::Requires, "docs/ADR-001-language-strategy.md"),
        ("crate/bioprism-docgraph", DocEdgeType::Governs, "AGENTS.md"),
        ("crate/bioprism-graph", DocEdgeType::Governs, "AGENTS.md"),
        ("AGENTS.md", DocEdgeType::PartOf, "README.md"),
        ("CLAUDE.md", DocEdgeType::PartOf, "README.md"),
        ("README.md", DocEdgeType::Contains, "AGENTS.md"),
        ("README.md", DocEdgeType::Contains, "CLAUDE.md"),
        (".agents/skills/verify-crate/SKILL.md", DocEdgeType::Evaluates, "crate/bioprism-docgraph"),
        ("docs/COVERAGE.md", DocEdgeType::Provides, "blueprint/41.00"),
        ("docs/BASELINE_COMPARISON.md", DocEdgeType::ExampleOf, "docs/FINDINGS.md"),
        ("docs/DISCRIMINATING_COMPARISON.md", DocEdgeType::ExampleOf, "docs/FINDINGS.md"),
    ] {
        graph.insert_edge(DocEdge::new(id(from), id(to), kind));
    }

    graph.insert_edge(
        DocEdge::new(
            id("blueprint/41.01"),
            id("blueprint/43.01"),
            DocEdgeType::Contradicts,
        )
        .resolved_by(id("blueprint/41.00")),
    );

    graph
}

/// Routes over [`repository_doc_graph`], phrased as tasks an agent in this repository actually has.
pub fn repository_routes() -> Vec<TaskRoute> {
    vec![
        TaskRoute::new(
            RouteId::parse("add-a-crate").expect("route id"),
            "Add a new crate implementing a blueprint section",
        )
        .must_read(id("docs/ARCHITECTURE.md"))
        .must_read(id(".agents/skills/add-module/SKILL.md"))
        .optional(id("docs/COVERAGE.md"))
        .expecting("a crate that compiles, tests green, zero clippy warnings"),
        TaskRoute::new(
            RouteId::parse("report-a-measurement").expect("route id"),
            "Report a measured result without overclaiming it",
        )
        .must_read(id("docs/FINDINGS.md"))
        .non_omittable(id("AGENTS.md"))
        .optional(id("docs/BASELINE_COMPARISON.md"))
        .optional(id("docs/DISCRIMINATING_COMPARISON.md"))
        .expecting("a claim labelled measured or estimated, never both"),
        TaskRoute::new(
            RouteId::parse("understand-the-projection-constraint").expect("route id"),
            "Understand why graphs are projections rather than the substrate",
        )
        .must_read(id("blueprint/41.01"))
        .expecting("a statement of the 43.01 constraint and where it binds"),
    ]
}

/// The deliberate broken fixture 41.03 and 41.16 ask for.
///
/// Every defect here is one [`crate::lint`] has a finding for, and each is a mistake that has
/// actually been made in a documentation corpus: a link to a deleted file, a requirement cycle, a
/// disagreement nobody resolved, and a withdrawn contract with no forwarding address.
pub fn broken_graph() -> DocGraph {
    let mut graph = DocGraph::new();
    for (path, title, status) in [
        ("a.md", "A", NodeStatus::Specification),
        ("b.md", "B", NodeStatus::Specification),
        ("c.md", "C", NodeStatus::Specification),
        ("old.md", "Old", NodeStatus::Withdrawn),
    ] {
        graph
            .insert_node(node(path, title, status, "stand-in"))
            .expect("unique");
    }
    graph.insert_edge(DocEdge::new(id("a.md"), id("b.md"), DocEdgeType::Requires));
    graph.insert_edge(DocEdge::new(id("b.md"), id("c.md"), DocEdgeType::Requires));
    graph.insert_edge(DocEdge::new(id("c.md"), id("a.md"), DocEdgeType::Requires));
    graph.insert_edge(DocEdge::new(
        id("a.md"),
        id("gone.md"),
        DocEdgeType::References,
    ));
    graph.insert_edge(DocEdge::new(
        id("b.md"),
        id("c.md"),
        DocEdgeType::Contradicts,
    ));
    graph
}
