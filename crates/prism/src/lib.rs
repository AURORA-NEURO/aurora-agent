//! PRISM: decision-state evaluation.
//!
//! Implements the core of blueprint 03 (Decision Cell IR, Oracle IR, Result Bundle), 06 (the
//! benchmark compiler's minimization step) and 07 (matched evaluation on deterministic evidence).
//!
//! The unit of evaluation is a frozen decision state rather than a task outcome. Candidates resume
//! from the identical state, so a difference between them is attributable to the one component the
//! cell left free — here, the context policy. That is the whole argument for cells over end-to-end
//! comparison, and it is why the fork reports *attribution* rather than a score.

pub mod architecture;
pub mod bundle;
pub mod cell;
pub mod fork;
pub mod minimize;

pub use architecture::{Architecture, StrategySpec};
pub use bundle::{Attestation, Reproduction, ResultBundle, BUNDLE_SCHEMA_VERSION};
pub use cell::{Acceptance, DecisionCell, InputRef, CELL_SCHEMA_VERSION};
pub use fork::{matched_fork, render_table, ForkResult, Trial};
pub use minimize::{minimize, minimize_world, preserves, Minimization};
