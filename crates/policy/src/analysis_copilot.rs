//! Federated continual analysis-model portfolio copilot.
//!
//! Atlas feature: `AFA-policy-P13-F12`.
//!
//! This policy-owned agent automation ranks already-declared analysis tools and admits only
//! typed, local-first result attestations. It never runs a model, reads raw data, or upgrades an
//! unknown/contradictory claim. The output is an auditable `QualifiedAnalysisResult3`-compatible
//! receipt with an explicit bounded-tool effect.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-policy-P13-F12";
pub const CONTRACT_VERSION: &str = "policy-federated-continual-analysis-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.policy-qualified-analysis-result-3+json";
pub const MAX_CANDIDATES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate5 {
    pub analysis_id: String,
    pub tool_id: String,
    pub estimand: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed_capability: bool,
    pub policy_permitted: bool,
    pub federation_permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub uncertainty_milli: u16,
    pub baseline_delta_milli: i32,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisQuestion4 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_analysis_order: Vec<String>,
    pub required_tool_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_analysis_count: u32,
    pub minimum_tool_count: u32,
    pub max_tool_calls: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub candidates: Vec<AnalysisCandidate5>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult3 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: AnalysisDisposition,
    pub ranked_analysis_order: Vec<String>,
    pub selected_analysis_order: Vec<String>,
    pub unresolved_analysis_order: Vec<String>,
    pub blocked_analysis_order: Vec<String>,
    pub missing_analysis_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub selected_tool_order: Vec<String>,
    pub unresolved_tool_order: Vec<String>,
    pub blocked_tool_order: Vec<String>,
    pub missing_tool_order: Vec<String>,
    pub estimand_order: Vec<String>,
    pub selected_estimand_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub result_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnalysisCopilotError {
    #[error("invalid policy analysis copilot request or receipt: {0}")]
    Invalid(String),
    #[error("analysis result artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> AnalysisCopilotError {
    AnalysisCopilotError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn insert_all(target: &mut BTreeSet<String>, values: &[String]) {
    target.extend(values.iter().cloned());
}

pub fn analysis_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "policy".into(),
        consumers: ["consortium operator".into(), "biostatistician".into(), "agent SDK".into()].into(),
        behavior: "ranks declared analysis-model capabilities and compiles a federated continual qualified-analysis receipt under policy and evidence gates without executing models".into(),
        value: "lets a consortium operator obtain reproducible model-portfolio admissions while keeping raw preclinical data local and unknown evidence explicit".into(),
        inputs: vec![TypedPort { name: "analysis_question".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_analysis_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::FederationExport].into(), permissions: ["invoke:declared-tools".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "consortium operator".into(), reason: "bounded tool invocation and federation exchange require explicit operator authority".into() }], autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &AnalysisQuestion4) -> Result<(), AnalysisCopilotError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_analysis_order.is_empty()
        || request.required_tool_order.is_empty()
        || !canonical(&request.required_analysis_order)
        || !canonical(&request.required_tool_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_analysis_count == 0
        || request.minimum_tool_count == 0
        || request.max_tool_calls == 0
        || !digest_valid(&request.replay_identity)
        || !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
    {
        return Err(invalid("analysis identity, closure, policy, capacity, replay, locality, boundary, or bounds are invalid"));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.analysis_id.trim().is_empty()
            || candidate.tool_id.trim().is_empty()
            || candidate.estimand.trim().is_empty()
            || candidate.semantic_profile != request.semantic_profile
            || candidate.uncertainty_milli > 1000
            || !canonical(&candidate.omission_order)
            || !canonical(&candidate.uncertainty_order)
            || !digest_valid(&candidate.provenance_digest)
            || !digest_valid(&candidate.replay_identity)
            || !ids.insert(candidate.analysis_id.clone())
        {
            return Err(invalid(
                "analysis candidate identity, profile, uncertainty, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), AnalysisCopilotError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    let mut flattened = Vec::new();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    for part in parts {
        if !canonical(part) || part.iter().any(|item| !expected.contains(item)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flattened.extend_from_slice(part);
    }
    if flattened.len() != expected.len()
        || flattened.iter().cloned().collect::<BTreeSet<_>>() != expected
    {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl QualifiedAnalysisResult3 {
    pub fn validate(&self) -> Result<(), AnalysisCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A2
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_analysis_order.is_empty()
            || self.tool_order.is_empty()
            || self.estimand_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
            || !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.result_digest)
            || self.artifact.content_hash != self.result_digest
        {
            return Err(invalid("analysis result identity, closure, digest, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_analysis_order,
            &self.selected_analysis_order,
            &self.unresolved_analysis_order,
            &self.blocked_analysis_order,
            &self.missing_analysis_order,
            &self.tool_order,
            &self.selected_tool_order,
            &self.unresolved_tool_order,
            &self.blocked_tool_order,
            &self.missing_tool_order,
            &self.estimand_order,
            &self.selected_estimand_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("analysis receipt ordering is not canonical"));
            }
        }
        if self
            .ranked_analysis_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != self.node_universe()
        {
            return Err(invalid(
                "analysis ranking is not a complete candidate universe",
            ));
        }
        partition(
            &self.ranked_analysis_order,
            &[
                &self.selected_analysis_order,
                &self.unresolved_analysis_order,
                &self.blocked_analysis_order,
                &self.missing_analysis_order,
            ],
            "analysis",
        )?;
        partition(
            &self.tool_order,
            &[
                &self.selected_tool_order,
                &self.unresolved_tool_order,
                &self.blocked_tool_order,
                &self.missing_tool_order,
            ],
            "tool",
        )?;
        partition(
            &self.estimand_order,
            &[
                &self.selected_estimand_order,
                &self
                    .estimand_order
                    .iter()
                    .filter(|id| !self.selected_estimand_order.contains(id))
                    .cloned()
                    .collect::<Vec<_>>(),
            ],
            "estimand",
        )?;
        if self.disposition == AnalysisDisposition::Qualified
            && self.effect_receipts != [format!("invoke:declared-tool:{}", self.request_id)]
        {
            return Err(invalid("qualified analysis effect is invalid"));
        }
        if self.disposition != AnalysisDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified analysis must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| AnalysisCopilotError::Artifact(e.to_string()))
    }
    fn node_universe(&self) -> BTreeSet<String> {
        self.ranked_analysis_order.iter().cloned().collect()
    }
    pub fn digest(&self) -> Result<ContentHash, AnalysisCopilotError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| AnalysisCopilotError::Artifact(e.to_string()))?,
        )
        .map_err(|e| AnalysisCopilotError::Artifact(e.to_string()))
    }
}

pub fn qualify_analysis_question(
    request: &AnalysisQuestion4,
) -> Result<QualifiedAnalysisResult3, AnalysisCopilotError> {
    validate_request(request)?;
    let mut rows = request.candidates.clone();
    rows.sort_by(|a, b| {
        let rank = |state: EvidenceState| match state {
            EvidenceState::Proven => 0u8,
            EvidenceState::Supported => 1,
            EvidenceState::Speculative => 2,
            EvidenceState::Unknown => 3,
            EvidenceState::Contradicted => 4,
        };
        (
            rank(a.evidence_state),
            a.uncertainty_milli,
            -a.baseline_delta_milli,
            a.analysis_id.clone(),
        )
            .cmp(&(
                rank(b.evidence_state),
                b.uncertainty_milli,
                -b.baseline_delta_milli,
                b.analysis_id.clone(),
            ))
    });
    let ranked_analysis_order = rows
        .iter()
        .map(|row| row.analysis_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_analysis_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut state = BTreeMap::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut tools = request
        .required_tool_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut estimands = BTreeSet::new();
    for row in &rows {
        tools.insert(row.tool_id.clone());
        estimands.insert(row.estimand.clone());
        insert_all(&mut omissions, &row.omission_order);
        insert_all(&mut uncertainty, &row.uncertainty_order);
        if row.negative_result {
            negative.insert(row.analysis_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.analysis_id.clone());
        }
        let hard = row.revoked
            || !row.signed_capability
            || !row.policy_permitted
            || !row.federation_permitted
            || !row.raw_data_local
            || !row.aggregate_only;
        let soft = row.stale
            || row.replay_identity != request.replay_identity
            || !row.omission_order.is_empty()
            || !row.uncertainty_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Contradicted
            );
        state.insert(
            row.analysis_id.clone(),
            if hard {
                AnalysisDisposition::Blocked
            } else if soft {
                AnalysisDisposition::Unresolved
            } else {
                AnalysisDisposition::Qualified
            },
        );
    }
    let selected = ranked_analysis_order
        .iter()
        .filter(|id| state.get(*id) == Some(&AnalysisDisposition::Qualified))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved = ranked_analysis_order
        .iter()
        .filter(|id| state.get(*id) == Some(&AnalysisDisposition::Unresolved))
        .cloned()
        .collect::<Vec<_>>();
    let blocked = ranked_analysis_order
        .iter()
        .filter(|id| state.get(*id) == Some(&AnalysisDisposition::Blocked))
        .cloned()
        .collect::<Vec<_>>();
    let missing = required
        .iter()
        .filter(|id| !state.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    omissions.extend(
        missing
            .iter()
            .map(|id| format!("missing required analysis: {id}")),
    );
    let selected_tools = tools
        .iter()
        .filter(|tool| {
            rows.iter().any(|row| {
                &row.tool_id == *tool && state[&row.analysis_id] == AnalysisDisposition::Qualified
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_tools = tools
        .iter()
        .filter(|tool| {
            !selected_tools.contains(tool)
                && rows.iter().any(|row| {
                    &row.tool_id == *tool
                        && state[&row.analysis_id] == AnalysisDisposition::Unresolved
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let blocked_tools = tools
        .iter()
        .filter(|tool| {
            !selected_tools.contains(tool)
                && !unresolved_tools.contains(tool)
                && rows.iter().any(|row| {
                    &row.tool_id == *tool && state[&row.analysis_id] == AnalysisDisposition::Blocked
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_tools = tools
        .iter()
        .filter(|tool| !rows.iter().any(|row| &row.tool_id == *tool))
        .cloned()
        .collect::<Vec<_>>();
    let tool_order = tools.into_iter().collect::<Vec<_>>();
    let selected_estimands = estimands
        .iter()
        .filter(|estimand| {
            rows.iter()
                .any(|row| &row.estimand == *estimand && selected.contains(&row.analysis_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let estimand_order = estimands.into_iter().collect::<Vec<_>>();
    let missing_analysis = !missing.is_empty();
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty()
        || selected.len() < request.minimum_analysis_count as usize && unresolved.is_empty()
        || selected_tools.len() < request.minimum_tool_count as usize && unresolved_tools.is_empty()
        || selected.len() as u32 > request.max_tool_calls
        || missing_analysis
        || !missing_tools.is_empty()
        || !blocked.is_empty()
        || !blocked_tools.is_empty()
    {
        AnalysisDisposition::Blocked
    } else if !unresolved.is_empty() || !unresolved_tools.is_empty() {
        AnalysisDisposition::Unresolved
    } else {
        AnalysisDisposition::Qualified
    };
    let reasons = vec![match disposition { AnalysisDisposition::Qualified => "all analysis portfolio and policy gates passed".into(), AnalysisDisposition::Unresolved => "unknown, stale, contradictory, omitted, or uncertain evidence prevents an automated analysis admission".into(), AnalysisDisposition::Blocked => "policy, authorization, closure, capability, tool, coverage, or adversarial gates blocked analysis admission".into() }];
    let effect_receipts = if disposition == AnalysisDisposition::Qualified {
        vec![format!("invoke:declared-tool:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let provenance_values = rows
        .iter()
        .map(|row| row.provenance_digest.to_string())
        .collect::<Vec<_>>();
    let provenance_digest = ContentHash::of_bytes(provenance_values.join("|").as_bytes());
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "researcher": request.researcher, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "disposition": disposition, "ranked_analysis_order": ranked_analysis_order, "selected_analysis_order": selected, "unresolved_analysis_order": unresolved, "blocked_analysis_order": blocked, "missing_analysis_order": missing, "tool_order": tool_order, "selected_tool_order": selected_tools, "unresolved_tool_order": unresolved_tools, "blocked_tool_order": blocked_tools, "missing_tool_order": missing_tools, "estimand_order": estimand_order, "selected_estimand_order": selected_estimands, "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "contradiction_order": contradiction, "adversarial_event_order": request.adversarial_event_order, "replay_identity": request.replay_identity, "provenance_digest": provenance_digest, "reasons": reasons, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "autonomy_tier": AutonomyTier::A2, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-analysis-result:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| AnalysisCopilotError::Artifact(e.to_string()))?;
    let result_digest = artifact.content_hash.clone();
    let receipt = QualifiedAnalysisResult3 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        ranked_analysis_order: serde_json::from_value(payload["ranked_analysis_order"].clone())
            .unwrap(),
        selected_analysis_order: serde_json::from_value(payload["selected_analysis_order"].clone())
            .unwrap(),
        unresolved_analysis_order: serde_json::from_value(
            payload["unresolved_analysis_order"].clone(),
        )
        .unwrap(),
        blocked_analysis_order: serde_json::from_value(payload["blocked_analysis_order"].clone())
            .unwrap(),
        missing_analysis_order: serde_json::from_value(payload["missing_analysis_order"].clone())
            .unwrap(),
        tool_order: serde_json::from_value(payload["tool_order"].clone()).unwrap(),
        selected_tool_order: serde_json::from_value(payload["selected_tool_order"].clone())
            .unwrap(),
        unresolved_tool_order: serde_json::from_value(payload["unresolved_tool_order"].clone())
            .unwrap(),
        blocked_tool_order: serde_json::from_value(payload["blocked_tool_order"].clone()).unwrap(),
        missing_tool_order: serde_json::from_value(payload["missing_tool_order"].clone()).unwrap(),
        estimand_order: serde_json::from_value(payload["estimand_order"].clone()).unwrap(),
        selected_estimand_order: serde_json::from_value(payload["selected_estimand_order"].clone())
            .unwrap(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest,
        reasons,
        result_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        autonomy_tier: AutonomyTier::A2,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }
    fn candidate(id: &str, state: EvidenceState) -> AnalysisCandidate5 {
        AnalysisCandidate5 {
            analysis_id: id.into(),
            tool_id: format!("tool:{id}"),
            estimand: "effect".into(),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            provenance_digest: hash(id),
            replay_identity: hash("replay"),
            signed_capability: true,
            policy_permitted: true,
            federation_permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            uncertainty_milli: 50,
            baseline_delta_milli: 100,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn request(candidates: Vec<AnalysisCandidate5>) -> AnalysisQuestion4 {
        AnalysisQuestion4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "analysis:1".into(),
            researcher: "consortium operator".into(),
            purpose: "model portfolio".into(),
            semantic_profile: "imaging-omics".into(),
            required_analysis_order: vec!["analysis:1".into()],
            required_tool_order: vec!["tool:analysis:1".into()],
            replay_identity: hash("replay"),
            minimum_analysis_count: 1,
            minimum_tool_count: 1,
            max_tool_calls: 4,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates,
        }
    }
    #[test]
    fn qualified_analysis_is_deterministic() {
        let receipt = qualify_analysis_question(&request(vec![candidate(
            "analysis:1",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Qualified);
        assert_eq!(receipt.effect_receipts.len(), 1);
    }
    #[test]
    fn unknown_analysis_is_unresolved() {
        let receipt = qualify_analysis_question(&request(vec![candidate(
            "analysis:1",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Unresolved);
    }
    #[test]
    fn revoked_capability_blocks() {
        let mut item = candidate("analysis:1", EvidenceState::Supported);
        item.revoked = true;
        let receipt = qualify_analysis_question(&request(vec![item])).unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Blocked);
    }
    #[test]
    fn negative_result_is_retained() {
        let mut item = candidate("analysis:1", EvidenceState::Supported);
        item.negative_result = true;
        let receipt = qualify_analysis_question(&request(vec![item])).unwrap();
        assert_eq!(receipt.negative_evidence_order, vec!["analysis:1"]);
    }
    #[test]
    fn adversarial_event_blocks() {
        let mut q = request(vec![candidate("analysis:1", EvidenceState::Supported)]);
        q.adversarial_event_order = vec!["poisoned-artifact".into()];
        let receipt = qualify_analysis_question(&q).unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Blocked);
    }
    #[test]
    fn manifest_is_valid() {
        analysis_copilot_manifest().validate().unwrap();
    }
}
