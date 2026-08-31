//! Lexer and parser behaviour: what BioQL reads, and what it refuses to read.

use bioprism_biolang::bioql::{
    lex, parse, BinaryOp, ExpansionPolicy, Expr, Literal, Projection, ProvenanceMode, TokenKind,
};
use bioprism_biolang::error::{LexError, ParseError, QueryError};
use bioprism_biolang::Canonical;

const MINIMAL: &str = r#"select tumor_volume from lesions labels {} cost limit 10"#;

#[test]
fn a_lexed_query_always_ends_with_exactly_one_end_token() {
    let tokens = lex(MINIMAL).expect("lexes");
    assert!(tokens.last().expect("non-empty").kind.is_end());
    assert_eq!(
        tokens.iter().filter(|token| token.kind.is_end()).count(),
        1,
        "the end token marks the end, so there is one of it"
    );
}

#[test]
fn a_comment_runs_to_end_of_line_and_never_swallows_the_next_clause() {
    let tokens =
        lex("select a -- everything after this is a comment\nfrom lesions").expect("lexes");
    let keywords: Vec<&TokenKind> = tokens.iter().map(|token| &token.kind).collect();
    assert!(
        keywords
            .iter()
            .any(|kind| matches!(kind, TokenKind::Ident(name) if name == "lesions")),
        "the collection after the comment survives"
    );
}

#[test]
fn a_span_points_at_the_character_that_broke_the_lex() {
    let error = lex("select a from b where c ~ 1").unwrap_err();
    let LexError::UnexpectedCharacter { found, span } = error else {
        panic!("expected an unexpected character");
    };
    assert_eq!(found, '~');
    assert_eq!(span.column, 25, "1-based column of the tilde");
}

#[test]
fn an_unterminated_string_is_reported_at_its_opening_quote() {
    let error = lex("select a from b where c == \"open").unwrap_err();
    let LexError::UnterminatedString { span } = error else {
        panic!("expected an unterminated string");
    };
    assert_eq!(span.start, 27);
}

#[test]
fn a_parse_error_names_the_token_that_broke_the_parse() {
    let error = parse("select tumor_volume from 42").unwrap_err();
    let QueryError::Parse(ParseError::UnexpectedToken {
        found, expected, ..
    }) = error
    else {
        panic!("expected an unexpected token");
    };
    assert_eq!(found, "number `42`");
    assert!(expected.contains("collection name"));
}

#[test]
fn a_query_that_ends_early_says_so_rather_than_naming_a_token() {
    let error = parse("select tumor_volume from").unwrap_err();
    assert!(matches!(
        error,
        QueryError::Parse(ParseError::UnexpectedEnd { .. })
    ));
}

#[test]
fn a_repeated_clause_is_a_duplicate_not_a_merge() {
    let error = parse("select a from b where a == 1 where a == 2").unwrap_err();
    let QueryError::Parse(ParseError::DuplicateClause { clause, .. }) = error else {
        panic!("expected a duplicate clause");
    };
    assert_eq!(clause, "where");
}

#[test]
fn clauses_written_out_of_order_do_not_get_reordered() {
    let error = parse("select a from b cost limit 5 where a == 1").unwrap_err();
    let QueryError::Parse(ParseError::ClauseOutOfOrder { clause, after, .. }) = error else {
        panic!("expected an out-of-order clause");
    };
    assert_eq!(clause, "where");
    assert_eq!(after, "cost");
}

#[test]
fn an_identifier_after_a_number_that_is_not_a_unit_is_reported_as_a_unit_error() {
    let error = parse("select a from b where a > 3 furlong labels {} cost limit 5").unwrap_err();
    let QueryError::Parse(ParseError::UnknownUnit { symbol, .. }) = error else {
        panic!("expected an unknown unit, got {error:?}");
    };
    assert_eq!(symbol, "furlong");
}

#[test]
fn a_slash_unit_is_taken_only_when_the_joined_symbol_is_in_the_table() {
    let query = parse("select a from b where dose > 150 mg/m2 labels {} cost limit 5")
        .expect("mg/m2 is in the standards unit table");
    let Some(Expr::Binary { right, .. }) = query.filter.as_ref() else {
        panic!("expected a comparison");
    };
    let Expr::Literal {
        value: Literal::Number {
            unit: Some(unit), ..
        },
        ..
    } = right.as_ref()
    else {
        panic!("expected a quantity literal");
    };
    assert_eq!(unit.symbol, "mg/m2");
}

#[test]
fn a_slash_after_a_unit_that_does_not_join_stays_a_division() {
    let query = parse("select a from b where dose > 150 mg / weight labels {} cost limit 5")
        .expect("parses as a division");
    let Some(Expr::Binary { right, .. }) = query.filter.as_ref() else {
        panic!("expected a comparison");
    };
    assert!(
        matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::Divide,
                ..
            }
        ),
        "mg/weight is not a unit, so the slash is the division operator"
    );
}

#[test]
fn comparison_does_not_chain() {
    let error = parse("select a from b where x < y < z labels {} cost limit 5").unwrap_err();
    assert!(
        matches!(error, QueryError::Parse(ParseError::UnexpectedToken { .. })),
        "a < b < c is a mistake in every language where it parses"
    );
}

#[test]
fn not_binds_looser_than_comparison_and_tighter_than_and() {
    let query = parse("select a from b where not x == 1 and y == 2 labels {} cost limit 5")
        .expect("parses");
    let Some(Expr::Binary {
        op: BinaryOp::And,
        left,
        ..
    }) = query.filter.as_ref()
    else {
        panic!("the top of the tree is the conjunction");
    };
    assert!(
        matches!(left.as_ref(), Expr::Unary { .. }),
        "`not x == 1` is the negation of the comparison, not a comparison of a negation"
    );
}

#[test]
fn every_clause_round_trips_through_the_parser_into_the_same_tree() {
    let source = r#"select lesion.volume, site
                    from lesions
                    in { site: "SITE-A", subject: {"S1","S2"} }
                    where lesion.volume > 12.5 mm3 and not site == "SITE-B"
                    expand ontology "mondo" release "2026-03-01" policy descendants
                    at event
                    labels { "phi:deidentified" }
                    aggregate mean(lesion.volume) provenance source_lineage
                    cost limit 5000"#;
    let query = parse(source).expect("parses");

    assert!(matches!(query.select, Projection::Fields { .. }));
    assert_eq!(query.from.name, "lesions");
    assert_eq!(query.scope.as_ref().expect("scope").bindings.len(), 2);
    assert_eq!(
        query.expand.as_ref().expect("expansion").policy,
        ExpansionPolicy::Descendants
    );
    assert_eq!(query.labels.as_ref().expect("labels").labels.len(), 1);
    assert_eq!(
        query
            .aggregate
            .as_ref()
            .expect("aggregate")
            .provenance
            .expect("provenance")
            .mode,
        ProvenanceMode::SourceLineage
    );
    assert_eq!(query.cost.expect("cost").limit, 5000);

    let reparsed = parse(source).expect("parses again");
    assert_eq!(
        query.digest().expect("digests"),
        reparsed.digest().expect("digests"),
        "parsing is a function"
    );
}

#[test]
fn whitespace_and_comments_do_not_change_the_parsed_tree() {
    let dense = "select a from b where a > 1 labels {} cost limit 5";
    let spaced = "select   a\n  from b\n  -- a comment\n  where a > 1\n  labels {}\n  cost limit 5";
    let left = parse(dense).expect("parses");
    let right = parse(spaced).expect("parses");
    assert_eq!(
        left.filter.as_ref().map(|f| f.predicate_count()),
        right.filter.as_ref().map(|f| f.predicate_count())
    );
    assert_eq!(left.from.name, right.from.name);
}

#[test]
fn a_cost_limit_must_be_a_whole_number() {
    let error = parse("select a from b labels {} cost limit 5.5").unwrap_err();
    assert!(matches!(
        error,
        QueryError::Parse(ParseError::UnexpectedToken { .. })
    ));
}

#[test]
fn a_cost_limit_outside_u64_is_rejected_instead_of_saturating() {
    let error = parse("select a from b labels {} cost limit 18446744073709551616").unwrap_err();
    assert!(matches!(
        error,
        QueryError::Parse(ParseError::UnexpectedToken { .. })
    ));
}

#[test]
fn a_malformed_timestamp_literal_is_a_parse_error_carrying_the_text() {
    let error = parse(r#"select a from b where t > instant "not-a-time" labels {} cost limit 5"#)
        .unwrap_err();
    let QueryError::Parse(ParseError::MalformedTimestamp { text, .. }) = error else {
        panic!("expected a malformed timestamp");
    };
    assert_eq!(text, "not-a-time");
}
