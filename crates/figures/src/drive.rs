//! Figure: an autopilot drive's attempt sequence.
//!
//! Renders `bioprism_autopilot`'s report document. The attempts are laid out in **logical order**
//! and the axis says so — "attempt sequence (logical, clock-free)" — because the autopilot kernel
//! owns no wall clock and a time axis would fabricate measurements the report does not contain.
//! (40.36 supplies the retry-classification vocabulary the report carries; the report document
//! itself is the autopilot crate's design, and this figure follows that document, not the
//! blueprint.)
//!
//! An attempt without a parsed mission report is drawn as "no report", never as a failure: an
//! undelivered dispatch leaves the outcome unknown at mission level, and unknown is not failed.
//!
//! The caption's "attempts used N of M" is cross-checked against the attempts it draws below it,
//! the way the baseline panel cross-checks `admissible` against the verdict fields it is defined
//! from: a report whose totals disagree with its own array would put a count in the caption that
//! the figure contradicts, so it is refused rather than rendered.

use crate::error::FigureError;
use crate::extract::{array_field, count_field, path, require, str_field};
use crate::svg::{
    self, hline, label, label_bold, label_italic, label_middle, rect, truncate_chars, ACCENT, INK,
    MUTED,
};
use serde_json::Value;

const BOXES_PER_ROW: usize = 4;
const BOX_W: f64 = 150.0;
const BOX_H: f64 = 56.0;
const X_PITCH: f64 = 172.0;
const Y_PITCH: f64 = 80.0;
const BOXES_TOP: f64 = 34.0;

const AXIS_LABEL: &str = "attempt sequence (logical, clock-free)";

struct Attempt {
    index: u64,
    kind: String,
    mission_status: Option<String>,
    transport_error: bool,
}

/// Render the drive-receipt sequence from an autopilot report document.
pub fn autopilot_drive(input: &Value) -> Result<String, FigureError> {
    let final_status = str_field(input, "", "final_status")?;
    if !matches!(final_status, "succeeded" | "exhausted" | "refused") {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "final_status is `{final_status}`, but an autopilot report may only end \
                 succeeded, exhausted, or refused"
            ),
        });
    }
    let base_mission_id = str_field(input, "", "base_mission_id")?;
    let totals = require(input, "", "totals")?;
    let attempts_used = count_field(totals, "totals", "attempts_used")?;
    let max_attempts = count_field(totals, "totals", "max_attempts")?;
    let steps_in_plan = count_field(totals, "totals", "steps_in_plan")?;

    let attempt_values = array_field(input, "", "attempts")?;
    let mut attempts = Vec::with_capacity(attempt_values.len());
    for (index, attempt) in attempt_values.iter().enumerate() {
        let parent = format!("attempts[{index}]");
        let attempt_index = count_field(attempt, &parent, "attempt_index")?;
        let kind = str_field(attempt, &parent, "kind")?.to_string();
        let mission_status = match require(attempt, &parent, "outcome_summary")? {
            Value::Null => None,
            summary @ Value::Object(_) => Some(
                str_field(summary, &path(&parent, "outcome_summary"), "mission_status")?
                    .to_string(),
            ),
            _ => {
                return Err(FigureError::WrongType {
                    field: path(&parent, "outcome_summary"),
                    expected: "an object or null",
                })
            }
        };
        let transport_error = match require(attempt, &parent, "dispatch_error")? {
            Value::Null => false,
            Value::String(_) => true,
            _ => {
                return Err(FigureError::WrongType {
                    field: path(&parent, "dispatch_error"),
                    expected: "a string or null",
                })
            }
        };
        attempts.push(Attempt {
            index: attempt_index,
            kind,
            mission_status,
            transport_error,
        });
    }

    if attempts_used != attempts.len() as u64 {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "totals.attempts_used claims {attempts_used} attempt(s) but the attempts array \
                 holds {}",
                attempts.len()
            ),
        });
    }
    if attempts_used > max_attempts {
        return Err(FigureError::Inconsistent {
            reason: format!(
                "totals.attempts_used is {attempts_used} but totals.max_attempts is \
                 {max_attempts}, so the caption would state a count the budget forbids"
            ),
        });
    }

    let mut body = String::new();
    let badge_text = format!("final: {final_status}");
    let badge_w = badge_text.chars().count() as f64 * 6.2 + 14.0;
    let badge_style = match final_status {
        "succeeded" => format!("rx=\"4\" fill=\"{ACCENT}\""),
        "exhausted" => format!("rx=\"4\" fill=\"{MUTED}\""),
        _ => format!("rx=\"4\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"1.5\""),
    };
    body.push_str(&rect(704.0 - badge_w, 2.0, badge_w, 20.0, &badge_style));
    body.push_str(&label_middle(
        704.0 - badge_w / 2.0,
        16.0,
        10.5,
        INK,
        &badge_text,
    ));

    let rows_used = if attempts.is_empty() {
        1
    } else {
        attempts.len().div_ceil(BOXES_PER_ROW)
    };
    if attempts.is_empty() {
        body.push_str(&label_italic(
            16.0,
            BOXES_TOP + 24.0,
            10.5,
            MUTED,
            "no attempts were dispatched",
        ));
    }
    for (position, attempt) in attempts.iter().enumerate() {
        let row = position / BOXES_PER_ROW;
        let col = position % BOXES_PER_ROW;
        let x = 16.0 + col as f64 * X_PITCH;
        let y = BOXES_TOP + row as f64 * Y_PITCH;
        if col > 0 {
            body.push_str(&label_middle(x - 11.0, y + BOX_H / 2.0 + 4.0, 12.0, MUTED, "→"));
        }
        let style = match &attempt.mission_status {
            Some(status) if status == "succeeded" => {
                format!("fill=\"{ACCENT}\" stroke=\"{INK}\" stroke-width=\"0.75\"")
            }
            Some(_) => format!("fill=\"url(#hatch-muted)\" stroke=\"{MUTED}\" stroke-width=\"1\""),
            None => format!(
                "fill=\"none\" stroke=\"{MUTED}\" stroke-width=\"1\" stroke-dasharray=\"4 3\""
            ),
        };
        body.push_str(&rect(x, y, BOX_W, BOX_H, &style));
        body.push_str(&label_bold(
            x + 8.0,
            y + 17.0,
            11.0,
            INK,
            &truncate_chars(&format!("#{} · {}", attempt.index, attempt.kind), 21),
        ));
        match &attempt.mission_status {
            Some(status) => {
                body.push_str(&label(
                    x + 8.0,
                    y + 33.0,
                    10.5,
                    INK,
                    &truncate_chars(status, 22),
                ));
            }
            None => {
                body.push_str(&label_italic(x + 8.0, y + 33.0, 10.5, MUTED, "no report"));
            }
        }
        if attempt.transport_error {
            body.push_str(&label_italic(
                x + 8.0,
                y + 47.0,
                9.5,
                MUTED,
                "outcome unknown (transport)",
            ));
        }
    }

    let axis_y = BOXES_TOP + rows_used as f64 * Y_PITCH + 2.0;
    body.push_str(&hline(
        16.0,
        696.0,
        axis_y,
        &format!("stroke=\"{MUTED}\" stroke-width=\"1\""),
    ));
    body.push_str(&label_middle(702.0, axis_y + 3.5, 10.0, MUTED, "→"));
    body.push_str(&label_middle(360.0, axis_y + 16.0, 10.5, MUTED, AXIS_LABEL));

    let frame = svg::Frame {
        title: "Autopilot drive",
        caption: format!(
            "base mission {base_mission_id} · attempts used {attempts_used} of {max_attempts} · \
             {steps_in_plan} step(s) in plan · final status {final_status}"
        ),
        body,
        body_height: axis_y + 24.0,
        width: svg::FIG_WIDTH,
    };
    svg::render(&frame, input)
}
