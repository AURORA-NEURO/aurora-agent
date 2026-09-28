//! Built-in P03-to-P05 reduction for preclinical glioma invasion microscopy.
//!
//! This bridges locally tracked cells to named P05 mechanism features without using a generic
//! image score. P03 performs biological-unit-held-out calibration and per-field QC; this adapter
//! only admits qualified, in-domain fields and requires a site-calibrated measurement uncertainty.
//! Raw images, tracks, and derived analysis objects remain in the institution's artifact store.

use super::discrimination::{
    MechanismDiscrimination, MechanismDiscriminatorAction, MechanismFeatureObservation,
};
use super::instrument_campaign::{MechanismPhenotypeReducer, MechanismPhenotypeReduction};
use crate::glioma::programs::p03_multimodal_ingestion_qc::microscopy_morphodynamics::{
    analyze_glioma_microscopy_morphodynamics, LabeledMorphodynamicField, MicroscopyFieldInput,
    MicroscopyMorphodynamicAnalysis, MicroscopyMorphodynamicRequest, MorphodynamicFieldDisposition,
    MorphodynamicModelDisposition, MORPHODYNAMIC_FEATURE_ORDER,
};
use crate::glioma::programs::p08_instrument_robotics::execution::{
    InstrumentExecutionDisposition, InstrumentExecutionRun,
};
use crate::glioma_engine::LocalArtifactRef;
use std::collections::{BTreeMap, BTreeSet};

/// An institution-local implementation resolves an acquired image into sorted, segmented cell
/// tracks and persists the derived P03 analysis bundle. It must not export the underlying bytes.
pub trait LocalMorphodynamicArtifactStore {
    fn load_segmented_field(
        &mut self,
        source_artifact: &LocalArtifactRef,
    ) -> Result<MicroscopyFieldInput, String>;

    fn persist_analysis(
        &mut self,
        analysis: &MicroscopyMorphodynamicAnalysis,
    ) -> Result<LocalArtifactRef, String>;
}

/// Maps one P05 phenotype ID to a P03 measurement and site-validated uncertainty floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorphodynamicMechanismFeatureRoute {
    pub mechanism_feature_id: String,
    pub morphodynamic_feature_id: String,
    /// Independently calibrated assay error in the feature's P05 fixed-point units.
    pub calibrated_uncertainty_milli: u64,
}

pub struct GliomaMorphodynamicMechanismReducer<S> {
    request: MicroscopyMorphodynamicRequest,
    reference_fields: Vec<LabeledMorphodynamicField>,
    qualified_reference_model: MicroscopyMorphodynamicAnalysis,
    routes: BTreeMap<String, MorphodynamicMechanismFeatureRoute>,
    store: S,
}

impl<S: LocalMorphodynamicArtifactStore> GliomaMorphodynamicMechanismReducer<S> {
    pub fn new(
        request: MicroscopyMorphodynamicRequest,
        reference_fields: Vec<LabeledMorphodynamicField>,
        routes: Vec<MorphodynamicMechanismFeatureRoute>,
        store: S,
    ) -> Result<Self, String> {
        if request.study_id.trim().is_empty()
            || reference_fields.is_empty()
            || routes.is_empty()
            || routes.iter().any(|route| {
                route.mechanism_feature_id.trim().is_empty()
                    || route.calibrated_uncertainty_milli == 0
                    || route.calibrated_uncertainty_milli > 1_000_000
                    || !MORPHODYNAMIC_FEATURE_ORDER
                        .contains(&route.morphodynamic_feature_id.as_str())
            })
        {
            return Err("a bounded P03 study, labeled reference fields, and calibrated feature routes are required".into());
        }
        let mut route_map = BTreeMap::new();
        for route in routes {
            if route_map
                .insert(route.mechanism_feature_id.clone(), route)
                .is_some()
            {
                return Err("mechanism feature routes must be unique".into());
            }
        }
        let qualified_reference_model =
            analyze_glioma_microscopy_morphodynamics(&request, &reference_fields, &[])
                .map_err(|error| format!("P03 reference-model preflight failed: {error}"))?;
        qualified_reference_model
            .validate()
            .map_err(|error| format!("P03 reference-model preflight output is invalid: {error}"))?;
        if qualified_reference_model.disposition != MorphodynamicModelDisposition::Qualified {
            return Err(format!(
                "P03 reference model must pass held-out qualification before instrument execution: {:?}; negative={:?}; uncertainty={:?}",
                qualified_reference_model.disposition,
                qualified_reference_model.negative_evidence,
                qualified_reference_model.uncertainty
            ));
        }
        Ok(Self {
            request,
            reference_fields,
            qualified_reference_model,
            routes: route_map,
            store,
        })
    }
}

impl<S: LocalMorphodynamicArtifactStore> MechanismPhenotypeReducer
    for GliomaMorphodynamicMechanismReducer<S>
{
    fn reduce_local_measurement(
        &mut self,
        action: &MechanismDiscriminatorAction,
        discrimination: &MechanismDiscrimination,
        instrument_run: &InstrumentExecutionRun,
        source_artifact: &LocalArtifactRef,
    ) -> Result<MechanismPhenotypeReduction, String> {
        instrument_run
            .validate()
            .map_err(|error| format!("instrument run failed validation: {error}"))?;
        if instrument_run.action_order.len() != 1
            || !matches!(
                instrument_run.disposition,
                InstrumentExecutionDisposition::Completed
                    | InstrumentExecutionDisposition::Negative
            )
            || !matches!(
                instrument_run.results[0].disposition,
                InstrumentExecutionDisposition::Completed
                    | InstrumentExecutionDisposition::Negative
            )
            || instrument_run.results[0].artifact.as_ref() != Some(source_artifact)
            || !source_artifact.local_only
            || source_artifact.contains_human_data
            || source_artifact.contains_direct_identifiers
        {
            return Err("P03 reducer requires one complete P08 result with the exact local de-identified source artifact".into());
        }
        let route = self.routes.get(&action.feature_id).ok_or_else(|| {
            format!(
                "no calibrated P03 route exists for mechanism feature {}",
                action.feature_id
            )
        })?;
        if discrimination.model_system != self.request.material.model_system() {
            return Err(
                "P03 microscopy material and P05 mechanism model system do not match".into(),
            );
        }
        let field = self.store.load_segmented_field(source_artifact)?;
        if field.artifact != *source_artifact
            || field.material != self.request.material
            || !field.artifact.local_only
            || field.artifact.contains_human_data
            || field.artifact.contains_direct_identifiers
        {
            return Err("local segmentation returned a field detached from the captured artifact or outside the preclinical material scope".into());
        }
        let reference_units = self
            .reference_fields
            .iter()
            .map(|labeled| labeled.field.independent_unit_id.as_str())
            .collect::<BTreeSet<_>>();
        if reference_units.contains(field.independent_unit_id.as_str())
            || self
                .reference_fields
                .iter()
                .any(|labeled| labeled.field.field_id == field.field_id)
        {
            return Err(
                "new assay field must use an independent biological unit and field identity".into(),
            );
        }
        let analysis = analyze_glioma_microscopy_morphodynamics(
            &self.request,
            &self.reference_fields,
            std::slice::from_ref(&field),
        )
        .map_err(|error| format!("P03 morphodynamic analysis failed: {error}"))?;
        analysis
            .validate()
            .map_err(|error| format!("P03 morphodynamic output failed validation: {error}"))?;
        if analysis.disposition != MorphodynamicModelDisposition::Qualified {
            return Err(format!(
                "P03 model did not pass biological-unit-held-out validation: {:?}; negative={:?}; uncertainty={:?}",
                analysis.disposition, analysis.negative_evidence, analysis.uncertainty
            ));
        }
        if analysis.state_order != self.qualified_reference_model.state_order
            || analysis.feature_order != self.qualified_reference_model.feature_order
            || analysis.prototype_features != self.qualified_reference_model.prototype_features
            || analysis.robust_feature_scales_milli
                != self.qualified_reference_model.robust_feature_scales_milli
        {
            return Err(
                "P03 target analysis did not preserve the prequalified reference model".into(),
            );
        }
        let estimate = analysis
            .fields
            .first()
            .filter(|estimate| {
                estimate.field_id == field.field_id
                    && estimate.source_artifact == *source_artifact
                    && estimate.disposition == MorphodynamicFieldDisposition::Classified
            })
            .ok_or_else(|| {
                format!(
                    "P03 withheld a mechanism measurement for this field: {:?}",
                    analysis
                        .fields
                        .first()
                        .map(|estimate| &estimate.uncertainty)
                )
            })?;
        let value = estimate
            .morphodynamic_features
            .iter()
            .find(|feature| feature.feature_id == route.morphodynamic_feature_id)
            .map(|feature| feature.value_milli)
            .ok_or_else(|| "P03 output is missing the routed morphodynamic feature".to_owned())?;
        if value.unsigned_abs() > 1_000_000 {
            return Err("P03 feature lies outside P05's supported fixed-point range".into());
        }
        let uncertainty = route
            .calibrated_uncertainty_milli
            .max(action.measurement_uncertainty_milli);
        if uncertainty > 1_000_000 {
            return Err(
                "combined site and action uncertainty exceeds P05's supported range".into(),
            );
        }
        let analysis_artifact = self.store.persist_analysis(&analysis)?;
        if analysis_artifact.content_hash != analysis.digest
            || !analysis_artifact.local_only
            || analysis_artifact.contains_human_data
            || analysis_artifact.contains_direct_identifiers
            || analysis_artifact.validate().is_err()
        {
            return Err("persisted P03 analysis artifact is not a local, content-addressed output of this analysis".into());
        }
        Ok(MechanismPhenotypeReduction {
            observation: MechanismFeatureObservation {
                feature_id: action.feature_id.clone(),
                observed_milli: value,
                uncertainty_milli: uncertainty,
                artifact: analysis_artifact,
            },
            source_artifact: source_artifact.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p03_multimodal_ingestion_qc::microscopy_morphodynamics::{
        CellTrackFrame, GliomaMicroscopyMaterial, MicroscopyMorphodynamicRequest, TrackedGliomaCell,
    };
    use crate::glioma::programs::p05_mechanism_exploration::discrimination::{
        MechanismDiscriminationDisposition, MechanismDiscriminationRanking,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        InstrumentExecutionResult, InstrumentExecutionStopReason,
    };
    use crate::glioma_engine::GliomaModelSystem;
    use bioprism_ids::ContentHash;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_value(&serde_json::json!({ "id": id })).unwrap(),
            content_type: "application/vnd.aurora.glioma.microscopy+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn field(id: &str, unit: &str, motile: bool, source: LocalArtifactRef) -> MicroscopyFieldInput {
        let x = if motile { [0, 1_000, 2_000] } else { [0, 0, 0] };
        let elongation = if motile { 1_800 } else { 1_000 };
        MicroscopyFieldInput {
            field_id: id.into(),
            region_id: if motile {
                "invasion-front"
            } else {
                "compact-core"
            }
            .into(),
            independent_unit_id: unit.into(),
            biological_stratum: if motile {
                "invasion-front"
            } else {
                "compact-core"
            }
            .into(),
            material: GliomaMicroscopyMaterial::MurineOrganoid,
            invasion_axis_x_milli: 1_000,
            invasion_axis_y_milli: 0,
            image_quality_milli: 950,
            spatial_novelty_milli: 700,
            artifact: source,
            tracks: (0..2)
                .map(|cell| TrackedGliomaCell {
                    track_id: format!("cell-{cell:02}"),
                    frames: (0..3)
                        .map(|frame| CellTrackFrame {
                            time_millis: frame * 60_000,
                            x_micrometre_milli: x[frame as usize],
                            y_micrometre_milli: 0,
                            area_micrometre_squared_milli: 1_000,
                            elongation_permille: elongation,
                            branch_count: if motile { 1 } else { 0 },
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn references() -> Vec<LabeledMorphodynamicField> {
        let mut fields = Vec::new();
        for unit in 0..3 {
            fields.push(LabeledMorphodynamicField {
                state_id: "invasive".into(),
                field: field(
                    &format!("field-m-{unit:02}"),
                    &format!("unit-m-{unit:02}"),
                    true,
                    artifact(&format!("ref-m-{unit:02}")),
                ),
            });
            fields.push(LabeledMorphodynamicField {
                state_id: "compact".into(),
                field: field(
                    &format!("field-s-{unit:02}"),
                    &format!("unit-s-{unit:02}"),
                    false,
                    artifact(&format!("ref-s-{unit:02}")),
                ),
            });
        }
        fields.sort_by(|left, right| left.field.field_id.cmp(&right.field.field_id));
        fields
    }

    fn analysis_request() -> MicroscopyMorphodynamicRequest {
        MicroscopyMorphodynamicRequest {
            study_id: "murine-organoid-study".into(),
            material: GliomaMicroscopyMaterial::MurineOrganoid,
            min_independent_units_per_state: 2,
            min_tracks_per_field: 2,
            min_image_quality_milli: 700,
            min_balanced_accuracy_milli: 800,
            max_multiclass_brier_milli: 500,
            min_feature_scale_milli: 1_000,
            max_out_of_domain_distance_milli: 100_000,
            min_posterior_milli: 0,
            min_posterior_margin_milli: 0,
            confusion_smoothing_milli: 1,
            analysis_artifact: artifact("analysis-input"),
        }
    }

    struct LocalStore {
        target: MicroscopyFieldInput,
    }

    impl LocalMorphodynamicArtifactStore for LocalStore {
        fn load_segmented_field(
            &mut self,
            source_artifact: &LocalArtifactRef,
        ) -> Result<MicroscopyFieldInput, String> {
            if self.target.artifact != *source_artifact {
                return Err("artifact not found".into());
            }
            Ok(self.target.clone())
        }

        fn persist_analysis(
            &mut self,
            analysis: &MicroscopyMorphodynamicAnalysis,
        ) -> Result<LocalArtifactRef, String> {
            Ok(LocalArtifactRef {
                artifact_id: format!("morphodynamic-analysis:{}", analysis.study_id),
                content_hash: analysis.digest.clone(),
                content_type: "application/vnd.aurora.glioma.microscopy-morphodynamics+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            })
        }
    }

    fn instrument_run(source: LocalArtifactRef) -> InstrumentExecutionRun {
        let action_id = "capture-invasion-front".to_owned();
        let mut run = InstrumentExecutionRun {
            feature_id: crate::glioma::programs::p08_instrument_robotics::execution::FEATURE_ID
                .into(),
            output_schema:
                crate::glioma::programs::p08_instrument_robotics::execution::OUTPUT_SCHEMA.into(),
            objective: "capture preclinical glioma invasion front".into(),
            plan_digest: artifact("plan").content_hash,
            instrument_id: "organoid-imager".into(),
            action_order: vec![action_id.clone()],
            results: vec![InstrumentExecutionResult {
                action_id: action_id.clone(),
                disposition: InstrumentExecutionDisposition::Completed,
                attempt_count: 1,
                started_tick: Some(1),
                completed_tick: Some(3),
                artifact: Some(source),
                note: "captured local test image".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            }],
            completed_order: vec![action_id],
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            unresolved_order: Vec::new(),
            skipped_order: Vec::new(),
            retry_count: 0,
            emergency_stop_requested: false,
            emergency_stop_succeeded: false,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: InstrumentExecutionDisposition::Completed,
            stop_reason: InstrumentExecutionStopReason::Completed,
            digest: ContentHash::of_bytes(b"unsealed-test-instrument-run"),
        };
        run.digest = ContentHash::of_value(
            &crate::glioma::programs::p08_instrument_robotics::execution::digest_input(&run),
        )
        .unwrap();
        run
    }

    fn discrimination() -> MechanismDiscrimination {
        MechanismDiscrimination {
            feature_id: super::super::discrimination::FEATURE_ID.into(),
            output_schema: super::super::discrimination::OUTPUT_SCHEMA.into(),
            objective: "measure glioma invasion speed".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanism_order: vec!["matrix".into(), "tumor-intrinsic".into()],
            rankings: Vec::<MechanismDiscriminationRanking>::new(),
            action_order: Vec::new(),
            actions: Vec::new(),
            selected_action_order: Vec::new(),
            unresolved_mechanism_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: MechanismDiscriminationDisposition::Unresolved,
            digest: ContentHash::of_bytes(b"test-mechanism-discrimination"),
        }
    }

    #[test]
    fn calibrated_p03_invasion_speed_becomes_a_linked_p05_measurement() {
        let source = artifact("captured-invasion-image");
        let target = field("target-front", "unit-new", true, source.clone());
        let mut reducer = GliomaMorphodynamicMechanismReducer::new(
            analysis_request(),
            references(),
            vec![MorphodynamicMechanismFeatureRoute {
                mechanism_feature_id: "invasion-front-speed".into(),
                morphodynamic_feature_id: "speed_um_per_hour_milli".into(),
                calibrated_uncertainty_milli: 25,
            }],
            LocalStore { target },
        )
        .unwrap();
        let action = MechanismDiscriminatorAction {
            action_id: "measure-front-speed".into(),
            feature_id: "invasion-front-speed".into(),
            predicted_milli_by_mechanism: BTreeMap::new(),
            measurement_uncertainty_milli: 20,
            feasibility_milli: 1_000,
            cost_units: 1,
        };
        let reduction = reducer
            .reduce_local_measurement(
                &action,
                &discrimination(),
                &instrument_run(source.clone()),
                &source,
            )
            .unwrap();

        assert_eq!(reduction.source_artifact, source);
        assert_eq!(reduction.observation.feature_id, "invasion-front-speed");
        assert_eq!(reduction.observation.uncertainty_milli, 25);
        assert!(reduction.observation.observed_milli > 0);
        assert_ne!(reduction.observation.artifact, reduction.source_artifact);
        assert_eq!(
            reduction.observation.artifact.content_type,
            "application/vnd.aurora.glioma.microscopy-morphodynamics+json"
        );
    }

    #[test]
    fn underpowered_reference_model_is_rejected_before_execution_adapter_creation() {
        let result = GliomaMorphodynamicMechanismReducer::new(
            analysis_request(),
            references()
                .into_iter()
                .filter(|labeled| labeled.field.field_id.ends_with("-00"))
                .collect(),
            vec![MorphodynamicMechanismFeatureRoute {
                mechanism_feature_id: "invasion-front-speed".into(),
                morphodynamic_feature_id: "speed_um_per_hour_milli".into(),
                calibrated_uncertainty_milli: 25,
            }],
            LocalStore {
                target: field(
                    "target-front",
                    "unit-new",
                    true,
                    artifact("captured-invasion-image"),
                ),
            },
        );

        let error = result.err().expect(
            "underpowered P03 references must be rejected before an execution adapter exists",
        );
        assert!(
            error.contains("must pass held-out qualification"),
            "reference data should be rejected specifically by the held-out qualification gate: {error}"
        );
    }
}
