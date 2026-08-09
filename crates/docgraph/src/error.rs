//! Typed failures for graph construction and bundle compilation.
//!
//! Two enums, because the two failures answer different questions. [`DocGraphError`] means the
//! graph itself is not well formed — an identifier that cannot be an identifier, an edge type
//! outside the 41.03 vocabulary. [`BundleError`] means the graph is fine but the request cannot
//! be honoured: 41.06 requires that routes "remain valid under their declared budget or fail
//! explicitly", and an explicit failure has to be a distinct type from a malformed input or the
//! caller cannot tell a broken corpus from a budget that is simply too small.
//!
//! Deliberately absent: there is no `DocGraphError::LintFailed`. Lint findings are data
//! ([`crate::lint::LintFinding`]), not errors, because 41.11 asks for a *report* over a possibly
//! broken corpus. A linter that returned `Err` on the first dangling edge could not produce the
//! report the module specifies.

use crate::registry::ModuleId;
use crate::route::RouteId;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DocGraphError {
    /// 41.03: "unknown edge types fail validation". Not a warning, not a coercion to `related`.
    #[error("unknown edge type `{0}`; the 41.03 vocabulary is closed")]
    UnknownEdgeType(String),

    #[error("malformed module id `{0}`: {reason}", reason = .1)]
    MalformedModuleId(String, &'static str),

    /// 41.02: "module IDs are unique".
    #[error("module id `{0}` is already registered")]
    DuplicateModuleId(ModuleId),

    #[error("module `{0}` is not in the registry")]
    UnknownModule(ModuleId),

    #[error("front matter for `{path}` is malformed: {reason}")]
    MalformedFrontMatter { path: String, reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundleError {
    /// The 41.06 explicit failure. Reported instead of dropping a mandatory module, because a
    /// truncated mandatory set is indistinguishable at the point of use from a complete one.
    #[error(
        "route `{route}` cannot close its mandatory set within budget: {mandatory_cost} estimated \
         tokens required, {budget} allowed (short by {shortfall})"
    )]
    MandatorySetExceedsBudget {
        route: RouteId,
        mandatory_cost: u32,
        budget: u32,
        shortfall: u32,
    },

    /// 41.11: "every route node exists".
    #[error("route `{route}` names module `{module}`, which is not in the registry")]
    RouteReferencesMissingModule { route: RouteId, module: ModuleId },

    /// 41.07: "no traversal crosses denied policy labels". A mandatory module behind a denied
    /// label is not a smaller bundle, it is an impossible one.
    #[error("route `{route}` requires module `{module}`, denied by policy label `{label}`")]
    MandatoryModuleDeniedByPolicy {
        route: RouteId,
        module: ModuleId,
        label: String,
    },

    /// The [`crate::vocabulary::CompanionRequirement::SourceWithTarget`] obligation of
    /// `supersedes`, raised when the successor is not in the registry at all.
    #[error("module `{superseded}` is superseded by `{successor}`, which is not in the registry")]
    SuccessorMissing {
        superseded: ModuleId,
        successor: ModuleId,
    },

    #[error(transparent)]
    Graph(#[from] DocGraphError),
}
