//! Instrument-backed execution for glioma mechanism-discrimination campaigns.
//!
//! P05 chooses which named phenotype would best separate the current mechanisms. This adapter
//! binds that choice to one institution-approved P08 action, executes it through P08's live
//! authorization/interlock path, and asks an institution-local reducer to extract the requested
//! phenotype from the local artifact. Hardware completion alone is never a scientific result.

use super::discrimination::{
    MechanismDiscrimination, MechanismDiscriminatorAction, MechanismFeatureObservation,
};
use super::discrimination_campaign::{
    execute_glioma_mechanism_discrimination_campaign, MechanismDiscriminationCampaign,
    MechanismDiscriminationCampaignError, MechanismDiscriminationCampaignExecutionFailure,
    MechanismDiscriminationCampaignExecutor, MechanismDiscriminationCampaignRequest,
};
use crate::glioma::programs::p08_instrument_robotics::execution::{
    execute_glioma_instrument_plan, InstrumentExecutionDisposition, InstrumentExecutionRequest,
    InstrumentExecutionRun, InstrumentExecutor,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// This is a concrete execution surface for the existing P05-F32 campaign capability.
pub const FEATURE_ID: &str = super::discrimination_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentBackedMechanismCampaign1@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanismInstrumentActionBinding {
    /// P05 discriminator action to execute, e.g. a named invasion-mode measurement.
    pub mechanism_action_id: String,
    /// A single-action, preflight-admitted P08 request with signed authorization.
    pub instrument_request: InstrumentExecutionRequest,
}

/// A quantitative result must point to the derived measurement artifact and separately name the
/// raw P08 object from which it was derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPhenotypeReduction {
    pub observation: MechanismFeatureObservation,
    pub source_artifact: LocalArtifactRef,
}

/// Local assay-specific feature extraction. Implementations resolve `artifact` only inside the
/// institution, apply the validated assay/QC pipeline, and return the P05 feature in its declared
/// units. Raw and derived bytes remain in the institution-local artifact store.
pub trait MechanismPhenotypeReducer {
    fn reduce_local_measurement(
        &mut self,
        action: &MechanismDiscriminatorAction,
        discrimination: &MechanismDiscrimination,
        instrument_run: &InstrumentExecutionRun,
        artifact: &LocalArtifactRef,
    ) -> Result<MechanismPhenotypeReduction, String>;
}

/// The instrument result and its P05 interpretation remain linked for downstream analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentBackedMechanismActionRun {
    pub mechanism_action_id: String,
    pub mechanism_feature_id: String,
    pub instrument_action_id: String,
    pub instrument_run: InstrumentExecutionRun,
    pub source_artifact: Option<LocalArtifactRef>,
    pub observation: Option<MechanismFeatureObservation>,
    pub observation_rejection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentBackedMechanismCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub campaign: MechanismDiscriminationCampaign,
    /// In actual execution order, including P08 runs that failed QC or were stopped safely.
    pub instrument_runs: Vec<InstrumentBackedMechanismActionRun>,
    pub simulation_only: bool,
    pub instrument_negative_evidence: Vec<String>,
    pub instrument_uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentBackedMechanismCampaignError {
    #[error("instrument action bindings are invalid: {0}")]
    InvalidBindings(String),
    #[error("mechanism campaign failed: {0}")]
    Campaign(#[from] MechanismDiscriminationCampaignError),
    #[error("instrument-backed campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument-backed campaign digest failed: {0}")]
    Digest(String),
}

fn digest_input(run: &InstrumentBackedMechanismCampaignRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "campaign": run.campaign,
        "instrument_runs": run.instrument_runs,
        "simulation_only": run.simulation_only,
        "instrument_negative_evidence": run.instrument_negative_evidence,
        "instrument_uncertainty": run.instrument_uncertainty,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InstrumentBackedMechanismCampaignRun {
    pub fn validate(&self) -> Result<(), InstrumentBackedMechanismCampaignError> {
        self.campaign.validate().map_err(|error| {
            InstrumentBackedMechanismCampaignError::InvalidOutput(error.to_string())
        })?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.simulation_only != self.campaign.simulation_only
            || !canonical(&self.instrument_negative_evidence)
            || !canonical(&self.instrument_uncertainty)
        {
            return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                "identity, execution mode, or canonical instrument evidence is inconsistent".into(),
            ));
        }

        let completed = self
            .campaign
            .completed_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let failed = self
            .campaign
            .failed_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let observations = self
            .campaign
            .observations
            .iter()
            .map(|observation| (observation.feature_id.as_str(), observation))
            .collect::<BTreeMap<_, _>>();
        let mut actions = BTreeSet::new();
        for entry in &self.instrument_runs {
            entry.instrument_run.validate().map_err(|error| {
                InstrumentBackedMechanismCampaignError::InvalidOutput(error.to_string())
            })?;
            if entry.mechanism_action_id.trim().is_empty()
                || entry.mechanism_feature_id.trim().is_empty()
                || entry.instrument_action_id.trim().is_empty()
                || !actions.insert(entry.mechanism_action_id.as_str())
                || entry.instrument_run.action_order != [entry.instrument_action_id.clone()]
            {
                return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                    "mechanism and single instrument action identities do not reconcile".into(),
                ));
            }
            let result = &entry.instrument_run.results[0];
            if entry.source_artifact != result.artifact {
                return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                    "linked source artifact does not match the P08 action result".into(),
                ));
            }
            match &entry.observation {
                Some(observation) => {
                    if !completed.contains(&entry.mechanism_action_id)
                        || observation.feature_id != entry.mechanism_feature_id
                        || !observation.artifact.local_only
                        || observation.artifact.contains_human_data
                        || observation.artifact.contains_direct_identifiers
                        || observation.artifact.validate().is_err()
                        || entry.observation_rejection_reason.is_some()
                        || observations.get(observation.feature_id.as_str()) != Some(&observation)
                    {
                        return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                            "reduced observation is not linked to its completed local instrument artifact and campaign".into(),
                        ));
                    }
                }
                None => {
                    if !failed.contains(&entry.mechanism_action_id)
                        || entry
                            .observation_rejection_reason
                            .as_deref()
                            .is_none_or(str::is_empty)
                    {
                        return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                            "unreduced instrument result is not an explicit failed campaign action"
                                .into(),
                        ));
                    }
                }
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentBackedMechanismCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentBackedMechanismCampaignError::InvalidOutput(
                "instrument-backed campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

struct P08MechanismCampaignExecutor<G, R> {
    bindings: BTreeMap<String, InstrumentExecutionRequest>,
    gateway: G,
    reducer: R,
    runs: Vec<InstrumentBackedMechanismActionRun>,
}

impl<G, R> P08MechanismCampaignExecutor<G, R> {
    fn new(
        bindings: Vec<MechanismInstrumentActionBinding>,
        gateway: G,
        reducer: R,
    ) -> Result<Self, InstrumentBackedMechanismCampaignError> {
        let mut by_action = BTreeMap::new();
        for binding in bindings {
            let instrument_action_ids = binding
                .instrument_request
                .actions
                .iter()
                .map(|action| action.action_id.as_str())
                .collect::<BTreeSet<_>>();
            if binding.mechanism_action_id.trim().is_empty()
                || binding.instrument_request.actions.len() != 1
                || binding.instrument_request.plan.action_order.len() != 1
                || !binding.instrument_request.require_artifacts
                || instrument_action_ids.len() != 1
                || instrument_action_ids
                    != binding
                        .instrument_request
                        .plan
                        .action_order
                        .iter()
                        .map(String::as_str)
                        .collect()
                || binding.instrument_request.plan.admitted_order
                    != binding.instrument_request.plan.action_order
                || !binding.instrument_request.plan.dispatch_permitted
                || binding.instrument_request.plan.disposition
                    != crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentPreflightDisposition::Admitted
            {
                return Err(InstrumentBackedMechanismCampaignError::InvalidBindings(
                    "each mechanism action needs one artifact-producing action in an admitted P08 plan".into(),
                ));
            }
            if by_action
                .insert(binding.mechanism_action_id, binding.instrument_request)
                .is_some()
            {
                return Err(InstrumentBackedMechanismCampaignError::InvalidBindings(
                    "mechanism action bindings must be unique".into(),
                ));
            }
        }
        Ok(Self {
            bindings: by_action,
            gateway,
            reducer,
            runs: Vec::new(),
        })
    }
}

impl<G: InstrumentExecutor, R: MechanismPhenotypeReducer> MechanismDiscriminationCampaignExecutor
    for P08MechanismCampaignExecutor<G, R>
{
    fn simulation_only(&self) -> bool {
        self.gateway.simulation_only()
    }

    fn execute_action(
        &mut self,
        action: &MechanismDiscriminatorAction,
        discrimination: &MechanismDiscrimination,
        _attempt: u8,
    ) -> Result<MechanismFeatureObservation, MechanismDiscriminationCampaignExecutionFailure> {
        // Consume before dispatch. A wet-lab action is never silently replayed by the P05 retry
        // loop if extraction or downstream scoring fails after material has been touched.
        let Some(request) = self.bindings.remove(&action.action_id) else {
            return Err(MechanismDiscriminationCampaignExecutionFailure {
                reason: "no unused, pre-authorized instrument binding exists for the selected mechanism action".into(),
                retryable: false,
            });
        };
        let instrument_action_id = request.actions[0].action_id.clone();
        let run = match execute_glioma_instrument_plan(&request, &mut self.gateway) {
            Ok(run) => run,
            Err(error) => {
                return Err(MechanismDiscriminationCampaignExecutionFailure {
                    reason: format!("P08 rejected or failed the bounded instrument run: {error}"),
                    retryable: false,
                });
            }
        };
        if let Err(error) = run.validate() {
            return Err(MechanismDiscriminationCampaignExecutionFailure {
                reason: format!("P08 returned an invalid instrument run: {error}"),
                retryable: false,
            });
        }
        let result_disposition = run.results[0].disposition;
        let result_artifact = run.results[0].artifact.clone();
        let acceptable_execution = matches!(
            run.disposition,
            InstrumentExecutionDisposition::Completed | InstrumentExecutionDisposition::Negative
        ) && matches!(
            result_disposition,
            InstrumentExecutionDisposition::Completed | InstrumentExecutionDisposition::Negative
        );
        let artifact = result_artifact.clone().filter(|artifact| {
            artifact.local_only
                && !artifact.contains_human_data
                && !artifact.contains_direct_identifiers
                && artifact.validate().is_ok()
        });
        if !acceptable_execution || artifact.is_none() {
            let reason = format!(
                "P08 action did not produce a complete, local, de-identified assay artifact (run: {:?}, action: {:?})",
                run.disposition, result_disposition
            );
            self.runs.push(InstrumentBackedMechanismActionRun {
                mechanism_action_id: action.action_id.clone(),
                mechanism_feature_id: action.feature_id.clone(),
                instrument_action_id,
                instrument_run: run,
                observation: None,
                source_artifact: result_artifact,
                observation_rejection_reason: Some(reason.clone()),
            });
            return Err(MechanismDiscriminationCampaignExecutionFailure {
                reason,
                retryable: false,
            });
        }
        let artifact = artifact.expect("the local artifact was checked above");
        let reduced =
            self.reducer
                .reduce_local_measurement(action, discrimination, &run, &artifact);
        let reduction = match reduced {
            Ok(reduction)
                if reduction.source_artifact == artifact
                    && reduction.observation.feature_id == action.feature_id
                    && reduction.observation.artifact.local_only
                    && !reduction.observation.artifact.contains_human_data
                    && !reduction.observation.artifact.contains_direct_identifiers
                    && reduction.observation.artifact.validate().is_ok()
                    && reduction.observation.uncertainty_milli
                        >= action.measurement_uncertainty_milli
                    && reduction.observation.uncertainty_milli > 0
                    && reduction.observation.observed_milli.unsigned_abs() <= 1_000_000 =>
            {
                reduction
            }
            Ok(_) => {
                let reason = "local phenotype reduction violated the selected feature, artifact, uncertainty, or numeric contract".to_owned();
                self.runs.push(InstrumentBackedMechanismActionRun {
                    mechanism_action_id: action.action_id.clone(),
                    mechanism_feature_id: action.feature_id.clone(),
                    instrument_action_id,
                    instrument_run: run,
                    source_artifact: Some(artifact.clone()),
                    observation: None,
                    observation_rejection_reason: Some(reason.clone()),
                });
                return Err(MechanismDiscriminationCampaignExecutionFailure {
                    reason,
                    retryable: false,
                });
            }
            Err(reason) => {
                let reason = format!("institution-local phenotype reduction failed: {reason}");
                self.runs.push(InstrumentBackedMechanismActionRun {
                    mechanism_action_id: action.action_id.clone(),
                    mechanism_feature_id: action.feature_id.clone(),
                    instrument_action_id,
                    instrument_run: run,
                    source_artifact: Some(artifact.clone()),
                    observation: None,
                    observation_rejection_reason: Some(reason.clone()),
                });
                return Err(MechanismDiscriminationCampaignExecutionFailure {
                    reason,
                    retryable: false,
                });
            }
        };
        let observation = reduction.observation;
        self.runs.push(InstrumentBackedMechanismActionRun {
            mechanism_action_id: action.action_id.clone(),
            mechanism_feature_id: action.feature_id.clone(),
            instrument_action_id,
            instrument_run: run,
            source_artifact: Some(reduction.source_artifact),
            observation: Some(observation.clone()),
            observation_rejection_reason: None,
        });
        Ok(observation)
    }
}

/// Execute the adaptive P05 campaign through one-shot, already authorized P08 actions.
///
/// P05 re-ranks the mechanisms after every accepted local assay observation. Physical operations
/// still require institution-owned signed approval and live interlocks; this coordinator cannot
/// create or widen either authority. P08 failures and reducer failures are not automatically
/// retried because a physical side effect may already have occurred.
pub fn execute_glioma_instrument_backed_mechanism_campaign<
    G: InstrumentExecutor,
    R: MechanismPhenotypeReducer,
>(
    request: &MechanismDiscriminationCampaignRequest,
    bindings: Vec<MechanismInstrumentActionBinding>,
    gateway: G,
    reducer: R,
) -> Result<InstrumentBackedMechanismCampaignRun, InstrumentBackedMechanismCampaignError> {
    let mut executor = P08MechanismCampaignExecutor::new(bindings, gateway, reducer)?;
    let campaign = execute_glioma_mechanism_discrimination_campaign(request, &mut executor)?;
    let simulation_only = executor.simulation_only();
    let instrument_runs = executor.runs;
    let negative_evidence = instrument_runs
        .iter()
        .flat_map(|entry| entry.instrument_run.negative_evidence.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let uncertainty = instrument_runs
        .iter()
        .flat_map(|entry| entry.instrument_run.uncertainty.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut output = InstrumentBackedMechanismCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        campaign,
        instrument_runs,
        simulation_only,
        instrument_negative_evidence: negative_evidence,
        instrument_uncertainty: uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-backed-mechanism-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentBackedMechanismCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::discrimination::{
        MechanismDiscriminationRequest, MechanismDiscriminatorAction, MechanismHypothesis,
        MechanismPrediction,
    };
    use crate::glioma::programs::p05_mechanism_exploration::discrimination_campaign::MechanismDiscriminationCampaignRequest;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::DryRunInstrumentExecutor;
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentAction, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentOperation, InstrumentParameter,
        InstrumentPreflightRequest,
    };
    use crate::glioma_engine::GliomaModelSystem;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({ "id": id })).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/vnd.aurora.local-assay+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn campaign_request() -> MechanismDiscriminationCampaignRequest {
        MechanismDiscriminationCampaignRequest {
            discrimination: MechanismDiscriminationRequest {
                objective: "distinguish matrix-guided from tumor-intrinsic invasion in a preclinical organoid assay".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_features: 2,
                max_mechanisms: 4,
                max_actions: 2,
                min_information_gain_milli: 10,
            },
            hypotheses: vec![
                MechanismHypothesis {
                    mechanism_id: "matrix".into(),
                    statement: "matrix-guided motility dominates".into(),
                    predictions: vec![
                        MechanismPrediction { feature_id: "collective-invasion-fraction".into(), predicted_milli: 400, uncertainty_milli: 10 },
                        MechanismPrediction { feature_id: "invasion-front-speed".into(), predicted_milli: 500, uncertainty_milli: 10 },
                    ],
                },
                MechanismHypothesis {
                    mechanism_id: "tumor-intrinsic".into(),
                    statement: "tumor-intrinsic plasticity dominates".into(),
                    predictions: vec![
                        MechanismPrediction { feature_id: "collective-invasion-fraction".into(), predicted_milli: 200, uncertainty_milli: 10 },
                        MechanismPrediction { feature_id: "invasion-front-speed".into(), predicted_milli: 100, uncertainty_milli: 10 },
                    ],
                },
            ],
            actions: vec![MechanismDiscriminatorAction {
                action_id: "measure-front-speed".into(),
                feature_id: "invasion-front-speed".into(),
                predicted_milli_by_mechanism: BTreeMap::from([
                    ("matrix".into(), 500),
                    ("tumor-intrinsic".into(), 100),
                ]),
                measurement_uncertainty_milli: 20,
                feasibility_milli: 1_000,
                cost_units: 1,
            }],
            observations: vec![
                MechanismFeatureObservation { feature_id: "invasion-front-speed".into(), observed_milli: 100, uncertainty_milli: 10, artifact: artifact("seed-speed") },
                MechanismFeatureObservation { feature_id: "collective-invasion-fraction".into(), observed_milli: 200, uncertainty_milli: 10, artifact: artifact("seed-collective") },
            ],
            budget_units: 1,
            max_rounds: 2,
            max_retries: 2,
            stop_on_qualified: true,
        }
    }

    fn preflighted_image_action() -> (InstrumentExecutionRequest, InstrumentInterlockSnapshot) {
        let interlocks = InstrumentInterlockSnapshot {
            observed_tick: 1,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 100_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100,
            calibration_sequence_index: 3,
        };
        let authorization = InstrumentAuthorization {
            authorization_id: "local-approval".into(),
            operator_id: "researcher-1".into(),
            instrument_scope: "organoid-imager".into(),
            approval_digest: hash("signed-approval"),
            issued_tick: 0,
            expires_tick: 100,
            revoked: false,
        };
        let calibration_runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("cal-{sequence_index}"),
                sequence_index,
                batch_id: format!("batch-{sequence_index}"),
                instrument_id: "organoid-imager".into(),
                metric_name: "control".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 500 + i64::from(sequence_index),
                expected_milli: 500,
                artifact: artifact(&format!("calibration-{sequence_index}")),
            })
            .collect::<Vec<_>>();
        let calibration = analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify organoid imaging controls".into(),
                instrument_id: "organoid-imager".into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "control".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 5,
                max_drift_milli: 20,
                max_slope_milli_per_tick: 10,
            },
            &calibration_runs,
        )
        .unwrap();
        let action = InstrumentAction {
            action_id: "capture-invasion-front".into(),
            instrument_id: "organoid-imager".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 1,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: false,
            output_schema: "GliomaInvasionFrontImage1@1".into(),
            parameters: Vec::<InstrumentParameter>::new(),
        };
        let objective = "distinguish matrix-guided from tumor-intrinsic invasion in a preclinical organoid assay";
        let plan = preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: objective.into(),
            instrument_id: "organoid-imager".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action.clone()],
            calibration,
            interlocks: interlocks.clone(),
            authorization: authorization.clone(),
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        })
        .unwrap();
        (
            InstrumentExecutionRequest {
                objective: objective.into(),
                plan,
                actions: vec![action],
                authorization,
                live_interlocks: interlocks.clone(),
                current_tick: 1,
                minimum_waste_capacity_milli: 100,
                max_retries: 1,
                require_artifacts: true,
            },
            interlocks,
        )
    }

    struct LocalInvasionFrontReducer;

    impl MechanismPhenotypeReducer for LocalInvasionFrontReducer {
        fn reduce_local_measurement(
            &mut self,
            action: &MechanismDiscriminatorAction,
            _discrimination: &MechanismDiscrimination,
            _instrument_run: &InstrumentExecutionRun,
            artifact: &LocalArtifactRef,
        ) -> Result<MechanismPhenotypeReduction, String> {
            let derived_artifact = LocalArtifactRef {
                artifact_id: "local-morphodynamic-feature:invasion-front-speed".into(),
                content_hash: hash("local-morphodynamic-feature"),
                content_type: "application/vnd.aurora.glioma.morphodynamic-feature+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            };
            Ok(MechanismPhenotypeReduction {
                observation: MechanismFeatureObservation {
                    feature_id: action.feature_id.clone(),
                    observed_milli: 300,
                    uncertainty_milli: action.measurement_uncertainty_milli,
                    artifact: derived_artifact,
                },
                source_artifact: artifact.clone(),
            })
        }
    }

    #[test]
    fn campaign_executes_one_preapproved_p08_action_then_reduces_local_feature() {
        let request = campaign_request();
        let (instrument_request, interlocks) = preflighted_image_action();
        let output = execute_glioma_instrument_backed_mechanism_campaign(
            &request,
            vec![MechanismInstrumentActionBinding {
                mechanism_action_id: "measure-front-speed".into(),
                instrument_request,
            }],
            DryRunInstrumentExecutor {
                interlocks,
                emergency_stop_called: false,
            },
            LocalInvasionFrontReducer,
        )
        .unwrap();

        assert!(output.simulation_only);
        assert!(output.campaign.simulation_only);
        assert_eq!(output.instrument_runs.len(), 1);
        assert_eq!(
            output.instrument_runs[0].instrument_run.completed_order,
            vec!["capture-invasion-front"]
        );
        assert_eq!(
            output.instrument_runs[0]
                .observation
                .as_ref()
                .unwrap()
                .feature_id,
            "invasion-front-speed"
        );
        assert_eq!(
            output
                .campaign
                .observations
                .iter()
                .find(|observation| observation.feature_id == "invasion-front-speed")
                .unwrap()
                .observed_milli,
            300
        );
        assert!(output.instrument_runs[0]
            .observation
            .as_ref()
            .unwrap()
            .artifact
            .artifact_id
            .starts_with("local-morphodynamic-feature:"));
        assert!(output.instrument_runs[0]
            .source_artifact
            .as_ref()
            .unwrap()
            .artifact_id
            .starts_with("dry-run-instrument:"));
        assert!(output
            .campaign
            .negative_evidence
            .iter()
            .any(|item| item.contains("not-biological-evidence")));
        output.validate().unwrap();
    }

    struct UnsafeArtifactReducer;

    impl MechanismPhenotypeReducer for UnsafeArtifactReducer {
        fn reduce_local_measurement(
            &mut self,
            action: &MechanismDiscriminatorAction,
            _discrimination: &MechanismDiscrimination,
            _instrument_run: &InstrumentExecutionRun,
            artifact: &LocalArtifactRef,
        ) -> Result<MechanismPhenotypeReduction, String> {
            let mut unsafe_artifact = artifact.clone();
            unsafe_artifact.contains_human_data = true;
            Ok(MechanismPhenotypeReduction {
                observation: MechanismFeatureObservation {
                    feature_id: action.feature_id.clone(),
                    observed_milli: 300,
                    uncertainty_milli: action.measurement_uncertainty_milli,
                    artifact: unsafe_artifact,
                },
                source_artifact: artifact.clone(),
            })
        }
    }

    #[test]
    fn rejected_artifact_is_not_scored_or_automatically_replayed() {
        let request = campaign_request();
        let (instrument_request, interlocks) = preflighted_image_action();
        let output = execute_glioma_instrument_backed_mechanism_campaign(
            &request,
            vec![MechanismInstrumentActionBinding {
                mechanism_action_id: "measure-front-speed".into(),
                instrument_request,
            }],
            DryRunInstrumentExecutor {
                interlocks,
                emergency_stop_called: false,
            },
            UnsafeArtifactReducer,
        )
        .unwrap();

        assert_eq!(
            output.campaign.failed_action_order,
            vec!["measure-front-speed"]
        );
        assert_eq!(output.campaign.retry_count, 0);
        assert_eq!(output.campaign.observations.len(), 2);
        assert!(output.instrument_runs[0].observation.is_none());
        assert!(output.instrument_runs[0]
            .observation_rejection_reason
            .as_deref()
            .unwrap()
            .contains("contract"));
        output.validate().unwrap();
    }

    #[test]
    fn binding_rejects_multi_action_plan_before_any_execution() {
        let (mut instrument_request, _) = preflighted_image_action();
        let mut second_action = instrument_request.actions[0].clone();
        second_action.action_id = "extra-action".into();
        instrument_request.actions.push(second_action);
        let error = P08MechanismCampaignExecutor::new(
            vec![MechanismInstrumentActionBinding {
                mechanism_action_id: "measure-front-speed".into(),
                instrument_request,
            }],
            DryRunInstrumentExecutor {
                interlocks: InstrumentInterlockSnapshot {
                    observed_tick: 1,
                    emergency_stop_clear: true,
                    guard_closed: true,
                    deck_clear: true,
                    consumables_available: true,
                    waste_capacity_milli: 100_000,
                    temperature_milli: Some(37_000),
                    minimum_temperature_milli: Some(36_000),
                    maximum_temperature_milli: Some(38_000),
                    calibration_valid_until_tick: 100,
                    calibration_sequence_index: 3,
                },
                emergency_stop_called: false,
            },
            LocalInvasionFrontReducer,
        );
        assert!(matches!(
            error,
            Err(InstrumentBackedMechanismCampaignError::InvalidBindings(_))
        ));
    }
}
