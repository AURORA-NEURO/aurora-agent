//! The linter run against this repository's own documentation (41.11, 41.16).
//!
//! 41.16 wants acceptance tests over "a clean extraction". This file is the closest thing to
//! that available at test time: it walks the working tree, builds a registry from the real bytes,
//! and lints it. The assertions are deliberately structural — that the scan resolves, that every
//! node hashes, that ids round-trip — because asserting on the *content* of the repository's docs
//! would make this test fail whenever someone fixes a sentence, and a test like that gets deleted
//! rather than fixed.
//!
//! The findings themselves are printed. Run with `-- --nocapture` to read them. What the linter
//! says about our own `docs/` is a result, not a bug in the linter.

use bioprism_docgraph::lint::lint;
use bioprism_docgraph::registry::NodeStatus;
use bioprism_docgraph::scan::{scan_markdown_tree, ScanOptions};
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("crates/docgraph sits two levels below the workspace root")
        .to_path_buf()
}

fn scan_options() -> ScanOptions {
    ScanOptions {
        skip_dirs: ["target", "node_modules", ".git", ".jj", ".claude"]
            .into_iter()
            .map(String::from)
            .collect(),
        default_status: NodeStatus::Guide,
        ..ScanOptions::default()
    }
    .with_status("docs/", NodeStatus::Implementation)
    .with_status("docs/COVERAGE.md", NodeStatus::Generated)
    .with_status("docs/BASELINE_COMPARISON.md", NodeStatus::Generated)
    .with_status("docs/DISCRIMINATING_COMPARISON.md", NodeStatus::Generated)
    .with_status(".agents/skills/", NodeStatus::Tool)
}

#[test]
fn this_repositorys_own_documentation_scans_into_a_lintable_graph() {
    let root = repository_root();
    if !root.join("AGENTS.md").exists() {
        eprintln!("workspace root not present; skipping repository scan");
        return;
    }
    let report = scan_markdown_tree(&root, &scan_options()).expect("the working tree scans");
    assert!(
        report.files_read >= 8,
        "expected the repository's Markdown corpus, read {}",
        report.files_read
    );
    assert_eq!(report.graph.node_count(), report.files_read);
    for node in report.graph.nodes() {
        assert!(
            node.hash.is_some(),
            "{} was read from bytes and must be hashed",
            node.id
        );
        assert_eq!(node.id.as_str(), node.path);
    }

    let lint_report = lint(&report.graph, &[]);
    println!("--- docgraph over this repository ---");
    println!("files read: {}", report.files_read);
    println!("edges: {}", report.graph.edges().len());
    println!("unresolved links: {}", report.unresolved_links.len());
    for (module, target) in &report.unresolved_links {
        println!("  {module} -> {target}");
    }
    println!(
        "links out of the documentation corpus (real files, not nodes): {}",
        report.out_of_corpus_links.len()
    );
    for (path, reason) in &report.unreadable_front_matter {
        println!("  malformed front matter: {path}: {reason}");
    }
    println!("modules:");
    for node in report.graph.nodes() {
        println!("  {} [{}]", node.id, node.status.as_str());
    }
    println!("lint findings by code:");
    for (code, count) in lint_report.counts() {
        println!("  {code}: {count}");
    }
    for finding in &lint_report.findings {
        println!("  {:?} {finding:?}", finding.severity());
    }
}

#[test]
fn a_scan_of_the_repository_is_byte_stable_across_runs() {
    let root = repository_root();
    if !root.join("AGENTS.md").exists() {
        return;
    }
    let options = scan_options();
    let first = scan_markdown_tree(&root, &options).expect("scan");
    let second = scan_markdown_tree(&root, &options).expect("scan");
    assert_eq!(first.graph, second.graph);
    assert_eq!(first.unresolved_links, second.unresolved_links);
}
