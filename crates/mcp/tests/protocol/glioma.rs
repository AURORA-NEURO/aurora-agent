//! MCP contract tests for glioma workflows.

use super::*;

#[path = "glioma/autonomous_research.rs"]
mod autonomous_research;
#[path = "glioma/biological_models.rs"]
mod biological_models;
#[path = "glioma/catalogue_reachability.rs"]
mod catalogue_reachability;
#[path = "glioma/computation.rs"]
mod computation;
#[path = "glioma/decision_context.rs"]
mod decision_context;
#[path = "glioma/experiments.rs"]
mod experiments;
#[path = "glioma/federation.rs"]
mod federation;
#[path = "glioma/instrument_operations.rs"]
mod instrument_operations;
#[path = "glioma/program_pipeline.rs"]
mod program_pipeline;
#[path = "glioma/release.rs"]
mod release;
