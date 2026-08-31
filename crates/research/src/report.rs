//! Report rendering: a dossier in, one Markdown document and its SVG figures out.
//!
//! [`render_report`] is a pure function of the dossier value — no I/O, no clock, no randomness —
//! so the same dossier always renders the same bytes, figures included. Figures come from
//! `bioprism-figures`, which stamps every one with the canonical digest of the exact artifact it
//! rendered; the caption repeats the artifact name and digest so figure, dossier record, and
//! caption can be checked against each other.
//!
//! The findings section renders negative findings in the same visual register as positive ones:
//! one table, one row shape, a `negative observation` tag where `negative` is true and nothing
//! else — no appendix, no smaller type, no softer wording. The question is reproduced verbatim
//! inside a fence sized past any backtick run it contains, and the section says outright that
//! the runner never interpreted it.

use crate::dossier::DOSSIER_SCHEMA;
use crate::error::ResearchError;
use serde_json::Value;
use std::fmt::Write as _;

/// The rendered report: the Markdown text and `(filename, svg)` pairs it references.
///
/// The Markdown links every figure as `./figures/<filename>`: the committed layout is the report
/// beside a `figures/` directory holding the SVGs, which is what `bioprism research run` writes.
/// A caller placing the files elsewhere owns rewriting the links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedReport {
    pub report_md: String,
    pub figures: Vec<(String, String)>,
}

fn invalid(reason: impl Into<String>) -> ResearchError {
    ResearchError::InvalidDossier {
        reason: reason.into(),
    }
}

fn str_of<'a>(value: &'a Value, path: &str) -> Result<&'a str, ResearchError> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("missing string at {path}")))
}

fn u64_of(value: &Value, path: &str) -> Result<u64, ResearchError> {
    value
        .pointer(path)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("missing integer at {path}")))
}

fn bool_of(value: &Value, path: &str) -> Result<bool, ResearchError> {
    value
        .pointer(path)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("missing boolean at {path}")))
}

struct FoundArtifact<'a> {
    artifact: &'a Value,
    digest: &'a str,
}

fn find_artifact<'a>(
    dossier: &'a Value,
    name: &str,
) -> Result<Option<FoundArtifact<'a>>, ResearchError> {
    let steps = dossier
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing steps array"))?;
    for step in steps {
        let outputs = step
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("step without outputs array"))?;
        for output in outputs {
            if output.get("name").and_then(Value::as_str) != Some(name) {
                continue;
            }
            let digest = output
                .get("sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("artifact {name} record without sha256")))?;
            let Some(artifact) = output.get("artifact") else {
                return Err(ResearchError::ArtifactNotInlined {
                    name: name.to_string(),
                    digest: digest.to_string(),
                });
            };
            return Ok(Some(FoundArtifact { artifact, digest }));
        }
    }
    Ok(None)
}

fn require_artifact<'a>(
    dossier: &'a Value,
    name: &str,
) -> Result<FoundArtifact<'a>, ResearchError> {
    find_artifact(dossier, name)?.ok_or_else(|| ResearchError::ArtifactMissing {
        name: name.to_string(),
    })
}

fn figure_failed(artifact: &str) -> impl Fn(bioprism_figures::FigureError) -> ResearchError + '_ {
    move |error| ResearchError::FigureFailed {
        artifact: artifact.to_string(),
        reason: error.to_string(),
    }
}

/// A fence long enough that the question's own backtick runs cannot close it.
fn fence_for(text: &str) -> String {
    let mut longest = 0usize;
    let mut current = 0usize;
    for character in text.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    "`".repeat(longest.max(2) + 1)
}

fn step_label(step: &Value) -> String {
    let kind = step.get("kind").and_then(Value::as_str).unwrap_or("unknown");
    match step.get("distractors").and_then(Value::as_u64) {
        Some(distractors) => format!("{} (d={distractors})", kind.replace('_', " ")),
        None => kind.replace('_', " "),
    }
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

struct FigurePlan {
    filename: String,
    title: String,
    artifact_name: String,
}

/// Renders the report and figures for one research dossier.
///
/// Optional figures follow the dossier: the sweep and diversity figures appear exactly when
/// their artifacts do. A required artifact that is missing or digest-only is an error — a report
/// silently missing a figure would hide evidence rather than declining to fabricate it.
pub fn render_report(dossier: &Value) -> Result<RenderedReport, ResearchError> {
    if !dossier.is_object() {
        return Err(invalid("dossier must be a JSON object"));
    }
    let schema = dossier.get("schema").and_then(Value::as_str).unwrap_or("");
    if schema != DOSSIER_SCHEMA {
        return Err(invalid(format!(
            "schema is {schema:?}, expected {DOSSIER_SCHEMA:?}"
        )));
    }

    let research_id = str_of(dossier, "/request/research_id")?;
    let question = str_of(dossier, "/request/question")?;
    let family = str_of(dossier, "/request/family")?;
    let seed = u64_of(dossier, "/request/seed")?;
    let dossier_digest = str_of(dossier, "/dossier_sha256")?;
    let request_digest = str_of(dossier, "/request_digest")?;
    let distractor_points: Vec<u64> = dossier
        .pointer("/request/distractor_points")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing distractor_points"))?
        .iter()
        .map(|point| {
            point
                .as_u64()
                .ok_or_else(|| invalid("distractor point is not an integer"))
        })
        .collect::<Result<_, _>>()?;
    let run_mutation = bool_of(dossier, "/request/run_mutation")?;
    let run_minimize = bool_of(dossier, "/request/run_minimize")?;

    let mut plans = vec![
        FigurePlan {
            filename: "selection-ratio-reference.svg".into(),
            title: "Selection ratio (reference fixture)".into(),
            artifact_name: "reference-certificate".into(),
        },
        FigurePlan {
            filename: "omission-accounting-reference.svg".into(),
            title: "Omission accounting (reference fixture)".into(),
            artifact_name: "reference-certificate".into(),
        },
    ];
    for point in &distractor_points {
        plans.push(FigurePlan {
            filename: format!("baseline-panel-d{point}.svg"),
            title: format!("Baseline panel at {point} distractors"),
            artifact_name: format!("comparison-d{point}"),
        });
    }
    let sweep = find_artifact(dossier, "sweep-table")?;
    if sweep.is_some() {
        plans.push(FigurePlan {
            filename: "sweep-grid.svg".into(),
            title: "Structural family sweep".into(),
            artifact_name: "sweep-table".into(),
        });
    }
    let diversity = find_artifact(dossier, "mutation-diversity")?;
    if diversity.is_some() {
        plans.push(FigurePlan {
            filename: "mutation-diversity.svg".into(),
            title: "Mutation effective diversity".into(),
            artifact_name: "mutation-diversity".into(),
        });
    }

    let mut figures: Vec<(String, String)> = Vec::new();
    let mut captions: Vec<(String, String, String, String)> = Vec::new();
    for plan in &plans {
        let found = require_artifact(dossier, &plan.artifact_name)?;
        let svg = match plan.filename.as_str() {
            "selection-ratio-reference.svg" => bioprism_figures::selection_ratio(found.artifact)
                .map_err(figure_failed(&plan.artifact_name))?,
            "omission-accounting-reference.svg" => {
                bioprism_figures::omission_accounting(found.artifact)
                    .map_err(figure_failed(&plan.artifact_name))?
            }
            "sweep-grid.svg" => bioprism_figures::sweep_grid(found.artifact)
                .map_err(figure_failed(&plan.artifact_name))?,
            "mutation-diversity.svg" => bioprism_figures::mutation_diversity(found.artifact)
                .map_err(figure_failed(&plan.artifact_name))?,
            _ => bioprism_figures::baseline_panel(found.artifact)
                .map_err(figure_failed(&plan.artifact_name))?,
        };
        figures.push((plan.filename.clone(), svg));
        captions.push((
            plan.filename.clone(),
            plan.title.clone(),
            plan.artifact_name.clone(),
            found.digest.to_string(),
        ));
    }

    let mut md = String::new();
    let _ = writeln!(md, "# Research dossier {research_id}\n");
    let _ = writeln!(md, "## Question (recorded verbatim)\n");
    let fence = fence_for(question);
    let _ = writeln!(md, "{fence}text\n{question}\n{fence}\n");
    let _ = writeln!(
        md,
        "The runner executed the protocol below; it did not interpret the question. Whether \
         these measurements bear on it is the reader's judgement.\n"
    );
    let _ = writeln!(md, "- family: `{family}`");
    let _ = writeln!(
        md,
        "- distractor points: {}",
        distractor_points
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(md, "- seed: {seed}");
    let _ = writeln!(md, "- request digest: `{request_digest}`");
    let _ = writeln!(md, "- dossier digest: `{dossier_digest}`\n");

    let _ = writeln!(md, "## Protocol\n");
    let _ = writeln!(md, "| # | Step | Outcome | Artifacts |");
    let _ = writeln!(md, "|--:|---|---|---|");
    let steps = dossier
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing steps array"))?;
    for step in steps {
        let index = step
            .get("step_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("step without step_index"))?;
        let label = step
            .get("step")
            .map(step_label)
            .ok_or_else(|| invalid("step without step object"))?;
        let outcome = step
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("step without outcome"))?;
        let artifacts = step
            .get("outputs")
            .and_then(Value::as_array)
            .map(|outputs| {
                outputs
                    .iter()
                    .filter_map(|output| output.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let _ = writeln!(
            md,
            "| {index} | {} | {outcome} | {} |",
            escape_cell(&label),
            escape_cell(&artifacts)
        );
    }

    let _ = writeln!(md, "\n## Findings\n");
    let _ = writeln!(
        md,
        "Every finding is level `observation` — the only level this runner can emit — and was \
         derived by a fixed rule from the cited artifacts. Negative findings are first-class \
         results and share this table's register with positive ones.\n"
    );
    let _ = writeln!(md, "| Rule | Level | Claim | Supported by |");
    let _ = writeln!(md, "|---|---|---|---|");
    let findings = dossier
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing findings array"))?;
    for entry in findings {
        let rule = entry.get("rule").and_then(Value::as_str).unwrap_or("?");
        let claim = entry.get("claim").and_then(Value::as_str).unwrap_or("?");
        let negative = entry
            .get("negative")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let level = entry
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or("?");
        let level = if negative {
            format!("negative {level}")
        } else {
            level.to_string()
        };
        let supported = entry
            .get("supported_by")
            .and_then(Value::as_array)
            .map(|digests| {
                digests
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|digest| format!("`{digest}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let _ = writeln!(
            md,
            "| {} | {level} | {} | {} |",
            escape_cell(rule),
            escape_cell(claim),
            escape_cell(&supported)
        );
    }

    let _ = writeln!(md, "\n## Figures\n");
    for (number, (filename, title, artifact_name, digest)) in captions.iter().enumerate() {
        let _ = writeln!(md, "### Figure {} — {title}\n", number + 1);
        let _ = writeln!(md, "![{title}](./figures/{filename})\n");
        let _ = writeln!(
            md,
            "Source artifact `{artifact_name}`, sha256 `{digest}`. The figure's footer carries \
             the same digest, computed over the exact value rendered.\n"
        );
    }

    let _ = writeln!(md, "## Limitations\n");
    let limitations = dossier
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing limitations array"))?;
    for limitation in limitations {
        let text = limitation
            .as_str()
            .ok_or_else(|| invalid("limitation is not a string"))?;
        let _ = writeln!(md, "- {text}");
    }

    let _ = writeln!(md, "\n## Reproduction\n");
    let _ = writeln!(
        md,
        "The dossier is a deterministic function of the request document: rerunning \
         `bioprism_research::run_research` on the request above reproduces every digest in this \
         report. Worlds regenerate in-library — `bioprism_worldgen::generate` is a pure function \
         of the spec, and the CLI's `world generate` exposes only the reference-like and \
         discriminating presets at each preset's built-in seed:\n"
    );
    let constructor = match family {
        "reference_like" => "reference_like",
        "discriminating" => "discriminating",
        "external_confirmation" => "external_confirmation",
        "policy_restricted" => "policy_restricted",
        other => return Err(invalid(format!("unknown family {other:?}"))),
    };
    let _ = writeln!(md, "```rust");
    for point in &distractor_points {
        let _ = writeln!(
            md,
            "let mut spec = bioprism_worldgen::WorldSpec::{constructor}({point});\n\
             spec.seed = {seed};\n\
             spec.world_id = \"research-{family}-d{point}\".into();\n\
             let generated = bioprism_worldgen::generate(&spec);"
        );
    }
    let _ = writeln!(md, "```\n");
    let _ = writeln!(
        md,
        "With each generated pair written to `world-d<n>.json` / `query-d<n>.json` (the \
         dossier inlines them when they fit the artifact cap, and pins their digests always):\n"
    );
    let _ = writeln!(md, "```text");
    for point in &distractor_points {
        let _ = writeln!(
            md,
            "bioprism context compile --world world-d{point}.json --query query-d{point}.json\n\
             bioprism context compare --world world-d{point}.json --query query-d{point}.json"
        );
    }
    if let Some(sweep) = &sweep {
        let sweep_seed = sweep
            .artifact
            .get("seed")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("sweep-table artifact without seed"))?;
        let _ = writeln!(
            md,
            "bioprism world sweep --seed {sweep_seed}   (the committed default grid; deliberately \
             not reseeded by the request)"
        );
    }
    if run_mutation {
        let base = distractor_points
            .first()
            .ok_or_else(|| invalid("no distractor points"))?;
        let _ = writeln!(md, "bioprism mutate family --world world-d{base}.json");
    }
    if run_minimize {
        let base = distractor_points
            .first()
            .ok_or_else(|| invalid("no distractor points"))?;
        let _ = writeln!(md, "bioprism prism minimize --world world-d{base}.json");
    }
    let _ = writeln!(md, "```");

    Ok(RenderedReport {
        report_md: md,
        figures,
    })
}
