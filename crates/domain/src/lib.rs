//! Domain packs: what carries the compiler to a decision question it was not born knowing.
//!
//! The FIBER pipeline is domain-neutral — facts, factors, scopes, closure, slicing, the
//! temporal cut and the certificate never mention biology — but the workspace ships exactly one
//! wired-in oracle, the split-integrity reference, and `bioprism-worldgen` records the
//! consequence on its `Skeleton`: a world with a genuinely different decision would compile to
//! `valid` with an empty witness list and read as clean rather than as unjudged. "A decision
//! the oracle does not know needs an oracle before it needs a variant."
//!
//! This crate is that oracle, in data. A **domain pack** is a versioned document declaring a
//! decision question: the scope dimensions its worlds use (loaded through
//! [`bioprism_scope::DimensionRegistry`]), the tags its queries should protect, and a **rule
//! oracle** — a deterministic set of violation checks over the compiled value map, each firing
//! a checkable [`bioprism_section::LeakageWitness::DomainCheck`] witness, never a score. The
//! pack plugs into [`bioprism_fiber::compile_with_oracle`]; the default [`bioprism_fiber::compile`]
//! and its parity bytes are untouched.
//!
//! The honesty rules of the reference oracle hold with no exception:
//!
//! * a check whose input is absent **did not run**, and a verdict with an unrun check is
//!   `underdetermined`, never `valid` — unmeasured is not zero;
//! * a variable the pack declares in `require` and the compiler could not deliver abstains the
//!   whole verdict before any check runs;
//! * a proven violation stands even when another check could not run: `invalid` outranks
//!   `underdetermined`.
//!
//! Worked packs for three non-biological domains live in `fixtures/domains/` — trade
//! surveillance, legal privilege review, software supply-chain release review — each compiled
//! end to end in this crate's tests.
//!
//! # Not implemented
//!
//! * **No blueprint section is claimed.** The BioPRISM blueprint states its oracle and packs
//!   biologically; this crate deliberately generalises beyond it and cites no module id rather
//!   than stretching one.
//! * **No relational predicates.** The reference identity check joins two maps key-by-key; the
//!   rule language cannot express that. A world that needs a join precomputes the conflict as
//!   its own variable (the fixtures show the pattern), or the domain implements
//!   [`bioprism_fiber::DecisionOracle`] natively.
//! * **String order is lexicographic**, matching the reference oracle's flagged behaviour, and
//!   is refused for non-strings rather than coerced.
//! * **No oracle selection from the query wire.** No `fiber-query` version carries an oracle
//!   field, so choosing a pack is a caller decision at the API or CLI boundary, recorded in the
//!   verdict's `oracle_kind`.
//! * **Checks see the selected value map only** — the same contract the reference oracle has.
//!   A rule cannot consult scopes, events, tags or omitted facts.

pub mod pack;
pub mod rules;

pub use pack::{DomainPack, DOMAIN_SCHEMA_VERSION};
pub use rules::{Predicate, RuleCheck, RuleOracle};

/// Errors from parsing a pack or rule document. Evaluation itself does not error: a check that
/// cannot run abstains the verdict instead, and that distinction is the point of the crate.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("domain pack: {0}")]
    Pack(String),
    #[error("rule oracle: {0}")]
    Rules(String),
    #[error("scope dimensions: {0}")]
    Dimensions(String),
}
