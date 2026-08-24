//! Project modeling: an entire software project compiled into a bioprism world, so the FIBER
//! pipeline can judge its release readiness and compile minimal decision-sufficient context
//! for working its issues.
//!
//! The one blueprint module this crate implements against is **40.17, the data adapter
//! contract**, and it cites nothing else: project modeling is beyond the biological blueprint's
//! scope, exactly as `bioprism-domain` generalises beyond it, and stretching another module id
//! over this crate would inflate coverage rather than describe it. What 40.17 does govern is
//! the shape of the ingestion — [`scan::ProjectAdapter`] implements
//! [`bioprism_adapter::Adapter`] directly, because a project root fits the
//! [`bioprism_adapter::Source`] directory locator exactly as `InventoryAdapter`'s repositories
//! do, so the sealed facts-plus-loss contract applies without mirroring. Scanning a project
//! loses information at every turn, and the loss ships: into the ingestion, and from there
//! into the world itself as the protected `scan_loss_summary` fact the release oracle
//! requires.
//!
//! The pipeline is three steps, each usable alone:
//!
//! 1. [`ProjectScan::scan`] — a deterministic, std-only walk producing a typed scan and the
//!    sealed ingestion (one fact per file, one per manifest, every skip declared).
//! 2. [`ProjectWorld::assemble`] — a `fiber-world/0.1` document of component inventories,
//!    aggregate decision inputs, per-issue facts and factors, plus the dimension document, the
//!    `project-release-readiness` pack and generated `fiber-query/0.2` queries.
//! 3. [`audit`] — assembles and runs [`bioprism_fiber::compile_with_oracle`] end to end,
//!    returning the verdict with its checkable witnesses.
//!
//! # Not implemented, deliberately
//!
//! * **No execution.** Tests are counted, never run — a counted test is not a passing test,
//!   and the `tests_absent` check's own description says it judges a substring count.
//! * **No git history.** The scanner reads the working tree only; `.git` is on the exclusion
//!   list and every file under it is a declared loss. Authorship, age and churn are absent,
//!   not zero.
//! * **No semantic code analysis.** Markers are case-sensitive substring counts and can over-
//!   and under-count — `TODO` inside a string literal counts, lowercase `todo!()` does not.
//!   The counts are proxies and every consumer of them is told so on the wire.
//! * **Narrow manifest reading with declared losses.** There is no general TOML parser here:
//!   Cargo and pyproject manifests are read line by line for the two common dependency forms,
//!   and every line the narrow reader does not understand becomes a loss entry naming its
//!   line. The loss report is the honesty valve.
//! * **No loss kind of its own.** Those unread manifest lines are declared under the borrowed
//!   [`bioprism_adapter::LossKind::UnmappedColumn`], because `LossKind` is a sealed vocabulary
//!   written for tabular sources and a manifest line is not a column. The reuse is argued on
//!   [`scan::ProjectAdapter`]'s `manifest` method. Consequence for a caller: a
//!   `losses_by_kind` total summed across adapters puts manifest lines and real unmapped
//!   columns in one bucket, and only each entry's `detail` and `location` tell them apart.
//! * **No network.** Requirements are never resolved against a registry; "pinned" is a claim
//!   about the declaration string under the definition on [`scan::DependencyRecord`].
//! * **No semantic issue relevance.** An issue's evidence region comes from the components it
//!   *declares*, resolved syntactically; there is no search, and an unresolvable declaration
//!   is recorded on the issue fact rather than guessed at.
//! * **No clocks.** The scan event and every generated query use one caller-supplied
//!   timestamp, defaulting to the fixed epoch [`assemble::DEFAULT_DECISION_TIME`].
//! * **No collision guard on the ingestion's per-file variable names.** Two component
//!   directories whose slugs collide fail [`ProjectWorld::assemble`] outright, because their
//!   inventory variables would collide in the world. The per-file facts in the sealed ingestion
//!   get no such guard: their `provides` names slug the path the same way, so `src/a-b.rs` and
//!   `src/a_b.rs` both emit `file_src_a_b_rs`. Nothing here consumes those names — the world is
//!   built from component, aggregate and issue facts — but a consumer loading the ingestion into
//!   a world of its own must not assume they are unique.

pub mod assemble;
pub mod packs;
pub mod scan;

pub use assemble::{
    AssemblyOptions, ProjectWorld, Thresholds, AGGREGATE_VARIABLES, DECISION_INPUTS,
    DEFAULT_DECISION_TIME,
};
pub use packs::{
    dimension_document, issue_query, release_query, release_readiness_pack, PACK_NAME,
    PROTECTED_TAG, RELEASE_ORACLE_KIND,
};
pub use scan::{
    DependencyRecord, FileContent, FileRecord, Issue, ManifestKind, ManifestRecord,
    ProjectAdapter, ProjectScan, ScanOptions, DEFAULT_MAX_FILE_BYTES, EXCLUDED_DIRS,
    PROJECT_ADAPTER, PROJECT_ADAPTER_VERSION,
};

use bioprism_domain::DomainPack;
use bioprism_fiber::{compile_with_oracle, Query};
use bioprism_section::{LeakageWitness, OracleStatus};
use bioprism_world::World;
use std::collections::BTreeMap;
use std::path::Path;

/// Typed failures. Scanning IO, adapter-contract violations, issue-file refusals, assembly
/// collisions, and downstream world/pack/compile rejections each keep their own shape so a
/// caller can tell "your issues file is malformed" from "the assembled world is a bug".
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("project adapter: {0}")]
    Adapter(#[from] bioprism_adapter::AdapterError),
    #[error("issues file: {0}")]
    Issues(String),
    #[error("world assembly: {0}")]
    Assembly(String),
    #[error("assembled world rejected by the reference validator: {0}")]
    World(#[from] bioprism_world::WorldError),
    #[error("emitted pack rejected by the domain parser: {0}")]
    Domain(#[from] bioprism_domain::DomainError),
    #[error("compile: {0}")]
    Fiber(#[from] bioprism_fiber::FiberError),
    #[error("io on {path}: {message}")]
    Io { path: String, message: String },
}

/// Options for the end-to-end [`audit`].
#[derive(Debug, Clone, Default)]
pub struct AuditOptions {
    pub scan: Option<ScanOptions>,
    pub assembly: AssemblyOptions,
}

impl AuditOptions {
    pub fn new(project: impl Into<String>) -> Self {
        AuditOptions {
            scan: Some(ScanOptions::new(project)),
            assembly: AssemblyOptions::default(),
        }
    }
}

/// What one end-to-end audit concluded, with the evidence to check it.
#[derive(Debug, Clone)]
pub struct AuditReport {
    pub world_id: String,
    pub oracle_kind: String,
    pub status: OracleStatus,
    /// The verdict's witnesses verbatim — fired checks and unrun checks alike, each stating
    /// which it is in its detail.
    pub witnesses: Vec<LeakageWitness>,
    /// Facts in the assembled world (not in the ingestion; the file layer stays below).
    pub fact_count: usize,
    pub selected_fact_count: usize,
    /// The scan's loss entries counted by kind — the same numbers the world's protected
    /// `scan_loss_summary` fact carries.
    pub loss_kind_counts: BTreeMap<String, u64>,
}

impl AuditReport {
    /// The `check` names carried by the verdict's witnesses, in verdict order.
    pub fn witness_check_names(&self) -> Vec<String> {
        self.witnesses
            .iter()
            .map(|witness| match witness {
                LeakageWitness::DomainCheck { check, .. } => check.clone(),
                other => other.kind().to_string(),
            })
            .collect()
    }

    /// One line for a log: status, witness names, fact counts, loss counts.
    pub fn summary(&self) -> String {
        format!(
            "{} judged {:?} by {} with witnesses [{}]; {} world facts, {} selected; {} loss entries by kind {:?}",
            self.world_id,
            self.status,
            self.oracle_kind,
            self.witness_check_names().join(", "),
            self.fact_count,
            self.selected_fact_count,
            self.loss_kind_counts.values().sum::<u64>(),
            self.loss_kind_counts
        )
    }
}

/// Scans, assembles, and compiles the release query under the emitted pack's oracle.
///
/// The intermediate documents all pass their own strict parsers on the way — the world through
/// [`World::from_json`], the pack through [`DomainPack::from_json`], the query through
/// [`Query::from_json`] — so an inconsistency between emitter and parser is an error here, at
/// the boundary, rather than a wrong verdict later.
pub fn audit(root: &Path, options: &AuditOptions) -> Result<AuditReport, ProjectError> {
    let scan_options = options
        .scan
        .clone()
        .unwrap_or_else(|| ScanOptions::new("project"));
    let (scan, _ingestion) = ProjectScan::scan(root, &scan_options)?;
    let assembled = ProjectWorld::assemble(&scan, &options.assembly)?;

    let world = World::from_json(assembled.world.clone())?;
    let pack = DomainPack::from_json(&assembled.pack)?;
    let query = Query::from_json(assembled.release_query.clone())?;
    let out = compile_with_oracle(&world, &query, pack.oracle())?;

    Ok(AuditReport {
        world_id: assembled.world_id,
        oracle_kind: out.certificate.oracle.oracle_kind.clone(),
        status: out.certificate.oracle.status,
        witnesses: out.certificate.oracle.witnesses.clone(),
        fact_count: world.facts.len(),
        selected_fact_count: out.certificate.selected_facts.len(),
        loss_kind_counts: scan.loss_kind_counts(),
    })
}
