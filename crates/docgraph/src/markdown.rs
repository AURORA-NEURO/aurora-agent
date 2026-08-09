//! A deliberately small Markdown reader: front matter, headings, links.
//!
//! There is no Markdown crate in this workspace and none is being added. What the registry needs
//! from a document is its front matter, its H1, its section headings and its outgoing links —
//! four things a hand-rolled reader gets right, in keeping with the CSV reader, argument parser,
//! JSON-RPC framing and PRNG that are already hand-rolled here.
//!
//! # What this reader does not understand
//!
//! Setext headings (`Title` underlined with `===`), reference-style links (`[text][label]` with
//! the target defined elsewhere), HTML anchors, tables, and every kind of inline emphasis. A
//! document using setext headings will be reported as having no H1 by [`crate::lint`], which is a
//! true statement about what this reader can resolve and a false one about the document. That is
//! the trade: the checker is honest about its own reach rather than silently guessing.
//!
//! It does understand fenced code blocks, because it must. A `# ` line inside a fence is code,
//! and a reader that counted it as a heading would report a structure the document does not have
//! — and would do it most often in exactly the documents that matter, the ones with examples.

use crate::error::DocGraphError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// YAML-ish front matter: a `---` fence, `key: value` lines, a closing `---`.
///
/// Not a YAML parser. Nested maps, sequences, anchors and multi-line scalars are not supported;
/// a line without a `:` is skipped rather than guessed at. The blueprint's front matter is flat
/// `key: "value"` throughout, so the supported subset is the whole observed language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontMatter {
    pub fields: BTreeMap<String, String>,
}

impl FrontMatter {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

/// A parsed document: front matter plus the body that followed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDoc<'a> {
    /// `None` when the file did not open with a `---` fence — a real state for this repository's
    /// own `docs/`, which carries no front matter at all.
    pub front_matter: Option<FrontMatter>,
    pub body: &'a str,
}

/// Split front matter from body.
///
/// An opening `---` with no closing `---` is an error rather than a document with no front
/// matter: the two look identical to a naive reader and mean opposite things, and treating an
/// unterminated fence as "no front matter" would silently swallow the whole file's metadata.
pub fn parse_document<'a>(path: &str, text: &'a str) -> Result<ParsedDoc<'a>, DocGraphError> {
    let without_bom = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(rest) = without_bom
        .strip_prefix("---\n")
        .or_else(|| without_bom.strip_prefix("---\r\n"))
    else {
        return Ok(ParsedDoc {
            front_matter: None,
            body: without_bom,
        });
    };

    let mut fields = BTreeMap::new();
    let mut consumed = 0usize;
    let mut closed = false;
    for line in rest.split_inclusive('\n') {
        consumed += line.len();
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed.trim_end() == "---" {
            closed = true;
            break;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            fields.insert(key.to_string(), unquote(value.trim()).to_string());
        }
    }
    if !closed {
        return Err(DocGraphError::MalformedFrontMatter {
            path: path.to_string(),
            reason: "opening `---` with no closing `---`",
        });
    }
    Ok(ParsedDoc {
        front_matter: Some(FrontMatter { fields }),
        body: &rest[consumed..],
    })
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// ATX headings outside fenced code blocks, in document order.
pub fn headings(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        match fence {
            Some(marker) => {
                if trimmed.starts_with(marker) {
                    fence = None;
                }
                continue;
            }
            None => {
                if trimmed.starts_with("```") {
                    fence = Some("```");
                    continue;
                }
                if trimmed.starts_with("~~~") {
                    fence = Some("~~~");
                    continue;
                }
            }
        }
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 {
            continue;
        }
        let after = &trimmed[hashes..];
        if !after.starts_with(' ') {
            continue;
        }
        out.push(Heading {
            level: hashes as u8,
            text: after.trim().to_string(),
        });
    }
    out
}

/// The first level-1 heading, which 41.01 requires every node to resolve to.
pub fn first_h1(body: &str) -> Option<String> {
    headings(body)
        .into_iter()
        .find(|heading| heading.level == 1)
        .map(|heading| heading.text)
}

/// Inline link targets, outside fenced code blocks.
///
/// Matches `](target)` only. Reference-style links and bare autolinks are not resolved, so a
/// corpus using them will produce fewer `references` edges than it has links — under-reporting,
/// which costs the linter recall and never invents an edge that is not there.
pub fn link_targets(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        match fence {
            Some(marker) => {
                if trimmed.starts_with(marker) {
                    fence = None;
                }
                continue;
            }
            None => {
                if trimmed.starts_with("```") {
                    fence = Some("```");
                    continue;
                }
                if trimmed.starts_with("~~~") {
                    fence = Some("~~~");
                    continue;
                }
            }
        }
        let bytes = line.as_bytes();
        let mut index = 0usize;
        while index + 1 < bytes.len() {
            if bytes[index] == b']' && bytes[index + 1] == b'(' {
                let start = index + 2;
                let mut depth = 1usize;
                let mut cursor = start;
                while cursor < bytes.len() {
                    match bytes[cursor] {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    cursor += 1;
                }
                if depth == 0 && cursor <= bytes.len() {
                    let target = line[start..cursor].trim();
                    let target = target.split_whitespace().next().unwrap_or(target);
                    if !target.is_empty() {
                        out.push(target.to_string());
                    }
                    index = cursor + 1;
                    continue;
                }
            }
            index += 1;
        }
    }
    out
}

/// The first paragraph after the H1: the module's own brief.
///
/// Used as [`ProfileLevel::Brief`](crate::tokens::ProfileLevel::Brief) text when a document does
/// not carry an explicit summary field. Blank-line delimited, headings and fences skipped.
pub fn first_paragraph(body: &str) -> Option<String> {
    let mut paragraph: Vec<&str> = Vec::new();
    let mut fence: Option<&str> = None;
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if paragraph.is_empty() {
                fence = Some(if trimmed.starts_with("```") {
                    "```"
                } else {
                    "~~~"
                });
                continue;
            }
            break;
        }
        if trimmed.starts_with('#') {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.is_empty() {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        paragraph.push(trimmed);
    }
    if paragraph.is_empty() {
        None
    } else {
        Some(paragraph.join(" "))
    }
}
