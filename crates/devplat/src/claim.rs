//! A tutorial that names an API is a claim about the workspace.
//!
//! `bioprism-cookbook` established the pattern: a recipe's crate names and entry points are
//! resolved against the working tree as text, so a rename turns the recipe suite red. Every claim
//! it can hold is an in-tree claim, because every recipe it holds is about this workspace.
//!
//! The developer-platform section is not like that. Its remaining modules describe a Python
//! package, a TypeScript package, a workflow file and an HTTP service, and a quickstart for any of
//! them names APIs that no test in this repository can look for. The failure mode is specific and
//! worth naming: a claim nobody *can* check reads exactly like a claim nobody *has* checked, and a
//! catalogue that stores both as "unresolved" reports a permanent condition as a to-do.
//!
//! [`ApiClaim`] therefore has three evidence states rather than two, and the third is not a
//! weaker version of the others — [`Evidence::OutsideTree`] is *terminal*. No amount of work in
//! this repository moves a claim out of it. That is why it carries a reason and why
//! [`ApiClaim::is_falsifiable_here`] is false for it: a reader deciding whether a document is
//! trustworthy needs to know the difference between "we checked" and "we could never check".
//!
//! # The gate
//!
//! Evidence and surface are checked against each other at construction ([`ApiClaimDraft::seal`]).
//! A Python API cannot carry in-tree evidence, and a crate in this workspace cannot be excused as
//! out of tree. The consequence is small and load bearing: [`Evidence::ResolvedInTree`] is a
//! *true* statement about a file that exists, everywhere it appears.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::ClaimError;
use crate::surface::{Locale, Surface};

/// A name a document tells a reader to use: `prism.compiler.mine`, `bioprism_devplat::render`.
///
/// Deliberately not [`bioprism_cookbook::EntryPoint`], which validates Rust path syntax. Most of
/// the names in scope here are not Rust paths — `POST /v1/runs`, `prism-action@v1`,
/// `prism.traces.import_path` — and running them through a Rust-identifier validator would reject
/// the majority of the corpus as malformed when it is merely foreign.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApiName(String);

impl ApiName {
    pub fn parse(value: impl Into<String>) -> Result<Self, ClaimError> {
        let value: String = value.into();
        if value.trim().is_empty() {
            return Err(ClaimError::UnnamedApi);
        }
        Ok(ApiName(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ApiName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What is known about whether the named API is really there.
///
/// Three states, and the asymmetry between them is the content of this module.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "evidence", rename_all = "snake_case")]
pub enum Evidence {
    /// Found, in a named file of this working tree. Only an in-repository surface may say this.
    ResolvedInTree { file: String },
    /// Looked for in this working tree and not found. This refutes the document.
    ///
    /// A distinct state from [`Evidence::OutsideTree`] because it is *actionable*: either the
    /// document is stale or the code was deleted, and someone can fix it today.
    AbsentFromTree,
    /// Cannot be checked from this repository, now or ever, and here is why.
    OutsideTree { reason: String },
}

impl Evidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Evidence::ResolvedInTree { .. } => "resolved in tree",
            Evidence::AbsentFromTree => "absent from tree",
            Evidence::OutsideTree { .. } => "outside tree",
        }
    }

    /// Whether this evidence state supports the document's claim.
    pub fn supports_the_document(&self) -> bool {
        matches!(self, Evidence::ResolvedInTree { .. })
    }

    /// Whether this evidence state contradicts the document's claim.
    ///
    /// Note that [`Evidence::OutsideTree`] is neither supporting nor contradicting. A three-valued
    /// answer is the honest one, and collapsing it to a boolean is the bug this module exists to
    /// prevent.
    pub fn refutes_the_document(&self) -> bool {
        matches!(self, Evidence::AbsentFromTree)
    }
}

/// One name a document uses, the surface it belongs to, and what is known about it.
///
/// Fields are private; [`ApiClaimDraft::seal`] is the only constructor, and deserialisation goes
/// through it too, because the wire form is the form a claim travels in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ApiClaimWire")]
pub struct ApiClaim {
    api: ApiName,
    surface: Surface,
    evidence: Evidence,
}

impl ApiClaim {
    /// Start describing a claim. Nothing is checked until [`ApiClaimDraft::seal`].
    pub fn about(api: ApiName, surface: Surface) -> ApiClaimDraft {
        ApiClaimDraft {
            api,
            surface,
            evidence: None,
        }
    }

    pub fn api(&self) -> &ApiName {
        &self.api
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// Whether a test in this workspace could disagree with this claim.
    pub fn is_falsifiable_here(&self) -> bool {
        self.surface.is_falsifiable_here()
    }

    /// Whether this claim currently contradicts the working tree.
    pub fn is_refuted(&self) -> bool {
        self.evidence.refutes_the_document()
    }

    /// A line for a report: what was claimed, where it lives, what was found.
    pub fn describe(&self) -> String {
        format!(
            "{} on {}: {}",
            self.api,
            self.surface.describe(),
            self.evidence.as_str()
        )
    }
}

/// An unchecked claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiClaimDraft {
    api: ApiName,
    surface: Surface,
    evidence: Option<Evidence>,
}

impl ApiClaimDraft {
    /// Record that the name was found in a file of this working tree.
    pub fn resolved_in(mut self, file: impl Into<String>) -> Self {
        self.evidence = Some(Evidence::ResolvedInTree { file: file.into() });
        self
    }

    /// Record that the name was looked for here and is not present.
    pub fn absent(mut self) -> Self {
        self.evidence = Some(Evidence::AbsentFromTree);
        self
    }

    /// Record that the name belongs to an artifact this repository does not contain.
    pub fn outside(mut self, reason: impl Into<String>) -> Self {
        self.evidence = Some(Evidence::OutsideTree {
            reason: reason.into(),
        });
        self
    }

    /// Check evidence against surface, and refuse rather than weaken.
    pub fn seal(self) -> Result<ApiClaim, ClaimError> {
        let evidence = match self.evidence {
            Some(evidence) => evidence,
            None => {
                return Err(ClaimError::UnverifiableWithoutReason {
                    api: self.api.as_str().to_string(),
                })
            }
        };
        let locale = self.surface.locale();
        match (&evidence, locale) {
            (Evidence::ResolvedInTree { file }, Locale::InRepository) if file.trim().is_empty() => {
                Err(ClaimError::ResolvedWithoutFile {
                    api: self.api.as_str().to_string(),
                })
            }
            (Evidence::ResolvedInTree { .. }, Locale::OutsideRepository)
            | (Evidence::AbsentFromTree, Locale::OutsideRepository) => {
                Err(ClaimError::ForeignSurfaceClaimsInTreeEvidence {
                    api: self.api.as_str().to_string(),
                    kind: self.surface.kind().as_str(),
                    artifact: self.surface.artifact().to_string(),
                })
            }
            (Evidence::OutsideTree { .. }, Locale::InRepository) => {
                Err(ClaimError::InTreeSurfaceClaimsForeignEvidence {
                    api: self.api.as_str().to_string(),
                    artifact: self.surface.artifact().to_string(),
                })
            }
            (Evidence::OutsideTree { reason }, Locale::OutsideRepository)
                if reason.trim().is_empty() =>
            {
                Err(ClaimError::UnverifiableWithoutReason {
                    api: self.api.as_str().to_string(),
                })
            }
            _ => Ok(ApiClaim {
                api: self.api,
                surface: self.surface,
                evidence,
            }),
        }
    }
}

#[derive(Deserialize)]
struct ApiClaimWire {
    api: ApiName,
    surface: Surface,
    evidence: Evidence,
}

impl TryFrom<ApiClaimWire> for ApiClaim {
    type Error = ClaimError;

    fn try_from(wire: ApiClaimWire) -> Result<Self, Self::Error> {
        let draft = ApiClaim::about(wire.api, wire.surface);
        match wire.evidence {
            Evidence::ResolvedInTree { file } => draft.resolved_in(file),
            Evidence::AbsentFromTree => draft.absent(),
            Evidence::OutsideTree { reason } => draft.outside(reason),
        }
        .seal()
    }
}
