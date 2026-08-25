//! Figure: the equal-engineering baseline panel.
//!
//! Renders the comparison document `bioprism_baseline::Comparison::to_json` emits (the 43.38
//! matched comparison, ranked with 43.41's admissibility vocabulary). One horizontal bar per
//! strategy shows `facts_exposed` on a linear scale; the four row states use the harness's own
//! vocabulary — admissible, lucky, unsound, refused — and the legend defines each in the words the
//! harness uses.
//!
//! Two honesty rules are structural here rather than stylistic. A refused row is drawn as a
//! *state*, not a bar: the oracle never judged it, so it gets no verdict-coloured geometry at all
//! and can never be misread as a measured zero (its cost, which *was* measured, is stated in
//! text). And the serialized `admissible` flag is cross-checked against the verdict fields it is
//! defined from; a document where they disagree is refused rather than rendered.

use crate::error::FigureError;
use crate::extract::{
    array_field, bool_field, count_field, f64_field, nullable_str_field, require, str_field,
};
use crate::svg::{
    self, esc, label, label_bold, label_italic, rect, truncate_chars, ACCENT, INK, MUTED,
};
use serde_json::Value;
use std::fmt::Write as _;

const ROW_PITCH: f64 = 26.0;
const BAR_X: f64 = 190.0;
const BAR_MAX: f64 = 300.0;
const LEGEND_PITCH: f64 = 17.0;

enum Category {
    Admissible,
    Lucky,
    Unsound,
    Refused,
}

struct Row {
    name: String,
    facts: u64,
    fraction: f64,
    recall: f64,
    category: Category,
}

/// Render the baseline comparison panel from a `Comparison::to_json` document.
pub fn baseline_panel(input: &Value) -> Result<String, FigureError> {
    let world_id = str_field(input, "", "world_id")?;
    let query_id = str_field(input, "", "query_id")?;
    let total_facts = count_field(input, "", "total_facts")?;
    let reference = require(input, "", "reference")?;
    let reference_status = str_field(reference, "reference", "status")?;
    let cheapest = nullable_str_field(input, "", "cheapest_admissible_strategy")?.map(str::to_string);

    let results = array_field(input, "", "results")?;
    if results.is_empty() {
        return Err(FigureError::EmptyCollection {
            field: "results".to_string(),
        });
    }

    let mut rows = Vec::with_capacity(results.len());
    for (index, result) in results.iter().enumerate() {
        let parent = format!("results[{index}]");
        let name = str_field(result, &parent, "name")?.to_string();
        let facts = count_field(result, &parent, "facts_exposed")?;
        let fraction = f64_field(result, &parent, "fraction_of_world")?;
        let judged = bool_field(result, &parent, "judged")?;
        let recall = f64_field(result, &parent, "protected_recall")?;
        let closure = bool_field(result, &parent, "closure_complete")?;
        let category = if judged {
            let preserving = bool_field(result, &parent, "verdict_preserving")?;
            let admissible = bool_field(result, &parent, "admissible")?;
            if admissible != (preserving && closure) {
                return Err(FigureError::Inconsistent {
                    reason: format!(
                        "{parent} claims admissible={admissible} but verdict_preserving=\
                         {preserving} and closure_complete={closure}"
                    ),
                });
            }
            if admissible {
                Category::Admissible
            } else if preserving {
                Category::Lucky
            } else {
                Category::Unsound
            }
        } else {
            str_field(result, &parent, "refusal")?;
            if result.get("admissible").is_some()
                || result.get("verdict_preserving").is_some()
                || result.get("status").is_some()
            {
                return Err(FigureError::Inconsistent {
                    reason: format!(
                        "{parent} is unjudged yet carries oracle-derived keys; absence is \
                         semantic for a refused row"
                    ),
                });
            }
            Category::Refused
        };
        rows.push(Row {
            name,
            facts,
            fraction,
            recall,
            category,
        });
    }

    if let Some(name) = &cheapest {
        let named_admissible = rows
            .iter()
            .any(|row| &row.name == name && matches!(row.category, Category::Admissible));
        if !named_admissible {
            return Err(FigureError::Inconsistent {
                reason: format!(
                    "cheapest_admissible_strategy names `{name}` but no admissible result row \
                     has that name"
                ),
            });
        }
    }

    let scale = rows.iter().map(|row| row.facts).max().unwrap_or(0).max(1) as f64;
    let mut body = String::new();
    for (index, row) in rows.iter().enumerate() {
        let y = index as f64 * ROW_PITCH;
        let baseline = y + 15.0;
        if cheapest.as_deref() == Some(row.name.as_str()) {
            body.push_str(&label_bold(4.0, baseline, 11.0, ACCENT, "◆"));
        }
        body.push_str(&label(
            16.0,
            baseline,
            11.0,
            INK,
            &truncate_chars(&row.name, 26),
        ));
        let bar_style = match row.category {
            Category::Admissible => Some(format!("fill=\"{ACCENT}\"")),
            Category::Lucky => Some(format!(
                "fill=\"url(#hatch-accent)\" stroke=\"{ACCENT}\" stroke-width=\"1\""
            )),
            Category::Unsound => Some(format!("fill=\"{MUTED}\"")),
            Category::Refused => None,
        };
        match bar_style {
            Some(style) => {
                let width = row.facts as f64 / scale * BAR_MAX;
                if width > 0.0 {
                    body.push_str(&rect(BAR_X, y + 3.0, width, 15.0, &style));
                }
                body.push_str(&label(
                    BAR_X + width + 8.0,
                    baseline,
                    10.5,
                    INK,
                    &format!(
                        "{} facts · {:.1}% · closure {:.0}%",
                        row.facts,
                        row.fraction * 100.0,
                        row.recall * 100.0
                    ),
                ));
            }
            None => {
                body.push_str(&label_italic(
                    BAR_X,
                    baseline,
                    10.5,
                    MUTED,
                    &format!(
                        "refused (not judged) — selected {} facts ({:.2}% of world); neither \
                         sound nor unsound",
                        row.facts,
                        row.fraction * 100.0
                    ),
                ));
            }
        }
    }

    let rows_height = rows.len() as f64 * ROW_PITCH;
    let legend_top = rows_height + 12.0;
    let mut legend = String::new();
    let swatch_entries: [(&str, String); 3] = [
        (
            "admissible — right verdict and full protected closure",
            format!("fill=\"{ACCENT}\""),
        ),
        (
            "lucky — right verdict from an incomplete protected closure (not a pass)",
            format!("fill=\"url(#hatch-accent)\" stroke=\"{ACCENT}\" stroke-width=\"1\""),
        ),
        (
            "unsound — judged and did not preserve the reference verdict",
            format!("fill=\"{MUTED}\""),
        ),
    ];
    for (index, (text, style)) in swatch_entries.iter().enumerate() {
        let y = legend_top + index as f64 * LEGEND_PITCH;
        legend.push_str(&rect(16.0, y, 12.0, 12.0, style));
        legend.push_str(&label(34.0, y + 10.0, 10.5, INK, text));
    }
    let refused_y = legend_top + 3.0 * LEGEND_PITCH + 10.0;
    legend.push_str(&label_italic(
        16.0,
        refused_y,
        10.5,
        MUTED,
        "refused (not judged) — the oracle refused the selection; neither sound nor unsound, \
         drawn without a bar",
    ));
    let marker_y = refused_y + LEGEND_PITCH;
    let _ = writeln!(
        legend,
        "<text x=\"16.00\" y=\"{marker_y:.2}\" font-family=\"{}\" font-size=\"10.5\" \
         fill=\"{INK}\"><tspan fill=\"{ACCENT}\" font-weight=\"600\">◆</tspan> {}</text>",
        svg::FONT_TEXT,
        esc("cheapest admissible strategy")
    );
    body.push_str(&legend);

    let frame = svg::Frame {
        title: "Equal-engineering baseline panel",
        caption: format!(
            "world {world_id} · query {query_id} · {total_facts} facts · reference verdict \
             (full-context): {reference_status}"
        ),
        body,
        body_height: marker_y + 8.0,
        width: svg::FIG_WIDTH,
    };
    svg::render(&frame, input)
}
