//! Deterministic, transport-neutral discovery of mission evaluator candidates.
//!
//! A mission claim can name any caller-owned `adapter_id`, but an agent still needs a useful
//! answer to the question "which evaluator contract fits this domain?" This module supplies that
//! answer without creating a semantic oracle or a second execution registry. The catalogue is a
//! bounded set of explicit, reviewable candidate contracts. Every row says what it can inspect,
//! which mission level it fits, and which existing MCP tools are plausible evidence producers;
//! none of those labels authorizes execution or establishes that a claim is true.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, Value};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Wire version for mission evaluator discovery.
pub const MISSION_EVALUATOR_SCHEMA_VERSION: &str = "bioprism-devplat-mission-evaluator/0.1";
const MAX_ITEMS: usize = 256;
const DEFAULT_MAX_ITEMS: usize = 32;
const MAX_FILTER_BYTES: usize = 512;
const MAX_REVIEW_SELECTIONS: usize = 64;
const MAX_BINDINGS_PER_CLAIM: usize = 16;

fn default_max_items() -> usize {
    DEFAULT_MAX_ITEMS
}

fn validate_filter(field: &'static str, value: &Option<String>) -> Result<(), EvaluatorError> {
    if let Some(value) = value {
        if value.trim().is_empty() {
            return Err(EvaluatorError::EmptyFilter { field });
        }
        if value.len() > MAX_FILTER_BYTES {
            return Err(EvaluatorError::FilterTooLong {
                field,
                bytes: value.len(),
                maximum: MAX_FILTER_BYTES,
            });
        }
        if value
            .chars()
            .any(|character| character == '\0' || character == '\n' || character == '\r')
        {
            return Err(EvaluatorError::ControlCharacter { field });
        }
    }
    Ok(())
}

fn tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn normalized(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn default_true() -> bool {
    true
}

fn valid_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    let bytes = pointer.as_bytes();
    if bytes.first() != Some(&b'/') {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return false;
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    true
}

fn visible_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value
            .chars()
            .any(|character| character == '\0' || character == '\n' || character == '\r')
}

/// One explicit evaluator candidate. The candidate is descriptive and non-executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorAdapter {
    /// Stable caller-visible identifier suitable for `MissionClaimEvaluatorBinding.adapter_id`.
    pub id: String,
    /// Capability group whose tools can produce the candidate's input evidence.
    pub group_id: String,
    /// Human-readable domain labels used for conjunctive discovery.
    pub domains: Vec<String>,
    /// Mission claim levels for which the candidate is a plausible review input.
    pub levels: Vec<String>,
    /// What the candidate can structurally inspect; this is not a truth assertion.
    pub purpose: String,
    /// Existing tools that can produce related evidence. They are not automatically selected.
    pub candidate_tools: Vec<String>,
    /// Example RFC 6901 pointers a caller may bind after inspecting a concrete tool result.
    pub output_pointer_examples: Vec<String>,
    /// Explicitly non-executable status of the catalogue row.
    pub status: String,
}

/// One bounded evaluator discovery query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorQuery {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
}

impl Default for MissionEvaluatorQuery {
    fn default() -> Self {
        Self {
            query: None,
            group_id: None,
            domain: None,
            level: None,
            adapter_id: None,
            max_items: DEFAULT_MAX_ITEMS,
        }
    }
}

impl MissionEvaluatorQuery {
    pub fn validate(&self) -> Result<(), EvaluatorError> {
        validate_filter("query", &self.query)?;
        validate_filter("group_id", &self.group_id)?;
        validate_filter("domain", &self.domain)?;
        validate_filter("level", &self.level)?;
        validate_filter("adapter_id", &self.adapter_id)?;
        if !(1..=MAX_ITEMS).contains(&self.max_items) {
            return Err(EvaluatorError::InvalidLimit {
                value: self.max_items,
            });
        }
        Ok(())
    }
}

/// One ranked evaluator candidate with the fields that caused the match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorMatch {
    pub adapter: MissionEvaluatorAdapter,
    pub score: u32,
    pub matched_fields: Vec<String>,
}

/// Discovery response bound to the exact evaluator catalogue digest used for ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorSearch {
    pub schema_version: String,
    pub catalog_digest: String,
    pub total_adapters: usize,
    pub query: MissionEvaluatorQuery,
    pub result_count: usize,
    pub matches: Vec<MissionEvaluatorMatch>,
}

/// One caller-proposed binding reviewed against a discovery response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorSelection {
    pub id: String,
    pub claim_id: String,
    pub adapter_id: String,
    pub domain: String,
    pub step_id: String,
    pub output_pointer: String,
    #[serde(default = "default_true")]
    pub required: bool,
}

/// Input to the non-executing discovery-to-claim binding review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorReviewRequest {
    pub discovery: Value,
    pub selections: Vec<MissionEvaluatorSelection>,
}

/// Validated evaluator catalogue with a content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionEvaluatorCatalogue {
    adapters: Vec<MissionEvaluatorAdapter>,
    digest: ContentHash,
}

impl MissionEvaluatorCatalogue {
    /// Build the in-tree catalogue covering each workspace capability group.
    pub fn standard() -> Self {
        let adapters = standard_adapters();
        let digest = ContentHash::of_value(&to_value(&adapters).expect("adapters are serializable"))
            .expect("standard evaluator catalogue must be hashable");
        Self { adapters, digest }
    }

    pub fn adapters(&self) -> &[MissionEvaluatorAdapter] {
        &self.adapters
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Rank explicit candidates. Scores are routing evidence only.
    pub fn search(
        &self,
        query: &MissionEvaluatorQuery,
    ) -> Result<MissionEvaluatorSearch, EvaluatorError> {
        query.validate()?;
        let query_tokens = query.query.as_deref().map(tokens).unwrap_or_default();
        let group_filter = query.group_id.as_deref().map(normalized);
        let domain_filter = query.domain.as_deref().map(normalized);
        let level_filter = query.level.as_deref().map(normalized);
        let adapter_filter = query.adapter_id.as_deref().map(normalized);
        let mut matches = Vec::new();

        for adapter in &self.adapters {
            let adapter_id = normalized(&adapter.id);
            if let Some(filter) = &adapter_filter {
                if adapter_id != *filter
                    && !adapter_id.starts_with(filter)
                    && !adapter_id.contains(filter)
                {
                    continue;
                }
            }
            let group_id = normalized(&adapter.group_id);
            if let Some(filter) = &group_filter {
                if group_id != *filter && !group_id.starts_with(filter) {
                    continue;
                }
            }
            if let Some(filter) = &domain_filter {
                if !adapter
                    .domains
                    .iter()
                    .any(|domain| normalized(domain).contains(filter))
                {
                    continue;
                }
            }
            if let Some(filter) = &level_filter {
                if !adapter
                    .levels
                    .iter()
                    .any(|level| normalized(level) == *filter)
                {
                    continue;
                }
            }

            let searchable = [
                ("adapter_id", adapter.id.as_str()),
                ("group_id", adapter.group_id.as_str()),
                ("domains", &adapter.domains.join(" ")),
                ("levels", &adapter.levels.join(" ")),
                ("purpose", adapter.purpose.as_str()),
                ("candidate_tools", &adapter.candidate_tools.join(" ")),
                ("output_pointer_examples", &adapter.output_pointer_examples.join(" ")),
            ];
            let mut score = 0;
            let mut matched_fields = BTreeSet::new();
            if adapter_filter.is_some() {
                score += if adapter_id == adapter_filter.as_deref().unwrap_or_default() {
                    1_000
                } else {
                    700
                };
                matched_fields.insert("adapter_id".to_string());
            }
            if group_filter.is_some() {
                score += if group_id == group_filter.as_deref().unwrap_or_default() {
                    900
                } else {
                    600
                };
                matched_fields.insert("group_id".to_string());
            }
            if domain_filter.is_some() {
                score += 500;
                matched_fields.insert("domains".to_string());
            }
            if level_filter.is_some() {
                score += 400;
                matched_fields.insert("levels".to_string());
            }
            let mut all_query_tokens_match = true;
            for query_token in &query_tokens {
                let fields_for_token = searchable
                    .iter()
                    .filter_map(|(field, value)| {
                        let field_tokens = tokens(value);
                        field_tokens.iter().any(|candidate| {
                            candidate == query_token || candidate.starts_with(query_token)
                        }).then_some(*field)
                    })
                    .collect::<Vec<_>>();
                if fields_for_token.is_empty() {
                    all_query_tokens_match = false;
                    break;
                }
                for field in fields_for_token {
                    matched_fields.insert(field.to_string());
                    score += if field == "candidate_tools" { 150 } else { 100 };
                }
            }
            if !query_tokens.is_empty() && !all_query_tokens_match {
                continue;
            }
            matches.push(MissionEvaluatorMatch {
                adapter: adapter.clone(),
                score,
                matched_fields: matched_fields.into_iter().collect(),
            });
        }

        matches.sort_by_key(|matched| (Reverse(matched.score), matched.adapter.id.clone()));
        matches.truncate(query.max_items);
        Ok(MissionEvaluatorSearch {
            schema_version: MISSION_EVALUATOR_SCHEMA_VERSION.into(),
            catalog_digest: self.digest.to_string(),
            total_adapters: self.adapters.len(),
            query: query.clone(),
            result_count: matches.len(),
            matches,
        })
    }

    /// Review caller-selected candidates and emit a mission-claim binding scaffold.
    ///
    /// The discovery response is checked against this catalogue's current digest before any
    /// selection is considered. Selection findings are returned as a blocked review rather than
    /// becoming a transport error, so an agent can repair a proposed handoff in one round trip.
    /// The returned scaffold still requires normal `agent_mission` validation, including known
    /// step IDs and the final claim statement. No evaluator or domain tool is executed here.
    pub fn review(&self, request: &MissionEvaluatorReviewRequest) -> Result<Value, EvaluatorError> {
        let discovery = request
            .discovery
            .as_object()
            .ok_or_else(|| EvaluatorError::InvalidReview {
                reason: "discovery must be an object".into(),
            })?;
        if discovery.get("workflow").and_then(Value::as_str)
            != Some("mission_evaluator_discover")
        {
            return Err(EvaluatorError::InvalidReview {
                reason: "discovery.workflow must be mission_evaluator_discover".into(),
            });
        }
        if discovery.get("selection_posture").and_then(Value::as_str)
            != Some("candidate_only")
        {
            return Err(EvaluatorError::InvalidReview {
                reason: "discovery.selection_posture must be candidate_only".into(),
            });
        }
        let catalog_digest = discovery
            .get("catalog_digest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| EvaluatorError::InvalidReview {
                reason: "discovery.catalog_digest must be a non-empty string".into(),
            })?;
        if catalog_digest != self.digest.to_string() {
            return Err(EvaluatorError::StaleDiscovery {
                expected: self.digest.to_string(),
                received: catalog_digest.into(),
            });
        }
        let matches = discovery
            .get("matches")
            .and_then(Value::as_array)
            .ok_or_else(|| EvaluatorError::InvalidReview {
                reason: "discovery.matches must be an array".into(),
            })?;
        let mut candidates = BTreeMap::<String, MissionEvaluatorAdapter>::new();
        for matched in matches {
            let Some(adapter) = matched.get("adapter") else {
                continue;
            };
            let parsed: MissionEvaluatorAdapter = serde_json::from_value(adapter.clone())
                .map_err(|error| EvaluatorError::InvalidReview {
                    reason: format!("discovery adapter is invalid: {error}"),
                })?;
            candidates.insert(parsed.id.clone(), parsed);
        }
        if request.selections.is_empty() || request.selections.len() > MAX_REVIEW_SELECTIONS {
            return Err(EvaluatorError::InvalidReview {
                reason: format!(
                    "selections must contain between 1 and {MAX_REVIEW_SELECTIONS} entries"
                ),
            });
        }

        let discovery_digest = ContentHash::of_value(&request.discovery)
            .map_err(|error| EvaluatorError::Canonicalisation(error.to_string()))?
            .to_string();
        let mut ids = BTreeSet::new();
        let mut claim_counts = BTreeMap::<String, usize>::new();
        let mut findings = Vec::new();
        let mut bindings = Vec::new();
        for selection in &request.selections {
            let mut row = json!({
                "id": selection.id,
                "claim_id": selection.claim_id,
                "adapter_id": selection.adapter_id,
                "domain": selection.domain,
                "step_id": selection.step_id,
                "output_pointer": selection.output_pointer,
                "required": selection.required,
                "binding_posture": "blocked",
            });
            let mut row_errors = Vec::new();
            if !visible_text(&selection.id, 128) {
                row_errors.push("selection.id must be a visible string of at most 128 bytes");
            }
            if !visible_text(&selection.claim_id, 128) {
                row_errors.push("selection.claim_id must be a visible string of at most 128 bytes");
            }
            if !visible_text(&selection.adapter_id, 256) {
                row_errors.push("selection.adapter_id must be a visible string of at most 256 bytes");
            }
            if !visible_text(&selection.domain, 256) {
                row_errors.push("selection.domain must be a visible string of at most 256 bytes");
            }
            if !visible_text(&selection.step_id, 128) {
                row_errors.push("selection.step_id must be a visible string of at most 128 bytes");
            }
            if selection.output_pointer.contains('\0')
                || selection.output_pointer.contains('\n')
                || selection.output_pointer.contains('\r')
                || !valid_json_pointer(&selection.output_pointer)
            {
                row_errors.push("selection.output_pointer must be a valid RFC 6901 pointer");
            }
            if !ids.insert(selection.id.clone()) {
                row_errors.push("selection.id must be unique within the review");
            }
            let claim_count = claim_counts
                .entry(selection.claim_id.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            if *claim_count > MAX_BINDINGS_PER_CLAIM {
                row_errors.push("a claim may have at most 16 evaluator bindings");
            }

            let candidate = candidates.get(&selection.adapter_id);
            let domain_supported = candidate.is_some_and(|adapter| {
                let requested = normalized(&selection.domain);
                adapter
                    .domains
                    .iter()
                    .any(|domain| normalized(domain).contains(&requested))
            });
            row["candidate_found"] = json!(candidate.is_some());
            row["domain_supported"] = json!(domain_supported);
            if candidate.is_none() {
                row_errors.push("selection.adapter_id is not present in the supplied discovery matches");
            }
            if candidate.is_some() && !domain_supported {
                row_errors.push("selection.domain is not covered by the selected adapter's catalogue domains");
            }
            if let Some(candidate) = candidate {
                row["candidate_tools"] = json!(candidate.candidate_tools);
                row["output_pointer_examples"] = json!(candidate.output_pointer_examples);
                row["adapter_status"] = json!(candidate.status);
            }
            if row_errors.is_empty() {
                row["binding_posture"] = json!("ready");
                row["proposed_binding"] = json!({
                    "id": selection.id,
                    "adapter_id": selection.adapter_id,
                    "domain": selection.domain,
                    "step_id": selection.step_id,
                    "output_pointer": selection.output_pointer,
                    "required": selection.required,
                });
            } else {
                for message in row_errors {
                    findings.push(json!({
                        "selection_id": selection.id,
                        "claim_id": selection.claim_id,
                        "severity": "error",
                        "code": "invalid_evaluator_binding",
                        "message": message,
                    }));
                }
            }
            bindings.push(row);
        }
        let review_document = json!({
            "catalog_digest": catalog_digest,
            "discovery_digest": discovery_digest,
            "selections": request.selections,
        });
        let review_id = ContentHash::of_value(&review_document)
            .map_err(|error| EvaluatorError::Canonicalisation(error.to_string()))?
            .to_string();
        let ready = findings.is_empty();
        Ok(json!({
            "schema": MISSION_EVALUATOR_SCHEMA_VERSION,
            "workflow": "mission_evaluator_review",
            "ok": true,
            "review_id": review_id,
            "catalog_digest": catalog_digest,
            "discovery_digest": discovery_digest,
            "selection_count": request.selections.len(),
            "claim_count": claim_counts.len(),
            "claim_binding_limits": {
                "max_selections": MAX_REVIEW_SELECTIONS,
                "max_bindings_per_claim": MAX_BINDINGS_PER_CLAIM,
            },
            "bindings": bindings,
            "findings": findings,
            "review_status": if ready { "ready" } else { "blocked" },
            "binding_posture": if ready {
                "ready_for_mission_claim_bindings"
            } else {
                "requires_caller_correction"
            },
            "execution": "not_started",
            "guarantees": [
                "the discovery catalogue digest is checked before selections are reviewed",
                "selected adapters must be present in the caller-supplied discovery matches",
                "the output is a proposed claim-binding scaffold and still requires agent_mission validation",
                "no evaluator or domain tool was executed",
            ],
            "limitations": [
                "step existence and claim statement validity are checked only by the later agent_mission request",
                "domain compatibility is label coverage evidence, not semantic validation",
                "a ready review does not make a claim true, calibrated, causal, clinical, or release-ready",
            ],
        }))
    }
}

/// Fail-closed evaluator query errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvaluatorError {
    #[error("{field} filter must be non-empty when supplied")]
    EmptyFilter { field: &'static str },
    #[error("{field} filter contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} filter is {bytes} bytes; maximum is {maximum}")]
    FilterTooLong {
        field: &'static str,
        bytes: usize,
        maximum: usize,
    },
    #[error("max_items must be between 1 and {MAX_ITEMS}, got {value}")]
    InvalidLimit { value: usize },
    #[error("invalid evaluator review: {reason}")]
    InvalidReview { reason: String },
    #[error("evaluator discovery is stale: expected catalogue {expected}, received {received}")]
    StaleDiscovery { expected: String, received: String },
    #[error("cannot canonicalise evaluator review: {0}")]
    Canonicalisation(String),
}

fn adapter(
    id: &str,
    group_id: &str,
    domains: &[&str],
    levels: &[&str],
    purpose: &str,
    candidate_tools: &[&str],
    output_pointer_examples: &[&str],
) -> MissionEvaluatorAdapter {
    MissionEvaluatorAdapter {
        id: id.into(),
        group_id: group_id.into(),
        domains: domains.iter().map(|value| (*value).into()).collect(),
        levels: levels.iter().map(|value| (*value).into()).collect(),
        purpose: purpose.into(),
        candidate_tools: candidate_tools.iter().map(|value| (*value).into()).collect(),
        output_pointer_examples: output_pointer_examples
            .iter()
            .map(|value| (*value).into())
            .collect(),
        status: "candidate_only".into(),
    }
}

fn standard_adapters() -> Vec<MissionEvaluatorAdapter> {
    vec![
        adapter("world.observation_integrity", "world_and_ingestion", &["world state", "ingestion", "observations"], &["observation", "evaluation"], "Compare declared world observations with retained source and acquisition metadata.", &["world_validate", "observed_world_declare", "world_claim_check", "lineage_audit"], &["/ok", "/valid", "/digest"]),
        adapter("decision.context_coverage", "decision_context", &["decision context", "evidence coverage", "information acquisition"], &["observation", "evaluation"], "Inspect whether a decision context names required evidence dimensions and unresolved acquisition obligations.", &["fiber_compile", "projection_bundle", "obligation_gate_check"], &["/ok", "/coverage", "/obligations"]),
        adapter("context.projection_integrity", "token_efficient_context", &["context projection", "token efficiency", "redaction"], &["observation", "operational"], "Compare a compact projection with its source digest and declared semantic-loss boundary.", &["projection_bundle", "lens_leakage_check", "telemetry_project"], &["/digest", "/loss", "/boundaries"]),
        adapter("trajectory.replay_consistency", "trajectory_and_decision_cells", &["trajectory", "decision cell", "replay"], &["evaluation", "release"], "Check that a trajectory or decision cell can be re-read without collapsing observed state and interpretation.", &["trace_analyze", "benchmark_trace_analyze", "runtime_tape_verify"], &["/digest", "/replay", "/status"]),
        adapter("evaluation.baseline_comparability", "evaluation_and_baselines", &["evaluation", "baseline", "comparability"], &["evaluation", "release"], "Inspect declared baseline, split, metric, and comparability conditions without certifying performance.", &["evaluation_reproduction_check", "measurement_compare", "biocapability_evidence_audit"], &["/comparability", "/metrics", "/evidence"]),
        adapter("benchmark.portfolio_coverage", "benchmark_pack_portfolio", &["benchmark", "packs", "coverage"], &["evaluation", "release"], "Inspect benchmark portfolio coverage, difficulty, and missingness declarations.", &["pack_coverage_audit", "atlas_surface_audit", "pack_health_assess"], &["/coverage", "/missing", "/digest"]),
        adapter("megafactory.oracle_consensus", "megafactory_scale_and_oracles", &["scale", "oracle", "worker placement"], &["evaluation", "operational"], "Compare distributed oracle or worker observations while preserving tier, disagreement, and placement boundaries.", &["oracle_combine", "oracle_reference_panel", "factory_lifecycle_simulate"], &["/disagreement", "/tier", "/trace"]),
        adapter("mutation.causal_identification", "mutation_and_causal_discovery", &["mutation", "causal discovery", "intervention"], &["evaluation", "observation"], "Inspect whether mutation or intervention evidence is separated from causal interpretation and holdout status.", &["mutation_family", "benchmark_counterfactual_check", "benchmark_oracle_review"], &["/causal", "/holdout", "/evidence"]),
        adapter("bioeval.reference_contract", "bioevaluation_reference_contracts", &["bioevaluation", "reference standard", "metamorphic testing"], &["evaluation", "release"], "Inspect reference, metamorphic, and re-execution contracts for bounded biological evaluation.", &["bioeval_reference_audit", "bioeval_metamorphic_audit", "evaluation_reproduction_check"], &["/reference", "/relations", "/reexecution"]),
        adapter("biology.observation_consistency", "biological_domains", &["biology", "biological observation", "assay"], &["observation", "evaluation"], "Compare biological observations across declared modalities without inferring clinical meaning.", &["biocapability_evidence_audit", "literature_bind_check", "modality_comparability_check"], &["/evidence", "/dimensions", "/status"]),
        adapter("biolang.query_contract", "biological_ir_and_query", &["biological IR", "BioQL", "query"], &["observation", "evaluation"], "Inspect query schema, typed projection, and source binding closure for a biological IR request.", &["bioql_compile"], &["/schema", "/bindings", "/errors"]),
        adapter("foundation.schema_integrity", "foundation_contracts", &["foundation", "schema", "determinism"], &["observation", "release"], "Inspect version, digest, determinism, and refusal fields at a foundational contract boundary.", &["foundation_contract_check", "world_validate", "bioql_compile"], &["/schema", "/digest", "/valid"]),
        adapter("oncoworlds.identity_transport", "oncoworlds_identity_and_transport", &["oncology", "identity", "transport"], &["observation", "evaluation"], "Inspect subject, specimen, time, and transport identity joins without asserting clinical identity.", &["oncoworlds_identity_join", "lineage_audit", "onco_boundary_check"], &["/identity", "/transport", "/findings"]),
        adapter("oncoworlds.assay_fidelity", "oncoworlds_models_and_assays", &["oncology models", "assays", "fidelity"], &["evaluation", "observation"], "Compare model and assay fidelity axes with their declared evidence and split conditions.", &["oncoworlds_model_transport", "oncoworlds_methylation_classify", "oncoworlds_radiogenomic_check"], &["/fidelity", "/evidence", "/split"]),
        adapter("oncoworlds.clonal_history", "oncoworlds_clonal_evolution", &["clonal evolution", "lineage", "tumour population"], &["observation", "evaluation"], "Inspect clonal history continuity, sampling limits, and unresolved lineage transitions.", &["oncoworlds_clonal_history_check", "oncoworlds_clonal_evidence_check", "lineage_audit"], &["/lineage", "/transitions", "/missing"]),
        adapter("oncoworlds.shift_equity", "oncoworlds_shift_and_equity", &["distribution shift", "equity", "cohort"], &["evaluation", "operational"], "Inspect shift strata, cohort representation, and transport caveats without converting them into a fairness conclusion.", &["oncoworlds_era_shift_check", "oncoworlds_equity_check", "oncoworlds_entity_world_check"], &["/strata", "/coverage", "/transport"]),
        adapter("safety.policy_boundary", "safety_privacy_and_policy", &["safety", "privacy", "policy", "dual use"], &["operational", "release"], "Inspect policy, consent, privacy, and dual-use boundaries around a proposed output.", &["policy_screen", "safety_posture", "bioethics_dual_use_review", "medical_boundary_check"], &["/decision", "/refusal", "/controls"]),
        adapter("influence.abstract_soundness", "influence_bounds_and_abstract_analysis", &["influence bounds", "abstract analysis", "perturbation"], &["evaluation", "release"], "Inspect declared soundness bounds and omitted regions in an abstract influence analysis.", &["influence_analyze"], &["/bounds", "/omissions", "/sound"]),
        adapter("orchestration.workflow_wellformedness", "agent_orchestration", &["orchestration", "workflow", "session types", "quorum"], &["operational", "release"], "Inspect workflow closure, typed-act ordering, budgets, and quorum posture without scheduling work.", &["choreography_check", "weavelang_compile", "interweave_workflow_catalogue"], &["/well_formed", "/waves", "/budget"]),
        adapter("registry.lifecycle_provenance", "registry_operations_and_infrastructure", &["registry", "deployment", "storage", "leases"], &["operational", "release"], "Inspect lifecycle, provenance, cache, lease, and capacity evidence at an infrastructure boundary.", &["registry_gate", "storage_lifecycle_simulate", "ops_acceptance", "telemetry_project"], &["/lifecycle", "/provenance", "/capacity"]),
        adapter("atlas.metrics_coverage", "atlas_metrics_and_research_ci", &["metrics", "failure atlas", "research CI", "coverage"], &["evaluation", "release"], "Inspect metric coverage, failure axes, weight sensitivity, and research-CI findings.", &["atlas_report", "metrics_analytics_audit", "research_ci_check"], &["/coverage", "/findings", "/sensitivity"]),
        adapter("release.reproduction_evidence", "release_and_reproduction", &["release", "reproduction", "result bundle", "digest"], &["release", "operational"], "Inspect artifact digest, replay, acceptance, and release evidence without promoting a deployment.", &["bundle_verify", "release_audit", "evaluation_reproduction_check", "ops_acceptance"], &["/digest", "/reproduction", "/ready"]),
        adapter("hub.provenance_disclosure", "public_hub_submission_and_moderation", &["hub", "publication", "provenance", "moderation"], &["release", "operational"], "Inspect public submission provenance, disclosure, licensing, and independent verification posture.", &["hub_submission_review", "hub_disclosure_review", "bioatlas_publication_audit"], &["/provenance", "/disclosures", "/status"]),
        adapter("observability.telemetry_integrity", "observability_and_telemetry_boundaries", &["observability", "telemetry", "redaction", "cardinality"], &["operational", "release"], "Inspect observed-versus-asserted telemetry, redaction, semantic loss, and correlation boundaries.", &["telemetry_project", "operations_catalog", "ops_capacity"], &["/observed", "/redaction", "/loss"]),
        adapter("inference.holdout_separation", "inference_lab", &["inference", "holdout", "hypothesis", "research"], &["evaluation", "observation"], "Inspect hypothesis, acquisition, holdout, and branch separation in an inference-lab result.", &["lab_plan", "lab_holdout_audit", "lab_branch_audit", "routing_decide"], &["/holdout", "/hypotheses", "/risk"]),
        adapter("oracle.disagreement_adjudication", "oracle_mesh", &["oracle mesh", "disagreement", "adjudication"], &["evaluation", "release"], "Expose oracle missingness, tier, disagreement, and adjudication inputs without selecting a winner.", &["oracle_combine", "oracle_reference_panel", "oracle_missingness"], &["/disagreement", "/missingness", "/adjudication"]),
        adapter("runtime.replay_integrity", "runtime_execution_and_replay", &["runtime", "effects", "replay", "checkpoint"], &["operational", "release"], "Inspect effect authorization, hash-chained tape, checkpoint, and refusal preservation.", &["runtime_effect_check", "runtime_tape_verify", "runtime_execution_simulate"], &["/trace", "/checkpoint", "/refusal"]),
        adapter("documentation.contract_coverage", "documentation_and_knowledge", &["documentation", "knowledge", "repository navigation", "context"], &["observation", "operational"], "Inspect documentation graph, context projection, route coverage, and source-to-claim links.", &["workspace_capabilities", "repository_impact", "projection_bundle", "lens_leakage_check"], &["/coverage", "/edges", "/digest"]),
        adapter("developer.release_contract", "developer_and_release_contracts", &["developer platform", "SDK", "conformance", "release contract"], &["operational", "release"], "Inspect developer-facing contract, conformance, CI evidence, and delivery receipt closure.", &["developer_delivery_audit", "conformance_run", "ci_execution_evidence_audit", "developer_delivery_receipt_verify"], &["/findings", "/conformance", "/receipt"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_catalogue_covers_every_capability_group_once() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        assert_eq!(catalogue.adapters().len(), 29);
        let groups = catalogue
            .adapters()
            .iter()
            .map(|adapter| adapter.group_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(groups.len(), catalogue.adapters().len());
        assert!(catalogue
            .adapters()
            .iter()
            .all(|adapter| adapter.status == "candidate_only"));
    }

    #[test]
    fn search_is_conjunctive_digest_bound_and_stable() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let query = MissionEvaluatorQuery {
            query: Some("oncology fidelity".into()),
            ..MissionEvaluatorQuery::default()
        };
        let first = catalogue.search(&query).unwrap();
        let second = catalogue.search(&query).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.catalog_digest, catalogue.digest().to_string());
        assert_eq!(first.matches[0].adapter.id, "oncoworlds.assay_fidelity");
        assert!(first.matches[0]
            .adapter
            .candidate_tools
            .contains(&"oncoworlds_model_transport".into()));
    }

    #[test]
    fn invalid_filters_fail_closed() {
        let query = MissionEvaluatorQuery {
            level: Some(" ".into()),
            ..MissionEvaluatorQuery::default()
        };
        assert!(matches!(
            MissionEvaluatorCatalogue::standard().search(&query),
            Err(EvaluatorError::EmptyFilter { field: "level" })
        ));
    }
}
