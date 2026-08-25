//! Claim-per-test coverage of the detection and dispatch layer.
//!
//! The two committed artifacts this suite runs against — the research dossier under
//! `docs/research-example` and the golden reference certificate under `fixtures/fiber-v0.1` — are
//! embedded at compile time, so the suite performs no runtime I/O and cannot pass because a path
//! happened to resolve.

use bioprism_figures::{
    classify, detect, render_all, render_detected, ArtifactKind, Detected, FigureError, FigureKind,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};

const DOSSIER: &str = include_str!("../../../docs/research-example/dossier.json");
const GOLDEN_CERTIFICATE: &str =
    include_str!("../../../fixtures/fiber-v0.1/golden/reference_certificate.json");

fn dossier() -> Value {
    serde_json::from_str(DOSSIER).expect("the committed dossier parses")
}

fn golden_certificate() -> Value {
    serde_json::from_str(GOLDEN_CERTIFICATE).expect("the golden certificate parses")
}

/// One inlined artifact out of the committed dossier, by the name the dossier filed it under.
fn dossier_artifact(name: &str) -> Value {
    let document = dossier();
    for step in document["steps"].as_array().expect("steps array") {
        for output in step["outputs"].as_array().expect("outputs array") {
            if output["name"].as_str() == Some(name) {
                return output["artifact"].clone();
            }
        }
    }
    panic!("the committed dossier carries no artifact named {name:?}");
}

fn kinds(detected: &[Detected]) -> Vec<FigureKind> {
    detected.iter().map(|item| item.kind).collect()
}

fn count_of(detected: &[Detected], kind: FigureKind) -> usize {
    detected.iter().filter(|item| item.kind == kind).count()
}

#[test]
fn the_committed_dossier_yields_one_entry_per_inlined_drawable_artifact() {
    let document = dossier();
    let detected = detect(&document).expect("the committed dossier is detectable");

    assert_eq!(
        detected.len(),
        13,
        "the dossier's four certificates (two figures each), three comparisons, sweep table and \
         diversity document are thirteen figures: {:?}",
        kinds(&detected)
    );
    assert_eq!(count_of(&detected, FigureKind::SelectionRatio), 4);
    assert_eq!(count_of(&detected, FigureKind::OmissionAccounting), 4);
    assert_eq!(count_of(&detected, FigureKind::BaselinePanel), 3);
    assert_eq!(count_of(&detected, FigureKind::SweepGrid), 1);
    assert_eq!(count_of(&detected, FigureKind::MutationDiversity), 1);
    assert_eq!(count_of(&detected, FigureKind::AutopilotDrive), 0);

    for item in &detected {
        assert!(
            document.pointer(&item.pointer).is_some(),
            "detection reported {:?}, which does not resolve in the document it came from",
            item.pointer
        );
    }
}

#[test]
fn a_dossier_carries_more_drawable_artifacts_than_its_own_report_renders() {
    let document = dossier();
    let detected = detect(&document).expect("the committed dossier is detectable");
    let per_point_certificates: Vec<&Detected> = detected
        .iter()
        .filter(|item| {
            item.kind == FigureKind::SelectionRatio
                && item.suggested_filename.contains("research-discriminating")
        })
        .collect();
    assert_eq!(
        per_point_certificates.len(),
        3,
        "the research report draws the reference certificate only; the builder must also reach \
         the certificate compiled at each distractor point: {:?}",
        detected
            .iter()
            .map(|item| item.suggested_filename.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_bare_comparison_is_detected_once_at_the_document_root() {
    let comparison = dossier_artifact("comparison-d50");
    let detected = detect(&comparison).expect("a comparison is detectable");
    assert_eq!(
        detected,
        vec![Detected {
            kind: FigureKind::BaselinePanel,
            artifact: ArtifactKind::Comparison,
            pointer: String::new(),
            suggested_filename: "baseline-panel-research-discriminating-d50.svg".to_string(),
        }]
    );
}

#[test]
fn the_golden_reference_certificate_yields_both_certificate_figures_at_the_root() {
    let certificate = golden_certificate();
    let detected = detect(&certificate).expect("the golden certificate is detectable");
    assert_eq!(
        detected
            .iter()
            .map(|item| (
                item.kind,
                item.pointer.as_str(),
                item.suggested_filename.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                FigureKind::SelectionRatio,
                "",
                "selection-ratio-radiogenomic-integrity-demo-v1.svg"
            ),
            (
                FigureKind::OmissionAccounting,
                "",
                "omission-accounting-radiogenomic-integrity-demo-v1.svg"
            ),
        ],
        "the plan block and the omission block are separate figures of one certificate and \
         neither can be derived from the other"
    );
}

#[test]
fn a_certificate_stripped_of_its_schema_string_is_still_detected_from_its_structure() {
    let mut certificate = golden_certificate();
    certificate
        .as_object_mut()
        .expect("certificate object")
        .remove("schema_version");
    assert_eq!(
        classify(&certificate).expect("classification succeeds"),
        Some(ArtifactKind::ContextCertificate),
        "detection reads required keys, not declarations; a certificate whose schema string was \
         dropped by a hand edit is still a certificate"
    );
}

#[test]
fn the_extended_certificate_profile_is_detected_by_the_same_required_keys() {
    let mut certificate = golden_certificate();
    let object = certificate.as_object_mut().expect("certificate object");
    object.insert(
        "schema_version".into(),
        json!("fiber-context-certificate/0.2-extended"),
    );
    object.insert("omission_manifest".into(), json!({ "groups": [] }));
    object.insert("supports_sufficiency_claim".into(), json!(false));
    assert_eq!(
        classify(&certificate).expect("classification succeeds"),
        Some(ArtifactKind::ContextCertificate),
        "the extended profile adds keys rather than replacing them, so one required-key set \
         recognises both profiles"
    );
}

#[test]
fn a_document_this_crate_has_never_seen_is_reported_as_nothing_drawable_rather_than_refused() {
    for name in [
        "world-d50",
        "query-d50",
        "compile-trace-d50",
        "worldspec-d50",
    ] {
        let artifact = dossier_artifact(name);
        assert_eq!(
            detect(&artifact).expect("an unrecognised document is not an error"),
            Vec::new(),
            "{name} is a perfectly good document that no figure here draws; reporting that as a \
             failure would tell an operator their file is broken"
        );
    }
    for value in [json!([1, 2, 3]), json!("a string"), json!(null), json!({})] {
        assert_eq!(
            detect(&value).expect("a non-artifact value is not an error"),
            Vec::new()
        );
    }
}

#[test]
fn a_mutation_family_document_is_recognised_as_a_family_and_draws_nothing() {
    let family = dossier_artifact("mutation-family");
    assert_eq!(
        classify(&family).expect("classification succeeds"),
        Some(ArtifactKind::MutationFamily),
        "a recognised shape with no renderer must be distinguishable from a shape nobody knows"
    );
    assert!(ArtifactKind::MutationFamily.figures().is_empty());
    assert_eq!(detect(&family).expect("detection succeeds"), Vec::new());
}

#[test]
fn a_value_matching_two_artifact_shapes_at_once_is_refused_rather_than_guessed() {
    let ambiguous = json!({
        "world_id": "w", "query_id": "q", "total_facts": 10,
        "reference": { "status": "valid" }, "results": [],
        "seed": 1, "cells": []
    });
    let error = classify(&ambiguous).expect_err("an ambiguous document must be refused");
    match error {
        FigureError::Inconsistent { reason } => {
            assert!(
                reason.contains("comparison"),
                "reason must name both: {reason}"
            );
            assert!(
                reason.contains("sweep-table"),
                "reason must name both: {reason}"
            );
        }
        other => panic!("expected Inconsistent, got {other:?}"),
    }
    assert!(matches!(
        detect(&ambiguous),
        Err(FigureError::Inconsistent { .. })
    ));
}

#[test]
fn a_document_declaring_a_schema_whose_required_keys_are_absent_is_refused() {
    let liar = json!({ "schema_version": "fiber-context-certificate/0.1", "world_id": "w" });
    assert!(
        matches!(classify(&liar), Err(FigureError::Inconsistent { .. })),
        "a document declaring a certificate schema and carrying no plan block makes two \
         statements that cannot both be true"
    );
    let dossier_liar = json!({ "schema": "bioprism-research/dossier/0.1" });
    assert!(matches!(
        classify(&dossier_liar),
        Err(FigureError::Inconsistent { .. })
    ));
    let autopilot_liar = json!({ "schema": "bioprism-autopilot/report/0.1" });
    assert!(matches!(
        classify(&autopilot_liar),
        Err(FigureError::Inconsistent { .. })
    ));
}

#[test]
fn a_world_sweep_envelope_is_drawn_at_its_root_because_the_envelope_is_the_sweep_table() {
    let table = dossier_artifact("sweep-table");
    let mut envelope = table.clone();
    let object = envelope.as_object_mut().expect("sweep table object");
    object.insert("ok".into(), json!(true));
    object.insert("admissible_cells".into(), json!({ "fiber": 12 }));

    let detected = detect(&envelope).expect("the sweep envelope is detectable");
    assert_eq!(kinds(&detected), vec![FigureKind::SweepGrid]);
    assert_eq!(
        detected[0].pointer, "",
        "the envelope is the value that exists"
    );
    assert_eq!(detected[0].artifact, ArtifactKind::SweepTable);

    let rendered = render_all(&envelope).expect("the sweep envelope renders");
    assert_eq!(
        rendered[0].source_sha256,
        ContentHash::of_value(&envelope)
            .expect("canonicalisable")
            .to_string(),
        "the digest names the envelope, because the envelope is what was rendered"
    );
    assert_ne!(
        rendered[0].source_sha256,
        ContentHash::of_value(&table)
            .expect("canonicalisable")
            .to_string()
    );
}

#[test]
fn a_mutate_family_envelope_is_unwrapped_to_the_diversity_block_it_carries() {
    let family = dossier_artifact("mutation-family");
    let diversity = dossier_artifact("mutation-diversity");
    let mut envelope = family;
    let object = envelope.as_object_mut().expect("family object");
    object.insert("ok".into(), json!(true));
    object.insert("diversity".into(), diversity.clone());
    object.insert(
        "headline".into(),
        json!("instance count is not benchmark count"),
    );

    let detected = detect(&envelope).expect("the mutate-family envelope is detectable");
    assert_eq!(
        detected
            .iter()
            .map(|item| (item.kind, item.pointer.as_str()))
            .collect::<Vec<_>>(),
        vec![(FigureKind::MutationDiversity, "/diversity")],
        "the envelope root is a family document, which draws nothing; the drawable region is \
         the diversity block inside it"
    );
    let rendered = render_all(&envelope).expect("the diversity block renders");
    assert_eq!(
        rendered[0].source_sha256,
        ContentHash::of_value(&diversity)
            .expect("canonicalisable")
            .to_string()
    );
}

#[test]
fn an_autopilot_run_envelope_is_unwrapped_to_the_report_it_carries() {
    let report = json!({
        "schema": "bioprism-autopilot/report/0.1",
        "base_mission_id": "mission-alpha",
        "final_status": "succeeded",
        "attempts": [{
            "attempt_index": 0,
            "kind": "full",
            "outcome_summary": { "mission_status": "completed" },
            "dispatch_error": null
        }],
        "totals": { "attempts_used": 1, "max_attempts": 3, "steps_in_plan": 2 },
        "limitations": [],
        "report_sha256": "0".repeat(64)
    });
    let envelope = json!({
        "ok": true,
        "workflow": "autopilot_run",
        "dry_run": false,
        "final_status": "succeeded",
        "report": report.clone()
    });

    let detected = detect(&envelope).expect("the autopilot envelope is detectable");
    assert_eq!(
        detected
            .iter()
            .map(|item| (
                item.kind,
                item.pointer.as_str(),
                item.suggested_filename.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![(
            FigureKind::AutopilotDrive,
            "/report",
            "autopilot-drive-mission-alpha.svg"
        )]
    );
    let svg = render_detected(&envelope, &detected[0]).expect("the report renders");
    let digest = ContentHash::of_value(&report)
        .expect("canonicalisable")
        .to_string();
    assert!(
        svg.contains(&format!("source sha256: {digest}")),
        "the figure's own footer must name the value at the reported pointer"
    );
}

#[test]
fn an_envelope_carrying_no_inline_artifact_is_reported_as_nothing_drawable() {
    let research_run = json!({
        "ok": true,
        "workflow": "research_run",
        "dry_run": false,
        "research_id": "demo",
        "findings": [{ "claim": "a tie", "negative": true }],
        "figures": 7,
        "artifacts": [{ "path": "out/dossier.json", "bytes": 12, "written": true }]
    });
    assert_eq!(
        detect(&research_run).expect("an envelope with no payload is not an error"),
        Vec::new(),
        "`research run --json` reports where it wrote the dossier; it does not carry one"
    );

    let compile = json!({
        "ok": true,
        "world_id": "w",
        "query_id": "q",
        "oracle": { "kind": "split-integrity", "status": "invalid", "witnesses": [] },
        "certificate_sha256": "0".repeat(64),
        "artifacts": []
    });
    assert_eq!(
        detect(&compile).expect("an envelope with no payload is not an error"),
        Vec::new(),
        "`context compile --json` summarises the certificate and writes it to --certificate-out"
    );
}

#[test]
fn an_envelope_wrapping_a_dossier_is_walked_through_to_the_artifacts_inside_it() {
    let envelope = json!({ "ok": true, "dossier": dossier() });
    let detected = detect(&envelope).expect("a wrapped dossier is detectable");
    assert_eq!(detected.len(), 13);
    for item in &detected {
        assert!(
            item.pointer.starts_with("/dossier/steps/"),
            "a wrapped dossier's pointers must be rooted at the wrapper: {:?}",
            item.pointer
        );
        assert!(envelope.pointer(&item.pointer).is_some());
    }
}

#[test]
fn a_dossier_output_recorded_without_its_bytes_is_not_reported_as_drawable() {
    let mut document = dossier();
    let mut removed = 0usize;
    for step in document["steps"].as_array_mut().expect("steps array") {
        for output in step["outputs"].as_array_mut().expect("outputs array") {
            if output["name"].as_str() == Some("comparison-d250") {
                output
                    .as_object_mut()
                    .expect("output object")
                    .remove("artifact");
                output["inlined"] = json!(false);
                removed += 1;
            }
        }
    }
    assert_eq!(
        removed, 1,
        "the fixture must contain exactly one such record"
    );
    let detected = detect(&document).expect("a dossier with a digest-only record is detectable");
    assert_eq!(
        count_of(&detected, FigureKind::BaselinePanel),
        2,
        "a record carrying a digest and no bytes has nothing to draw; the dossier already states \
         the omission through its own `inlined` flag"
    );
    assert_eq!(detected.len(), 12);
}

#[test]
fn every_rendered_figure_carries_the_canonical_digest_of_the_value_it_was_drawn_from() {
    let document = dossier();
    let rendered = render_all(&document).expect("the committed dossier renders");
    assert_eq!(rendered.len(), 13);
    for figure in &rendered {
        let source = document
            .pointer(&figure.pointer)
            .expect("the reported pointer resolves");
        let expected = ContentHash::of_value(source)
            .expect("canonicalisable")
            .to_string();
        assert_eq!(
            figure.source_sha256, expected,
            "{} claims a digest that is not the canonical digest of the value at {}",
            figure.filename, figure.pointer
        );
        assert!(
            figure.svg.contains(&format!("source sha256: {expected}")),
            "{}'s footer disagrees with its reported source digest",
            figure.filename
        );
        assert!(
            figure.svg.starts_with("<svg "),
            "{} is not an SVG",
            figure.filename
        );
    }
}

/// The figures `bioprism research run` committed beside its dossier, paired with the filename
/// the detection layer suggests for the same artifact.
///
/// The two name their outputs differently on purpose — the report names a figure by its role in
/// the report, the builder names it by the artifact's own identity — so the pairing is written
/// out rather than derived.
const COMMITTED_FIGURES: [(&str, &str); 7] = [
    (
        "selection-ratio-radiogenomic-integrity-demo-v1.svg",
        include_str!("../../../docs/research-example/figures/selection-ratio-reference.svg"),
    ),
    (
        "omission-accounting-radiogenomic-integrity-demo-v1.svg",
        include_str!("../../../docs/research-example/figures/omission-accounting-reference.svg"),
    ),
    (
        "baseline-panel-research-discriminating-d50.svg",
        include_str!("../../../docs/research-example/figures/baseline-panel-d50.svg"),
    ),
    (
        "baseline-panel-research-discriminating-d250.svg",
        include_str!("../../../docs/research-example/figures/baseline-panel-d250.svg"),
    ),
    (
        "baseline-panel-research-discriminating-d750.svg",
        include_str!("../../../docs/research-example/figures/baseline-panel-d750.svg"),
    ),
    (
        "sweep-grid-seed-20260823.svg",
        include_str!("../../../docs/research-example/figures/sweep-grid.svg"),
    ),
    (
        "mutation-diversity.svg",
        include_str!("../../../docs/research-example/figures/mutation-diversity.svg"),
    ),
];

#[test]
fn the_builder_reproduces_the_committed_report_figures_byte_for_byte() {
    let document = dossier();
    let rendered = render_all(&document).expect("the committed dossier renders");
    for (filename, committed) in COMMITTED_FIGURES {
        let produced = rendered
            .iter()
            .find(|figure| figure.filename == filename)
            .unwrap_or_else(|| panic!("the builder produced no figure named {filename}"));
        assert_eq!(
            produced.svg, committed,
            "{filename} differs from the figure `bioprism research run` committed for the same \
             artifact; the builder and the report renderer must not drift into two renderings of \
             one document"
        );
    }
}

#[test]
fn render_all_never_suggests_one_filename_twice() {
    let document = dossier();
    let rendered = render_all(&document).expect("the committed dossier renders");
    let mut names: Vec<&str> = rendered.iter().map(|f| f.filename.as_str()).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        before,
        "two figures sharing a filename would silently overwrite each other on disk"
    );
}

#[test]
fn a_filename_collision_is_broken_on_the_pointer_rather_than_overwriting() {
    let comparison = dossier_artifact("comparison-d50");
    let envelope = json!({ "ok": true, "left": comparison.clone(), "right": comparison });
    let detected = detect(&envelope).expect("two comparisons are detectable");
    let names: Vec<&str> = detected
        .iter()
        .map(|item| item.suggested_filename.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "baseline-panel-research-discriminating-d50-left.svg",
            "baseline-panel-research-discriminating-d50-right.svg"
        ],
        "every claimant of an ambiguous name is qualified, so adding an artifact never renames \
         the figure another artifact already had"
    );
}

#[test]
fn render_detected_refuses_a_pointer_that_does_not_resolve() {
    let comparison = dossier_artifact("comparison-d50");
    let stray = Detected {
        kind: FigureKind::BaselinePanel,
        artifact: ArtifactKind::Comparison,
        pointer: "/nowhere/2".to_string(),
        suggested_filename: "baseline-panel.svg".to_string(),
    };
    assert_eq!(
        render_detected(&comparison, &stray),
        Err(FigureError::MissingField {
            field: "/nowhere/2".to_string()
        }),
        "a pointer passed by hand that names nothing must be reported as the pointer it was, in \
         the notation the caller typed"
    );
}

#[test]
fn a_refused_artifact_fails_the_whole_render_rather_than_yielding_a_partial_directory() {
    let mut document = dossier();
    for step in document["steps"].as_array_mut().expect("steps array") {
        for output in step["outputs"].as_array_mut().expect("outputs array") {
            if output["name"].as_str() == Some("mutation-diversity") {
                output["artifact"]["inflation_ratio"] = json!(99.0);
            }
        }
    }
    assert!(
        matches!(render_all(&document), Err(FigureError::Inconsistent { .. })),
        "a partial result would be reported as a success with a figure missing, and the \
         directory written from it would look complete"
    );
}

#[test]
fn every_figure_kind_round_trips_through_its_slug_and_the_registry_is_total() {
    for kind in FigureKind::ALL {
        assert_eq!(
            FigureKind::from_slug(kind.slug()),
            Some(kind),
            "{} does not round-trip",
            kind.slug()
        );
        assert!(!kind.summary().is_empty());
    }
    assert_eq!(FigureKind::from_slug("no-such-figure"), None);
    assert_eq!(FigureKind::from_slug(""), None);

    let mut slugs: Vec<&str> = ArtifactKind::ALL.iter().map(|kind| kind.slug()).collect();
    let before = slugs.len();
    slugs.sort_unstable();
    slugs.dedup();
    assert_eq!(slugs.len(), before, "two artifact kinds share one slug");

    let drawable: Vec<FigureKind> = ArtifactKind::ALL
        .iter()
        .flat_map(|kind| kind.figures().iter().copied())
        .collect();
    for figure in FigureKind::ALL {
        assert!(
            drawable.contains(&figure),
            "{} is a renderer no artifact kind reaches, so detection can never produce it",
            figure.slug()
        );
    }
}
