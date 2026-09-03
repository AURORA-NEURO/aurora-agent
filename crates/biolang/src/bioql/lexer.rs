//! A hand-written lexer for BioQL.
//!
//! Hand-written because the workspace builds offline against pinned dependencies and there is no
//! parser generator to reach for — the same reason the CSV reader, the argument parser, the JSON-RPC
//! layer and the PRNG here are hand-rolled. It also keeps the diagnostics: a generated lexer's error
//! positions are a property of the generator, and 25.21's release gate asks for explicitness.
//!
//! The lexer is total over its input in the sense that it either returns every token or returns one
//! [`LexError`] naming a position. It never skips a character it does not understand.
//!
//! # Two decisions worth knowing
//!
//! **Numbers do not carry their unit.** `12.5 mm` lexes as a number followed by an identifier. The
//! *parser* decides whether that identifier is a unit suffix, by asking
//! [`bioprism_standards::Unit::parse`]. Doing it here would require the lexer to know the unit table,
//! and would make `where n > 3 and x` depend on whether `and` happened to be a unit.
//!
//! **Comments are `--` to end of line.** There is no block comment, because an unterminated block
//! comment silently eats the rest of a query and this language's whole posture is that silence is
//! the enemy.

use crate::bioql::token::{Keyword, Token, TokenKind};
use crate::error::LexError;
use crate::span::Span;

/// Turns query source into tokens, ending with exactly one [`TokenKind::End`].
pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    offset: usize,
    line: u32,
    column: u32,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Lexer {
            source,
            bytes: source.as_bytes(),
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            if self.offset >= self.bytes.len() {
                let span = self.point_span();
                tokens.push(Token::new(TokenKind::End, span));
                return Ok(tokens);
            }
            tokens.push(self.next_token()?);
        }
    }

    fn skip_trivia(&mut self) {
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if byte == b'\n' {
                self.offset += 1;
                self.line += 1;
                self.column = 1;
            } else if byte.is_ascii_whitespace() {
                self.offset += 1;
                self.column += 1;
            } else if byte == b'-' && self.bytes.get(self.offset + 1) == Some(&b'-') {
                while self.offset < self.bytes.len() && self.bytes[self.offset] != b'\n' {
                    self.offset += 1;
                    self.column += 1;
                }
            } else {
                return;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.offset;
        let line = self.line;
        let column = self.column;
        let byte = self.bytes[start];

        if byte == b'"' {
            return self.lex_string(start, line, column);
        }
        if byte.is_ascii_digit() {
            return self.lex_number(start, line, column);
        }
        if byte == b'_' || byte.is_ascii_alphabetic() {
            return Ok(self.lex_word(start, line, column));
        }

        let (kind, width) = match byte {
            b'*' => (TokenKind::Star, 1),
            b',' => (TokenKind::Comma, 1),
            b'.' => (TokenKind::Dot, 1),
            b':' => (TokenKind::Colon, 1),
            b'{' => (TokenKind::LBrace, 1),
            b'}' => (TokenKind::RBrace, 1),
            b'(' => (TokenKind::LParen, 1),
            b')' => (TokenKind::RParen, 1),
            b'+' => (TokenKind::Plus, 1),
            b'-' => (TokenKind::Minus, 1),
            b'/' => (TokenKind::Slash, 1),
            b'=' if self.bytes.get(start + 1) == Some(&b'=') => (TokenKind::Equal, 2),
            b'!' if self.bytes.get(start + 1) == Some(&b'=') => (TokenKind::NotEqual, 2),
            b'<' if self.bytes.get(start + 1) == Some(&b'=') => (TokenKind::LessEqual, 2),
            b'>' if self.bytes.get(start + 1) == Some(&b'=') => (TokenKind::GreaterEqual, 2),
            b'<' => (TokenKind::Less, 1),
            b'>' => (TokenKind::Greater, 1),
            _ => {
                let found = self.source[start..].chars().next().unwrap_or('\u{fffd}');
                let span = Span::new(start, start + found.len_utf8(), line, column);
                return Err(LexError::UnexpectedCharacter { found, span });
            }
        };

        self.offset += width;
        self.column += width as u32;
        Ok(Token::new(
            kind,
            Span::new(start, self.offset, line, column),
        ))
    }

    fn lex_word(&mut self, start: usize, line: u32, column: u32) -> Token {
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if byte == b'_' || byte.is_ascii_alphanumeric() {
                self.offset += 1;
                self.column += 1;
            } else {
                break;
            }
        }
        let text = &self.source[start..self.offset];
        let span = Span::new(start, self.offset, line, column);
        match Keyword::lookup(text) {
            Some(kw) => Token::new(TokenKind::Keyword(kw), span),
            None => Token::new(TokenKind::Ident(text.to_string()), span),
        }
    }

    fn lex_number(&mut self, start: usize, line: u32, column: u32) -> Result<Token, LexError> {
        let mut integral = true;
        while self.offset < self.bytes.len() && self.bytes[self.offset].is_ascii_digit() {
            self.offset += 1;
            self.column += 1;
        }
        if self.offset < self.bytes.len()
            && self.bytes[self.offset] == b'.'
            && self
                .bytes
                .get(self.offset + 1)
                .is_some_and(u8::is_ascii_digit)
        {
            integral = false;
            self.offset += 1;
            self.column += 1;
            while self.offset < self.bytes.len() && self.bytes[self.offset].is_ascii_digit() {
                self.offset += 1;
                self.column += 1;
            }
        }
        if self.offset < self.bytes.len() && matches!(self.bytes[self.offset], b'e' | b'E') {
            let mut lookahead = self.offset + 1;
            if matches!(self.bytes.get(lookahead), Some(b'+') | Some(b'-')) {
                lookahead += 1;
            }
            if self.bytes.get(lookahead).is_some_and(u8::is_ascii_digit) {
                integral = false;
                while lookahead < self.bytes.len() && self.bytes[lookahead].is_ascii_digit() {
                    lookahead += 1;
                }
                self.column += (lookahead - self.offset) as u32;
                self.offset = lookahead;
            }
        }

        let text = &self.source[start..self.offset];
        let span = Span::new(start, self.offset, line, column);
        let value: f64 =
            text.parse().map_err(
                |error: std::num::ParseFloatError| LexError::MalformedNumber {
                    text: text.to_string(),
                    span,
                    detail: error.to_string(),
                },
            )?;
        if !value.is_finite() {
            return Err(LexError::MalformedNumber {
                text: text.to_string(),
                span,
                detail: "literal is not finite; a non-finite value cannot be canonically hashed"
                    .to_string(),
            });
        }
        Ok(Token::new(
            TokenKind::Number {
                value,
                text: text.to_string(),
                integral,
            },
            span,
        ))
    }

    fn lex_string(&mut self, start: usize, line: u32, column: u32) -> Result<Token, LexError> {
        self.offset += 1;
        self.column += 1;
        let mut value = String::new();
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            match byte {
                b'"' => {
                    self.offset += 1;
                    self.column += 1;
                    return Ok(Token::new(
                        TokenKind::Text(value),
                        Span::new(start, self.offset, line, column),
                    ));
                }
                b'\\' => {
                    let escape = self.bytes.get(self.offset + 1).copied();
                    let replacement = match escape {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        other => {
                            let text = other
                                .map(|b| format!("\\{}", b as char))
                                .unwrap_or_else(|| "\\".to_string());
                            return Err(LexError::UnknownEscape {
                                escape: text,
                                span: Span::new(
                                    self.offset,
                                    self.offset + 2,
                                    self.line,
                                    self.column,
                                ),
                            });
                        }
                    };
                    value.push(replacement);
                    self.offset += 2;
                    self.column += 2;
                }
                b'\n' => {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.offset, line, column),
                    })
                }
                _ => {
                    let ch = self.source[self.offset..]
                        .chars()
                        .next()
                        .unwrap_or('\u{fffd}');
                    value.push(ch);
                    self.offset += ch.len_utf8();
                    self.column += 1;
                }
            }
        }
        Err(LexError::UnterminatedString {
            span: Span::new(start, self.offset, line, column),
        })
    }

    fn point_span(&self) -> Span {
        Span::new(self.offset, self.offset, self.line, self.column)
    }
}
