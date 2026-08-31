//! Figure: the plan's selection ratio.
//!
//! Renders the `plan` block of a context certificate (the receipt 43.26 makes mandatory) as two
//! proportional strips: compiled facts against total facts, and compiled factors against total
//! factors — on the reference world, 11 of 761 facts. The counts are the compiler's own
//! accounting, taken verbatim from the certificate; this figure adds no measurement of its own,
//! only proportion.

use crate::error::FigureError;
use crate::extract::{count_field, require, str_field};
use crate::svg::{self, label, label_bold, label_end, rect, ACCENT, MUTED};
use serde_json::Value;

const STRIP_X: f64 = 16.0;
const STRIP_W: f64 = 560.0;
const STRIP_H: f64 = 22.0;

fn strip(body: &mut String, y: f64, noun: &str, compiled: u64, total: u64) {
    let ratio = compiled as f64 / total as f64;
    body.push_str(&label_bold(
        STRIP_X,
        y,
        11.0,
        svg::INK,
        &format!("{noun}: {compiled} of {total} compiled ({:.2}%)", ratio * 100.0),
    ));
    body.push_str(&rect(
        STRIP_X,
        y + 8.0,
        STRIP_W,
        STRIP_H,
        &format!("fill=\"none\" stroke=\"{MUTED}\" stroke-width=\"1\""),
    ));
    if compiled > 0 {
        let width = (ratio * STRIP_W).max(1.0);
        body.push_str(&rect(
            STRIP_X,
            y + 8.0,
            width,
            STRIP_H,
            &format!("fill=\"{ACCENT}\""),
        ));
    }
    body.push_str(&label_end(
        704.0,
        y + 24.0,
        15.0,
        ACCENT,
        &format!("{:.2}%", ratio * 100.0),
    ));
}

/// Render the selection-ratio strips from a certificate document's `plan` block.
pub fn selection_ratio(input: &Value) -> Result<String, FigureError> {
    let world_id = str_field(input, "", "world_id")?;
    let query_id = str_field(input, "", "query_id")?;
    let plan = require(input, "", "plan")?;
    let backend = str_field(plan, "plan", "backend")?;
    let compiled_facts = count_field(plan, "plan", "compiled_fact_count")?;
    let total_facts = count_field(plan, "plan", "total_fact_count")?;
    let compiled_factors = count_field(plan, "plan", "compiled_factor_count")?;
    let total_factors = count_field(plan, "plan", "total_factor_count")?;

    for (label, compiled, total) in [
        ("fact", compiled_facts, total_facts),
        ("factor", compiled_factors, total_factors),
    ] {
        if total == 0 {
            return Err(FigureError::Inconsistent {
                reason: format!("plan.total_{label}_count is 0, so no selection ratio exists"),
            });
        }
        if compiled > total {
            return Err(FigureError::Inconsistent {
                reason: format!(
                    "plan.compiled_{label}_count ({compiled}) exceeds plan.total_{label}_count \
                     ({total})"
                ),
            });
        }
    }

    let mut body = String::new();
    strip(&mut body, 12.0, "facts", compiled_facts, total_facts);
    strip(&mut body, 70.0, "factors", compiled_factors, total_factors);
    body.push_str(&label(
        16.0,
        124.0,
        10.0,
        MUTED,
        "segments are drawn to proportion of the plan's own totals; a nonzero segment is kept at \
         least 1px wide to stay visible",
    ));

    let frame = svg::Frame {
        title: "Context selection ratio",
        caption: format!("world {world_id} · query {query_id} · plan backend {backend}"),
        body,
        body_height: 134.0,
        width: svg::FIG_WIDTH,
    };
    svg::render(&frame, input)
}
