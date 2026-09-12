//! Evidence-to-typed-knowledge compilation vertical.
//!
//! This is the first executable slice of the atlas' twelve production verticals.  It implements
//! `AFA-bioir-P02-F01`: a local retrieval-and-synthesis capability for one preclinical study.  It
//! deliberately compiles *references* and typed metadata, not uncontrolled source bytes.  The
//! output is therefore safe to replay and federate as an envelope while protected material stays
//! institution-local.

use crate::evidence::{EvidenceLedger, EvidenceObject, Modality, Stance};
use bioprism_foundation::{
    CapabilityManifest, DecisionImpact, EvidenceAvailability, EvidenceReceipt, EvidenceSource,
    EvidenceState, Omission, PolicyDecision, PolicyReceipt, ProvenanceLink, ResearchContractError,
    SemanticLoss, TypedResearchArtifact, UncertaintyStatement, PRECLINICAL_BOUNDARY,
};
use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

/// Atlas feature implemented by this module.
pub const FEATURE_ID: &str = "AFA-bioir-P02-F01";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub query_id: String,
    pub intent: String,
    pub decision_time: Timestamp,
    /// Empty means every evidence object in the local ledger.
    #[serde(default)]
    pub selected_evidence: BTreeSet<String>,
    /// Labels the caller is permitted to inspect.  Protected evidence may still be named as a
    /// local-only omission, but its bytes and uncontrolled metadata never leave the ledger.
    #[serde(default)]
    pub permitted_labels: BTreeSet<String>,
    pub max_sources: usize,
}

impl ScopedRetrievalQuery {
    pub fn validate(&self) -> Result<(), KnowledgeError> {
        if self.query_id.trim().is_empty() || self.intent.trim().is_empty() {
            return Err(KnowledgeError::InvalidQuery(
                "query_id and intent are required".into(),
            ));
        }
        if self.max_sources == 0 {
            return Err(KnowledgeError::InvalidQuery(
                "max_sources must be positive".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReferenceView {
    pub evidence_id: String,
    pub artifact_hash: ContentHash,
    pub modality: Modality,
    pub content_type: String,
    pub locator: String,
    pub labels: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis {
    pub feature_id: String,
    pub query_id: String,
    pub artifact: TypedResearchArtifact,
    pub receipt: EvidenceReceipt,
    pub policy: PolicyReceipt,
}

impl EvidenceSynthesis {
    pub fn verify(&self, payload: &Value) -> Result<(), KnowledgeError> {
        self.receipt.validate().map_err(KnowledgeError::Contract)?;
        self.policy.validate().map_err(KnowledgeError::Contract)?;
        self.artifact
            .verify_payload(payload)
            .map_err(KnowledgeError::Contract)
    }

    /// Returns a digest for the complete, validated synthesis envelope.
    ///
    /// The artifact hash alone identifies the protected-payload projection.  This envelope
    /// digest additionally binds the receipt's omissions/uncertainty and the policy decision,
    /// giving replay and federation consumers one identity for the exact research decision
    /// surface without exporting raw evidence bytes.
    pub fn digest(&self, payload: &Value) -> Result<ContentHash, KnowledgeError> {
        self.verify(payload)?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("invalid retrieval query: {0}")]
    InvalidQuery(String),
    #[error("ledger evidence error: {0}")]
    Evidence(#[from] crate::error::EvidenceError),
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("cannot serialize retrieval payload: {0}")]
    Serialization(String),
}

/// Deterministic, local-first retrieval and synthesis compiler.
#[derive(Debug, Clone, Copy, Default)]
pub struct KnowledgeCompiler;

impl KnowledgeCompiler {
    pub fn manifest() -> CapabilityManifest {
        CapabilityManifest {
            schema_version: bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            capability_id: FEATURE_ID.into(),
            version: FEATURE_CONTRACT_VERSION.into(),
            owner_crate: "bioir".into(),
            consumers: ["imaging core scientist".into(), "bioinformatician".into()].into(),
            behavior: "retrieves a bounded local evidence corpus and emits a typed synthesis with omissions, uncertainty, contradictions and replay hashes".into(),
            value: "makes evidence discovery auditable without moving protected source bytes or turning absence into confidence".into(),
            inputs: vec![
                bioprism_foundation::TypedPort { name: "query".into(), schema: "ScopedRetrievalQuery@1".into(), required: true },
                bioprism_foundation::TypedPort { name: "evidence_ledger".into(), schema: "EvidenceLedger@1".into(), required: true },
            ],
            outputs: vec![
                bioprism_foundation::TypedPort { name: "synthesis".into(), schema: "EvidenceSynthesis@1".into(), required: true },
                bioprism_foundation::TypedPort { name: "policy_receipt".into(), schema: "PolicyReceipt@1".into(), required: true },
            ],
            effects: [bioprism_foundation::Effect::ReadLocalData, bioprism_foundation::Effect::WriteLocalArtifact, bioprism_foundation::Effect::ExecuteLocalComputation].into(),
            permissions: ["read:institution-local-evidence".into(), "write:local-research-artifact".into()].into(),
            determinism: bioprism_foundation::Determinism::ByteStable,
            evidence: vec![bioprism_foundation::EvidenceReference { source_id: "fixture:bioir-knowledge-compiler".into(), state: EvidenceState::Supported, locator: Some("fixtures/knowledge".into()) }],
            authority_requirements: Vec::new(),
            autonomy_tier: bioprism_foundation::AutonomyTier::A0,
            surfaces: [bioprism_foundation::ResearchSurface::Cli, bioprism_foundation::ResearchSurface::Api, bioprism_foundation::ResearchSurface::Sdk].into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    pub fn compile(
        &self,
        ledger: &EvidenceLedger,
        query: &ScopedRetrievalQuery,
    ) -> Result<EvidenceSynthesis, KnowledgeError> {
        query.validate()?;
        let mut sources = Vec::new();
        let mut references = Vec::new();
        let mut omissions = Vec::new();
        let mut uncertainty = vec![UncertaintyStatement {
            kind: "epistemic".into(),
            statement:
                "retrieval is bounded by the local ledger, selection scope and decision-time cutoff"
                    .into(),
        }];
        let mut competing = Vec::new();
        let mut negative = Vec::new();
        let mut provenance = Vec::new();

        for object in ledger.iter() {
            let id = object.id.to_string();
            if !query.selected_evidence.is_empty() && !query.selected_evidence.contains(&id) {
                omissions.push(Omission {
                    item: id,
                    reason: "outside query selection".into(),
                    could_change_decision: DecisionImpact::Unknown,
                });
                continue;
            }
            if sources.len() >= query.max_sources {
                omissions.push(Omission {
                    item: id,
                    reason: "bounded by max_sources budget".into(),
                    could_change_decision: DecisionImpact::Unknown,
                });
                continue;
            }
            if object.is_stale_at(query.decision_time) {
                omissions.push(Omission {
                    item: id.clone(),
                    reason: "stale at decision time".into(),
                    could_change_decision: DecisionImpact::PotentiallyMaterial,
                });
                uncertainty.push(UncertaintyStatement {
                    kind: "temporal".into(),
                    statement: format!("{id} was not valid at the decision cutoff"),
                });
                continue;
            }
            let labels = ledger.effective_access_labels(&object.id)?;
            if !labels.is_subset(&query.permitted_labels) {
                omissions.push(Omission {
                    item: id,
                    reason: "protected by local access policy; reference retained, bytes withheld"
                        .into(),
                    could_change_decision: DecisionImpact::Unknown,
                });
                continue;
            }

            let view = reference_view(object, labels.clone())?;
            let digest = object.content_hash()?;
            references.push(view);
            provenance.push(ProvenanceLink {
                source_id: id.clone(),
                relation: "retrieved_from_local_ledger".into(),
                digest: digest.clone(),
            });
            let contradiction = ledger
                .relations_for(&object.id)
                .into_iter()
                .any(|relation| relation.stance == Stance::Contradicts);
            let availability = if contradiction {
                EvidenceAvailability::Contradictory
            } else {
                EvidenceAvailability::Available
            };
            sources.push(EvidenceSource {
                source_id: id.clone(),
                source_type: format_modality(object),
                locator: locator_string(object)?,
                digest: Some(digest),
                availability,
            });
            if contradiction {
                competing.push(bioprism_foundation::CompetingExplanation {
                    explanation: format!("contradictory assertion involves {id}"),
                    supporting_sources: vec![id.clone()],
                    unresolved: true,
                });
                negative.push(bioprism_foundation::NegativeEvidence {
                    source_id: id.clone(),
                    result: "contradictory relation recorded".into(),
                    interpretation: "retain as negative evidence; do not collapse disagreement"
                        .into(),
                });
            }
        }

        if sources.is_empty() {
            if omissions.is_empty() {
                omissions.push(Omission {
                    item: format!("query:{}", query.query_id),
                    reason: "no admissible evidence was present in the local ledger".into(),
                    could_change_decision: DecisionImpact::Unknown,
                });
            }
            uncertainty.push(UncertaintyStatement {
                kind: "epistemic".into(),
                statement: "no admissible evidence was selected at the decision cutoff".into(),
            });
        }
        let conclusion_state = if sources.is_empty() {
            EvidenceState::Unknown
        } else if !competing.is_empty() {
            EvidenceState::Contradicted
        } else {
            EvidenceState::Supported
        };
        let payload = json!({
            "feature_id": FEATURE_ID,
            "query_id": query.query_id,
            "intent": query.intent,
            "decision_time": query.decision_time,
            "references": references,
            "omissions": omissions,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        let artifact = TypedResearchArtifact::from_payload(
            format!("evidence-synthesis:{}", query.query_id),
            "application/vnd.aurora.evidence-synthesis+json",
            &payload,
            vec![SemanticLoss {
                field: "source_bytes".into(),
                reason: "protected/local-first references only".into(),
                severity: bioprism_foundation::LossSeverity::Bounded,
            }],
            provenance,
        )?;
        let receipt = EvidenceReceipt {
            schema_version: bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: format!("evidence-receipt:{}", query.query_id),
            intent: query.intent.clone(),
            sources,
            derivation: vec![
                format!("feature:{FEATURE_ID}"),
                "select:decision-time-cut".into(),
                "serialize:typed-references".into(),
            ],
            uncertainty,
            omissions,
            competing_explanations: competing,
            negative_evidence: negative,
            conclusion_state,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let decision = if receipt.sources.is_empty() {
            PolicyDecision::Unresolved
        } else if !receipt.omissions.is_empty() {
            PolicyDecision::LocalOnly
        } else {
            PolicyDecision::Allow
        };
        let policy = PolicyReceipt {
            schema_version: bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: format!("policy-receipt:{}", query.query_id),
            decision,
            reasons: vec![
                "local-first evidence selection completed".into(),
                if receipt.omissions.is_empty() {
                    "no omissions recorded".into()
                } else {
                    "omissions carried into receipt".into()
                },
            ],
            evaluated_artifacts: vec![artifact.content_hash.clone()],
            authority_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        receipt.validate()?;
        policy.validate()?;
        Ok(EvidenceSynthesis {
            feature_id: FEATURE_ID.into(),
            query_id: query.query_id.clone(),
            artifact,
            receipt,
            policy,
        })
    }
}

fn format_modality(object: &EvidenceObject) -> String {
    serde_json::to_value(object.modality)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}

fn locator_string(object: &EvidenceObject) -> Result<String, KnowledgeError> {
    serde_json::to_value(&object.locator)
        .and_then(|value| serde_json::to_string(&value))
        .map_err(|error| KnowledgeError::Serialization(error.to_string()))
}

fn reference_view(
    object: &EvidenceObject,
    labels: BTreeSet<String>,
) -> Result<EvidenceReferenceView, KnowledgeError> {
    Ok(EvidenceReferenceView {
        evidence_id: object.id.to_string(),
        artifact_hash: object.artifact_hash.clone(),
        modality: object.modality,
        content_type: object.content_type.clone(),
        locator: locator_string(object)?,
        labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{
        AccessPolicy, Locator, MeasurementContext, Provenance, QualityAssertion,
    };
    use crate::ids::EvidenceId;
    use bioprism_ids::ContentHash;
    use bioprism_scope::{Interval, Timestamp};

    fn evidence(id: &str, labels: &[&str]) -> EvidenceObject {
        EvidenceObject {
            id: EvidenceId::parse(id).unwrap(),
            artifact_hash: ContentHash::of_bytes(id.as_bytes()),
            locator: Locator::DocumentSpan {
                document: format!("{id}.txt"),
                start: 0,
                end: 10,
            },
            modality: Modality::Text,
            content_type: "text/plain".into(),
            bindings: Default::default(),
            context: MeasurementContext::default(),
            quality: QualityAssertion {
                grade: "screened".into(),
                asserted_by: "curator".into(),
                caveats: Default::default(),
            },
            provenance: Provenance {
                adapter: "fixture".into(),
                adapter_version: "1".into(),
                parser_version: "1".into(),
                extracted_at: Timestamp::parse("2024-01-01T00:00:00Z").unwrap(),
                source: id.into(),
            },
            validity: Interval::UNBOUNDED,
            access: AccessPolicy {
                labels: labels.iter().map(|s| (*s).into()).collect(),
                embeddable: labels.is_empty(),
            },
            derivation: None,
        }
    }

    #[test]
    fn compiler_keeps_protected_evidence_as_an_omission() {
        let mut ledger = EvidenceLedger::new();
        ledger
            .insert(evidence("paper-1", &["institution-a"]))
            .unwrap();
        let query = ScopedRetrievalQuery {
            query_id: "q-1".into(),
            intent: "retrieve preclinical evidence".into(),
            decision_time: Timestamp::parse("2025-01-01T00:00:00Z").unwrap(),
            selected_evidence: Default::default(),
            permitted_labels: Default::default(),
            max_sources: 10,
        };
        let result = KnowledgeCompiler::default()
            .compile(&ledger, &query)
            .unwrap();
        assert_eq!(result.policy.decision, PolicyDecision::Unresolved);
        assert_eq!(result.receipt.sources.len(), 0);
        assert_eq!(result.receipt.omissions.len(), 1);
        let payload = json!({"feature_id": FEATURE_ID, "query_id": "q-1", "intent": "retrieve preclinical evidence", "decision_time": query.decision_time, "references": [], "omissions": result.receipt.omissions, "boundary": PRECLINICAL_BOUNDARY});
        result.verify(&payload).unwrap();
    }

    #[test]
    fn compiler_is_deterministic_for_two_identical_ledgers() {
        let mut left = EvidenceLedger::new();
        let mut right = EvidenceLedger::new();
        left.insert(evidence("paper-1", &[])).unwrap();
        right.insert(evidence("paper-1", &[])).unwrap();
        let query = ScopedRetrievalQuery {
            query_id: "q-2".into(),
            intent: "retrieve".into(),
            decision_time: Timestamp::parse("2025-01-01T00:00:00Z").unwrap(),
            selected_evidence: Default::default(),
            permitted_labels: Default::default(),
            max_sources: 10,
        };
        let a = KnowledgeCompiler::default().compile(&left, &query).unwrap();
        let b = KnowledgeCompiler::default()
            .compile(&right, &query)
            .unwrap();
        assert_eq!(a.artifact.content_hash, b.artifact.content_hash);
    }

    #[test]
    fn synthesis_digest_binds_receipt_and_policy_to_artifact() {
        let mut ledger = EvidenceLedger::new();
        ledger
            .insert(evidence("paper-1", &["institution-a"]))
            .unwrap();
        let query = ScopedRetrievalQuery {
            query_id: "q-digest".into(),
            intent: "retrieve".into(),
            decision_time: Timestamp::parse("2025-01-01T00:00:00Z").unwrap(),
            selected_evidence: Default::default(),
            permitted_labels: Default::default(),
            max_sources: 10,
        };
        let synthesis = KnowledgeCompiler::default()
            .compile(&ledger, &query)
            .unwrap();
        let payload = json!({
            "feature_id": FEATURE_ID,
            "query_id": "q-digest",
            "intent": "retrieve",
            "decision_time": query.decision_time,
            "references": [],
            "omissions": synthesis.receipt.omissions.clone(),
            "boundary": PRECLINICAL_BOUNDARY,
        });
        let digest = synthesis.digest(&payload).unwrap();
        assert_eq!(digest, synthesis.digest(&payload).unwrap());
        let tampered = json!({"tampered": true});
        assert!(synthesis.digest(&tampered).is_err());
    }
}
