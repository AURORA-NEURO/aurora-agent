//! Content-addressed, conformance-certified research ingestion bundles.
//!
//! Atlas feature: `AFA-adapter-P06-F01`.
//!
//! The ordinary adapter API already refuses malformed facts and records semantic
//! loss. This module turns that result into a portable product contract: a
//! downstream workflow receives the conformance verdict, the source digest, the
//! normalized-ingestion digest, and a typed loss/provenance envelope without
//! receiving or reinterpreting protected raw bytes.

use crate::{
    conformance::{self, ConformanceReport},
    Adapter, AdapterError, Ingestion, LossSeverity as AdapterLossSeverity, SemanticLoss, Source,
};
use bioprism_foundation::{
    LossSeverity, ProvenanceLink, ResearchContractError, SemanticLoss as ContractSemanticLoss,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{to_canonical_bytes, ContentHash};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P06-F01";
pub const FEATURE_VERSION: &str = "0.1.0";
const ARTIFACT_CONTENT_TYPE: &str = "application/vnd.aurora.ingestion+json";

#[derive(Debug, Error)]
pub enum ResearchIngestionError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error(transparent)]
    Contract(#[from] ResearchContractError),
    #[error("adapter conformance failed for source {source_id}: {failures:?}")]
    ConformanceFailed {
        source_id: String,
        failures: Vec<String>,
    },
}

/// A portable certificate for one normalized local ingest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchIngestionBundle {
    pub schema_version: String,
    pub feature_id: String,
    pub source_id: String,
    pub adapter: String,
    pub adapter_version: String,
    pub source_digest: ContentHash,
    pub ingestion_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub conformance: ConformanceReport,
    /// Raw experimental bytes remain at the institution that performed the ingest.
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ResearchIngestionBundle {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.feature_id != FEATURE_ID
            || self.source_id.trim().is_empty()
            || self.adapter.trim().is_empty()
            || self.adapter_version.trim().is_empty()
        {
            return Err(ResearchContractError::MissingField { field: "source_id" });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.source_id.clone(),
            });
        }
        if !self.raw_data_local {
            return Err(ResearchContractError::LocalizationMismatch {
                envelope: self.source_id.clone(),
            });
        }
        if !self.conformance.verified() {
            return Err(ResearchContractError::IncompleteEvaluation {
                capability: self.source_id.clone(),
            });
        }
        if self.conformance.adapter != self.adapter
            || self.conformance.adapter_version != self.adapter_version
            || self.conformance.source_id != self.source_id
        {
            return Err(ResearchContractError::MissingField {
                field: "conformance identity",
            });
        }
        if self.artifact.artifact_id != format!("ingestion:{}", self.source_id)
            || self.artifact.content_type != ARTIFACT_CONTENT_TYPE
            || self.artifact.content_hash != self.ingestion_digest
            || self.artifact.provenance
                != vec![source_provenance(&self.source_id, &self.source_digest)]
        {
            return Err(ResearchContractError::MissingField {
                field: "ingestion artifact binding",
            });
        }
        self.artifact.validate_metadata()
    }

    /// Verify the bundle against the local ingestion payload before publication
    /// or federation. The payload never needs to leave the institution.
    pub fn verify_ingestion(&self, ingestion: &Ingestion) -> Result<(), ResearchIngestionError> {
        if ingestion.manifest().source_id != self.source_id {
            return Err(ResearchIngestionError::ConformanceFailed {
                source_id: self.source_id.clone(),
                failures: vec!["source id differs from bundle".into()],
            });
        }
        if ingestion.manifest().source_digest != self.source_digest {
            return Err(ResearchIngestionError::ConformanceFailed {
                source_id: self.source_id.clone(),
                failures: vec!["source digest differs from bundle".into()],
            });
        }
        let payload = ingestion.to_value()?;
        self.artifact.verify_payload(&payload)?;
        let digest = ingestion.digest()?;
        if digest != self.ingestion_digest {
            return Err(ResearchIngestionError::ConformanceFailed {
                source_id: self.source_id.clone(),
                failures: vec!["ingestion digest differs from bundle".into()],
            });
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ResearchIngestionError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            ResearchIngestionError::Contract(ResearchContractError::Serialization {
                item: self.source_id.clone(),
                message: error.to_string(),
            })
        })?;
        Ok(ContentHash::of_bytes(&to_canonical_bytes(&value).map_err(
            |error| {
                ResearchIngestionError::Contract(ResearchContractError::Serialization {
                    item: self.source_id.clone(),
                    message: error.to_string(),
                })
            },
        )?))
    }
}

/// Run the independent conformance suite and create the portable bundle.
pub fn certify_research_ingest<A: Adapter + ?Sized>(
    adapter: &A,
    source: &Source,
) -> Result<ResearchIngestionBundle, ResearchIngestionError> {
    let (conformance, ingestion) = conformance::certify(adapter, source)?;
    if !conformance.verified() {
        return Err(ResearchIngestionError::ConformanceFailed {
            source_id: source.id.clone(),
            failures: conformance
                .failures()
                .map(|failure| format!("{}: {}", failure.check, failure.detail))
                .collect(),
        });
    }
    let payload = ingestion.to_value()?;
    let ingestion_digest = ingestion.digest()?;
    let semantic_loss = contract_losses(ingestion.loss());
    let provenance = vec![source_provenance(
        &source.id,
        &ingestion.manifest().source_digest,
    )];
    let artifact = TypedResearchArtifact::from_payload(
        format!("ingestion:{}", source.id),
        ARTIFACT_CONTENT_TYPE,
        &payload,
        semantic_loss,
        provenance,
    )?;
    let bundle = ResearchIngestionBundle {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        source_id: source.id.clone(),
        adapter: conformance.adapter.clone(),
        adapter_version: conformance.adapter_version.clone(),
        source_digest: ingestion.manifest().source_digest.clone(),
        ingestion_digest,
        artifact,
        conformance,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    bundle.validate()?;
    bundle.verify_ingestion(&ingestion)?;
    Ok(bundle)
}

fn source_provenance(source_id: &str, source_digest: &ContentHash) -> ProvenanceLink {
    ProvenanceLink {
        source_id: source_id.into(),
        relation: "derived_from_source_bytes".into(),
        digest: source_digest.clone(),
    }
}

fn contract_losses(loss: &SemanticLoss) -> Vec<ContractSemanticLoss> {
    match loss {
        SemanticLoss::Unaudited { reason } => vec![ContractSemanticLoss {
            field: "source".into(),
            reason: reason.clone(),
            severity: LossSeverity::Unknown,
        }],
        SemanticLoss::Lossless { .. } => Vec::new(),
        SemanticLoss::Lossy { lost, .. } => lost
            .entries()
            .iter()
            .map(|entry| ContractSemanticLoss {
                field: entry.location.to_string(),
                reason: entry.detail.clone(),
                severity: match entry.severity {
                    AdapterLossSeverity::Advisory => LossSeverity::None,
                    AdapterLossSeverity::Degrading => LossSeverity::Bounded,
                    AdapterLossSeverity::Blocking => LossSeverity::DecisionRelevant,
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TabularAdapter, TabularProfile, VariableMapping};

    fn adapter() -> TabularAdapter {
        TabularAdapter::new(
            TabularProfile::new("research-ingest-v1")
                .scope("subject", "subject")
                .variable("age", VariableMapping::new("age")),
        )
    }

    fn source() -> Source {
        Source::bytes("study-a", b"subject,age\nS1,41\n".to_vec()).with_format("text/csv")
    }

    #[test]
    fn certification_produces_a_local_content_addressed_bundle() {
        let bundle = certify_research_ingest(&adapter(), &source()).unwrap();
        assert!(bundle.raw_data_local);
        assert_eq!(bundle.artifact.content_hash, bundle.ingestion_digest);
        assert!(bundle.conformance.verified());
        bundle.validate().unwrap();
        bundle.digest().unwrap();
    }

    #[test]
    fn the_bundle_rejects_a_tampered_local_ingestion() {
        let bundle = certify_research_ingest(&adapter(), &source()).unwrap();
        let tampered =
            Source::bytes("study-a", b"subject,age\nS1,42\n".to_vec()).with_format("text/csv");
        let (_, ingestion) = conformance::certify(&adapter(), &tampered).unwrap();
        assert!(bundle.verify_ingestion(&ingestion).is_err());
    }

    #[test]
    fn identical_certifications_have_identical_bundle_digests() {
        let left = certify_research_ingest(&adapter(), &source()).unwrap();
        let right = certify_research_ingest(&adapter(), &source()).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }

    #[test]
    fn bundle_rejects_artifact_metadata_rebinding() {
        let mut bundle = certify_research_ingest(&adapter(), &source()).unwrap();
        bundle.artifact.artifact_id = "ingestion:other-source".into();
        assert_eq!(
            bundle.validate().unwrap_err(),
            ResearchContractError::MissingField {
                field: "ingestion artifact binding"
            }
        );
    }

    #[test]
    fn bundle_rejects_conformance_for_a_different_source() {
        let mut bundle = certify_research_ingest(&adapter(), &source()).unwrap();
        bundle.conformance.source_id = "study-other".into();
        assert_eq!(
            bundle.validate().unwrap_err(),
            ResearchContractError::MissingField {
                field: "conformance identity"
            }
        );
    }
}
