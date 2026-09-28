//! Where a named API actually lives, and whether this repository can be asked about it.
//!
//! The remainder of the developer-platform section is not one kind of thing. Some of it is a
//! Python package, some a TypeScript package, some a workflow file in a consumer's repository,
//! some an HTTP service, some a user interface. Locality belongs to the named artifact, not its
//! language: a Python package, TypeScript client, or composite action can live in this checkout,
//! while a hosted consumer workflow remains outside it.
//!
//! So the first type here is not a document type. It is [`Surface`]: the artifact a name belongs
//! to, and the [`Locale`] of that artifact. Locale is fixed by a checked constructor and serialized
//! with the address, so [`crate::claim::ApiClaim`] cannot be handed in-tree evidence about a
//! declared foreign surface.
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
/// Two values, and no third for "partly". A surface either names an artifact in this working tree
/// that a test can read, or it does not. Generated-client provenance is a relationship between
/// surfaces, not a locale.
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
    /// A crate of this workspace, addressed by its package name.
    RustCrate,
    /// An importable Python distribution: `prism_sdk`, `prism_compiler`.
    PythonPackage,
    /// An npm package consumed from a browser or Node process.
    TypeScriptPackage,
    /// A workflow or composite-action definition evaluated by a CI provider.
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

    /// Compatibility default for old serialized surfaces that did not record their locale.
    fn legacy_locale(self) -> Locale {
        match self {
            SurfaceKind::RustCrate => Locale::InRepository,
            _ => Locale::OutsideRepository,
        }
    }
}

impl fmt::Display for SurfaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named artifact of a given kind: the package, file or process a name lives in.
///
/// Fields are private and construction goes through [`Surface::rust`],
/// [`Surface::in_repository`] or [`Surface::foreign`], so local non-Rust surfaces use a safe
/// repository-relative path, a Rust surface names a crate that [`CrateName`] accepts, and no
/// `Surface` has an empty artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "SurfaceWire")]
pub struct Surface {
    kind: SurfaceKind,
    artifact: String,
    locale: Locale,
}

#[derive(Deserialize)]
struct SurfaceWire {
    kind: SurfaceKind,
    artifact: String,
    #[serde(default)]
    locale: Option<Locale>,
}

impl TryFrom<SurfaceWire> for Surface {
    type Error = SurfaceError;

    fn try_from(wire: SurfaceWire) -> Result<Self, Self::Error> {
        let locale = wire.locale.unwrap_or_else(|| wire.kind.legacy_locale());
        if wire.kind == SurfaceKind::RustCrate && locale != Locale::InRepository {
            return Err(SurfaceError::InvalidArtifact {
                kind: SurfaceKind::RustCrate.as_str(),
            });
        }
        match locale {
            Locale::InRepository if wire.kind == SurfaceKind::RustCrate => {
                let name = CrateName::parse(wire.artifact.clone()).map_err(|_| {
                    SurfaceError::NotAWorkspaceCrate {
                        artifact: wire.artifact.clone(),
                    }
                })?;
                Surface::rust(&name)
            }
            Locale::InRepository => Surface::in_repository(wire.kind, wire.artifact),
            Locale::OutsideRepository => Surface::foreign(wire.kind, wire.artifact),
        }
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
            locale: Locale::InRepository,
        })
    }

    /// A non-Rust artifact held in this checkout, addressed by a normalized relative path.
    ///
    /// This records the artifact boundary; it does not claim the file currently exists. Claims
    /// carry the specific file they were checked against, and [`crate::walkthrough::recheck`]
    /// checks that evidence against the working tree.
    pub fn in_repository(
        kind: SurfaceKind,
        artifact: impl Into<String>,
    ) -> Result<Self, SurfaceError> {
        let artifact = validate_artifact(kind, artifact.into())?;
        if kind == SurfaceKind::RustCrate {
            return Err(SurfaceError::InvalidArtifact {
                kind: SurfaceKind::RustCrate.as_str(),
            });
        }
        if !is_normalized_repository_path(&artifact) {
            return Err(SurfaceError::InvalidRepositoryPath { artifact });
        }
        Ok(Surface {
            kind,
            artifact,
            locale: Locale::InRepository,
        })
    }

    /// Any surface outside this repository.
    ///
    /// Refuses [`SurfaceKind::RustCrate`] except for a valid workspace package, which routes
    /// through [`Surface::rust`] so there is exactly one way to construct that surface.
    pub fn foreign(kind: SurfaceKind, artifact: impl Into<String>) -> Result<Self, SurfaceError> {
        let artifact = validate_artifact(kind, artifact.into())?;
        if kind == SurfaceKind::RustCrate {
            return Surface::rust(&CrateName::parse(artifact).map_err(|_| {
                SurfaceError::UnnamedArtifact {
                    kind: SurfaceKind::RustCrate.as_str(),
                }
            })?);
        }
        Ok(Surface {
            kind,
            artifact,
            locale: Locale::OutsideRepository,
        })
    }

    pub fn kind(&self) -> SurfaceKind {
        self.kind
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn is_falsifiable_here(&self) -> bool {
        self.locale == Locale::InRepository
    }

    /// A one-line address for a report.
    pub fn describe(&self) -> String {
        format!("{} `{}`", self.kind.as_str(), self.artifact)
    }
}

fn validate_artifact(kind: SurfaceKind, artifact: String) -> Result<String, SurfaceError> {
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
    Ok(artifact)
}

pub(crate) fn is_normalized_repository_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
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
    /// The external scope that remains unresolved.
    pub surface: Surface,
    /// Why the in-tree implementation does not close that full scope. Concrete, not "out of scope".
    pub why_not_here: &'static str,
}

/// The developer-platform subjects whose complete blueprint artifact is not in this repository.
///
/// Two of them. This group is part of [`crate::classify::classification`]: three-quarters of the
/// twenty modules are not implemented by this crate, and the evidence distinguishes process,
/// external artifacts and capabilities already held elsewhere.
pub fn foreign_subjects() -> Vec<ForeignSubject> {
    let entries: [(&'static str, SurfaceKind, &'static str, &'static str); 2] = [
        (
            "Python SDK",
            SurfaceKind::PythonPackage,
            "full nine-distribution Python SDK",
            "the full blueprint specifies nine importable distributions, Python 3.12 typing, \
             async-first methods with sync facades, and entry-point discovery. The in-tree \
             dependency-free `python/prism_sdk` integration client covers the HTTP boundary, but \
             it does not supply that complete multi-distribution SDK surface.",
        ),
        (
            "GitHub Action and CI Integration",
            SurfaceKind::GitHubAction,
            "hosted consumer workflow execution",
            "the repository now contains a reusable composite action and exercises it locally in \
             CI. The blueprint's consumer-owned gating policy, hosted runner behavior, and \
             published action revision still depend on a workflow and provider outside this \
             checkout.",
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
