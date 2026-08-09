//! The concrete domains, none of which 43.11 names.
//!
//! Blueprint 43.11 states the soundness condition `f(γ(a)) ⊆ γ(f#(a))` and stops. It does not name
//! a domain, a lattice, a widening operator or a refinement strategy, so everything below is an
//! invention of this implementation, chosen for what the rest of the crate already needed:
//!
//! | Domain | Abstracts | Concretisation | Widening |
//! |---|---|---|---|
//! | [`RatioIntervalDomain`] | a pointwise reweighting of the joint measure | infinite; laws are a falsification search | classical `0 / ∞` |
//! | [`DisplacementDomain`] | an accumulated total-variation budget | infinite; laws are a falsification search | a power-of-two threshold ladder |
//! | [`SupportDomain`] | a factor potential, one sign per entry | finite and complete; laws are proofs | `join`, because the lattice is of finite height |
//! | [`ProductDomain`] | one inner element per region variable | inherited | pointwise |
//!
//! Three domains rather than two because a registry with one implementor is a trait, and a registry
//! with two of the same shape does not test that the ordering, the concretisation and the widening
//! are genuinely per-domain. The first two share a lattice and share nothing else; the third shares
//! neither; the fourth is built from any of them.

pub mod displacement;
pub mod product;
pub mod ratio_interval;
pub mod support;

pub use displacement::{Displacement, DisplacementDomain, DISPLACEMENT_DOMAIN};
pub use product::{ProductDomain, PRODUCT_DOMAIN_PREFIX};
pub use ratio_interval::{RatioInterval, RatioIntervalDomain, RATIO_INTERVAL_DOMAIN};
pub use support::{EntrySign, Support, SupportDomain, SUPPORT_DOMAIN_PREFIX};
