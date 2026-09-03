//! A recursive-descent parser for BioQL.
//!
//! Hand-written, like the lexer, because the workspace has no parser generator available offline and
//! because the error messages are part of the contract: every failure names the token that broke the
//! parse and where it was.
//!
//! # The grammar, in full
//!
//! ```text
//! query        := "select" projection "from" ident clause*
//! projection   := "*" | path ("," path)*
//! path         := ident ("." ident)*
//! clause       := scope | filter | expand | time | labels | aggregate | cost
//! scope        := "in" "{" [ binding ("," binding)* ] "}"
//! binding      := ident ":" ( string | "{" string ("," string)* "}" )
//! filter       := "where" expr
//! expand       := "expand" "ontology" string "release" string "policy" ident
//! time         := "at" ident
//! labels       := "labels" "{" [ string ("," string)* ] "}"
//! aggregate    := "aggregate" agg ("," agg)* [ "provenance" ident ]
//! agg          := ident "(" path ")"
//! cost         := "cost" "limit" integer
//!
//! expr         := disjunction
//! disjunction  := conjunction ( "or" conjunction )*
//! conjunction  := negation ( "and" negation )*
//! negation     := "not" negation | comparison
//! comparison   := additive [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" | "in" ) additive ]
//! additive     := multiplicative ( ( "+" | "-" ) multiplicative )*
//! multiplicative := unary ( ( "*" | "/" ) unary )*
//! unary        := "-" unary | primary
//! primary      := number [ unit ] | string | "true" | "false"
//!               | "instant" string | "(" expr ")" | "{" expr ("," expr)* "}" | path
//! ```
//!
//! # Three deliberate choices
//!
//! **Clauses are ordered and may not repeat.** Two `where` clauses, or a `cost` before a `where`,
//! are parse errors rather than being merged or reordered. A language that quietly accepts either
//! order gives two spellings of the same query different source text and therefore different query
//! digests, and the digest is what a result bundle cites.
//!
//! **Comparison is non-associative.** `a < b < c` does not parse. It is either a range test the
//! author should write with `and` or a mistake, and in every language where it parses it means the
//! wrong thing.
//!
//! **An identifier after a number is always a unit.** `12.5 mm3` is a quantity. `12.5 foo` is
//! [`ParseError::UnknownUnit`], not a stray identifier, because nothing else may follow a number in
//! this grammar and naming the real problem beats reporting a downstream one.

use crate::bioql::ast::{
    AggregateClause, Aggregation, BinaryOp, CollectionRef, CostClause, ExpansionClause,
    ExpansionPolicy, Expr, LabelClause, Literal, Path, Projection, ProvenanceClause,
    ProvenanceMode, Query, ScopeBinding, ScopeClause, ScopeLiteral, TimeClause, UnaryOp,
};
use crate::bioql::lexer::lex;
use crate::bioql::token::{Keyword, Token, TokenKind};
use crate::clock::Clock;
use crate::error::{ParseError, QueryError};
use crate::span::Span;
use bioprism_scope::Timestamp;
use bioprism_standards::Unit;
use std::collections::BTreeSet;

/// Lexes and parses a query. Produces an untyped [`Query`]; nothing is checked here.
pub fn parse(source: &str) -> Result<Query, QueryError> {
    let tokens = lex(source)?;
    let query = Parser::new(tokens).query()?;
    Ok(query)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

/// Clause ordering. The index is the rank a clause must exceed to be accepted.
const CLAUSE_ORDER: [Keyword; 7] = [
    Keyword::In,
    Keyword::Where,
    Keyword::Expand,
    Keyword::At,
    Keyword::Labels,
    Keyword::Aggregate,
    Keyword::Cost,
];

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn at_end(&self) -> bool {
        self.peek().kind.is_end()
    }

    fn unexpected(&self, expected: &str) -> ParseError {
        let token = self.peek();
        if token.kind.is_end() {
            ParseError::UnexpectedEnd {
                expected: expected.to_string(),
                span: token.span,
            }
        } else {
            ParseError::UnexpectedToken {
                expected: expected.to_string(),
                found: token.kind.describe(),
                span: token.span,
            }
        }
    }

    fn expect(&mut self, kind: &TokenKind, expected: &str) -> Result<Token, ParseError> {
        if &self.peek().kind == kind {
            Ok(self.advance())
        } else {
            Err(self.unexpected(expected))
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<Token, ParseError> {
        if self.peek().keyword() == Some(keyword) {
            Ok(self.advance())
        } else {
            Err(self.unexpected(&format!("keyword `{keyword}`")))
        }
    }

    fn expect_ident(&mut self, expected: &str) -> Result<(String, Span), ParseError> {
        match self.peek().ident() {
            Some(name) => {
                let name = name.to_string();
                let span = self.advance().span;
                Ok((name, span))
            }
            None => Err(self.unexpected(expected)),
        }
    }

    fn expect_string(&mut self, expected: &str) -> Result<(String, Span), ParseError> {
        match &self.peek().kind {
            TokenKind::Text(value) => {
                let value = value.clone();
                let span = self.advance().span;
                Ok((value, span))
            }
            _ => Err(self.unexpected(expected)),
        }
    }

    fn query(&mut self) -> Result<Query, ParseError> {
        let start = self.peek().span;
        self.expect_keyword(Keyword::Select)?;
        let select = self.projection()?;
        self.expect_keyword(Keyword::From)?;
        let (name, collection_span) = self.expect_ident("a collection name after `from`")?;
        let from = CollectionRef {
            name,
            span: collection_span,
        };

        let mut query = Query {
            select,
            from,
            scope: None,
            filter: None,
            expand: None,
            time: None,
            labels: None,
            aggregate: None,
            cost: None,
            span: start,
        };

        let mut last_rank: Option<usize> = None;
        while !self.at_end() {
            let Some(keyword) = self.peek().keyword() else {
                return Err(self.unexpected("a clause keyword or the end of the query"));
            };
            let Some(rank) = CLAUSE_ORDER.iter().position(|k| *k == keyword) else {
                return Err(self.unexpected("a clause keyword or the end of the query"));
            };
            let span = self.peek().span;
            if let Some(previous) = last_rank {
                if rank == previous {
                    return Err(ParseError::DuplicateClause {
                        clause: keyword.to_string(),
                        span,
                    });
                }
                if rank < previous {
                    return Err(ParseError::ClauseOutOfOrder {
                        clause: keyword.to_string(),
                        after: CLAUSE_ORDER[previous].to_string(),
                        span,
                    });
                }
            }
            last_rank = Some(rank);
            match keyword {
                Keyword::In => query.scope = Some(self.scope_clause()?),
                Keyword::Where => {
                    self.advance();
                    query.filter = Some(self.expr()?);
                }
                Keyword::Expand => query.expand = Some(self.expand_clause()?),
                Keyword::At => query.time = Some(self.time_clause()?),
                Keyword::Labels => query.labels = Some(self.label_clause()?),
                Keyword::Aggregate => query.aggregate = Some(self.aggregate_clause()?),
                Keyword::Cost => query.cost = Some(self.cost_clause()?),
                _ => {
                    return Err(self.unexpected("one of the supported clause keywords"));
                }
            }
        }

        let end = self.peek().span;
        query.span = start.merge(end);
        Ok(query)
    }

    fn projection(&mut self) -> Result<Projection, ParseError> {
        if matches!(self.peek().kind, TokenKind::Star) {
            let span = self.advance().span;
            return Ok(Projection::Everything { span });
        }
        let first = self.path()?;
        let mut span = first.span;
        let mut paths = vec![first];
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.advance();
            let next = self.path()?;
            span = span.merge(next.span);
            paths.push(next);
        }
        Ok(Projection::Fields { paths, span })
    }

    fn path(&mut self) -> Result<Path, ParseError> {
        let (first, first_span) = self.expect_ident("a field name")?;
        let mut span = first_span;
        let mut segments = vec![first];
        while matches!(self.peek().kind, TokenKind::Dot) {
            self.advance();
            let (segment, segment_span) = self.expect_ident("a field name after `.`")?;
            span = span.merge(segment_span);
            segments.push(segment);
        }
        Ok(Path::new(segments, span))
    }

    fn scope_clause(&mut self) -> Result<ScopeClause, ParseError> {
        let start = self.expect_keyword(Keyword::In)?.span;
        self.expect(&TokenKind::LBrace, "`{` after `in`")?;
        let mut bindings = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RBrace) {
            loop {
                bindings.push(self.scope_binding()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` closing the scope")?
            .span;
        Ok(ScopeClause {
            bindings,
            span: start.merge(end),
        })
    }

    fn scope_binding(&mut self) -> Result<ScopeBinding, ParseError> {
        let (dimension, start) = self.expect_ident("a scope dimension name")?;
        self.expect(&TokenKind::Colon, "`:` after a scope dimension")?;
        let (value, end) = match self.peek().kind {
            TokenKind::LBrace => {
                let open = self.advance().span;
                let mut values = BTreeSet::new();
                loop {
                    let (item, _) = self.expect_string("a scope value")?;
                    values.insert(item);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let close = self.expect(&TokenKind::RBrace, "`}` closing a scope value set")?;
                (ScopeLiteral::OneOf { values }, open.merge(close.span))
            }
            _ => {
                let (value, span) = self.expect_string("a scope value")?;
                (ScopeLiteral::Exact { value }, span)
            }
        };
        Ok(ScopeBinding {
            dimension,
            value,
            span: start.merge(end),
        })
    }

    fn expand_clause(&mut self) -> Result<ExpansionClause, ParseError> {
        let start = self.expect_keyword(Keyword::Expand)?.span;
        self.expect_keyword(Keyword::Ontology)?;
        let (ontology, _) = self.expect_string("an ontology name")?;
        self.expect_keyword(Keyword::Release)?;
        let (release, _) = self.expect_string("an ontology release")?;
        self.expect_keyword(Keyword::Policy)?;
        let (policy_name, policy_span) = self.expect_ident("an expansion policy")?;
        let policy = ExpansionPolicy::parse(&policy_name).ok_or(ParseError::UnexpectedToken {
            expected: "one of `exact`, `descendants`, `ancestors`".to_string(),
            found: format!("identifier `{policy_name}`"),
            span: policy_span,
        })?;
        Ok(ExpansionClause {
            ontology,
            release,
            policy,
            span: start.merge(policy_span),
        })
    }

    fn time_clause(&mut self) -> Result<TimeClause, ParseError> {
        let start = self.expect_keyword(Keyword::At)?.span;
        let (name, span) = self.expect_ident("a clock name")?;
        let clock = Clock::parse(&name).ok_or(ParseError::UnexpectedToken {
            expected: "one of `event`, `record`, `decision`, `reveal`".to_string(),
            found: format!("identifier `{name}`"),
            span,
        })?;
        Ok(TimeClause {
            clock,
            span: start.merge(span),
        })
    }

    fn label_clause(&mut self) -> Result<LabelClause, ParseError> {
        let start = self.expect_keyword(Keyword::Labels)?.span;
        self.expect(&TokenKind::LBrace, "`{` after `labels`")?;
        let mut labels = BTreeSet::new();
        if !matches!(self.peek().kind, TokenKind::RBrace) {
            loop {
                let (label, _) = self.expect_string("an access label")?;
                labels.insert(label);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` closing the label set")?
            .span;
        Ok(LabelClause {
            labels,
            span: start.merge(end),
        })
    }

    fn aggregate_clause(&mut self) -> Result<AggregateClause, ParseError> {
        let start = self.expect_keyword(Keyword::Aggregate)?.span;
        let mut items = Vec::new();
        let mut span = start;
        loop {
            let item = self.aggregation()?;
            span = span.merge(item.span);
            items.push(item);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        let provenance = if self.peek().keyword() == Some(Keyword::Provenance) {
            let keyword_span = self.advance().span;
            let (name, name_span) = self.expect_ident("an aggregation provenance mode")?;
            let mode = ProvenanceMode::parse(&name).ok_or(ParseError::UnexpectedToken {
                expected: "one of `source_lineage`, `evidence_ids`".to_string(),
                found: format!("identifier `{name}`"),
                span: name_span,
            })?;
            span = span.merge(name_span);
            Some(ProvenanceClause {
                mode,
                span: keyword_span.merge(name_span),
            })
        } else {
            None
        };
        Ok(AggregateClause {
            items,
            provenance,
            span,
        })
    }

    fn aggregation(&mut self) -> Result<Aggregation, ParseError> {
        let (function, start) = self.expect_ident("an aggregate function name")?;
        self.expect(&TokenKind::LParen, "`(` after an aggregate function")?;
        let argument = self.path()?;
        let end = self
            .expect(&TokenKind::RParen, "`)` closing an aggregate argument")?
            .span;
        Ok(Aggregation {
            function,
            argument,
            span: start.merge(end),
        })
    }

    fn cost_clause(&mut self) -> Result<CostClause, ParseError> {
        let start = self.expect_keyword(Keyword::Cost)?.span;
        self.expect_keyword(Keyword::Limit)?;
        let token = self.advance();
        match token.kind {
            TokenKind::Number {
                ref text,
                integral: true,
                ..
            } => {
                let limit = text
                    .parse::<u64>()
                    .map_err(|_| ParseError::UnexpectedToken {
                        expected: "a non-negative whole number of cost units within u64"
                            .to_string(),
                        found: format!("number `{text}`"),
                        span: token.span,
                    })?;
                Ok(CostClause {
                    limit,
                    span: start.merge(token.span),
                })
            }
            TokenKind::Number { ref text, .. } => Err(ParseError::UnexpectedToken {
                expected: "a non-negative whole number of cost units".to_string(),
                found: format!("number `{text}`"),
                span: token.span,
            }),
            ref other => Err(ParseError::UnexpectedToken {
                expected: "a non-negative whole number of cost units".to_string(),
                found: other.describe(),
                span: token.span,
            }),
        }
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        self.disjunction()
    }

    fn disjunction(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.conjunction()?;
        while self.peek().keyword() == Some(Keyword::Or) {
            self.advance();
            let right = self.conjunction()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn conjunction(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.negation()?;
        while self.peek().keyword() == Some(Keyword::And) {
            self.advance();
            let right = self.negation()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn negation(&mut self) -> Result<Expr, ParseError> {
        if self.peek().keyword() == Some(Keyword::Not) {
            let start = self.advance().span;
            let operand = self.negation()?;
            let span = start.merge(operand.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                span,
            });
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.additive()?;
        let op = match self.peek().kind {
            TokenKind::Equal => Some(BinaryOp::Equal),
            TokenKind::NotEqual => Some(BinaryOp::NotEqual),
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::LessEqual => Some(BinaryOp::LessEqual),
            TokenKind::Greater => Some(BinaryOp::Greater),
            TokenKind::GreaterEqual => Some(BinaryOp::GreaterEqual),
            TokenKind::Keyword(Keyword::In) => Some(BinaryOp::In),
            _ => None,
        };
        let Some(op) = op else {
            return Ok(left);
        };
        self.advance();
        let right = self.additive()?;
        let span = left.span().merge(right.span());
        Ok(Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        })
    }

    fn additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Subtract,
                _ => return Ok(left),
            };
            self.advance();
            let right = self.multiplicative()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Multiply,
                TokenKind::Slash => BinaryOp::Divide,
                _ => return Ok(left),
            };
            self.advance();
            let right = self.unary()?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek().kind, TokenKind::Minus) {
            let start = self.advance().span;
            let operand = self.unary()?;
            let span = start.merge(operand.span());
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(operand),
                span,
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().kind.clone() {
            TokenKind::Number {
                value,
                text: _,
                integral,
            } => {
                let span = self.advance().span;
                let (unit, span) = self.unit_suffix(span)?;
                Ok(Expr::Literal {
                    value: Literal::Number {
                        value,
                        unit,
                        integral,
                    },
                    span,
                })
            }
            TokenKind::Text(text) => {
                let span = self.advance().span;
                Ok(Expr::Literal {
                    value: Literal::Text { value: text },
                    span,
                })
            }
            TokenKind::Keyword(Keyword::True) => {
                let span = self.advance().span;
                Ok(Expr::Literal {
                    value: Literal::Bool { value: true },
                    span,
                })
            }
            TokenKind::Keyword(Keyword::False) => {
                let span = self.advance().span;
                Ok(Expr::Literal {
                    value: Literal::Bool { value: false },
                    span,
                })
            }
            TokenKind::Keyword(Keyword::Instant) => {
                let start = self.advance().span;
                let (text, text_span) = self.expect_string("an RFC 3339 timestamp")?;
                let parsed =
                    Timestamp::parse(&text).map_err(|error| ParseError::MalformedTimestamp {
                        text: text.clone(),
                        span: text_span,
                        detail: error.to_string(),
                    })?;
                Ok(Expr::Literal {
                    value: Literal::Instant {
                        rfc3339: parsed.to_rfc3339(),
                        nanos_utc: parsed.as_nanos_utc(),
                    },
                    span: start.merge(text_span),
                })
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.expr()?;
                self.expect(&TokenKind::RParen, "`)` closing a parenthesised expression")?;
                Ok(inner)
            }
            TokenKind::LBrace => {
                let start = self.advance().span;
                let mut items = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RBrace) {
                    loop {
                        items.push(self.expr()?);
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                let end = self
                    .expect(&TokenKind::RBrace, "`}` closing a set literal")?
                    .span;
                Ok(Expr::Set {
                    items,
                    span: start.merge(end),
                })
            }
            TokenKind::Ident(_) => {
                let path = self.path()?;
                Ok(Expr::Field { path })
            }
            _ => Err(self.unexpected("a value, a field name, or `(`")),
        }
    }

    /// Reads the unit that must follow a numeric literal, if one is written.
    ///
    /// Nothing else in the grammar may follow a number, so an identifier here is a unit by
    /// construction and an unreadable one is reported as such.
    ///
    /// The slash lookahead exists because the standards table contains `mg/m2`, `mg/kg` and
    /// `cells/uL`, which lex as three tokens. The composite is taken only when the joined symbol is
    /// itself in the table, so `5 mg / weight` still parses as a division: a symbol that is not a
    /// unit never silently eats the operator.
    ///
    /// Two table entries remain unwritable as literal suffixes — the ratio units `%` and `1`, which
    /// are not identifiers. A field may be declared in them; a literal may not be written in them.
    fn unit_suffix(&mut self, number_span: Span) -> Result<(Option<Unit>, Span), ParseError> {
        let Some(symbol) = self.peek().ident().map(str::to_string) else {
            return Ok((None, number_span));
        };
        let span = self.advance().span;

        if matches!(self.peek().kind, TokenKind::Slash) {
            let denominator = self.tokens.get(self.pos + 1).and_then(Token::ident);
            if let Some(denominator) = denominator.map(str::to_string) {
                let composite = format!("{symbol}/{denominator}");
                if let Ok(unit) = Unit::parse(&composite) {
                    self.advance();
                    let end = self.advance().span;
                    return Ok((Some(unit), number_span.merge(end)));
                }
            }
        }

        let unit = Unit::parse(&symbol).map_err(|error| ParseError::UnknownUnit {
            symbol: symbol.clone(),
            span,
            detail: error.to_string(),
        })?;
        Ok((Some(unit), number_span.merge(span)))
    }
}
