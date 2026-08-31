//! The project scanner: a deterministic, std-only walk that reads a source tree the way
//! [`bioprism_adapter::InventoryAdapter`] reads a data repository — and declares what it skips.
//!
//! Blueprint 40.17 (the data adapter contract) is the only spec this module answers to. Its
//! rule is that every ingest returns facts *and* a semantic-loss audit in one sealed value, and
//! that every unread byte is declared rather than remembered. A project tree exercises that
//! rule constantly: binary blobs, oversized files, excluded build directories, manifest lines
//! the narrow readers do not understand — each becomes a [`LossEntry`] naming its
//! [`SourceLocation`], never a silence.
//!
//! Everything counted here is a **static textual proxy**. A `TODO` occurrence is a substring
//! match, which over-counts markers quoted inside string literals and under-counts lowercase
//! `todo!()`. A `#[test]` occurrence is a counted attribute, not an executed test. The scanner
//! says what it counted; it never claims the counts mean more than they do.

use crate::ProjectError;
use bioprism_adapter::probe::{walk, FileEntry};
use bioprism_adapter::{
    Adapter, AdapterError, AdapterManifest, ConformanceLevel, FactDraft, Ingestion, LocationSet,
    LossAudit, LossEntry, LossKind, LossSeverity, SemanticLoss, Source, SourceLocation,
    SourceManifest,
};
use bioprism_ids::ContentHash;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Stable adapter name, used in manifests, fact provenance and conformance reports.
pub const PROJECT_ADAPTER: &str = "bioprism.project";
pub const PROJECT_ADAPTER_VERSION: &str = "0.1.0";

/// Directories never descended into for content, at any depth.
///
/// The list is a mapping decision and mapping decisions must be declared, so every file found
/// under one of these names becomes a [`LossKind::ContentUninterpreted`] entry rather than
/// vanishing. Build outputs and vendored dependency caches are excluded because their bytes are
/// derived, not authored; `.git` is excluded because history is out of this scanner's declared
/// scope (see the crate-level "not implemented" list).
pub const EXCLUDED_DIRS: [&str; 4] = ["target", "node_modules", ".git", "dist"];

/// Files larger than this are inventoried by path and byte count only: no digest, no content
/// scan. Reading is whole-file in memory, so an unbounded scan of a tree with one huge artifact
/// would exhaust memory; the skip is declared as loss, never silent.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Marker substrings counted in every UTF-8 file. Case-sensitive, plain substring matches.
const TODO_MARKER: &str = "TODO";
const FIXME_MARKER: &str = "FIXME";
const UNIMPLEMENTED_MARKER: &str = "unimplemented!";
const TEST_MARKER: &str = "#[test]";

/// Policy for one project scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Value bound to the `project` scope dimension of every emitted fact, and the source id
    /// of every location in the loss report.
    pub project: String,
    /// See [`DEFAULT_MAX_FILE_BYTES`].
    pub max_file_bytes: u64,
}

impl ScanOptions {
    pub fn new(project: impl Into<String>) -> Self {
        ScanOptions {
            project: project.into(),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        }
    }

    pub fn with_max_file_bytes(mut self, limit: u64) -> Self {
        self.max_file_bytes = limit;
        self
    }

    fn digest(&self) -> Option<ContentHash> {
        ContentHash::of_value(&serde_json::to_value(self).ok()?).ok()
    }
}

/// What the scanner established about one file's content.
///
/// The variants are mutually exclusive claims. `Text` counts ride only here — a `Binary` file
/// does not have `lines: 0`, it has no line count at all, because "counted zero" and "could not
/// count" must never share a representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    Text {
        lines: u64,
        todo_markers: u64,
        fixme_markers: u64,
        unimplemented_markers: u64,
        /// `Some` only for `.rs` files, where the `#[test]` proxy is defined. For every other
        /// file the count was never taken, and `None` records that rather than a zero.
        test_functions: Option<u64>,
    },
    /// Bytes hashed but not valid UTF-8; content declared uninterpreted.
    Binary,
    /// Larger than the byte cap; named and sized but neither hashed nor read.
    Oversized,
    /// Recorded but never followed.
    Symlink,
}

/// One scanned file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    /// Slash-separated path relative to the project root.
    pub path: String,
    /// Component directory key: `""` for the project root, otherwise a directory path such as
    /// `src` or `crates/adapter`. See [`component_display`].
    pub component: String,
    /// Absent for symlinks, whose length is not a meaningful property.
    pub byte_length: Option<u64>,
    /// Absent for symlinks and oversized files — both absences are declared as loss.
    pub sha256: Option<String>,
    pub content: FileContent,
}

impl FileRecord {
    pub fn is_uninterpreted(&self) -> bool {
        !matches!(self.content, FileContent::Text { .. })
    }
}

/// Which narrow reader a manifest went through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestKind {
    CargoToml,
    PackageJson,
    PyprojectToml,
}

impl ManifestKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ManifestKind::CargoToml => "cargo_toml",
            ManifestKind::PackageJson => "package_json",
            ManifestKind::PyprojectToml => "pyproject_toml",
        }
    }

    fn of_basename(name: &str) -> Option<ManifestKind> {
        match name {
            "Cargo.toml" => Some(ManifestKind::CargoToml),
            "package.json" => Some(ManifestKind::PackageJson),
            "pyproject.toml" => Some(ManifestKind::PyprojectToml),
            _ => None,
        }
    }
}

/// One dependency declaration as written in a manifest — a requirement string, never a
/// resolved version.
///
/// "Pinned" is defined narrowly and stated: a Cargo requirement is pinned when it begins with
/// `=`; a package.json requirement is pinned when it is an exact three-part numeric semver
/// (`1.2.3` — prerelease and build suffixes are treated as not pinned); a pyproject requirement
/// is pinned when it contains the `==` operator. `requirement: None` means the declaration
/// carries no version requirement at all (a path or workspace dependency); such a declaration
/// is neither pinned nor unpinned and never enters `unpinned_dependencies`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DependencyRecord {
    pub manifest: String,
    pub name: String,
    pub requirement: Option<String>,
    pub pinned: bool,
}

/// What one manifest's narrow reader carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestRecord {
    pub path: String,
    pub kind: ManifestKind,
    /// `[workspace] members` entries, Cargo only.
    pub workspace_members: Vec<String>,
    pub dependencies: Vec<DependencyRecord>,
    /// `scripts` key names, package.json only. Carried into the ingestion's manifest fact and
    /// nowhere else.
    pub script_names: Vec<String>,
}

/// One issue from a caller-supplied issues file.
///
/// Parsing is strict: an undeclared key is refused, because a misspelled `component` key that
/// silently parsed would make an issue's evidence region look deliberately empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    /// Component directory paths, component slugs, or file paths inside a component. Relevance
    /// comes from these declarations alone — there is no semantic search.
    pub components: Vec<String>,
}

impl Issue {
    /// Parses a JSON array of issue objects. Declared keys: `id`, `title`, `body` (optional),
    /// `components` (optional). Ids must be non-empty `[A-Za-z0-9._-]` and unique, because they
    /// are spliced verbatim into variable names.
    pub fn parse_array(document: &Value) -> Result<Vec<Issue>, ProjectError> {
        let items = document
            .as_array()
            .ok_or_else(|| ProjectError::Issues("issues document is not an array".into()))?;
        let mut seen = BTreeSet::new();
        let mut issues = Vec::new();
        for (position, item) in items.iter().enumerate() {
            let map = item.as_object().ok_or_else(|| {
                ProjectError::Issues(format!("issue at position {position} is not an object"))
            })?;
            const DECLARED: &[&str] = &["id", "title", "body", "components"];
            if let Some(unknown) = map.keys().find(|key| !DECLARED.contains(&key.as_str())) {
                return Err(ProjectError::Issues(format!(
                    "undeclared key {unknown:?} on issue at position {position}"
                )));
            }
            let id = require_string(map, "id", position)?;
            if id.is_empty()
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            {
                return Err(ProjectError::Issues(format!(
                    "issue id {id:?} must be non-empty [A-Za-z0-9._-]; it becomes part of a \
                     variable name"
                )));
            }
            if !seen.insert(id.clone()) {
                return Err(ProjectError::Issues(format!("duplicate issue id {id:?}")));
            }
            let title = require_string(map, "title", position)?;
            let body = match map.get("body") {
                None => None,
                Some(value) => Some(value.as_str().map(str::to_string).ok_or_else(|| {
                    ProjectError::Issues(format!("issue {id:?}: \"body\" is not a string"))
                })?),
            };
            let components = match map.get("components") {
                None => Vec::new(),
                Some(value) => value
                    .as_array()
                    .ok_or_else(|| {
                        ProjectError::Issues(format!("issue {id:?}: \"components\" is not an array"))
                    })?
                    .iter()
                    .map(|entry| {
                        entry.as_str().map(str::to_string).ok_or_else(|| {
                            ProjectError::Issues(format!(
                                "issue {id:?}: \"components\" carries a non-string entry"
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            issues.push(Issue {
                id,
                title,
                body,
                components,
            });
        }
        Ok(issues)
    }

    /// Reads and parses an issues file.
    pub fn load(path: &Path) -> Result<Vec<Issue>, ProjectError> {
        let text = std::fs::read_to_string(path).map_err(|error| ProjectError::Io {
            path: path.to_string_lossy().replace('\\', "/"),
            message: error.to_string(),
        })?;
        let document: Value = serde_json::from_str(&text)
            .map_err(|error| ProjectError::Issues(format!("issues file is not JSON: {error}")))?;
        Issue::parse_array(&document)
    }
}

fn require_string(
    map: &Map<String, Value>,
    field: &str,
    position: usize,
) -> Result<String, ProjectError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ProjectError::Issues(format!(
                "issue at position {position} needs a string {field:?}"
            ))
        })
}

/// The typed result of one scan. Excluded files are counted, not listed — their per-file
/// declarations live in [`ProjectScan::loss`].
#[derive(Debug, Clone)]
pub struct ProjectScan {
    pub project: String,
    /// Every non-excluded entry, sorted by path.
    pub files: Vec<FileRecord>,
    pub manifests: Vec<ManifestRecord>,
    /// All dependency declarations across manifests, sorted by (manifest, name, requirement).
    pub dependencies: Vec<DependencyRecord>,
    /// Workflow file paths under `.github/workflows/`, sorted.
    pub workflows: Vec<String>,
    /// `*.md` paths, sorted.
    pub docs: Vec<String>,
    pub excluded_file_count: u64,
    /// The same audit the ingestion carries. The loss travels with the scan so world assembly
    /// can put it *into* the world as evidence.
    pub loss: SemanticLoss,
}

impl ProjectScan {
    /// Scans `root` and returns the typed scan alongside the sealed ingestion.
    ///
    /// The ingestion is [`ProjectAdapter`]'s output for the same walk: one fact per file, one
    /// fact per manifest, and the mandatory loss audit. The two views are built from a single
    /// pass, so they cannot disagree about what was read.
    pub fn scan(root: &Path, options: &ScanOptions) -> Result<(ProjectScan, Ingestion), ProjectError> {
        let source = Source::directory(options.project.clone(), root);
        let collected = collect(&source, options)?;
        let ingestion = build_ingestion(&collected, &source, options)?;
        let scan = ProjectScan {
            project: options.project.clone(),
            dependencies: flatten_dependencies(&collected.manifests),
            files: collected.files,
            manifests: collected.manifests,
            workflows: collected.workflows,
            docs: collected.docs,
            excluded_file_count: collected.excluded_file_count,
            loss: ingestion.loss().clone(),
        };
        Ok((scan, ingestion))
    }

    /// The subset of [`ProjectScan::dependencies`] with a version requirement that is not an
    /// exact pin, under the definition on [`DependencyRecord`].
    pub fn unpinned_dependencies(&self) -> Vec<&DependencyRecord> {
        self.dependencies
            .iter()
            .filter(|dep| dep.requirement.is_some() && !dep.pinned)
            .collect()
    }

    pub fn test_function_total(&self) -> u64 {
        self.files
            .iter()
            .filter_map(|file| match &file.content {
                FileContent::Text { test_functions, .. } => *test_functions,
                _ => None,
            })
            .sum()
    }

    pub fn todo_marker_total(&self) -> u64 {
        self.files
            .iter()
            .filter_map(|file| match &file.content {
                FileContent::Text { todo_markers, .. } => Some(*todo_markers),
                _ => None,
            })
            .sum()
    }

    pub fn uninterpreted_file_total(&self) -> u64 {
        self.files.iter().filter(|f| f.is_uninterpreted()).count() as u64
    }

    /// Loss entries counted by kind. This is the summary that ships into the world as the
    /// `scan_loss_summary` fact, so the audit's gaps are evidence the oracle must receive.
    pub fn loss_kind_counts(&self) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::new();
        for entry in self.loss.entries() {
            *counts.entry(entry.kind.as_str().to_string()).or_insert(0) += 1;
        }
        counts
    }
}

fn flatten_dependencies(manifests: &[ManifestRecord]) -> Vec<DependencyRecord> {
    let mut all: Vec<DependencyRecord> = manifests
        .iter()
        .flat_map(|manifest| manifest.dependencies.iter().cloned())
        .collect();
    all.sort();
    all
}

/// The project adapter, implementing [`bioprism_adapter::Adapter`] directly: a project root is
/// a [`Source`] directory exactly as [`bioprism_adapter::InventoryAdapter`]'s repositories are,
/// so the sealed contract fits without mirroring.
#[derive(Debug, Clone)]
pub struct ProjectAdapter {
    options: ScanOptions,
}

impl ProjectAdapter {
    pub fn new(options: ScanOptions) -> Self {
        ProjectAdapter { options }
    }

    pub fn options(&self) -> &ScanOptions {
        &self.options
    }
}

impl Adapter for ProjectAdapter {
    fn name(&self) -> &str {
        PROJECT_ADAPTER
    }

    /// The three loss kinds this adapter can emit.
    ///
    /// [`LossKind::UnmappedColumn`] is **borrowed**, and the borrowing is the one thing a reader
    /// of an aggregated loss report has to know about this adapter. `LossKind` is a sealed
    /// eight-variant vocabulary written for tabular biological sources, where the variant means
    /// "a source column exists but no mapping rule claims it". A manifest has no columns. Here
    /// it carries the structurally identical fact one level up — *a declaration exists in the
    /// source and the narrow reader claimed none of it* — because that is the closest true
    /// statement the sealed vocabulary can make, and inventing a variant for one non-biological
    /// adapter would widen a contract every other adapter and every full-loss-surface manifest
    /// declares against.
    ///
    /// The cost is real and is not hidden: a consumer summing `losses_by_kind` across adapters
    /// reads project manifest lines and tabular columns in one bucket. Every such entry's
    /// `detail` says "narrow reader" and its `location` names the exact line, so the entries
    /// stay distinguishable individually even though the counts do not separate themselves.
    fn manifest(&self) -> AdapterManifest {
        AdapterManifest::new(
            PROJECT_ADAPTER,
            PROJECT_ADAPTER_VERSION,
            ConformanceLevel::Normalize,
        )
        .declaring([
            LossKind::ContentUninterpreted,
            LossKind::ProvenanceUnavailable,
            LossKind::UnmappedColumn,
        ])
        .binding(["project", "component"])
    }

    fn ingest(&self, source: &Source) -> Result<Ingestion, AdapterError> {
        let collected = collect(source, &self.options)?;
        build_ingestion(&collected, source, &self.options)
    }
}

/// Everything one walk established, before it is shaped into facts or a typed scan.
struct Collected {
    files: Vec<FileRecord>,
    manifests: Vec<ManifestRecord>,
    workflows: Vec<String>,
    docs: Vec<String>,
    excluded_file_count: u64,
    mapped: LocationSet,
    losses: Vec<LossEntry>,
    total_bytes: u64,
}

fn collect(source: &Source, options: &ScanOptions) -> Result<Collected, AdapterError> {
    let root = source.as_directory(PROJECT_ADAPTER)?;
    let entries = walk(root)?;
    let source_id = source.id.as_str();

    let manifest_dirs: BTreeSet<String> = entries
        .iter()
        .filter(|entry| !is_excluded(&entry.relative) && !entry.is_symlink)
        .filter(|entry| ManifestKind::of_basename(basename(&entry.relative)).is_some())
        .map(|entry| parent_dir(&entry.relative).to_string())
        .collect();

    let mut collected = Collected {
        files: Vec::new(),
        manifests: Vec::new(),
        workflows: Vec::new(),
        docs: Vec::new(),
        excluded_file_count: 0,
        mapped: LocationSet::new(),
        losses: Vec::new(),
        total_bytes: 0,
    };

    for entry in &entries {
        let location = SourceLocation::artifact(source_id, &entry.relative);
        if is_excluded(&entry.relative) {
            collected.excluded_file_count += 1;
            collected.losses.push(LossEntry::new(
                LossKind::ContentUninterpreted,
                LossSeverity::Advisory,
                location,
                format!(
                    "file skipped by the declared exclusion list ({}); its bytes are neither \
                     hashed nor counted",
                    EXCLUDED_DIRS.join(", ")
                ),
            ));
            continue;
        }

        collected.mapped.insert(location.clone());
        let component = component_of(&entry.relative, &manifest_dirs);

        if entry.is_symlink {
            collected.losses.push(LossEntry::new(
                LossKind::ContentUninterpreted,
                LossSeverity::Degrading,
                location,
                "symbolic link recorded but not followed, so neither its target's bytes nor \
                 its digest are in the scan",
            ));
            collected.files.push(FileRecord {
                path: entry.relative.clone(),
                component,
                byte_length: None,
                sha256: None,
                content: FileContent::Symlink,
            });
            continue;
        }

        let length = entry.length.unwrap_or_default();
        collected.total_bytes += length;
        if length > options.max_file_bytes {
            collected.losses.push(LossEntry::new(
                LossKind::ContentUninterpreted,
                LossSeverity::Degrading,
                location,
                format!(
                    "{length} bytes exceeds the {}-byte cap, so the file is named and sized \
                     but neither hashed nor scanned; every marker and line count that would \
                     have come from it is missing, not zero",
                    options.max_file_bytes
                ),
            ));
            collected.files.push(FileRecord {
                path: entry.relative.clone(),
                component,
                byte_length: Some(length),
                sha256: None,
                content: FileContent::Oversized,
            });
            continue;
        }

        let bytes = std::fs::read(&entry.absolute).map_err(|error| AdapterError::Io {
            path: entry.relative.clone(),
            message: error.to_string(),
        })?;
        let sha256 = ContentHash::of_bytes(&bytes).to_string();

        match String::from_utf8(bytes) {
            Err(_) => {
                collected.losses.push(LossEntry::new(
                    LossKind::ContentUninterpreted,
                    LossSeverity::Advisory,
                    location,
                    "bytes hashed but not valid UTF-8; content declared uninterpreted",
                ));
                collected.files.push(FileRecord {
                    path: entry.relative.clone(),
                    component,
                    byte_length: Some(length),
                    sha256: Some(sha256),
                    content: FileContent::Binary,
                });
            }
            Ok(text) => {
                collect_text(source_id, entry, &text, component, sha256, &mut collected);
            }
        }
    }

    Ok(collected)
}

fn collect_text(
    source_id: &str,
    entry: &FileEntry,
    text: &str,
    component: String,
    sha256: String,
    collected: &mut Collected,
) {
    let relative = entry.relative.as_str();
    let location = SourceLocation::artifact(source_id, relative);
    let is_rust = relative.ends_with(".rs");

    let content = FileContent::Text {
        lines: line_count(text),
        todo_markers: text.matches(TODO_MARKER).count() as u64,
        fixme_markers: text.matches(FIXME_MARKER).count() as u64,
        unimplemented_markers: text.matches(UNIMPLEMENTED_MARKER).count() as u64,
        test_functions: is_rust.then(|| text.matches(TEST_MARKER).count() as u64),
    };

    if relative.ends_with(".md") {
        collected.docs.push(relative.to_string());
    }

    if let Some(kind) = ManifestKind::of_basename(basename(relative)) {
        collected.losses.push(LossEntry::new(
            LossKind::ContentUninterpreted,
            LossSeverity::Advisory,
            location,
            "manifest read narrowly: only workspace members and dependency declarations are \
             carried, and every line the narrow reader does not understand is declared below, \
             line by line",
        ));
        let record = match kind {
            ManifestKind::CargoToml => {
                read_cargo_manifest(source_id, relative, text, &mut collected.losses)
            }
            ManifestKind::PackageJson => {
                read_package_json(source_id, relative, text, &mut collected.losses)
            }
            ManifestKind::PyprojectToml => {
                read_pyproject(source_id, relative, text, &mut collected.losses)
            }
        };
        collected.manifests.push(record);
    } else if relative.starts_with(".github/workflows/") {
        collected.workflows.push(relative.to_string());
        collected.losses.push(LossEntry::new(
            LossKind::ContentUninterpreted,
            LossSeverity::Degrading,
            location,
            "CI workflow inventoried but its content is not interpreted; the audit sees that a \
             workflow exists, never what it runs, so a workflow that does nothing would still \
             satisfy the presence check",
        ));
    } else {
        collected.losses.push(LossEntry::new(
            LossKind::ContentUninterpreted,
            LossSeverity::Advisory,
            location,
            "bytes read for line and marker counts only; the content receives no semantic \
             reading",
        ));
    }

    collected.files.push(FileRecord {
        path: relative.to_string(),
        component,
        byte_length: entry.length,
        sha256: Some(sha256),
        content,
    });
}

/// `\n` count, plus one for a non-empty final line without a terminator.
fn line_count(text: &str) -> u64 {
    let newlines = text.matches('\n').count() as u64;
    if !text.is_empty() && !text.ends_with('\n') {
        newlines + 1
    } else {
        newlines
    }
}

/// True when any path segment is on [`EXCLUDED_DIRS`], at any depth.
fn is_excluded(relative: &str) -> bool {
    relative
        .split('/')
        .any(|segment| EXCLUDED_DIRS.contains(&segment))
}

fn basename(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

/// Parent directory of a relative path, `""` for a root-level entry.
fn parent_dir(relative: &str) -> &str {
    match relative.rfind('/') {
        Some(index) => &relative[..index],
        None => "",
    }
}

/// Assigns a file to its component.
///
/// The rule is syntactic and stated: the nearest ancestor directory *below the root* that
/// directly contains a recognized manifest wins; otherwise the file's top-level directory;
/// otherwise the root component `""`. The root manifest deliberately does not claim the whole
/// tree — "component = top-level crate/package dir or src tree" — so `src/lib.rs` belongs to
/// `src` even when `Cargo.toml` sits beside it at the root.
fn component_of(relative: &str, manifest_dirs: &BTreeSet<String>) -> String {
    let mut dir = parent_dir(relative);
    while !dir.is_empty() {
        if manifest_dirs.contains(dir) {
            return dir.to_string();
        }
        dir = parent_dir(dir);
    }
    match relative.find('/') {
        Some(index) => relative[..index].to_string(),
        None => String::new(),
    }
}

/// The display name of a component key: `root` for the root component, the directory path
/// otherwise.
pub fn component_display(component: &str) -> String {
    if component.is_empty() {
        "root".to_string()
    } else {
        component.to_string()
    }
}

/// Lowercased `[a-z0-9]`-and-underscore slug of a component key, for variable names.
pub fn component_slug(component: &str) -> String {
    if component.is_empty() {
        return "root".to_string();
    }
    component
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn path_slug(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The line-based Cargo.toml reader. Understood: section headers, comments, blank lines, the
/// `[workspace] members` array, and — inside the four dependency sections — the two common
/// forms `name = "req"` and `name = { ... }` (with a narrow `version = "..."` extraction).
/// Everything else is a declared loss at its line, Degrading when the line sits in a
/// dependency-shaped section it cannot read.
fn read_cargo_manifest(
    source_id: &str,
    relative: &str,
    text: &str,
    losses: &mut Vec<LossEntry>,
) -> ManifestRecord {
    const DEP_SECTIONS: [&str; 4] = [
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "workspace.dependencies",
    ];

    let mut section = String::new();
    let mut in_members = false;
    let mut members = Vec::new();
    let mut dependencies = Vec::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        let location = SourceLocation::record(source_id, (index + 1) as u64).within(relative);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if in_members {
            members.extend(quoted_strings(line));
            if line.contains(']') {
                in_members = false;
            }
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        if DEP_SECTIONS.contains(&section.as_str()) {
            match parse_cargo_dep_line(line) {
                Some((name, requirement, extraction_gap)) => {
                    if let Some(gap) = extraction_gap {
                        losses.push(LossEntry::new(
                            LossKind::UnmappedColumn,
                            LossSeverity::Degrading,
                            location,
                            gap,
                        ));
                    }
                    let pinned = requirement
                        .as_deref()
                        .is_some_and(|req| req.trim().starts_with('='));
                    dependencies.push(DependencyRecord {
                        manifest: relative.to_string(),
                        name,
                        requirement,
                        pinned,
                    });
                }
                None => losses.push(LossEntry::new(
                    LossKind::UnmappedColumn,
                    LossSeverity::Degrading,
                    location,
                    format!(
                        "line in [{section}] is not one of the two dependency forms the narrow \
                         reader understands; any dependency it declares is not in the scan"
                    ),
                )),
            }
        } else if section == "workspace" {
            match parse_members_opening(line) {
                Some((line_members, closed)) => {
                    members.extend(line_members);
                    in_members = !closed;
                }
                None => losses.push(LossEntry::new(
                    LossKind::UnmappedColumn,
                    LossSeverity::Advisory,
                    location,
                    "line in [workspace] other than the members array is not read",
                )),
            }
        } else {
            let severity = if section.contains("dependenc") {
                LossSeverity::Degrading
            } else {
                LossSeverity::Advisory
            };
            losses.push(LossEntry::new(
                LossKind::UnmappedColumn,
                severity,
                location,
                format!(
                    "line in section [{section}] is outside the narrow reader (workspace \
                     members and the four plain dependency sections)"
                ),
            ));
        }
    }

    ManifestRecord {
        path: relative.to_string(),
        kind: ManifestKind::CargoToml,
        workspace_members: members,
        dependencies,
        script_names: Vec::new(),
    }
}

/// `members = [...` — returns the entries found on this line and whether the array closed.
fn parse_members_opening(line: &str) -> Option<(Vec<String>, bool)> {
    let rest = line.strip_prefix("members")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    Some((quoted_strings(rest), rest.contains(']')))
}

/// Parses `name = "req"` or `name = { ... }`. Returns `(name, requirement, extraction_gap)`;
/// `None` means the line is not one of the two forms.
#[allow(clippy::type_complexity)]
fn parse_cargo_dep_line(line: &str) -> Option<(String, Option<String>, Option<String>)> {
    let (left, right) = line.split_once('=')?;
    let name = left.trim().trim_matches('"').to_string();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return None;
    }
    let right = right.trim();

    if right.len() >= 2 && right.starts_with('"') && right.ends_with('"') {
        return Some((name, Some(right[1..right.len() - 1].to_string()), None));
    }

    if right.starts_with('{') && right.ends_with('}') {
        if let Some(index) = right.find("version") {
            let tail = right[index + "version".len()..].trim_start();
            if let Some(tail) = tail.strip_prefix('=') {
                let tail = tail.trim_start();
                if let Some(stripped) = tail.strip_prefix('"') {
                    if let Some(end) = stripped.find('"') {
                        return Some((name, Some(stripped[..end].to_string()), None));
                    }
                }
            }
            return Some((
                name,
                None,
                Some(
                    "the inline table names a version the narrow reader cannot extract; the \
                     requirement is recorded as absent, not guessed"
                        .to_string(),
                ),
            ));
        }
        return Some((name, None, None));
    }

    None
}

/// All substrings between successive pairs of `"` on one line.
fn quoted_strings(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        match after.find('"') {
            Some(end) => {
                out.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    out
}

/// package.json via serde_json. Understood top-level keys: `dependencies`, `devDependencies`
/// (name → requirement string) and `scripts` (key names). Every other top-level key is a
/// declared loss.
fn read_package_json(
    source_id: &str,
    relative: &str,
    text: &str,
    losses: &mut Vec<LossEntry>,
) -> ManifestRecord {
    let mut record = ManifestRecord {
        path: relative.to_string(),
        kind: ManifestKind::PackageJson,
        workspace_members: Vec::new(),
        dependencies: Vec::new(),
        script_names: Vec::new(),
    };

    let document: Value = match serde_json::from_str(text) {
        Ok(document) => document,
        Err(error) => {
            losses.push(LossEntry::new(
                LossKind::ContentUninterpreted,
                LossSeverity::Degrading,
                SourceLocation::artifact(source_id, relative),
                format!("package.json did not parse as JSON, so nothing was carried: {error}"),
            ));
            return record;
        }
    };
    let Some(map) = document.as_object() else {
        losses.push(LossEntry::new(
            LossKind::ContentUninterpreted,
            LossSeverity::Degrading,
            SourceLocation::artifact(source_id, relative),
            "package.json is valid JSON but not an object, so nothing was carried",
        ));
        return record;
    };

    for (key, value) in map {
        let field_location = SourceLocation {
            source: source_id.to_string(),
            artifact: Some(relative.to_string()),
            record: None,
            field: Some(key.clone()),
        };
        match key.as_str() {
            "dependencies" | "devDependencies" => match value.as_object() {
                Some(entries) => {
                    for (name, requirement) in entries {
                        match requirement.as_str() {
                            Some(requirement) => record.dependencies.push(DependencyRecord {
                                manifest: relative.to_string(),
                                name: name.clone(),
                                requirement: Some(requirement.to_string()),
                                pinned: exact_semver(requirement),
                            }),
                            None => losses.push(LossEntry::new(
                                LossKind::UnmappedColumn,
                                LossSeverity::Degrading,
                                SourceLocation {
                                    field: Some(format!("{key}.{name}")),
                                    ..field_location.clone()
                                },
                                "dependency requirement is not a string and was not carried",
                            )),
                        }
                    }
                }
                None => losses.push(LossEntry::new(
                    LossKind::UnmappedColumn,
                    LossSeverity::Degrading,
                    field_location,
                    format!("{key:?} is not an object and was not carried"),
                )),
            },
            "scripts" => match value.as_object() {
                Some(entries) => record.script_names.extend(entries.keys().cloned()),
                None => losses.push(LossEntry::new(
                    LossKind::UnmappedColumn,
                    LossSeverity::Advisory,
                    field_location,
                    "\"scripts\" is not an object and was not carried",
                )),
            },
            _ => losses.push(LossEntry::new(
                LossKind::UnmappedColumn,
                LossSeverity::Advisory,
                field_location,
                "top-level key outside the narrow reader (dependencies, devDependencies, \
                 scripts) and not carried",
            )),
        }
    }

    record.dependencies.sort();
    record.script_names.sort();
    record
}

/// Exact three-part numeric semver, the declared package.json pin definition.
fn exact_semver(requirement: &str) -> bool {
    let parts: Vec<&str> = requirement.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// The line-based pyproject.toml reader: only the `[project] dependencies` array is carried.
fn read_pyproject(
    source_id: &str,
    relative: &str,
    text: &str,
    losses: &mut Vec<LossEntry>,
) -> ManifestRecord {
    let mut section = String::new();
    let mut in_dependencies = false;
    let mut dependencies = Vec::new();

    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        let location = SourceLocation::record(source_id, (index + 1) as u64).within(relative);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if in_dependencies {
            for entry in quoted_strings(line) {
                dependencies.push(pep508_record(relative, &entry));
            }
            if line.contains(']') {
                in_dependencies = false;
            }
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        if section == "project" {
            if let Some(rest) = line.strip_prefix("dependencies") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim_start();
                    if rest.starts_with('[') {
                        for entry in quoted_strings(rest) {
                            dependencies.push(pep508_record(relative, &entry));
                        }
                        in_dependencies = !rest.contains(']');
                        continue;
                    }
                }
            }
            losses.push(LossEntry::new(
                LossKind::UnmappedColumn,
                LossSeverity::Advisory,
                location,
                "line in [project] other than the dependencies array is not read",
            ));
        } else {
            let severity = if section.contains("dependenc") {
                LossSeverity::Degrading
            } else {
                LossSeverity::Advisory
            };
            losses.push(LossEntry::new(
                LossKind::UnmappedColumn,
                severity,
                location,
                format!(
                    "line in section [{section}] is outside the narrow reader ([project] \
                     dependencies only)"
                ),
            ));
        }
    }

    dependencies.sort();
    ManifestRecord {
        path: relative.to_string(),
        kind: ManifestKind::PyprojectToml,
        workspace_members: Vec::new(),
        dependencies,
        script_names: Vec::new(),
    }
}

/// Splits a PEP 508 requirement string at the end of the leading name run. The pyproject pin
/// definition: a requirement containing `==`.
fn pep508_record(manifest: &str, entry: &str) -> DependencyRecord {
    let split = entry
        .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
        .unwrap_or(entry.len());
    let (name, rest) = entry.split_at(split);
    let rest = rest.trim();
    let requirement = if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    };
    DependencyRecord {
        manifest: manifest.to_string(),
        name: name.to_string(),
        requirement: requirement.clone(),
        pinned: requirement.as_deref().is_some_and(|r| r.contains("==")),
    }
}

fn build_ingestion(
    collected: &Collected,
    source: &Source,
    options: &ScanOptions,
) -> Result<Ingestion, AdapterError> {
    let mut audit = LossAudit::new();
    for location in &collected.mapped {
        audit.mapped(location.clone());
    }
    for entry in &collected.losses {
        audit.lost(entry.clone());
    }

    let source_provenance = match &source.provenance {
        Some(provenance) if !provenance.is_empty() => provenance.strings(),
        _ => {
            audit.record(
                LossKind::ProvenanceUnavailable,
                LossSeverity::Degrading,
                SourceLocation::source(&source.id),
                "the caller supplied no upstream accession, version or retrieval time, and this \
                 scanner reads no git history; a filesystem mtime is not evidence about the \
                 project's origin",
            );
            Vec::new()
        }
    };

    let mut facts = Vec::new();
    for file in &collected.files {
        let location = SourceLocation::artifact(&source.id, &file.path);
        let mut value = Map::new();
        value.insert("path".to_string(), Value::String(file.path.clone()));
        value.insert(
            "component".to_string(),
            Value::String(component_display(&file.component)),
        );
        if let Some(length) = file.byte_length {
            value.insert("byte_length".to_string(), Value::from(length));
        }
        if let Some(sha256) = &file.sha256 {
            value.insert("sha256".to_string(), Value::String(sha256.clone()));
        }
        match &file.content {
            FileContent::Text {
                lines,
                todo_markers,
                fixme_markers,
                unimplemented_markers,
                test_functions,
            } => {
                value.insert("lines".to_string(), Value::from(*lines));
                value.insert("todo_markers".to_string(), Value::from(*todo_markers));
                value.insert("fixme_markers".to_string(), Value::from(*fixme_markers));
                value.insert(
                    "unimplemented_markers".to_string(),
                    Value::from(*unimplemented_markers),
                );
                if let Some(tests) = test_functions {
                    value.insert("test_functions".to_string(), Value::from(*tests));
                }
            }
            FileContent::Binary => {
                value.insert("binary".to_string(), Value::Bool(true));
            }
            FileContent::Oversized => {
                value.insert("oversized".to_string(), Value::Bool(true));
            }
            FileContent::Symlink => {
                value.insert("symlink".to_string(), Value::Bool(true));
            }
        }

        let scope = ScopeKey::new()
            .exact("project", &options.project)
            .exact("component", component_display(&file.component));
        let draft = FactDraft::new(
            format!("fact.file.{}", file.path),
            format!("file_{}", path_slug(&file.path)),
            Value::Object(value),
            scope,
            location.clone(),
        )
        .provenances(source_provenance.iter().cloned())
        .provenance(location.locator())
        .tag("file");
        facts.push(draft.build()?);
    }

    for manifest in &collected.manifests {
        let location = SourceLocation::artifact(&source.id, &manifest.path);
        let dependencies: Vec<Value> = manifest
            .dependencies
            .iter()
            .map(dependency_value)
            .collect();
        let mut value = Map::new();
        value.insert("path".to_string(), Value::String(manifest.path.clone()));
        value.insert(
            "kind".to_string(),
            Value::String(manifest.kind.as_str().to_string()),
        );
        value.insert(
            "workspace_members".to_string(),
            Value::Array(
                manifest
                    .workspace_members
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        value.insert("dependencies".to_string(), Value::Array(dependencies));
        value.insert(
            "scripts".to_string(),
            Value::Array(
                manifest
                    .script_names
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );

        let component = collected
            .files
            .iter()
            .find(|file| file.path == manifest.path)
            .map(|file| file.component.clone())
            .unwrap_or_default();
        let scope = ScopeKey::new()
            .exact("project", &options.project)
            .exact("component", component_display(&component));
        let draft = FactDraft::new(
            format!("fact.manifest.{}", manifest.path),
            format!("manifest_{}", path_slug(&manifest.path)),
            Value::Object(value),
            scope,
            location.clone(),
        )
        .provenances(source_provenance.iter().cloned())
        .provenance(location.locator())
        .tag("manifest");
        facts.push(draft.build()?);
    }

    // Both manifest measurements describe the *scanned* subset, not the whole tree, and that
    // differs from `InventoryAdapter`, whose `byte_length` sums every walked entry. Excluded
    // files and symlinks contribute to neither number here: a build cache that grew by a
    // gigabyte must not read as the project having grown, and re-scanning after `cargo build`
    // must not read as the tree having drifted. What was left out of them is not hidden — every
    // excluded file has its own entry in the loss report above.
    let manifest = SourceManifest {
        source_id: source.id.clone(),
        declared_format: source.declared_format.clone(),
        source_digest: listing_digest(&collected.files, &source.id)?,
        byte_length: Some(collected.total_bytes),
        adapter: PROJECT_ADAPTER.to_string(),
        adapter_version: PROJECT_ADAPTER_VERSION.to_string(),
        profile_digest: options.digest(),
        provenance: source.provenance.clone(),
    };

    Ingestion::new(manifest, facts, audit.finish())
}

/// The wire form of one dependency record.
pub(crate) fn dependency_value(record: &DependencyRecord) -> Value {
    let mut map = Map::new();
    map.insert(
        "manifest".to_string(),
        Value::String(record.manifest.clone()),
    );
    map.insert("name".to_string(), Value::String(record.name.clone()));
    map.insert(
        "requirement".to_string(),
        match &record.requirement {
            Some(requirement) => Value::String(requirement.clone()),
            None => Value::Null,
        },
    );
    map.insert("pinned".to_string(), Value::Bool(record.pinned));
    Value::Object(map)
}

/// Digest of the canonical file listing — the "digest of a directory" in the same sense as the
/// inventory adapter's: re-scanning later and getting a different value means the tree drifted.
fn listing_digest(files: &[FileRecord], source_id: &str) -> Result<ContentHash, AdapterError> {
    let listing = listing_value(files);
    ContentHash::of_value(&listing).map_err(|source| AdapterError::Canonical {
        location: Box::new(SourceLocation::source(source_id)),
        source,
    })
}

/// The canonical `[path, digest-or-size]` listing shared by the source digest and the world id.
pub(crate) fn listing_value(files: &[FileRecord]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| {
                Value::Array(vec![
                    Value::String(file.path.clone()),
                    match &file.sha256 {
                        Some(sha256) => Value::String(sha256.clone()),
                        None => Value::String(format!(
                            "unhashed:{}",
                            file.byte_length.unwrap_or_default()
                        )),
                    },
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_scanner_can_actually_see_a_planted_marker_and_ignores_a_clean_line() {
        let planted = format!("{}{}: later", "TO", "DO");
        assert_eq!(planted.matches(TODO_MARKER).count(), 1);
        assert_eq!("a clean line about tasks".matches(TODO_MARKER).count(), 0);
    }

    #[test]
    fn the_test_attribute_scanner_counts_attributes_not_test_shaped_words() {
        let source = "#[test]\nfn a() {}\n// a test of tests\n";
        assert_eq!(source.matches(TEST_MARKER).count(), 1);
    }

    #[test]
    fn a_file_inside_an_excluded_directory_is_excluded_at_any_depth() {
        assert!(is_excluded("target/debug/app.exe"));
        assert!(is_excluded("packages/site/node_modules/x/index.js"));
        assert!(!is_excluded("src/targets.rs"));
        assert!(!is_excluded("distributions/notes.md"));
    }

    #[test]
    fn a_nested_manifest_directory_claims_its_files_but_the_root_manifest_does_not() {
        let dirs: BTreeSet<String> = ["", "crates/adapter"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            component_of("crates/adapter/src/lib.rs", &dirs),
            "crates/adapter"
        );
        assert_eq!(component_of("src/lib.rs", &dirs), "src");
        assert_eq!(component_of("README.md", &dirs), "");
    }

    #[test]
    fn the_two_cargo_dependency_forms_parse_and_a_dotted_key_form_is_refused() {
        assert_eq!(
            parse_cargo_dep_line("exact-widget = \"=1.0.0\""),
            Some(("exact-widget".into(), Some("=1.0.0".into()), None))
        );
        let (name, requirement, gap) =
            parse_cargo_dep_line("serde = { version = \"=1.0.229\", features = [\"derive\"] }")
                .unwrap();
        assert_eq!(name, "serde");
        assert_eq!(requirement.as_deref(), Some("=1.0.229"));
        assert!(gap.is_none());
        assert_eq!(parse_cargo_dep_line("serde.workspace = true"), None);
    }

    /// Guards what an aggregated loss report cannot show on its own: manifest lines are counted
    /// under a kind whose sealed definition says "column". If that attribution silently changes,
    /// the argument on `ProjectAdapter::manifest` is wrong and every consumer's bucket totals
    /// shift meaning with no signal — so the borrowing is pinned here rather than left to prose.
    #[test]
    fn an_unread_manifest_line_takes_the_borrowed_unmapped_column_kind_and_a_detail_that_separates_it_from_a_real_column(
    ) {
        let mut losses = Vec::new();
        read_cargo_manifest(
            "demo",
            "Cargo.toml",
            "[dependencies]\nserde.workspace = true\n",
            &mut losses,
        );

        let entry = losses
            .iter()
            .find(|entry| entry.location.record == Some(2))
            .expect("the dotted-key line is declared at its own line number");
        assert_eq!(
            entry.kind,
            LossKind::UnmappedColumn,
            "an unread manifest line takes the borrowed kind the crate docs argue for"
        );
        assert!(
            entry.detail.contains("narrow reader"),
            "the detail is the only thing separating this entry from a genuine unmapped column, \
             because the kind alone cannot: {}",
            entry.detail
        );

        assert_eq!(
            ProjectAdapter::new(ScanOptions::new("demo"))
                .manifest()
                .declared_loss_kinds
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                LossKind::UnmappedColumn,
                LossKind::ProvenanceUnavailable,
                LossKind::ContentUninterpreted,
            ],
            "a fourth declared kind would owe the reader its own argument in the crate docs"
        );
    }

    #[test]
    fn an_inline_table_version_the_reader_cannot_extract_reports_a_gap_not_a_guess() {
        let (_, requirement, gap) =
            parse_cargo_dep_line("odd = { version-table = 1, version! = 2 }").unwrap();
        assert!(requirement.is_none());
        assert!(gap.unwrap().contains("cannot extract"));
    }

    #[test]
    fn a_workspace_dependency_without_a_version_is_neither_pinned_nor_unpinned() {
        let (name, requirement, gap) =
            parse_cargo_dep_line("bioprism-ids = { workspace = true }").unwrap();
        assert_eq!(name, "bioprism-ids");
        assert!(requirement.is_none());
        assert!(gap.is_none());
    }

    #[test]
    fn the_package_json_pin_definition_is_exact_numeric_semver_only() {
        assert!(exact_semver("1.2.3"));
        assert!(!exact_semver("^1.2.3"));
        assert!(!exact_semver("1.2"));
        assert!(!exact_semver("1.2.3-beta.1"));
    }

    #[test]
    fn a_pep508_requirement_splits_into_name_and_requirement_and_pins_on_double_equals() {
        let loose = pep508_record("pyproject.toml", "requests>=2.0");
        assert_eq!(loose.name, "requests");
        assert_eq!(loose.requirement.as_deref(), Some(">=2.0"));
        assert!(!loose.pinned);
        let pinned = pep508_record("pyproject.toml", "numpy==1.26.4");
        assert!(pinned.pinned);
        let bare = pep508_record("pyproject.toml", "tomli");
        assert!(bare.requirement.is_none());
    }

    #[test]
    fn a_final_line_without_a_terminator_still_counts_as_a_line() {
        assert_eq!(line_count("a\nb\n"), 2);
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count(""), 0);
    }

    #[test]
    fn an_issue_with_an_undeclared_key_is_refused_not_skimmed() {
        let document = serde_json::json!([{ "id": "I-1", "title": "t", "component": ["src"] }]);
        let error = Issue::parse_array(&document).unwrap_err();
        assert!(error.to_string().contains("component"));
    }

    #[test]
    fn duplicate_issue_ids_are_refused_because_ids_become_variable_names() {
        let document = serde_json::json!([
            { "id": "I-1", "title": "a" },
            { "id": "I-1", "title": "b" }
        ]);
        assert!(Issue::parse_array(&document).is_err());
    }
}
