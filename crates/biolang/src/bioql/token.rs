//! BioQL tokens.
//!
//! Keywords are *reserved*, not contextual. A contextual-keyword lexer would let a schema declare a
//! field called `where` and then produce a parse that depends on where the word sits, which is
//! exactly the class of ambiguity blueprint 25.21's release gate ("no unresolved ambiguity") is
//! asking to be designed out. The cost is a small list of names a schema may not use; the list is
//! [`Keyword::ALL`] and it is checkable.

use crate::span::Span;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Reserved words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyword {
    Select,
    From,
    In,
    Where,
    Expand,
    At,
    Labels,
    Aggregate,
    Cost,
    And,
    Or,
    Not,
    True,
    False,
    Ontology,
    Release,
    Policy,
    Provenance,
    Limit,
    Instant,
}

impl Keyword {
    pub const ALL: [Keyword; 20] = [
        Keyword::Select,
        Keyword::From,
        Keyword::In,
        Keyword::Where,
        Keyword::Expand,
        Keyword::At,
        Keyword::Labels,
        Keyword::Aggregate,
        Keyword::Cost,
        Keyword::And,
        Keyword::Or,
        Keyword::Not,
        Keyword::True,
        Keyword::False,
        Keyword::Ontology,
        Keyword::Release,
        Keyword::Policy,
        Keyword::Provenance,
        Keyword::Limit,
        Keyword::Instant,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Keyword::Select => "select",
            Keyword::From => "from",
            Keyword::In => "in",
            Keyword::Where => "where",
            Keyword::Expand => "expand",
            Keyword::At => "at",
            Keyword::Labels => "labels",
            Keyword::Aggregate => "aggregate",
            Keyword::Cost => "cost",
            Keyword::And => "and",
            Keyword::Or => "or",
            Keyword::Not => "not",
            Keyword::True => "true",
            Keyword::False => "false",
            Keyword::Ontology => "ontology",
            Keyword::Release => "release",
            Keyword::Policy => "policy",
            Keyword::Provenance => "provenance",
            Keyword::Limit => "limit",
            Keyword::Instant => "instant",
        }
    }

    pub fn lookup(text: &str) -> Option<Keyword> {
        Keyword::ALL.into_iter().find(|kw| kw.as_str() == text)
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a token is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "token", rename_all = "snake_case")]
pub enum TokenKind {
    Keyword(Keyword),
    Ident(String),
    /// A numeric literal, with the source text kept so a diagnostic can quote it exactly.
    Number {
        value: f64,
        text: String,
        integral: bool,
    },
    Text(String),
    Star,
    Comma,
    Dot,
    Colon,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Slash,
    /// The end of input. A real token so that "unexpected end" carries a position.
    End,
}

impl TokenKind {
    /// How this token appears in a diagnostic.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Keyword(kw) => format!("keyword `{kw}`"),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Number { text, .. } => format!("number `{text}`"),
            TokenKind::Text(value) => format!("string \"{value}\""),
            TokenKind::Star => "`*`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Dot => "`.`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::Equal => "`==`".to_string(),
            TokenKind::NotEqual => "`!=`".to_string(),
            TokenKind::Less => "`<`".to_string(),
            TokenKind::LessEqual => "`<=`".to_string(),
            TokenKind::Greater => "`>`".to_string(),
            TokenKind::GreaterEqual => "`>=`".to_string(),
            TokenKind::Plus => "`+`".to_string(),
            TokenKind::Minus => "`-`".to_string(),
            TokenKind::Slash => "`/`".to_string(),
            TokenKind::End => "end of query".to_string(),
        }
    }

    pub fn is_end(&self) -> bool {
        matches!(self, TokenKind::End)
    }
}

/// A token and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    pub fn keyword(&self) -> Option<Keyword> {
        match &self.kind {
            TokenKind::Keyword(kw) => Some(*kw),
            _ => None,
        }
    }

    pub fn ident(&self) -> Option<&str> {
        match &self.kind {
            TokenKind::Ident(name) => Some(name.as_str()),
            _ => None,
        }
    }
}
