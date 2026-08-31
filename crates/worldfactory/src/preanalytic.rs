//! Assay-fault and pre-analytic mutations (27.10).
//!
//! # Why this is not `crates/stress`
//!
//! `crates/stress` implements §32's measurement-degradation families: it multiplies an assay's
//! standard deviation, offsets a batch, jitters a segmentation, shifts a prevalence. Every one of
//! those acts on *measured values* or on *the cohort*. Nothing in it touches the specimen.
//!
//! A pre-analytic fault acts **before measurement exists**. The tissue sat on a bench; the fixative
//! was the wrong one or acted for the wrong duration; an aliquot went through three freeze-thaw
//! cycles; the reagent lot changed between the morning run and the afternoon one. By the time an
//! instrument reads that specimen, the thing being measured is already different from the thing the
//! benchmark is about — and the instrument is working perfectly.
//!
//! The split is real, and it produces a different postcondition. `crates/stress` asks "does this
//! conclusion depend on a fact about the world that could have been otherwise?", and its stresses
//! are *allowed* to change what the right answer is. A pre-analytic fault is not: it degrades what
//! can be measured and leaves the biology alone. That is
//! [`crate::error::PreanalyticRefusal::BiologicalStateChanged`], it is 27.10's own failure
//! "biological state accidentally changes", and it is the line between this family and 27.09's
//! controlled *semantic* mutations, which change the biology on purpose and require the conclusion
//! to move with it.
//!
//! # The stages are the blueprint's
//!
//! 27.10's workflow step 2 enumerates them: "change collection, preservation, protocol, instrument,
//! batch, QC, or processing". [`Stage`] is that list, in that order, and the pipeline order is what
//! [`apply`]'s cross-stage consistency check reads.
//!
//! # The shape is `crates/mutation`'s
//!
//! A [`PreanalyticMutation`] declares its edits and does not check them; [`apply`] runs the
//! postconditions and can reject the mutation as a defect. That separation is deliberate and is
//! `bioprism_mutation`'s: "applying a mutation never checks that relation … so the transformation
//! cannot mark its own homework". The postconditions here are:
//!
//! 1. the biological state is byte-identical afterwards;
//! 2. a fault at non-zero intensity leaves a QC signature, because a fault with no observable
//!    signature asks an agent to detect something the world does not contain;
//! 3. no QC field name and no handling value names the fault — 27.10's failure "QC label leaks
//!    answer";
//! 4. downstream stage records that declare what they received are not stale.
//!
//! and [`validate_family`] adds the fifth, which is 27.10's "false-positive control": the
//! zero-intensity member of a family must be a genuine no-op. A family without it produces
//! detections nobody can interpret.
//!
//! # What is deliberately not here
//!
//! No assay, no instrument, no noise model, and **no clinical fault catalogue**. Which QC signal a
//! given handling fault actually moves, and by how much, is an empirical question about a
//! particular laboratory and a particular platform; the blueprint states none of it and this crate
//! invents none of it. The caller declares the signature and this module checks the declaration
//! against the postconditions. [`FaultKind`]'s variants are named after ordinary laboratory
//! handling variables and are illustrative: the stage each is assigned to follows 27.10's own stage
//! list, and no variant asserts a magnitude, a threshold or an effect on any biomarker.

use crate::error::PreanalyticRefusal;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The pipeline stages of 27.10's workflow step 2, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Collection,
    Preservation,
    Protocol,
    Instrument,
    Batch,
    Qc,
    Processing,
}

impl Stage {
    /// The order 27.10 lists them in, which this module treats as the pipeline order.
    pub const PIPELINE: [Stage; 7] = [
        Stage::Collection,
        Stage::Preservation,
        Stage::Protocol,
        Stage::Instrument,
        Stage::Batch,
        Stage::Qc,
        Stage::Processing,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Collection => "collection",
            Stage::Preservation => "preservation",
            Stage::Protocol => "protocol",
            Stage::Instrument => "instrument",
            Stage::Batch => "batch",
            Stage::Qc => "qc",
            Stage::Processing => "processing",
        }
    }

    fn position(self) -> usize {
        Stage::PIPELINE
            .iter()
            .position(|s| *s == self)
            .expect("every stage is in PIPELINE")
    }

    /// Stages that receive the specimen after this one.
    pub fn downstream(self) -> Vec<Stage> {
        let start = self.position().saturating_add(1);
        Stage::PIPELINE.get(start..).unwrap_or_default().to_vec()
    }
}

/// The field a stage record may carry to say what it received.
///
/// A downstream record that declares this is asserting the digest of the acted stage's record as
/// it saw it. [`apply`] refuses when a fault upstream leaves that assertion stale, because a world
/// in which the processing lab received a specimen that no longer exists is internally
/// contradictory in a way no laboratory produces — 27.10's validation item "cross-stage
/// consistency". Records that omit the field make no assertion and are left alone.
pub const UPSTREAM_STATE_FIELD: &str = "upstream_state";

/// A handling variable that can go wrong before measurement.
///
/// Illustrative, and named as such in the module header: these are ordinary laboratory handling
/// variables, each assigned to one of 27.10's own stages. No variant asserts a magnitude, a
/// threshold, or an effect on any measurement — the caller declares the signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "fault", rename_all = "snake_case")]
pub enum FaultKind {
    /// Time between devascularisation and preservation. A collection-stage variable.
    ColdIschaemia { minutes: u32 },
    /// How long the specimen sat in fixative. A preservation-stage variable.
    FixationDuration { hours: u32 },
    /// A different fixative from the protocol's. Preservation.
    FixativeSubstitution { fixative: String },
    /// Repeated freezing and thawing of an aliquot. Processing.
    FreezeThaw { cycles: u32 },
    /// Storage outside the specified temperature range. Preservation.
    StorageExcursion { hours: u32 },
    /// A change of reagent lot partway through a run. Batch.
    ReagentLotChange { lot: String },
    /// Delay between one processing step and the next. Processing.
    ProcessingDelay { hours: u32 },
}

impl FaultKind {
    pub fn stage(&self) -> Stage {
        match self {
            FaultKind::ColdIschaemia { .. } => Stage::Collection,
            FaultKind::FixationDuration { .. }
            | FaultKind::FixativeSubstitution { .. }
            | FaultKind::StorageExcursion { .. } => Stage::Preservation,
            FaultKind::ReagentLotChange { .. } => Stage::Batch,
            FaultKind::FreezeThaw { .. } | FaultKind::ProcessingDelay { .. } => Stage::Processing,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FaultKind::ColdIschaemia { .. } => "cold_ischaemia",
            FaultKind::FixationDuration { .. } => "fixation_duration",
            FaultKind::FixativeSubstitution { .. } => "fixative_substitution",
            FaultKind::FreezeThaw { .. } => "freeze_thaw",
            FaultKind::StorageExcursion { .. } => "storage_excursion",
            FaultKind::ReagentLotChange { .. } => "reagent_lot_change",
            FaultKind::ProcessingDelay { .. } => "processing_delay",
        }
    }
}

/// How hard the fault is applied, in parts per ten thousand.
///
/// Integer so that a sweep is reproducible. [`Intensity::NULL`] is the false-positive control and
/// [`Intensity::FULL`] the declared signature at full strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Intensity(u32);

impl Intensity {
    pub const NULL: Intensity = Intensity(0);
    pub const FULL: Intensity = Intensity(10_000);

    pub fn per_ten_thousand(value: u32) -> Self {
        Intensity(value.min(10_000))
    }

    pub fn get(self) -> u32 {
        self.0
    }

    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    fn scale(self, magnitude: i64) -> i64 {
        magnitude.saturating_mul(self.0 as i64) / 10_000
    }

    fn scale_u32(self, magnitude: u32) -> u32 {
        ((magnitude as u64 * self.0 as u64) / 10_000) as u32
    }
}

/// One change a fault makes to a specimen.
///
/// [`Edit::Biology`] exists so that the biological-state postcondition can actually fire. A shape
/// in which biology were unreachable would make the check vacuous and the guarantee unearned; the
/// point is that an author *can* write a mutation that edits the biology, and [`apply`] rejects it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum Edit {
    Biology { field: String, value: Value },
    /// A QC signal moves. Scaled by intensity.
    Qc { field: String, delta: i64 },
    /// The handling record gains or changes a field. **Not** scaled by intensity, because a record
    /// entry is not a magnitude — which is exactly how a family loses its false-positive control,
    /// and why [`validate_family`] checks for it.
    Handling {
        stage: Stage,
        field: String,
        value: Value,
    },
    /// How much of an assay axis survives, in parts per ten thousand removed. Scaled by intensity.
    Measurability { axis: String, loss: u32 },
}

/// What the evaluation asks of an agent — 27.10's critical design decision, verbatim: "detection,
/// correction, abstention, or selection of a confirmatory measurement depending on available
/// actions".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expected", rename_all = "snake_case")]
pub enum ExpectedResponse {
    Detect,
    Correct { action: String },
    Abstain,
    SelectConfirmatory { measurement: String },
}

impl ExpectedResponse {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExpectedResponse::Detect => "detect",
            ExpectedResponse::Correct { .. } => "correct",
            ExpectedResponse::Abstain => "abstain",
            ExpectedResponse::SelectConfirmatory { .. } => "select_confirmatory",
        }
    }

    fn requires(&self) -> Option<&str> {
        match self {
            ExpectedResponse::Correct { action } => Some(action),
            ExpectedResponse::SelectConfirmatory { measurement } => Some(measurement),
            ExpectedResponse::Detect | ExpectedResponse::Abstain => None,
        }
    }
}

/// A specimen as the world records it.
///
/// Three separate maps rather than one, because the whole module is about the difference between
/// them. `biological_state` is what the benchmark is about; `handling` is what happened to the
/// material; `qc` and `measurability` are what an instrument would notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Specimen {
    pub id: String,
    /// Opaque. Nothing in this crate interprets a biological value; it only checks that a
    /// pre-analytic fault left the map alone.
    pub biological_state: BTreeMap<String, Value>,
    pub handling: BTreeMap<Stage, BTreeMap<String, Value>>,
    pub qc: BTreeMap<String, i64>,
    /// Parts per ten thousand of each assay axis that remain measurable.
    pub measurability: BTreeMap<String, u32>,
}

impl Specimen {
    pub fn new(id: impl Into<String>) -> Self {
        Specimen {
            id: id.into(),
            biological_state: BTreeMap::new(),
            handling: BTreeMap::new(),
            qc: BTreeMap::new(),
            measurability: BTreeMap::new(),
        }
    }

    pub fn with_biology(mut self, field: impl Into<String>, value: Value) -> Self {
        self.biological_state.insert(field.into(), value);
        self
    }

    pub fn with_qc(mut self, field: impl Into<String>, value: i64) -> Self {
        self.qc.insert(field.into(), value);
        self
    }

    pub fn with_measurability(mut self, axis: impl Into<String>, remaining: u32) -> Self {
        self.measurability.insert(axis.into(), remaining.min(10_000));
        self
    }

    pub fn with_handling(
        mut self,
        stage: Stage,
        field: impl Into<String>,
        value: Value,
    ) -> Self {
        self.handling
            .entry(stage)
            .or_default()
            .insert(field.into(), value);
        self
    }

    /// Digest of the biological state alone.
    ///
    /// The quantity the central postcondition compares. Taken over the biology map only, so a
    /// change to handling, QC or measurability leaves it untouched by construction — which is the
    /// definition of a pre-analytic fault.
    pub fn biology_digest(&self) -> String {
        digest_of(&self.biological_state)
    }

    /// Digest of the whole record, used by the false-positive control.
    pub fn digest(&self) -> String {
        digest_of(self)
    }

    fn stage_digest(&self, stage: Stage) -> String {
        digest_of(&self.handling.get(&stage))
    }
}

fn digest_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| ContentHash::of_value(&v).ok())
        .map(|h| h.as_str().to_string())
        .unwrap_or_else(|| "uncanonicalisable".to_string())
}

/// A declared pre-analytic fault. Declares; does not check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreanalyticMutation {
    pub id: String,
    /// The family this belongs to, so that a sweep over intensities can be validated together.
    pub family: String,
    pub kind: FaultKind,
    pub intensity: Intensity,
    pub edits: Vec<Edit>,
    pub expected: ExpectedResponse,
}

impl PreanalyticMutation {
    pub fn new(
        id: impl Into<String>,
        family: impl Into<String>,
        kind: FaultKind,
        intensity: Intensity,
        expected: ExpectedResponse,
    ) -> Self {
        PreanalyticMutation {
            id: id.into(),
            family: family.into(),
            kind,
            intensity,
            edits: Vec::new(),
            expected,
        }
    }

    pub fn editing(mut self, edit: Edit) -> Self {
        self.edits.push(edit);
        self
    }

    pub fn at(mut self, intensity: Intensity) -> Self {
        self.intensity = intensity;
        self
    }
}

/// A specimen after a fault, with the postcondition results that admitted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faulted {
    pub mutation: String,
    pub specimen: Specimen,
    /// QC fields that moved, and by how much after scaling.
    pub qc_signature: BTreeMap<String, i64>,
    pub measurability_lost: BTreeMap<String, u32>,
    pub stage: Stage,
}

impl Faulted {
    /// Whether the fault left anything an instrument could notice.
    pub fn has_signature(&self) -> bool {
        !self.qc_signature.is_empty() || !self.measurability_lost.is_empty()
    }
}

/// Apply a fault and run its postconditions.
///
/// Refuses rather than returning a damaged specimen. See the module header for the four checks and
/// why each is 27.10's rather than this crate's invention.
pub fn apply(
    specimen: &Specimen,
    mutation: &PreanalyticMutation,
) -> Result<Faulted, PreanalyticRefusal> {
    let acted = mutation.kind.stage();
    let biology_before = specimen.biology_digest();

    for edit in &mutation.edits {
        if let Edit::Qc { field, .. } = edit {
            if names_the_fault(field, mutation) {
                return Err(PreanalyticRefusal::QcLabelLeaksAnswer {
                    mutation: mutation.id.clone(),
                    field: field.clone(),
                });
            }
        }
        if let Edit::Handling { field, value, .. } = edit {
            if names_the_fault(field, mutation)
                || value.as_str().is_some_and(|v| names_the_fault(v, mutation))
            {
                return Err(PreanalyticRefusal::QcLabelLeaksAnswer {
                    mutation: mutation.id.clone(),
                    field: field.clone(),
                });
            }
        }
    }

    let mut out = specimen.clone();
    let mut qc_signature = BTreeMap::new();
    let mut measurability_lost = BTreeMap::new();

    for edit in &mutation.edits {
        match edit {
            Edit::Biology { field, value } => {
                out.biological_state.insert(field.clone(), value.clone());
            }
            Edit::Qc { field, delta } => {
                let scaled = mutation.intensity.scale(*delta);
                if scaled != 0 {
                    let entry = out.qc.entry(field.clone()).or_insert(0);
                    *entry = entry.saturating_add(scaled);
                    qc_signature.insert(field.clone(), scaled);
                }
            }
            Edit::Handling {
                stage,
                field,
                value,
            } => {
                out.handling
                    .entry(*stage)
                    .or_default()
                    .insert(field.clone(), value.clone());
            }
            Edit::Measurability { axis, loss } => {
                let scaled = mutation.intensity.scale_u32(*loss);
                if scaled != 0 {
                    let entry = out.measurability.entry(axis.clone()).or_insert(10_000);
                    *entry = entry.saturating_sub(scaled);
                    measurability_lost.insert(axis.clone(), scaled);
                }
            }
        }
    }

    if out.biology_digest() != biology_before {
        let field = mutation
            .edits
            .iter()
            .find_map(|e| match e {
                Edit::Biology { field, .. } => Some(field.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        return Err(PreanalyticRefusal::BiologicalStateChanged {
            mutation: mutation.id.clone(),
            field,
        });
    }

    if !mutation.intensity.is_null() && qc_signature.is_empty() && measurability_lost.is_empty() {
        return Err(PreanalyticRefusal::NoQcSignature {
            mutation: mutation.id.clone(),
        });
    }

    let acted_digest = out.stage_digest(acted);
    for downstream in acted.downstream() {
        let stale = out
            .handling
            .get(&downstream)
            .and_then(|record| record.get(UPSTREAM_STATE_FIELD))
            .and_then(Value::as_str)
            .is_some_and(|declared| declared != acted_digest);
        if stale {
            return Err(PreanalyticRefusal::StagesInconsistent {
                mutation: mutation.id.clone(),
                stage: acted.as_str().to_string(),
                downstream: downstream.as_str().to_string(),
            });
        }
    }

    Ok(Faulted {
        mutation: mutation.id.clone(),
        specimen: out,
        qc_signature,
        measurability_lost,
        stage: acted,
    })
}

fn names_the_fault(text: &str, mutation: &PreanalyticMutation) -> bool {
    let lowered = text.to_lowercase();
    lowered.contains(mutation.kind.as_str())
        || lowered.contains(&mutation.id.to_lowercase())
        || lowered.contains(&mutation.family.to_lowercase())
}

/// Check that a family has a genuine false-positive control.
///
/// 27.10's validation item. The null member must leave the specimen byte-identical; a family whose
/// zero-intensity run still stamps something into the handling record produces detections that
/// cannot be told apart from the harness's own footprint.
///
/// The members are given rather than generated, because a family's intensity ladder is the
/// author's design and this crate does not know what rungs are meaningful for a given fault.
pub fn validate_family(
    specimen: &Specimen,
    family: &str,
    members: &[PreanalyticMutation],
) -> Result<(), PreanalyticRefusal> {
    let before = specimen.digest();
    for member in members.iter().filter(|m| m.intensity.is_null()) {
        let faulted = apply(specimen, member)?;
        if faulted.specimen.digest() != before {
            return Err(PreanalyticRefusal::NullMemberIsNotNull {
                family: family.to_string(),
            });
        }
    }
    Ok(())
}

/// The smallest intensity in a sweep at which the fault becomes visible.
///
/// 27.10's validation item "detectability range". `None` means no member in the sweep crossed the
/// threshold — which is a finding about the family, not a failure of the sweep, and is why the
/// return type is an `Option` rather than a saturating default.
///
/// The threshold is the caller's: a QC panel's alert level is a property of a laboratory, and this
/// crate has no laboratory.
pub fn detectability_floor(
    specimen: &Specimen,
    sweep: &[PreanalyticMutation],
    qc_field: &str,
    alert_at: i64,
) -> Option<Intensity> {
    let mut ordered: Vec<&PreanalyticMutation> = sweep.iter().collect();
    ordered.sort_by_key(|m| m.intensity);
    for member in ordered {
        let Ok(faulted) = apply(specimen, member) else {
            continue;
        };
        if faulted
            .qc_signature
            .get(qc_field)
            .is_some_and(|delta| delta.abs() >= alert_at)
        {
            return Some(member.intensity);
        }
    }
    None
}

/// Check that the response a mutation expects is one the world actually offers.
///
/// 27.10's critical design decision makes the asked-for response depend on "available actions", so
/// asking for a correction in a world with no correction action, or a confirmatory measurement
/// that does not exist, is an unanswerable task dressed as a hard one.
pub fn check_response(
    mutation: &PreanalyticMutation,
    available_actions: &BTreeSet<String>,
) -> Result<(), PreanalyticRefusal> {
    if let Some(required) = mutation.expected.requires() {
        if !available_actions.contains(required) {
            return Err(PreanalyticRefusal::ResponseNotAvailable {
                mutation: mutation.id.clone(),
                response: mutation.expected.as_str().to_string(),
                missing: required.to_string(),
            });
        }
    }
    Ok(())
}
