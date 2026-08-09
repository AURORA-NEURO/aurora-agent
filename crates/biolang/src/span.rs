//! Source positions, so a refusal can point at the thing it refused.
//!
//! Blueprint 25.21 requires a "query parser" and a "type checker" under validation and conformance
//! but says nothing about diagnostics. The choice made here is that a type error which cannot say
//! *where* is not usable by the author, and an incomparability that cannot say *which two operands*
//! is not usable by anyone. Every [`crate::bioql`] error therefore carries a [`Span`].
//!
//! Spans are byte offsets into the original source plus a 1-based line and column, computed at lex
//! time. Columns count Unicode scalar values, not bytes, because a column number that lands in the
//! middle of a multi-byte character is worse than no column number.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A half-open byte range in the query source, with a human position for its start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset one past the last character.
    pub end: usize,
    /// 1-based line of `start`.
    pub line: u32,
    /// 1-based column of `start`, counted in characters.
    pub column: u32,
}

impl Span {
    pub const fn new(start: usize, end: usize, line: u32, column: u32) -> Self {
        Span {
            start,
            end,
            line,
            column,
        }
    }

    /// A span covering both, used when an error is about a binary operator's two operands.
    pub fn merge(self, other: Span) -> Span {
        let (line, column) = if self.start <= other.start {
            (self.line, self.column)
        } else {
            (other.line, other.column)
        };
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line,
            column,
        }
    }

    /// The source text this span covers, when the caller still has the source.
    pub fn slice<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.start..self.end)
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}
