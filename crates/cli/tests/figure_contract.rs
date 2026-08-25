//! The contract of the `figure` command group.
//!
//! The claims asserted here are the ones a user depends on: what is drawable is decided by the
//! document's structure and not its name, `--dry-run` writes nothing at all, an input holding
//! nothing drawable is a verdict rather than a failure, and a batch never drops a skipped input.
//! The figures are drawn from artifacts committed to this repository and from artifacts this
//! binary produces during the test, so nothing here passes because a fixture was hand-written to
//! match.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_bioprism");

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn committed(relative: &str) -> String {
    let mut path = repo_root();
    for part in relative.split('/') {
        path.push(part);
    }
    path.display().to_string()
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("bioprism-figure-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch dir");
    path
}

fn run(arguments: &[&str]) -> Output {
    Command::new(BIN)
        .args(arguments)
        .output()
        .expect("cli binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("process reported an exit code")
}

fn json_of(output: &Output) -> Value {
    serde_json::from_str(&stdout(output)).expect("stdout is exactly one JSON document")
}

fn dossier() -> String {
    committed("docs/research-example/dossier.json")
}

fn certificate() -> String {
    committed("fixtures/fiber-v0.1/golden/reference_certificate.json")
}

fn svg_files(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".svg"))
        .collect();
    names.sort();
    names
}

#[test]
fn figure_list_reports_every_drawable_region_of_the_committed_dossier_and_writes_nothing() {
    let output = run(&["--json", "figure", "list", "--input", &dossier()]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(document["ok"], Value::Bool(true));
    assert_eq!(
        document["drawable"], 13,
        "the dossier's four certificates, three comparisons, sweep table and diversity document \
         are thirteen figures"
    );
    let figures = document["figures"].as_array().expect("figures array");
    for figure in figures {
        assert!(figure["kind"].is_string());
        assert!(figure["pointer"].is_string());
        assert!(figure["suggested_filename"]
            .as_str()
            .expect("filename")
            .ends_with(".svg"));
    }
    assert!(
        !Path::new("figures").exists() || svg_files(Path::new("figures")).is_empty(),
        "`figure list` must not write into the default output directory"
    );
}

#[test]
fn figure_list_reports_an_unrecognised_document_as_nothing_drawable_and_still_exits_zero() {
    let world = committed("fixtures/fiber-v0.1/radiogenomic_world.json");
    let output = run(&["--json", "figure", "list", "--input", &world]);
    assert_eq!(
        code(&output),
        0,
        "listing succeeded; an empty list is the answer, not a failure"
    );
    let document = json_of(&output);
    assert_eq!(document["drawable"], 0);
    assert_eq!(document["figures"].as_array().expect("array").len(), 0);
    let named: Vec<&str> = document["recognised_kinds"]
        .as_array()
        .expect("kinds array")
        .iter()
        .map(|kind| kind.as_str().expect("slug"))
        .collect();
    assert!(
        named.contains(&"baseline-panel") && named.contains(&"sweep-grid"),
        "the answer must name what would have been drawable: {named:?}"
    );
}

#[test]
fn figure_render_writes_one_svg_per_drawable_region_and_stamps_each_with_its_source_digest() {
    let out = scratch("render-dossier");
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &dossier(),
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(document["selected"], 13);
    assert_eq!(document["written"], 13);
    assert_eq!(svg_files(&out).len(), 13);

    for figure in document["figures"].as_array().expect("figures array") {
        let path = PathBuf::from(figure["path"].as_str().expect("path"));
        let digest = figure["source_sha256"].as_str().expect("digest");
        assert_eq!(digest.len(), 64, "a source digest must be a sha256 hex");
        let svg = std::fs::read_to_string(&path).expect("the figure was written");
        assert!(
            svg.contains(&format!("source sha256: {digest}")),
            "{} carries a footer disagreeing with the digest the command reported",
            path.display()
        );
    }
}

#[test]
fn figure_render_dry_run_reports_the_plan_and_writes_nothing() {
    let out = scratch("render-dry");
    std::fs::remove_dir_all(&out).expect("start from an absent directory");
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &certificate(),
        "--out-dir",
        &out.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(document["dry_run"], Value::Bool(true));
    assert_eq!(document["selected"], 2);
    assert_eq!(
        document["written"], 0,
        "a dry run reports the plan and performs no write"
    );
    for figure in document["figures"].as_array().expect("figures array") {
        assert_eq!(figure["written"], Value::Bool(false));
        assert!(
            figure["bytes"].as_u64().expect("byte count") > 0,
            "a dry run still reports how large the figure would be"
        );
    }
    assert!(
        !out.exists(),
        "a dry run created {}, so it had an undeclared effect",
        out.display()
    );
}

#[test]
fn figure_render_exits_one_when_the_document_carries_nothing_drawable() {
    let out = scratch("render-empty");
    std::fs::remove_dir_all(&out).expect("start from an absent directory");
    let world = committed("fixtures/fiber-v0.1/radiogenomic_world.json");
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &world,
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(
        code(&output),
        1,
        "a document holding nothing drawable is a completed run whose verdict is negative, not \
         an error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = json_of(&output);
    assert_eq!(document["ok"], Value::Bool(false));
    assert_eq!(document["drawable"], 0);
    assert!(!out.exists(), "nothing drawable must mean nothing written");
}

#[test]
fn figure_render_exits_one_when_a_filter_selects_none_of_what_the_document_holds() {
    let out = scratch("render-filtered-empty");
    std::fs::remove_dir_all(&out).expect("start from an absent directory");
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &certificate(),
        "--out-dir",
        &out.display().to_string(),
        "--kind",
        "autopilot-drive",
    ]);
    assert_eq!(code(&output), 1);
    let document = json_of(&output);
    assert_eq!(
        document["drawable"], 2,
        "the empty selection must not be reported as an empty document"
    );
    assert_eq!(document["selected"], 0);
    assert!(!out.exists());
}

#[test]
fn a_kind_filter_selects_exactly_the_figures_it_names() {
    let out = scratch("render-kind-filter");
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &dossier(),
        "--out-dir",
        &out.display().to_string(),
        "--kind",
        "baseline-panel",
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(document["drawable"], 13);
    assert_eq!(document["selected"], 3);
    for figure in document["figures"].as_array().expect("figures array") {
        assert_eq!(figure["kind"], "baseline-panel");
    }
    assert_eq!(svg_files(&out).len(), 3);
}

#[test]
fn a_pointer_filter_selects_exactly_the_region_it_names() {
    let out = scratch("render-pointer-filter");
    let listing = json_of(&run(&["--json", "figure", "list", "--input", &dossier()]));
    let pointer = listing["figures"][0]["pointer"]
        .as_str()
        .expect("the listing names a pointer")
        .to_string();
    let output = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &dossier(),
        "--out-dir",
        &out.display().to_string(),
        "--pointer",
        &pointer,
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(
        document["selected"], 2,
        "one certificate pointer carries both certificate figures"
    );
    for figure in document["figures"].as_array().expect("figures array") {
        assert_eq!(figure["pointer"].as_str(), Some(pointer.as_str()));
    }
}

#[test]
fn a_kind_outside_the_registry_is_a_usage_error_rather_than_an_empty_selection() {
    let output = run(&[
        "figure",
        "render",
        "--input",
        &certificate(),
        "--kind",
        "pie-chart",
    ]);
    assert_eq!(
        code(&output),
        2,
        "a mistyped flag must not be reported as a defect in the document"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("baseline-panel") && stderr.contains("sweep-grid"),
        "the refusal must name the registry: {stderr}"
    );
}

#[test]
fn a_pointer_that_is_not_an_rfc_6901_pointer_is_a_usage_error() {
    let output = run(&[
        "figure",
        "render",
        "--input",
        &certificate(),
        "--pointer",
        "report",
    ]);
    assert_eq!(code(&output), 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("RFC 6901"), "{stderr}");
}

#[test]
fn a_document_that_is_not_json_exits_three_and_a_missing_file_exits_five() {
    let scratch_dir = scratch("bad-inputs");
    let broken = scratch_dir.join("broken.json");
    std::fs::write(&broken, "{ this is not json").expect("write the broken input");
    let output = run(&["figure", "render", "--input", &broken.display().to_string()]);
    assert_eq!(code(&output), 3);

    let absent = scratch_dir.join("absent.json");
    let output = run(&["figure", "list", "--input", &absent.display().to_string()]);
    assert_eq!(
        code(&output),
        5,
        "a dependency that could not be read is an I/O failure, not a malformed document"
    );
}

#[test]
fn figure_batch_writes_a_manifest_naming_every_figure_and_every_skipped_input() {
    let inputs = scratch("batch-inputs");
    let out = scratch("batch-out");
    std::fs::copy(certificate(), inputs.join("certificate.json")).expect("copy the certificate");
    std::fs::write(inputs.join("broken.json"), "{ not json").expect("write a broken input");
    std::fs::copy(
        committed("fixtures/fiber-v0.1/radiogenomic_world.json"),
        inputs.join("world.json"),
    )
    .expect("copy a document that draws nothing");
    std::fs::write(inputs.join("notes.md"), "not a json file at all").expect("write a non-input");

    let output = run(&[
        "--json",
        "figure",
        "batch",
        "--input-dir",
        &inputs.display().to_string(),
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(
        document["inputs_total"], 3,
        "the walk considers *.json files and leaves everything else alone"
    );
    assert_eq!(document["figures_total"], 2);
    assert_eq!(document["skipped_total"], 2);
    assert_eq!(document["recursive"], Value::Bool(false));

    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("manifest.json")).expect("the manifest was written"),
    )
    .expect("the manifest is JSON");
    assert_eq!(manifest["inputs"].as_array().expect("inputs").len(), 3);
    assert_eq!(manifest["figures"].as_array().expect("figures").len(), 2);
    let skipped = manifest["skipped"].as_array().expect("skipped");
    assert_eq!(
        skipped.len(),
        2,
        "a skipped input is a first-class manifest entry, never a silent omission"
    );
    let reasons: Vec<&str> = skipped
        .iter()
        .map(|entry| entry["reason"].as_str().expect("reason"))
        .collect();
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("not valid JSON")),
        "the manifest must say why: {reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("no artifact this builder draws")),
        "the manifest must say why: {reasons:?}"
    );
    for figure in manifest["figures"].as_array().expect("figures") {
        for key in ["input", "kind", "pointer", "filename", "source_sha256"] {
            assert!(
                figure.get(key).is_some(),
                "the manifest figure record is missing {key}: {figure}"
            );
        }
        let svg = std::fs::read_to_string(figure["filename"].as_str().expect("filename"))
            .expect("the manifest names a file that exists");
        let digest = figure["source_sha256"].as_str().expect("digest");
        assert!(svg.contains(&format!("source sha256: {digest}")));
    }
}

#[test]
fn figure_batch_keeps_two_inputs_carrying_the_same_artifact_from_overwriting_each_other() {
    let inputs = scratch("batch-collision-inputs");
    let out = scratch("batch-collision-out");
    std::fs::copy(certificate(), inputs.join("first.json")).expect("copy once");
    std::fs::copy(certificate(), inputs.join("second.json")).expect("copy twice");

    let output = run(&[
        "--json",
        "figure",
        "batch",
        "--input-dir",
        &inputs.display().to_string(),
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 0);
    assert_eq!(json_of(&output)["figures_total"], 4);
    assert_eq!(
        svg_files(&out.join("first")).len(),
        2,
        "each input gets its own directory, so identical artifacts cannot overwrite each other"
    );
    assert_eq!(svg_files(&out.join("second")).len(), 2);
}

#[test]
fn figure_batch_dry_run_writes_neither_figures_nor_a_manifest() {
    let inputs = scratch("batch-dry-inputs");
    let out = scratch("batch-dry-out");
    std::fs::copy(certificate(), inputs.join("certificate.json")).expect("copy the certificate");
    std::fs::remove_dir_all(&out).expect("start from an absent directory");

    let output = run(&[
        "--json",
        "figure",
        "batch",
        "--input-dir",
        &inputs.display().to_string(),
        "--out-dir",
        &out.display().to_string(),
        "--dry-run",
    ]);
    assert_eq!(code(&output), 0);
    let document = json_of(&output);
    assert_eq!(document["dry_run"], Value::Bool(true));
    assert_eq!(document["figures_total"], 2);
    assert_eq!(document["manifest_written"], Value::Bool(false));
    assert!(
        !out.exists(),
        "a dry run created {}, so it had an undeclared effect",
        out.display()
    );
    assert!(
        document["manifest_document"]["figures"]
            .as_array()
            .expect("figures")
            .len()
            == 2,
        "the dry run still reports the manifest it would have written"
    );
}

#[test]
fn figure_batch_exits_one_when_nothing_in_the_directory_is_drawable_and_still_writes_the_manifest()
{
    let inputs = scratch("batch-nothing-inputs");
    let out = scratch("batch-nothing-out");
    std::fs::copy(
        committed("fixtures/fiber-v0.1/radiogenomic_world.json"),
        inputs.join("world.json"),
    )
    .expect("copy a document that draws nothing");

    let output = run(&[
        "--json",
        "figure",
        "batch",
        "--input-dir",
        &inputs.display().to_string(),
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(code(&output), 1);
    let document = json_of(&output);
    assert_eq!(document["ok"], Value::Bool(false));
    assert_eq!(document["figures_total"], 0);
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).expect(
            "the manifest is written even when nothing was drawable, because it is the answer",
        ))
        .expect("the manifest is JSON");
    assert_eq!(manifest["skipped"].as_array().expect("skipped").len(), 1);
}

#[test]
fn a_json_envelope_this_binary_prints_is_rendered_without_the_operator_unwrapping_it() {
    let work = scratch("envelope-round-trip");
    let world = committed("fixtures/fiber-v0.1/radiogenomic_world.json");
    let query = committed("fixtures/fiber-v0.1/leakage_query.json");

    let compare = run(&[
        "--json", "context", "compare", "--world", &world, "--query", &query,
    ]);
    assert_eq!(code(&compare), 0);
    let comparison_path = work.join("comparison.json");
    std::fs::write(&comparison_path, stdout(&compare)).expect("save the comparison");

    let out = work.join("figures");
    let rendered = run(&[
        "--json",
        "figure",
        "render",
        "--input",
        &comparison_path.display().to_string(),
        "--out-dir",
        &out.display().to_string(),
    ]);
    assert_eq!(
        code(&rendered),
        0,
        "what `context compare --json` prints must be drawable exactly as printed: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let document = json_of(&rendered);
    assert_eq!(document["selected"], 1);
    assert_eq!(document["figures"][0]["kind"], "baseline-panel");
    assert_eq!(
        document["figures"][0]["pointer"], "",
        "a bare comparison is drawn at the document root"
    );
    assert_eq!(svg_files(&out).len(), 1);
}

#[test]
fn a_certificate_this_binary_writes_is_drawable_under_any_filename() {
    let work = scratch("certificate-round-trip");
    let world = committed("fixtures/fiber-v0.1/radiogenomic_world.json");
    let query = committed("fixtures/fiber-v0.1/leakage_query.json");
    let renamed = work.join("nothing-in-this-name-says-certificate.json");

    let compile = run(&[
        "--json",
        "context",
        "compile",
        "--world",
        &world,
        "--query",
        &query,
        "--certificate-out",
        &renamed.display().to_string(),
    ]);
    assert_eq!(code(&compile), 0);

    let listing = run(&[
        "--json",
        "figure",
        "list",
        "--input",
        &renamed.display().to_string(),
    ]);
    assert_eq!(code(&listing), 0);
    let document = json_of(&listing);
    assert_eq!(
        document["drawable"], 2,
        "recognition is structural: a certificate under any name is still a certificate"
    );
    let kinds: Vec<&str> = document["figures"]
        .as_array()
        .expect("figures")
        .iter()
        .map(|figure| figure["kind"].as_str().expect("kind"))
        .collect();
    assert_eq!(kinds, vec!["selection-ratio", "omission-accounting"]);
}

#[test]
fn the_help_text_publishes_the_figure_group_its_verdict_code_and_its_non_recursive_walk() {
    let text = stdout(&run(&["--help"]));
    for expected in [
        "figure list",
        "figure render",
        "figure batch",
        "--pointer <json-pointer>",
        "non-recursive",
        "it does not attest that the artifact is correct",
        "baseline-panel, selection-ratio, omission-accounting, sweep-grid",
    ] {
        assert!(text.contains(expected), "help must document {expected:?}");
    }
}
