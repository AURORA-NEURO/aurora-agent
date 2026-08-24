//! The WeaveLang lexer.
//!
//! Blueprint 23.37 ("Lexical structure") specifies: UTF-8 source, `kebab-case` identifiers,
//! package-qualified names of the shape `namespace:name/item@version`, `//` and `/* ... */`
//! comments, quoted strings, durations suffixed `ms`/`s`/`m`/`h`/`d`, and hashes written with an
//! explicit algorithm such as `sha256:...`.
//!
//! Two of those rules interact badly and the resolution is worth stating, because it is the only
//! place the lexer is cleverer than a textbook one.
//!
//! **Kebab-case versus the arrow.** `a-b` is one identifier but `a -> b` is three tokens, and
//! WeaveLang writes both without reliable spacing. The rule here is that a `-` continues an
//! identifier only when the character *after* it is alphanumeric, so `first-valid` lexes as one
//! identifier and `a->b` lexes as three tokens. There is no backtracking.
//!
//! **`//` is both a comment and a URI scheme separator.** 23.37 specifies `//` line comments and
//! also writes `minimum-profile prism://capability/challenge >= 0.80`. Taken literally the second
//! is a comment and the role declaration never closes. The rule adopted here is the narrowest one
//! that keeps both examples working: `//` immediately after a `:` is a scheme separator, anywhere
//! else it opens a comment. This is a genuine conflict inside 23.37, not an under-specification,
//! and it is resolved rather than reported because either choice alone breaks one of its examples.
//!
//! **Qualified names are not lexed.** `aurora:reliable-repair@0.1.0`, `sha256:program` and
//! `prism://capability/challenge` all look like a single lexeme, but `Reviewer: propose<Plan>` in
//! the choreography grammar puts a colon directly after an identifier where a qualified name is
//! *not* intended. A lexer cannot tell those apart without knowing the grammar, so it does not
//! try: `:`, `/`, `@` and `.` are ordinary punctuation, and the parser reassembles a qualified name
//! from adjacent tokens using their spans. That keeps the lexer context-free and puts the
//! ambiguity where the grammar can resolve it.
//!
//! Not implemented, deliberately: no string interpolation (23.04 writes
//! `repo.write:branch/${thread}` in an effect pattern, but that is an effect-pattern syntax, not a
//! WeaveLang string), no raw strings, no nested block comments, and no numeric bases other than
//! decimal. 23.37 asks for none of them.

use crate::diagnostic::{Diagnostic, Span};

/// What a token is, before the grammar gives it meaning.
///
/// Keywords are *not* a token kind. 23.37's reserved list contains kebab-case words like
/// `decision-cell` and role-position words like `human` that are also plausible identifiers in
/// other positions, so keyword recognition is done by the parser against
/// [`is_reserved`], which keeps the token set small and the reserved list a single readable table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// A kebab-case name, possibly a reserved word.
    Ident,
    /// A decimal integer, `_` separators removed.
    Integer,
    /// A decimal number with exactly one fraction part.
    Float,
    /// Three dot-separated integer components, as in `0.1.0`. Never a number.
    Version,
    /// An integer with a `ms`/`s`/`m`/`h`/`d` suffix.
    Duration,
    /// A double-quoted string, escapes already decoded.
    Str,
    Arrow,
    FatArrow,
    LessEq,
    GreaterEq,
    Less,
    Greater,
    Equals,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    At,
    Slash,
    Plus,
    Minus,
    /// The synthetic token at the end of input. Always present, so the parser never indexes past
    /// the end and every "unexpected end of input" error still carries a span.
    Eof,
}

impl TokenKind {
    /// A human-readable name for use in "expected X, found Y" messages.
    pub fn describe(self) -> &'static str {
        match self {
            TokenKind::Ident => "an identifier",
            TokenKind::Integer => "an integer",
            TokenKind::Float => "a number",
            TokenKind::Version => "a version",
            TokenKind::Duration => "a duration",
            TokenKind::Str => "a string",
            TokenKind::Arrow => "`->`",
            TokenKind::FatArrow => "`=>`",
            TokenKind::LessEq => "`<=`",
            TokenKind::GreaterEq => "`>=`",
            TokenKind::Less => "`<`",
            TokenKind::Greater => "`>`",
            TokenKind::Equals => "`=`",
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBracket => "`[`",
            TokenKind::RBracket => "`]`",
            TokenKind::Comma => "`,`",
            TokenKind::Colon => "`:`",
            TokenKind::Semicolon => "`;`",
            TokenKind::Dot => "`.`",
            TokenKind::At => "`@`",
            TokenKind::Slash => "`/`",
            TokenKind::Plus => "`+`",
            TokenKind::Minus => "`-`",
            TokenKind::Eof => "end of input",
        }
    }
}

/// One lexeme with its decoded text and its position in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    /// The token's value: the identifier, the digits with `_` removed, or the decoded string
    /// body without its quotes.
    pub text: String,
    pub span: Span,
}

impl Token {
    /// Whether this is the identifier `word`. The parser's keyword test.
    pub fn is_word(&self, word: &str) -> bool {
        self.kind == TokenKind::Ident && self.text == word
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LexError {
    #[error("unexpected character `{character}` at {span}")]
    UnexpectedCharacter { character: char, span: Span },

    #[error("string starting at {span} is never closed")]
    UnterminatedString { span: Span },

    #[error("block comment starting at {span} is never closed")]
    UnterminatedComment { span: Span },

    #[error("`{text}` at {span} has more than three version components")]
    MalformedVersion { text: String, span: Span },

    #[error("unknown escape `\\{character}` at {span}")]
    UnknownEscape { character: char, span: Span },
}

impl Diagnostic for LexError {
    fn code(&self) -> &'static str {
        match self {
            LexError::UnexpectedCharacter { .. } => "WEAVE-E1001",
            LexError::UnterminatedString { .. } => "WEAVE-E1002",
            LexError::UnterminatedComment { .. } => "WEAVE-E1003",
            LexError::MalformedVersion { .. } => "WEAVE-E1004",
            LexError::UnknownEscape { .. } => "WEAVE-E1005",
        }
    }

    fn span(&self) -> Option<Span> {
        Some(match self {
            LexError::UnexpectedCharacter { span, .. }
            | LexError::UnterminatedString { span }
            | LexError::UnterminatedComment { span }
            | LexError::MalformedVersion { span, .. }
            | LexError::UnknownEscape { span, .. } => *span,
        })
    }
}

/// 23.37's reserved keyword list, verbatim, plus the words its own examples use as keywords.
///
/// The list in 23.37 is incomplete against its own code blocks — `using`, `await`, `match`,
/// `choose`, `provides`, `requires`, `clearance`, `throws`, `record`, `variant`, `deliver`,
/// `satisfy`, `compensate`, `include`, `resolution`, `publish`, `execute`, `resolve` and `current`
/// all appear in keyword position in the reference examples but are absent from the reserved
/// table. They are reserved here, and the divergence is recorded rather than silently smoothed
/// over, because a program that binds `let match = ...` would otherwise parse differently in this
/// implementation than the reference table implies.
pub fn is_reserved(word: &str) -> bool {
    const RESERVED: &[&str] = &[
        // 23.37 "Reserved keywords", in its order.
        "package",
        "import",
        "type",
        "interface",
        "role",
        "policy",
        "choreography",
        "weave",
        "molecule",
        "bind",
        "let",
        "ask",
        "send",
        "claim",
        "propose",
        "accept",
        "reject",
        "challenge",
        "commit",
        "delegate",
        "context",
        "checkpoint",
        "fork",
        "branch",
        "join",
        "par",
        "race",
        "choice",
        "watch",
        "spawn",
        "retire",
        "allow",
        "deny",
        "require",
        "budget",
        "effects",
        "before",
        "after",
        "when",
        "until",
        "repeat",
        "stop",
        "return",
        // Used as keywords by 23.37's and 23.02's examples but missing from the table above.
        "using",
        "await",
        "match",
        "choose",
        "provides",
        "requires",
        "clearance",
        "throws",
        "record",
        "variant",
        "deliver",
        "satisfy",
        "compensate",
        "include",
        "resolution",
        "publish",
        "execute",
        "resolve",
        "current",
        "from",
        "to",
        "by",
        "with",
        "where",
        "into",
        "as",
        "shared",
        "human",
        "for",
        "on",
    ];
    RESERVED.contains(&word)
}

struct Cursor<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    index: usize,
    line: u32,
    column: u32,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str) -> Self {
        Cursor {
            source,
            chars: source.char_indices().collect(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).map(|(_, c)| *c)
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.index + ahead).map(|(_, c)| *c)
    }

    fn offset(&self) -> usize {
        self.chars
            .get(self.index)
            .map(|(offset, _)| *offset)
            .unwrap_or(self.source.len())
    }

    /// Whether the character just consumed was a `:`.
    ///
    /// This is the whole of the `//` ambiguity resolution. 23.37 specifies `//` line comments and,
    /// four sections later, writes `minimum-profile prism://capability/challenge >= 0.80`. Both
    /// cannot hold without a rule, and the rule chosen here is the narrowest one that keeps every
    /// example in 23.37 working: a `//` directly after a colon is a URI scheme separator, not a
    /// comment. The cost is that `label: // note` no longer starts a comment; the alternative
    /// costs every `prism://` reference in the blueprint.
    fn preceded_by_colon(&self) -> bool {
        self.index > 0 && self.chars[self.index - 1].1 == ':'
    }

    fn bump(&mut self) -> Option<char> {
        let (_, character) = *self.chars.get(self.index)?;
        self.index += 1;
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }
}

/// Tokenizes a WeaveLang source file.
///
/// The returned vector always ends with exactly one [`TokenKind::Eof`].
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let mut cursor = Cursor::new(source);
    let mut tokens = Vec::new();

    loop {
        skip_trivia(&mut cursor)?;
        let start = cursor.offset();
        let line = cursor.line;
        let column = cursor.column;

        let Some(character) = cursor.peek() else {
            tokens.push(Token {
                kind: TokenKind::Eof,
                text: String::new(),
                span: Span::empty_at(start, line, column),
            });
            return Ok(tokens);
        };

        let token = if character.is_ascii_alphabetic() || character == '_' {
            lex_identifier(&mut cursor, start, line, column)
        } else if character.is_ascii_digit() {
            lex_number(&mut cursor, start, line, column)?
        } else if character == '"' {
            lex_string(&mut cursor, start, line, column)?
        } else {
            lex_punctuation(&mut cursor, start, line, column)?
        };
        tokens.push(token);
    }
}

fn skip_trivia(cursor: &mut Cursor<'_>) -> Result<(), LexError> {
    loop {
        match cursor.peek() {
            Some(c) if c.is_whitespace() => {
                cursor.bump();
            }
            Some('/') if cursor.peek_at(1) == Some('/') && !cursor.preceded_by_colon() => {
                while let Some(c) = cursor.peek() {
                    if c == '\n' {
                        break;
                    }
                    cursor.bump();
                }
            }
            Some('/') if cursor.peek_at(1) == Some('*') => {
                let start = cursor.offset();
                let line = cursor.line;
                let column = cursor.column;
                cursor.bump();
                cursor.bump();
                loop {
                    match cursor.peek() {
                        None => {
                            return Err(LexError::UnterminatedComment {
                                span: Span::new(start, cursor.offset(), line, column),
                            })
                        }
                        Some('*') if cursor.peek_at(1) == Some('/') => {
                            cursor.bump();
                            cursor.bump();
                            break;
                        }
                        Some(_) => {
                            cursor.bump();
                        }
                    }
                }
            }
            _ => return Ok(()),
        }
    }
}

fn lex_identifier(cursor: &mut Cursor<'_>, start: usize, line: u32, column: u32) -> Token {
    let mut text = String::new();
    while let Some(c) = cursor.peek() {
        let continues = c.is_ascii_alphanumeric()
            || c == '_'
            || (c == '-'
                && cursor
                    .peek_at(1)
                    .is_some_and(|next| next.is_ascii_alphanumeric()));
        if !continues {
            break;
        }
        text.push(c);
        cursor.bump();
    }
    Token {
        kind: TokenKind::Ident,
        text,
        span: Span::new(start, cursor.offset(), line, column),
    }
}

fn lex_number(
    cursor: &mut Cursor<'_>,
    start: usize,
    line: u32,
    column: u32,
) -> Result<Token, LexError> {
    let mut components: Vec<String> = vec![String::new()];
    read_digits(cursor, components.last_mut().expect("one component"));

    while cursor.peek() == Some('.') && cursor.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
        cursor.bump();
        components.push(String::new());
        read_digits(cursor, components.last_mut().expect("just pushed"));
    }

    if components.len() > 3 {
        return Err(LexError::MalformedVersion {
            text: components.join("."),
            span: Span::new(start, cursor.offset(), line, column),
        });
    }

    if components.len() == 1 {
        if let Some(suffix) = read_duration_suffix(cursor) {
            return Ok(Token {
                kind: TokenKind::Duration,
                text: format!("{}{}", components[0], suffix),
                span: Span::new(start, cursor.offset(), line, column),
            });
        }
    }

    let kind = match components.len() {
        1 => TokenKind::Integer,
        2 => TokenKind::Float,
        _ => TokenKind::Version,
    };
    Ok(Token {
        kind,
        text: components.join("."),
        span: Span::new(start, cursor.offset(), line, column),
    })
}

fn read_digits(cursor: &mut Cursor<'_>, into: &mut String) {
    while let Some(c) = cursor.peek() {
        if c.is_ascii_digit() {
            into.push(c);
            cursor.bump();
        } else if c == '_' && cursor.peek_at(1).is_some_and(|next| next.is_ascii_digit()) {
            cursor.bump();
        } else {
            break;
        }
    }
}

/// Reads a duration suffix, but only when it is not the start of a longer word.
///
/// `15m` is fifteen minutes; `15minutes` is not a duration and must not silently become one, so
/// the suffix is accepted only when no identifier character follows it.
fn read_duration_suffix(cursor: &mut Cursor<'_>) -> Option<&'static str> {
    let candidates: &[&str] = &["ms", "s", "m", "h", "d"];
    for candidate in candidates {
        let matches = candidate
            .chars()
            .enumerate()
            .all(|(i, c)| cursor.peek_at(i) == Some(c));
        if !matches {
            continue;
        }
        let after = cursor.peek_at(candidate.len());
        if after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            continue;
        }
        for _ in 0..candidate.len() {
            cursor.bump();
        }
        return Some(candidate);
    }
    None
}

fn lex_string(
    cursor: &mut Cursor<'_>,
    start: usize,
    line: u32,
    column: u32,
) -> Result<Token, LexError> {
    cursor.bump();
    let mut text = String::new();
    loop {
        let escape_line = cursor.line;
        let escape_column = cursor.column;
        let escape_start = cursor.offset();
        match cursor.bump() {
            None => {
                return Err(LexError::UnterminatedString {
                    span: Span::new(start, cursor.offset(), line, column),
                })
            }
            Some('"') => {
                return Ok(Token {
                    kind: TokenKind::Str,
                    text,
                    span: Span::new(start, cursor.offset(), line, column),
                })
            }
            Some('\\') => {
                let escaped = cursor.bump().ok_or(LexError::UnterminatedString {
                    span: Span::new(start, cursor.offset(), line, column),
                })?;
                text.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    other => {
                        return Err(LexError::UnknownEscape {
                            character: other,
                            span: Span::new(
                                escape_start,
                                cursor.offset(),
                                escape_line,
                                escape_column,
                            ),
                        })
                    }
                });
            }
            Some(c) => text.push(c),
        }
    }
}

fn lex_punctuation(
    cursor: &mut Cursor<'_>,
    start: usize,
    line: u32,
    column: u32,
) -> Result<Token, LexError> {
    let first = cursor.bump().expect("caller peeked a character");
    let second = cursor.peek();

    let (kind, consume_second) = match (first, second) {
        ('-', Some('>')) => (TokenKind::Arrow, true),
        ('=', Some('>')) => (TokenKind::FatArrow, true),
        ('<', Some('=')) => (TokenKind::LessEq, true),
        ('>', Some('=')) => (TokenKind::GreaterEq, true),
        ('<', _) => (TokenKind::Less, false),
        ('>', _) => (TokenKind::Greater, false),
        ('=', _) => (TokenKind::Equals, false),
        ('{', _) => (TokenKind::LBrace, false),
        ('}', _) => (TokenKind::RBrace, false),
        ('(', _) => (TokenKind::LParen, false),
        (')', _) => (TokenKind::RParen, false),
        ('[', _) => (TokenKind::LBracket, false),
        (']', _) => (TokenKind::RBracket, false),
        (',', _) => (TokenKind::Comma, false),
        (':', _) => (TokenKind::Colon, false),
        (';', _) => (TokenKind::Semicolon, false),
        ('.', _) => (TokenKind::Dot, false),
        ('@', _) => (TokenKind::At, false),
        ('/', _) => (TokenKind::Slash, false),
        ('+', _) => (TokenKind::Plus, false),
        ('-', _) => (TokenKind::Minus, false),
        (other, _) => {
            return Err(LexError::UnexpectedCharacter {
                character: other,
                span: Span::new(start, cursor.offset(), line, column),
            })
        }
    };
    if consume_second {
        cursor.bump();
    }

    Ok(Token {
        kind,
        text: cursor
            .source
            .get(start..cursor.offset())
            .unwrap_or_default()
            .to_string(),
        span: Span::new(start, cursor.offset(), line, column),
    })
}
