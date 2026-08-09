//! Synthetic structural benchmark families.
//!
//! Implements blueprint 43.39. The purpose is falsifiability: `docs/FINDINGS.md` records that the
//! shipped reference world cannot separate FIBER from a tuned graph walk or a lexical retriever,
//! because all three select the identical eleven facts. A claim measured only on that world is
//! not a claim about the method.
//!
//! This crate makes the structure a parameter, so the question becomes empirical — *under which
//! structures does each strategy succeed?* — rather than rhetorical.

pub mod generate;
pub mod rng;
pub mod spec;

pub use generate::{generate, Generated};
pub use spec::{DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};
