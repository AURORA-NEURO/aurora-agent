//! Bounded prospective evidence-surveillance copilot.
//!
//! Atlas feature: `AFA-lab-P01-F11`.
//!
//! This is a product boundary, not a claim generator.  It accepts a typed, institution-local
//! evidence feed; ranks candidates deterministically; admits only scoped, supported,
//! provenance-complete observations; and returns a qualified evidence-set artifact with explicit
//! unknown, contradicted, omitted, negative, policy, and budget witnesses.  No retrieval provider
//! or clinical decision is executed by this module.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState as FoundationEvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-lab-P01-F11";
pub const FEATURE_CONTRACT_VERSION: &str = "evidence-surveillance-copilot/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedItem {
    pub evidence_id: String,
    pub study_id: String,
    pub scope: String,
    pub source_type: String,
    pub relevance_milli: u16,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub state: FeedEvidenceState,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeed {
    pub feed_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub items: Vec<EvidenceFeedItem>,
    pub required_evidence_ids: Vec<String>,
    pub max_results: usize,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub tool_allow: bool,
    pub declared_tools: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub feed_id: String,
    pub workflow_id: String,
    pub disposition: CopilotDisposition,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub source_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence feed: {0}")]
    Invalid(String),
    #[error("evidence surveillance contract failed: {0}")]
    Contract(String),
}

impl QualifiedEvidenceSet {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != FEATURE_CONTRACT_VERSION
            || self.feed_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.effect_receipts.is_empty()
            || self.qualified_order.is_empty()
                && self.blocked_order.is_empty()
                && self.unknown_order.is_empty()
        {
            return Err(EvidenceSurveillanceError::Contract(
                "evidence-set identity, locality, effects, boundary, or retained state is incomplete".into(),
            ));
        }
        for values in [
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceSurveillanceError::Contract(
                    "evidence-set ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.source_order, &self.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceSurveillanceError::Contract(
                    "evidence-set digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tools:")
                && effect != "block:evidence-surveillance-release"
        }) {
            return Err(EvidenceSurveillanceError::Contract(
                "evidence-set effect is outside bounded tool invocation".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceError::Contract(error.to_string()))?;
        Ok(())
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "lab".into(),
        consumers: ["preclinical neuroscientist".into(), "research software engineer".into()]
            .into(),
        behavior: "ranks a typed prospective evidence feed and returns a qualified, omission-aware evidence set without silently substituting missing evidence".into(),
        value: "increases auditable discovery rate while preserving replay, provenance, negative results, and explicit unknown states".into(),
        inputs: vec![TypedPort {
            name: "evidence_feed".into(),
            schema: "EvidenceFeed3@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_evidence_set".into(),
            schema: "QualifiedEvidenceSet3@1".into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::ExternalDataAccess,
        ]
        .into(),
        permissions: ["invoke:declared-tools".into(), "read:institution-local-evidence".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "mcp-2025-06-18".into(),
            state: FoundationEvidenceState::Supported,
            locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "authorized evidence steward".into(),
            reason: "prospective tool invocation and evidence release require institution-local approval".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::McpTool, ResearchSurface::Sdk, ResearchSurface::Api].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn run(feed: &EvidenceFeed) -> Result<QualifiedEvidenceSet, EvidenceSurveillanceError> {
    validate_feed(feed)?;
    let mut items = feed.items.clone();
    items.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;

    for item in &items {
        let cost = item.evidence_id.len() as u64 + item.source_type.len() as u64 + 1;
        if cost > feed.budget.saturating_sub(spent) {
            blocked.insert(item.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:budget-ceiling-exceeded",
                item.evidence_id
            ));
            continue;
        }
        if item.scope != feed.scope {
            blocked.insert(item.evidence_id.clone());
            omissions.insert(format!("evidence:{}:scope-mismatch", item.evidence_id));
            continue;
        }
        match item.state {
            FeedEvidenceState::Contradicted => {
                blocked.insert(item.evidence_id.clone());
                negative.insert(format!(
                    "evidence:{}:contradicted-result-retained",
                    item.evidence_id
                ));
                continue;
            }
            FeedEvidenceState::Unknown | FeedEvidenceState::Unmeasured => {
                unknown.insert(item.evidence_id.clone());
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-qualified",
                        item.evidence_id, item.state
                    )
                    .to_ascii_lowercase(),
                );
                continue;
            }
            FeedEvidenceState::Supported => {}
        }
        if !item.omissions.is_empty() {
            unknown.insert(item.evidence_id.clone());
            omissions.extend(
                item.omissions
                    .iter()
                    .map(|value| format!("evidence:{}:{value}", item.evidence_id)),
            );
            continue;
        }
        if !item.uncertainty.is_empty() {
            unknown.insert(item.evidence_id.clone());
            uncertainty.extend(
                item.uncertainty
                    .iter()
                    .map(|value| format!("evidence:{}:{value}", item.evidence_id)),
            );
            continue;
        }
        let (Some(source_digest), Some(provenance_digest)) =
            (item.source_digest.clone(), item.provenance_digest.clone())
        else {
            unknown.insert(item.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:source-or-provenance-digest-missing",
                item.evidence_id
            ));
            continue;
        };
        if qualified.len() >= feed.max_results {
            blocked.insert(item.evidence_id.clone());
            omissions.insert(format!("evidence:{}:max-results-ceiling", item.evidence_id));
            continue;
        }
        qualified.insert(item.evidence_id.clone());
        sources.insert(source_digest);
        provenance.insert(provenance_digest);
        spent = spent.saturating_add(cost);
        if item.negative_result {
            negative.insert(format!(
                "evidence:{}:negative-result-retained",
                item.evidence_id
            ));
        }
    }

    for required in &feed.required_evidence_ids {
        if !qualified.contains(required) {
            omissions.insert(format!("evidence:{}:required-but-not-qualified", required));
        }
    }
    if !feed.tool_allow {
        blocked.insert("feed:declared-tool-approval-required".into());
        omissions.insert("feed:declared-tool-approval-required".into());
    }
    if feed.declared_tools.is_empty() {
        blocked.insert("feed:no-declared-tools".into());
        omissions.insert("feed:no-declared-tools".into());
    }
    if !feed.policy_allow {
        blocked.insert("feed:policy-denied".into());
        negative.insert("feed:policy-denied-no-tool-effect".into());
    }
    if !feed.protected_closure {
        unknown.insert("feed:protected-closure-incomplete".into());
        uncertainty.insert("feed:protected-closure-incomplete".into());
    }
    if !feed.signed_approval {
        blocked.insert("feed:signed-approval-required".into());
        omissions.insert("feed:signed-approval-required".into());
    }
    if !feed.raw_data_local {
        blocked.insert("feed:raw-data-locality-required".into());
        omissions.insert("feed:raw-data-locality-required".into());
    }

    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let source_order = sources.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let hard_block = !feed.tool_allow
        || feed.declared_tools.is_empty()
        || !feed.policy_allow
        || !feed.signed_approval
        || !feed.raw_data_local;
    let disposition = if hard_block {
        CopilotDisposition::Blocked
    } else if qualified_order.is_empty() {
        CopilotDisposition::Unknown
    } else if !blocked_order.is_empty()
        || !unknown_order.is_empty()
        || !omissions.is_empty()
        || !uncertainty.is_empty()
        || !feed.protected_closure
    {
        CopilotDisposition::Partial
    } else {
        CopilotDisposition::Qualified
    };
    let invocation_allowed = feed.tool_allow
        && feed.policy_allow
        && feed.protected_closure
        && feed.signed_approval
        && feed.raw_data_local;
    let mut effect_receipts = if invocation_allowed {
        feed.declared_tools
            .iter()
            .map(|tool| format!("invoke:declared-tools:{tool}"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if disposition != CopilotDisposition::Qualified {
        effect_receipts.push("block:evidence-surveillance-release".into());
    }
    effect_receipts.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": FEATURE_CONTRACT_VERSION,
        "feed_id": feed.feed_id,
        "workflow_id": feed.workflow_id,
        "disposition": disposition,
        "qualified_order": qualified_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "source_order": source_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": feed.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-evidence-set:{}", feed.feed_id),
        "application/vnd.aurora.qualified-evidence-set+json",
        &payload,
        Vec::new(),
        source_order
            .iter()
            .map(|digest| bioprism_foundation::ProvenanceLink {
                source_id: digest.to_string(),
                relation: "evidence-source".into(),
                digest: digest.clone(),
            })
            .collect(),
    )
    .map_err(|error| EvidenceSurveillanceError::Contract(error.to_string()))?;
    let receipt = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: FEATURE_CONTRACT_VERSION.into(),
        feed_id: feed.feed_id.clone(),
        workflow_id: feed.workflow_id.clone(),
        disposition,
        qualified_order,
        blocked_order,
        unknown_order,
        source_order,
        provenance_order,
        omissions,
        uncertainty,
        negative_evidence,
        replay_identity: feed.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_feed(feed: &EvidenceFeed) -> Result<(), EvidenceSurveillanceError> {
    if feed.feed_id.trim().is_empty()
        || feed.workflow_id.trim().is_empty()
        || feed.scope.trim().is_empty()
        || feed.items.is_empty()
        || feed.max_results == 0
        || feed.budget == 0
        || feed.boundary != PRECLINICAL_BOUNDARY
        || feed
            .required_evidence_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || feed
            .declared_tools
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(EvidenceSurveillanceError::Invalid(
            "feed identity, scope, items, closure, budget, tools, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for item in &feed.items {
        if item.evidence_id.trim().is_empty()
            || item.study_id.trim().is_empty()
            || item.scope.trim().is_empty()
            || item.source_type.trim().is_empty()
            || item.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(item.evidence_id.clone())
            || item.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || item.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(EvidenceSurveillanceError::Invalid(format!(
                "evidence item {} is invalid or duplicated",
                item.evidence_id
            )));
        }
    }
    if feed
        .required_evidence_ids
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(EvidenceSurveillanceError::Invalid(
            "required evidence closure references an unknown item".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn item(id: &str, state: FeedEvidenceState, negative_result: bool) -> EvidenceFeedItem {
        EvidenceFeedItem {
            evidence_id: id.into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            source_type: "preprint".into(),
            relevance_milli: if id.ends_with('a') { 950 } else { 800 },
            source_digest: Some(hash(&format!("source:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            state,
            negative_result,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn feed(items: Vec<EvidenceFeedItem>) -> EvidenceFeed {
        EvidenceFeed {
            feed_id: "feed:surveillance".into(),
            workflow_id: "workflow:evidence".into(),
            scope: "organoid:neural".into(),
            items,
            required_evidence_ids: vec!["evidence:a".into(), "evidence:b".into()],
            max_results: 8,
            replay_identity: hash("replay"),
            budget: 10_000,
            tool_allow: true,
            declared_tools: vec!["tool:local-index".into()],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualifies_supported_feed_and_retains_negative_results() {
        let receipt = run(&feed(vec![
            item("evidence:a", FeedEvidenceState::Supported, false),
            item("evidence:b", FeedEvidenceState::Supported, true),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, CopilotDisposition::Qualified);
        assert_eq!(receipt.qualified_order.len(), 2);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("negative-result")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|value| value == "invoke:declared-tools:tool:local-index"));
    }

    #[test]
    fn capability_manifest_exposes_typed_a2_surfaces() {
        let manifest = capability_manifest();
        assert!(manifest.validate().is_ok());
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.permissions.contains("invoke:declared-tools"));
    }

    #[test]
    fn unknown_and_unmeasured_items_remain_visible() {
        let receipt = run(&feed(vec![
            item("evidence:a", FeedEvidenceState::Supported, false),
            item("evidence:b", FeedEvidenceState::Unknown, false),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, CopilotDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"evidence:b".into()));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|value| value == "block:evidence-surveillance-release"));
    }

    #[test]
    fn contradiction_is_blocked_with_negative_evidence() {
        let receipt = run(&feed(vec![
            item("evidence:a", FeedEvidenceState::Supported, false),
            item("evidence:b", FeedEvidenceState::Contradicted, false),
        ]))
        .unwrap();
        assert!(receipt.blocked_order.contains(&"evidence:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("contradicted")));
        assert_ne!(receipt.disposition, CopilotDisposition::Qualified);
    }

    #[test]
    fn policy_and_tool_denial_blocks_release() {
        let mut feed = feed(vec![
            item("evidence:a", FeedEvidenceState::Supported, false),
            item("evidence:b", FeedEvidenceState::Supported, false),
        ]);
        feed.policy_allow = false;
        feed.tool_allow = false;
        let receipt = run(&feed).unwrap();
        assert_eq!(receipt.disposition, CopilotDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|value| value == "block:evidence-surveillance-release"));
        assert!(!receipt
            .effect_receipts
            .iter()
            .any(|value| value.starts_with("invoke:declared-tools:")));
    }

    #[test]
    fn duplicate_items_are_rejected() {
        let result = run(&feed(vec![
            item("evidence:a", FeedEvidenceState::Supported, false),
            item("evidence:a", FeedEvidenceState::Supported, false),
        ]));
        assert!(result.is_err());
    }
}
