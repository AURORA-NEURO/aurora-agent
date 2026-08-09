//! Correctness decomposition by layer (26.02).
//!
//! 26.02's design detail is the one sentence in the section worth building around: "The engine
//! records both the first incorrect layer and the final consequence. This makes a sample-lineage
//! failure distinguishable from a reasoning failure even when both produce the same wrong
//! conclusion." Two runs can emit the identical wrong sentence — *this tumour is IDH-mutant* — and
//! be entirely different defects. One read the wrong tube. The other read the right tube and
//! reasoned badly. A harness that records only the sentence has thrown away the only thing that
//! would tell an engineer which system to fix.
//!
//! So the unit of record is a [`LayeredOutcome`]: a verdict per [`CorrectnessLayer`] plus the
//! [`Conclusion`] that came out the end, and the pair is the diagnostic. [`FailureSignature`]
//! makes the pair explicit and comparable.
//!
//! # Downstream layers go void, not correct
//!
//! 26.02 step 5 asks that critical failures propagate to downstream claims. The propagation here
//! is deliberately not a penalty — it is a change of *modality*. Once the specimen identity is
//! wrong, the statistical estimand computed over that specimen is not incorrect; it is
//! undefined, and calling it correct because the arithmetic checked out is 26.02's "technically
//! valid analysis on swapped specimens" recorded as a pass. [`LayerVerdict::Void`] is the third
//! value that makes the record honest.
//!
//! # Not implemented
//!
//! Layers are totally ordered by declaration, which asserts that specimen identity always
//! precedes measurement interpretation and so on. Real pipelines have partial orders — a
//! mechanistic scope error can precede a measurement error in an agentic run that reasoned before
//! it measured. Modelling that needs the decision trace of blueprint 26.22, which this crate does
//! not consume; until then a caller with an out-of-order pipeline should record the layer where
//! the *evidence* sits, not where the step ran.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::wrongness::{BiologicalErrorClass, Severity};

/// The levels at which a biological claim can be wrong, ordered upstream to downstream.
///
/// Taken from 26.02's evaluation target list. The ordering is load-bearing: `Ord` is what
/// [`LayeredOutcome`] uses to find the first failure and to decide which layers a critical
/// failure invalidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectnessLayer {
    /// The gene, variant, drug or ontology term the claim is about.
    EntityIdentifier,
    /// Which specimen, block, aliquot or subject the evidence came from.
    SpecimenIdentity,
    /// What the assay's numbers mean — threshold, dynamic range, the difference between absent
    /// and below the limit of detection.
    MeasurementInterpretation,
    /// The estimand and the analysis that targets it.
    StatisticalEstimand,
    /// Whether the claim is causal, associational, or scoped to a subtype.
    MechanisticScope,
    /// Moving a claim between scales — in vitro to in vivo, cell line to patient, model organism
    /// to human.
    ScaleTranslation,
    /// Whether the output would actually inform the decision or experiment it was produced for.
    DecisionUtility,
}

impl CorrectnessLayer {
    pub const CANONICAL: [CorrectnessLayer; 7] = [
        CorrectnessLayer::EntityIdentifier,
        CorrectnessLayer::SpecimenIdentity,
        CorrectnessLayer::MeasurementInterpretation,
        CorrectnessLayer::StatisticalEstimand,
        CorrectnessLayer::MechanisticScope,
        CorrectnessLayer::ScaleTranslation,
        CorrectnessLayer::DecisionUtility,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CorrectnessLayer::EntityIdentifier => "entity_identifier",
            CorrectnessLayer::SpecimenIdentity => "specimen_identity",
            CorrectnessLayer::MeasurementInterpretation => "measurement_interpretation",
            CorrectnessLayer::StatisticalEstimand => "statistical_estimand",
            CorrectnessLayer::MechanisticScope => "mechanistic_scope",
            CorrectnessLayer::ScaleTranslation => "scale_translation",
            CorrectnessLayer::DecisionUtility => "decision_utility",
        }
    }
}

impl fmt::Display for CorrectnessLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failure with its class and its position, plus what was seen against what was expected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifiedError {
    pub layer: CorrectnessLayer,
    pub class: BiologicalErrorClass,
    pub observed: String,
    pub expected: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl ClassifiedError {
    pub fn new(
        layer: CorrectnessLayer,
        class: BiologicalErrorClass,
        observed: impl Into<String>,
        expected: impl Into<String>,
    ) -> Self {
        ClassifiedError {
            layer,
            class,
            observed: observed.into(),
            expected: expected.into(),
            note: None,
        }
    }

    pub fn noting(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn severity(&self) -> Severity {
        self.class.severity()
    }
}

/// What one layer of a run is worth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum LayerVerdict {
    Correct,
    Failed(ClassifiedError),
    /// Undefined because an upstream critical failure removed the ground it stands on. Assigned
    /// by [`LayeredOutcome`], never by the caller.
    Void { blocked_by: CorrectnessLayer },
    /// Nobody looked. Distinct from [`LayerVerdict::Correct`] in exactly the way an unrun test is
    /// distinct from a passing one.
    NotAssessed,
}

impl LayerVerdict {
    pub fn is_failure(&self) -> bool {
        matches!(self, LayerVerdict::Failed(_))
    }

    pub fn error(&self) -> Option<&ClassifiedError> {
        match self {
            LayerVerdict::Failed(e) => Some(e),
            _ => None,
        }
    }
}

/// What came out the end of the run.
///
/// Recorded independently of the layer verdicts, because the whole point is that the same
/// conclusion can sit on top of different failures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "conclusion", rename_all = "snake_case")]
pub enum Conclusion {
    /// The final claim matched the reference standard's admissible set.
    Held { statement: String },
    /// The final claim did not.
    Wrong { statement: String },
    /// The system declined to conclude. 26.04 treats abstention as a first-class outcome, and it
    /// is not a wrong answer.
    Withheld { reason: String },
}

impl Conclusion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Conclusion::Held { .. } => "held",
            Conclusion::Wrong { .. } => "wrong",
            Conclusion::Withheld { .. } => "withheld",
        }
    }
}

/// The pair that distinguishes two runs with the same output.
///
/// 26.02's diagnostic output 7 asks each run to suggest "a regression-cell family"; this is the
/// key that family is grouped by. Two runs sharing a signature are the same defect and should be
/// minimised together; two runs sharing only a conclusion are not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FailureSignature {
    pub first_failed_layer: Option<CorrectnessLayer>,
    pub first_error_class: Option<BiologicalErrorClass>,
    pub conclusion: &'static str,
}

impl FailureSignature {
    /// A stable string key for grouping regression cells.
    pub fn regression_family(&self) -> String {
        let layer = self
            .first_failed_layer
            .map_or("none", CorrectnessLayer::as_str);
        let class = self
            .first_error_class
            .map_or("none", BiologicalErrorClass::as_str);
        format!("{layer}/{class}/{}", self.conclusion)
    }
}

/// A run scored layer by layer.
///
/// Built through [`LayeredOutcome::assess`], which applies critical-failure propagation. There is
/// no way to hand-assemble one with a `Correct` verdict sitting downstream of a specimen swap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayeredOutcome {
    verdicts: BTreeMap<CorrectnessLayer, LayerVerdict>,
    conclusion: Conclusion,
}

impl LayeredOutcome {
    /// Records the observed layer verdicts and propagates critical failures downstream.
    ///
    /// Layers not mentioned default to [`LayerVerdict::NotAssessed`]. Every layer strictly after
    /// the first critical failure becomes [`LayerVerdict::Void`], whatever the caller observed
    /// there — including layers the caller marked `Correct`.
    pub fn assess(
        conclusion: Conclusion,
        observations: impl IntoIterator<Item = (CorrectnessLayer, LayerVerdict)>,
    ) -> Self {
        let mut verdicts: BTreeMap<CorrectnessLayer, LayerVerdict> = CorrectnessLayer::CANONICAL
            .iter()
            .map(|&l| (l, LayerVerdict::NotAssessed))
            .collect();
        for (layer, verdict) in observations {
            verdicts.insert(layer, verdict);
        }

        let critical = verdicts
            .iter()
            .find(|(_, v)| {
                v.error()
                    .is_some_and(|e| e.severity() == Severity::Critical)
            })
            .map(|(&layer, _)| layer);

        if let Some(blocked_by) = critical {
            for (&layer, verdict) in verdicts.iter_mut() {
                if layer > blocked_by {
                    *verdict = LayerVerdict::Void { blocked_by };
                }
            }
        }

        LayeredOutcome {
            verdicts,
            conclusion,
        }
    }

    pub fn conclusion(&self) -> &Conclusion {
        &self.conclusion
    }

    pub fn verdict(&self, layer: CorrectnessLayer) -> &LayerVerdict {
        self.verdicts
            .get(&layer)
            .expect("every canonical layer is populated at construction")
    }

    pub fn errors(&self) -> impl Iterator<Item = &ClassifiedError> {
        self.verdicts.values().filter_map(LayerVerdict::error)
    }

    /// The earliest layer that failed. This, not the conclusion, is what a triage queue sorts on.
    pub fn first_failed_layer(&self) -> Option<CorrectnessLayer> {
        self.verdicts
            .iter()
            .find(|(_, v)| v.is_failure())
            .map(|(&layer, _)| layer)
    }

    pub fn first_error(&self) -> Option<&ClassifiedError> {
        self.verdicts.values().find_map(LayerVerdict::error)
    }

    pub fn worst_severity(&self) -> Option<Severity> {
        self.errors().map(ClassifiedError::severity).max()
    }

    /// Whether any layer was voided by an upstream critical failure.
    pub fn has_void_layers(&self) -> bool {
        self.verdicts
            .values()
            .any(|v| matches!(v, LayerVerdict::Void { .. }))
    }

    pub fn signature(&self) -> FailureSignature {
        let first = self.first_error();
        FailureSignature {
            first_failed_layer: self.first_failed_layer(),
            first_error_class: first.map(|e| e.class),
            conclusion: self.conclusion.as_str(),
        }
    }
}
