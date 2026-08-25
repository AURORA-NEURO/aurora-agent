//! Figure: the structural family sweep as a matrix.
//!
//! Renders the sweep table `bioprism world sweep` serialises (the 43.39 grid: attachment × relay
//! depth × tag style as rows, distractor count as columns). Each cell is coloured by which
//! strategies were admissible in that generated world, with the panel's deliberate-negative
//! honesty built into the encoding: a tie between FIBER and a baseline is a first-class result —
//! the repository's own headline finding is one — and is drawn with at least the visual weight of
//! a FIBER-only cell, never washed out.
//!
//! The column count comes from the table, not from a constant — `world sweep --distractors` takes
//! an arbitrary list — so this is the one figure whose frame is not the fixed width. It widens to
//! fit the columns it drew, because a column rendered past the viewBox is clipped away with no
//! mark on the figure, and a sweep that silently drops its highest distractor counts would
//! misreport exactly the cells the sweep exists to show.
//!
//! Full-context is excluded from the cell categories because it is admissible by construction
//! (it exposes everything), so counting it would turn every cell into a tie and drown the signal;
//! the exclusion is stated in the legend, and a cell where full-context is unexpectedly
//! inadmissible is marked rather than silently absorbed.

use crate::error::FigureError;
use crate::extract::{array_field, bool_field, count_field, str_field};
use crate::svg::{
    self, label, label_italic, label_middle, rect, truncate_chars, wrap_chars, ACCENT, INK, MUTED,
};
use serde_json::Value;
use std::collections::BTreeMap;

const GRID_X: f64 = 216.0;
const GRID_TOP: f64 = 40.0;
const CELL_W: f64 = 64.0;
const CELL_H: f64 = 24.0;
const X_PITCH: f64 = 72.0;
const Y_PITCH: f64 = 30.0;
const LEGEND_PITCH: f64 = 17.0;
const RIGHT_MARGIN: f64 = 16.0;

/// Transcribed verbatim from `bioprism_baseline::sweep::SweepGrid`'s declaration (minus doc-link
/// markup), because a figure of the sweep must carry the sweep's own scope caveat.
const UNSWEPT_KNOBS_CAVEAT: &str =
    "The other WorldSpec knobs — skeleton, events, protected set, decision time, policy — are \
     deliberately not swept: they change what the decision is, not the structure around it, and a \
     sweep that varied them would be comparing strategies across different questions.";

#[derive(Clone, Copy)]
enum Category {
    Tie,
    FiberOnly,
    BaselineOnly,
    NoneAdmissible,
}

struct CellSummary {
    category: Category,
    admissible_count: usize,
    refused_present: bool,
    full_context_inadmissible: bool,
}

/// Render the sweep matrix from a `world sweep` JSON document.
pub fn sweep_grid(input: &Value) -> Result<String, FigureError> {
    let seed = count_field(input, "", "seed")?;
    let cells = array_field(input, "", "cells")?;
    if cells.is_empty() {
        return Err(FigureError::EmptyCollection {
            field: "cells".to_string(),
        });
    }
    if let Some(claimed) = input.get("cells_total") {
        let claimed = claimed.as_u64().ok_or_else(|| FigureError::WrongType {
            field: "cells_total".to_string(),
            expected: "a non-negative integer",
        })?;
        if claimed as usize != cells.len() {
            return Err(FigureError::Inconsistent {
                reason: format!(
                    "cells_total claims {claimed} cells but the cells array holds {}",
                    cells.len()
                ),
            });
        }
    }

    let mut row_keys: Vec<(String, u64, String)> = Vec::new();
    let mut col_keys: Vec<u64> = Vec::new();
    let mut summaries: BTreeMap<(usize, usize), CellSummary> = BTreeMap::new();
    let mut panel_size = 0usize;

    for (index, cell) in cells.iter().enumerate() {
        let parent = format!("cells[{index}]");
        let attachment = str_field(cell, &parent, "attachment")?.to_string();
        let relay_depth = count_field(cell, &parent, "relay_depth")?;
        let tag_style = str_field(cell, &parent, "tag_style")?.to_string();
        let distractors = count_field(cell, &parent, "distractors")?;
        let rows = array_field(cell, &parent, "rows")?;
        if index == 0 {
            panel_size = rows.len();
        }

        let mut fiber_admissible = None;
        let mut others_admissible = 0usize;
        let mut refused_present = false;
        let mut full_context_inadmissible = false;
        for (row_index, row) in rows.iter().enumerate() {
            let row_parent = format!("{parent}.rows[{row_index}]");
            let strategy = str_field(row, &row_parent, "strategy")?;
            let judged = bool_field(row, &row_parent, "judged")?;
            let admissible = bool_field(row, &row_parent, "admissible")?;
            if !judged {
                refused_present = true;
            }
            match strategy {
                "fiber" => fiber_admissible = Some(admissible),
                "full-context" => full_context_inadmissible = !admissible,
                _ => {
                    if admissible {
                        others_admissible += 1;
                    }
                }
            }
        }
        let Some(fiber_admissible) = fiber_admissible else {
            return Err(FigureError::Inconsistent {
                reason: format!(
                    "{parent} has no `fiber` row, so the cell cannot be categorised against the \
                     baselines"
                ),
            });
        };

        let row_key = (attachment, relay_depth, tag_style);
        let row_index = match row_keys.iter().position(|key| key == &row_key) {
            Some(position) => position,
            None => {
                row_keys.push(row_key);
                row_keys.len() - 1
            }
        };
        let col_index = match col_keys.iter().position(|key| *key == distractors) {
            Some(position) => position,
            None => {
                col_keys.push(distractors);
                col_keys.len() - 1
            }
        };

        let category = match (fiber_admissible, others_admissible) {
            (true, 0) => Category::FiberOnly,
            (true, _) => Category::Tie,
            (false, 0) => Category::NoneAdmissible,
            (false, _) => Category::BaselineOnly,
        };
        let summary = CellSummary {
            category,
            admissible_count: others_admissible + usize::from(fiber_admissible),
            refused_present,
            full_context_inadmissible,
        };
        if summaries.insert((row_index, col_index), summary).is_some() {
            return Err(FigureError::Inconsistent {
                reason: format!(
                    "{parent} repeats an (attachment, relay_depth, tag_style, distractors) \
                     combination already present in the table"
                ),
            });
        }
    }

    let mut body = String::new();
    let grid_width = col_keys.len() as f64 * X_PITCH - (X_PITCH - CELL_W);
    body.push_str(&label_italic(
        GRID_X + grid_width / 2.0 - 30.0,
        12.0,
        10.5,
        MUTED,
        "distractors",
    ));
    for (col, distractors) in col_keys.iter().enumerate() {
        body.push_str(&label_middle(
            GRID_X + col as f64 * X_PITCH + CELL_W / 2.0,
            30.0,
            10.5,
            INK,
            &distractors.to_string(),
        ));
    }
    let mut any_refused = false;
    let mut any_full_context_inadmissible = false;
    for (row, (attachment, relay_depth, tag_style)) in row_keys.iter().enumerate() {
        let y = GRID_TOP + row as f64 * Y_PITCH;
        body.push_str(&label(
            16.0,
            y + 16.0,
            10.5,
            INK,
            &truncate_chars(&format!("{attachment} · r{relay_depth} · {tag_style}"), 32),
        ));
        for col in 0..col_keys.len() {
            let Some(summary) = summaries.get(&(row, col)) else {
                continue;
            };
            let x = GRID_X + col as f64 * X_PITCH;
            let style = match summary.category {
                Category::Tie => format!(
                    "fill=\"url(#hatch-accent)\" stroke=\"{INK}\" stroke-width=\"2\""
                ),
                Category::FiberOnly => {
                    format!("fill=\"{ACCENT}\" stroke=\"{INK}\" stroke-width=\"0.75\"")
                }
                Category::BaselineOnly => format!("fill=\"{MUTED}\""),
                Category::NoneAdmissible => format!(
                    "fill=\"none\" stroke=\"{MUTED}\" stroke-width=\"1\" \
                     stroke-dasharray=\"4 3\""
                ),
            };
            body.push_str(&rect(x, y, CELL_W, CELL_H, &style));
            let mut mark = summary.admissible_count.to_string();
            if summary.refused_present {
                any_refused = true;
                mark.push('†');
            }
            if summary.full_context_inadmissible {
                any_full_context_inadmissible = true;
                mark.push('‡');
            }
            body.push_str(&label_middle(
                x + CELL_W / 2.0,
                y + CELL_H / 2.0 + 3.5,
                10.0,
                INK,
                &mark,
            ));
        }
    }

    let mut y = GRID_TOP + row_keys.len() as f64 * Y_PITCH + 16.0;
    let legend: [(&str, String); 4] = [
        (
            "tie — FIBER and at least one baseline both admissible (a first-class result, drawn \
             as prominently as a win)",
            format!("fill=\"url(#hatch-accent)\" stroke=\"{INK}\" stroke-width=\"2\""),
        ),
        (
            "FIBER only — no baseline admissible",
            format!("fill=\"{ACCENT}\" stroke=\"{INK}\" stroke-width=\"0.75\""),
        ),
        (
            "baseline only — FIBER inadmissible",
            format!("fill=\"{MUTED}\""),
        ),
        (
            "none admissible",
            format!(
                "fill=\"none\" stroke=\"{MUTED}\" stroke-width=\"1\" stroke-dasharray=\"4 3\""
            ),
        ),
    ];
    for (text, style) in &legend {
        body.push_str(&rect(16.0, y - 10.0, 12.0, 12.0, style));
        body.push_str(&label(34.0, y, 10.5, INK, text));
        y += LEGEND_PITCH;
    }
    body.push_str(&label(
        16.0,
        y,
        10.5,
        INK,
        "admissible = right verdict and full protected closure — the only axis the sweep ranks on",
    ));
    y += LEGEND_PITCH;
    body.push_str(&label(
        16.0,
        y,
        10.5,
        INK,
        "cell number counts admissible strategies excluding full-context, which is admissible by \
         construction",
    ));
    y += LEGEND_PITCH;
    if any_refused {
        body.push_str(&label(
            16.0,
            y,
            10.5,
            MUTED,
            "† cell contains a row the oracle refused — counted as neither sound nor unsound",
        ));
        y += LEGEND_PITCH;
    }
    if any_full_context_inadmissible {
        body.push_str(&label(
            16.0,
            y,
            10.5,
            MUTED,
            "‡ full-context inadmissible in this cell — unexpected; inspect the table itself",
        ));
        y += LEGEND_PITCH;
    }

    y += 8.0;
    for line in wrap_chars(UNSWEPT_KNOBS_CAVEAT, 118) {
        body.push_str(&label(16.0, y, 10.0, MUTED, &line));
        y += 13.0;
    }

    let frame = svg::Frame {
        title: "Structural family sweep",
        caption: format!(
            "seed {seed} · {} cells · panel of {panel_size} strategies · ranked on \
             admissibility, never verdict alone",
            cells.len()
        ),
        body,
        body_height: y + 2.0,
        width: (GRID_X + grid_width + RIGHT_MARGIN).max(svg::FIG_WIDTH),
    };
    svg::render(&frame, input)
}
