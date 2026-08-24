//! Issue repair planning and acceptance verification over a scanned project world.
//!
//! `bioprism-project` can compile the minimal declared-evidence region for working one issue. This
//! crate closes the loop: from that region it produces a typed, checkable **repair plan**, and
//! later it **verifies** a claimed repair against the plan's own declared criteria — three-valued,
//! staleness-aware, and without ever claiming the issue is fixed.
//!
//! # The three commitments this crate is built on
//!
//! **Three-valued acceptance.** A criterion is `Met`, `Unmet`, or `NotEvaluable`, and the third is
//! never folded into either of the others. It is `bioprism_domain::Predicate`'s existing strong
//! three-valued evaluation, reused rather than reimplemented: the same `Obstruction` that tells a
//! rule oracle a check did not run tells a reader here which criterion could not be checked and
//! why. There is no second predicate language in this crate, and there is no `is_met_or_default`.
//!
//! **A plan is bound to its evidence.** [`RepairPlan`] carries an [`EvidenceBinding`] naming the
//! world, its digest, the compiled region's fact ids and the query digest. [`verify()`] checks that
//! binding *before* evaluating anything, and returns [`AcceptanceReport::Stale`] rather than a
//! verdict when it does not hold. Because any edit changes a project world's id, checking a
//! repaired tree needs [`verify_successor`] and a [`Succession`] — a named person's assertion that
//! the new world is the repaired successor of the old one, recorded verbatim and never verified.
//!
//! **Verification never claims the issue is resolved.** It reports which declared criteria held.
//! Every report carries the plan's limitations verbatim plus its own, including the sentence that
//! meeting every criterion is not proof of anything about the issue.
//!
//! # Blueprint
//!
//! **Nothing is cited, and the source carries no module id to be counted.** Repair planning over a
//! scanned software project is beyond the BioPRISM blueprint's scope, exactly as `bioprism-domain`
//! and `bioprism-project` are, and both of those crates say so rather than stretching a module id.
//! The nearest neighbour in the blueprint is section 39's **staleness and recomputation** module,
//! implemented by `bioprism-tokens`; this crate reaches the same conclusion about never silently
//! reusing something bound to a world that moved, and [`verify()`]'s documentation says so as prior
//! art rather than as a coverage claim. The blueprint itself is not readable from this worktree, so
//! no id here could have been checked against its text, and citing one unread would inflate
//! coverage rather than describe it.
//!
//! That neighbour is named by its title and never in dotted `NN.MM` form, which is the discipline
//! and not a stylistic preference. `tools/coverage.sh` counts a module as covered when its token
//! appears *anywhere* under `crates/`, and `tools/status.sh` derives the README's Blueprint column
//! the same way, so a dotted id written into this file — even inside a sentence disclaiming it —
//! would make the generated table attribute a blueprint section to a crate whose first sentence
//! here says it cites nothing. A prose disclaimer is not a mechanism; the absence of the token is.
//! `bioprism-tokens` holds the id, and `tests/citations.rs` reproduces the coverage script's own
//! token rule over this crate's files so the claim is checked rather than asserted.
//!
//! # Not implemented, deliberately
//!
//! * **No editing.** Nothing in this crate writes a source file, applies a patch, or suggests a
//!   diff. It plans and it checks.
//! * **No execution.** Nothing is built, run or tested. A criterion about tests is a claim about
//!   the *scan* — `bioprism-project` counts `#[test]` substrings — and a counted test is not a
//!   passing test. A plan whose criteria are all met may sit on a tree that does not compile.
//! * **No semantic understanding of the issue.** The generator never reads the title or body as
//!   language. The goal is copied verbatim precisely because nothing here can restate it.
//! * **Derived criteria are proxies for what the pack could see**, not for what the issue means.
//!   The gap is on every generated plan's limitations, and it is the plan author's to close.
//! * **No obligation is ever derived.** Whether a change is admissible to make is a judgement
//!   about process, and the scan sees none of it. A plan with no obligations has declared no
//!   prerequisites — [`Admissibility::Undeclared`] — which is not a claim that none are needed.
//! * **No multi-issue interaction.** A plan is about one issue. Two plans over one tree do not
//!   know about each other, and nothing detects that satisfying one would break the other.
//! * **No cost, effort, ordering or risk estimate.** This is not a work plan and proposes no steps.
//! * **No recompiled region at verification time.** Criteria are evaluated against every fact in
//!   the world, so verification applies no budget and is not evidence about what a region compiler
//!   would have delivered. The reason is argued in [`verify()`]'s documentation: blinding the checker
//!   to the world would make "the compiler judged this irrelevant" and "the variable is gone"
//!   arrive as the same unevaluable status.
//! * **No `serde` derive.** The documents are hand-emitted and hand-read because
//!   `bioprism_domain::Predicate`'s canonical form is defined by a hand-written strict parser with
//!   no `Serialize` impl. `bioprism-repair` therefore declares no dependency on `serde` itself —
//!   declaring one it does not use would be the same kind of overstatement as an uncited claim.

pub mod generate;
pub mod plan;
pub mod predicate_json;
pub mod verify;

pub use generate::{plan_for_issue, DeclaredItem, PlanOptions, REGION_EVIDENCE_REMOVED};
pub use plan::{
    AcceptanceCriterion, EvidenceBinding, Falsifier, Obligation, Origin, RepairPlan,
    RepairPlanDraft, CRITERIA_ARE_NOT_PROOF, PLAN_SCHEMA_VERSION,
};
pub use predicate_json::{predicate_from_json, predicate_to_json};
pub use verify::{
    verify, verify_successor, world_value_map, AcceptanceReport, Admissibility, ItemKind,
    ItemOutcome, ItemStatus, Outcome, Succession, REPORT_SCHEMA_VERSION,
};

/// Typed failures.
///
/// Every variant names the specific defect. "Invalid plan" would send an author looking for the
/// wrong problem, and an author who cannot find the problem edits until the error stops, which is
/// how a plan ends up shaped by a parser rather than by a decision.
///
/// Note what is *not* here: verification does not error. A criterion that cannot be evaluated
/// produces [`ItemStatus::NotEvaluable`] on the report, and a world that does not match the plan's
/// binding produces [`AcceptanceReport::Stale`]. Both are findings, and a finding routed through an
/// error type is a finding a caller can discard with `?`.
#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    #[error("repair document: {0}")]
    Document(String),
    #[error("predicate: {0}")]
    Predicate(#[from] bioprism_domain::DomainError),
    #[error("canonical bytes: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error(
        "plan for issue {issue:?} declares no falsifier; a plan that cannot be shown to be the \
         wrong plan is not a plan, only a list of ways to succeed"
    )]
    NoFalsifier { issue: String },
    #[error(
        "plan for issue {issue:?} declares no acceptance criterion; a plan that declares nothing \
         to check would verify as met against any world"
    )]
    NoCriterion { issue: String },
    #[error(
        "two items in the plan are named {name:?}; the acceptance report is one list of named \
         statuses and a duplicate makes it impossible to say which item could not run"
    )]
    DuplicateItemName { name: String },
    #[error(
        "the plan does not carry the mandatory limitation stating that meeting every criterion is \
         not proof the issue is resolved"
    )]
    MissingMandatoryLimitation,
    #[error("{what} is empty")]
    EmptyField { what: String },
    #[error("evidence binding: {0}")]
    RegionFactIds(String),
    #[error(
        "the plan document declares plan_id {declared:?} but its body hashes to {derived:?}; the \
         id is content-derived, so a mismatch means the body was edited after the id was minted"
    )]
    PlanIdMismatch { declared: String, derived: String },
    #[error(
        "no fact provides {variable:?}, so the world carries no record of issue {issue_id:?}; \
         assemble the world with that issue before planning against it"
    )]
    UnknownIssue { issue_id: String, variable: String },
    #[error("the issue record is not the shape the assembler emits: {0}")]
    MalformedIssueFact(String),
    #[error(
        "the region certificate names fact {fact_id:?}, which the world does not contain; the \
         certificate was not compiled from this world"
    )]
    RegionFactUnknown { fact_id: String },
    #[error(
        "the region certificate is about {found:?}, not {expected:?}; a plan bound to a region \
         compiled from something else is bound to nothing"
    )]
    RegionWorldMismatch { expected: String, found: String },
    #[error("predicate cannot be written to its own wire form: {0}")]
    UnrepresentablePredicate(String),
}
