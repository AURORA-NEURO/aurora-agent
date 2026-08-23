//! Deterministic, transport-neutral discovery of mission evaluator candidates.
//!
//! A mission claim can name any caller-owned `adapter_id`, but an agent still needs a useful
//! answer to the question "which evaluator contract fits this domain?" This module supplies that
//! answer without creating a semantic oracle or a second execution registry. The catalogue is a
//! bounded set of explicit, reviewable candidate contracts. Every row says what it can inspect,
//! which mission level it fits, and which existing MCP tools are plausible evidence producers;
//! none of those labels authorizes execution or establishes that a claim is true.

use crate::mission::validate_route_review_provenance;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, Value};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Wire version for mission evaluator discovery.
pub const MISSION_EVALUATOR_SCHEMA_VERSION: &str = "bioprism-devplat-mission-evaluator/0.1";
pub const MISSION_EVALUATOR_CATALOGUE_SNAPSHOT_SCHEMA_VERSION: &str =
    "bioprism-devplat-mission-evaluator-catalogue-snapshot/0.1";
pub const MISSION_EVALUATOR_REPLAY_COMPARE_SCHEMA_VERSION: &str =
    "bioprism-devplat-mission-evaluator-replay-compare/0.1";
const MAX_ITEMS: usize = 256;
const DEFAULT_MAX_ITEMS: usize = 32;
const MAX_FILTER_BYTES: usize = 512;
const MAX_REVIEW_SELECTIONS: usize = 64;
const MAX_BINDINGS_PER_CLAIM: usize = 16;
const MAX_REPLAY_ITEMS: usize = 512;
const DEFAULT_REPLAY_ITEMS: usize = 128;
const MAX_CATALOGUE_SNAPSHOT_BYTES: usize = 512 * 1024;

fn default_max_items() -> usize {
    DEFAULT_MAX_ITEMS
}

fn default_include_fixtures() -> bool {
    true
}

fn default_replay_items() -> usize {
    DEFAULT_REPLAY_ITEMS
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

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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

/// Input to the non-executing replay/audit projection for a retained agent mission report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorReplayRequest {
    pub mission: Value,
    #[serde(default = "default_include_fixtures")]
    pub include_fixtures: bool,
    #[serde(default = "default_replay_items")]
    pub max_items: usize,
}

/// Input to the bounded, non-executing comparison between retained mission evaluator evidence
/// and the evaluator catalogue available in the current process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionEvaluatorReplayCompareRequest {
    pub mission: Value,
    #[serde(default = "default_include_fixtures")]
    pub include_fixtures: bool,
    #[serde(default = "default_replay_items")]
    pub max_items: usize,
}

/// Validated evaluator catalogue with a content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionEvaluatorCatalogue {
    adapters: Vec<MissionEvaluatorAdapter>,
    digest: ContentHash,
}

fn fixture_output_for_pointer(
    adapter: &MissionEvaluatorAdapter,
    pointer: &str,
    signal: &str,
) -> Value {
    let leaf = json!({
        "fixture_signal": signal,
        "adapter_id": adapter.id,
        "group_id": adapter.group_id,
        "domain": adapter.domains.first().cloned().unwrap_or_default(),
        "non_semantic": true,
    });
    if pointer.is_empty() {
        return leaf;
    }
    let mut output = leaf;
    let tokens = pointer
        .split('/')
        .skip(1)
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    for token in tokens.into_iter().rev() {
        let mut object = serde_json::Map::new();
        object.insert(token, output);
        output = Value::Object(object);
    }
    output
}

fn evaluator_fixture(adapter: &MissionEvaluatorAdapter) -> Value {
    let output_pointer = adapter
        .output_pointer_examples
        .first()
        .cloned()
        .unwrap_or_default();
    let retained_output = fixture_output_for_pointer(adapter, &output_pointer, "retained");
    let retained_value = if output_pointer.is_empty() {
        retained_output.clone()
    } else {
        retained_output
            .pointer(&output_pointer)
            .cloned()
            .unwrap_or(Value::Null)
    };
    let disagreement_output = fixture_output_for_pointer(adapter, &output_pointer, "disagreement");
    let retained_digest = ContentHash::of_value(&retained_value)
        .map(|digest| digest.to_string())
        .unwrap_or_default();
    let disagreement_digest = ContentHash::of_value(if output_pointer.is_empty() {
        &disagreement_output
    } else {
        disagreement_output
            .pointer(&output_pointer)
            .unwrap_or(&disagreement_output)
    })
    .map(|digest| digest.to_string())
    .unwrap_or_default();
    json!({
        "fixture_id": format!("mission-evaluator::{}", adapter.id),
        "adapter_id": adapter.id,
        "group_id": adapter.group_id,
        "domains": adapter.domains,
        "levels": adapter.levels,
        "output_pointer": output_pointer,
        "retained_output": retained_output,
        "retained_output_digest": retained_digest,
        "variants": [
            { "state": "retained", "pointer_resolves": true, "output_retained": true },
            { "state": "refused", "pointer_resolves": false, "output_retained": false, "step_status": "refused" },
            { "state": "omitted", "pointer_resolves": false, "output_retained": false, "step_status": "succeeded" },
            { "state": "disagreement", "pointer_resolves": true, "output_retained": true, "distinct_output_digest": disagreement_digest }
        ],
        "guarantee": "structural fixture coverage only; no evaluator, domain tool, or semantic claim was executed"
    })
}

impl MissionEvaluatorCatalogue {
    /// Build the in-tree catalogue covering each workspace capability group.
    pub fn standard() -> Self {
        let adapters = standard_adapters();
        let digest =
            ContentHash::of_value(&to_value(&adapters).expect("adapters are serializable"))
                .expect("standard evaluator catalogue must be hashable");
        Self { adapters, digest }
    }

    pub fn adapters(&self) -> &[MissionEvaluatorAdapter] {
        &self.adapters
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Return the bounded, content-addressed catalogue rows used by a review.
    ///
    /// The snapshot is descriptive and non-executable. Retaining it lets a later replay compare
    /// exact adapter rows instead of inferring a row-level explanation from a digest alone.
    pub fn snapshot(&self) -> Value {
        let rows = to_value(&self.adapters).expect("evaluator adapters are serializable");
        let snapshot_digest = ContentHash::of_value(&rows)
            .expect("evaluator catalogue rows must be hashable")
            .to_string();
        json!({
            "schema": MISSION_EVALUATOR_CATALOGUE_SNAPSHOT_SCHEMA_VERSION,
            "catalog_digest": self.digest.to_string(),
            "snapshot_digest": snapshot_digest,
            "row_count": self.adapters.len(),
            "group_count": self.adapters.iter().map(|adapter| adapter.group_id.as_str()).collect::<BTreeSet<_>>().len(),
            "rows": rows,
            "retention": {
                "rows_retained": true,
                "bounded": true,
                "maximum_bytes": MAX_CATALOGUE_SNAPSHOT_BYTES
            },
            "execution": "not_started"
        })
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
                (
                    "output_pointer_examples",
                    &adapter.output_pointer_examples.join(" "),
                ),
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
                        field_tokens
                            .iter()
                            .any(|candidate| {
                                candidate == query_token || candidate.starts_with(query_token)
                            })
                            .then_some(*field)
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
        let discovery =
            request
                .discovery
                .as_object()
                .ok_or_else(|| EvaluatorError::InvalidReview {
                    reason: "discovery must be an object".into(),
                })?;
        if discovery.get("workflow").and_then(Value::as_str) != Some("mission_evaluator_discover") {
            return Err(EvaluatorError::InvalidReview {
                reason: "discovery.workflow must be mission_evaluator_discover".into(),
            });
        }
        if discovery.get("selection_posture").and_then(Value::as_str) != Some("candidate_only") {
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
            let parsed: MissionEvaluatorAdapter =
                serde_json::from_value(adapter.clone()).map_err(|error| {
                    EvaluatorError::InvalidReview {
                        reason: format!("discovery adapter is invalid: {error}"),
                    }
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
                row_errors
                    .push("selection.adapter_id must be a visible string of at most 256 bytes");
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
                row_errors
                    .push("selection.adapter_id is not present in the supplied discovery matches");
            }
            if candidate.is_some() && !domain_supported {
                row_errors.push(
                    "selection.domain is not covered by the selected adapter's catalogue domains",
                );
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
                    "claim_id": selection.claim_id,
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
            "catalogue_snapshot": self.snapshot(),
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

    /// Replay retained mission lineage against the current evaluator catalogue without executing
    /// an evaluator or domain tool. The projection is intentionally structural: it checks adapter
    /// identity, label coverage, output-digest shape, outcome accounting, disagreement summaries,
    /// and catalogue coverage, while the generated fixtures exercise every catalogue row.
    pub fn replay(&self, request: &MissionEvaluatorReplayRequest) -> Result<Value, EvaluatorError> {
        if !(1..=MAX_REPLAY_ITEMS).contains(&request.max_items) {
            return Err(EvaluatorError::InvalidReplayLimit {
                value: request.max_items,
            });
        }
        let mission = request
            .mission
            .as_object()
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "mission must be an object".into(),
            })?;
        if mission.get("workflow").and_then(Value::as_str) != Some("agent_mission") {
            return Err(EvaluatorError::InvalidReplay {
                reason: "mission.workflow must be agent_mission".into(),
            });
        }
        let mission_id = mission
            .get("plan")
            .and_then(Value::as_object)
            .and_then(|plan| plan.get("mission_id"))
            .or_else(|| mission.get("mission_id"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "mission must contain a non-empty mission_id or plan.mission_id".into(),
            })?;
        let mission_digest = ContentHash::of_value(&request.mission)
            .map_err(|error| EvaluatorError::Canonicalisation(error.to_string()))?
            .to_string();
        let lineage = mission
            .get("claim_lineage")
            .and_then(Value::as_object)
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "mission.claim_lineage must be an object".into(),
            })?;
        let claims = lineage
            .get("claims")
            .and_then(Value::as_array)
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "mission.claim_lineage.claims must be an array".into(),
            })?;
        let mut findings = Vec::new();
        let route_review_provenance = mission
            .get("plan")
            .and_then(Value::as_object)
            .and_then(|plan| plan.get("route_review_provenance"))
            .cloned();
        let route_review_status = match route_review_provenance.as_ref() {
            None => "absent",
            Some(provenance) => match validate_route_review_provenance(provenance) {
                Ok(()) => "valid",
                Err(reason) => {
                    findings.push(json!({
                        "severity": "error",
                        "code": "route_review_provenance_invalid",
                        "message": reason,
                    }));
                    "invalid"
                }
            },
        };
        let mut binding_rows = Vec::new();
        let mut claim_rows = Vec::new();
        let mut state_counts = BTreeMap::<String, usize>::new();
        let mut replayed_adapter_ids = BTreeSet::new();
        let mut replayed_group_ids = BTreeSet::new();
        let mut returned_bindings = 0usize;
        let mut omitted_bindings = 0usize;

        for (claim_index, claim) in claims.iter().enumerate() {
            if claim_index >= request.max_items {
                omitted_bindings = omitted_bindings.saturating_add(1);
                continue;
            }
            let Some(claim_object) = claim.as_object() else {
                findings.push(json!({
                    "severity": "error",
                    "code": "claim_row_not_object",
                    "claim_index": claim_index,
                }));
                continue;
            };
            let claim_id = claim_object.get("id").and_then(Value::as_str).unwrap_or("");
            let bindings = claim_object
                .get("evaluator_bindings")
                .and_then(Value::as_array)
                .ok_or_else(|| EvaluatorError::InvalidReplay {
                    reason: format!("claim `{claim_id}` evaluator_bindings must be an array"),
                })?;
            let mut claim_states = BTreeMap::<String, usize>::new();
            let mut claim_digests = BTreeSet::new();
            let mut claim_returned_bindings = 0usize;
            for (binding_index, binding) in bindings.iter().enumerate() {
                let include_binding = returned_bindings < request.max_items;
                if !include_binding {
                    omitted_bindings = omitted_bindings.saturating_add(1);
                } else {
                    returned_bindings += 1;
                    claim_returned_bindings += 1;
                }
                let Some(binding_object) = binding.as_object() else {
                    findings.push(json!({
                        "severity": "error",
                        "code": "binding_row_not_object",
                        "claim_id": claim_id,
                        "binding_index": binding_index,
                    }));
                    continue;
                };
                let binding_id = binding_object
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let adapter_id = binding_object
                    .get("adapter_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let domain = binding_object
                    .get("domain")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let outcome_state = binding_object
                    .get("outcome_state")
                    .and_then(Value::as_str)
                    .unwrap_or("unreported");
                *state_counts.entry(outcome_state.to_string()).or_default() += 1;
                *claim_states.entry(outcome_state.to_string()).or_default() += 1;
                if let Some(digest) = binding_object.get("output_digest").and_then(Value::as_str) {
                    if valid_digest(digest) {
                        claim_digests.insert(digest.to_string());
                    } else {
                        findings.push(json!({
                            "severity": "error",
                            "code": "invalid_output_digest",
                            "claim_id": claim_id,
                            "binding_id": binding_id,
                        }));
                    }
                }
                let adapter = self
                    .adapters
                    .iter()
                    .find(|candidate| candidate.id == adapter_id);
                let domain_supported = adapter.is_some_and(|candidate| {
                    let requested = normalized(domain);
                    !requested.is_empty()
                        && candidate.domains.iter().any(|candidate_domain| {
                            normalized(candidate_domain).contains(&requested)
                        })
                });
                if adapter.is_none() {
                    findings.push(json!({
                        "severity": "error",
                        "code": "unknown_adapter",
                        "claim_id": claim_id,
                        "binding_id": binding_id,
                        "adapter_id": adapter_id,
                    }));
                } else if let Some(adapter) = adapter {
                    replayed_adapter_ids.insert(adapter_id.to_string());
                    replayed_group_ids.insert(adapter.group_id.clone());
                }
                if adapter.is_some() && !domain_supported {
                    findings.push(json!({
                        "severity": "error",
                        "code": "unsupported_domain_label",
                        "claim_id": claim_id,
                        "binding_id": binding_id,
                        "adapter_id": adapter_id,
                        "domain": domain,
                    }));
                }
                if outcome_state == "retained"
                    && binding_object
                        .get("output_digest")
                        .and_then(Value::as_str)
                        .is_none()
                {
                    findings.push(json!({
                        "severity": "error",
                        "code": "retained_without_output_digest",
                        "claim_id": claim_id,
                        "binding_id": binding_id,
                    }));
                }
                let replay_state = if adapter.is_none() {
                    "unknown_adapter"
                } else if !domain_supported {
                    "unsupported_domain"
                } else if outcome_state == "retained"
                    && binding_object
                        .get("output_digest")
                        .and_then(Value::as_str)
                        .is_some()
                {
                    "replayed_retained"
                } else if matches!(
                    outcome_state,
                    "refused" | "blocked" | "cancelled" | "output_omitted" | "pointer_missing"
                ) {
                    outcome_state
                } else {
                    "unreported"
                };
                let mut row = binding.clone();
                row["claim_id"] = json!(claim_id);
                row["catalog_match"] = json!(adapter.is_some());
                row["domain_supported"] = json!(domain_supported);
                row["replay_state"] = json!(replay_state);
                if include_binding {
                    binding_rows.push(row);
                }
            }
            let retained_count = claim_states.get("retained").copied().unwrap_or_default();
            let replayed_disagreement_posture = if bindings.is_empty() {
                "not_requested"
            } else if retained_count == 0 {
                "unavailable"
            } else if retained_count < bindings.len() {
                "partial"
            } else if retained_count == 1 {
                "single_observation"
            } else if claim_digests.len() == 1 {
                "unanimous_digest"
            } else {
                "disagreement"
            };
            let coverage = claim_object
                .get("evaluator_coverage")
                .and_then(Value::as_object);
            if let Some(coverage) = coverage {
                if let Some(expected) = coverage.get("outcome_counts") {
                    if expected != &json!(claim_states) {
                        findings.push(json!({
                            "severity": "error",
                            "code": "outcome_count_mismatch",
                            "claim_id": claim_id,
                            "expected": expected,
                            "replayed": claim_states,
                        }));
                    }
                }
                if let Some(expected) = coverage
                    .get("distinct_output_digests")
                    .and_then(Value::as_u64)
                {
                    if expected != claim_digests.len() as u64 {
                        findings.push(json!({
                            "severity": "error",
                            "code": "digest_count_mismatch",
                            "claim_id": claim_id,
                            "expected": expected,
                            "replayed": claim_digests.len(),
                        }));
                    }
                }
                if let Some(expected) = coverage.get("disagreement_posture").and_then(Value::as_str)
                {
                    if expected != replayed_disagreement_posture {
                        findings.push(json!({
                            "severity": "error",
                            "code": "disagreement_posture_mismatch",
                            "claim_id": claim_id,
                            "expected": expected,
                            "replayed": replayed_disagreement_posture,
                        }));
                    }
                }
            } else if !bindings.is_empty() {
                findings.push(json!({
                    "severity": "error",
                    "code": "missing_evaluator_coverage",
                    "claim_id": claim_id,
                }));
            }
            claim_rows.push(json!({
                "claim_id": claim_id,
                "binding_count": bindings.len(),
                "returned_binding_count": claim_returned_bindings,
                "outcome_counts": claim_states,
                "distinct_output_digests": claim_digests.len(),
                "disagreement_posture": coverage
                    .and_then(|value| value.get("disagreement_posture"))
                    .cloned()
                    .unwrap_or(Value::String("unreported".into())),
                "replayed_disagreement_posture": replayed_disagreement_posture,
            }));
        }
        let catalogue_adapter_ids = self
            .adapters
            .iter()
            .map(|adapter| adapter.id.clone())
            .collect::<BTreeSet<_>>();
        let catalogue_group_ids = self
            .adapters
            .iter()
            .map(|adapter| adapter.group_id.clone())
            .collect::<BTreeSet<_>>();
        let unrepresented_adapters = catalogue_adapter_ids
            .difference(&replayed_adapter_ids)
            .cloned()
            .collect::<Vec<_>>();
        let unrepresented_groups = catalogue_group_ids
            .difference(&replayed_group_ids)
            .cloned()
            .collect::<Vec<_>>();
        let omitted_fixtures = if request.include_fixtures {
            self.adapters.len().saturating_sub(request.max_items)
        } else {
            self.adapters.len()
        };
        let fixtures = if request.include_fixtures {
            self.adapters
                .iter()
                .take(request.max_items)
                .map(evaluator_fixture)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let ready = findings.is_empty();
        Ok(json!({
            "schema": MISSION_EVALUATOR_SCHEMA_VERSION,
            "workflow": "mission_evaluator_replay",
            "ok": true,
            "mission_id": mission_id,
            "mission_digest": mission_digest,
            "mission_status": mission.get("mission_status").cloned().unwrap_or(Value::Null),
            "review_provenance": lineage.get("evaluator_review").cloned().unwrap_or(json!({"present": false})),
            "route_review_provenance": route_review_provenance,
            "route_review_status": route_review_status,
            "catalog_digest": self.digest.to_string(),
            "binding_count": returned_bindings,
            "omitted_bindings": omitted_bindings,
            "state_counts": state_counts,
            "claims": claim_rows,
            "bindings": binding_rows,
            "coverage": {
                "catalogue_adapter_count": self.adapters.len(),
                "catalogue_group_count": catalogue_group_ids.len(),
                "replayed_adapter_count": replayed_adapter_ids.len(),
                "replayed_group_count": replayed_group_ids.len(),
                "unrepresented_adapters": unrepresented_adapters,
                "unrepresented_groups": unrepresented_groups,
                "complete": replayed_group_ids.len() == catalogue_group_ids.len(),
            },
            "findings": findings,
            "replay_status": if ready { "ready" } else { "blocked" },
            "execution": "not_started",
            "fixtures": fixtures,
            "omitted_fixtures": omitted_fixtures,
            "guarantees": [
                "replay inspects retained mission lineage and current catalogue labels only",
                "fixture rows cover every standard evaluator adapter without invoking a tool",
                "refusal, omission, pointer, digest, and disagreement states remain explicit",
            ],
            "limitations": [
                "replay does not rerun an evaluator or validate domain semantics",
                "catalogue coverage is structural and does not establish scientific, clinical, causal, or release truth",
                "a complete replay still requires caller-supplied retained mission outputs",
            ],
        }))
    }

    /// Compare retained mission evaluator evidence with the current catalogue without executing
    /// any evaluator or domain tool. Historical catalogue rows are not reconstructed from a
    /// digest; the result therefore separates digest drift from current binding compatibility.
    pub fn compare(
        &self,
        request: &MissionEvaluatorReplayCompareRequest,
    ) -> Result<Value, EvaluatorError> {
        let replay = self.replay(&MissionEvaluatorReplayRequest {
            mission: request.mission.clone(),
            include_fixtures: request.include_fixtures,
            max_items: request.max_items,
        })?;
        let mission = request
            .mission
            .as_object()
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "mission must be an object".into(),
            })?;
        let review = mission
            .get("claim_lineage")
            .and_then(Value::as_object)
            .and_then(|lineage| lineage.get("evaluator_review"))
            .and_then(Value::as_object)
            .or_else(|| mission.get("evaluator_review").and_then(Value::as_object));
        let historical_snapshot = review.and_then(|value| value.get("catalogue_snapshot"));
        let adapter_ids = mission_referenced_adapter_ids(&request.mission);
        let catalog_drift = self.catalog_drift(
            review
                .and_then(|value| value.get("catalog_digest"))
                .and_then(Value::as_str),
            review
                .and_then(|value| value.get("review_id"))
                .and_then(Value::as_str),
            review
                .and_then(|value| value.get("discovery_digest"))
                .and_then(Value::as_str),
            &adapter_ids,
            historical_snapshot,
            "mission_review_provenance",
        );
        Ok(json!({
            "schema": MISSION_EVALUATOR_REPLAY_COMPARE_SCHEMA_VERSION,
            "workflow": "mission_evaluator_replay_compare",
            "ok": true,
            "mission_id": replay.get("mission_id").cloned().unwrap_or(Value::Null),
            "replay": replay,
            "catalog_drift": catalog_drift,
            "execution": "not_started",
            "guarantees": [
                "the retained replay is compared against the current in-process evaluator catalogue without dispatch",
                "historical review and discovery digests remain separate from the current catalogue digest",
                "missing current adapters are reported rather than silently rebound"
            ],
            "limitations": [
                "a digest identifies historical catalogue content but does not contain its row-by-row snapshot",
                "the comparison cannot enumerate historical additions or removals unless the original catalogue rows were retained",
                "digest drift and binding compatibility are structural evidence, not semantic, scientific, clinical, causal, or release validation"
            ]
        }))
    }

    /// Compare a compact durable replay summary after the original mission result has been
    /// omitted. This keeps the same drift posture while refusing to imply that omitted rows can
    /// be reconstructed.
    pub fn compare_summary(&self, summary: &Value) -> Result<Value, EvaluatorError> {
        let object = summary
            .as_object()
            .ok_or_else(|| EvaluatorError::InvalidReplay {
                reason: "replay summary must be an object".into(),
            })?;
        if object.get("workflow").and_then(Value::as_str)
            != Some("mission_evaluator_replay_summary")
        {
            return Err(EvaluatorError::InvalidReplay {
                reason: "replay summary.workflow must be mission_evaluator_replay_summary".into(),
            });
        }
        let adapter_ids = object
            .get("referenced_adapter_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let historical_snapshot = object
            .get("historical_catalogue_snapshot")
            .or_else(|| object.get("catalogue_snapshot"));
        let historical_catalog_digest = object
            .get("historical_catalog_digest")
            .and_then(Value::as_str)
            .or_else(|| {
                historical_snapshot
                    .and_then(|snapshot| snapshot.get("catalog_digest"))
                    .and_then(Value::as_str)
            });
        let catalog_drift = self.catalog_drift(
            historical_catalog_digest,
            object.get("historical_review_id").and_then(Value::as_str),
            object
                .get("historical_discovery_digest")
                .and_then(Value::as_str),
            &adapter_ids,
            historical_snapshot,
            "durable_replay_summary",
        );
        Ok(json!({
            "schema": MISSION_EVALUATOR_REPLAY_COMPARE_SCHEMA_VERSION,
            "workflow": "mission_evaluator_replay_compare",
            "ok": true,
            "mission_id": object.get("mission_id").cloned().unwrap_or(Value::Null),
            "replay": summary,
            "catalog_drift": catalog_drift,
            "execution": "not_started",
            "guarantees": [
                "the comparison uses only the compact checkpoint summary and current catalogue",
                "result omission and non-executing posture remain explicit"
            ],
            "limitations": [
                "omitted raw outputs and historical catalogue rows cannot be reconstructed",
                "summary-only comparison cannot enumerate exact historical adapter additions or removals",
                "the comparison is structural evidence rather than semantic or release validation"
            ]
        }))
    }

    fn catalog_drift(
        &self,
        historical_catalog_digest: Option<&str>,
        historical_review_id: Option<&str>,
        historical_discovery_digest: Option<&str>,
        referenced_adapter_ids: &BTreeSet<String>,
        historical_snapshot: Option<&Value>,
        source: &str,
    ) -> Value {
        let current_catalog_digest = self.digest.to_string();
        let current_adapter_ids = self
            .adapters
            .iter()
            .map(|adapter| adapter.id.clone())
            .collect::<BTreeSet<_>>();
        let missing_referenced_adapters = referenced_adapter_ids
            .difference(&current_adapter_ids)
            .cloned()
            .collect::<Vec<_>>();
        let compatible_referenced_adapters = referenced_adapter_ids
            .intersection(&current_adapter_ids)
            .cloned()
            .collect::<Vec<_>>();
        let historical_digest_valid = historical_catalog_digest.is_some_and(valid_digest);
        let digest_match = historical_catalog_digest
            .filter(|_| historical_digest_valid)
            .map(|value| value == current_catalog_digest);
        let status = if historical_catalog_digest.is_none() {
            "not_recorded"
        } else if !historical_digest_valid {
            "invalid_recorded_digest"
        } else if !missing_referenced_adapters.is_empty() && digest_match == Some(true) {
            "unchanged_with_missing_bindings"
        } else if !missing_referenced_adapters.is_empty() {
            "drifted_with_missing_bindings"
        } else if digest_match == Some(true) {
            "unchanged"
        } else {
            "drifted"
        };
        let mut result = json!({
            "status": status,
            "historical_catalog_digest": historical_catalog_digest.map(str::to_string),
            "current_catalog_digest": current_catalog_digest,
            "digest_match": digest_match,
            "historical_digest_valid": historical_catalog_digest.is_some() && historical_digest_valid,
            "historical_review_id": historical_review_id.map(str::to_string),
            "historical_discovery_digest": historical_discovery_digest.map(str::to_string),
            "referenced_adapter_count": referenced_adapter_ids.len(),
            "compatible_referenced_adapters": compatible_referenced_adapters,
            "missing_referenced_adapters": missing_referenced_adapters,
            "current_catalogue_adapter_count": self.adapters.len(),
            "current_catalogue_group_count": self
                .adapters
                .iter()
                .map(|adapter| adapter.group_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            "comparison_scope": "historical_digest_and_current_binding_compatibility",
            "historical_catalogue_rows_retained": false,
            "source": source,
        });
        if let (Some(target), Some(snapshot_diff)) = (
            result.as_object_mut(),
            self.catalogue_snapshot_diff(historical_snapshot)
                .as_object(),
        ) {
            for (key, value) in snapshot_diff {
                target.insert(key.clone(), value.clone());
            }
        }
        result
    }

    fn catalogue_snapshot_diff(&self, snapshot: Option<&Value>) -> Value {
        let Some(snapshot) = snapshot else {
            return json!({
                "historical_snapshot_present": false,
                "historical_snapshot_valid": false,
                "historical_catalogue_rows_retained": false,
                "exact_row_diff_available": false,
                "row_diff_status": "not_recorded",
                "comparison_scope": "historical_digest_and_current_binding_compatibility",
                "added_adapter_ids": [],
                "removed_adapter_ids": [],
                "changed_adapter_ids": [],
                "unchanged_adapter_ids": [],
                "changed_adapter_fields": {}
            });
        };
        let Some(object) = snapshot.as_object() else {
            return json!({
                "historical_snapshot_present": true,
                "historical_snapshot_valid": false,
                "historical_catalogue_rows_retained": false,
                "exact_row_diff_available": false,
                "row_diff_status": "invalid",
                "historical_snapshot_invalid_reason": "snapshot must be an object",
                "comparison_scope": "historical_digest_and_current_binding_compatibility"
            });
        };
        let Some(rows) = object.get("rows").and_then(Value::as_array) else {
            return json!({
                "historical_snapshot_present": true,
                "historical_snapshot_valid": false,
                "historical_catalogue_rows_retained": false,
                "exact_row_diff_available": false,
                "row_diff_status": "invalid",
                "historical_snapshot_invalid_reason": "snapshot.rows must be an array",
                "comparison_scope": "historical_digest_and_current_binding_compatibility"
            });
        };
        let encoded_size = serde_json::to_vec(snapshot)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        if rows.len() > MAX_REPLAY_ITEMS || encoded_size > MAX_CATALOGUE_SNAPSHOT_BYTES {
            return json!({
                "historical_snapshot_present": true,
                "historical_snapshot_valid": false,
                "historical_catalogue_rows_retained": false,
                "exact_row_diff_available": false,
                "row_diff_status": "invalid",
                "historical_snapshot_invalid_reason": "snapshot exceeds the bounded row or byte limit",
                "comparison_scope": "historical_digest_and_current_binding_compatibility"
            });
        }
        let rows_value = Value::Array(rows.clone());
        let recomputed_snapshot_digest = ContentHash::of_value(&rows_value)
            .map(|digest| digest.to_string())
            .unwrap_or_default();
        let supplied_snapshot_digest = object
            .get("snapshot_digest")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let supplied_catalog_digest = object
            .get("catalog_digest")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut historical_rows = BTreeMap::<String, Value>::new();
        let mut invalid_reason = None;
        for row in rows {
            let Ok(adapter) = serde_json::from_value::<MissionEvaluatorAdapter>(row.clone()) else {
                invalid_reason = Some("snapshot.rows contains an invalid adapter row".to_string());
                break;
            };
            if historical_rows
                .insert(adapter.id.clone(), row.clone())
                .is_some()
            {
                invalid_reason = Some("snapshot.rows contains duplicate adapter IDs".to_string());
                break;
            }
        }
        let snapshot_digest_match = valid_digest(supplied_snapshot_digest)
            && supplied_snapshot_digest == recomputed_snapshot_digest;
        let catalog_digest_match = valid_digest(supplied_catalog_digest)
            && supplied_catalog_digest == recomputed_snapshot_digest;
        if invalid_reason.is_none() && (!snapshot_digest_match || !catalog_digest_match) {
            invalid_reason = Some("snapshot digest does not match its retained rows".to_string());
        }
        if let Some(reason) = invalid_reason {
            return json!({
                "historical_snapshot_present": true,
                "historical_snapshot_valid": false,
                "historical_catalogue_rows_retained": false,
                "exact_row_diff_available": false,
                "row_diff_status": "invalid",
                "historical_snapshot_digest": supplied_snapshot_digest,
                "recomputed_historical_snapshot_digest": recomputed_snapshot_digest,
                "historical_snapshot_digest_match": snapshot_digest_match,
                "historical_snapshot_invalid_reason": reason,
                "comparison_scope": "historical_digest_and_current_binding_compatibility"
            });
        }
        let current_rows = self
            .adapters
            .iter()
            .map(|adapter| {
                (
                    adapter.id.clone(),
                    to_value(adapter).expect("adapter is serializable"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let historical_ids = historical_rows.keys().cloned().collect::<BTreeSet<_>>();
        let current_ids = current_rows.keys().cloned().collect::<BTreeSet<_>>();
        let added_adapter_ids = current_ids
            .difference(&historical_ids)
            .cloned()
            .collect::<Vec<_>>();
        let removed_adapter_ids = historical_ids
            .difference(&current_ids)
            .cloned()
            .collect::<Vec<_>>();
        let mut changed_adapter_ids = Vec::new();
        let mut unchanged_adapter_ids = Vec::new();
        let mut changed_adapter_fields = BTreeMap::<String, Vec<String>>::new();
        for id in historical_ids.intersection(&current_ids) {
            let historical = historical_rows.get(id).expect("intersection row exists");
            let current = current_rows.get(id).expect("intersection row exists");
            if historical == current {
                unchanged_adapter_ids.push(id.clone());
                continue;
            }
            changed_adapter_ids.push(id.clone());
            let mut fields = BTreeSet::new();
            if let (Some(historical), Some(current)) = (historical.as_object(), current.as_object())
            {
                let keys = historical
                    .keys()
                    .chain(current.keys())
                    .cloned()
                    .collect::<BTreeSet<_>>();
                for key in keys {
                    if historical.get(&key) != current.get(&key) {
                        fields.insert(key);
                    }
                }
            }
            changed_adapter_fields.insert(id.clone(), fields.into_iter().collect());
        }
        json!({
            "historical_snapshot_present": true,
            "historical_snapshot_valid": true,
            "historical_catalogue_rows_retained": true,
            "exact_row_diff_available": true,
            "row_diff_status": "exact",
            "historical_snapshot_digest": supplied_snapshot_digest,
            "recomputed_historical_snapshot_digest": recomputed_snapshot_digest,
            "historical_snapshot_digest_match": true,
            "historical_snapshot_row_count": historical_rows.len(),
            "added_adapter_ids": added_adapter_ids,
            "removed_adapter_ids": removed_adapter_ids,
            "changed_adapter_ids": changed_adapter_ids,
            "unchanged_adapter_ids": unchanged_adapter_ids,
            "changed_adapter_fields": changed_adapter_fields,
            "comparison_scope": "exact_adapter_row_comparison"
        })
    }
}

fn mission_referenced_adapter_ids(mission: &Value) -> BTreeSet<String> {
    mission
        .get("claim_lineage")
        .and_then(|lineage| lineage.get("claims"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|claim| claim.get("evaluator_bindings"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|binding| binding.get("adapter_id"))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
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
    #[error("invalid evaluator replay: {reason}")]
    InvalidReplay { reason: String },
    #[error("evaluator replay max_items must be between 1 and {MAX_REPLAY_ITEMS}, got {value}")]
    InvalidReplayLimit { value: usize },
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
        candidate_tools: candidate_tools
            .iter()
            .map(|value| (*value).into())
            .collect(),
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

    #[test]
    fn replay_fixtures_cover_every_adapter_without_execution() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let request = MissionEvaluatorReplayRequest {
            mission: json!({
                "workflow": "agent_mission",
                "plan": {"mission_id": "mission-fixtures"},
                "claim_lineage": {"claims": []}
            }),
            include_fixtures: true,
            max_items: MAX_REPLAY_ITEMS,
        };
        let replay = catalogue.replay(&request).unwrap();
        assert_eq!(replay["workflow"], json!("mission_evaluator_replay"));
        assert_eq!(replay["execution"], json!("not_started"));
        assert_eq!(replay["fixtures"].as_array().unwrap().len(), 29);
        assert_eq!(replay["omitted_fixtures"], json!(0));
        assert!(replay["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fixture| {
                fixture["variants"]
                    .as_array()
                    .is_some_and(|variants| variants.len() == 4)
                    && fixture["guarantee"]
                        .as_str()
                        .is_some_and(|value| value.contains("structural"))
            }));
    }

    #[test]
    fn replay_recomputes_claim_accounting_and_reports_unknown_rows() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let request = MissionEvaluatorReplayRequest {
            mission: json!({
                "workflow": "agent_mission",
                "plan": {"mission_id": "mission-audit"},
                "claim_lineage": {"claims": [{
                    "id": "claim-1",
                    "evaluator_bindings": [
                        {
                            "id": "known",
                            "adapter_id": "oncoworlds.assay_fidelity",
                            "domain": "oncology",
                            "outcome_state": "retained",
                            "output_digest": "a".repeat(64)
                        },
                        {
                            "id": "unknown",
                            "adapter_id": "missing.adapter",
                            "domain": "unknown",
                            "outcome_state": "retained",
                            "output_digest": "b".repeat(64)
                        }
                    ],
                    "evaluator_coverage": {
                        "outcome_counts": {"retained": 1},
                        "distinct_output_digests": 1,
                        "disagreement_posture": "unreported"
                    }
                }]}
            }),
            include_fixtures: false,
            max_items: DEFAULT_REPLAY_ITEMS,
        };
        let replay = catalogue.replay(&request).unwrap();
        assert_eq!(replay["replay_status"], json!("blocked"));
        assert!(replay["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| { finding["code"] == json!("outcome_count_mismatch") }));
        assert!(replay["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| { finding["code"] == json!("unknown_adapter") }));
        assert_eq!(replay["fixtures"], json!([]));
        assert_eq!(replay["omitted_fixtures"], json!(29));
    }

    #[test]
    fn replay_comparison_separates_digest_drift_from_binding_compatibility() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let mission = |catalog_digest: String, adapter_id: &str| {
            json!({
                "workflow": "agent_mission",
                "plan": {"mission_id": "mission-compare"},
                "claim_lineage": {
                    "evaluator_review": {
                        "present": true,
                        "review_id": "b".repeat(64),
                        "catalog_digest": catalog_digest,
                        "discovery_digest": "c".repeat(64)
                    },
                    "claims": [{
                        "id": "claim-1",
                        "evaluator_bindings": [{
                            "id": "binding-1",
                            "adapter_id": adapter_id,
                            "domain": "oncology",
                            "step_id": "assay",
                            "output_pointer": "/fidelity",
                            "required": true,
                            "outcome_state": "output_omitted"
                        }],
                        "evaluator_coverage": {
                            "outcome_counts": {"output_omitted": 1},
                            "distinct_output_digests": 0,
                            "disagreement_posture": "unavailable"
                        }
                    }]
                }
            })
        };
        let unchanged = catalogue
            .compare(&MissionEvaluatorReplayCompareRequest {
                mission: mission(catalogue.digest().to_string(), "oncoworlds.assay_fidelity"),
                include_fixtures: false,
                max_items: 16,
            })
            .unwrap();
        assert_eq!(unchanged["workflow"], "mission_evaluator_replay_compare");
        assert_eq!(unchanged["catalog_drift"]["status"], "unchanged");
        assert_eq!(unchanged["catalog_drift"]["digest_match"], true);
        assert_eq!(
            unchanged["catalog_drift"]["missing_referenced_adapters"],
            json!([])
        );

        let drifted = catalogue
            .compare(&MissionEvaluatorReplayCompareRequest {
                mission: mission("a".repeat(64), "oncoworlds.assay_fidelity"),
                include_fixtures: false,
                max_items: 16,
            })
            .unwrap();
        assert_eq!(drifted["catalog_drift"]["status"], "drifted");
        assert_eq!(drifted["catalog_drift"]["digest_match"], false);

        let missing = catalogue
            .compare(&MissionEvaluatorReplayCompareRequest {
                mission: mission("a".repeat(64), "removed.adapter"),
                include_fixtures: false,
                max_items: 16,
            })
            .unwrap();
        assert_eq!(
            missing["catalog_drift"]["status"],
            "drifted_with_missing_bindings"
        );
        assert_eq!(
            missing["catalog_drift"]["missing_referenced_adapters"],
            json!(["removed.adapter"])
        );
    }

    #[test]
    fn summary_comparison_preserves_omission_and_historical_digest_limits() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let summary = json!({
            "workflow": "mission_evaluator_replay_summary",
            "mission_id": "mission-summary",
            "historical_catalog_digest": "a".repeat(64),
            "historical_review_id": "b".repeat(64),
            "historical_discovery_digest": "c".repeat(64),
            "referenced_adapter_ids": ["oncoworlds.assay_fidelity"],
            "result_retained": true,
            "result_digest": "d".repeat(64)
        });
        let comparison = catalogue.compare_summary(&summary).unwrap();
        assert_eq!(comparison["catalog_drift"]["status"], "drifted");
        assert_eq!(
            comparison["catalog_drift"]["historical_catalogue_rows_retained"],
            false
        );
        assert!(comparison["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| {
                item.as_str()
                    .is_some_and(|value| value.contains("historical catalogue rows"))
            }));
    }

    #[test]
    fn retained_catalogue_snapshot_produces_exact_adapter_row_diff() {
        let catalogue = MissionEvaluatorCatalogue::standard();
        let snapshot = catalogue.snapshot();
        assert_eq!(
            snapshot["schema"],
            MISSION_EVALUATOR_CATALOGUE_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(snapshot["row_count"], json!(29));
        assert_eq!(snapshot["snapshot_digest"], catalogue.digest().to_string());
        let mission = json!({
            "workflow": "agent_mission",
            "plan": {"mission_id": "mission-snapshot"},
            "claim_lineage": {
                "evaluator_review": {
                    "catalog_digest": catalogue.digest().to_string(),
                    "review_id": "a".repeat(64),
                    "discovery_digest": "b".repeat(64),
                    "catalogue_snapshot": snapshot
                },
                "claims": []
            }
        });
        let unchanged = catalogue
            .compare(&MissionEvaluatorReplayCompareRequest {
                mission,
                include_fixtures: false,
                max_items: 16,
            })
            .unwrap();
        assert_eq!(unchanged["catalog_drift"]["row_diff_status"], "exact");
        assert_eq!(unchanged["catalog_drift"]["exact_row_diff_available"], true);
        assert_eq!(unchanged["catalog_drift"]["changed_adapter_ids"], json!([]));
        assert_eq!(unchanged["catalog_drift"]["removed_adapter_ids"], json!([]));

        let mut changed_snapshot = catalogue.snapshot();
        changed_snapshot["rows"][0]["purpose"] =
            json!("historical purpose before a catalogue revision");
        let row_digest = ContentHash::of_value(&changed_snapshot["rows"])
            .unwrap()
            .to_string();
        changed_snapshot["snapshot_digest"] = json!(row_digest.clone());
        changed_snapshot["catalog_digest"] = json!(row_digest);
        let changed_mission = json!({
            "workflow": "agent_mission",
            "plan": {"mission_id": "mission-snapshot-changed"},
            "claim_lineage": {
                "evaluator_review": {
                    "catalog_digest": changed_snapshot["catalog_digest"],
                    "review_id": "a".repeat(64),
                    "discovery_digest": "b".repeat(64),
                    "catalogue_snapshot": changed_snapshot
                },
                "claims": []
            }
        });
        let changed = catalogue
            .compare(&MissionEvaluatorReplayCompareRequest {
                mission: changed_mission,
                include_fixtures: false,
                max_items: 16,
            })
            .unwrap();
        assert_eq!(changed["catalog_drift"]["row_diff_status"], "exact");
        assert_eq!(changed["catalog_drift"]["status"], "drifted");
        assert_eq!(
            changed["catalog_drift"]["changed_adapter_ids"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(changed["catalog_drift"]["changed_adapter_fields"]
            .as_object()
            .unwrap()
            .values()
            .any(|fields| fields
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "purpose")));
    }

    #[test]
    fn replay_rejects_zero_limit() {
        let request = MissionEvaluatorReplayRequest {
            mission: json!({"workflow": "agent_mission"}),
            include_fixtures: false,
            max_items: 0,
        };
        assert!(matches!(
            MissionEvaluatorCatalogue::standard().replay(&request),
            Err(EvaluatorError::InvalidReplayLimit { value: 0 })
        ));
    }
}

#[test]
fn replay_validates_retained_route_review_provenance_without_execution() {
    let provenance = json!({
        "present": true,
        "review_id": "a".repeat(64),
        "route_id": "b".repeat(64),
        "catalog_digest": "c".repeat(64),
        "evidence_present": false,
        "posture": "not_supplied",
        "readiness_claimed": false,
        "execution": "not_started"
    });
    let catalogue = MissionEvaluatorCatalogue::standard();
    let request = MissionEvaluatorReplayRequest {
        mission: json!({
            "workflow": "agent_mission",
            "plan": {
                "mission_id": "mission-route-replay",
                "route_review_provenance": provenance
            },
            "claim_lineage": {"claims": []}
        }),
        include_fixtures: false,
        max_items: DEFAULT_REPLAY_ITEMS,
    };
    let replay = catalogue.replay(&request).unwrap();
    assert_eq!(replay["route_review_status"], "valid");
    assert_eq!(replay["replay_status"], "ready");

    let mut tampered = request.mission.clone();
    tampered["plan"]["route_review_provenance"]["readiness_claimed"] = json!(true);
    let blocked = catalogue
        .replay(&MissionEvaluatorReplayRequest {
            mission: tampered,
            include_fixtures: false,
            max_items: DEFAULT_REPLAY_ITEMS,
        })
        .unwrap();
    assert_eq!(blocked["route_review_status"], "invalid");
    assert_eq!(blocked["replay_status"], "blocked");
    assert!(blocked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["code"] == "route_review_provenance_invalid"));
}
