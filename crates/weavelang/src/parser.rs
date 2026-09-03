//! A hand-written recursive-descent parser for WeaveLang.
//!
//! Blueprint 23.03 phase 1 ("parse and normalize") and the grammar sketched by 23.37. There is no
//! parser generator in this workspace and none is wanted: the CSV reader, the argument parser, the
//! JSON-RPC framing and the RFC 3339 codec are all hand-rolled for the same reason, which is that
//! the build is offline against pinned dependencies. The grammar 23.37 asks for is small enough
//! that a table-driven parser would cost more than it saves.
//!
//! The parser accepts the dialect of 23.37. 23.02 sketches a second, overlapping dialect — an
//! inline `policy { }` block inside a `weave` body, `effects allow [...]` with the keywords
//! reversed, `shared evidence: EvidenceLattice`, `molecule`, `thread`, `fork N from checkpoint c`,
//! `join c using r { require ... }`, `assurance provenance-required`, and
//! `information max-label confidential`. Where the two dialects differ only in keyword order the
//! parser accepts both; where 23.02 introduces a construct 23.37 never confirms, the parser rejects
//! it and the divergence is listed in the crate docs. Guessing at a grammar the reference does not
//! state would produce a compiler that accepts programs no other implementation would.
//!
//! Not implemented: error recovery. The parser stops at the first error. Recovery would let it
//! report several errors per pass, but a resynchronising parser also invents phantom errors
//! downstream of the real one, and the caller here is usually an agent that will fix one thing and
//! recompile.

use crate::ast::*;
use crate::diagnostic::{Diagnostic, Span};
use crate::lexer::{is_reserved, tokenize, LexError, Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    Lex(#[from] LexError),

    #[error("expected {expected} but found {found} `{text}` at {span}")]
    Unexpected {
        expected: String,
        found: &'static str,
        text: String,
        span: Span,
    },

    #[error("`{keyword}` at {span} is a reserved keyword and cannot be used as a name")]
    ReservedKeyword { keyword: String, span: Span },

    #[error("`{found}` at {span} does not start a declaration; expected one of package, import, type, interface, role, policy, choreography, weave")]
    NotADeclaration { found: String, span: Span },

    #[error("`{clause}` at {span} is not a clause of a {construct} block")]
    UnknownClause {
        construct: &'static str,
        clause: String,
        span: Span,
    },

    #[error("{keyword} `{name}` at {span} is declared more than once; the first is at {previous}")]
    DuplicateDeclaration {
        keyword: &'static str,
        name: String,
        span: Span,
        previous: Span,
    },

    #[error("`{text}` at {span} is not a valid {expected}")]
    Malformed {
        expected: &'static str,
        text: String,
        span: Span,
    },
}

impl Diagnostic for ParseError {
    fn code(&self) -> &'static str {
        match self {
            ParseError::Lex(error) => error.code(),
            ParseError::Unexpected { .. } => "WEAVE-E2001",
            ParseError::ReservedKeyword { .. } => "WEAVE-E2002",
            ParseError::NotADeclaration { .. } => "WEAVE-E2003",
            ParseError::UnknownClause { .. } => "WEAVE-E2004",
            ParseError::DuplicateDeclaration { .. } => "WEAVE-E2005",
            ParseError::Malformed { .. } => "WEAVE-E2006",
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            ParseError::Lex(error) => error.span(),
            ParseError::Unexpected { span, .. }
            | ParseError::ReservedKeyword { span, .. }
            | ParseError::NotADeclaration { span, .. }
            | ParseError::UnknownClause { span, .. }
            | ParseError::DuplicateDeclaration { span, .. }
            | ParseError::Malformed { span, .. } => Some(*span),
        }
    }
}

/// Parses a complete WeaveLang source file.
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = tokenize(source)?;
    let mut parser = Parser {
        source,
        tokens,
        position: 0,
        eof: Token {
            kind: TokenKind::Eof,
            text: String::new(),
            span: Span::empty_at(source.len(), 1, 1),
        },
    };
    parser.program()
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    position: usize,
    eof: Token,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&self.eof)
    }

    fn peek_at(&self, ahead: usize) -> &Token {
        self.tokens
            .get(self.position.saturating_add(ahead))
            .unwrap_or(&self.eof)
    }

    fn kind(&self) -> TokenKind {
        self.peek().kind
    }

    fn at_word(&self, word: &str) -> bool {
        self.peek().is_word(word)
    }

    fn at_end(&self) -> bool {
        self.kind() == TokenKind::Eof
    }

    fn bump(&mut self) -> Token {
        let token = self.peek().clone();
        if token.kind != TokenKind::Eof {
            self.position += 1;
        }
        token
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.kind() == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, word: &str) -> bool {
        if self.at_word(word) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn unexpected(&self, expected: impl Into<String>) -> ParseError {
        let token = self.peek();
        ParseError::Unexpected {
            expected: expected.into(),
            found: token.kind.describe(),
            text: token.text.clone(),
            span: token.span,
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.kind() == kind {
            Ok(self.bump())
        } else {
            Err(self.unexpected(kind.describe()))
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<Token, ParseError> {
        if self.at_word(word) {
            Ok(self.bump())
        } else {
            Err(self.unexpected(format!("`{word}`")))
        }
    }

    /// An identifier in binding position: a name the program is introducing.
    ///
    /// Reserved words are refused here and only here, so that `budget.low` still parses as a path
    /// while `let budget = ...` does not parse at all.
    fn binding_name(&mut self) -> Result<(String, Span), ParseError> {
        let token = self.expect(TokenKind::Ident)?;
        if is_reserved(&token.text) {
            return Err(ParseError::ReservedKeyword {
                keyword: token.text,
                span: token.span,
            });
        }
        Ok((token.text, token.span))
    }

    /// An identifier in reference position, where reserved words are legitimate names.
    ///
    /// Act names are the reason: `Reviewer -> Lead: accept<Plan>` names the act `accept`, which is
    /// on 23.37's reserved list because it is also a statement keyword.
    fn any_name(&mut self) -> Result<(String, Span), ParseError> {
        let token = self.expect(TokenKind::Ident)?;
        Ok((token.text, token.span))
    }

    fn program(&mut self) -> Result<Program, ParseError> {
        let mut package = None;
        let mut imports = Vec::new();
        let mut items: Vec<Item> = Vec::new();

        while !self.at_end() {
            if self.at_word("package") && package.is_none() {
                let start = self.bump().span;
                let name = self.qualified_name()?;
                package = Some(PackageDecl {
                    span: start.merge(name.span),
                    name,
                });
            } else if self.at_word("import") {
                let start = self.bump().span;
                let name = self.qualified_name()?;
                let alias = if self.eat_word("as") {
                    Some(self.binding_name()?.0)
                } else {
                    None
                };
                imports.push(ImportDecl {
                    span: start.merge(name.span),
                    name,
                    alias,
                });
            } else {
                let item = self.item()?;
                if let Some(previous) = items
                    .iter()
                    .find(|existing| existing.name() == item.name() && !item.name().is_empty())
                {
                    return Err(ParseError::DuplicateDeclaration {
                        keyword: item.keyword(),
                        name: item.name().to_string(),
                        span: item.span(),
                        previous: previous.span(),
                    });
                }
                items.push(item);
            }
        }

        Ok(Program {
            package,
            imports,
            items,
        })
    }

    fn item(&mut self) -> Result<Item, ParseError> {
        if self.kind() != TokenKind::Ident {
            return Err(ParseError::NotADeclaration {
                found: self.peek().text.clone(),
                span: self.peek().span,
            });
        }
        match self.peek().text.as_str() {
            "type" => self.type_decl().map(Item::Type),
            "interface" => self.interface_decl().map(Item::Interface),
            "role" => self.role_decl().map(Item::Role),
            "policy" => self.policy_decl().map(Item::Policy),
            "choreography" => self.choreography_decl().map(Item::Choreography),
            "weave" => self.weave_decl().map(Item::Weave),
            other => Err(ParseError::NotADeclaration {
                found: other.to_string(),
                span: self.peek().span,
            }),
        }
    }

    fn qualified_name(&mut self) -> Result<QualifiedName, ParseError> {
        let (namespace, start) = self.any_name()?;
        self.expect(TokenKind::Colon)?;
        let (name, mut span) = self.any_name()?;
        span = start.merge(span);

        let item = if self.kind() == TokenKind::Slash {
            self.bump();
            let (item, item_span) = self.any_name()?;
            span = span.merge(item_span);
            Some(item)
        } else {
            None
        };

        let version = if self.kind() == TokenKind::At {
            self.bump();
            let token = self.version_token()?;
            span = span.merge(token.1);
            Some(token.0)
        } else {
            None
        };

        Ok(QualifiedName {
            namespace,
            name,
            item,
            version,
            span,
        })
    }

    /// A semantic version, which the lexer may have produced as an integer, a float or a version.
    ///
    /// `@1`, `@0.1` and `@0.1.0` are all written in 23.37's own examples, and they are three
    /// different token kinds.
    fn version_token(&mut self) -> Result<(String, Span), ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Integer | TokenKind::Float | TokenKind::Version => {
                self.bump();
                Ok((token.text, token.span))
            }
            _ => Err(self.unexpected("a semantic version")),
        }
    }

    /// A run of adjacent tokens forming a URI-like reference such as
    /// `prism://capability/challenge` or `sha256:0f1e`.
    fn reference(&mut self) -> Result<(String, Span), ParseError> {
        let start = self.peek().span;
        if self.kind() != TokenKind::Ident {
            return Err(self.unexpected("a reference"));
        }
        let mut end = self.bump().span;
        loop {
            let continues = matches!(
                self.kind(),
                TokenKind::Colon
                    | TokenKind::Slash
                    | TokenKind::Dot
                    | TokenKind::At
                    | TokenKind::Ident
                    | TokenKind::Integer
                    | TokenKind::Float
                    | TokenKind::Version
            );
            if !continues || end.end != self.peek().span.start {
                break;
            }
            end = self.bump().span;
        }
        let span = start.merge(end);
        let text = span
            .slice(self.source)
            .ok_or(ParseError::Malformed {
                expected: "reference",
                text: String::new(),
                span,
            })?
            .to_string();
        Ok((text, span))
    }

    fn type_decl(&mut self) -> Result<TypeDecl, ParseError> {
        let start = self.expect_word("type")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::Equals)?;

        let body = if self.at_word("record") {
            self.bump();
            self.expect(TokenKind::LBrace)?;
            let mut fields = Vec::new();
            while self.kind() != TokenKind::RBrace {
                let (field, field_span) = self.binding_name()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.type_ref()?;
                fields.push(Param {
                    span: field_span.merge(ty.span),
                    name: field,
                    ty,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            TypeBody::Record(fields)
        } else if self.at_word("variant") {
            self.bump();
            self.expect(TokenKind::LBrace)?;
            let mut cases = Vec::new();
            while self.kind() != TokenKind::RBrace {
                let (case, case_span) = self.any_name()?;
                let payload = if self.eat(TokenKind::LParen) {
                    let ty = self.type_ref()?;
                    self.expect(TokenKind::RParen)?;
                    Some(ty)
                } else {
                    None
                };
                cases.push(VariantCase {
                    name: case,
                    payload,
                    span: case_span,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            TypeBody::Variant(cases)
        } else {
            TypeBody::Alias(self.type_ref()?)
        };

        Ok(TypeDecl {
            name,
            body,
            span: start,
        })
    }

    fn type_ref(&mut self) -> Result<TypeRef, ParseError> {
        let (name, start) = self.any_name()?;
        let mut span = start;
        let mut arguments = Vec::new();
        if self.kind() == TokenKind::Less {
            self.bump();
            loop {
                let argument = self.type_ref()?;
                span = span.merge(argument.span);
                arguments.push(argument);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            span = span.merge(self.expect(TokenKind::Greater)?.span);
        }
        Ok(TypeRef {
            name,
            arguments,
            span,
        })
    }

    fn interface_decl(&mut self) -> Result<InterfaceDecl, ParseError> {
        let start = self.expect_word("interface")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while self.kind() != TokenKind::RBrace {
            let (method, method_span) = self.any_name()?;
            self.expect(TokenKind::LParen)?;
            let mut params = Vec::new();
            while self.kind() != TokenKind::RParen {
                let (param, param_span) = self.binding_name()?;
                self.expect(TokenKind::Colon)?;
                let ty = self.type_ref()?;
                params.push(Param {
                    span: param_span.merge(ty.span),
                    name: param,
                    ty,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;

            let returns = if self.eat(TokenKind::Arrow) {
                Some(self.type_ref()?)
            } else {
                None
            };

            let mut effects = Vec::new();
            let mut throws = Vec::new();
            loop {
                if self.at_word("effects") {
                    self.bump();
                    effects.extend(self.path_list()?);
                } else if self.at_word("throws") {
                    self.bump();
                    throws.extend(self.path_list()?.into_iter().map(|path| path.text()));
                } else {
                    break;
                }
            }

            methods.push(MethodDecl {
                name: method,
                params,
                returns,
                effects,
                throws,
                span: method_span,
            });
        }
        self.expect(TokenKind::RBrace)?;

        Ok(InterfaceDecl {
            name,
            methods,
            span: start,
        })
    }

    fn path_list(&mut self) -> Result<Vec<Path>, ParseError> {
        self.expect(TokenKind::LBracket)?;
        let mut paths = Vec::new();
        while self.kind() != TokenKind::RBracket {
            paths.push(self.path()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(paths)
    }

    fn path(&mut self) -> Result<Path, ParseError> {
        let (first, start) = self.any_name()?;
        let mut segments = vec![first];
        let mut span = start;
        while self.kind() == TokenKind::Dot {
            self.bump();
            let (segment, segment_span) = self.any_name()?;
            segments.push(segment);
            span = span.merge(segment_span);
        }
        Ok(Path { segments, span })
    }

    fn role_decl(&mut self) -> Result<RoleDecl, ParseError> {
        let start = self.expect_word("role")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::LBrace)?;

        let mut provides = Vec::new();
        let mut requires = Vec::new();
        let mut clearance = None;
        let mut minimum_profile = None;

        while self.kind() != TokenKind::RBrace {
            let clause = self.peek().clone();
            match clause.text.as_str() {
                "provides" => {
                    self.bump();
                    self.expect(TokenKind::LBracket)?;
                    while self.kind() != TokenKind::RBracket {
                        let (capability, capability_span) = self.any_name()?;
                        let version = if self.kind() == TokenKind::At {
                            self.bump();
                            Some(self.version_token()?.0)
                        } else {
                            None
                        };
                        provides.push(VersionedName {
                            name: capability,
                            version,
                            span: capability_span,
                        });
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RBracket)?;
                }
                "requires" => {
                    self.bump();
                    requires.extend(self.path_list()?);
                }
                "clearance" => {
                    self.bump();
                    let (level, level_span) = self.any_name()?;
                    let mut compartments = Vec::new();
                    while self.kind() == TokenKind::Slash {
                        self.bump();
                        compartments.push(self.any_name()?.0);
                    }
                    clearance = Some(Clearance {
                        level,
                        compartments,
                        span: level_span,
                    });
                }
                "minimum-profile" | "minimum-evidence" => {
                    self.bump();
                    let (reference, reference_span) = self.reference()?;
                    if !self.eat(TokenKind::GreaterEq) {
                        return Err(self.unexpected("`>=`"));
                    }
                    let threshold = self.decimal()?;
                    minimum_profile = Some(MinimumProfile {
                        reference,
                        threshold,
                        span: reference_span,
                    });
                }
                other => {
                    return Err(ParseError::UnknownClause {
                        construct: "role",
                        clause: other.to_string(),
                        span: clause.span,
                    })
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        Ok(RoleDecl {
            name,
            provides,
            requires,
            clearance,
            minimum_profile,
            span: start,
        })
    }

    fn decimal(&mut self) -> Result<f64, ParseError> {
        let token = self.peek().clone();
        if !matches!(token.kind, TokenKind::Integer | TokenKind::Float) {
            return Err(self.unexpected("a number"));
        }
        self.bump();
        token
            .text
            .parse::<f64>()
            .map_err(|_| ParseError::Malformed {
                expected: "number",
                text: token.text.clone(),
                span: token.span,
            })
    }

    fn integer(&mut self) -> Result<u64, ParseError> {
        let token = self.expect(TokenKind::Integer)?;
        token
            .text
            .parse::<u64>()
            .map_err(|_| ParseError::Malformed {
                expected: "integer",
                text: token.text.clone(),
                span: token.span,
            })
    }

    fn policy_decl(&mut self) -> Result<PolicyDecl, ParseError> {
        let start = self.expect_word("policy")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::LBrace)?;

        let mut allow_effects = Vec::new();
        let mut deny_effects = Vec::new();
        let mut require_human_for = Vec::new();
        let mut budgets = Vec::new();
        let mut max_participants = None;

        while self.kind() != TokenKind::RBrace {
            let clause = self.peek().clone();
            match clause.text.as_str() {
                // 23.37 writes `allow effects [...]`; 23.02 writes `effects allow [...]`. The two
                // are the same clause with the keywords transposed, so both are accepted.
                "allow" | "deny" | "require" => {
                    self.bump();
                    self.policy_effect_clause(
                        &clause.text,
                        &mut allow_effects,
                        &mut deny_effects,
                        &mut require_human_for,
                    )?;
                }
                "effects" => {
                    self.bump();
                    let (direction, _) = self.any_name()?;
                    self.policy_effect_clause(
                        &direction,
                        &mut allow_effects,
                        &mut deny_effects,
                        &mut require_human_for,
                    )?;
                }
                "budget" => {
                    let budget_span = self.bump().span;
                    let (resource, _) = self.any_name()?;
                    if !self.eat(TokenKind::LessEq) {
                        return Err(self.unexpected("`<=`"));
                    }
                    let limit = self.budget_amount()?;
                    budgets.push(BudgetLimit {
                        resource,
                        limit,
                        span: budget_span,
                    });
                }
                "max-participants" => {
                    self.bump();
                    max_participants = Some(self.integer()?);
                }
                other => {
                    return Err(ParseError::UnknownClause {
                        construct: "policy",
                        clause: other.to_string(),
                        span: clause.span,
                    })
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        Ok(PolicyDecl {
            name,
            allow_effects,
            deny_effects,
            require_human_for,
            budgets,
            max_participants,
            span: start,
        })
    }

    fn policy_effect_clause(
        &mut self,
        direction: &str,
        allow: &mut Vec<Path>,
        deny: &mut Vec<Path>,
        human: &mut Vec<Path>,
    ) -> Result<(), ParseError> {
        match direction {
            "allow" => {
                self.eat_word("effects");
                allow.extend(self.path_list()?);
            }
            "deny" => {
                self.eat_word("effects");
                deny.extend(self.path_list()?);
            }
            "require" => {
                self.expect_word("human")?;
                self.expect_word("for")?;
                human.extend(self.path_list()?);
            }
            other => {
                return Err(ParseError::UnknownClause {
                    construct: "policy",
                    clause: other.to_string(),
                    span: self.peek().span,
                })
            }
        }
        Ok(())
    }

    /// `120000`, `usd(5)` or `15m`.
    ///
    /// Money is kept exact in minor units rather than parsed as a float, because a budget ceiling
    /// that rounds is not a ceiling.
    fn budget_amount(&mut self) -> Result<Literal, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Integer => {
                self.bump();
                let value = token
                    .text
                    .parse::<i64>()
                    .map_err(|_| ParseError::Malformed {
                        expected: "integer",
                        text: token.text.clone(),
                        span: token.span,
                    })?;
                Ok(Literal::Integer(value))
            }
            TokenKind::Duration => {
                self.bump();
                duration_literal(&token.text, token.span)
            }
            TokenKind::Ident => {
                let (currency, _) = self.any_name()?;
                self.expect(TokenKind::LParen)?;
                let amount = self.peek().clone();
                let minor_units = match amount.kind {
                    TokenKind::Integer => {
                        self.bump();
                        amount
                            .text
                            .parse::<i64>()
                            .map_err(|_| ParseError::Malformed {
                                expected: "amount",
                                text: amount.text.clone(),
                                span: amount.span,
                            })?
                            * 100
                    }
                    TokenKind::Float => {
                        self.bump();
                        minor_units_of(&amount.text, amount.span)?
                    }
                    _ => return Err(self.unexpected("an amount")),
                };
                self.expect(TokenKind::RParen)?;
                Ok(Literal::Money {
                    currency,
                    minor_units,
                })
            }
            _ => Err(self.unexpected("a budget amount")),
        }
    }

    fn choreography_decl(&mut self) -> Result<ChoreographyDecl, ParseError> {
        let start = self.expect_word("choreography")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::LBrace)?;
        let steps = self.choreo_steps()?;
        self.expect(TokenKind::RBrace)?;
        Ok(ChoreographyDecl {
            name,
            steps,
            span: start,
        })
    }

    fn choreo_steps(&mut self) -> Result<Vec<ChoreoStep>, ParseError> {
        let mut steps = Vec::new();
        while self.kind() != TokenKind::RBrace && !self.at_end() {
            // A branch label is an identifier followed by `:`; a message is an identifier followed
            // by `->`. One token of lookahead separates them.
            if self.kind() == TokenKind::Ident && self.peek_at(1).kind == TokenKind::Colon {
                break;
            }
            steps.push(self.choreo_step()?);
        }
        Ok(steps)
    }

    fn choreo_step(&mut self) -> Result<ChoreoStep, ParseError> {
        if self.at_word("choice") {
            let start = self.bump().span;
            self.expect_word("by")?;
            let (by, _) = self.any_name()?;
            self.expect(TokenKind::LBrace)?;
            let mut branches = Vec::new();
            while self.kind() != TokenKind::RBrace {
                let (label, label_span) = self.any_name()?;
                self.expect(TokenKind::Colon)?;
                let steps = self.choreo_steps()?;
                branches.push(ChoiceBranch {
                    label,
                    steps,
                    span: label_span,
                });
            }
            self.expect(TokenKind::RBrace)?;
            return Ok(ChoreoStep::Choice {
                by,
                branches,
                span: start,
            });
        }

        let (from, start) = self.any_name()?;
        self.expect(TokenKind::Arrow)?;
        let (to, _) = self.any_name()?;
        self.expect(TokenKind::Colon)?;
        let (act, act_span) = self.any_name()?;
        let payload = if self.kind() == TokenKind::Less {
            self.bump();
            let ty = self.type_ref()?;
            self.expect(TokenKind::Greater)?;
            Some(ty)
        } else {
            None
        };
        Ok(ChoreoStep::Message {
            from,
            to,
            act,
            payload,
            span: start.merge(act_span),
        })
    }

    fn weave_decl(&mut self) -> Result<WeaveDecl, ParseError> {
        let start = self.expect_word("weave")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        while self.kind() != TokenKind::RParen {
            let (param, param_span) = self.binding_name()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.type_ref()?;
            params.push(Param {
                span: param_span.merge(ty.span),
                name: param,
                ty,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let returns = if self.eat(TokenKind::Arrow) {
            Some(self.type_ref()?)
        } else {
            None
        };
        let using_policy = if self.eat_word("using") {
            Some(self.any_name()?.0)
        } else {
            None
        };

        self.expect(TokenKind::LBrace)?;
        let body = self.block()?;
        self.expect(TokenKind::RBrace)?;

        Ok(WeaveDecl {
            name,
            params,
            returns,
            using_policy,
            body,
            span: start,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();
        while self.kind() != TokenKind::RBrace && !self.at_end() && !self.at_match_arm() {
            statements.push(self.statement()?);
            self.eat(TokenKind::Semicolon);
        }
        Ok(statements)
    }

    /// Whether the cursor sits on `case =>` or `case(binding) =>`.
    fn at_match_arm(&self) -> bool {
        if self.kind() != TokenKind::Ident {
            return false;
        }
        if self.peek_at(1).kind == TokenKind::FatArrow {
            return true;
        }
        self.peek_at(1).kind == TokenKind::LParen
            && self.peek_at(2).kind == TokenKind::Ident
            && self.peek_at(3).kind == TokenKind::RParen
            && self.peek_at(4).kind == TokenKind::FatArrow
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        if self.kind() == TokenKind::At {
            let mut attributes = Vec::new();
            while self.kind() == TokenKind::At {
                attributes.push(self.attribute()?);
            }
            return self.let_stmt(attributes);
        }
        if self.kind() != TokenKind::Ident {
            return Err(self.unexpected("a statement"));
        }

        match self.peek().text.as_str() {
            "bind" => {
                let start = self.bump().span;
                let (name, _) = self.binding_name()?;
                self.expect_word("to")?;
                self.expect_word("role")?;
                let (role, role_span) = self.any_name()?;
                Ok(Stmt::Bind {
                    name,
                    role,
                    span: start.merge(role_span),
                })
            }
            "let" => self.let_stmt(Vec::new()),
            "send" => {
                let start = self.bump().span;
                let (act, _) = self.any_name()?;
                let mut arguments = Vec::new();
                if self.eat(TokenKind::LParen) {
                    while self.kind() != TokenKind::RParen {
                        arguments.push(self.expression()?);
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                self.expect_word("from")?;
                let (from, _) = self.any_name()?;
                self.expect_word("to")?;
                let (to, to_span) = self.any_name()?;
                Ok(Stmt::Send {
                    act,
                    arguments,
                    from,
                    to,
                    span: start.merge(to_span),
                })
            }
            "match" => {
                let start = self.bump().span;
                let scrutinee = self.expression()?;
                self.expect(TokenKind::LBrace)?;
                let mut arms = Vec::new();
                while self.kind() != TokenKind::RBrace {
                    let (case, case_span) = self.any_name()?;
                    let binding = if self.eat(TokenKind::LParen) {
                        let (binding, _) = self.binding_name()?;
                        self.expect(TokenKind::RParen)?;
                        Some(binding)
                    } else {
                        None
                    };
                    self.expect(TokenKind::FatArrow)?;
                    let body = self.block()?;
                    arms.push(MatchArm {
                        pattern: Pattern {
                            case,
                            binding,
                            span: case_span,
                        },
                        body,
                        span: case_span,
                    });
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Match {
                    scrutinee,
                    arms,
                    span: start,
                })
            }
            "par" => {
                let start = self.bump().span;
                self.expect(TokenKind::LBrace)?;
                let body = self.block()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Par { body, span: start })
            }
            "race" => {
                let start = self.bump().span;
                let mut words = Vec::new();
                while self.kind() == TokenKind::Ident && !self.at_word("branch") {
                    words.push(self.bump().text);
                }
                self.expect(TokenKind::LBrace)?;
                let branches = self.branches()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Race {
                    policy: words.join(" "),
                    branches,
                    span: start,
                })
            }
            "checkpoint" => {
                let start = self.bump().span;
                let (name, _) = self.binding_name()?;
                self.expect(TokenKind::Equals)?;
                let (source, source_span) = self.any_name()?;
                Ok(Stmt::Checkpoint {
                    name,
                    source,
                    span: start.merge(source_span),
                })
            }
            "fork" => {
                let start = self.bump().span;
                self.expect_word("from")?;
                let (from, _) = self.any_name()?;
                self.expect(TokenKind::LBrace)?;
                let branches = self.branches()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Fork {
                    from,
                    branches,
                    span: start,
                })
            }
            "join" => {
                let start = self.bump().span;
                self.expect_word("using")?;
                let (using, using_span) = self.any_name()?;
                Ok(Stmt::Join {
                    using,
                    span: start.merge(using_span),
                })
            }
            "commit" => self.commit_stmt().map(Stmt::Commit),
            "watch" => {
                let start = self.bump().span;
                let (subject, _) = self.any_name()?;
                self.expect_word("where")?;
                let condition = self.path()?;
                self.expect(TokenKind::LBrace)?;
                let mut actions = Vec::new();
                while self.kind() != TokenKind::RBrace {
                    if self.at_word("pause") {
                        let action_span = self.bump().span;
                        self.expect_word("effects")?;
                        actions.push(WatchAction::PauseEffects {
                            effects: self.path_list()?,
                            span: action_span,
                        });
                    } else if self.at_word("spawn") {
                        let action_span = self.bump().span;
                        self.expect_word("role")?;
                        let (role, _) = self.any_name()?;
                        actions.push(WatchAction::SpawnRole {
                            role,
                            span: action_span,
                        });
                    } else {
                        return Err(ParseError::UnknownClause {
                            construct: "watch",
                            clause: self.peek().text.clone(),
                            span: self.peek().span,
                        });
                    }
                }
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Watch {
                    subject,
                    condition,
                    actions,
                    span: start,
                })
            }
            "context" => self.context_stmt().map(Stmt::Context),
            "stop" => {
                let start = self.bump().span;
                let (outcome, _) = self.any_name()?;
                self.expect_word("when")?;
                let condition = self.expression()?;
                Ok(Stmt::Stop {
                    outcome,
                    condition,
                    span: start,
                })
            }
            "return" => {
                let start = self.bump().span;
                let value = self.expression()?;
                Ok(Stmt::Return { value, span: start })
            }
            "execute" => {
                let start = self.bump().span;
                let value = self.expression()?;
                Ok(Stmt::Execute { value, span: start })
            }
            "resolve" => {
                let start = self.bump().span;
                let value = self.expression()?;
                Ok(Stmt::Resolve { value, span: start })
            }
            "delegate" => {
                let start = self.bump().span;
                let value = self.expression()?;
                Ok(Stmt::Delegate { value, span: start })
            }
            "publish" => {
                let start = self.bump().span;
                let value = self.expression()?;
                self.expect_word("into")?;
                let (into, into_span) = self.any_name()?;
                Ok(Stmt::Publish {
                    value,
                    into,
                    span: start.merge(into_span),
                })
            }
            "repeat" => {
                let start = self.bump().span;
                self.expect_word("until")?;
                let until = self.expression()?;
                self.expect(TokenKind::LBrace)?;
                let body = self.block()?;
                self.expect(TokenKind::RBrace)?;
                Ok(Stmt::Repeat {
                    until,
                    body,
                    span: start,
                })
            }
            "spawn" => {
                let start = self.bump().span;
                self.expect_word("role")?;
                let (role, role_span) = self.any_name()?;
                Ok(Stmt::Spawn {
                    role,
                    span: start.merge(role_span),
                })
            }
            other => Err(ParseError::UnknownClause {
                construct: "weave",
                clause: other.to_string(),
                span: self.peek().span,
            }),
        }
    }

    fn attribute(&mut self) -> Result<Attribute, ParseError> {
        let start = self.expect(TokenKind::At)?.span;
        let (name, name_span) = self.any_name()?;
        let mut arguments = Vec::new();
        if self.eat(TokenKind::LParen) {
            while self.kind() != TokenKind::RParen {
                let (key, _) = self.any_name()?;
                self.expect(TokenKind::Equals)?;
                let value = self.literal()?;
                arguments.push((key, value));
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
        }
        Ok(Attribute {
            name,
            arguments,
            span: start.merge(name_span),
        })
    }

    fn let_stmt(&mut self, attributes: Vec<Attribute>) -> Result<Stmt, ParseError> {
        let start = self.expect_word("let")?.span;
        let (name, _) = self.binding_name()?;
        self.expect(TokenKind::Equals)?;
        let value = self.expression()?;
        Ok(Stmt::Let {
            attributes,
            name,
            span: start.merge(value.span()),
            value,
        })
    }

    fn branches(&mut self) -> Result<Vec<Branch>, ParseError> {
        let mut branches = Vec::new();
        while self.at_word("branch") {
            let start = self.bump().span;
            let (name, _) = self.any_name()?;
            let mut budget = Vec::new();
            if self.eat_word("with") {
                self.expect_word("budget")?;
                loop {
                    let (resource, resource_span) = self.any_name()?;
                    self.expect(TokenKind::LParen)?;
                    let amount = self.integer()?;
                    self.expect(TokenKind::RParen)?;
                    budget.push(BudgetGrant {
                        resource,
                        amount,
                        span: resource_span,
                    });
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::LBrace)?;
            let body = self.block()?;
            self.expect(TokenKind::RBrace)?;
            branches.push(Branch {
                name,
                budget,
                body,
                span: start,
            });
        }
        Ok(branches)
    }

    fn commit_stmt(&mut self) -> Result<CommitStmt, ParseError> {
        let start = self.expect_word("commit")?.span;
        let (debtor, _) = self.any_name()?;
        self.expect_word("to")?;
        let (creditor, _) = self.any_name()?;
        self.expect_word("when")?;
        let trigger = self.path()?;
        self.expect(TokenKind::LBrace)?;

        let mut deliver = None;
        let mut before = None;
        let mut satisfy = Vec::new();
        let mut compensate = None;

        while self.kind() != TokenKind::RBrace {
            let clause = self.peek().clone();
            match clause.text.as_str() {
                "deliver" => {
                    self.bump();
                    deliver = Some(self.type_ref()?);
                }
                "before" => {
                    self.bump();
                    let token = self.expect(TokenKind::Duration)?;
                    before = Some(duration_literal(&token.text, token.span)?);
                }
                "satisfy" => {
                    self.bump();
                    self.expect_word("with")?;
                    satisfy.push(self.path()?);
                }
                "compensate" => {
                    let compensate_span = self.bump().span;
                    let (action, _) = self.any_name()?;
                    self.expect_word("on")?;
                    let (on, _) = self.any_name()?;
                    compensate = Some(Compensation {
                        action,
                        on,
                        span: compensate_span,
                    });
                }
                other => {
                    return Err(ParseError::UnknownClause {
                        construct: "commit",
                        clause: other.to_string(),
                        span: clause.span,
                    })
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        let deliver = deliver.ok_or(ParseError::UnknownClause {
            construct: "commit",
            clause: "deliver".to_string(),
            span: start,
        })?;

        Ok(CommitStmt {
            debtor,
            creditor,
            trigger,
            deliver,
            before,
            satisfy,
            compensate,
            span: start,
        })
    }

    fn context_stmt(&mut self) -> Result<ContextStmt, ParseError> {
        let start = self.expect_word("context")?.span;
        self.expect_word("for")?;
        let (recipient, _) = self.any_name()?;
        self.expect(TokenKind::LBrace)?;

        let mut includes = Vec::new();
        let mut resolution = None;
        let mut max_tokens = None;

        while self.kind() != TokenKind::RBrace {
            let clause = self.peek().clone();
            match clause.text.as_str() {
                "include" => {
                    self.bump();
                    if self.kind() == TokenKind::LBracket {
                        for path in self.path_list()? {
                            includes.push(Include {
                                subject: path.text(),
                                selectors: Vec::new(),
                                limit: None,
                                span: path.span,
                            });
                        }
                        continue;
                    }
                    let (subject, subject_span) = self.any_name()?;
                    let mut selectors = Vec::new();
                    let mut limit = None;
                    while self.kind() == TokenKind::Ident
                        && !matches!(
                            self.peek().text.as_str(),
                            "include" | "resolution" | "max-tokens" | "exclude"
                        )
                    {
                        let (selector, _) = self.any_name()?;
                        if selector == "top" && self.kind() == TokenKind::Integer {
                            limit = Some(self.integer()?);
                        } else {
                            selectors.push(selector);
                        }
                    }
                    includes.push(Include {
                        subject,
                        selectors,
                        limit,
                        span: subject_span,
                    });
                }
                "resolution" => {
                    self.bump();
                    resolution = Some(self.any_name()?.0);
                }
                "max-tokens" => {
                    self.bump();
                    max_tokens = Some(self.integer()?);
                }
                other => {
                    return Err(ParseError::UnknownClause {
                        construct: "context",
                        clause: other.to_string(),
                        span: clause.span,
                    })
                }
            }
        }
        self.expect(TokenKind::RBrace)?;

        Ok(ContextStmt {
            recipient,
            includes,
            resolution,
            max_tokens,
            span: start,
        })
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.or_expression()
    }

    fn or_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.and_expression()?;
        while self.at_word("or") {
            self.bump();
            let right = self.and_expression()?;
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

    fn and_expression(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.comparison()?;
        while self.at_word("and") {
            self.bump();
            let right = self.comparison()?;
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

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.primary()?;
        let op = match self.kind() {
            TokenKind::LessEq => BinaryOp::LessEq,
            TokenKind::GreaterEq => BinaryOp::GreaterEq,
            TokenKind::Less => BinaryOp::Less,
            TokenKind::Greater => BinaryOp::Greater,
            _ => return Ok(left),
        };
        self.bump();
        let right = self.primary()?;
        let span = left.span().merge(right.span());
        Ok(Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        })
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        if self.at_word("ask") {
            let start = self.bump().span;
            let call = self.primary()?;
            let span = start.merge(call.span());
            return Ok(Expr::Ask {
                call: Box::new(call),
                span,
            });
        }
        if self.at_word("await") {
            let start = self.bump().span;
            let target = self.path()?;
            let span = start.merge(target.span);
            return Ok(Expr::Await { target, span });
        }
        if self.at_word("choose") {
            let start = self.bump().span;
            let (subject, _) = self.any_name()?;
            self.expect_word("by")?;
            let by = self.path()?;
            let span = start.merge(by.span);
            return Ok(Expr::Choose { subject, by, span });
        }
        if self.at_word("current") {
            let span = self.bump().span;
            return Ok(Expr::Current { span });
        }
        if matches!(
            self.kind(),
            TokenKind::Integer | TokenKind::Float | TokenKind::Duration | TokenKind::Str
        ) {
            let span = self.peek().span;
            let value = self.literal()?;
            return Ok(Expr::Literal { value, span });
        }

        let path = self.path()?;
        if self.kind() != TokenKind::LParen {
            return Ok(Expr::Path(path));
        }
        self.bump();
        let mut arguments = Vec::new();
        while self.kind() != TokenKind::RParen {
            let name =
                if self.kind() == TokenKind::Ident && self.peek_at(1).kind == TokenKind::Colon {
                    let (name, _) = self.any_name()?;
                    self.bump();
                    Some(name)
                } else {
                    None
                };
            let value = self.expression()?;
            arguments.push(Argument {
                span: value.span(),
                name,
                value,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(TokenKind::RParen)?.span;
        Ok(Expr::Call {
            span: path.span.merge(end),
            callee: path,
            arguments,
        })
    }

    fn literal(&mut self) -> Result<Literal, ParseError> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Integer => {
                self.bump();
                token
                    .text
                    .parse::<i64>()
                    .map(Literal::Integer)
                    .map_err(|_| ParseError::Malformed {
                        expected: "integer",
                        text: token.text.clone(),
                        span: token.span,
                    })
            }
            TokenKind::Float => {
                self.bump();
                token
                    .text
                    .parse::<f64>()
                    .map(Literal::Float)
                    .map_err(|_| ParseError::Malformed {
                        expected: "number",
                        text: token.text.clone(),
                        span: token.span,
                    })
            }
            TokenKind::Duration => {
                self.bump();
                duration_literal(&token.text, token.span)
            }
            TokenKind::Str => {
                self.bump();
                Ok(Literal::Text(token.text))
            }
            _ => Err(self.unexpected("a literal")),
        }
    }
}

/// Converts `15m` into milliseconds.
fn duration_literal(text: &str, span: Span) -> Result<Literal, ParseError> {
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .ok_or(ParseError::Malformed {
            expected: "duration",
            text: text.to_string(),
            span,
        })?;
    let (digits, unit) = text.split_at(split);
    let value: u64 = digits.parse().map_err(|_| ParseError::Malformed {
        expected: "duration",
        text: text.to_string(),
        span,
    })?;
    let scale = match unit {
        "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        _ => {
            return Err(ParseError::Malformed {
                expected: "duration",
                text: text.to_string(),
                span,
            })
        }
    };
    Ok(Literal::Duration {
        millis: value.saturating_mul(scale),
        text: text.to_string(),
    })
}

/// Converts a decimal money amount into exact minor units, refusing more than two decimal places.
fn minor_units_of(text: &str, span: Span) -> Result<i64, ParseError> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.len() > 2 {
        return Err(ParseError::Malformed {
            expected: "money amount with at most two decimal places",
            text: text.to_string(),
            span,
        });
    }
    let whole: i64 = whole.parse().map_err(|_| ParseError::Malformed {
        expected: "money amount",
        text: text.to_string(),
        span,
    })?;
    let padded = format!("{fraction:0<2}");
    let minor: i64 = padded.parse().map_err(|_| ParseError::Malformed {
        expected: "money amount",
        text: text.to_string(),
        span,
    })?;
    Ok(whole * 100 + minor)
}
