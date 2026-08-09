//! Parser and lexer invariants (blueprint 23.37, and 23.03 phase 1).

use bioprism_weavelang::ast::*;
use bioprism_weavelang::diagnostic::Diagnostic;
use bioprism_weavelang::lexer::{tokenize, TokenKind};
use bioprism_weavelang::parser::{parse, ParseError};
use bioprism_weavelang::printer::print;
use bioprism_weavelang::reference::{COMPLETE_PROGRAM, CONTROL_FLOW_REFERENCE, SYNTAX_REFERENCE};
use serde_json::Value;

/// A tree with every source position removed.
///
/// Round-tripping through source cannot preserve byte offsets, and requiring it to would test the
/// formatter's whitespace rather than the grammar. Everything else must survive exactly.
fn without_spans(program: &Program) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("span");
                for nested in map.values_mut() {
                    strip(nested);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(program).expect("the AST serialises");
    strip(&mut value);
    value
}

#[test]
fn every_declaration_block_in_the_syntax_reference_parses() {
    let program = parse(SYNTAX_REFERENCE).expect("23.37's declaration blocks must parse");

    let package = program.package.expect("23.37 declares a package");
    assert_eq!(package.name.text(), "aurora:reliable-repair@0.1.0");
    assert_eq!(program.imports.len(), 2);
    assert_eq!(program.imports[1].alias.as_deref(), Some("git"));

    let names: Vec<&str> = program.items.iter().map(Item::name).collect();
    assert_eq!(
        names,
        vec![
            "hypothesis",
            "outcome",
            "investigator",
            "skeptic",
            "safe-repair",
            "review",
            "run"
        ]
    );
}

#[test]
fn every_control_flow_block_in_the_syntax_reference_parses() {
    let program = parse(CONTROL_FLOW_REFERENCE).expect("23.37's statement blocks must parse");
    let Some(Item::Weave(weave)) = program
        .items
        .iter()
        .find(|item| matches!(item, Item::Weave(_)))
    else {
        panic!("the control-flow reference declares one weave");
    };

    let kinds: Vec<&'static str> = weave
        .body
        .iter()
        .map(|statement| match statement {
            Stmt::Par { .. } => "par",
            Stmt::Race { .. } => "race",
            Stmt::Checkpoint { .. } => "checkpoint",
            Stmt::Fork { .. } => "fork",
            Stmt::Join { .. } => "join",
            Stmt::Commit(_) => "commit",
            Stmt::Watch { .. } => "watch",
            Stmt::Context(_) => "context",
            Stmt::Let { .. } => "let",
            Stmt::Stop { .. } => "stop",
            other => panic!("unexpected statement {other:?}"),
        })
        .collect();

    assert_eq!(
        kinds,
        vec![
            "par",
            "race",
            "checkpoint",
            "fork",
            "join",
            "commit",
            "watch",
            "context",
            "let",
            "stop",
            "stop",
            "stop"
        ]
    );
}

#[test]
fn a_kebab_case_identifier_is_one_token_but_an_arrow_is_not() {
    let tokens = tokenize("first-valid a->b").expect("lexes");
    let kinds: Vec<TokenKind> = tokens.iter().map(|token| token.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Ident,
            TokenKind::Ident,
            TokenKind::Arrow,
            TokenKind::Ident,
            TokenKind::Eof
        ]
    );
    assert_eq!(tokens[0].text, "first-valid");
    assert_eq!(tokens[1].text, "a");
}

#[test]
fn a_duration_suffix_is_only_a_duration_when_no_word_follows_it() {
    let tokens = tokenize("15m 15minutes").expect("lexes");
    assert_eq!(tokens[0].kind, TokenKind::Duration);
    assert_eq!(tokens[0].text, "15m");
    assert_eq!(tokens[1].kind, TokenKind::Integer);
    assert_eq!(tokens[2].kind, TokenKind::Ident);
    assert_eq!(tokens[2].text, "minutes");
}

#[test]
fn a_three_component_version_is_not_a_number() {
    let tokens = tokenize("0.1.0 0.1 120_000").expect("lexes");
    assert_eq!(tokens[0].kind, TokenKind::Version);
    assert_eq!(tokens[0].text, "0.1.0");
    assert_eq!(tokens[1].kind, TokenKind::Float);
    assert_eq!(tokens[2].kind, TokenKind::Integer);
    assert_eq!(tokens[2].text, "120000");
}

#[test]
fn a_parse_error_names_the_token_that_broke_the_parse_and_where_it_is() {
    let source = "policy p {\n  budget tokens 120000\n}\n";
    let error = parse(source).expect_err("`<=` is missing");
    let ParseError::Unexpected { text, span, .. } = &error else {
        panic!("expected an unexpected-token error, got {error:?}");
    };
    assert_eq!(text, "120000");
    assert_eq!(span.line, 2);
    assert_eq!(error.code(), "WEAVE-E2001");
    assert!(error.to_string().contains("`<=`"));
}

#[test]
fn a_reserved_keyword_cannot_be_bound_as_a_name() {
    let source = "weave w() { let budget = ask a.b() }";
    let error = parse(source).expect_err("`budget` is reserved by 23.37");
    let ParseError::ReservedKeyword { keyword, .. } = &error else {
        panic!("expected a reserved-keyword error, got {error:?}");
    };
    assert_eq!(keyword, "budget");
    assert_eq!(error.code(), "WEAVE-E2002");
}

#[test]
fn a_reserved_keyword_is_still_legal_inside_a_dotted_path() {
    let program = parse("weave w() { stop partial when budget.exhausted }").expect("parses");
    let Some(Item::Weave(weave)) = program.items.first() else {
        panic!("one weave");
    };
    let Stmt::Stop { condition, .. } = &weave.body[0] else {
        panic!("a stop statement");
    };
    let Expr::Path(path) = condition else {
        panic!("a path condition");
    };
    assert_eq!(path.text(), "budget.exhausted");
}

#[test]
fn two_declarations_with_the_same_name_are_rejected_with_both_locations() {
    let source = "role a { requires [x.read] }\nrole a { requires [y.read] }\n";
    let error = parse(source).expect_err("a duplicate name is ambiguous");
    let ParseError::DuplicateDeclaration {
        name,
        span,
        previous,
        ..
    } = &error
    else {
        panic!("expected a duplicate-declaration error, got {error:?}");
    };
    assert_eq!(name, "a");
    assert_eq!(previous.line, 1);
    assert_eq!(span.line, 2);
}

#[test]
fn an_unterminated_block_comment_reports_where_it_opened() {
    let error = parse("weave w() { }\n/* never closed").expect_err("comment is open");
    assert_eq!(error.code(), "WEAVE-E1003");
    assert_eq!(error.span().expect("a span").line, 2);
}

#[test]
fn a_money_amount_keeps_exact_minor_units_rather_than_a_float() {
    let program = parse("policy p { budget money <= usd(2.05) }").expect("parses");
    let Some(Item::Policy(policy)) = program.items.first() else {
        panic!("one policy");
    };
    assert_eq!(
        policy.budgets[0].limit,
        Literal::Money {
            currency: "usd".into(),
            minor_units: 205
        }
    );
}

#[test]
fn a_money_amount_with_three_decimal_places_is_rejected_rather_than_rounded() {
    let error = parse("policy p { budget money <= usd(2.055) }").expect_err("sub-cent");
    assert_eq!(error.code(), "WEAVE-E2006");
}

#[test]
fn a_qualified_name_is_reassembled_from_adjacent_tokens_but_a_choreography_colon_is_not() {
    let program = parse(
        "package a:b/c@1.2.3\nchoreography x { Lead -> Reviewer: propose<Plan> }",
    )
    .expect("parses");
    assert_eq!(
        program.package.expect("a package").name.text(),
        "a:b/c@1.2.3"
    );
    let Some(Item::Choreography(choreography)) = program.items.first() else {
        panic!("one choreography");
    };
    let ChoreoStep::Message { from, to, act, .. } = &choreography.steps[0] else {
        panic!("a message step");
    };
    assert_eq!((from.as_str(), to.as_str(), act.as_str()), ("Lead", "Reviewer", "propose"));
}

#[test]
fn a_uri_reference_survives_the_lexer_that_does_not_know_about_uris() {
    let program = parse(
        "role r { minimum-profile prism://capability/challenge >= 0.80 }",
    )
    .expect("parses");
    let Some(Item::Role(role)) = program.items.first() else {
        panic!("one role");
    };
    let profile = role.minimum_profile.as_ref().expect("a minimum profile");
    assert_eq!(profile.reference, "prism://capability/challenge");
    assert!((profile.threshold - 0.80).abs() < f64::EPSILON);
}

#[test]
fn the_evaluation_hook_attribute_binds_to_the_let_it_precedes() {
    let program = parse(
        "weave w() {\n@decision-cell(capability=\"context.information-value\")\nlet next = choose evidence by information-value\n}",
    )
    .expect("parses");
    let Some(Item::Weave(weave)) = program.items.first() else {
        panic!("one weave");
    };
    let Stmt::Let {
        attributes, value, ..
    } = &weave.body[0]
    else {
        panic!("a let statement");
    };
    assert_eq!(attributes[0].name, "decision-cell");
    assert_eq!(
        attributes[0].arguments[0],
        (
            "capability".to_string(),
            Literal::Text("context.information-value".into())
        )
    );
    assert!(matches!(value, Expr::Choose { .. }));
}

#[test]
fn a_match_arm_body_ends_where_the_next_arm_begins() {
    let program = parse(COMPLETE_PROGRAM).expect("parses");
    let Some(Item::Weave(weave)) = program
        .items
        .iter()
        .find(|item| matches!(item, Item::Weave(_)))
    else {
        panic!("one weave");
    };
    let Stmt::Match { arms, .. } = weave
        .body
        .iter()
        .find(|statement| matches!(statement, Stmt::Match { .. }))
        .expect("a match")
    else {
        unreachable!()
    };
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].pattern.case, "accept");
    assert_eq!(arms[0].pattern.binding.as_deref(), Some("p"));
    assert_eq!(arms[0].body.len(), 1);
    assert_eq!(arms[1].pattern.case, "challenge");
    assert_eq!(arms[1].body.len(), 1);
}

#[test]
fn a_context_include_separates_selectors_from_a_top_n_limit() {
    let program = parse(CONTROL_FLOW_REFERENCE).expect("parses");
    let Some(Item::Weave(weave)) = program
        .items
        .iter()
        .find(|item| matches!(item, Item::Weave(_)))
    else {
        panic!("one weave");
    };
    let Some(Stmt::Context(context)) = weave
        .body
        .iter()
        .find(|statement| matches!(statement, Stmt::Context(_)))
    else {
        panic!("a context statement");
    };
    assert_eq!(context.includes.len(), 3);
    assert_eq!(context.includes[1].subject, "evidence");
    assert_eq!(context.includes[1].selectors, vec!["strongest", "both-sides"]);
    assert_eq!(context.includes[2].limit, Some(5));
    assert_eq!(context.resolution.as_deref(), Some("audit"));
    assert_eq!(context.max_tokens, Some(16000));
}

#[test]
fn a_fork_branch_carries_its_own_budget_lease() {
    let program = parse(CONTROL_FLOW_REFERENCE).expect("parses");
    let Some(Item::Weave(weave)) = program
        .items
        .iter()
        .find(|item| matches!(item, Item::Weave(_)))
    else {
        panic!("one weave");
    };
    let Some(Stmt::Fork { from, branches, .. }) = weave
        .body
        .iter()
        .find(|statement| matches!(statement, Stmt::Fork { .. }))
    else {
        panic!("a fork statement");
    };
    assert_eq!(from, "c");
    assert_eq!(branches.len(), 2);
    assert_eq!(
        branches[0].budget[0],
        BudgetGrant {
            resource: "tokens".into(),
            amount: 10000,
            span: branches[0].budget[0].span
        }
    );
}

#[test]
fn both_keyword_orders_for_a_policy_effect_clause_are_accepted() {
    let reference_order = parse("policy p { allow effects [a.read] }").expect("23.37's order");
    let overview_order = parse("policy p { effects allow [a.read] }").expect("23.02's order");
    let extract = |program: &Program| match program.items.first() {
        Some(Item::Policy(policy)) => policy
            .allow_effects
            .iter()
            .map(Path::text)
            .collect::<Vec<_>>(),
        _ => panic!("one policy"),
    };
    assert_eq!(extract(&reference_order), extract(&overview_order));
    assert_eq!(extract(&reference_order), vec!["a.read"]);
}

#[test]
fn parsing_printing_and_parsing_again_yields_the_same_tree() {
    for source in [SYNTAX_REFERENCE, CONTROL_FLOW_REFERENCE, COMPLETE_PROGRAM] {
        let first = parse(source).expect("parses");
        let printed = print(&first);
        let second = parse(&printed).unwrap_or_else(|error| {
            panic!("the printer emitted source the parser rejects: {error}\n{printed}")
        });
        assert_eq!(
            without_spans(&first),
            without_spans(&second),
            "round trip changed the tree; printed source was:\n{printed}"
        );
    }
}

#[test]
fn printing_a_program_twice_produces_the_same_text() {
    for source in [SYNTAX_REFERENCE, CONTROL_FLOW_REFERENCE, COMPLETE_PROGRAM] {
        let once = print(&parse(source).expect("parses"));
        let twice = print(&parse(&once).expect("parses"));
        assert_eq!(once, twice, "the canonical form must be a fixed point");
    }
}

#[test]
fn a_construct_that_only_the_overview_defines_is_refused_rather_than_guessed_at() {
    let error = parse("molecule paper-reproducer { bind extractor: claim-extractor }")
        .expect_err("23.37 gives `molecule` no body grammar");
    assert_eq!(error.code(), "WEAVE-E2003");
}
