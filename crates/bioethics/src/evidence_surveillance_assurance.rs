//! Multimodal evidence-surveillance assurance for `AFA-bioethics-P01-F26`.
//!
//! This product boundary composes the typed retrieval/synthesis contract with the bioethics
//! release questions that must be answered before a multimodal preclinical evidence set is
//! allowed into a downstream workflow.  It records declarations and gates; it never classifies
//! biological content, performs retrieval, exports raw data, or makes a clinical decision.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{
    assure_retrieval_synthesis, ContentHash, EvidenceSynthesis11, RetrievalSynthesisAssuranceError,
    ScopedRetrievalQuery6,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P01-F26";
pub const CONTRACT_VERSION: &str =
    "bioethics-multimodal-evidence-surveillance-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-qualified-evidence-set-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsEvidenceRequest {
    pub schema_version: String,
    pub request_id: String,
    pub reviewer: String,
    pub query: ScopedRetrievalQuery6,
    pub dual_use_reviewed: bool,
    pub privacy_reviewed: bool,
    pub representation_reviewed: bool,
    pub institutional_authorized: bool,
    pub autonomy_grant_present: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BioethicsEvidenceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsEvidenceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub reviewer: String,
    pub corpus_id: String,
    pub semantic_profile: String,
    pub disposition: BioethicsEvidenceDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub source_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub ethical_gate_order: Vec<String>,
    pub synthesis_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceSurveillanceAssuranceError {
    #[error("invalid bioethics evidence request: {0}")]
    Invalid(String),
    #[error("retrieval synthesis assurance failed: {0}")]
    Retrieval(String),
    #[error("bioethics evidence artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> EvidenceSurveillanceAssuranceError {
    EvidenceSurveillanceAssuranceError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn gate_names(request: &BioethicsEvidenceRequest) -> Vec<String> {
    let mut gates = BTreeSet::new();
    if !request.dual_use_reviewed {
        gates.insert("gate:dual-use-review-missing".to_string());
    }
    if !request.privacy_reviewed {
        gates.insert("gate:privacy-review-missing".to_string());
    }
    if !request.representation_reviewed {
        gates.insert("gate:representation-review-missing".to_string());
    }
    if !request.institutional_authorized {
        gates.insert("gate:institutional-authorization-missing".to_string());
    }
    if !request.autonomy_grant_present {
        gates.insert("gate:autonomy-grant-missing".to_string());
    }
    gates.into_iter().collect()
}

impl BioethicsEvidenceReceipt {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.reviewer.trim().is_empty()
            || self.corpus_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "evidence identity, locality, candidate closure, reviewer, or effects are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.source_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.ethical_gate_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("evidence assurance ordering is not canonical"));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len()
            || parts.len() != candidates.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != candidates
        {
            return Err(invalid("candidate states do not form a complete partition"));
        }
        for value in [
            &self.synthesis_digest,
            &self.evidence_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("evidence digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.evidence_digest
        {
            return Err(invalid(
                "evidence artifact metadata or digest is inconsistent",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:bioethics-evidence:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the bioethics evidence gate"));
        }
        if self.disposition == BioethicsEvidenceDisposition::Qualified
            && self.effect_receipts != [format!("verify:bioethics-evidence:{}", self.request_id)]
        {
            return Err(invalid("qualified evidence effect is invalid"));
        }
        if self.disposition != BioethicsEvidenceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified evidence must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceSurveillanceAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| EvidenceSurveillanceAssuranceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| EvidenceSurveillanceAssuranceError::Artifact(error.to_string()))
    }
}

pub fn evidence_surveillance_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "bioethics".into(),
        consumers: [
            "research data steward".into(),
            "bioethics reviewer".into(),
            "downstream evidence workflow".into(),
        ]
        .into(),
        behavior: "verifies a multimodal preclinical evidence stream under explicit ethics, privacy, representation, provenance, policy, and replay gates without inferring biology".into(),
        value: "prevents incomplete, unauthorized, adversarial, or ethically unreviewed evidence from being presented as release-ready".into(),
        inputs: vec![TypedPort {
            name: "evidence_feed".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_evidence_set".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["evaluate:research-evidence".into(), "evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "slsa-provenance-1.2".into(),
            state: EvidenceState::Supported,
            locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
        }],
        authority_requirements: vec![
            AuthorityRequirement {
                role: "bioethics reviewer".into(),
                reason: "dual-use, privacy, and representation review".into(),
            },
            AuthorityRequirement {
                role: "institutional steward".into(),
                reason: "authorization for multimodal preclinical evidence".into(),
            },
            AuthorityRequirement {
                role: "autonomy grant issuer".into(),
                reason: "bounded A1 local evaluation".into(),
            },
        ],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_evidence_surveillance(
    request: &BioethicsEvidenceRequest,
) -> Result<BioethicsEvidenceReceipt, EvidenceSurveillanceAssuranceError> {
    validate_request(request)?;
    let synthesis = assure_retrieval_synthesis(&request.query).map_err(
        |error: RetrievalSynthesisAssuranceError| {
            EvidenceSurveillanceAssuranceError::Retrieval(error.to_string())
        },
    )?;
    build_receipt(request, synthesis)
}

fn build_receipt(
    request: &BioethicsEvidenceRequest,
    synthesis: EvidenceSynthesis11,
) -> Result<BioethicsEvidenceReceipt, EvidenceSurveillanceAssuranceError> {
    let ethical_gate_order = gate_names(request);
    let ethics_block = !ethical_gate_order.is_empty() || !request.adversarial_events.is_empty();
    let candidate_order = synthesis.candidate_order.clone();
    let mut selected = synthesis.qualified_order.clone();
    let mut unresolved = synthesis.unresolved_order.clone();
    let mut blocked = synthesis.blocked_order.clone();
    let mut omission = synthesis.omission_order.clone();
    let mut uncertainty = synthesis.uncertainty_order.clone();
    let mut negative = synthesis.negative_evidence_order.clone();
    if ethics_block {
        blocked = candidate_order.clone();
        selected.clear();
        unresolved.clear();
        omission.extend(ethical_gate_order.iter().cloned());
        omission.extend(
            request
                .adversarial_events
                .iter()
                .map(|event| format!("adversarial:{event}")),
        );
        uncertainty.extend(ethical_gate_order.iter().cloned());
        negative.extend(
            request
                .adversarial_events
                .iter()
                .map(|event| format!("adversarial:{event}")),
        );
        omission.push("release:bioethics-gate-blocked".into());
    }
    let disposition = if ethics_block || synthesis.disposition == "blocked" {
        BioethicsEvidenceDisposition::Blocked
    } else if synthesis.disposition != "qualified" {
        BioethicsEvidenceDisposition::Unresolved
    } else {
        BioethicsEvidenceDisposition::Qualified
    };
    let effect_receipts = if disposition == BioethicsEvidenceDisposition::Qualified {
        vec![format!("verify:bioethics-evidence:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let candidate_order = sorted_unique(candidate_order);
    let selected = sorted_unique(selected);
    let unresolved = sorted_unique(unresolved);
    let blocked = sorted_unique(blocked);
    let source_order = sorted_unique(synthesis.source_order);
    let omission = sorted_unique(omission);
    let uncertainty = sorted_unique(uncertainty);
    let negative = sorted_unique(negative);
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "reviewer": request.reviewer,
        "corpus_id": request.query.corpus_id,
        "semantic_profile": request.query.semantic_profile,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "source_order": source_order,
        "omission_order": omission,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "ethical_gate_order": ethical_gate_order,
        "synthesis_digest": synthesis.synthesis_digest,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.query.raw_data_local,
        "aggregate_only": request.query.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let evidence_digest = ContentHash::of_value(&payload)
        .map_err(|error| EvidenceSurveillanceAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioethics-qualified-evidence:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceSurveillanceAssuranceError::Artifact(error.to_string()))?;
    let receipt = BioethicsEvidenceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        reviewer: request.reviewer.clone(),
        corpus_id: request.query.corpus_id.clone(),
        semantic_profile: request.query.semantic_profile.clone(),
        disposition,
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        source_order: payload["source_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        ethical_gate_order: payload["ethical_gate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        synthesis_digest: synthesis.synthesis_digest,
        evidence_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.query.raw_data_local,
        aggregate_only: request.query.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn validate_request(
    request: &BioethicsEvidenceRequest,
) -> Result<(), EvidenceSurveillanceAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.reviewer.trim().is_empty()
        || request.query.request_id != request.request_id
        || request.boundary != PRECLINICAL_BOUNDARY
        || !canonical(&request.adversarial_events)
    {
        return Err(invalid(
            "request identity, reviewer, query binding, adversarial ordering, or boundary is invalid",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::{RetrievalEvidence7, RetrievalEvidenceState, RetrievalPeer6};

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> BioethicsEvidenceRequest {
        let evidence = |id: &str, relevance: i64| RetrievalEvidence7 {
            evidence_id: id.into(),
            source_id: format!("source:{id}"),
            origin: "site:one".into(),
            title: format!("Preclinical evidence {id}"),
            terms: vec!["mechanism".into(), "neuron".into()],
            relevance_milli: relevance,
            freshness_milli: 900,
            content_digest: hash(id),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            estimated_units: 5,
            evidence_state: RetrievalEvidenceState::Supported,
            signed: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            comparable: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        };
        let peer = RetrievalPeer6 {
            peer_id: "peer:one".into(),
            origin: "site:one".into(),
            corpus_id: "corpus:bioethics".into(),
            semantic_profile: "neuro:evidence:v1".into(),
            checkpoint: 2,
            synthesis_digest: hash("peer"),
            source_count: 2,
            evidence_state: RetrievalEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        };
        BioethicsEvidenceRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:bioethics".into(),
            reviewer: "steward:one".into(),
            query: ScopedRetrievalQuery6 {
                request_id: "request:bioethics".into(),
                corpus_id: "corpus:bioethics".into(),
                requester: "research-data-steward".into(),
                purpose: "preclinical-evidence-surveillance".into(),
                semantic_profile: "neuro:evidence:v1".into(),
                query_terms: vec!["mechanism".into(), "neuron".into()],
                candidates: vec![evidence("evidence:a", 900), evidence("evidence:b", 800)],
                peers: vec![peer],
                checkpoint: 2,
                minimum_relevance_milli: 600,
                minimum_freshness_milli: 500,
                minimum_peer_quorum: 1,
                max_budget_units: 100,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signed_approval: true,
                federation_approved: true,
                raw_data_local: true,
                aggregate_only: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            dual_use_reviewed: true,
            privacy_reviewed: true,
            representation_reviewed: true,
            institutional_authorized: true,
            autonomy_grant_present: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            evidence_surveillance_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }

    #[test]
    fn qualified_review_is_deterministic() {
        let receipt = assure_evidence_surveillance(&request()).unwrap();
        assert_eq!(receipt.disposition, BioethicsEvidenceDisposition::Qualified);
        assert_eq!(receipt.selected_order.len(), 2);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn missing_ethics_review_blocks_all_candidates() {
        let mut request = request();
        request.dual_use_reviewed = false;
        let receipt = assure_evidence_surveillance(&request).unwrap();
        assert_eq!(receipt.disposition, BioethicsEvidenceDisposition::Blocked);
        assert!(receipt.selected_order.is_empty());
        assert_eq!(receipt.blocked_order.len(), 2);
        assert!(receipt
            .ethical_gate_order
            .contains(&"gate:dual-use-review-missing".into()));
    }

    #[test]
    fn adversarial_event_is_negative_and_blocked() {
        let mut request = request();
        request.adversarial_events = vec!["poisoned-source".into()];
        let receipt = assure_evidence_surveillance(&request).unwrap();
        assert_eq!(receipt.disposition, BioethicsEvidenceDisposition::Blocked);
        assert!(receipt
            .negative_evidence_order
            .contains(&"adversarial:poisoned-source".into()));
    }

    #[test]
    fn underlying_quorum_gap_remains_unresolved() {
        let mut request = request();
        request.query.minimum_peer_quorum = 2;
        let receipt = assure_evidence_surveillance(&request).unwrap();
        assert_eq!(
            receipt.disposition,
            BioethicsEvidenceDisposition::Unresolved
        );
        assert!(receipt
            .uncertainty_order
            .contains(&"peer:minimum-quorum-unmet".into()));
    }

    #[test]
    fn privacy_and_authority_gates_are_explicit() {
        let mut request = request();
        request.privacy_reviewed = false;
        request.institutional_authorized = false;
        request.autonomy_grant_present = false;
        let receipt = assure_evidence_surveillance(&request).unwrap();
        assert_eq!(receipt.disposition, BioethicsEvidenceDisposition::Blocked);
        assert!(receipt
            .omission_order
            .contains(&"gate:privacy-review-missing".into()));
        assert!(receipt
            .uncertainty_order
            .contains(&"gate:autonomy-grant-missing".into()));
    }

    #[test]
    fn contradictory_underlying_evidence_is_preserved() {
        let mut request = request();
        request.query.candidates[0].evidence_state = RetrievalEvidenceState::Contradicted;
        let receipt = assure_evidence_surveillance(&request).unwrap();
        assert!(receipt
            .negative_evidence_order
            .iter()
            .any(|value| value.contains("contradicted")));
        assert!(receipt.blocked_order.contains(&"evidence:a".into()));
    }
}
