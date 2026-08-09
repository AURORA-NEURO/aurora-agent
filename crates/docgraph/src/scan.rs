//! A deliberate filesystem walk (41.02, 41.11).
//!
//! Everything else in this crate is pure: a [`DocGraph`](crate::registry::DocGraph) is data and
//! the compiler, linter and impact analysis never touch a disk. This module is the exception, and
//! it exists because 41.02 asks for "one machine-readable row for every Markdown contract, root
//! guide, tool, and generated registry" *of a repository*, and 41.11 asks for "graph-to-file
//! consistency" — neither of which can be checked by a crate that has never seen the files.
//!
//! It is opt-in: nothing in the rest of the crate calls it, so a consumer that wants a pure
//! in-memory graph never links a filesystem walk into their reasoning.
//!
//! # What the walk produces
//!
//! One [`ModuleNode`] per Markdown file, hashed from the exact bytes read, plus two kinds of edge:
//!
//! - `part_of` / `contains` between a file and its directory's index file, when the directory has
//!   one. No synthetic directory nodes are created — 41.01 requires that "every node resolves to
//!   a file and H1", and a node standing for a folder resolves to neither.
//! - `references` for every inline link that points at a local file.
//!
//! A link to a Markdown file that was not scanned still becomes a `references` edge, pointing at
//! a module id the registry does not hold. That is intentional: it turns a broken link into a
//! [`LintFinding::DanglingEdge`](crate::lint::LintFinding::DanglingEdge), which is how 41.11's
//! "release has no broken links" gets checked without a second link-checking pass with its own
//! notion of correctness.
//!
//! # A link to source code is a link, not a broken edge
//!
//! Documentation in this workspace links to `crates/fiber/src/qir.rs` and to crate directories.
//! Those targets are not documentation nodes and never will be, so emitting `references` edges to
//! them would report a documentation graph with dozens of dangling edges and no defects — a
//! linter that cries wolf gets switched off. Non-Markdown local targets are checked against the
//! filesystem instead, where the answer is a fact rather than an inference: they land in
//! [`ScanReport::out_of_corpus_links`] when they exist and in [`ScanReport::unresolved_links`]
//! when they do not. The first run of this scanner over this repository reported 28 broken links,
//! every one of them a working link to a source file; that count is the reason this distinction
//! exists.
//!
//! # Not implemented
//!
//! No symlink following (a cycle in the filesystem would become a cycle in the walk), no glob
//! language, no `.gitignore` awareness, and no content-type sniffing — extension only. Skipped
//! directory names are an explicit list, not a heuristic.

use crate::error::DocGraphError;
use crate::markdown::{first_h1, first_paragraph, link_targets, parse_document};
use crate::registry::{ContextCard, DocGraph, ModuleId, ModuleNode, NodeStatus};
use crate::tokens::ProfileLevel;
use crate::vocabulary::{DocEdge, DocEdgeType};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("scanning `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Graph(#[from] DocGraphError),
}

/// File names treated as a directory's index.
pub const INDEX_NAMES: [&str; 4] = ["README.md", "index.md", "00_INDEX.md", "00_SECTION_INDEX.md"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    pub extensions: Vec<String>,
    pub skip_dirs: BTreeSet<String>,
    pub default_status: NodeStatus,
    /// Path-prefix rules, longest prefix wins. Explicit rather than inferred, so the status a
    /// node ends up with can be traced to a line the caller wrote.
    pub status_by_prefix: Vec<(String, NodeStatus)>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            extensions: vec!["md".to_string()],
            skip_dirs: ["target", "node_modules", ".git", ".jj"]
                .into_iter()
                .map(String::from)
                .collect(),
            default_status: NodeStatus::Guide,
            status_by_prefix: Vec::new(),
        }
    }
}

impl ScanOptions {
    pub fn with_status(mut self, prefix: impl Into<String>, status: NodeStatus) -> Self {
        self.status_by_prefix.push((prefix.into(), status));
        self
    }

    fn status_for(&self, relative: &str) -> NodeStatus {
        self.status_by_prefix
            .iter()
            .filter(|(prefix, _)| relative.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, status)| *status)
            .unwrap_or(self.default_status)
    }
}

#[derive(Debug)]
pub struct ScanReport {
    pub graph: DocGraph,
    pub files_read: usize,
    /// Link targets that resolve to nothing: a Markdown file outside the scan, or a local path
    /// that is not on disk. These are the broken links 41.11 forbids in a release.
    pub unresolved_links: Vec<(ModuleId, String)>,
    /// Local targets that exist but are not documentation nodes — source files, directories.
    /// Real links, deliberately not edges.
    pub out_of_corpus_links: Vec<(ModuleId, String)>,
    /// Files whose front matter was malformed. Recorded, not fatal: one broken header should not
    /// prevent the rest of the corpus from being linted.
    pub unreadable_front_matter: Vec<(String, String)>,
}

/// Walk `root`, building a registry and the edges the files imply.
///
/// Module ids are repository-relative paths with forward slashes, which makes them stable across
/// platforms and directly citable by an agent — 41.12 requires agents to "cite paths/headings",
/// and an id that *is* the path removes a lookup from that obligation.
pub fn scan_markdown_tree(root: &Path, options: &ScanOptions) -> Result<ScanReport, ScanError> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_files(root, options, &mut files)?;
    files.sort();

    let mut graph = DocGraph::new();
    let mut relative_paths: BTreeSet<String> = BTreeSet::new();
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    let mut unreadable = Vec::new();

    for path in &files {
        let relative = relative_path(root, path);
        let source = std::fs::read_to_string(path).map_err(|error| ScanError::Io {
            path: relative.clone(),
            source: error,
        })?;
        let parsed = match parse_document(&relative, &source) {
            Ok(parsed) => parsed,
            Err(error) => {
                unreadable.push((relative.clone(), error.to_string()));
                continue;
            }
        };
        let id = ModuleId::parse(relative.clone())?;
        let title = first_h1(parsed.body).unwrap_or_default();
        let brief = first_paragraph(parsed.body).unwrap_or_default();
        let front = parsed.front_matter.as_ref();
        let profile = front
            .and_then(|matter| matter.get("token_profile"))
            .and_then(ProfileLevel::parse)
            .unwrap_or(ProfileLevel::Brief);
        let cluster = front
            .and_then(|matter| matter.get("graph_cluster"))
            .unwrap_or_default()
            .to_string();

        let node = ModuleNode::new(
            id.clone(),
            relative.clone(),
            title,
            options.status_for(&relative),
        )
        .with_cluster(cluster)
        .with_profile(profile)
        .with_card(ContextCard {
            decision: truncate_sentence(&brief),
            ..ContextCard::default()
        })
        .with_brief(brief)
        .with_hashed_body(&source);

        graph.insert_node(node)?;
        relative_paths.insert(relative.clone());
        bodies.insert(relative, parsed.body.to_string());
    }

    let mut unresolved_links = Vec::new();
    let mut out_of_corpus_links = Vec::new();
    for (relative, body) in &bodies {
        let id = ModuleId::parse(relative.clone())?;
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for target in link_targets(body) {
            let Some(resolved) = resolve_link(relative, &target) else {
                continue;
            };
            if !seen.insert(resolved.clone()) {
                continue;
            }
            let is_markdown = options
                .extensions
                .iter()
                .any(|extension| resolved.ends_with(&format!(".{extension}")));
            if !is_markdown {
                if root.join(&resolved).exists() {
                    out_of_corpus_links.push((id.clone(), target.clone()));
                } else {
                    unresolved_links.push((id.clone(), target.clone()));
                }
                continue;
            }
            if !relative_paths.contains(&resolved) {
                unresolved_links.push((id.clone(), target.clone()));
            }
            if let Ok(target_id) = ModuleId::parse(resolved) {
                if target_id != id {
                    graph.insert_edge(DocEdge::new(
                        id.clone(),
                        target_id,
                        DocEdgeType::References,
                    ));
                }
            }
        }

        if let Some(index) = index_for(relative, &relative_paths) {
            let index_id = ModuleId::parse(index)?;
            graph.insert_edge(DocEdge::new(
                id.clone(),
                index_id.clone(),
                DocEdgeType::PartOf,
            ));
            graph.insert_edge(DocEdge::new(index_id, id, DocEdgeType::Contains));
        }
    }

    Ok(ScanReport {
        graph,
        files_read: bodies.len(),
        unresolved_links,
        out_of_corpus_links,
        unreadable_front_matter: unreadable,
    })
}

fn collect_files(
    directory: &Path,
    options: &ScanOptions,
    out: &mut Vec<PathBuf>,
) -> Result<(), ScanError> {
    let entries = std::fs::read_dir(directory).map_err(|error| ScanError::Io {
        path: directory.display().to_string(),
        source: error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| ScanError::Io {
            path: directory.display().to_string(),
            source: error,
        })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| ScanError::Io {
            path: path.display().to_string(),
            source: error,
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if options.skip_dirs.contains(&name) {
                continue;
            }
            collect_files(&path, options, out)?;
        } else if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            if options.extensions.iter().any(|allowed| allowed == extension) {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Resolve a link target against the linking file, or `None` when it is not a local file link.
fn resolve_link(from: &str, target: &str) -> Option<String> {
    if target.starts_with('#') {
        return None;
    }
    let lowered = target.to_ascii_lowercase();
    for scheme in ["http://", "https://", "mailto:", "ftp://", "data:"] {
        if lowered.starts_with(scheme) {
            return None;
        }
    }
    let target = target.split('#').next().unwrap_or(target);
    if target.is_empty() {
        return None;
    }
    let mut stack: Vec<&str> = Vec::new();
    if !target.starts_with('/') {
        let mut parts: Vec<&str> = from.split('/').collect();
        parts.pop();
        stack.extend(parts);
    }
    for segment in target.trim_start_matches('/').split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    if stack.is_empty() {
        None
    } else {
        Some(stack.join("/"))
    }
}

fn index_for(relative: &str, known: &BTreeSet<String>) -> Option<String> {
    let mut parts: Vec<&str> = relative.split('/').collect();
    let file = parts.pop()?;
    if INDEX_NAMES.contains(&file) {
        return None;
    }
    let prefix = if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    };
    INDEX_NAMES
        .iter()
        .map(|name| format!("{prefix}{name}"))
        .find(|candidate| known.contains(candidate))
}

/// First sentence of a paragraph, for the card's one-sentence decision.
fn truncate_sentence(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        out.push(character);
        if character == '.' && out.len() > 20 {
            break;
        }
        if out.chars().count() >= 240 {
            break;
        }
    }
    out.trim().to_string()
}
