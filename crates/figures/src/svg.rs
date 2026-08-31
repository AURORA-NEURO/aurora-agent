//! Shared SVG scaffolding: the fixed frame, palette, escaping, and text layout every figure uses.
//!
//! Everything here is deterministic string assembly. Coordinates are formatted to two decimals so
//! the same input value renders the same bytes on every platform; there is no font measurement,
//! only character-count truncation and wrapping, which is why the layout constants leave slack.

use crate::error::FigureError;
use bioprism_ids::ContentHash;
use serde_json::Value;
use std::fmt::Write as _;

/// Dark ink, drawn on a transparent background.
pub const INK: &str = "#1a1a1a";
/// The site's accent.
pub const ACCENT: &str = "#d97036";
/// Muted grey for captions, scaffolding, and de-emphasised marks.
pub const MUTED: &str = "#8a8a8a";

pub(crate) const FIG_WIDTH: f64 = 720.0;
const HEADER_HEIGHT: f64 = 60.0;
const FOOTER_HEIGHT: f64 = 30.0;

/// Characters per caption line at the caption's 11.5px size inside [`FIG_WIDTH`], with the same
/// slack the crate's other wrap widths leave instead of measuring glyphs.
const CAPTION_WRAP: usize = 118;
/// A caption may grow the header by at most two lines. The bound exists so a hostile artifact
/// cannot push the body arbitrarily far down; the earlier fix — a hard cut at 128 characters —
/// bounded the header by silently deleting the end of the caption, which is where the baseline
/// panel states the reference verdict.
const CAPTION_MAX_LINES: usize = 3;
const CAPTION_LINE_PITCH: f64 = 14.0;

pub(crate) const FONT_TEXT: &str = "system-ui, 'Segoe UI', Helvetica, Arial, sans-serif";
const FONT_MONO: &str = "ui-monospace, Consolas, 'Courier New', monospace";

/// Escape text for use in XML content and double-quoted attribute values.
///
/// All five XML-significant characters are escaped. JSON strings may also carry control
/// characters that XML 1.0 forbids outright; a figure must stay well-formed for *any* input
/// string, so tab/newline/CR become a space (layout here is single-line) and the remaining
/// controls become U+FFFD rather than invalid bytes.
pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(' '),
            c if (c as u32) < 0x20 => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}

/// Character-count truncation with an ellipsis. Truncation happens before escaping so an entity
/// can never be cut in half.
pub(crate) fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Greedy word wrap by character count. Oversized unbroken words are hard-split so hostile input
/// cannot force a line past the frame.
pub(crate) fn wrap_chars(text: &str, max: usize) -> Vec<String> {
    let max = max.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let push_word = |lines: &mut Vec<String>, current: &mut String, word: &str| {
        let mut word = word;
        while word.chars().count() > max {
            if !current.is_empty() {
                lines.push(std::mem::take(current));
            }
            let head: String = word.chars().take(max).collect();
            let split = head.len();
            lines.push(head);
            word = &word[split..];
        }
        let word_len = word.chars().count();
        let current_len = current.chars().count();
        if current.is_empty() {
            current.push_str(word);
        } else if current_len + 1 + word_len <= max {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(current));
            current.push_str(word);
        }
    };
    for word in text.split_whitespace() {
        push_word(&mut lines, &mut current, word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(crate) fn label(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_TEXT}\" font-size=\"{size}\" \
         fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

pub(crate) fn label_bold(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_TEXT}\" font-size=\"{size}\" \
         font-weight=\"600\" fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

pub(crate) fn label_italic(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_TEXT}\" font-size=\"{size}\" \
         font-style=\"italic\" fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

pub(crate) fn label_middle(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_TEXT}\" font-size=\"{size}\" \
         text-anchor=\"middle\" fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

pub(crate) fn label_end(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_TEXT}\" font-size=\"{size}\" \
         font-weight=\"600\" text-anchor=\"end\" fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

pub(crate) fn label_mono(x: f64, y: f64, size: f64, fill: &str, content: &str) -> String {
    format!(
        "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{FONT_MONO}\" font-size=\"{size}\" \
         fill=\"{fill}\">{}</text>\n",
        esc(content)
    )
}

/// `style` is a raw attribute string and must come from this crate's own constants, never from
/// input; input text reaches the document only through [`esc`].
pub(crate) fn rect(x: f64, y: f64, w: f64, h: f64, style: &str) -> String {
    format!("<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" {style}/>\n")
}

pub(crate) fn hline(x1: f64, x2: f64, y: f64, style: &str) -> String {
    format!("<line x1=\"{x1:.2}\" y1=\"{y:.2}\" x2=\"{x2:.2}\" y2=\"{y:.2}\" {style}/>\n")
}

pub(crate) struct Frame<'a> {
    pub(crate) title: &'a str,
    pub(crate) caption: String,
    pub(crate) body: String,
    pub(crate) body_height: f64,
    /// viewBox width. Every figure whose layout constants assume the fixed frame passes
    /// [`FIG_WIDTH`]; a figure whose column count comes from the input (the sweep grid) passes a
    /// width that fits what it drew, because a cell rendered past the viewBox is clipped away
    /// invisibly and a silently missing column is worse than a wide figure.
    pub(crate) width: f64,
}

/// Wrap the caption to the frame, bounded at [`CAPTION_MAX_LINES`] lines.
fn caption_lines(caption: &str) -> Vec<String> {
    let mut lines = wrap_chars(caption, CAPTION_WRAP);
    if lines.len() > CAPTION_MAX_LINES {
        lines.truncate(CAPTION_MAX_LINES);
        if let Some(last) = lines.last_mut() {
            *last = truncate_chars(&format!("{last}…"), CAPTION_WRAP);
        }
    }
    lines
}

/// Assemble the frame around a figure body and stamp the source digest.
///
/// The frame is fixed at [`FIG_WIDTH`] for every figure that asks for it, and the header grows
/// only by the lines its caption needs, so a figure whose caption fits one line renders exactly
/// the bytes it always did.
///
/// The digest is computed here, from the exact `Value` being rendered, via the workspace's single
/// canonicalisation (`bioprism_ids::ContentHash::of_value`). It is deliberately not a parameter: a
/// caller-supplied digest could disagree with the figure above it, and a figure that mislabels its
/// own source is worse than no figure.
pub(crate) fn render(frame: &Frame<'_>, input: &Value) -> Result<String, FigureError> {
    let digest = ContentHash::of_value(input).map_err(|error| FigureError::Canonicalisation {
        reason: error.to_string(),
    })?;
    let caption = caption_lines(&frame.caption);
    let header_height = HEADER_HEIGHT + (caption.len() - 1) as f64 * CAPTION_LINE_PITCH;
    let width = frame.width;
    let height = header_height + frame.body_height + FOOTER_HEIGHT;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.2}\" \
         role=\"img\" aria-label=\"{}\">",
        esc(frame.title)
    );
    let _ = writeln!(
        out,
        "<defs><pattern id=\"hatch-accent\" width=\"6\" height=\"6\" \
         patternUnits=\"userSpaceOnUse\" patternTransform=\"rotate(45)\"><line x1=\"0\" y1=\"0\" \
         x2=\"0\" y2=\"6\" stroke=\"{ACCENT}\" stroke-width=\"2.4\"/></pattern><pattern \
         id=\"hatch-muted\" width=\"6\" height=\"6\" patternUnits=\"userSpaceOnUse\" \
         patternTransform=\"rotate(45)\"><line x1=\"0\" y1=\"0\" x2=\"0\" y2=\"6\" \
         stroke=\"{MUTED}\" stroke-width=\"2.4\"/></pattern></defs>"
    );
    out.push_str(&label_bold(16.0, 26.0, 16.0, INK, frame.title));
    for (index, line) in caption.iter().enumerate() {
        out.push_str(&label(
            16.0,
            46.0 + index as f64 * CAPTION_LINE_PITCH,
            11.5,
            MUTED,
            line,
        ));
    }
    let _ = writeln!(out, "<g transform=\"translate(0,{header_height:.0})\">");
    out.push_str(&frame.body);
    out.push_str("</g>\n");
    out.push_str(&label_mono(
        16.0,
        height - 12.0,
        10.0,
        MUTED,
        &format!("source sha256: {digest}"),
    ));
    out.push_str("</svg>\n");
    Ok(out)
}
