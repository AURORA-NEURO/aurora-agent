//! Evidence-gated orchestration for autonomous preclinical glioma research.
//!
//! The director can compile and execute a dependency-safe stage graph, but execution should not
//! begin merely because a workflow is structurally ready. This feature joins the P01 scientific
//! triangulation result to P07 direction: qualified claims can admit a local research batch;
//! partial, negative, contradictory, incomplete, or source-dominant claims produce a typed hold
//! with the exact next evidence actions. It never turns a dry run into biology and never makes a
//! clinical decision.

use super::action_execution::GliomaActionExecutor;
use super::director::{
    execute_glioma_research_director, GliomaResearchDirectorError, GliomaResearchDirectorRequest,
    GliomaResearchDirectorRun,
};
use crate::glioma::programs::p01_evidence_surveillance::{
    EvidenceTriangulation, EvidenceTriangulationDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceGatedResearch1@1";
pub const MAX_QUALIFIED_CLAIMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaEvidenceGatedResearchRequest {
    pub director: GliomaResearchDirectorRequest,
    pub triangulation: EvidenceTriangulation,
    pub min_qualified_claims: usize,
    pub require_global_qualification: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGatedResearchDisposition {
    EvidenceHold,
    ResearchPlanned,
    ResearchExecuted,
    ResearchBlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaEvidenceGatedResearchRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub triangulation_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub qualified_claim_order: Vec<String>,
    pub blocked_claim_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub hold_reason_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub director: Option<GliomaResearchDirectorRun>,
    pub execution_started: bool,
    pub disposition: EvidenceGatedResearchDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaEvidenceGatedResearchError {
    #[error("evidence-gated research request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence-gated research output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence-gated research director failed: {0}")]
    Director(String),
    #[error("evidence-gated research digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaEvidenceGatedResearchRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "triangulation_digest": output.triangulation_digest,
        "claim_order": output.claim_order,
        "qualified_claim_order": output.qualified_claim_order,
        "blocked_claim_order": output.blocked_claim_order,
        "next_action_order": output.next_action_order,
        "hold_reason_order": output.hold_reason_order,
        "uncertainty": output.uncertainty,
        "director": output.director,
        "execution_started": output.execution_started,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &GliomaEvidenceGatedResearchRequest,
) -> Result<(), GliomaEvidenceGatedResearchError> {
    if request.director.intent.objective.trim().is_empty()
        || request.triangulation.objective.trim().is_empty()
        || request.min_qualified_claims == 0
        || request.min_qualified_claims > MAX_QUALIFIED_CLAIMS
        || request.director.budget_units == 0
        || request.director.max_actions == 0
    {
        return Err(GliomaEvidenceGatedResearchError::InvalidRequest(
            "director objective, triangulation objective, positive evidence floor, budget, and action bound are required".into(),
        ));
    }
    request
        .triangulation
        .validate()
        .map_err(|error| GliomaEvidenceGatedResearchError::InvalidRequest(error.to_string()))
}

impl GliomaEvidenceGatedResearchRun {
    pub fn validate(&self) -> Result<(), GliomaEvidenceGatedResearchError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.qualified_claim_order)
            || !canonical(&self.blocked_claim_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.hold_reason_order)
            || !canonical(&self.uncertainty)
            || self.qualified_claim_order.iter().any(|id| {
                self.blocked_claim_order.binary_search(id).is_ok()
                    || self.claim_order.binary_search(id).is_err()
            })
            || self
                .blocked_claim_order
                .iter()
                .any(|id| self.claim_order.binary_search(id).is_err())
            || self.claim_order.len()
                != self.qualified_claim_order.len() + self.blocked_claim_order.len()
            || self.execution_started
                != self
                    .director
                    .as_ref()
                    .and_then(|director| director.execution.as_ref())
                    .is_some()
            || matches!(
                self.disposition,
                EvidenceGatedResearchDisposition::EvidenceHold
            ) && self.director.is_some()
            || matches!(
                self.disposition,
                EvidenceGatedResearchDisposition::ResearchPlanned
            ) && self.director.is_none()
            || matches!(
                self.disposition,
                EvidenceGatedResearchDisposition::ResearchExecuted
            ) && (!self.execution_started || self.director.is_none())
            || matches!(
                self.disposition,
                EvidenceGatedResearchDisposition::ResearchBlocked
            ) && self.director.is_none()
        {
            return Err(GliomaEvidenceGatedResearchError::InvalidOutput(
                "identity, claim partition, execution binding, or disposition invariants are invalid".into(),
            ));
        }
        if let Some(director) = &self.director {
            director.validate().map_err(|error| {
                GliomaEvidenceGatedResearchError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaEvidenceGatedResearchError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaEvidenceGatedResearchError::InvalidOutput(
                "evidence-gated research digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Gate a director execution on independently triangulated, scoped preclinical evidence.
pub fn execute_glioma_evidence_gated_research<E: GliomaActionExecutor>(
    request: &GliomaEvidenceGatedResearchRequest,
    executor: &mut E,
) -> Result<GliomaEvidenceGatedResearchRun, GliomaEvidenceGatedResearchError> {
    validate_request(request)?;
    let triangulation = &request.triangulation;
    let claim_order = triangulation.claim_order.clone();
    let qualified_claim_order = triangulation.qualified_order.clone();
    let blocked_claim_order = claim_order
        .iter()
        .filter(|claim_id| !qualified_claim_order.contains(claim_id))
        .cloned()
        .collect::<Vec<_>>();
    let globally_qualified =
        triangulation.disposition == EvidenceTriangulationDisposition::Qualified;
    let enough_qualified = qualified_claim_order.len() >= request.min_qualified_claims;
    let evidence_admitted = enough_qualified
        && (!request.require_global_qualification || globally_qualified)
        && (!request.require_global_qualification || blocked_claim_order.is_empty());

    let (director, next_action_order, hold_reason_order, disposition, execution_started) =
        if !evidence_admitted {
            let mut hold_reasons = BTreeSet::new();
            hold_reasons.insert(format!(
                "triangulation-disposition:{:?}",
                triangulation.disposition
            ));
            if !enough_qualified {
                hold_reasons.insert(format!(
                    "qualified-claim-floor:{}<{}",
                    qualified_claim_order.len(),
                    request.min_qualified_claims
                ));
            }
            if request.require_global_qualification && !blocked_claim_order.is_empty() {
                hold_reasons.insert("global-qualification-required".into());
            }
            let mut actions = triangulation.next_action_order.clone();
            if actions.is_empty() {
                actions = blocked_claim_order
                    .iter()
                    .map(|claim_id| format!("resolve-claim:{claim_id}"))
                    .collect();
            }
            (
                None,
                actions,
                hold_reasons.into_iter().collect(),
                EvidenceGatedResearchDisposition::EvidenceHold,
                false,
            )
        } else {
            let director = execute_glioma_research_director(&request.director, executor).map_err(
                |error: GliomaResearchDirectorError| {
                    GliomaEvidenceGatedResearchError::Director(error.to_string())
                },
            )?;
            let execution_started = director.execution.is_some();
            let disposition = if matches!(
                director.disposition,
                super::director::GliomaDirectorDisposition::Blocked
                    | super::director::GliomaDirectorDisposition::NoRunnableActions
            ) {
                EvidenceGatedResearchDisposition::ResearchBlocked
            } else if execution_started {
                EvidenceGatedResearchDisposition::ResearchExecuted
            } else {
                EvidenceGatedResearchDisposition::ResearchPlanned
            };
            let mut actions = director.next_stage_order.clone();
            actions.extend(director.hold_order.iter().cloned());
            actions.extend(director.approval_order.iter().cloned());
            actions.extend(director.blocked_order.iter().cloned());
            actions.sort();
            actions.dedup();
            let mut reasons = director.blocked_order.clone();
            reasons.extend(director.approval_order.clone());
            reasons.sort();
            reasons.dedup();
            (
                Some(director),
                actions,
                reasons,
                disposition,
                execution_started,
            )
        };

    let mut output = GliomaEvidenceGatedResearchRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.director.intent.objective.clone(),
        triangulation_digest: triangulation.digest.clone(),
        claim_order,
        qualified_claim_order,
        blocked_claim_order,
        next_action_order,
        hold_reason_order,
        uncertainty: triangulation.uncertainty.clone(),
        director,
        execution_started,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-gated-research"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaEvidenceGatedResearchError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p01_evidence_surveillance::triangulate_glioma_evidence;
    use crate::glioma::programs::p01_evidence_surveillance::EvidenceTriangulationRequest;
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaResearchIntent, GliomaSelectionWeights,
        LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_ids::ContentHash;
    use bioprism_onco::OutputUse;
    use std::collections::BTreeSet;

    fn evidence_request() -> EvidenceTriangulationRequest {
        EvidenceTriangulationRequest {
            objective: "triangulate invasion evidence".into(),
            min_source_kinds: 3,
            min_independent_artifacts: 3,
            min_support_milli: 600,
            max_contradiction_milli: 200,
            min_diversity_milli: 1_000,
            max_leave_one_artifact_shift_milli: 100,
            max_claims: 8,
        }
    }

    fn evidence(
        state: EvidenceState,
        id: &str,
        kind: EvidenceSourceKind,
    ) -> crate::glioma::evidence::EvidenceRecord {
        crate::glioma::evidence::EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: kind,
            claim: "EGFR signaling increases organoid invasion".into(),
            scope: "organoid:invasion".into(),
            modality: GliomaModality::FunctionalPerturbation,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn director_request() -> GliomaResearchDirectorRequest {
        let hash = ContentHash::of_bytes(b"gated-input");
        GliomaResearchDirectorRequest {
            intent: GliomaResearchIntent {
                research_id: "gated-research".into(),
                study_id: "gated-study".into(),
                objective: "identify reproducible invasion mechanisms in organoids".into(),
                output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
                input_artifacts: vec![LocalArtifactRef {
                    artifact_id: "gated-input".into(),
                    content_hash: hash.clone(),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                requested_autonomy: AutonomyTier::A1,
                approval_reference: None,
                budget_units: 80,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: hash,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            focus: super::super::director::GliomaDirectorFocus::MechanismFirst,
            completed_checkpoints: Vec::new(),
            budget_units: 80,
            max_actions: 2,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
        }
    }

    fn request(states: [EvidenceState; 3]) -> GliomaEvidenceGatedResearchRequest {
        let records = vec![
            evidence(states[0], "e1", EvidenceSourceKind::Literature),
            evidence(states[1], "e2", EvidenceSourceKind::Assay),
            evidence(states[2], "e3", EvidenceSourceKind::Replication),
        ];
        let triangulation = triangulate_glioma_evidence(&evidence_request(), &records).unwrap();
        GliomaEvidenceGatedResearchRequest {
            director: director_request(),
            triangulation,
            min_qualified_claims: 1,
            require_global_qualification: true,
        }
    }

    #[test]
    fn incomplete_evidence_holds_before_director_execution() {
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let run = execute_glioma_evidence_gated_research(
            &request([
                EvidenceState::Supported,
                EvidenceState::Supported,
                EvidenceState::Unknown,
            ]),
            &mut executor,
        )
        .unwrap();
        assert_eq!(
            run.disposition,
            EvidenceGatedResearchDisposition::EvidenceHold
        );
        assert!(run.director.is_none());
        assert!(!run.execution_started);
        assert!(!run.next_action_order.is_empty());
        run.validate().unwrap();
    }

    #[test]
    fn qualified_evidence_admits_the_existing_director() {
        let mut executor = super::super::action_execution::DryRunGliomaActionExecutor;
        let run = execute_glioma_evidence_gated_research(
            &request([
                EvidenceState::Supported,
                EvidenceState::Supported,
                EvidenceState::Supported,
            ]),
            &mut executor,
        )
        .unwrap();
        assert!(matches!(
            run.disposition,
            EvidenceGatedResearchDisposition::ResearchExecuted
                | EvidenceGatedResearchDisposition::ResearchPlanned
        ));
        assert!(run.director.is_some());
        run.validate().unwrap();
    }
}
