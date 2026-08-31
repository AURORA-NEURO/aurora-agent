//! Checking that the cookbook is not lying, by reading the workspace as text.
//!
//! A cookbook that references a deleted function is worse than no cookbook: it costs a reader the
//! time to follow the reference *and* the confidence they had in everything else in the book. So
//! every crate, entry point, enforcing test and quotation this crate names is resolved against the
//! working tree, and a name that no longer resolves fails a test.
//!
//! # Text, not linkage
//!
//! Nothing here is a dependency. The workspace's `Cargo.toml` is read as text to learn which
//! packages exist, each package's `src/lib.rs` is read as text to learn what it exports, and each
//! named test file is read as text to confirm the function is still in it. That is deliberate and
//! it is what makes the check possible at all: `bioprism-cookbook` names entry points in fourteen
//! crates, and depending on fourteen crates would mean this crate cannot build while any of them
//! is mid-edit — which is exactly when a reader most wants to know what the platform can do.
//! `bioprism-bioworlds` reads `bioprism-examples` the same way and for the same reason.
//!
//! # What the parser is, and what it is not
//!
//! [`exported_items`] is a line-oriented reader of `pub` item declarations and `pub use`
//! statements, not a Rust parser. It understands `pub use a::b::{C, D as E};` across line breaks,
//! `pub mod`, and the seven `pub` item keywords. It does not understand `cfg` attributes, macro-
//! generated items, glob re-exports, or `pub use` inside a module body. Each of those is a way
//! this check can produce a **false negative** — reporting a name as absent when it is reachable —
//! and none of them is a way it can produce a false *positive*, because a name it reports as
//! present was written literally in the file.
//!
//! That asymmetry is the design. A false negative is a failing test somebody investigates; a false
//! positive is a cookbook that says a deleted function still exists. Where the two trade off, this
//! module takes the noisy one.
//!
//! # Not implemented
//!
//! No check that a named entry point has the *signature* the recipe implies, and no check that the
//! steps are in a runnable order. Both would require compiling against the crate, which is the
//! dependency this module exists to avoid. The consequence is stated in [`crate::recipe`]: a green
//! verification means the names resolve, not that the recipe works.

use crate::antirecipe::AntiRecipe;
use crate::error::WorkspaceError;
use crate::quotes::PinnedQuote;
use crate::recipe::{CrateName, EntryPoint, Recipe, WorkspaceTest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The repository root, derived at compile time from this crate's manifest directory.
///
/// Compile-time rather than a runtime search for a `Cargo.toml` with a `[workspace]` table: a
/// search would silently succeed against a *different* checkout if one happened to be an ancestor,
/// and the whole value of this module is that it checks the tree it was built from.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The workspace's packages, as read from `Cargo.toml` files.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    /// package name → directory under the workspace root, forward-slashed.
    packages: BTreeMap<String, String>,
}

impl Workspace {
    /// Read the member list and each member's package name.
    ///
    /// The directory is not assumed to equal the package name. `crates/fiber` holds
    /// `bioprism-fiber`, and a check that inferred one from the other would pass for a crate whose
    /// directory was renamed and fail for one whose package was.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let root = root.into();
        let manifest_path = root.join("Cargo.toml");
        let manifest = read_to_string(&manifest_path)?;
        let members = members_of(&manifest, &manifest_path)?;
        let mut packages = BTreeMap::new();
        for member in members {
            let member_manifest = root.join(&member).join("Cargo.toml");
            let Ok(text) = read_to_string(&member_manifest) else {
                continue;
            };
            let Some(name) = package_name_of(&text) else {
                return Err(WorkspaceError::NoPackageName {
                    path: member_manifest.display().to_string(),
                });
            };
            packages.insert(name, member);
        }
        Ok(Workspace { root, packages })
    }

    /// Open the workspace this crate was compiled inside.
    pub fn here() -> Result<Self, WorkspaceError> {
        Workspace::open(workspace_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn contains_package(&self, name: &CrateName) -> bool {
        self.packages.contains_key(name.as_str())
    }

    /// The member directory holding a package, forward-slashed and relative to the root.
    pub fn directory_of(&self, name: &CrateName) -> Option<&str> {
        self.packages.get(name.as_str()).map(String::as_str)
    }

    /// Read a workspace-relative file.
    pub fn read(&self, relative: &str) -> Result<String, WorkspaceError> {
        read_to_string(&self.root.join(relative))
    }

    fn read_module_file(&self, directory: &str, path: &[&str]) -> Result<String, WorkspaceError> {
        let mut candidate = format!("{directory}/src");
        for segment in path {
            candidate.push('/');
            candidate.push_str(segment);
        }
        if path.is_empty() {
            return self.read(&format!("{candidate}/lib.rs"));
        }
        match self.read(&format!("{candidate}.rs")) {
            Ok(text) => Ok(text),
            Err(_) => self.read(&format!("{candidate}/mod.rs")),
        }
    }

    /// Resolve one entry point, all the way down through however many module levels it names.
    pub fn resolve(&self, entry: &EntryPoint) -> ReferenceStatus {
        let owner = entry.crate_name();
        let Some(directory) = self.directory_of(&owner) else {
            return ReferenceStatus::CrateNotInWorkspace {
                krate: owner.to_string(),
            };
        };
        let segments = entry.item_path();
        let mut module_path: Vec<&str> = Vec::new();
        for (index, segment) in segments.iter().enumerate() {
            let source = match self.read_module_file(directory, &module_path) {
                Ok(text) => text,
                Err(_) => {
                    return ReferenceStatus::SourceUnreadable {
                        path: module_source_path(directory, &module_path),
                    }
                }
            };
            let exports = exported_items(&source);
            if !exports.contains(*segment) {
                let from = module_source_path(directory, &module_path);
                return if index + 1 == segments.len() {
                    ReferenceStatus::ItemNotExported {
                        item: (*segment).to_string(),
                        from,
                    }
                } else {
                    ReferenceStatus::ModuleNotExported {
                        module: (*segment).to_string(),
                        from,
                    }
                };
            }
            if index + 1 < segments.len() {
                module_path.push(segment);
            }
        }
        ReferenceStatus::Present
    }

    /// Resolve one named test.
    pub fn resolve_test(&self, test: &WorkspaceTest) -> TestStatus {
        let Some(directory) = self.directory_of(&test.krate) else {
            return TestStatus::CrateNotInWorkspace {
                krate: test.krate.to_string(),
            };
        };
        let prefix = format!("{directory}/");
        if !test.path.starts_with(&prefix) {
            return TestStatus::PathIsNotInsideTheNamedCrate {
                expected_prefix: prefix,
            };
        }
        let Ok(source) = self.read(&test.path) else {
            return TestStatus::FileMissing;
        };
        if test_function_present(&source, &test.name) {
            TestStatus::Present
        } else {
            TestStatus::FunctionMissing
        }
    }

    /// Resolve one pinned quotation.
    pub fn resolve_quote(&self, quote: &PinnedQuote) -> QuoteStatus {
        match self.read(&quote.source) {
            Err(_) => QuoteStatus::SourceUnreadable,
            Ok(source) if quote.still_present_in(&source) => QuoteStatus::Present,
            Ok(_) => QuoteStatus::Reworded,
        }
    }
}

fn module_source_path(directory: &str, path: &[&str]) -> String {
    if path.is_empty() {
        format!("{directory}/src/lib.rs")
    } else {
        format!("{directory}/src/{}.rs", path.join("/"))
    }
}

fn read_to_string(path: &Path) -> Result<String, WorkspaceError> {
    std::fs::read_to_string(path).map_err(|error| WorkspaceError::Unreadable {
        path: path.display().to_string(),
        reason: error.to_string(),
    })
}

/// The `crates/…` entries of a workspace manifest's `members` array.
fn members_of(manifest: &str, path: &Path) -> Result<Vec<String>, WorkspaceError> {
    let Some(start) = manifest.find("members") else {
        return Err(WorkspaceError::NoMembersList {
            path: path.display().to_string(),
        });
    };
    let tail = &manifest[start..];
    let Some(open) = tail.find('[') else {
        return Err(WorkspaceError::NoMembersList {
            path: path.display().to_string(),
        });
    };
    let Some(close) = tail[open..].find(']') else {
        return Err(WorkspaceError::NoMembersList {
            path: path.display().to_string(),
        });
    };
    Ok(tail[open + 1..open + close]
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').trim())
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect())
}

fn package_name_of(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("name = "))
        .map(|value| value.trim().trim_matches('"').to_string())
}

/// Whether a file declares a `#[test]`-style function with this exact name.
///
/// Matched as `fn <name>(` rather than as a substring, so `fn a_budget_check` does not satisfy a
/// reference to `a_budget`. Prefix-anchored on the trimmed line so a call to the function inside
/// another test does not count as its declaration.
pub fn test_function_present(source: &str, name: &str) -> bool {
    let needle = format!("fn {name}(");
    source
        .lines()
        .map(str::trim)
        .any(|line| line.starts_with(&needle) || line.starts_with(&format!("pub {needle}")))
}

/// The names a Rust source file makes public: `pub mod`, `pub` items, and `pub use` leaves.
///
/// See the module docs for what this deliberately does not understand.
pub fn exported_items(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in source.lines().map(str::trim) {
        for keyword in [
            "pub mod ",
            "pub fn ",
            "pub struct ",
            "pub enum ",
            "pub trait ",
            "pub type ",
            "pub const ",
            "pub static ",
        ] {
            if let Some(rest) = line.strip_prefix(keyword) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
        if let Some(rest) = line.strip_prefix("pub unsafe fn ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    for statement in use_statements(source) {
        collect_use_leaves(&statement, &mut names);
    }
    names
}

/// Every `pub use …;` statement in a file, each joined into one line.
fn use_statements(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current: Option<String> = None;
    for line in source.lines().map(str::trim) {
        match &mut current {
            None => {
                if line.starts_with("pub use ") {
                    if let Some(end) = line.find(';') {
                        statements.push(line[..end].to_string());
                    } else {
                        current = Some(line.to_string());
                    }
                }
            }
            Some(buffer) => {
                if let Some(end) = line.find(';') {
                    buffer.push(' ');
                    buffer.push_str(&line[..end]);
                    statements.push(std::mem::take(buffer));
                    current = None;
                } else {
                    buffer.push(' ');
                    buffer.push_str(line);
                }
            }
        }
    }
    statements
}

/// The names a single `pub use` statement introduces.
///
/// `pub use a::b::{C, D as E}` introduces `C` and `E`, not `a`, `b` or `D`. The alias is what the
/// crate actually exports, so taking the pre-`as` name would report a symbol under a name no
/// caller can use.
fn collect_use_leaves(statement: &str, names: &mut BTreeSet<String>) {
    let body = statement.trim_start_matches("pub use ").trim();
    match (body.find('{'), body.rfind('}')) {
        (Some(open), Some(close)) if close > open => {
            for item in body[open + 1..close].split(',') {
                push_leaf(item, names);
            }
        }
        _ => push_leaf(body, names),
    }
}

fn push_leaf(item: &str, names: &mut BTreeSet<String>) {
    let item = item.trim();
    if item.is_empty() || item == "self" || item.ends_with('*') {
        return;
    }
    let leaf = match item.split(" as ").nth(1) {
        Some(alias) => alias.trim(),
        None => item.rsplit("::").next().unwrap_or(item).trim(),
    };
    let name: String = leaf
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if !name.is_empty() {
        names.insert(name);
    }
}

/// Whether a named entry point resolves, and if not, how far it got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ReferenceStatus {
    Present,
    /// No package of that name is a workspace member.
    CrateNotInWorkspace {
        krate: String,
    },
    /// The crate exists but a source file on the path could not be read. Distinct from a missing
    /// symbol: an unreadable file is a fact about the checkout, not about the cookbook.
    SourceUnreadable {
        path: String,
    },
    /// An intermediate path segment is not a public module of its parent.
    ModuleNotExported {
        module: String,
        from: String,
    },
    /// The final segment is not exported by the module that should hold it.
    ItemNotExported {
        item: String,
        from: String,
    },
}

impl ReferenceStatus {
    pub fn is_present(&self) -> bool {
        matches!(self, ReferenceStatus::Present)
    }
}

/// Whether a named enforcing test still exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum TestStatus {
    Present,
    CrateNotInWorkspace {
        krate: String,
    },
    FileMissing,
    /// The file exists and no function of that name is declared in it. The commonest way a
    /// cookbook goes stale, because renaming a test is a refactor nobody thinks twice about.
    FunctionMissing,
    /// The path does not live under the crate it was attributed to. An authoring mistake, and one
    /// worth separating: the test may well exist, just not where the cookbook says it does.
    PathIsNotInsideTheNamedCrate {
        expected_prefix: String,
    },
}

impl TestStatus {
    pub fn is_present(&self) -> bool {
        matches!(self, TestStatus::Present)
    }
}

/// Whether a quotation is still in the file it was taken from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum QuoteStatus {
    Present,
    SourceUnreadable,
    /// The file is there and the quoted text is not. Someone reworded it, and this cookbook is now
    /// attributing to them something they no longer say.
    Reworded,
}

impl QuoteStatus {
    pub fn is_present(&self) -> bool {
        matches!(self, QuoteStatus::Present)
    }
}

/// One resolved reference, with enough context to find where it was named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceCheck {
    /// The recipe or anti-recipe that named it.
    pub named_by: String,
    pub reference: String,
    pub status: ReferenceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCheck {
    pub named_by: String,
    pub test: WorkspaceTest,
    pub status: TestStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteCheck {
    pub quote: PinnedQuote,
    pub status: QuoteStatus,
}

/// Everything the cookbook asserts about the workspace, resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub crates: Vec<ReferenceCheck>,
    pub entry_points: Vec<ReferenceCheck>,
    pub tests: Vec<TestCheck>,
    pub quotes: Vec<QuoteCheck>,
}

impl VerificationReport {
    pub fn is_clean(&self) -> bool {
        self.crates.iter().all(|c| c.status.is_present())
            && self.entry_points.iter().all(|c| c.status.is_present())
            && self.tests.iter().all(|c| c.status.is_present())
            && self.quotes.iter().all(|c| c.status.is_present())
    }

    /// Every failure, rendered one per line, in the order a reader would fix them.
    pub fn defects(&self) -> Vec<String> {
        let mut out = Vec::new();
        for check in self.crates.iter().chain(self.entry_points.iter()) {
            if !check.status.is_present() {
                out.push(format!(
                    "{} names `{}`: {:?}",
                    check.named_by, check.reference, check.status
                ));
            }
        }
        for check in &self.tests {
            if !check.status.is_present() {
                out.push(format!(
                    "{} leans on `{}::{}`: {:?}",
                    check.named_by, check.test.path, check.test.name, check.status
                ));
            }
        }
        for check in &self.quotes {
            if !check.status.is_present() {
                out.push(format!(
                    "quotation from `{}` (`{}`): {:?}",
                    check.quote.source, check.quote.needle, check.status
                ));
            }
        }
        out
    }

    pub fn render(&self) -> String {
        let defects = self.defects();
        let mut out = format!(
            "resolved {} crates, {} entry points, {} tests, {} quotations\n",
            self.crates.len(),
            self.entry_points.len(),
            self.tests.len(),
            self.quotes.len()
        );
        if defects.is_empty() {
            out.push_str("no unresolved references\n");
        } else {
            out.push_str(&format!("{} unresolved:\n", defects.len()));
            for defect in defects {
                out.push_str(&format!("  - {defect}\n"));
            }
        }
        out
    }
}

/// Resolve every reference a set of recipes and anti-recipes makes.
///
/// Quotations are passed in rather than read from [`crate::quotes::all`] so that a caller checking
/// a subset of the catalogue is not forced to check every pin as well.
pub fn verify_cookbook(
    workspace: &Workspace,
    recipes: &[Recipe],
    anti_recipes: &[AntiRecipe],
    quotes: &[PinnedQuote],
) -> VerificationReport {
    let mut crates = Vec::new();
    let mut entry_points = Vec::new();
    let mut tests = Vec::new();

    for recipe in recipes {
        let named_by = recipe.id().to_string();
        for krate in recipe.crates() {
            crates.push(ReferenceCheck {
                named_by: named_by.clone(),
                reference: krate.to_string(),
                status: if workspace.contains_package(&krate) {
                    ReferenceStatus::Present
                } else {
                    ReferenceStatus::CrateNotInWorkspace {
                        krate: krate.to_string(),
                    }
                },
            });
        }
        for entry in recipe.entry_points() {
            entry_points.push(ReferenceCheck {
                named_by: named_by.clone(),
                reference: entry.to_string(),
                status: workspace.resolve(entry),
            });
        }
        for test in recipe.enforcing_tests() {
            tests.push(TestCheck {
                named_by: named_by.clone(),
                test: test.clone(),
                status: workspace.resolve_test(test),
            });
        }
    }

    for anti in anti_recipes {
        let named_by = anti.id().to_string();
        for test in anti.enforced_by() {
            tests.push(TestCheck {
                named_by: named_by.clone(),
                test: test.clone(),
                status: workspace.resolve_test(test),
            });
        }
    }

    let quotes = quotes
        .iter()
        .map(|quote| QuoteCheck {
            status: workspace.resolve_quote(quote),
            quote: quote.clone(),
        })
        .collect();

    VerificationReport {
        crates,
        entry_points,
        tests,
        quotes,
    }
}
