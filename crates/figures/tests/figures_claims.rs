//! Claim-per-test coverage of the figure renderers: golden byte-stability against pinned digests,
//! refused rows drawn as states rather than zero bars, missing-field and inconsistency refusals,
//! escaping under hostile strings, and XML well-formedness checked by a hand-rolled parser.
//!
//! The certificate figures run against the repository's real golden reference certificate
//! (`fixtures/fiber-v0.1/golden`), embedded at compile time so the suite performs no runtime I/O.

use bioprism_figures::{
    autopilot_drive, baseline_panel, mutation_diversity, omission_accounting, selection_ratio,
    sweep_grid, FigureError,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const GOLDEN_CERTIFICATE: &str =
    include_str!("../../../fixtures/fiber-v0.1/golden/reference_certificate.json");

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn golden_certificate() -> Value {
    serde_json::from_str(GOLDEN_CERTIFICATE).expect("the golden certificate parses")
}

fn comparison_fixture() -> Value {
    json!({
        "world_id": "sweep-discriminating-d750",
        "query_id": "audit-split-integrity-v1",
        "total_facts": 762,
        "reference": { "source": "full-context", "status": "invalid", "witnesses": ["identity_leakage"] },
        "cheapest_admissible_strategy": "fiber",
        "results": [
            {
                "name": "full-context", "method": "expose every fact in the world",
                "facts_exposed": 762, "fraction_of_world": 1.0, "judged": true,
                "status": "invalid", "witnesses": ["identity_leakage"],
                "verdict_preserving": true, "missing_witnesses": [], "spurious_witnesses": [],
                "protected_recall": 1.0, "closure_complete": true, "admissible": true, "notes": []
            },
            {
                "name": "graph-5-hop", "method": "k-hop incidence walk",
                "facts_exposed": 750, "fraction_of_world": 0.984251968503937, "judged": true,
                "status": "valid", "witnesses": [],
                "verdict_preserving": false, "missing_witnesses": ["identity_leakage"], "spurious_witnesses": [],
                "protected_recall": 0.0, "closure_complete": false, "admissible": false, "notes": []
            },
            {
                "name": "lexical-top-11", "method": "lexical overlap top-k",
                "facts_exposed": 11, "fraction_of_world": 0.014435695538057743, "judged": true,
                "status": "invalid", "witnesses": ["identity_leakage"],
                "verdict_preserving": true, "missing_witnesses": [], "spurious_witnesses": [],
                "protected_recall": 0.9090909090909091, "closure_complete": false, "admissible": false, "notes": []
            },
            {
                "name": "fiber", "method": "protected closure, then backward dependency slice, then temporal cut",
                "facts_exposed": 11, "fraction_of_world": 0.014435695538057743, "judged": true,
                "status": "invalid", "witnesses": ["identity_leakage"],
                "verdict_preserving": true, "missing_witnesses": [], "spurious_witnesses": [],
                "protected_recall": 1.0, "closure_complete": true, "admissible": true, "notes": []
            },
            {
                "name": "directed-walk", "method": "directed dependency walk",
                "facts_exposed": 9, "fraction_of_world": 0.011811023622047244, "judged": false,
                "refusal": "the oracle refused the 9 fact(s) this strategy selected: alias spans subjects whose split arms cannot be ordered",
                "protected_recall": 0.2727272727272727, "closure_complete": false, "notes": []
            }
        ]
    })
}

fn sweep_row(strategy: &str, facts: u64, sound: Option<bool>, closure: f64, admissible: bool) -> Value {
    let mut row = json!({
        "strategy": strategy,
        "facts_selected": facts,
        "judged": sound.is_some(),
        "protected_closure": closure,
        "admissible": admissible,
    });
    if let Some(sound) = sound {
        row["sound"] = json!(sound);
    }
    row
}

fn sweep_cell(world_id: &str, attachment: &str, relay: u64, tag: &str, distractors: u64, rows: Vec<Value>) -> Value {
    json!({
        "world_id": world_id,
        "attachment": attachment,
        "relay_depth": relay,
        "tag_style": tag,
        "distractors": distractors,
        "total_facts": 762,
        "rows": rows,
    })
}

fn sweep_fixture() -> Value {
    json!({
        "ok": false,
        "seed": 20_260_823u64,
        "cells_total": 4,
        "admissible_cells": { "full-context": 4, "fiber": 2, "graph-5-hop": 1, "lexical-top-11": 1 },
        "cells": [
            sweep_cell("sweep-hub-r0-distinct-d50", "hub", 0, "distinct", 50, vec![
                sweep_row("full-context", 762, Some(true), 1.0, true),
                sweep_row("fiber", 11, Some(true), 1.0, true),
                sweep_row("graph-5-hop", 40, Some(true), 1.0, true),
                sweep_row("lexical-top-11", 11, Some(true), 0.9090909090909091, false),
            ]),
            sweep_cell("sweep-hub-r0-distinct-d250", "hub", 0, "distinct", 250, vec![
                sweep_row("full-context", 262, Some(true), 1.0, true),
                sweep_row("fiber", 11, Some(true), 1.0, true),
                sweep_row("graph-5-hop", 250, Some(false), 0.0, false),
                sweep_row("lexical-top-11", 11, Some(true), 0.9090909090909091, false),
            ]),
            sweep_cell("sweep-neartarget-r4-camouflaged-d50", "near_target", 4, "camouflaged", 50, vec![
                sweep_row("full-context", 62, Some(true), 1.0, true),
                sweep_row("fiber", 12, Some(false), 1.0, false),
                sweep_row("graph-5-hop", 50, Some(false), 0.0, false),
                sweep_row("lexical-top-11", 11, None, 0.5454545454545454, false),
            ]),
            sweep_cell("sweep-neartarget-r4-camouflaged-d250", "near_target", 4, "camouflaged", 250, vec![
                sweep_row("full-context", 262, Some(true), 1.0, true),
                sweep_row("fiber", 12, Some(false), 1.0, false),
                sweep_row("graph-5-hop", 60, Some(true), 1.0, true),
                sweep_row("lexical-top-11", 11, Some(true), 0.9090909090909091, false),
            ]),
        ]
    })
}

/// A sweep whose distractor list is far wider than the three-column default `world sweep` grid
/// uses — the CLI takes an arbitrary `--distractors` list, so the column count is input.
fn wide_sweep_fixture(columns: u64) -> Value {
    let cells: Vec<Value> = (0..columns)
        .map(|column| {
            sweep_cell(
                "sweep-hub-r0-distinct",
                "hub",
                0,
                "distinct",
                50 + column * 100,
                vec![
                    sweep_row("full-context", 762, Some(true), 1.0, true),
                    sweep_row("fiber", 11, Some(true), 1.0, true),
                    sweep_row("graph-5-hop", 40, Some(false), 0.0, false),
                ],
            )
        })
        .collect();
    json!({
        "ok": false,
        "seed": 20_260_823u64,
        "cells_total": cells.len(),
        "cells": cells,
    })
}

/// The viewBox width of a rendered figure.
fn viewbox_width(svg: &str) -> f64 {
    let start = svg.find("viewBox=\"0 0 ").expect("every figure declares a viewBox") + 13;
    let rest = &svg[start..];
    let end = rest.find('"').expect("the viewBox attribute is quoted");
    rest[..end]
        .split_whitespace()
        .next()
        .expect("the viewBox carries a width")
        .parse()
        .expect("the viewBox width is a number")
}

/// Every `x` (plus `width`, where the element has one) in the document, so a claim can assert that
/// nothing was drawn outside the frame that clips it.
fn right_edges(svg: &str) -> Vec<f64> {
    let mut edges = Vec::new();
    for element in svg.split('<').skip(1) {
        let Some(head) = element.split('>').next() else {
            continue;
        };
        let attribute = |name: &str| -> Option<f64> {
            let needle = format!("{name}=\"");
            let start = head.find(&needle)? + needle.len();
            let rest = &head[start..];
            rest[..rest.find('"')?].parse().ok()
        };
        let Some(x) = attribute("x").or_else(|| attribute("x2")) else {
            continue;
        };
        edges.push(x + attribute("width").unwrap_or(0.0));
    }
    edges
}

fn diversity_fixture() -> Value {
    json!({
        "instances": 240,
        "parents": 2,
        "families": 6,
        "signatures": 9,
        "equivalence_classes": 30,
        "inflation_ratio": 8.0,
        "caveat": "Equivalence classes are counted as distinct (parent, mutation family, oracle signature) triples. This measures independent diagnostic information, not difficulty or realism, and makes no claim about correlation beyond those three axes."
    })
}

fn drive_attempt(index: u64, kind: &str, mission_status: Option<&str>, dispatch_error: Option<&str>) -> Value {
    json!({
        "attempt_index": index,
        "kind": kind,
        "mission_digest": "0".repeat(64),
        "dispatched_step_ids": ["s1", "s2", "s3"],
        "report_digest": mission_status.map(|_| "1".repeat(64)),
        "outcome_summary": mission_status.map(|status| json!({
            "mission_status": status,
            "succeeded": ["s1"],
            "refused": [],
            "blocked": [],
            "cancelled": [],
            "required_failures": if status == "succeeded" { json!([]) } else { json!(["s2"]) },
        })),
        "dispatch_error": dispatch_error,
    })
}

fn drive_fixture() -> Value {
    json!({
        "schema": "bioprism-autopilot/report/0.1",
        "base_mission_id": "mission-alpha",
        "final_status": "succeeded",
        "totals": { "attempts_used": 2, "max_attempts": 4, "steps_in_plan": 3 },
        "attempts": [
            drive_attempt(1, "full", Some("failed"), None),
            drive_attempt(2, "repair", Some("succeeded"), None),
        ],
    })
}

fn drive_no_report_fixture() -> Value {
    json!({
        "schema": "bioprism-autopilot/report/0.1",
        "base_mission_id": "mission-beta",
        "final_status": "refused",
        "totals": { "attempts_used": 2, "max_attempts": 4, "steps_in_plan": 3 },
        "attempts": [
            drive_attempt(1, "full", Some("failed"), None),
            drive_attempt(2, "repair", None, Some("transport: connection reset before a report arrived")),
        ],
    })
}

type FigureCase = (&'static str, fn(&Value) -> Result<String, FigureError>, Value);

fn all_figures() -> Vec<(&'static str, String, Value)> {
    let cases: Vec<FigureCase> = vec![
        ("Equal-engineering baseline panel", baseline_panel, comparison_fixture()),
        ("Context selection ratio", selection_ratio, golden_certificate()),
        ("Reference omission accounting", omission_accounting, golden_certificate()),
        ("Structural family sweep", sweep_grid, sweep_fixture()),
        ("Effective diversity", mutation_diversity, diversity_fixture()),
        ("Autopilot drive", autopilot_drive, drive_fixture()),
    ];
    cases
        .into_iter()
        .map(|(title, figure, input)| {
            let svg = figure(&input).expect("every fixture renders");
            (title, svg, input)
        })
        .collect()
}

/// Strips tags and collapses whitespace so verbatim-text claims survive line wrapping.
fn visible_text(svg: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in svg.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn assert_valid_text_node(text: &str, context: &str) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        assert!(
            byte >= 0x20 || byte == b'\n' || byte == b'\r' || byte == b'\t',
            "raw control byte 0x{byte:02x} in {context}"
        );
        if byte == b'&' {
            let tail = &text[index..];
            let semicolon = tail
                .find(';')
                .unwrap_or_else(|| panic!("unterminated entity in {context}: {tail:.20}"));
            let entity = &tail[1..semicolon];
            let named = matches!(entity, "amp" | "lt" | "gt" | "quot" | "apos");
            let numeric = entity
                .strip_prefix('#')
                .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
            assert!(named || numeric, "invalid entity &{entity}; in {context}");
            index += semicolon + 1;
        } else {
            index += 1;
        }
    }
}

/// A hand-rolled well-formedness check: balanced tags, one root, quoted attributes, and text
/// nodes whose only ampersands are valid entities.
fn assert_well_formed_xml(svg: &str) {
    let mut stack: Vec<String> = Vec::new();
    let mut roots = 0usize;
    let mut rest = svg;
    while let Some(open) = rest.find('<') {
        assert_valid_text_node(&rest[..open], "a text node");
        let close = rest[open..]
            .find('>')
            .unwrap_or_else(|| panic!("unclosed tag near: {:.40}", &rest[open..]))
            + open;
        let tag = &rest[open + 1..close];
        assert!(!tag.is_empty(), "empty tag");
        assert!(!tag.contains('<'), "nested '<' inside tag <{tag}>");
        assert_eq!(tag.matches('"').count() % 2, 0, "unbalanced quotes in <{tag}>");
        if let Some(name) = tag.strip_prefix('/') {
            let expected = stack
                .pop()
                .unwrap_or_else(|| panic!("closing </{name}> with nothing open"));
            assert_eq!(expected, name.trim(), "mismatched closing tag");
            if stack.is_empty() {
                roots += 1;
            }
        } else {
            let self_closing = tag.ends_with('/');
            let body = if self_closing { &tag[..tag.len() - 1] } else { tag };
            let name = body
                .split_whitespace()
                .next()
                .unwrap_or_else(|| panic!("tag with no name: <{tag}>"))
                .to_string();
            if self_closing {
                if stack.is_empty() {
                    roots += 1;
                }
            } else {
                stack.push(name);
            }
        }
        rest = &rest[close + 1..];
    }
    assert_valid_text_node(rest, "the document tail");
    assert!(stack.is_empty(), "unclosed elements remain: {stack:?}");
    assert_eq!(roots, 1, "expected exactly one root element");
}

const BASELINE_PANEL_SHA256: &str =
    "78c18d898c95a895c8ae36564cbaefb70fa1675e50810c5a0b22cdea500ab774";
const SELECTION_RATIO_SHA256: &str =
    "d4e6bae9049b455c10e0e3d78fa0b08e6d41fbba3bb486c0c6f0b3d420722c61";
const OMISSION_ACCOUNTING_SHA256: &str =
    "8a89ff459d8dae60473bd392e7b2b8b1cf63b37bd80eb99b421ffb1ce252154a";
const SWEEP_GRID_SHA256: &str =
    "eabe3595be81974d83b84f52d4d722aaf0498a9ab0dfb69924258f2a1c9f0545";
const MUTATION_DIVERSITY_SHA256: &str =
    "f3696ea8847ccafce00d0d81cac37e2ec169c54a9ed03a07bffa747dd5a8a4df";
const AUTOPILOT_DRIVE_SHA256: &str =
    "7f75efcf1c646f3b5f02c5e2718f9c27819ea734d83e349870acd764f32a8f53";

fn assert_byte_stable(
    figure: fn(&Value) -> Result<String, FigureError>,
    input: &Value,
    pinned: &str,
) {
    let first = figure(input).expect("the fixture renders");
    let second = figure(input).expect("the fixture renders twice");
    assert_eq!(first, second, "two renders of the same value must be identical bytes");
    let digest = sha256_hex(first.as_bytes());
    assert_eq!(digest, pinned, "figure bytes changed; actual sha256 is {digest}");
}

#[test]
fn a_baseline_panel_is_byte_stable_for_a_fixed_comparison() {
    assert_byte_stable(baseline_panel, &comparison_fixture(), BASELINE_PANEL_SHA256);
}

#[test]
fn a_selection_ratio_figure_is_byte_stable_for_the_golden_reference_certificate() {
    assert_byte_stable(selection_ratio, &golden_certificate(), SELECTION_RATIO_SHA256);
}

#[test]
fn an_omission_accounting_figure_is_byte_stable_for_the_golden_reference_certificate() {
    assert_byte_stable(omission_accounting, &golden_certificate(), OMISSION_ACCOUNTING_SHA256);
}

#[test]
fn a_sweep_figure_is_byte_stable_for_a_fixed_table() {
    assert_byte_stable(sweep_grid, &sweep_fixture(), SWEEP_GRID_SHA256);
}

#[test]
fn a_mutation_diversity_figure_is_byte_stable_for_a_fixed_diversity_document() {
    assert_byte_stable(mutation_diversity, &diversity_fixture(), MUTATION_DIVERSITY_SHA256);
}

#[test]
fn an_autopilot_drive_figure_is_byte_stable_for_a_fixed_report() {
    assert_byte_stable(autopilot_drive, &drive_fixture(), AUTOPILOT_DRIVE_SHA256);
}

#[test]
fn a_refused_row_is_rendered_as_refused_and_never_as_a_zero_length_bar() {
    let svg = baseline_panel(&comparison_fixture()).expect("the fixture renders");
    assert!(
        svg.contains("refused (not judged) — selected 9 facts"),
        "the refused row must be named as a refused state with its measured cost"
    );
    assert!(
        !svg.contains(" width=\"0\"") && !svg.contains(" width=\"0.00\""),
        "no zero-width geometry may stand in for a refusal"
    );
    let facts_selected_ratio = 9.0 / 762.0 * 300.0;
    assert!(
        !svg.contains(&format!(" width=\"{facts_selected_ratio:.2}\"")),
        "a refused row must not receive verdict-coloured bar geometry at all"
    );
}

#[test]
fn every_figure_is_well_formed_xml_with_exactly_one_root_element() {
    for (title, svg, _) in all_figures() {
        assert_well_formed_xml(&svg);
        assert!(svg.starts_with("<svg "), "{title} must start with the svg root");
    }
}

#[test]
fn every_figure_embeds_its_title_and_the_canonical_source_digest_of_its_input() {
    for (title, svg, input) in all_figures() {
        let digest = ContentHash::of_value(&input).expect("fixtures canonicalise");
        assert!(
            svg.contains(&format!("source sha256: {digest}")),
            "{title} must carry the canonical digest of exactly the value it rendered"
        );
        assert!(svg.contains(title), "{title} must render its title text");
    }
}

#[test]
fn hostile_strings_are_escaped_rather_than_injected_into_the_markup() {
    let hostile = "a<b&\"c\"";
    let mut comparison = comparison_fixture();
    comparison["world_id"] = json!(hostile);
    comparison["results"][4]["name"] = json!("<script>'attack'</script>");
    comparison["results"][4]["refusal"] = json!("refused & \"quoted\" <detail>");
    let svg = baseline_panel(&comparison).expect("hostile strings still render");
    assert_well_formed_xml(&svg);
    assert!(!svg.contains("a<b&"), "raw hostile bytes must never reach the markup");
    assert!(!svg.contains("<script>"), "markup injection must be impossible");
    assert!(svg.contains("a&lt;b&amp;&quot;c&quot;"), "the hostile text survives, escaped");

    let mut diversity = diversity_fixture();
    diversity["caveat"] = json!("evil\u{0001}\tcaveat with 'quotes' & <tags>");
    let svg = mutation_diversity(&diversity).expect("hostile caveats still render");
    assert_well_formed_xml(&svg);
    assert!(svg.contains("&apos;quotes&apos; &amp; &lt;tags&gt;"));
    assert!(!svg.contains('\u{0001}'), "control bytes must not survive into XML");
}

#[test]
fn a_missing_field_is_an_error_naming_the_dotted_path_never_a_silent_zero() {
    let mut comparison = comparison_fixture();
    comparison["results"][0]
        .as_object_mut()
        .expect("row is an object")
        .remove("facts_exposed");
    assert_eq!(
        baseline_panel(&comparison),
        Err(FigureError::MissingField { field: "results[0].facts_exposed".to_string() })
    );

    let mut certificate = golden_certificate();
    certificate["plan"]
        .as_object_mut()
        .expect("plan is an object")
        .remove("compiled_fact_count");
    assert_eq!(
        selection_ratio(&certificate),
        Err(FigureError::MissingField { field: "plan.compiled_fact_count".to_string() })
    );

    let mut certificate = golden_certificate();
    certificate["omissions"]
        .as_object_mut()
        .expect("omissions is an object")
        .remove("classification");
    assert_eq!(
        omission_accounting(&certificate),
        Err(FigureError::MissingField { field: "omissions.classification".to_string() })
    );

    let mut table = sweep_fixture();
    table["cells"][0]
        .as_object_mut()
        .expect("cell is an object")
        .remove("rows");
    assert_eq!(
        sweep_grid(&table),
        Err(FigureError::MissingField { field: "cells[0].rows".to_string() })
    );

    let mut diversity = diversity_fixture();
    diversity.as_object_mut().expect("diversity is an object").remove("caveat");
    assert_eq!(
        mutation_diversity(&diversity),
        Err(FigureError::MissingField { field: "caveat".to_string() })
    );

    let mut report = drive_fixture();
    report["totals"]
        .as_object_mut()
        .expect("totals is an object")
        .remove("attempts_used");
    assert_eq!(
        autopilot_drive(&report),
        Err(FigureError::MissingField { field: "totals.attempts_used".to_string() })
    );
}

#[test]
fn an_internally_contradictory_document_is_refused_rather_than_rendered() {
    let mut certificate = golden_certificate();
    certificate["plan"]["compiled_fact_count"] = json!(900);
    assert!(matches!(
        selection_ratio(&certificate),
        Err(FigureError::Inconsistent { .. })
    ));

    let mut comparison = comparison_fixture();
    comparison["results"][0]["closure_complete"] = json!(false);
    assert!(matches!(
        baseline_panel(&comparison),
        Err(FigureError::Inconsistent { .. })
    ));

    let mut diversity = diversity_fixture();
    diversity["inflation_ratio"] = json!(3.0);
    assert!(matches!(
        mutation_diversity(&diversity),
        Err(FigureError::Inconsistent { .. })
    ));

    let mut table = sweep_fixture();
    table["cells"][0]["rows"]
        .as_array_mut()
        .expect("rows is an array")
        .retain(|row| row["strategy"] != "fiber");
    assert!(matches!(sweep_grid(&table), Err(FigureError::Inconsistent { .. })));
}

#[test]
fn the_sweep_figure_draws_ties_as_prominently_as_wins_and_carries_the_unswept_knob_caveat_verbatim() {
    let svg = sweep_grid(&sweep_fixture()).expect("the fixture renders");
    let text = visible_text(&svg);
    assert!(
        text.contains("tie — FIBER and at least one baseline both admissible (a first-class result, drawn as prominently as a win)"),
        "the tie category must be legended as a first-class result"
    );
    assert!(
        text.contains(
            "The other WorldSpec knobs — skeleton, events, protected set, decision time, policy — \
             are deliberately not swept: they change what the decision is, not the structure \
             around it, and a sweep that varied them would be comparing strategies across \
             different questions."
        ),
        "the sweep's own scope caveat must travel verbatim"
    );
    assert!(
        text.contains("† cell contains a row the oracle refused — counted as neither sound nor unsound"),
        "a refused sweep row must be marked, not absorbed into unsound"
    );
    assert!(svg.contains("url(#hatch-accent)"), "tie cells use the accent hatch, not a washed-out tone");
}

#[test]
fn a_sweep_grid_with_many_distractor_columns_stays_inside_its_frame() {
    let default_columns = sweep_grid(&wide_sweep_fixture(3)).expect("three columns render");
    assert_eq!(
        viewbox_width(&default_columns),
        720.0,
        "a grid that fits the fixed frame must keep it, byte for byte"
    );

    for columns in [8u64, 12, 20] {
        let svg = sweep_grid(&wide_sweep_fixture(columns)).expect("a wide sweep renders");
        assert_well_formed_xml(&svg);
        let width = viewbox_width(&svg);
        assert!(
            width > 720.0,
            "{columns} columns need more than the fixed frame, but the viewBox is still {width}"
        );
        for edge in right_edges(&svg) {
            assert!(
                edge <= width,
                "{columns} columns: geometry reaches {edge}, past the {width}-unit viewBox that \
                 clips it"
            );
        }
        let last = 50 + (columns - 1) * 100;
        assert!(
            visible_text(&svg).contains(&format!(" {last} ")),
            "the highest distractor column must be labelled inside the frame, not clipped away"
        );
    }
}

#[test]
fn a_caption_longer_than_one_line_wraps_instead_of_losing_its_tail() {
    let mut comparison = comparison_fixture();
    comparison["world_id"] = json!("research-discriminating-d750");
    comparison["query_id"] = json!("research-discriminating-d750-split-integrity");
    let svg = baseline_panel(&comparison).expect("the fixture renders");
    assert_well_formed_xml(&svg);
    let text = visible_text(&svg);
    assert!(
        text.contains("reference verdict (full-context): invalid"),
        "the caption states the reference verdict every other row is judged against; truncating \
         it away leaves the panel unreadable"
    );
    assert!(!text.contains('…'), "nothing in this caption needed eliding");
    assert_eq!(
        svg.matches("font-size=\"11.5\"").count(),
        2,
        "a wrapped caption occupies two header lines rather than one clipped one"
    );
    assert!(svg.contains("y=\"60.00\""), "the second caption line sits under the first");
    assert!(
        svg.contains("<g transform=\"translate(0,74)\">"),
        "the body starts below the wrapped caption, never underneath it"
    );
}

#[test]
fn a_caption_stays_bounded_however_long_the_input_strings_are() {
    let mut comparison = comparison_fixture();
    comparison["world_id"] = json!("w".repeat(400));
    comparison["query_id"] = json!("q".repeat(400));
    let svg = baseline_panel(&comparison).expect("an absurd caption still renders");
    assert_well_formed_xml(&svg);
    let caption_lines = svg.matches("font-size=\"11.5\"").count();
    assert!(
        caption_lines <= 3,
        "the header must not grow without bound: {caption_lines} caption lines"
    );
    assert!(svg.contains('…'), "the bounded caption says it was cut");
}

#[test]
fn an_attempts_total_that_contradicts_the_attempts_array_is_refused_rather_than_captioned() {
    let mut report = drive_fixture();
    report["totals"]["attempts_used"] = json!(3);
    assert!(
        matches!(autopilot_drive(&report), Err(FigureError::Inconsistent { .. })),
        "a caption may not claim more attempts than the figure below it draws"
    );

    let mut report = drive_fixture();
    report["totals"]["attempts_used"] = json!(1);
    assert!(matches!(
        autopilot_drive(&report),
        Err(FigureError::Inconsistent { .. })
    ));

    let mut report = drive_fixture();
    report["totals"]["max_attempts"] = json!(1);
    assert!(
        matches!(autopilot_drive(&report), Err(FigureError::Inconsistent { .. })),
        "attempts used may not exceed the budget the same totals block declares"
    );
}

#[test]
fn the_autopilot_axis_is_labelled_as_a_logical_clock_free_sequence() {
    let svg = autopilot_drive(&drive_fixture()).expect("the fixture renders");
    assert!(
        svg.contains("attempt sequence (logical, clock-free)"),
        "the axis must state it is logical order, never wall-clock time"
    );
}

#[test]
fn an_attempt_without_a_report_is_drawn_as_no_report_not_as_a_failure() {
    let svg = autopilot_drive(&drive_no_report_fixture()).expect("the fixture renders");
    assert!(svg.contains("no report"), "an undelivered dispatch has no outcome to draw");
    assert!(
        svg.contains("outcome unknown (transport)"),
        "a transport error must be labelled unknown, not failed"
    );
    assert!(svg.contains("stroke-dasharray"), "the no-report box is an outline, not a verdict fill");
    assert!(svg.contains("final: refused"), "the final-status badge names the report's own stop state");
}

#[test]
fn the_diversity_caveat_travels_verbatim_into_the_figure() {
    let fixture = diversity_fixture();
    let svg = mutation_diversity(&fixture).expect("the fixture renders");
    let caveat = fixture["caveat"].as_str().expect("caveat is a string");
    assert!(
        visible_text(&svg).contains(caveat),
        "the caveat is part of the measurement and must be reproduced verbatim"
    );
    assert!(svg.contains("instance count is not benchmark count"));
}
