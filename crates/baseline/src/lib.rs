//! Equal-engineering context baselines.
//!
//! Implements the comparative evaluation program of blueprint 43.38 and the baseline panel named
//! in 43.41: full files, vector top-k, graph k-hop, query graph, hypergraph component, and FIBER.
//!
//! The crate exists so that the central claim is *falsifiable*. Without competent baselines,
//! "FIBER compiles a smaller context" is unfalsifiable marketing; with them it is a measurement
//! that can come out the other way — and if a graph baseline stays compact under equal
//! optimisation, 43.41 requires reporting that result rather than burying it.

pub mod compare;
pub mod incidence;
pub mod lexical;
pub mod strategy;

pub use compare::{compare, default_panel, Comparison, StrategyResult};
pub use incidence::{ConnectedComponent, KHopIncidence, QueryGraph};
pub use lexical::LexicalTopK;
pub use strategy::{ContextStrategy, FiberCompiled, FullContext, Selection};
