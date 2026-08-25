//! Finding the drawable artifacts inside a document a user already has on disk.
//!
//! The six renderers each take one exact artifact shape. Everything a user actually keeps —
//! a `--json` envelope redirected to a file, a research dossier, a certificate written by
//! `context compile --certificate-out` — is either that shape, a superset of it, or a container
//! holding several. This module is the layer that says which, and where.
//!
//! # Detection is structural
//!
//! A document is classified by the keys it carries and the schema strings it declares, never by
//! its filename. A file called `comparison.json` that holds a certificate must render as a
//! certificate, and a certificate saved as `out-3.json` must still be found; naming is a habit,
//! not evidence. [`classify`] therefore reads required-key sets, and where a shape publishes a
//! schema string it is checked as a second, independent statement: a document that claims a
//! certificate schema but carries no `plan` block contradicts itself and is refused rather than
//! guessed at.
//!
//! # Nothing drawable is an answer, not an error
//!
//! [`detect`] returns `Ok(vec![])` for a document this crate does not recognise. A world, a
//! query, a compile trace and a repair plan are all perfectly good documents that no figure in
//! this crate draws, and reporting that as a failure would tell an operator their file is broken
//! when it is merely not a figure's input. The distinction matters most in
//! `bioprism figure batch`, where "nothing drawable here" is a manifest entry beside the files
//! that did render.
//!
//! # Two kinds at one pointer is a refusal
//!
//! If a value's keys satisfy two different artifact shapes at once, [`classify`] returns
//! [`FigureError::Inconsistent`] naming both. Picking one would mean rendering a figure of
//! something the document does not unambiguously claim to be, and the caller — who wrote the
//! document — is the only party that can resolve it.
//!
//! [`ArtifactKind::CliEnvelope`] is deliberately outside that competition. An envelope is a
//! wrapper whose command-specific keys can *complete* an artifact's key set: `world sweep --json`
//! emits `ok` and `admissible_cells` alongside the sweep table's own `seed` and `cells`, so the
//! envelope and the artifact are the same object. Envelope membership is therefore a marker
//! checked after the artifact shapes have had their say, and it never makes a document ambiguous.
//!
//! # What the source digest in each figure means
//!
//! [`RenderedFigure::source_sha256`] is `bioprism_ids::ContentHash::of_value` over the exact value
//! at the reported pointer — the same digest the figure stamps into its own footer. For a
//! superset envelope like `world sweep --json` that is the digest of the envelope, because the
//! envelope is what was rendered. The hex identifies the artifact; it does not attest that the
//! artifact is correct, and no part of this module checks a claimed digest against a recomputed
//! one.

use crate::error::FigureError;
use bioprism_ids::ContentHash;
use serde_json::{Map, Value};

/// The dossier schema `bioprism-research` stamps, checked as a declaration rather than trusted as
/// one: a document carrying it must still hold the `steps` array the walk reads.
const DOSSIER_SCHEMA: &str = "bioprism-research/dossier/0.1";
/// The report schema `bioprism-autopilot` stamps.
const AUTOPILOT_REPORT_SCHEMA: &str = "bioprism-autopilot/report/0.1";
/// Both certificate profiles share this prefix; the extended profile adds keys rather than
/// replacing them, so one required-key set recognises both.
const CERTIFICATE_SCHEMA_PREFIX: &str = "fiber-context-certificate/";

/// The longest label this module will put into a suggested filename.
const LABEL_MAX_CHARS: usize = 60;

/// What a document region is.
///
/// Three of these draw nothing on their own. [`ArtifactKind::ResearchDossier`] and
/// [`ArtifactKind::CliEnvelope`] are containers, and [`ArtifactKind::MutationFamily`] is a
/// recognised document with no renderer — this crate draws effective diversity, which is the
/// measurement over a family, not the family's membership list. They are named here rather than
/// left as "unrecognised" so a caller can tell "I know what this is and there is no figure for
/// it" from "I have never seen this shape".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactKind {
    /// `bioprism_baseline::Comparison::to_json` — what `context compare --json` prints.
    Comparison,
    /// A context certificate in either the reference or the extended profile.
    ContextCertificate,
    /// The `world sweep` table, whether bare or inside its `--json` envelope.
    SweepTable,
    /// `bioprism_mutation::Diversity`.
    MutationDiversity,
    /// `bioprism_mutation::Family` — recognised, and not drawable by this crate.
    MutationFamily,
    /// An autopilot drive report.
    AutopilotReport,
    /// A research dossier, which carries many artifacts inline.
    ResearchDossier,
    /// A `--json` envelope from any command, recognised by its `ok` flag.
    CliEnvelope,
}

impl ArtifactKind {
    /// Every kind, in declaration order.
    pub const ALL: [ArtifactKind; 8] = [
        ArtifactKind::Comparison,
        ArtifactKind::ContextCertificate,
        ArtifactKind::SweepTable,
        ArtifactKind::MutationDiversity,
        ArtifactKind::MutationFamily,
        ArtifactKind::AutopilotReport,
        ArtifactKind::ResearchDossier,
        ArtifactKind::CliEnvelope,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            ArtifactKind::Comparison => "comparison",
            ArtifactKind::ContextCertificate => "context-certificate",
            ArtifactKind::SweepTable => "sweep-table",
            ArtifactKind::MutationDiversity => "mutation-diversity",
            ArtifactKind::MutationFamily => "mutation-family",
            ArtifactKind::AutopilotReport => "autopilot-report",
            ArtifactKind::ResearchDossier => "research-dossier",
            ArtifactKind::CliEnvelope => "cli-envelope",
        }
    }

    /// The figures drawable from this kind *at its own pointer*.
    ///
    /// Empty for the containers and for [`ArtifactKind::MutationFamily`]. A certificate yields
    /// two, because the plan block and the omission block are separate figures of one document
    /// and neither can be derived from the other.
    pub fn figures(self) -> &'static [FigureKind] {
        match self {
            ArtifactKind::Comparison => &[FigureKind::BaselinePanel],
            ArtifactKind::ContextCertificate => {
                &[FigureKind::SelectionRatio, FigureKind::OmissionAccounting]
            }
            ArtifactKind::SweepTable => &[FigureKind::SweepGrid],
            ArtifactKind::MutationDiversity => &[FigureKind::MutationDiversity],
            ArtifactKind::AutopilotReport => &[FigureKind::AutopilotDrive],
            ArtifactKind::MutationFamily
            | ArtifactKind::ResearchDossier
            | ArtifactKind::CliEnvelope => &[],
        }
    }
}

/// Which renderer draws a detected region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FigureKind {
    BaselinePanel,
    SelectionRatio,
    OmissionAccounting,
    SweepGrid,
    MutationDiversity,
    AutopilotDrive,
}

impl FigureKind {
    /// Every figure, in declaration order.
    pub const ALL: [FigureKind; 6] = [
        FigureKind::BaselinePanel,
        FigureKind::SelectionRatio,
        FigureKind::OmissionAccounting,
        FigureKind::SweepGrid,
        FigureKind::MutationDiversity,
        FigureKind::AutopilotDrive,
    ];

    /// The name a caller types after `--kind`, and the stem of every suggested filename.
    pub fn slug(self) -> &'static str {
        match self {
            FigureKind::BaselinePanel => "baseline-panel",
            FigureKind::SelectionRatio => "selection-ratio",
            FigureKind::OmissionAccounting => "omission-accounting",
            FigureKind::SweepGrid => "sweep-grid",
            FigureKind::MutationDiversity => "mutation-diversity",
            FigureKind::AutopilotDrive => "autopilot-drive",
        }
    }

    /// The inverse of [`FigureKind::slug`], total over the registry and rejecting anything else.
    pub fn from_slug(slug: &str) -> Option<FigureKind> {
        FigureKind::ALL.into_iter().find(|kind| kind.slug() == slug)
    }

    /// One line naming what the figure shows, for a `--help` or a listing.
    pub fn summary(self) -> &'static str {
        match self {
            FigureKind::BaselinePanel => {
                "one bar per strategy over a comparison, refused rows drawn as refused"
            }
            FigureKind::SelectionRatio => "compiled facts and factors against their totals",
            FigureKind::OmissionAccounting => {
                "omitted facts by class, with the v0.1 summary's own caveat"
            }
            FigureKind::SweepGrid => {
                "the structural family sweep, ties drawn as prominently as wins"
            }
            FigureKind::MutationDiversity => "instances against independent equivalence classes",
            FigureKind::AutopilotDrive => "the attempt sequence, in logical order and clock-free",
        }
    }
}

/// One drawable region of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// Which renderer draws it.
    pub kind: FigureKind,
    /// What the region is, which is what made [`Detected::kind`] applicable.
    pub artifact: ArtifactKind,
    /// RFC 6901 JSON pointer from the document root to the value the renderer receives. The empty
    /// string means the document itself.
    pub pointer: String,
    /// A filename derived from the figure and the artifact's own identity. Unique within one
    /// [`detect`] result; see [`detect`] for how a collision is broken.
    pub suggested_filename: String,
}

/// One rendered figure and the digest of exactly what it was rendered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFigure {
    pub filename: String,
    pub svg: String,
    pub kind: FigureKind,
    pub pointer: String,
    /// The canonical digest of the value at [`RenderedFigure::pointer`]. Identical to the hex in
    /// the figure's own footer, and an identity rather than an attestation.
    pub source_sha256: String,
}

fn object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

fn schema<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn is_object_at(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_some_and(Value::is_object)
}

fn is_array_at(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_some_and(Value::is_array)
}

fn is_string_at(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_some_and(Value::is_string)
}

fn is_number_at(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).is_some_and(Value::is_number)
}

fn nested_has(map: &Map<String, Value>, key: &str, required: &[&str]) -> bool {
    map.get(key)
        .and_then(Value::as_object)
        .is_some_and(|inner| required.iter().all(|name| inner.contains_key(*name)))
}

fn matches_comparison(map: &Map<String, Value>) -> bool {
    is_string_at(map, "world_id")
        && is_string_at(map, "query_id")
        && is_number_at(map, "total_facts")
        && is_object_at(map, "reference")
        && is_array_at(map, "results")
}

fn matches_certificate(map: &Map<String, Value>) -> bool {
    is_string_at(map, "world_id")
        && is_string_at(map, "query_id")
        && is_array_at(map, "selected_facts")
        && nested_has(
            map,
            "plan",
            &[
                "backend",
                "compiled_fact_count",
                "total_fact_count",
                "compiled_factor_count",
                "total_factor_count",
            ],
        )
        && nested_has(
            map,
            "omissions",
            &[
                "total_facts",
                "exploratory_facts",
                "classification",
                "inaccessible_selected_before_cut",
            ],
        )
}

fn matches_sweep_table(map: &Map<String, Value>) -> bool {
    is_number_at(map, "seed") && is_array_at(map, "cells")
}

fn matches_diversity(map: &Map<String, Value>) -> bool {
    [
        "instances",
        "parents",
        "families",
        "signatures",
        "equivalence_classes",
        "inflation_ratio",
    ]
    .iter()
    .all(|key| is_number_at(map, key))
        && is_string_at(map, "caveat")
}

fn matches_mutation_family(map: &Map<String, Value>) -> bool {
    is_string_at(map, "parent_id")
        && is_string_at(map, "parent_sha256")
        && is_array_at(map, "accepted")
        && is_array_at(map, "rejected")
        && is_array_at(map, "duplicates")
}

fn matches_autopilot_report(map: &Map<String, Value>) -> bool {
    is_string_at(map, "final_status")
        && is_string_at(map, "base_mission_id")
        && is_array_at(map, "attempts")
        && nested_has(
            map,
            "totals",
            &["attempts_used", "max_attempts", "steps_in_plan"],
        )
}

fn matches_dossier(map: &Map<String, Value>) -> bool {
    is_array_at(map, "steps") && is_object_at(map, "request")
}

/// A schema string that names a shape whose required keys are absent.
///
/// Refused rather than ignored: a document declaring itself a certificate and carrying no plan
/// block is not a certificate with a typo, it is two statements that cannot both be true, and the
/// author is the only party who can say which one to keep.
fn schema_without_shape(declared: &str, shape: &str, present: bool) -> Option<FigureError> {
    (!present).then(|| FigureError::Inconsistent {
        reason: format!(
            "the document declares schema {declared:?} but carries none of the keys a {shape} \
             must have"
        ),
    })
}

/// What one value is, or `None` when this crate has never seen the shape.
///
/// Ambiguity between two artifact shapes is [`FigureError::Inconsistent`]; see the module
/// documentation for why [`ArtifactKind::CliEnvelope`] never participates in that competition.
pub fn classify(value: &Value) -> Result<Option<ArtifactKind>, FigureError> {
    let Some(map) = object(value) else {
        return Ok(None);
    };

    if let Some(declared) = schema(map, "schema") {
        if declared == DOSSIER_SCHEMA {
            if let Some(error) =
                schema_without_shape(declared, "research dossier", matches_dossier(map))
            {
                return Err(error);
            }
            return Ok(Some(ArtifactKind::ResearchDossier));
        }
        if declared == AUTOPILOT_REPORT_SCHEMA {
            if let Some(error) =
                schema_without_shape(declared, "autopilot report", matches_autopilot_report(map))
            {
                return Err(error);
            }
            return Ok(Some(ArtifactKind::AutopilotReport));
        }
    }
    if let Some(declared) = schema(map, "schema_version") {
        if declared.starts_with(CERTIFICATE_SCHEMA_PREFIX) {
            if let Some(error) =
                schema_without_shape(declared, "context certificate", matches_certificate(map))
            {
                return Err(error);
            }
            return Ok(Some(ArtifactKind::ContextCertificate));
        }
    }

    let mut matched: Vec<ArtifactKind> = Vec::new();
    for (kind, hit) in [
        (ArtifactKind::Comparison, matches_comparison(map)),
        (ArtifactKind::ContextCertificate, matches_certificate(map)),
        (ArtifactKind::SweepTable, matches_sweep_table(map)),
        (ArtifactKind::MutationDiversity, matches_diversity(map)),
        (ArtifactKind::MutationFamily, matches_mutation_family(map)),
        (ArtifactKind::AutopilotReport, matches_autopilot_report(map)),
    ] {
        if hit {
            matched.push(kind);
        }
    }

    match matched.as_slice() {
        [] => Ok(map
            .get("ok")
            .is_some_and(Value::is_boolean)
            .then_some(ArtifactKind::CliEnvelope)),
        [only] => Ok(Some(*only)),
        many => Err(FigureError::Inconsistent {
            reason: format!(
                "the document satisfies the required keys of {} artifact shapes at once ({}); \
                 rendering it would pick one the document does not claim",
                many.len(),
                many.iter()
                    .map(|kind| kind.slug())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }),
    }
}

/// Escape one object key or array index into an RFC 6901 pointer token.
fn token(raw: &str) -> String {
    raw.replace('~', "~0").replace('/', "~1")
}

/// Turn a pointer into the field name [`FigureError`] reports, so a caller reading a diagnostic
/// sees the same notation they passed to `--pointer`.
fn pointer_field(pointer: &str) -> String {
    if pointer.is_empty() {
        "input".to_string()
    } else {
        pointer.to_string()
    }
}

fn resolve<'a>(document: &'a Value, pointer: &str) -> Result<&'a Value, FigureError> {
    document
        .pointer(pointer)
        .ok_or_else(|| FigureError::MissingField {
            field: pointer_field(pointer),
        })
}

/// Reduce a label to filename-safe characters.
///
/// World ids and mission ids are already constrained, but a hand-edited document can carry
/// anything, and a suggested filename is written to a filesystem by `bioprism figure render`.
/// Everything outside `[a-z0-9.-]` collapses to a single `-`, which can map two distinct ids onto
/// one label; that is why [`detect`] breaks a filename collision on the pointer rather than
/// trusting labels to be unique.
fn sanitise(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    for character in raw.chars() {
        let mapped = match character {
            'a'..='z' | '0'..='9' | '-' | '.' => character,
            'A'..='Z' => character.to_ascii_lowercase(),
            _ => '-',
        };
        if mapped == '-' && out.ends_with('-') {
            continue;
        }
        out.push(mapped);
        if out.chars().count() >= LABEL_MAX_CHARS {
            break;
        }
    }
    let trimmed = out.trim_matches(['-', '.']).to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The artifact's own identity, preferred over the name a container filed it under.
///
/// A dossier's `outputs[].name` is the dossier's bookkeeping ("comparison-d50"); the artifact's
/// `world_id` is what the artifact is about. The bookkeeping name is the fallback, used when the
/// artifact carries no identity of its own — a `Diversity` document, for instance, names no world.
fn label_for(kind: ArtifactKind, value: &Value, record_name: Option<&str>) -> Option<String> {
    let own = match kind {
        ArtifactKind::Comparison | ArtifactKind::ContextCertificate => value
            .get("world_id")
            .and_then(Value::as_str)
            .and_then(sanitise),
        ArtifactKind::SweepTable => value
            .get("seed")
            .and_then(Value::as_u64)
            .map(|seed| format!("seed-{seed}")),
        ArtifactKind::AutopilotReport => value
            .get("base_mission_id")
            .and_then(Value::as_str)
            .and_then(sanitise),
        _ => None,
    };
    own.or_else(|| record_name.and_then(sanitise))
}

fn filename_for(kind: FigureKind, label: Option<&str>) -> String {
    let slug = kind.slug();
    match label {
        Some(label) if label != slug => format!("{slug}-{label}.svg"),
        _ => format!("{slug}.svg"),
    }
}

fn push_figures(
    kind: ArtifactKind,
    value: &Value,
    pointer: &str,
    record_name: Option<&str>,
    out: &mut Vec<Detected>,
) {
    let label = label_for(kind, value, record_name);
    for figure in kind.figures() {
        out.push(Detected {
            kind: *figure,
            artifact: kind,
            pointer: pointer.to_string(),
            suggested_filename: filename_for(*figure, label.as_deref()),
        });
    }
}

/// Walk a dossier's inlined artifacts.
///
/// An output record whose artifact was not inlined carries a digest and no bytes. There is
/// nothing to draw there, and the dossier already states the omission through its own
/// `inlined: false`, so the walk passes over it rather than inventing an entry.
fn walk_dossier(document: &Value, base: &str, out: &mut Vec<Detected>) -> Result<(), FigureError> {
    let dossier = resolve(document, base)?;
    let steps = dossier
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| FigureError::MissingField {
            field: format!("{base}/steps"),
        })?;
    for (step_index, step) in steps.iter().enumerate() {
        let outputs = step
            .get("outputs")
            .and_then(Value::as_array)
            .ok_or_else(|| FigureError::MissingField {
                field: format!("{base}/steps/{step_index}/outputs"),
            })?;
        for (output_index, output) in outputs.iter().enumerate() {
            let Some(artifact) = output.get("artifact") else {
                continue;
            };
            let Some(kind) = classify(artifact)? else {
                continue;
            };
            let pointer = format!("{base}/steps/{step_index}/outputs/{output_index}/artifact");
            let record_name = output.get("name").and_then(Value::as_str);
            push_figures(kind, artifact, &pointer, record_name, out);
        }
    }
    Ok(())
}

/// Every drawable region of one document, in document order.
///
/// A bare artifact yields its own figures at the empty pointer. A research dossier yields one
/// entry per inlined drawable artifact. A `--json` envelope yields whatever its top-level members
/// hold, in addition to anything the envelope itself is: `world sweep --json` is a sweep table
/// *and* an envelope, and is drawn once, at the root, because the envelope is the value that
/// exists.
///
/// The scan is bounded on purpose. It reads the root, the root's own members when the root is an
/// envelope, and a dossier's `steps[].outputs[].artifact` records — one container deep, plus the
/// dossier walk. It is not a recursive search of arbitrary JSON: an unbounded scan would start
/// finding "artifacts" inside fields that merely resemble them, and a figure of a coincidence is
/// worse than no figure.
///
/// Filenames are unique within the returned vector. Two artifacts whose labels collide — the same
/// world compiled twice into one dossier, or two ids that sanitise to the same characters — are
/// both suffixed with their pointer, so neither silently overwrites the other on disk.
pub fn detect(document: &Value) -> Result<Vec<Detected>, FigureError> {
    let mut out: Vec<Detected> = Vec::new();
    let root_kind = classify(document)?;

    if let Some(kind) = root_kind {
        push_figures(kind, document, "", None, &mut out);
        if kind == ArtifactKind::ResearchDossier {
            walk_dossier(document, "", &mut out)?;
        }
    }

    if let Some(map) = object(document).filter(|map| map.get("ok").is_some_and(Value::is_boolean)) {
        for (key, member) in map {
            let Some(kind) = classify(member)? else {
                continue;
            };
            let pointer = format!("/{}", token(key));
            push_figures(kind, member, &pointer, Some(key), &mut out);
            if kind == ArtifactKind::ResearchDossier {
                walk_dossier(document, &pointer, &mut out)?;
            }
        }
    }

    break_filename_collisions(&mut out);
    Ok(out)
}

/// Suffix every entry of a colliding filename with its pointer.
///
/// Applied to *all* members of a colliding group rather than to the second onward, so that adding
/// an artifact to a document never renames the figure some other artifact already had: either a
/// name is unambiguous and is used bare, or it is ambiguous and every claimant is qualified.
fn break_filename_collisions(detected: &mut [Detected]) {
    let names: Vec<String> = detected
        .iter()
        .map(|item| item.suggested_filename.clone())
        .collect();
    for (index, item) in detected.iter_mut().enumerate() {
        let collides = names
            .iter()
            .enumerate()
            .any(|(other, name)| other != index && name == &item.suggested_filename);
        if !collides {
            continue;
        }
        let qualifier = sanitise(&item.pointer).unwrap_or_else(|| "root".to_string());
        let stem = item
            .suggested_filename
            .strip_suffix(".svg")
            .unwrap_or(&item.suggested_filename)
            .to_string();
        item.suggested_filename = format!("{stem}-{qualifier}.svg");
    }
}

fn render_kind(kind: FigureKind, value: &Value) -> Result<String, FigureError> {
    match kind {
        FigureKind::BaselinePanel => crate::panel::baseline_panel(value),
        FigureKind::SelectionRatio => crate::selection::selection_ratio(value),
        FigureKind::OmissionAccounting => crate::omission::omission_accounting(value),
        FigureKind::SweepGrid => crate::grid::sweep_grid(value),
        FigureKind::MutationDiversity => crate::diversity::mutation_diversity(value),
        FigureKind::AutopilotDrive => crate::drive::autopilot_drive(value),
    }
}

/// Render one region [`detect`] reported, resolving its pointer against the same document.
///
/// A pointer that no longer resolves is [`FigureError::MissingField`] naming the pointer, which
/// is what a caller sees if they pass a `--pointer` by hand or hold a [`Detected`] from a
/// different document.
pub fn render_detected(document: &Value, detected: &Detected) -> Result<String, FigureError> {
    render_kind(detected.kind, resolve(document, &detected.pointer)?)
}

/// Render every drawable region of a document, each with the digest of exactly what it drew.
///
/// One refusal fails the whole call. A partial result would have to be reported as a success with
/// a figure missing, and a caller writing the returned vector to disk would produce a directory
/// that looks complete and is not.
pub fn render_all(document: &Value) -> Result<Vec<RenderedFigure>, FigureError> {
    let detected = detect(document)?;
    let mut out = Vec::with_capacity(detected.len());
    for item in &detected {
        let value = resolve(document, &item.pointer)?;
        let svg = render_kind(item.kind, value)?;
        let source_sha256 = ContentHash::of_value(value)
            .map_err(|error| FigureError::Canonicalisation {
                reason: error.to_string(),
            })?
            .to_string();
        out.push(RenderedFigure {
            filename: item.suggested_filename.clone(),
            svg,
            kind: item.kind,
            pointer: item.pointer.clone(),
            source_sha256,
        });
    }
    Ok(out)
}
