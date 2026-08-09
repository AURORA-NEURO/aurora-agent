//! AssayLenses: the measurement process as a declared object.
//!
//! Blueprint 24.08 writes the lens as `Y = A(Z, S, P, I, B, R) + ε` — latent state seen through
//! specimen state, protocol, instrument, batch, and computational processing. The equation's
//! content is that `Y` is not `Z`, and that four of the six arguments are things a benchmark
//! usually forgets to record.
//!
//! This module does not implement `A`. Implementing a measurement operator means simulating an
//! assay, and 24.08 is explicit that a synthetic noise model "must not be represented as
//! empirical truth". What it implements is the declaration and two decisions the blueprint
//! names as evaluation tasks in their own right: whether two measurements are comparable
//! ([`AssayLens::comparable_with`]), and whether a threshold may cross protocols
//! ([`AssayLens::transport_threshold_to`]).
//!
//! Comparability here is conservative — any difference on a comparability dimension is refused,
//! including batch and site. That will refuse comparisons a careful analyst could defend after
//! adjustment. The asymmetry is deliberate: 24.08's named failure is "detect that an apparent
//! biological difference is a batch effect", and a false refusal costs an analyst one explicit
//! override, while a false permission costs a finding.

use crate::error::LensError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The declared contents of one measurement operator (24.08, "lens contents").
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AssayLens {
    pub id: String,
    /// The biological quantity the lens is aimed at — which is not the quantity it returns.
    pub target_quantity: String,
    pub specimen_requirements: String,
    /// Whether using this lens consumes specimen. Feeds the tissue ledger of 24.10.
    #[serde(default)]
    pub destructive: bool,
    pub collection_and_preservation: String,
    pub protocol_version: String,
    pub instrument_configuration: String,
    pub batch_and_site: String,
    /// Reference build, annotation or ontology version. Two runs against different builds are
    /// not two measurements of the same thing.
    pub reference_build: String,
    pub preprocessing_path: String,
    /// Sensitivity, specificity, dynamic range and missingness, as declared. Required before
    /// scoring, because 24.08 asks systems to judge "whether a negative result is informative
    /// given sensitivity" — a question with no answer if sensitivity was never stated.
    pub sensitivity_and_specificity: String,
    pub explicit_limits: String,
    #[serde(default)]
    pub known_artifacts: Vec<String>,
    #[serde(default)]
    pub golden_fixtures: BTreeSet<String>,
    #[serde(default)]
    pub failure_cases: BTreeSet<String>,
    /// Empirical calibration data, when they exist.
    pub calibration_evidence: Option<String>,
    /// Expert assumptions standing in for calibration. 24.08 accepts either, and refuses
    /// neither being present.
    pub declared_expert_assumptions: Option<String>,
    #[serde(default)]
    pub synthetic_noise_model: bool,
    /// Whether the lens's output is being described as empirical. In combination with
    /// `synthetic_noise_model`, the one flag pair that is refused outright.
    #[serde(default)]
    pub presented_as_empirical: bool,
    #[serde(default)]
    pub valid_comparison_operations: BTreeSet<String>,
}

/// A dimension on which two lenses must agree before their outputs may be compared.
const COMPARABILITY_DIMENSIONS: [&str; 5] = [
    "protocol version",
    "instrument configuration",
    "batch and site",
    "reference build",
    "preprocessing path",
];

impl AssayLens {
    fn comparability_values(&self) -> [&String; 5] {
        [
            &self.protocol_version,
            &self.instrument_configuration,
            &self.batch_and_site,
            &self.reference_build,
            &self.preprocessing_path,
        ]
    }

    /// The validation requirements of 24.08: golden fixtures, calibration data *or* declared
    /// expert assumptions, failure-case examples, versioned metadata, and explicit limits.
    pub fn check(&self) -> Result<(), LensError> {
        if self.synthetic_noise_model && self.presented_as_empirical {
            return Err(LensError::SyntheticPresentedAsEmpirical {
                lens: self.id.clone(),
            });
        }
        let required: [(&'static str, &String); 4] = [
            ("target biological quantity", &self.target_quantity),
            ("protocol version", &self.protocol_version),
            ("reference build or annotation version", &self.reference_build),
            (
                "sensitivity, specificity, dynamic range and missingness",
                &self.sensitivity_and_specificity,
            ),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(LensError::Incomplete {
                    lens: self.id.clone(),
                    field,
                });
            }
        }
        if self.explicit_limits.trim().is_empty() {
            return Err(LensError::Incomplete {
                lens: self.id.clone(),
                field: "explicit limits",
            });
        }
        if self.golden_fixtures.is_empty() {
            return Err(LensError::Incomplete {
                lens: self.id.clone(),
                field: "golden fixtures",
            });
        }
        if self.failure_cases.is_empty() {
            return Err(LensError::Incomplete {
                lens: self.id.clone(),
                field: "failure-case examples",
            });
        }
        let calibrated = self
            .calibration_evidence
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty());
        let assumed = self
            .declared_expert_assumptions
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty());
        if !calibrated && !assumed {
            return Err(LensError::UncalibratedAndUndeclared {
                lens: self.id.clone(),
            });
        }
        Ok(())
    }

    /// Whether measurements taken through these two lenses may be compared directly.
    pub fn comparable_with(&self, other: &AssayLens) -> Result<(), LensError> {
        for (index, dimension) in COMPARABILITY_DIMENSIONS.iter().enumerate() {
            let left = self.comparability_values()[index];
            let right = other.comparability_values()[index];
            if left != right {
                return Err(LensError::Incomparable {
                    left: self.id.clone(),
                    right: other.id.clone(),
                    dimension,
                    left_value: left.clone(),
                    right_value: right.clone(),
                });
            }
        }
        Ok(())
    }

    /// Whether a decision threshold fitted through this lens may be applied through `other`.
    ///
    /// Stricter than comparability in the dimensions that matter for a cut point, and narrower:
    /// batch and site differences shift a distribution but a protocol or preprocessing change
    /// redefines the quantity, which is the case 24.08 calls out.
    pub fn transport_threshold_to(&self, other: &AssayLens) -> Result<(), LensError> {
        for (dimension, left, right) in [
            (
                "protocol version",
                &self.protocol_version,
                &other.protocol_version,
            ),
            (
                "preprocessing path",
                &self.preprocessing_path,
                &other.preprocessing_path,
            ),
            (
                "reference build",
                &self.reference_build,
                &other.reference_build,
            ),
        ] {
            if left != right {
                return Err(LensError::Incomparable {
                    left: self.id.clone(),
                    right: other.id.clone(),
                    dimension,
                    left_value: left.clone(),
                    right_value: right.clone(),
                });
            }
        }
        Ok(())
    }
}

/// One stage in a composed lens: biopsy, fixation, sectioning, staining, scanning, tiling,
/// feature extraction, classifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LensStage {
    pub name: String,
    /// What this stage takes in.
    pub consumes: String,
    /// What it hands on.
    pub produces: String,
}

/// A chain of stages, with the intermediate lineage 24.08 requires composition to preserve.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LensComposition {
    stages: Vec<LensStage>,
}

impl LensComposition {
    pub fn new() -> Self {
        LensComposition::default()
    }

    /// Appends a stage, refusing one whose input is not the previous stage's output.
    ///
    /// The refusal is what makes the composition a lens rather than a list. A pipeline whose
    /// stages do not actually chain is the "wrong raw-file pairing" failure of 24.04 wearing a
    /// diagram.
    pub fn then(mut self, stage: LensStage) -> Result<Self, LensError> {
        if let Some(previous) = self.stages.last() {
            if previous.produces != stage.consumes {
                return Err(LensError::UncomposableStages {
                    upstream: previous.name.clone(),
                    downstream: stage.name.clone(),
                    produces: previous.produces.clone(),
                    consumes: stage.consumes.clone(),
                });
            }
        }
        self.stages.push(stage);
        Ok(self)
    }

    /// Every stage, in order. Composition never collapses to endpoints, because a benchmark
    /// "may intervene at any stage to create a controlled fault" and cannot intervene in a
    /// stage the composition forgot.
    pub fn stages(&self) -> &[LensStage] {
        &self.stages
    }

    pub fn produces(&self) -> Option<&str> {
        self.stages.last().map(|stage| stage.produces.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lens(id: &str) -> AssayLens {
        AssayLens {
            id: id.to_string(),
            target_quantity: "MGMT promoter methylation fraction".to_string(),
            specimen_requirements: "one 10um FFPE section".to_string(),
            destructive: true,
            collection_and_preservation: "FFPE, <24h to fixation".to_string(),
            protocol_version: "pyroseq-v2".to_string(),
            instrument_configuration: "PyroMark Q48".to_string(),
            batch_and_site: "site-a/batch-7".to_string(),
            reference_build: "GRCh38.p14".to_string(),
            preprocessing_path: "bisulfite-qc-v3".to_string(),
            sensitivity_and_specificity: "sensitivity 0.91, specificity 0.88".to_string(),
            explicit_limits: "not validated below 10% tumour content".to_string(),
            known_artifacts: vec!["incomplete bisulfite conversion".to_string()],
            golden_fixtures: ["fixture:mgmt:methylated".to_string()].into(),
            failure_cases: ["failure:low-tumour-content".to_string()].into(),
            calibration_evidence: Some("titration series, n=12".to_string()),
            declared_expert_assumptions: None,
            synthetic_noise_model: false,
            presented_as_empirical: true,
            valid_comparison_operations: ["within-batch difference".to_string()].into(),
        }
    }

    #[test]
    fn a_lens_with_neither_calibration_data_nor_declared_assumptions_is_refused() {
        let mut lens = lens("lens:a");
        lens.calibration_evidence = None;
        assert_eq!(
            lens.check().unwrap_err(),
            LensError::UncalibratedAndUndeclared {
                lens: "lens:a".to_string()
            }
        );
    }

    #[test]
    fn declared_expert_assumptions_are_an_acceptable_substitute_for_calibration_data() {
        let mut lens = lens("lens:a");
        lens.calibration_evidence = None;
        lens.declared_expert_assumptions = Some("assumed linear in the 10-90% range".to_string());
        assert!(lens.check().is_ok());
    }

    #[test]
    fn a_synthetic_noise_model_presented_as_empirical_truth_is_refused() {
        let mut lens = lens("lens:a");
        lens.synthetic_noise_model = true;
        assert_eq!(
            lens.check().unwrap_err(),
            LensError::SyntheticPresentedAsEmpirical {
                lens: "lens:a".to_string()
            }
        );
    }

    #[test]
    fn a_synthetic_noise_model_is_legal_as_long_as_it_is_not_called_empirical() {
        let mut lens = lens("lens:a");
        lens.synthetic_noise_model = true;
        lens.presented_as_empirical = false;
        assert!(lens.check().is_ok());
    }

    #[test]
    fn a_lens_that_never_states_its_sensitivity_cannot_support_an_informative_negative() {
        let mut lens = lens("lens:a");
        lens.sensitivity_and_specificity = String::new();
        assert!(matches!(
            lens.check().unwrap_err(),
            LensError::Incomplete { .. }
        ));
    }

    #[test]
    fn a_lens_without_golden_fixtures_is_not_validated() {
        let mut lens = lens("lens:a");
        lens.golden_fixtures.clear();
        assert_eq!(
            lens.check().unwrap_err(),
            LensError::Incomplete {
                lens: "lens:a".to_string(),
                field: "golden fixtures"
            }
        );
    }

    #[test]
    fn measurements_from_two_batches_are_not_directly_comparable_and_the_dimension_is_named() {
        let left = lens("lens:a");
        let mut right = lens("lens:b");
        right.batch_and_site = "site-b/batch-3".to_string();
        assert_eq!(
            left.comparable_with(&right).unwrap_err(),
            LensError::Incomparable {
                left: "lens:a".to_string(),
                right: "lens:b".to_string(),
                dimension: "batch and site",
                left_value: "site-a/batch-7".to_string(),
                right_value: "site-b/batch-3".to_string()
            }
        );
    }

    #[test]
    fn two_lenses_agreeing_on_every_comparability_dimension_are_comparable() {
        assert!(lens("lens:a").comparable_with(&lens("lens:b")).is_ok());
    }

    #[test]
    fn a_threshold_may_cross_batches_but_never_a_protocol_change() {
        let left = lens("lens:a");
        let mut other_batch = lens("lens:b");
        other_batch.batch_and_site = "site-b/batch-3".to_string();
        assert!(left.transport_threshold_to(&other_batch).is_ok());

        let mut other_protocol = lens("lens:c");
        other_protocol.protocol_version = "msp-v1".to_string();
        assert!(matches!(
            left.transport_threshold_to(&other_protocol).unwrap_err(),
            LensError::Incomparable {
                dimension: "protocol version",
                ..
            }
        ));
    }

    #[test]
    fn a_reference_build_change_blocks_threshold_transport() {
        let left = lens("lens:a");
        let mut rebuilt = lens("lens:b");
        rebuilt.reference_build = "GRCh37".to_string();
        assert!(left.transport_threshold_to(&rebuilt).is_err());
    }

    #[test]
    fn a_stage_whose_input_is_not_the_previous_stages_output_cannot_be_composed() {
        let composition = LensComposition::new()
            .then(LensStage {
                name: "sectioning".to_string(),
                consumes: "ffpe-block".to_string(),
                produces: "slide".to_string(),
            })
            .unwrap();
        let err = composition
            .then(LensStage {
                name: "tiling".to_string(),
                consumes: "whole-slide-image".to_string(),
                produces: "tiles".to_string(),
            })
            .unwrap_err();
        assert_eq!(
            err,
            LensError::UncomposableStages {
                upstream: "sectioning".to_string(),
                downstream: "tiling".to_string(),
                produces: "slide".to_string(),
                consumes: "whole-slide-image".to_string()
            }
        );
    }

    #[test]
    fn a_composed_lens_keeps_every_intermediate_stage_so_a_fault_can_be_injected_at_any_of_them() {
        let composition = LensComposition::new()
            .then(LensStage {
                name: "sectioning".to_string(),
                consumes: "ffpe-block".to_string(),
                produces: "slide".to_string(),
            })
            .unwrap()
            .then(LensStage {
                name: "staining".to_string(),
                consumes: "slide".to_string(),
                produces: "stained-slide".to_string(),
            })
            .unwrap()
            .then(LensStage {
                name: "scanning".to_string(),
                consumes: "stained-slide".to_string(),
                produces: "whole-slide-image".to_string(),
            })
            .unwrap();
        assert_eq!(composition.stages().len(), 3);
        assert_eq!(composition.produces(), Some("whole-slide-image"));
    }
}
