//! Where a named API actually lives, and whether this repository can be asked about it.
//!
//! The remainder of the developer-platform section is not one kind of thing. Some of it is a
//! Python package, some a TypeScript package, some a workflow file in a consumer's repository,
//! some an HTTP service, some a user interface. A crate in this workspace can resolve exactly one
//! of those against the working tree — a Rust crate — and pretending otherwise is how
//! documentation for a thing that does not exist passes a green test suite.
//!
//! So the first type here is not a document type. It is [`Surface`]: the artifact a name belongs
//! to, and the [`Locale`] that follows from its kind. [`Locale`] is derived, never supplied, which
//! is the whole point — an author cannot declare a Python module to be in this repository, so
//! [`crate::claim::ApiClaim`] cannot be handed evidence that it was resolved here.
//!
//! # What this is not
//!
//! Not a registry. `bioprism-sdk` owns plugin registration and capability declaration; nothing
//! here is discovered, loaded, or dispatched to. A [`Surface`] is a *description of an address*,
//! and the only question it answers is whether an address is inside the boundary of this checkout.

use bioprism_cookbook::CrateName;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::SurfaceError;

const MAX_ARTIFACT_BYTES: usize = 4_096;

/// Whether a surface is something this checkout contains.
///
/// Two values, and no third for "partly". A surface either has bytes in this working tree that a
/// test can read, or it does not; "the schema is generated from ours" is a relationship between
/// two surfaces, not a third locale, and modelling it as one would let a generated TypeScript
/// client be reported as verified here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locale {
    /// Bytes in this working tree. A test can open the file and look.
    InRepository,
    /// Bytes somewhere else: another language, another repository, a running process, a browser.
    OutsideRepository,
}

impl Locale {
    pub fn as_str(self) -> &'static str {
        match self {
            Locale::InRepository => "in repository",
            Locale::OutsideRepository => "outside repository",
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The kinds of artifact the developer-platform section asks for.
///
/// Closed, and closed at nine because these are the nine that appear in the section's remaining
/// modules — not because nine is a round number. Adding a tenth means the section asked for
/// something new, which is worth a compile error at every match site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// A crate of this workspace. The only kind whose locale is [`Locale::InRepository`].
    RustCrate,
    /// An importable Python distribution: `prism_sdk`, `prism_compiler`.
    PythonPackage,
    /// An npm package consumed from a browser or Node process.
    TypeScriptPackage,
    /// A workflow or action file evaluated by a CI provider in somebody else's repository.
    GitHubAction,
    /// A request/response surface reached over the network.
    HttpApi,
    /// A push surface: a stream a client subscribes to, or a webhook the platform calls.
    EventStream,
    /// A tool exposed to an agent over the Model Context Protocol.
    McpTool,
    /// A notebook: cells, kernel state, and outputs that are not the source of truth.
    Notebook,
    /// Pixels. A studio, a dashboard, a viewer.
    UserInterface,
}

impl SurfaceKind {
    /// The whole set.
    pub const ALL: [SurfaceKind; 9] = [
        SurfaceKind::RustCrate,
        SurfaceKind::PythonPackage,
        SurfaceKind::TypeScriptPackage,
        SurfaceKind::GitHubAction,
        SurfaceKind::HttpApi,
        SurfaceKind::EventStream,
        SurfaceKind::McpTool,
        SurfaceKind::Notebook,
        SurfaceKind::UserInterface,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SurfaceKind::RustCrate => "rust crate",
            SurfaceKind::PythonPackage => "python package",
            SurfaceKind::TypeScriptPackage => "typescript package",
            SurfaceKind::GitHubAction => "github action",
            SurfaceKind::HttpApi => "http api",
            SurfaceKind::EventStream => "event stream",
            SurfaceKind::McpTool => "mcp tool",
            SurfaceKind::Notebook => "notebook",
            SurfaceKind::UserInterface => "user interface",
        }
    }

    /// The language an author writes this surface in, for a reader deciding whether they can help.
    pub fn language(self) -> &'static str {
        match self {
            SurfaceKind::RustCrate => "Rust",
            SurfaceKind::PythonPackage | SurfaceKind::Notebook => "Python",
            SurfaceKind::TypeScriptPackage => "TypeScript",
            SurfaceKind::GitHubAction => "YAML",
            SurfaceKind::HttpApi | SurfaceKind::EventStream | SurfaceKind::McpTool => {
                "wire protocol, language-independent"
            }
            SurfaceKind::UserInterface => "none: it is an interface, not a source artifact",
        }
    }

    /// Derived, not declared. Only a Rust crate is in this repository.
    pub fn locale(self) -> Locale {
        match self {
            SurfaceKind::RustCrate => Locale::InRepository,
            _ => Locale::OutsideRepository,
        }
    }

    /// Whether a test in this workspace could, in principle, read the artifact and disagree.
    pub fn is_falsifiable_here(self) -> bool {
        self.locale() == Locale::InRepository
    }
}

impl fmt::Display for SurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named artifact of a given kind: the package, file or process a name lives in.
///
/// Fields are private and construction goes through [`Surface::rust`] or [`Surface::foreign`], so
/// a `Surface` whose kind is [`SurfaceKind::RustCrate`] always names a crate that
/// [`CrateName`] accepts, and no `Surface` has an empty artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "SurfaceWire")]
pub struct Surface {
    kind: SurfaceKind,
    artifact: String,
}

#[derive(Deserialize)]
struct SurfaceWire {
    kind: SurfaceKind,
    artifact: String,
}

impl TryFrom<SurfaceWire> for Surface {
    type Error = SurfaceError;

    fn try_from(wire: SurfaceWire) -> Result<Self, Self::Error> {
        Surface::foreign(wire.kind, wire.artifact)
    }
}

impl Surface {
    /// The in-repository surface. The artifact is a workspace package name.
    ///
    /// Reuses `bioprism-cookbook`'s [`CrateName`] rather than defining a second package-name type:
    /// the two crates resolve names against the same working tree, and two spellings of "crate
    /// name" would eventually disagree about a rename.
    pub fn rust(krate: &CrateName) -> Result<Self, SurfaceError> {
        let artifact = krate.as_str().to_string();
        if !artifact.starts_with("bioprism-") {
            return Err(SurfaceError::NotAWorkspaceCrate { artifact });
        }
        Ok(Surface {
            kind: SurfaceKind::RustCrate,
            artifact,
        })
    }

    /// Any surface outside this repository.
    ///
    /// Refuses [`SurfaceKind::RustCrate`] by routing it through the same validation as
    /// [`Surface::rust`], so there is exactly one way to obtain an in-repository surface.
    pub fn foreign(kind: SurfaceKind, artifact: impl Into<String>) -> Result<Self, SurfaceError> {
        let artifact: String = artifact.into();
        if artifact.trim().is_empty() {
            return Err(SurfaceError::UnnamedArtifact {
                kind: kind.as_str(),
            });
        }
        if artifact != artifact.trim()
            || artifact.len() > MAX_ARTIFACT_BYTES
            || artifact.chars().any(char::is_control)
        {
            return Err(SurfaceError::InvalidArtifact {
                kind: kind.as_str(),
            });
        }
        if kind == SurfaceKind::RustCrate {
            return Surface::rust(&CrateName::parse(artifact).map_err(|_| {
                SurfaceError::UnnamedArtifact {
                    kind: SurfaceKind::RustCrate.as_str(),
                }
            })?);
        }
        Ok(Surface { kind, artifact })
    }

    pub fn kind(&self) -> SurfaceKind {
        self.kind
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    /// Derived from the kind. There is no setter.
    pub fn locale(&self) -> Locale {
        self.kind.locale()
    }

    pub fn is_falsifiable_here(&self) -> bool {
        self.kind.is_falsifiable_here()
    }

    /// A one-line address for a report.
    pub fn describe(&self) -> String {
        format!("{} `{}`", self.kind.as_str(), self.artifact)
    }
}

impl fmt::Display for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

/// One developer-platform subject whose artifact is not, and cannot be, in this repository.
///
/// Named by title rather than by module id, on purpose. The workspace's coverage tool matches any
/// `NN.MM` token anywhere under `crates/`, so writing the id of a module this crate did not
/// implement would move the coverage figure without moving the platform. See
/// [`crate::citations`], which turns that rule into a test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignSubject {
    /// The blueprint module's title, as the blueprint spells it.
    pub title: &'static str,
    /// Where its artifact lives.
    pub surface: Surface,
    /// What a Rust crate would have to become in order to hold it. Concrete, not "out of scope".
    pub why_not_here: &'static str,
}

/// The developer-platform subjects that are code-bearing but not Rust and not in this repository.
///
/// Three of them. This is the largest single group in [`crate::classify::classification`] and the
/// reason this crate exists in the shape it does: a section can be two-thirds unimplemented in a
/// Rust workspace without a single line of it being vague.
pub fn foreign_subjects() -> Vec<ForeignSubject> {
    let entries: [(&'static str, SurfaceKind, &'static str, &'static str); 3] = [
        (
            "Python SDK",
            SurfaceKind::PythonPackage,
            "prism_sdk",
            "the module specifies nine importable distributions, Python 3.12 typing, async-first \
             methods with sync facades, and entry-point discovery. None of that has a Rust \
             rendering; a Rust type mirroring it would be a translation nobody imports.",
        ),
        (
            "GitHub Action for Consumer Repositories",
            SurfaceKind::GitHubAction,
            "prism-action",
            "a composite action evaluated by a CI provider in a repository that is not this one. \
             Its correctness is a property of somebody else's workflow file.",
        ),
        (
            "GitHub Action and CI Integration",
            SurfaceKind::GitHubAction,
            ".github/workflows/prism.yml",
            "the module's own example is nine lines of workflow YAML. The gate policy it states \
             is checkable, but the artifact that carries it is a CI configuration, not a crate.",
        ),
    ];
    entries
        .into_iter()
        .map(|(title, kind, artifact, why)| ForeignSubject {
            title,
            surface: Surface::foreign(kind, artifact)
                .expect("static foreign surfaces are well formed"),
            why_not_here: why,
        })
        .collect()
}
