//! Execution faults (32.14) and digital twins (32.20): two ways a computation stops being evidence.
//!
//! 32.14's worked relation: "A differential-analysis tool writes a partial table before failing. The
//! agent must not interpret it as a valid completed result." The file exists, it parses, its columns
//! are right, and it is not a result. Nothing about the artifact says so — only the outcome of the
//! step that wrote it does.
//!
//! So [`ToolOutcome::result`] returns `Option<&ContentHash>` and only [`ToolOutcome::Completed`]
//! returns `Some`. There is no `output()` that reaches the bytes of a partial write, because the
//! moment such a method exists it is the one that gets called.
//!
//! # Retry is a decision about the fault, not about the caller's patience
//!
//! 32.14's failure risks list "blind retries" second. [`retry_advice`] answers per fault: a timeout is
//! retryable, a corrupted cache is retryable *after* invalidation and comes back unresolved naming
//! that, and a version incompatibility is a contradiction — retrying it repeats it. Any policy that
//! returns one answer for all three is the blind retry.
//!
//! # Twins
//!
//! 32.20's failure risks are "agent learns simulator quirks", "realism mistaken for validity" and
//! "calibration data leaks into test". The third is decidable from two unit sets and
//! [`calibration_separation`] decides it. The second is decidable from a declaration:
//! [`twin_supports`] refuses to let a simulator establish anything about real biology without
//! transfer evidence, no matter how well it fits. A twin's exact latent state is exact about the twin.
//!
//! # Not implemented
//!
//! No simulation, no parameter recovery, no posterior predictive checks, no sandbox, no process
//! supervision. 32.20's validation program needs a simulator and 32.14's needs a runtime; a
//! [`ToolOutcome`] here is a record of something that already happened elsewhere.

use std::collections::BTreeSet;

use bioprism_ids::ContentHash;
use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Witness};

/// How a pipeline step ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ToolOutcome {
    /// The step finished and its output is content-addressed.
    Completed { digest: ContentHash },
    /// The step wrote something and then failed. The bytes exist and are not a result.
    PartialOutput { step: String, wrote: String },
    Timeout { step: String, budget: String },
    /// The tool is not the version the pipeline was written against.
    VersionIncompatible {
        step: String,
        expected: String,
        found: String,
    },
    /// A reference asset does not match the one the analysis assumed.
    ReferenceAssetMismatch { asset: String, expected: String },
    /// A default changed underneath the pipeline without the pipeline changing.
    SilentDefaultChange { setting: String },
    ResourceExhausted { step: String, resource: String },
    CorruptedCache { key: String },
}

impl ToolOutcome {
    /// The output digest, and only for a completed step.
    ///
    /// The one accessor. A caller wanting the partial bytes has to reach into the variant explicitly,
    /// at a call site that names `PartialOutput`.
    pub fn result(&self) -> Option<&ContentHash> {
        match self {
            ToolOutcome::Completed { digest } => Some(digest),
            _ => None,
        }
    }

    pub fn step(&self) -> &str {
        match self {
            ToolOutcome::Completed { .. } => "completed",
            ToolOutcome::PartialOutput { step, .. }
            | ToolOutcome::Timeout { step, .. }
            | ToolOutcome::VersionIncompatible { step, .. }
            | ToolOutcome::ResourceExhausted { step, .. } => step,
            ToolOutcome::ReferenceAssetMismatch { asset, .. } => asset,
            ToolOutcome::SilentDefaultChange { setting } => setting,
            ToolOutcome::CorruptedCache { key } => key,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ToolOutcome::Completed { .. } => "completed",
            ToolOutcome::PartialOutput { .. } => "partial_output",
            ToolOutcome::Timeout { .. } => "timeout",
            ToolOutcome::VersionIncompatible { .. } => "version_incompatible",
            ToolOutcome::ReferenceAssetMismatch { .. } => "reference_asset_mismatch",
            ToolOutcome::SilentDefaultChange { .. } => "silent_default_change",
            ToolOutcome::ResourceExhausted { .. } => "resource_exhausted",
            ToolOutcome::CorruptedCache { .. } => "corrupted_cache",
        }
    }
}

/// Whether a downstream claim may treat this step's output as a completed result.
///
/// [`ToolOutcome::PartialOutput`] is a contradiction rather than an abstention: the bytes exist and
/// something already treated them as complete by asking. Every other failure is not-evaluable, because
/// there is nothing to mistake.
pub fn assert_complete(outcome: &ToolOutcome) -> Determination {
    match outcome {
        ToolOutcome::Completed { digest } => Determination::supported(
            EvidenceTier::Execution,
            format!("the step completed and produced {}", digest.as_str()),
        ),
        ToolOutcome::PartialOutput { step, wrote } => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::IncompleteTreatedAsComplete {
                step: step.clone(),
                outcome: format!("wrote {wrote} and then failed"),
            },
        ),
        other => Determination::not_evaluable(match other {
            ToolOutcome::Timeout { .. } => "the step timed out, so it produced no result",
            ToolOutcome::VersionIncompatible { .. } => {
                "the step ran a different tool version, so its output answers a different question"
            }
            ToolOutcome::ReferenceAssetMismatch { .. } => {
                "the step used a reference asset the analysis did not assume"
            }
            ToolOutcome::SilentDefaultChange { .. } => {
                "a setting changed underneath the pipeline, so the output is not the one specified"
            }
            ToolOutcome::ResourceExhausted { .. } => "the step ran out of resources",
            ToolOutcome::CorruptedCache { .. } => "the step read a corrupted cache",
            ToolOutcome::Completed { .. } | ToolOutcome::PartialOutput { .. } => {
                unreachable!("both handled above")
            }
        }),
    }
}

/// Whether re-running the step could produce a different outcome, and what must happen first.
pub fn retry_advice(outcome: &ToolOutcome) -> Determination {
    match outcome {
        ToolOutcome::Completed { .. } => {
            Determination::not_evaluable("a completed step has nothing to retry")
        }
        ToolOutcome::Timeout { budget, .. } => Determination::supported(
            EvidenceTier::Property,
            format!("a timeout at budget {budget} may resolve on a longer run"),
        ),
        ToolOutcome::ResourceExhausted { resource, .. } => Determination::unresolved(
            format!("more {resource} than the failed run had"),
            "retrying under the same limit repeats the same failure",
        ),
        ToolOutcome::CorruptedCache { key } => Determination::unresolved(
            format!("invalidation of cache entry '{key}'"),
            "retrying against the same cache reads the same corruption",
        ),
        ToolOutcome::PartialOutput { step, .. } => Determination::unresolved(
            format!("removal of the partial output of '{step}'"),
            "retrying with the partial file in place may make it look like a completed run",
        ),
        ToolOutcome::VersionIncompatible {
            expected, found, ..
        } => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "retrying resolves the fault".to_string(),
                expected: format!("tool version {expected}"),
                observed: format!("version {found}, which every retry will also find"),
            },
        ),
        ToolOutcome::ReferenceAssetMismatch { asset, expected } => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "retrying resolves the fault".to_string(),
                expected: format!("{asset} at {expected}"),
                observed: "a different asset, which a retry will not change".to_string(),
            },
        ),
        ToolOutcome::SilentDefaultChange { setting } => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "retrying resolves the fault".to_string(),
                expected: format!("{setting} pinned by the pipeline"),
                observed: "an unpinned default, which a retry inherits".to_string(),
            },
        ),
    }
}

/// A mechanistic model and the units it was fitted and tested on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Twin {
    pub model: String,
    calibration_units: BTreeSet<String>,
    test_units: BTreeSet<String>,
    /// What the model is known to get wrong. Mandatory in spirit: a twin with an empty
    /// misspecification statement has not been characterised, and [`twin_supports`] treats it as such.
    pub known_misspecification: String,
    /// Evidence that findings from this twin held on real data. `None` is the honest default.
    pub transfer_evidence: Option<String>,
}

impl Twin {
    pub fn new(model: impl Into<String>, known_misspecification: impl Into<String>) -> Self {
        Twin {
            model: model.into(),
            calibration_units: BTreeSet::new(),
            test_units: BTreeSet::new(),
            known_misspecification: known_misspecification.into(),
            transfer_evidence: None,
        }
    }

    pub fn calibrated_on(mut self, units: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.calibration_units
            .extend(units.into_iter().map(Into::into));
        self
    }

    pub fn tested_on(mut self, units: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.test_units.extend(units.into_iter().map(Into::into));
        self
    }

    pub fn with_transfer_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.transfer_evidence = Some(evidence.into());
        self
    }
}

/// Whether the twin's calibration and test sets are disjoint.
///
/// 32.20's "calibration data leaks into test" is a set intersection, which makes it one of the few
/// things in section 32 that a check can settle outright.
pub fn calibration_separation(twin: &Twin) -> Determination {
    if twin.calibration_units.is_empty() || twin.test_units.is_empty() {
        return Determination::unresolved(
            "declared calibration and test unit sets",
            "separation cannot be checked when either set is unstated",
        );
    }
    let overlap: Vec<&str> = twin
        .calibration_units
        .intersection(&twin.test_units)
        .map(String::as_str)
        .collect();
    match overlap.first() {
        Some(unit) => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: format!("{} calibration and test sets are disjoint", twin.model),
                expected: "no shared unit".to_string(),
                observed: format!("{} unit(s) shared, including '{unit}'", overlap.len()),
            },
        ),
        None => Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "{} was calibrated on {} units and tested on {} disjoint units",
                twin.model,
                twin.calibration_units.len(),
                twin.test_units.len()
            ),
        ),
    }
}

/// What a twin can establish about a claim on real biology.
///
/// The answer is never more than [`EvidenceTier::Property`], and without transfer evidence it is not
/// support at all. 32.20's "realism mistaken for validity" is a claim about how convincing the
/// simulator looked; this function does not read that, because there is no field for it.
pub fn twin_supports(twin: &Twin, claim_is_about_real_biology: bool) -> Determination {
    if !claim_is_about_real_biology {
        return Determination::supported(
            EvidenceTier::Property,
            format!(
                "{} reproduces its own generative process, which is what a within-simulator claim asks",
                twin.model
            ),
        );
    }
    if twin.known_misspecification.trim().is_empty() {
        return Determination::unresolved(
            "a statement of the model's known misspecification",
            "an uncharacterised simulator cannot bound how far its conclusions transport",
        );
    }
    match &twin.transfer_evidence {
        Some(evidence) => Determination::supported(
            EvidenceTier::Property,
            format!("{} transported: {evidence}", twin.model),
        ),
        None => Determination::unresolved(
            "evidence that this finding holds on observed data",
            format!(
                "{} is a simulator with known misspecification '{}' and no transfer evidence",
                twin.model, twin.known_misspecification
            ),
        ),
    }
}
