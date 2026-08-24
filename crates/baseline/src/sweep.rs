//! The structural family sweep.
//!
//! `docs/FINDINGS.md` §4 states the standing limitation of the single-world comparisons: "Both
//! are single points. A claim about the method needs the full family swept — attachment × relay
//! depth × tag style × distractor count — with the result reported wherever it lands." This module
//! is that sweep. It takes a declared grid over [`WorldSpec`] knobs and one seed, generates each
//! world and its query, runs the full strategy panel over every cell, and returns a typed table:
//! per cell, per strategy, the facts selected, whether the verdict was sound, the protected
//! closure fraction, and admissibility.
//!
//! Ranking stays on **admissibility** — right verdict *and* full protected closure — never on
//! verdict alone, for the reason [`crate::compare`] documents: on camouflaged worlds a lexical
//! retriever reaches the right verdict from an incomplete closure, and crediting that would crown
//! a strategy that violated the contract and got away with it.
//!
//! Determinism: [`generate`] is a pure function of the spec including its seed, every strategy in
//! the panel is deterministic, and the grid is traversed in declared order, so the same grid and
//! seed produce an identical [`SweepTable`] byte for byte. `tests/sweep_grid.rs` asserts it.
//!
//! Soundness is `Option<bool>`, not `bool`: a row the oracle refused was never judged, and
//! reporting it as unsound would restate the refusal-as-refutation defect [`crate::compare`] was
//! fixed for. No generated world in the default grid produces a refusal — the state exists here so
//! that a grid over caller-supplied specs cannot silently coerce one.

use crate::compare::{compare, CompareError};
use crate::strategy::ContextStrategy;
use bioprism_fiber::Query;
use bioprism_world::World;
use bioprism_worldgen::{generate, DistractorAttachment, TagStyle, WorldSpec};
use std::fmt::Write as _;

/// The declared grid: every combination of these knob values is one cell.
///
/// The other [`WorldSpec`] knobs — skeleton, events, protected set, decision time, policy — are
/// deliberately *not* swept: they change what the decision is, not the structure around it, and a
/// sweep that varied them would be comparing strategies across different questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepGrid {
    pub attachments: Vec<DistractorAttachment>,
    pub relay_depths: Vec<usize>,
    pub tag_styles: Vec<TagStyle>,
    pub distractor_counts: Vec<usize>,
    pub seed: u64,
}

impl SweepGrid {
    /// The default grid: attachment {Hub, NearTarget} × relay depth {0, 2, 4} × tag style
    /// {Distinct, Camouflaged} × distractors {50, 250, 750}, 36 cells.
    pub fn default_grid() -> Self {
        SweepGrid {
            attachments: vec![DistractorAttachment::Hub, DistractorAttachment::NearTarget],
            relay_depths: vec![0, 2, 4],
            tag_styles: vec![TagStyle::Distinct, TagStyle::Camouflaged],
            distractor_counts: vec![50, 250, 750],
            seed: 20_260_823,
        }
    }

    /// The specs this grid declares, in traversal order.
    pub fn specs(&self) -> Vec<WorldSpec> {
        let mut specs = Vec::new();
        for attachment in &self.attachments {
            for relay_depth in &self.relay_depths {
                for tag_style in &self.tag_styles {
                    for distractors in &self.distractor_counts {
                        let mut spec = WorldSpec::reference_like(*distractors);
                        spec.attachment = *attachment;
                        spec.relay_depth = *relay_depth;
                        spec.tag_style = *tag_style;
                        spec.seed = self.seed;
                        spec.world_id = format!(
                            "sweep-{}-r{relay_depth}-{}-d{distractors}",
                            attachment_label(*attachment),
                            tag_label(*tag_style),
                        );
                        specs.push(spec);
                    }
                }
            }
        }
        specs
    }
}

fn attachment_label(attachment: DistractorAttachment) -> &'static str {
    match attachment {
        DistractorAttachment::Hub => "hub",
        DistractorAttachment::NearTarget => "neartarget",
    }
}

fn tag_label(style: TagStyle) -> &'static str {
    match style {
        TagStyle::Distinct => "distinct",
        TagStyle::Camouflaged => "camouflaged",
    }
}

/// One strategy's measurement in one cell.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepRow {
    pub strategy: String,
    pub facts_selected: usize,
    /// `Some(true)`: reproduced the reference verdict and witnesses. `Some(false)`: judged and
    /// found not to. `None`: the oracle refused this selection, so nothing was established.
    pub sound: Option<bool>,
    /// Fraction of the query's protected facts retained.
    pub protected_closure: f64,
    /// Sound *and* full protected closure — the only axis the sweep ranks on.
    pub admissible: bool,
}

/// One world's worth of measurements, tagged with the knob values that built it.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepCell {
    pub world_id: String,
    pub attachment: DistractorAttachment,
    pub relay_depth: usize,
    pub tag_style: TagStyle,
    pub distractors: usize,
    pub total_facts: usize,
    pub rows: Vec<SweepRow>,
}

impl SweepCell {
    pub fn row(&self, strategy: &str) -> Option<&SweepRow> {
        self.rows.iter().find(|row| row.strategy == strategy)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SweepTable {
    pub seed: u64,
    pub cells: Vec<SweepCell>,
}

/// Why a cell produced no measurements.
#[derive(Debug, thiserror::Error)]
pub enum SweepError {
    /// The generator emitted a world document `bioprism_world` rejects.
    #[error("generated world {world_id} does not load: {detail}")]
    WorldRejected { world_id: String, detail: String },
    /// The generator emitted a query document `bioprism_fiber` rejects.
    #[error("generated query for {world_id} does not load: {detail}")]
    QueryRejected { world_id: String, detail: String },
    /// The oracle refused the full-context reference, so the cell has no verdict to measure
    /// strategies against. Propagated rather than recorded as an empty cell, because a table with
    /// a silently missing cell reads as a table nobody found anything wrong with.
    #[error("cell {world_id} has no reference verdict: {source}")]
    NoReference {
        world_id: String,
        #[source]
        source: CompareError,
    },
}

/// The sweep's panel: every strategy family in the repository at its documented settings.
///
/// The graph walk enters at depths 4–7, the range containing its best reference-world setting
/// (5–6) — the equal-engineering obligation [`crate::compare::default_panel`] documents. The two
/// retrieval baselines enter at k=11 (FIBER's reference-world size) and k=50; the directed walk
/// enters unbounded, its strongest setting.
pub fn sweep_panel() -> Vec<Box<dyn ContextStrategy>> {
    vec![
        Box::new(crate::strategy::FullContext),
        Box::new(crate::incidence::KHopIncidence { depth: 4 }),
        Box::new(crate::incidence::KHopIncidence { depth: 5 }),
        Box::new(crate::incidence::KHopIncidence { depth: 6 }),
        Box::new(crate::incidence::KHopIncidence { depth: 7 }),
        Box::new(crate::lexical::LexicalTopK { k: 11 }),
        Box::new(crate::lexical::LexicalTopK { k: 50 }),
        Box::new(crate::embedding::EmbeddingTopK { k: 11 }),
        Box::new(crate::embedding::EmbeddingTopK { k: 50 }),
        Box::new(crate::directed::DirectedDependencyWalk::unbounded()),
        Box::new(crate::strategy::FiberCompiled),
    ]
}

/// Generates one spec's world and query and measures the full panel over them.
///
/// Public on its own so a test can run a *preset* — [`WorldSpec::discriminating`] — through
/// exactly the machinery the grid uses, pinning the sweep to the documented FINDINGS.md rows.
pub fn run_cell(spec: &WorldSpec) -> Result<SweepCell, SweepError> {
    let generated = generate(spec);
    let world = World::from_json(generated.world).map_err(|error| SweepError::WorldRejected {
        world_id: spec.world_id.clone(),
        detail: error.to_string(),
    })?;
    let query = Query::from_json(generated.query).map_err(|error| SweepError::QueryRejected {
        world_id: spec.world_id.clone(),
        detail: error.to_string(),
    })?;

    let panel = sweep_panel();
    let borrowed: Vec<&dyn ContextStrategy> = panel.iter().map(|boxed| boxed.as_ref()).collect();
    let comparison =
        compare(&world, &query, &borrowed).map_err(|source| SweepError::NoReference {
            world_id: spec.world_id.clone(),
            source,
        })?;

    Ok(SweepCell {
        world_id: spec.world_id.clone(),
        attachment: spec.attachment,
        relay_depth: spec.relay_depth,
        tag_style: spec.tag_style,
        distractors: spec.distractors,
        total_facts: comparison.total_facts,
        rows: comparison
            .results
            .iter()
            .map(|result| SweepRow {
                strategy: result.name.clone(),
                facts_selected: result.facts_exposed,
                sound: result.verdict_preserving(),
                protected_closure: result.protected_recall,
                admissible: result.admissible(),
            })
            .collect(),
    })
}

/// Runs every cell of the grid, in declared order.
pub fn run_sweep(grid: &SweepGrid) -> Result<SweepTable, SweepError> {
    let mut cells = Vec::new();
    for spec in grid.specs() {
        cells.push(run_cell(&spec)?);
    }
    Ok(SweepTable {
        seed: grid.seed,
        cells,
    })
}

impl SweepTable {
    /// Strategies in panel order, taken from the first cell. Every cell runs the same panel.
    fn strategies(&self) -> Vec<&str> {
        self.cells
            .first()
            .map(|cell| cell.rows.iter().map(|row| row.strategy.as_str()).collect())
            .unwrap_or_default()
    }

    /// Cells in which `strategy` was admissible.
    pub fn admissible_cells(&self, strategy: &str) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.row(strategy).is_some_and(|row| row.admissible))
            .count()
    }

    pub fn to_markdown(&self) -> String {
        let mut text = String::new();
        let _ = writeln!(
            text,
            "# Structural family sweep\n\nseed {}, {} cells, panel of {} strategies. Ranking is \
             on admissibility (right verdict **and** full protected closure), never on verdict \
             alone.",
            self.seed,
            self.cells.len(),
            self.strategies().len()
        );

        let _ = writeln!(text, "\n## Admissibility summary\n");
        let _ = writeln!(text, "| Strategy | Admissible cells | Mean facts when admissible |");
        let _ = writeln!(text, "|---|---:|---:|");
        for strategy in self.strategies() {
            let admissible: Vec<usize> = self
                .cells
                .iter()
                .filter_map(|cell| cell.row(strategy))
                .filter(|row| row.admissible)
                .map(|row| row.facts_selected)
                .collect();
            let mean = if admissible.is_empty() {
                "—".to_string()
            } else {
                format!(
                    "{:.1}",
                    admissible.iter().sum::<usize>() as f64 / admissible.len() as f64
                )
            };
            let _ = writeln!(
                text,
                "| {strategy} | {} / {} | {mean} |",
                admissible.len(),
                self.cells.len()
            );
        }

        for cell in &self.cells {
            let _ = writeln!(
                text,
                "\n## {} — attachment {:?}, relay depth {}, tags {:?}, {} distractors ({} facts)\n",
                cell.world_id,
                cell.attachment,
                cell.relay_depth,
                cell.tag_style,
                cell.distractors,
                cell.total_facts
            );
            let _ = writeln!(text, "| Strategy | Facts | Sound? | Closure | Admissible |");
            let _ = writeln!(text, "|---|---:|:-:|---:|:-:|");
            for row in &cell.rows {
                let sound = match row.sound {
                    Some(true) => "yes",
                    Some(false) => "**no**",
                    None => "**refused**",
                };
                let _ = writeln!(
                    text,
                    "| {} | {} | {} | {:.0}% | {} |",
                    row.strategy,
                    row.facts_selected,
                    sound,
                    row.protected_closure * 100.0,
                    if row.admissible { "yes" } else { "**no**" }
                );
            }
        }

        text
    }
}
