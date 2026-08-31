//! Figure: effective diversity.
//!
//! Renders `bioprism_mutation::Diversity` — PRISM Gate 3's measurement that instance count is not
//! benchmark count. The paired bars put the naive number (instances) and the honest denominator
//! (independent equivalence classes) on the same scale, and the document's own caveat string is
//! reproduced verbatim under the title, because the caveat is part of the measurement.
//!
//! The serialized `inflation_ratio` is cross-checked against `instances / equivalence_classes`;
//! a document whose ratio disagrees with its own counts is refused rather than rendered.

use crate::error::FigureError;
use crate::extract::{count_field, f64_field, str_field};
use crate::svg::{self, label, label_bold, rect, wrap_chars, ACCENT, INK, MUTED};
use serde_json::Value;

const BAR_X: f64 = 170.0;
const BAR_MAX: f64 = 400.0;
const BAR_PITCH: f64 = 28.0;

/// Render the effective-diversity figure from a serialized `Diversity` document.
pub fn mutation_diversity(input: &Value) -> Result<String, FigureError> {
    let instances = count_field(input, "", "instances")?;
    let parents = count_field(input, "", "parents")?;
    let families = count_field(input, "", "families")?;
    let signatures = count_field(input, "", "signatures")?;
    let classes = count_field(input, "", "equivalence_classes")?;
    let inflation = f64_field(input, "", "inflation_ratio")?;
    let caveat = str_field(input, "", "caveat")?;

    if classes > instances {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "equivalence_classes ({classes}) exceeds instances ({instances}); every class \
                 needs at least one instance"
            ),
        });
    }
    let expected_inflation = if classes == 0 {
        0.0
    } else {
        instances as f64 / classes as f64
    };
    if (inflation - expected_inflation).abs() > 1e-9 * expected_inflation.max(1.0) {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "inflation_ratio ({inflation}) does not equal instances / equivalence_classes \
                 ({expected_inflation})"
            ),
        });
    }

    let mut body = String::new();
    let mut y = 12.0;
    for line in wrap_chars(caveat, 118) {
        body.push_str(&label(16.0, y, 10.0, MUTED, &line));
        y += 13.0;
    }
    y += 12.0;

    let scale = instances.max(1) as f64;
    let bars = [
        ("instances", instances, MUTED),
        ("equivalence classes", classes, ACCENT),
    ];
    for (name, count, fill) in bars {
        body.push_str(&label(16.0, y + 12.0, 11.0, INK, name));
        let width = count as f64 / scale * BAR_MAX;
        if width > 0.0 {
            body.push_str(&rect(BAR_X, y, width, 16.0, &format!("fill=\"{fill}\"")));
        }
        body.push_str(&label(
            BAR_X + width + 8.0,
            y + 12.0,
            10.5,
            INK,
            &count.to_string(),
        ));
        y += BAR_PITCH;
    }

    y += 10.0;
    body.push_str(&label_bold(
        16.0,
        y,
        12.5,
        INK,
        &format!("inflation ×{inflation:.2} — instance count is not benchmark count"),
    ));
    y += 18.0;
    body.push_str(&label(
        16.0,
        y,
        10.5,
        MUTED,
        &format!(
            "{parents} audited parent(s) · {families} mutation famil(ies) · {signatures} oracle \
             signature(s)"
        ),
    ));

    let frame = svg::Frame {
        title: "Effective diversity",
        caption: "independent (parent, mutation family, oracle signature) classes are the honest \
                  denominator"
            .to_string(),
        body,
        body_height: y + 6.0,
        width: svg::FIG_WIDTH,
    };
    svg::render(&frame, input)
}
