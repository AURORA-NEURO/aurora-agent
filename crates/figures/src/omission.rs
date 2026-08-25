//! Figure: reference omission accounting.
//!
//! Renders a certificate's `omissions` block — the v0.1 count-and-a-string summary of
//! `ReferenceOmissions` — as labeled segments, with the classification string reproduced
//! verbatim. The figure repeats the summary's own limitation rather than hiding it: a
//! classification string cannot distinguish "provably cannot matter" from "nobody checked",
//! which is the distinction 43.26 requires and the extended certificate profile exists to add.

use crate::error::FigureError;
use crate::extract::{array_field, count_field, path, require, str_field};
use crate::svg::{
    self, label, label_bold, label_italic, label_mono, rect, wrap_chars, ACCENT, INK, MUTED,
};
use serde_json::Value;

const STRIP_X: f64 = 16.0;
const STRIP_W: f64 = 560.0;
const STRIP_H: f64 = 22.0;
const LISTED_IDS_MAX: usize = 6;

/// Carried verbatim on every rendering because it is a property of the v0.1 summary itself, not
/// of any particular certificate.
const V01_SUMMARY_CAVEAT: &str =
    "A v0.1 omission summary is a count and a classification string; it cannot distinguish \
     'provably cannot matter' from 'nobody checked' — the distinction 43.26 requires and the \
     extended certificate profile exists to add.";

/// Render the omission accounting from a certificate document's `omissions` block.
pub fn omission_accounting(input: &Value) -> Result<String, FigureError> {
    let world_id = str_field(input, "", "world_id")?;
    let query_id = str_field(input, "", "query_id")?;
    let omissions = require(input, "", "omissions")?;
    let total = count_field(omissions, "omissions", "total_facts")?;
    let exploratory = count_field(omissions, "omissions", "exploratory_facts")?;
    let classification = str_field(omissions, "omissions", "classification")?;
    let inaccessible_values = array_field(omissions, "omissions", "inaccessible_selected_before_cut")?;
    let mut inaccessible = Vec::with_capacity(inaccessible_values.len());
    for (index, entry) in inaccessible_values.iter().enumerate() {
        let id = entry.as_str().ok_or_else(|| FigureError::WrongType {
            field: format!(
                "{}[{index}]",
                path("omissions", "inaccessible_selected_before_cut")
            ),
            expected: "a string",
        })?;
        inaccessible.push(id);
    }
    if exploratory > total {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "omissions.exploratory_facts ({exploratory}) exceeds omissions.total_facts \
                 ({total})"
            ),
        });
    }
    let non_exploratory = total - exploratory;

    let mut body = String::new();
    body.push_str(&label_bold(
        STRIP_X,
        10.0,
        12.0,
        INK,
        &format!("omitted facts: {total}"),
    ));
    if total == 0 {
        body.push_str(&label_italic(
            STRIP_X,
            32.0,
            10.5,
            MUTED,
            "0 facts omitted — the compiled context left nothing out",
        ));
    } else {
        body.push_str(&rect(
            STRIP_X,
            18.0,
            STRIP_W,
            STRIP_H,
            &format!("fill=\"none\" stroke=\"{MUTED}\" stroke-width=\"1\""),
        ));
        let exploratory_width = exploratory as f64 / total as f64 * STRIP_W;
        if exploratory > 0 {
            body.push_str(&rect(
                STRIP_X,
                18.0,
                exploratory_width.max(1.0),
                STRIP_H,
                &format!("fill=\"{MUTED}\""),
            ));
        }
        if non_exploratory > 0 {
            let width = (non_exploratory as f64 / total as f64 * STRIP_W).max(1.0);
            body.push_str(&rect(
                STRIP_X + exploratory_width,
                18.0,
                width.min(STRIP_W - exploratory_width),
                STRIP_H,
                &format!("fill=\"{ACCENT}\""),
            ));
        }
    }
    body.push_str(&rect(16.0, 50.0, 12.0, 12.0, &format!("fill=\"{MUTED}\"")));
    body.push_str(&label(
        34.0,
        60.0,
        10.5,
        INK,
        &format!("exploratory: {exploratory}"),
    ));
    body.push_str(&rect(250.0, 50.0, 12.0, 12.0, &format!("fill=\"{ACCENT}\"")));
    body.push_str(&label(
        268.0,
        60.0,
        10.5,
        INK,
        &format!("non-exploratory: {non_exploratory}"),
    ));

    let mut y = 86.0;
    body.push_str(&label(16.0, y, 10.0, MUTED, "classification (verbatim):"));
    y += 15.0;
    for line in wrap_chars(classification, 92) {
        body.push_str(&label_mono(16.0, y, 10.5, INK, &line));
        y += 14.0;
    }

    y += 8.0;
    let inaccessible_text = if inaccessible.is_empty() {
        "inaccessible-but-selected before the cut: none".to_string()
    } else {
        let listed = inaccessible
            .iter()
            .take(LISTED_IDS_MAX)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        let overflow = inaccessible.len().saturating_sub(LISTED_IDS_MAX);
        if overflow == 0 {
            format!(
                "inaccessible-but-selected before the cut: {} — {listed}",
                inaccessible.len()
            )
        } else {
            format!(
                "inaccessible-but-selected before the cut: {} — {listed} … and {overflow} more \
                 not shown",
                inaccessible.len()
            )
        }
    };
    for line in wrap_chars(&inaccessible_text, 108) {
        body.push_str(&label(16.0, y, 10.5, INK, &line));
        y += 14.0;
    }

    y += 8.0;
    for line in wrap_chars(V01_SUMMARY_CAVEAT, 118) {
        body.push_str(&label(16.0, y, 10.0, MUTED, &line));
        y += 13.0;
    }

    let frame = svg::Frame {
        title: "Reference omission accounting",
        caption: format!("world {world_id} · query {query_id}"),
        body,
        body_height: y + 2.0,
        width: svg::FIG_WIDTH,
    };
    svg::render(&frame, input)
}
