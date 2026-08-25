//! The runner: executes a planned protocol with in-process library calls only.
//!
//! No subprocess, no network, no filesystem: the reference fixtures are compiled into the crate,
//! generated worlds come from `bioprism_worldgen::generate` (a pure function of the spec), and
//! every measurement is a direct call into the workspace crate that owns it — `bioprism_fiber`
//! for compilation (43.26 certificates), `bioprism_baseline` for the 43.38 panel and the 43.39
//! sweep, `bioprism_mutation` for the 03.08/32 metamorphic suite, `bioprism_prism` for the
//! 1-minimal reduction. The runner adds orchestration and receipts, never measurement logic.
//!
//! Step 0 anchors the dossier: the committed `fixtures/fiber-v0.1` pair must compile to the
//! pinned cross-language parity certificate digest
//! ([`PINNED_REFERENCE_CERTIFICATE_SHA256`]) and the certificate must survive its own
//! verification round-trip. A mismatch aborts the run — a dossier standing on a broken parity
//! anchor would be measurement over an unverified engine.
//!
//! A step that cannot complete is a typed [`ResearchError`], never a thinner dossier: partial
//! protocols are unrepresentable in the output, which is why [`crate::dossier::StepOutcome`] has
//! exactly one variant.

use crate::dossier::{artifact_record, build_dossier, step_record, StepOutcome};
use crate::error::ResearchError;
use crate::findings::{
    comparison_findings, minimization_findings, mutation_findings, reference_anchor_finding,
    sweep_findings, Finding,
};
use crate::protocol::{plan_protocol, ProtocolStep};
use crate::request::ResearchRequest;
use bioprism_baseline::{compare, default_panel, run_sweep, ContextStrategy, SweepGrid, SweepTable};
use bioprism_fiber::{compile, CompileOutput, Query};
use bioprism_ids::ContentHash;
use bioprism_mutation::{generate as mutate, measure, standard_suite};
use bioprism_prism::{minimize_world, preserves};
use bioprism_section::{CertificateProfile, CertificateVerification, ContextCertificate};
use bioprism_world::World;
use bioprism_worldgen::{generate, DistractorAttachment, TagStyle};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The certificate digest three implementations agree on for the committed reference fixture:
/// CPython, the eager Rust path, and the indexed store. `crates/fiber/tests/reference_parity.rs`
/// pins the same value.
pub const PINNED_REFERENCE_CERTIFICATE_SHA256: &str =
    "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4";

/// Transcribed verbatim from `bioprism_baseline::sweep::SweepGrid`'s declaration (minus doc-link
/// markup), carried on the sweep artifact because a document of the sweep must carry the sweep's
/// own scope caveat.
pub const UNSWEPT_KNOBS_CAVEAT: &str =
    "The other WorldSpec knobs — skeleton, events, protected set, decision time, policy — are \
     deliberately not swept: they change what the decision is, not the structure around it, and a \
     sweep that varied them would be comparing strategies across different questions.";

const REFERENCE_WORLD_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/fiber-v0.1/radiogenomic_world.json"
));
const REFERENCE_QUERY_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/fiber-v0.1/leakage_query.json"
));

fn canonicalisation(error: impl std::fmt::Display) -> ResearchError {
    ResearchError::Canonicalisation {
        reason: error.to_string(),
    }
}

fn digest_of(value: &Value) -> Result<String, ResearchError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(canonicalisation)
}

struct PointState {
    world_value: Value,
    world: World,
    query: Query,
    world_digest: String,
    query_digest: String,
    world_id: String,
}

/// Compiles, digest-checks against the pinned value, and round-trips the certificate through
/// [`ContextCertificate::verify`]. Shared by the anchor step and every generated point so the
/// two cannot drift apart in what "compiled" means.
fn compile_and_roundtrip(
    world: &World,
    query: &Query,
    world_id: &str,
) -> Result<(CompileOutput, Value), ResearchError> {
    let output = compile(world, query).map_err(|error| ResearchError::CompileFailed {
        world_id: world_id.to_string(),
        reason: error.to_string(),
    })?;
    let certificate = output
        .certificate
        .to_json(CertificateProfile::Reference)
        .map_err(canonicalisation)?;
    match ContextCertificate::verify(&certificate).map_err(canonicalisation)? {
        CertificateVerification::Valid => Ok((output, certificate)),
        CertificateVerification::DigestMismatch { claimed, recomputed } => {
            Err(ResearchError::CertificateRoundTrip {
                world_id: world_id.to_string(),
                reason: format!("digest mismatch: claimed {claimed}, recomputed {recomputed}"),
            })
        }
        CertificateVerification::Malformed(reason) => Err(ResearchError::CertificateRoundTrip {
            world_id: world_id.to_string(),
            reason: format!("malformed: {reason}"),
        }),
    }
}

fn trace_summary(output: &CompileOutput) -> Value {
    json!({
        "passes": output
            .trace
            .passes
            .iter()
            .map(|pass| json!({ "name": pass.name, "retained": pass.retained, "note": pass.note }))
            .collect::<Vec<_>>(),
        "deferred_passes": output
            .trace
            .deferred_passes
            .iter()
            .map(|(name, reason)| json!([name, reason]))
            .collect::<Vec<_>>(),
        "unmatched_protected_tags": output.trace.unmatched_protected_tags,
        "dropped_protected": output.trace.dropped_protected,
        "optional_passes_claimed": {
            "decision_quotient": output.trace.decision_quotient.is_some(),
            "rate_distortion": output.trace.rate_distortion.is_some(),
            "adaptive_acquisition": output.trace.adaptive_acquisition.is_some(),
        },
    })
}

fn attachment_label(attachment: DistractorAttachment) -> &'static str {
    match attachment {
        DistractorAttachment::Hub => "hub",
        DistractorAttachment::NearTarget => "near_target",
    }
}

fn tag_label(style: TagStyle) -> &'static str {
    match style {
        TagStyle::Distinct => "distinct",
        TagStyle::Camouflaged => "camouflaged",
    }
}

/// The sweep table in the same document shape the CLI's `world sweep` emits, plus the sweep's
/// own scope caveat. Refused rows omit `sound`; `judged` tags the state so absence cannot be
/// read as zero.
fn sweep_document(table: &SweepTable) -> Value {
    json!({
        "seed": table.seed,
        "cells_total": table.cells.len(),
        "caveat": UNSWEPT_KNOBS_CAVEAT,
        "cells": table.cells.iter().map(|cell| json!({
            "world_id": cell.world_id,
            "attachment": attachment_label(cell.attachment),
            "relay_depth": cell.relay_depth,
            "tag_style": tag_label(cell.tag_style),
            "distractors": cell.distractors,
            "total_facts": cell.total_facts,
            "rows": cell.rows.iter().map(|row| {
                let mut object = Map::new();
                object.insert("strategy".into(), json!(row.strategy));
                object.insert("facts_selected".into(), json!(row.facts_selected));
                object.insert("judged".into(), json!(row.sound.is_some()));
                if let Some(sound) = row.sound {
                    object.insert("sound".into(), json!(sound));
                }
                object.insert("protected_closure".into(), json!(row.protected_closure));
                object.insert("admissible".into(), json!(row.admissible));
                Value::Object(object)
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn grid_description(grid: &SweepGrid) -> Value {
    json!({
        "attachments": grid.attachments.iter().map(|a| attachment_label(*a)).collect::<Vec<_>>(),
        "relay_depths": grid.relay_depths,
        "tag_styles": grid.tag_styles.iter().map(|t| tag_label(*t)).collect::<Vec<_>>(),
        "distractor_counts": grid.distractor_counts,
        "seed": grid.seed,
    })
}

fn point_state<'a>(
    points: &'a BTreeMap<u32, PointState>,
    step: &ProtocolStep,
    distractors: u32,
) -> Result<&'a PointState, ResearchError> {
    points
        .get(&distractors)
        .ok_or_else(|| ResearchError::ProtocolOutOfOrder {
            step: step.label(),
            distractors,
        })
}

/// Executes the planned protocol for one validated request and returns the digested dossier.
///
/// Deterministic byte-for-byte: worlds are pure functions of their specs, every strategy and
/// suite in the workspace is deterministic, and the dossier is assembled in plan order with
/// canonical hashing throughout — the same request always produces the same
/// `dossier_sha256`.
pub fn run_research(request: &ResearchRequest) -> Result<Value, ResearchError> {
    let protocol = plan_protocol(request);
    let request_digest = request.digest()?;
    let mut steps: Vec<Value> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut points: BTreeMap<u32, PointState> = BTreeMap::new();

    for (step_index, step) in protocol.steps.iter().enumerate() {
        let record = match step {
            ProtocolStep::AnchorReferenceFixture => {
                let world_value: Value = serde_json::from_str(REFERENCE_WORLD_JSON)
                    .map_err(|error| ResearchError::ReferenceFixtureUnusable {
                        reason: format!("world fixture does not parse: {error}"),
                    })?;
                let query_value: Value = serde_json::from_str(REFERENCE_QUERY_JSON)
                    .map_err(|error| ResearchError::ReferenceFixtureUnusable {
                        reason: format!("query fixture does not parse: {error}"),
                    })?;
                let world_digest = digest_of(&world_value)?;
                let query_digest = digest_of(&query_value)?;
                let world = World::from_json(world_value).map_err(|error| {
                    ResearchError::ReferenceFixtureUnusable {
                        reason: format!("world fixture does not load: {error}"),
                    }
                })?;
                let query = Query::from_json(query_value).map_err(|error| {
                    ResearchError::ReferenceFixtureUnusable {
                        reason: format!("query fixture does not load: {error}"),
                    }
                })?;
                let (output, certificate) =
                    compile_and_roundtrip(&world, &query, world.world_id.as_str())?;
                let recomputed = output
                    .certificate
                    .digest(CertificateProfile::Reference)
                    .map_err(canonicalisation)?
                    .to_string();
                if recomputed != PINNED_REFERENCE_CERTIFICATE_SHA256 {
                    return Err(ResearchError::ReferenceAnchorMismatch {
                        pinned: PINNED_REFERENCE_CERTIFICATE_SHA256.to_string(),
                        recomputed,
                    });
                }
                let recorded = artifact_record("reference-certificate", &certificate)?;
                findings.push(reference_anchor_finding(
                    PINNED_REFERENCE_CERTIFICATE_SHA256,
                    &recorded.digest,
                ));
                step_record(
                    step_index,
                    step,
                    json!({ "world": world_digest, "query": query_digest }),
                    vec![recorded.record],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::GenerateWorld { distractors } => {
                let spec = request.family().spec(*distractors, request.seed());
                let spec_value = serde_json::to_value(&spec).map_err(canonicalisation)?;
                let generated = generate(&spec);
                let world_digest = digest_of(&generated.world)?;
                let query_digest = digest_of(&generated.query)?;
                let world_id = spec.world_id.clone();
                let world = World::from_json(generated.world.clone()).map_err(|error| {
                    ResearchError::WorldRejected {
                        world_id: world_id.clone(),
                        reason: error.to_string(),
                    }
                })?;
                let query = Query::from_json(generated.query.clone()).map_err(|error| {
                    ResearchError::QueryRejected {
                        world_id: world_id.clone(),
                        reason: error.to_string(),
                    }
                })?;
                let spec_recorded =
                    artifact_record(&format!("worldspec-d{distractors}"), &spec_value)?;
                let world_recorded =
                    artifact_record(&format!("world-d{distractors}"), &generated.world)?;
                let query_recorded =
                    artifact_record(&format!("query-d{distractors}"), &generated.query)?;
                points.insert(
                    *distractors,
                    PointState {
                        world_value: generated.world,
                        world,
                        query,
                        world_digest: world_digest.clone(),
                        query_digest: query_digest.clone(),
                        world_id,
                    },
                );
                step_record(
                    step_index,
                    step,
                    json!({ "request": request_digest }),
                    vec![
                        spec_recorded.record,
                        world_recorded.record,
                        query_recorded.record,
                    ],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::CompileFiber { distractors } => {
                let state = point_state(&points, step, *distractors)?;
                let (output, certificate) =
                    compile_and_roundtrip(&state.world, &state.query, &state.world_id)?;
                let certificate_recorded =
                    artifact_record(&format!("certificate-d{distractors}"), &certificate)?;
                let trace_recorded = artifact_record(
                    &format!("compile-trace-d{distractors}"),
                    &trace_summary(&output),
                )?;
                step_record(
                    step_index,
                    step,
                    json!({ "world": state.world_digest, "query": state.query_digest }),
                    vec![certificate_recorded.record, trace_recorded.record],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::ComparePanel { distractors } => {
                let state = point_state(&points, step, *distractors)?;
                let panel = default_panel();
                let borrowed: Vec<&dyn ContextStrategy> =
                    panel.iter().map(|boxed| boxed.as_ref()).collect();
                let comparison =
                    compare(&state.world, &state.query, &borrowed).map_err(|error| {
                        ResearchError::NoReferenceVerdict {
                            world_id: state.world_id.clone(),
                            reason: error.to_string(),
                        }
                    })?;
                let recorded =
                    artifact_record(&format!("comparison-d{distractors}"), &comparison.to_json())?;
                findings.extend(comparison_findings(&comparison, &recorded.digest));
                step_record(
                    step_index,
                    step,
                    json!({ "world": state.world_digest, "query": state.query_digest }),
                    vec![recorded.record],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::SweepStructuralGrid => {
                let grid = SweepGrid::default_grid();
                let grid_value = grid_description(&grid);
                let grid_digest = digest_of(&grid_value)?;
                let table = run_sweep(&grid).map_err(|error| ResearchError::SweepFailed {
                    reason: error.to_string(),
                })?;
                let recorded = artifact_record("sweep-table", &sweep_document(&table))?;
                findings.extend(sweep_findings(&table, &recorded.digest));
                step_record(
                    step_index,
                    step,
                    json!({ "grid": grid_digest }),
                    vec![recorded.record],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::MutateBaseWorld { distractors } => {
                let state = point_state(&points, step, *distractors)?;
                let family = mutate(&state.world_value, &standard_suite()).map_err(|error| {
                    ResearchError::MutationFailed {
                        world_id: state.world_id.clone(),
                        reason: error.to_string(),
                    }
                })?;
                let diversity = measure(std::slice::from_ref(&family));
                let family_value = json!({
                    "parent_id": family.parent_id,
                    "parent_sha256": family.parent_sha256,
                    "accepted": family.accepted,
                    "rejected": family.rejected,
                    "duplicates": family.duplicates,
                    "yield_rate": family.yield_rate(),
                    "headline": diversity.headline(),
                });
                let diversity_value = serde_json::to_value(&diversity).map_err(canonicalisation)?;
                let family_recorded = artifact_record("mutation-family", &family_value)?;
                let diversity_recorded = artifact_record("mutation-diversity", &diversity_value)?;
                findings.extend(mutation_findings(
                    &family,
                    &diversity,
                    &family_recorded.digest,
                    &diversity_recorded.digest,
                ));
                step_record(
                    step_index,
                    step,
                    json!({ "world": state.world_digest }),
                    vec![family_recorded.record, diversity_recorded.record],
                    StepOutcome::Completed,
                )?
            }

            ProtocolStep::MinimizeBaseWorld { distractors } => {
                let state = point_state(&points, step, *distractors)?;
                let minimization = minimize_world(&state.world).map_err(|error| {
                    ResearchError::MinimizeFailed {
                        world_id: state.world_id.clone(),
                        reason: error.to_string(),
                    }
                })?;
                let preservation = preserves(&state.world, &minimization);
                let artifact = json!({
                    "minimization": serde_json::to_value(&minimization)
                        .map_err(canonicalisation)?,
                    "preservation": serde_json::to_value(&preservation)
                        .map_err(canonicalisation)?,
                });
                let recorded = artifact_record("minimization", &artifact)?;
                findings.extend(minimization_findings(
                    &minimization,
                    &preservation,
                    &recorded.digest,
                ));
                step_record(
                    step_index,
                    step,
                    json!({ "world": state.world_digest }),
                    vec![recorded.record],
                    StepOutcome::Completed,
                )?
            }
        };
        steps.push(record);
    }

    build_dossier(request, &protocol, steps, &findings)
}
