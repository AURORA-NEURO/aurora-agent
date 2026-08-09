//! Typed failures for recipe construction, routing and reference verification.
//!
//! Three enums, because they answer three different questions and a caller has to be able to tell
//! them apart without reading a message string.
//!
//! [`CookbookError`] means an *author* wrote a bad recipe: a recipe with no steps, no claim, or —
//! the one this crate exists to make impossible — no checkable property. These are raised at
//! construction and at deserialisation, so a recipe that lost a mandatory part in transit is a
//! parse failure rather than a silently weaker object.
//!
//! [`RouteError`] means the recipe is well formed but cannot be delivered: its route does not
//! resolve against the documentation graph, or its declared budget will not hold its mandatory
//! set. `bioprism-docgraph` already refuses to truncate in that second case and this crate does
//! not soften it — [`RouteError::Bundle`] is that refusal, forwarded.
//!
//! [`WorkspaceError`] means the *workspace* could not be read to check a reference. It is
//! deliberately distinct from "the reference is absent": a missing `lib.rs` and a missing symbol
//! are different states, and collapsing them would let an unreadable workspace be reported as a
//! cookbook full of dangling references — or, worse, an unreadable workspace be reported as clean.

use bioprism_docgraph::{BundleError, DocGraphError};
use thiserror::Error;

/// An authoring mistake in a recipe or an anti-recipe.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CookbookError {
    /// A recipe with no ordered steps is a title. 19.09 and 19.11 both describe a *worked*
    /// example, and the work is the ordering.
    #[error("recipe `{0}` has no steps; a recipe with no ordered steps is a title")]
    NoSteps(String),

    /// The claim is what the recipe is evidence *for*. Without it a reader cannot tell whether
    /// following the steps proved anything.
    #[error("recipe `{0}` demonstrates no claim")]
    NoClaim(String),

    /// The rule this crate is built around. See [`crate::recipe::Recipe`].
    #[error("recipe `{0}` carries no checkable property; a recipe a reader cannot check is prose")]
    NoCheckableProperty(String),

    /// Every recipe here names the thing that is easy to get wrong. A worked example that shows
    /// only the happy path teaches the happy path.
    #[error("recipe `{0}` names nothing that is easy to get wrong")]
    NoPitfall(String),

    #[error("recipe `{recipe}` has an empty `{field}`")]
    EmptyField {
        recipe: String,
        field: &'static str,
    },

    #[error("malformed recipe id `{0}`: {1}")]
    MalformedId(String, &'static str),

    /// An entry point is a Rust path rooted at a crate: `bioprism_fiber::compile`. A bare item
    /// name cannot be verified, because nothing says which crate to look in.
    #[error("malformed entry point `{0}`: {1}")]
    MalformedEntryPoint(String, &'static str),

    #[error("recipe id `{0}` is registered twice")]
    DuplicateRecipe(String),

    #[error("no recipe with id `{0}`")]
    UnknownRecipe(String),

    #[error("the module id `{0}` this recipe routes through is malformed")]
    MalformedModuleId(String),

    #[error(transparent)]
    Graph(#[from] DocGraphError),
}

/// A recipe that cannot be delivered as a documentation context.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteError {
    /// 41.05: "every route resolves". A recipe naming a documentation module the cookbook graph
    /// does not hold is an authoring defect, reported before any bundle is attempted.
    #[error("recipe `{recipe}` routes through `{module}`, which the cookbook graph does not hold")]
    UnroutableModule { recipe: String, module: String },

    /// Forwarded from `bioprism-docgraph`, unchanged. In particular
    /// [`BundleError::MandatorySetExceedsBudget`] is not caught and retried at a larger budget:
    /// the whole point of the rule is that the failure reaches the caller.
    #[error(transparent)]
    Bundle(#[from] BundleError),

    #[error(transparent)]
    Cookbook(#[from] CookbookError),
}

/// The workspace could not be read. Never conflated with "the reference is not there".
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkspaceError {
    #[error("cannot read `{path}`: {reason}")]
    Unreadable { path: String, reason: String },

    #[error("`{path}` does not declare a `[workspace] members` list")]
    NoMembersList { path: String },

    #[error("`{path}` does not declare a package name")]
    NoPackageName { path: String },
}
