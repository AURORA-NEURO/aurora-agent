//! Reading inputs and writing artifacts.
//!
//! Writes go through [`write_artifact`], which honours `--dry-run`. Blueprint 40.13 makes that a
//! non-negotiable invariant: a dry run has no undeclared effects, so the plan is reported and
//! nothing touches the filesystem.

use crate::exit::{CliError, CliResult, ExitCode};
use bioprism_fiber::Query;
use bioprism_store::LazyWorld;
use bioprism_world::{World, WorldSource};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn read_json(path: &Path) -> CliResult<Value> {
    let text = std::fs::read_to_string(path).map_err(|e| CliError::io(path, e))?;
    serde_json::from_str(&text).map_err(|e| {
        CliError::invalid(format!("not valid JSON: {e}")).about(path.display().to_string())
    })
}

/// Loads a world from either a `fiber-world/0.1` document or an indexed store directory.
///
/// The distinction is deliberately invisible at the command line: 43.16 requires logical
/// semantics to be independent of the physical backend, so `--world` accepting either is the
/// honest surface. A directory containing `manifest.json` is a store; anything else is a document.
pub fn load_source(path: &Path) -> CliResult<Box<dyn WorldSource>> {
    if path.is_dir() && path.join("manifest.json").exists() {
        let lazy = LazyWorld::open(path)
            .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))?;
        Ok(Box::new(lazy))
    } else {
        Ok(Box::new(load_world(path)?))
    }
}

pub fn load_world(path: &Path) -> CliResult<World> {
    let raw = read_json(path)?;
    World::from_json(raw)
        .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))
}

pub fn load_query(path: &Path) -> CliResult<Query> {
    let raw = read_json(path)?;
    Query::from_json(raw)
        .map_err(|e| CliError::invalid(e.to_string()).about(path.display().to_string()))
}

/// A write that either happened or was suppressed by `--dry-run`.
#[derive(Debug, Clone)]
pub struct WrittenArtifact {
    pub path: PathBuf,
    pub bytes: usize,
    pub written: bool,
}

pub fn write_artifact(path: &Path, document: &Value, dry_run: bool) -> CliResult<WrittenArtifact> {
    let mut body = serde_json::to_string_pretty(document)
        .map_err(|e| CliError::new(ExitCode::Io, e.to_string()))?;
    body.push('\n');

    if !dry_run {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| CliError::io(parent, e))?;
            }
        }
        std::fs::write(path, &body).map_err(|e| CliError::io(path, e))?;
    }

    Ok(WrittenArtifact {
        path: path.to_path_buf(),
        bytes: body.len(),
        written: !dry_run,
    })
}
