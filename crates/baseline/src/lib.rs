//! Equal-engineering context baselines.
//!
//! Implements the comparative evaluation program of blueprint 43.38 and the baseline panel named
//! in 43.41: full files, vector top-k, graph k-hop, query graph, hypergraph component, and FIBER —
//! plus the two competitors `docs/FINDINGS.md` recorded as missing: a fixed-basis embedding
//! retriever and a directed dependency walk. [`sweep`] runs the whole panel over the structural
//! family grid of 43.39.
//!
//! The crate exists so that the central claim is *falsifiable*. Without competent baselines,
//! "FIBER compiles a smaller context" is unfalsifiable marketing; with them it is a measurement
//! that can come out the other way — and if a graph baseline stays compact under equal
//! optimisation, 43.41 requires reporting that result rather than burying it. It has come out the
//! other way: the directed dependency walk selects the identical facts FIBER does on every world
//! in the shipped sweep. See `docs/FINDINGS.md`.
//!
//! # Not implemented
//!
//! - **A learned or neural retriever.** [`LexicalTopK`] is a BM25 proxy and [`EmbeddingTopK`] a
//!   hashed character-n-gram basis; both label themselves as such in every report row. A trained
//!   encoder could behave differently in either direction and nothing here measures one.
//! - **Reranking or score fusion.** Each strategy submits one selection; no strategy sees another
//!   strategy's output.
//! - **Sweeping the decision itself.** [`sweep`] varies the four structural knobs only; skeleton,
//!   events, protected set, decision time and policy stay at their reference values, because cells
//!   that ask different questions are not comparable rows.
//! - **Cost other than facts-exposed.** Token counts, latency and compile time are not measured
//!   here; facts exposed is the only cost column, and it ranks only among admissible strategies.

pub mod compare;
pub mod directed;
pub mod embedding;
pub mod incidence;
pub mod index;
pub mod lexical;
pub mod strategy;
pub mod sweep;

pub use compare::{
    compare, default_panel, Comparison, CompareError, Judgement, RowRefusal, RowVerdict,
    StrategyResult,
};
pub use directed::DirectedDependencyWalk;
pub use embedding::EmbeddingTopK;
pub use incidence::{ConnectedComponent, KHopIncidence, QueryGraph};
pub use index::PanelIndex;
pub use lexical::LexicalTopK;
pub use strategy::{ContextStrategy, FiberCompiled, FullContext, Selection};
pub use sweep::{run_cell, run_sweep, sweep_panel, SweepCell, SweepError, SweepGrid, SweepRow, SweepTable};
