//! Worked recipes for a reader who arrived with a goal rather than a crate name.
//!
//! Implements blueprint section 19, Reference Examples. `bioprism-examples` already implements the
//! *runnable* half of 19 — a vertical slice that executes, asserts the property it claims, and
//! registers the claims nothing exercises. This crate is the other half, the eighteen §19 modules
//! nothing had covered: the layer that answers "I have a cohort and a decision to make", "I need to
//! know whether my split leaks", "I want to compare two context strategies honestly".
//!
//! ```
//! use bioprism_cookbook::{standard_cookbook, CookbookReport};
//!
//! let cookbook = standard_cookbook()?;
//! let report = CookbookReport::of(&cookbook)?;
//! assert!(report.digest_is_intact());
//! # Ok::<(), bioprism_cookbook::CookbookError>(())
//! ```
//!
//! # The one idea
//!
//! A [`Recipe`] is a value, and it cannot be vacuous. Goal, ordered steps naming real crates and
//! real entry points, the claim it demonstrates, at least one checkable property, and the thing
//! that is easy to get wrong. The fields are private and the only constructor is
//! [`RecipeDraft::seal`], which refuses rather than returning a weaker object — so "every recipe in
//! this catalogue is checkable" is a property of the type, not of a review process. Deserialisation
//! goes through the same gate, because the wire form is the form a recipe travels in.
//!
//! Everything else follows. A recipe compiles to a 41.05 task route and goes through
//! `bioprism-docgraph`'s bundle compiler ([`crate::graph`]), so it arrives with a token profile and
//! an omission record and fails rather than truncating. A recipe's references are resolved against
//! the working tree as text ([`crate::verify`]), so naming a deleted function fails a test. And the
//! mistakes are first-class: an [`AntiRecipe`] cannot be constructed without the workspace test
//! that already refutes it.
//!
//! # Reading a report back
//!
//! [`CookbookReport`], and every struct it is made of down to a [`PinnedQuote`], refuses a field it
//! does not declare. The digest is recomputed by re-serialising the *parsed* report, so a key the
//! reader dropped would be outside the seal by construction: the recomputation never sees it, the
//! claimed digest still agrees, and a report carrying content nobody hashed reads as intact.
//!
//! # What this crate does not do, stated plainly
//!
//! **No recipe runs.** Nothing here compiles a context, generates a world or executes a pipeline. A
//! recipe *describes* an order of operations and *verifies its references*; it never produces the
//! artefact it talks about. A green cookbook therefore proves exactly this much: every crate, entry
//! point and enforcing test a recipe names exists, every quotation it attributes to another file is
//! still there, and every documentation route compiles inside its declared budget. It does **not**
//! prove that a recipe produces the result it claims — the test named in each
//! [`Check::EnforcedByTest`] proves that, and it lives in the crate that owns the behaviour.
//!
//! The reason is structural rather than lazy. `bioprism-examples` owns execution; a second
//! executing catalogue would be a second answer to "does this still work" and the two would drift.
//! And a cookbook naming entry points in fifteen crates could not be *built* while any of them was
//! mid-edit if it linked them — which is exactly when a reader most needs to know what the platform
//! can do.
//!
//! Also absent: no CLI (`bioprism-cli` owns binaries), no rendering beyond plain text, no
//! filesystem walk outside [`crate::verify`], and no second documentation graph — [`crate::graph`]
//! populates `bioprism-docgraph`'s [`DocGraph`](bioprism_docgraph::DocGraph) rather than defining
//! one.
//!
//! # Where section 19 under-specifies, and the measurement behind that claim
//!
//! Section 41 is 72.6% shared scaffolding and could be discharged with one implementation. Section
//! 19 is not: across its 22 modules, only **5.22%** of non-blank line occurrences appear in all 22,
//! **8.47%** appear in 15 or more, and **21.44%** are markdown furniture (front matter, headings,
//! code fences) — three numbers from three definitions, all reported in [`crate::catalog`] with the
//! method and the sensitivity check. Mean pairwise line-set Jaccard between modules is 0.036.
//!
//! §19 therefore hands down almost no shared contract. Nearly every decision in this crate is its
//! own, and each one is defended in the module that makes it rather than attributed to the
//! blueprint:
//!
//! - **What a recipe is made of.** §19's modules are worked examples of *artifacts* — a decision
//!   cell, an oracle, a result bundle. Nothing in them says what a reference example must carry.
//!   The five mandatory parts are this crate's, argued in [`crate::recipe`].
//! - **That the mistakes are in scope.** §19 has no anti-example. [`crate::antirecipe`] argues why
//!   a catalogue of successes is the failure mode of every cookbook ever written.
//! - **That a recipe is a route.** 41.05 calls a task-shaped documentation slice a task route;
//!   reading a recipe as one is the choice that let this crate reuse the bundle compiler instead of
//!   growing a delivery mechanism.
//! - **How much of §19 is out of reach.** Six §19 subjects have no implementation to point at.
//!   They are listed with their obstacles in [`report::unwritten_recipes`], not omitted.

pub mod antirecipe;
pub mod book;
pub mod catalog;
pub mod error;
pub mod graph;
pub mod quotes;
pub mod recipe;
pub mod report;
pub mod verify;

pub use antirecipe::AntiRecipe;
pub use book::{standard_cookbook, Cookbook};
pub use catalog::ROUTE_BUDGET;
pub use error::{CookbookError, RouteError, WorkspaceError};
pub use graph::{
    anti_recipe_route, compile_anti_recipe_route, compile_recipe_route, cookbook_doc_graph,
    every_omission_supports_sufficiency, omissions_by_influence, recipe_route, HOUSE_RULES,
};
pub use quotes::PinnedQuote;
pub use recipe::{
    Check, CheckableProperty, Claim, CrateName, EntryPoint, Pitfall, Recipe, RecipeDraft, RecipeId,
    Step, WorkspaceTest,
};
pub use report::{unwritten_recipes, CookbookReport, RecipeSummary, UnwrittenRecipe};
pub use verify::{
    verify_cookbook, QuoteStatus, ReferenceCheck, ReferenceStatus, TestCheck, TestStatus,
    VerificationReport, Workspace,
};
