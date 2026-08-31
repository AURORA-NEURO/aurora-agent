//! Federated continual context-compilation assurance.
//!
//! Atlas feature: `AFA-atlasx-P03-F28`.
//!
//! This surface admits typed, institution-local context fragments and produces a deterministic
//! compilation receipt. It does not read source bytes or compile a Decision Section itself. The
//! receipt makes context, source, replay, provenance, omission, and policy closure observable
//! before a downstream compiler is allowed to act.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlasx-P03-F28";
pub const CONTRACT_VERSION: &str = "atlasx-federated-continual-context-compilation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ContextCompilationQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "CompiledResearchContext6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlasx-compiled-research-context-6+json";
pub const MAX_FRAGMENTS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFragment5 {
    pub context_id: String,
    pub source_id: String,
    pub section_digest: ContentHash,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationQuestion4 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_context_order: Vec<String>,
    pub required_source_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_context_count: u32,
    pub minimum_source_count: u32,
    pub max_fragments: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub fragments: Vec<ContextFragment5>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCompilationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledResearchContext6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: ContextCompilationDisposition,
    pub ranked_context_order: Vec<String>,
    pub selected_context_order: Vec<String>,
    pub unresolved_context_order: Vec<String>,
    pub blocked_context_order: Vec<String>,
    pub missing_context_order: Vec<String>,
    pub source_order: Vec<String>,
    pub selected_source_order: Vec<String>,
    pub unresolved_source_order: Vec<String>,
    pub blocked_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub context_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextCompilationError {
    #[error("invalid atlasx context-compilation request or receipt: {0}")]
    Invalid(String),
    #[error("atlasx context-compilation artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ContextCompilationError {
    ContextCompilationError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), ContextCompilationError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flat = Vec::new();
    for part in parts {
        if !canonical(part) || part.iter().any(|id| !expected.contains(id)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flat.extend_from_slice(part);
    }
    if flat.len() != expected.len() || flat.iter().collect::<BTreeSet<_>>().len() != flat.len() {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl CompiledResearchContext6 {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
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
            || self.ranked_context_order.is_empty()
            || self.source_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "context identity, closure, locality, autonomy, or effects are incomplete",
            ));
        }
        for values in [
            &self.ranked_context_order,
            &self.selected_context_order,
            &self.unresolved_context_order,
            &self.blocked_context_order,
            &self.missing_context_order,
            &self.source_order,
            &self.selected_source_order,
            &self.unresolved_source_order,
            &self.blocked_source_order,
            &self.missing_source_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("context receipt ordering is not canonical"));
            }
        }
        partition(
            &self.ranked_context_order,
            &[
                &self.selected_context_order,
                &self.unresolved_context_order,
                &self.blocked_context_order,
                &self.missing_context_order,
            ],
            "context",
        )?;
        partition(
            &self.source_order,
            &[
                &self.selected_source_order,
                &self.unresolved_source_order,
                &self.blocked_source_order,
                &self.missing_source_order,
            ],
            "source",
        )?;
        if !self
            .selected_source_order
            .iter()
            .all(|id| self.source_order.contains(id))
            || !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.context_digest)
            || self.artifact.content_hash != self.context_digest
        {
            return Err(invalid(
                "context digest, source coverage, or replay identity is invalid",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("compile:local-context:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid(
                "context effect is outside the bounded compilation gate",
            ));
        }
        if self.disposition == ContextCompilationDisposition::Qualified
            && self.effect_receipts != vec![format!("compile:local-context:{}", self.request_id)]
        {
            return Err(invalid("qualified context effect is invalid"));
        }
        if self.disposition != ContextCompilationDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified context must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| ContextCompilationError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| ContextCompilationError::Artifact(e.to_string()))
            })
    }
}

pub fn context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "atlasx".into(), consumers: ["context compiler".into(), "research workbench".into(), "federation verifier".into()].into(), behavior: "qualifies institution-local context fragments and compiles a deterministic omission-aware context receipt without reading raw sources".into(), value: "prevents incomplete or unauthorized context from becoming a confident Decision Section input while preserving federated evidence boundaries".into(), inputs: vec![TypedPort { name: "context_compilation_question".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "compiled_research_context".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::FederationExport].into(), permissions: ["compile:authorized-context".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "context compiler operator".into(), reason: "context compilation can expose protected summaries and therefore requires explicit authority".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(request: &ContextCompilationQuestion4) -> Result<(), ContextCompilationError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_context_order.is_empty()
        || request.required_source_order.is_empty()
        || !canonical(&request.required_context_order)
        || !canonical(&request.required_source_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_context_count == 0
        || request.minimum_source_count == 0
        || request.max_fragments == 0
        || request.fragments.is_empty()
        || request.fragments.len() > MAX_FRAGMENTS
        || !digest_valid(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "context identity, closure, capacity, replay, boundary, or bounds are invalid",
        ));
    }
    let mut contexts = BTreeSet::new();
    for fragment in &request.fragments {
        if fragment.context_id.trim().is_empty()
            || fragment.source_id.trim().is_empty()
            || fragment.semantic_profile != request.semantic_profile
            || !digest_valid(&fragment.section_digest)
            || !digest_valid(&fragment.provenance_digest)
            || !digest_valid(&fragment.replay_identity)
            || !canonical(&fragment.omission_order)
            || !canonical(&fragment.uncertainty_order)
            || !contexts.insert(fragment.context_id.clone())
        {
            return Err(invalid(
                "context fragment identity, profile, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

fn evidence_rank(state: EvidenceState) -> u8 {
    match state {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    }
}

pub fn compile_context(
    request: &ContextCompilationQuestion4,
) -> Result<CompiledResearchContext6, ContextCompilationError> {
    validate_request(request)?;
    let mut rows = request.fragments.clone();
    rows.sort_by(|left, right| {
        (
            evidence_rank(left.evidence_state),
            left.stale,
            left.uncertainty_order.len(),
            left.context_id.as_str(),
        )
            .cmp(&(
                evidence_rank(right.evidence_state),
                right.stale,
                right.uncertainty_order.len(),
                right.context_id.as_str(),
            ))
    });
    let ranked = rows
        .iter()
        .map(|row| row.context_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_context_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    for row in &rows {
        omissions.extend(row.omission_order.iter().cloned());
        uncertainty.extend(row.uncertainty_order.iter().cloned());
        if row.negative_result {
            negative.insert(row.context_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.context_id.clone());
        }
        let hard = row.revoked
            || !row.policy_allowed
            || !row.protected_closure
            || !row.signed_approval
            || !row.federation_allowed
            || !row.raw_data_local
            || !row.aggregate_only;
        let soft = row.stale
            || row.replay_identity != request.replay_identity
            || !row.omission_order.is_empty()
            || !row.uncertainty_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.context_id.clone());
        } else if soft {
            unresolved.insert(row.context_id.clone());
        } else {
            selected.insert(row.context_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omissions.insert(format!("missing required context: {id}"));
    }
    let mut source_order = request
        .required_source_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    source_order.extend(rows.iter().map(|row| row.source_id.clone()));
    let mut selected_sources = BTreeSet::new();
    let mut unresolved_sources = BTreeSet::new();
    let mut blocked_sources = BTreeSet::new();
    for row in &rows {
        if selected.contains(&row.context_id) {
            selected_sources.insert(row.source_id.clone());
        } else if unresolved.contains(&row.context_id) {
            unresolved_sources.insert(row.source_id.clone());
        } else {
            blocked_sources.insert(row.source_id.clone());
        }
    }
    unresolved_sources = unresolved_sources
        .difference(&selected_sources)
        .cloned()
        .collect();
    blocked_sources = blocked_sources
        .difference(&selected_sources)
        .cloned()
        .collect();
    let missing_sources = source_order
        .difference(&selected_sources)
        .filter(|id| !unresolved_sources.contains(*id) && !blocked_sources.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let context_order = ranked.clone();
    let source_order = source_order.into_iter().collect::<Vec<_>>();
    let globally_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.federation_allow
        && request.raw_data_local
        && request.aggregate_only
        && request.adversarial_event_order.is_empty();
    let disposition = if !globally_open
        || !blocked.is_empty()
        || !missing.is_empty()
        || !blocked_sources.is_empty()
        || !missing_sources.is_empty()
        || selected.len() as u32 > request.max_fragments
        || selected.len() < request.minimum_context_count as usize && unresolved.is_empty()
        || selected_sources.len() < request.minimum_source_count as usize
            && unresolved_sources.is_empty()
    {
        ContextCompilationDisposition::Blocked
    } else if !unresolved.is_empty() || !unresolved_sources.is_empty() {
        ContextCompilationDisposition::Unresolved
    } else {
        ContextCompilationDisposition::Qualified
    };
    let effect_receipts = if disposition == ContextCompilationDisposition::Qualified {
        vec![format!("compile:local-context:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons = vec![match disposition { ContextCompilationDisposition::Qualified => "all context, source, policy, replay, provenance, and protected-closure gates passed".into(), ContextCompilationDisposition::Unresolved => "stale, uncertain, omitted, unknown, or replay-mismatched context remains unresolved".into(), ContextCompilationDisposition::Blocked => "policy, closure, source, coverage, authorization, contradiction, capacity, or adversarial gates blocked context compilation".into() }];
    let provenance_values = rows
        .iter()
        .map(|row| row.provenance_digest.to_string())
        .collect::<Vec<_>>();
    let provenance_digest = ContentHash::of_bytes(provenance_values.join("|").as_bytes());
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "researcher": request.researcher, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "disposition": disposition, "ranked_context_order": context_order, "selected_context_order": selected, "unresolved_context_order": unresolved, "blocked_context_order": blocked, "missing_context_order": missing, "source_order": source_order, "selected_source_order": selected_sources, "unresolved_source_order": unresolved_sources, "blocked_source_order": blocked_sources, "missing_source_order": missing_sources, "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "contradiction_order": contradiction, "adversarial_event_order": request.adversarial_event_order, "replay_identity": request.replay_identity, "provenance_digest": provenance_digest, "reasons": reasons, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "autonomy_tier": AutonomyTier::A2, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("compiled-research-context:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let context_digest = artifact.content_hash.clone();
    let receipt = CompiledResearchContext6 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        ranked_context_order: context_order,
        selected_context_order: selected.into_iter().collect(),
        unresolved_context_order: unresolved.into_iter().collect(),
        blocked_context_order: blocked.into_iter().collect(),
        missing_context_order: missing.into_iter().collect(),
        source_order,
        selected_source_order: selected_sources.into_iter().collect(),
        unresolved_source_order: unresolved_sources.into_iter().collect(),
        blocked_source_order: blocked_sources.into_iter().collect(),
        missing_source_order: missing_sources.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        replay_identity: request.replay_identity.clone(),
        provenance_digest,
        reasons,
        context_digest,
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
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn fragment(id: &str, state: EvidenceState) -> ContextFragment5 {
        ContextFragment5 {
            context_id: id.into(),
            source_id: format!("source:{id}"),
            section_digest: hash(id),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            federation_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn request(items: Vec<ContextFragment5>) -> ContextCompilationQuestion4 {
        ContextCompilationQuestion4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "context:1".into(),
            researcher: "operator".into(),
            purpose: "decision section".into(),
            semantic_profile: "imaging-omics".into(),
            required_context_order: vec!["context:1".into()],
            required_source_order: vec!["source:context:1".into()],
            replay_identity: hash("replay"),
            minimum_context_count: 1,
            minimum_source_count: 1,
            max_fragments: 8,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            fragments: items,
        }
    }
    #[test]
    fn qualified_is_deterministic() {
        let result = compile_context(&request(vec![fragment(
            "context:1",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert_eq!(result.disposition, ContextCompilationDisposition::Qualified);
        assert_eq!(result.effect_receipts.len(), 1);
    }
    #[test]
    fn unknown_is_unresolved() {
        let result = compile_context(&request(vec![fragment(
            "context:1",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(
            result.disposition,
            ContextCompilationDisposition::Unresolved
        );
    }
    #[test]
    fn contradiction_is_blocked() {
        let result = compile_context(&request(vec![fragment(
            "context:1",
            EvidenceState::Contradicted,
        )]))
        .unwrap();
        assert_eq!(result.disposition, ContextCompilationDisposition::Blocked);
    }
    #[test]
    fn missing_context_is_blocked() {
        let mut value = request(vec![fragment("context:other", EvidenceState::Supported)]);
        value.required_context_order = vec!["context:1".into()];
        let result = compile_context(&value).unwrap();
        assert_eq!(result.disposition, ContextCompilationDisposition::Blocked);
        assert_eq!(result.missing_context_order, vec!["context:1"]);
    }
    #[test]
    fn adversarial_event_blocks() {
        let mut value = request(vec![fragment("context:1", EvidenceState::Supported)]);
        value.adversarial_event_order = vec!["poisoned-artifact".into()];
        let result = compile_context(&value).unwrap();
        assert_eq!(result.disposition, ContextCompilationDisposition::Blocked);
    }
    #[test]
    fn manifest_is_valid() {
        context_compilation_assurance_manifest().validate().unwrap();
    }
}
