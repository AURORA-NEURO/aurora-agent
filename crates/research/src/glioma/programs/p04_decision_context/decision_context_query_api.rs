//! Typed, bounded, omission-aware queries over local glioma decision contexts.
//!
//! The API indexes content-addressed semantic records rather than exposing raw research payloads.
//! Scope, capability, schema identity, result budgets, and cursor integrity are checked before a
//! page is emitted. Omitted and uncertain fields are either preserved as typed rows or reported as
//! policy omissions; they are never silently treated as absent evidence.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextQueryResult1@1";
pub const MAX_RECORDS: usize = 8_192;
pub const MAX_PAGE_SIZE: usize = 256;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryField {
    Claim,
    Action,
    Omission,
    NegativeEvidence,
    Uncertainty,
}

impl QueryField {
    fn tag(self) -> &'static str {
        match self {
            Self::Claim => "claim",
            Self::Action => "action",
            Self::Omission => "omission",
            Self::NegativeEvidence => "negative_evidence",
            Self::Uncertainty => "uncertainty",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryValueState {
    Measured,
    Null,
    Omitted,
    Uncertain,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextQueryRecord {
    pub context_id: String,
    pub schema_version: String,
    pub context_digest: ContentHash,
    pub scope: String,
    pub field: QueryField,
    pub record_id: String,
    pub value_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub state: QueryValueState,
    pub reason: Option<String>,
    pub updated_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCapability {
    pub capability_id: String,
    pub allowed_scope_order: Vec<String>,
    pub allow_omissions: bool,
    pub allow_uncertainty: bool,
    pub max_result_budget: usize,
    pub expires_at_tick: u64,
    pub revoked: bool,
    pub capability_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextQueryCursor {
    pub context_id: String,
    pub schema_version: String,
    pub context_digest: ContentHash,
    pub scope_prefix: String,
    pub field_order: Vec<QueryField>,
    pub capability_id: String,
    pub page_size: usize,
    pub result_budget: usize,
    pub last_record_key: String,
    pub cursor_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextQueryRequest {
    pub context_id: String,
    pub schema_version: String,
    pub context_digest: ContentHash,
    pub scope_prefix: String,
    pub field_order: Vec<QueryField>,
    pub after: Option<DecisionContextQueryCursor>,
    pub page_size: usize,
    pub result_budget: usize,
    pub capability: QueryCapability,
    pub current_tick: u64,
    pub records: Vec<DecisionContextQueryRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryCompleteness {
    Complete,
    More,
    Omitted,
    BudgetLimited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextQueryRow {
    pub record_id: String,
    pub scope: String,
    pub field: QueryField,
    pub state: QueryValueState,
    pub value_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextQueryResult {
    pub feature_id: String,
    pub output_schema: String,
    pub context_id: String,
    pub schema_version: String,
    pub context_digest: ContentHash,
    pub scope_prefix: String,
    pub field_order: Vec<QueryField>,
    pub rows: Vec<DecisionContextQueryRow>,
    pub next_cursor: Option<DecisionContextQueryCursor>,
    pub omitted_scope_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub completeness: QueryCompleteness,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextQueryError {
    #[error("decision-context query request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision-context query is unauthorized: {0}")]
    Unauthorized(String),
    #[error("decision-context query cursor is invalid: {0}")]
    InvalidCursor(String),
    #[error("decision-context query result is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision-context query digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_text(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values
            .iter()
            .all(|value| safe_text(value) && seen.insert(value.clone()))
}

fn scope_allowed(scope: &str, allowed: &[String]) -> bool {
    allowed
        .iter()
        .any(|prefix| scope == prefix || scope.starts_with(&format!("{prefix}/")))
}

fn record_key(record: &DecisionContextQueryRecord) -> String {
    format!(
        "{}|{}|{}",
        record.field.tag(),
        record.scope,
        record.record_id
    )
}

fn cursor_body(cursor: &DecisionContextQueryCursor) -> serde_json::Value {
    serde_json::json!({
        "context_id": cursor.context_id,
        "schema_version": cursor.schema_version,
        "context_digest": cursor.context_digest,
        "scope_prefix": cursor.scope_prefix,
        "field_order": cursor.field_order,
        "capability_id": cursor.capability_id,
        "page_size": cursor.page_size,
        "result_budget": cursor.result_budget,
        "last_record_key": cursor.last_record_key,
    })
}

fn capability_body(capability: &QueryCapability) -> serde_json::Value {
    serde_json::json!({
        "capability_id": capability.capability_id,
        "allowed_scope_order": capability.allowed_scope_order,
        "allow_omissions": capability.allow_omissions,
        "allow_uncertainty": capability.allow_uncertainty,
        "max_result_budget": capability.max_result_budget,
        "expires_at_tick": capability.expires_at_tick,
        "revoked": capability.revoked,
    })
}

fn digest_input(result: &DecisionContextQueryResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "context_id": result.context_id,
        "schema_version": result.schema_version,
        "context_digest": result.context_digest,
        "scope_prefix": result.scope_prefix,
        "field_order": result.field_order,
        "rows": result.rows,
        "next_cursor": result.next_cursor,
        "omitted_scope_order": result.omitted_scope_order,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "completeness": result.completeness,
    })
}

fn validate_capability(capability: &QueryCapability) -> Result<(), DecisionContextQueryError> {
    if !safe_text(&capability.capability_id)
        || !unique_text(&capability.allowed_scope_order, MAX_TEXT_LEN)
        || !canonical(&capability.allowed_scope_order)
        || capability.max_result_budget == 0
        || capability.max_result_budget > MAX_RECORDS
        || capability.expires_at_tick == 0
        || !valid_hash(&capability.capability_digest)
    {
        return Err(DecisionContextQueryError::InvalidRequest(
            "capability identity, ordered scopes, positive result budget, expiry, and digest are required".into(),
        ));
    }
    let expected = ContentHash::of_value(&capability_body(capability))
        .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?;
    if expected != capability.capability_digest {
        return Err(DecisionContextQueryError::Unauthorized(
            "capability digest does not match its declared scope and limits".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &DecisionContextQueryRequest,
) -> Result<(), DecisionContextQueryError> {
    if !safe_text(&request.context_id)
        || !safe_text(&request.schema_version)
        || !valid_hash(&request.context_digest)
        || !safe_text(&request.scope_prefix)
        || request.field_order.is_empty()
        || !canonical(&request.field_order)
        || request.page_size == 0
        || request.page_size > MAX_PAGE_SIZE
        || request.result_budget == 0
        || request.result_budget > MAX_RECORDS
        || request.page_size > request.result_budget
        || request.current_tick == 0
        || request.records.len() > MAX_RECORDS
    {
        return Err(DecisionContextQueryError::InvalidRequest(
            "bounded context identity, ordered typed fields, positive page/result budgets, current tick, and record limits are required".into(),
        ));
    }
    validate_capability(&request.capability)?;
    if request.capability.revoked {
        return Err(DecisionContextQueryError::Unauthorized(
            "capability is revoked".into(),
        ));
    }
    if request.current_tick > request.capability.expires_at_tick {
        return Err(DecisionContextQueryError::Unauthorized(
            "capability is expired".into(),
        ));
    }
    if request.result_budget > request.capability.max_result_budget {
        return Err(DecisionContextQueryError::InvalidRequest(
            "requested result budget exceeds the caller capability".into(),
        ));
    }
    if !scope_allowed(
        &request.scope_prefix,
        &request.capability.allowed_scope_order,
    ) {
        return Err(DecisionContextQueryError::Unauthorized(
            "requested scope is outside the caller capability".into(),
        ));
    }
    let mut record_ids = BTreeSet::new();
    for record in &request.records {
        if !safe_text(&record.context_id)
            || !safe_text(&record.schema_version)
            || !valid_hash(&record.context_digest)
            || !safe_text(&record.scope)
            || !safe_text(&record.record_id)
            || !valid_hash(&record.value_digest)
            || !valid_hash(&record.provenance_digest)
            || record.updated_tick == 0
            || !record_ids.insert((record.context_id.clone(), record.record_id.clone()))
            || (record.state == QueryValueState::Omitted
                && record
                    .reason
                    .as_deref()
                    .is_none_or(|value| !safe_text(value)))
        {
            return Err(DecisionContextQueryError::InvalidRequest(
                "records require unique bounded identities, content-addressed values/provenance, current ticks, and omission reasons".into(),
            ));
        }
    }
    if let Some(cursor) = &request.after {
        if cursor.context_id != request.context_id
            || cursor.schema_version != request.schema_version
            || cursor.context_digest != request.context_digest
            || cursor.scope_prefix != request.scope_prefix
            || cursor.field_order != request.field_order
            || cursor.capability_id != request.capability.capability_id
            || cursor.page_size != request.page_size
            || cursor.result_budget != request.result_budget
            || !safe_text(&cursor.last_record_key)
            || !valid_hash(&cursor.cursor_digest)
        {
            return Err(DecisionContextQueryError::InvalidCursor(
                "cursor identity does not match the query".into(),
            ));
        }
        let expected = ContentHash::of_value(&cursor_body(cursor))
            .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?;
        if expected != cursor.cursor_digest {
            return Err(DecisionContextQueryError::InvalidCursor(
                "cursor digest is tampered or stale".into(),
            ));
        }
    }
    Ok(())
}

impl DecisionContextQueryResult {
    pub fn validate(&self) -> Result<(), DecisionContextQueryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.context_id)
            || !safe_text(&self.schema_version)
            || !valid_hash(&self.context_digest)
            || !safe_text(&self.scope_prefix)
            || self.field_order.is_empty()
            || !canonical(&self.field_order)
            || !unique_text(&self.omitted_scope_order, MAX_RECORDS)
            || !canonical(&self.omitted_scope_order)
            || !unique_text(&self.negative_evidence_order, MAX_RECORDS)
            || !canonical(&self.negative_evidence_order)
            || !unique_text(&self.uncertainty_order, MAX_RECORDS)
            || !canonical(&self.uncertainty_order)
            || !valid_hash(&self.digest)
        {
            return Err(DecisionContextQueryError::InvalidOutput(
                "query result identity, field ordering, omission partitions, or digest is invalid"
                    .into(),
            ));
        }
        let mut keys = BTreeSet::new();
        for row in &self.rows {
            if !safe_text(&row.record_id)
                || !safe_text(&row.scope)
                || !valid_hash(&row.value_digest)
                || !valid_hash(&row.provenance_digest)
                || !keys.insert(format!(
                    "{}|{}|{}",
                    row.field.tag(),
                    row.scope,
                    row.record_id
                ))
                || (row.state == QueryValueState::Omitted
                    && row.reason.as_deref().is_none_or(|value| !safe_text(value)))
            {
                return Err(DecisionContextQueryError::InvalidOutput(
                    "query row identity, content-addressed values, or omission reason is invalid"
                        .into(),
                ));
            }
        }
        if let Some(cursor) = &self.next_cursor {
            if !safe_text(&cursor.last_record_key)
                || !valid_hash(&cursor.cursor_digest)
                || ContentHash::of_value(&cursor_body(cursor))
                    .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?
                    != cursor.cursor_digest
            {
                return Err(DecisionContextQueryError::InvalidOutput(
                    "next cursor is not content-addressed".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextQueryError::InvalidOutput(
                "query result digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn make_cursor(
    request: &DecisionContextQueryRequest,
    last_record_key: String,
) -> Result<DecisionContextQueryCursor, DecisionContextQueryError> {
    let mut cursor = DecisionContextQueryCursor {
        context_id: request.context_id.clone(),
        schema_version: request.schema_version.clone(),
        context_digest: request.context_digest.clone(),
        scope_prefix: request.scope_prefix.clone(),
        field_order: request.field_order.clone(),
        capability_id: request.capability.capability_id.clone(),
        page_size: request.page_size,
        result_budget: request.result_budget,
        last_record_key,
        cursor_digest: ContentHash::of_bytes(b"unsealed-glioma-query-cursor"),
    };
    cursor.cursor_digest = ContentHash::of_value(&cursor_body(&cursor))
        .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?;
    Ok(cursor)
}

/// Execute an omission-aware, capability-scoped query against a local context index.
pub fn query_glioma_decision_context(
    request: &DecisionContextQueryRequest,
) -> Result<DecisionContextQueryResult, DecisionContextQueryError> {
    validate_request(request)?;
    let mut candidates = request
        .records
        .iter()
        .filter(|record| {
            record.context_id == request.context_id
                && record.schema_version == request.schema_version
                && record.context_digest == request.context_digest
                && (record.scope == request.scope_prefix
                    || record
                        .scope
                        .starts_with(&format!("{}/", request.scope_prefix)))
                && request.field_order.binary_search(&record.field).is_ok()
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|record| record_key(record));
    if let Some(cursor) = &request.after {
        candidates.retain(|record| record_key(record) > cursor.last_record_key);
    }
    let mut omitted_scopes = BTreeSet::new();
    let mut negative = Vec::new();
    let mut uncertainty = Vec::new();
    let mut visible = Vec::new();
    for record in candidates {
        if !scope_allowed(&record.scope, &request.capability.allowed_scope_order) {
            omitted_scopes.insert(record.scope.clone());
            negative.push(format!("{}:scope-unauthorized", record.record_id));
            continue;
        }
        if record.state == QueryValueState::Omitted && !request.capability.allow_omissions {
            omitted_scopes.insert(record.scope.clone());
            negative.push(format!("{}:omission-policy", record.record_id));
            continue;
        }
        if record.state == QueryValueState::Uncertain && !request.capability.allow_uncertainty {
            omitted_scopes.insert(record.scope.clone());
            uncertainty.push(format!("{}:uncertainty-policy", record.record_id));
            continue;
        }
        visible.push(record);
    }
    let has_more = visible.len() > request.page_size;
    let rows = visible
        .iter()
        .take(request.page_size)
        .map(|record| DecisionContextQueryRow {
            record_id: record.record_id.clone(),
            scope: record.scope.clone(),
            field: record.field,
            state: record.state,
            value_digest: record.value_digest.clone(),
            provenance_digest: record.provenance_digest.clone(),
            reason: record.reason.clone(),
        })
        .collect::<Vec<_>>();
    if has_more {
        negative.push("page-size-limited-results-remain".into());
    }
    let next_cursor = rows
        .last()
        .map(|row| format!("{}|{}|{}", row.field.tag(), row.scope, row.record_id))
        .filter(|_| has_more)
        .map(|last_key| make_cursor(request, last_key))
        .transpose()?;
    let mut omitted_scope_order = omitted_scopes.into_iter().collect::<Vec<_>>();
    omitted_scope_order.sort();
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let completeness = if has_more {
        QueryCompleteness::More
    } else if !negative.is_empty() || !uncertainty.is_empty() {
        QueryCompleteness::Omitted
    } else {
        QueryCompleteness::Complete
    };
    let mut result = DecisionContextQueryResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        context_id: request.context_id.clone(),
        schema_version: request.schema_version.clone(),
        context_digest: request.context_digest.clone(),
        scope_prefix: request.scope_prefix.clone(),
        field_order: request.field_order.clone(),
        rows,
        next_cursor,
        omitted_scope_order,
        negative_evidence_order: negative,
        uncertainty_order: uncertainty,
        completeness,
        digest: ContentHash::of_bytes(b"unsealed-glioma-query-result"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| DecisionContextQueryError::Digest(error.to_string()))?;
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn capability(allow_omissions: bool) -> QueryCapability {
        let mut capability = QueryCapability {
            capability_id: "cap-glioma-query".into(),
            allowed_scope_order: vec!["claims".into()],
            allow_omissions,
            allow_uncertainty: true,
            max_result_budget: 8,
            expires_at_tick: 100,
            revoked: false,
            capability_digest: hash("unsealed-capability"),
        };
        capability.capability_digest =
            ContentHash::of_value(&capability_body(&capability)).unwrap();
        capability
    }

    fn record(
        id: &str,
        field: QueryField,
        state: QueryValueState,
        reason: Option<&str>,
    ) -> DecisionContextQueryRecord {
        DecisionContextQueryRecord {
            context_id: "ctx-glioma".into(),
            schema_version: "decision-context/1".into(),
            context_digest: hash("context"),
            scope: "claims/glioma".into(),
            field,
            record_id: id.into(),
            value_digest: hash(id),
            provenance_digest: hash("provenance"),
            state,
            reason: reason.map(str::to_owned),
            updated_tick: 1,
        }
    }

    fn request(
        records: Vec<DecisionContextQueryRecord>,
        page_size: usize,
    ) -> DecisionContextQueryRequest {
        DecisionContextQueryRequest {
            context_id: "ctx-glioma".into(),
            schema_version: "decision-context/1".into(),
            context_digest: hash("context"),
            scope_prefix: "claims/glioma".into(),
            field_order: vec![QueryField::Claim, QueryField::Omission],
            after: None,
            page_size,
            result_budget: 8,
            capability: capability(true),
            current_tick: 2,
            records,
        }
    }

    #[test]
    fn query_pages_with_a_content_addressed_cursor() {
        let first = query_glioma_decision_context(&request(
            vec![
                record(
                    "claim-a",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
                record(
                    "claim-b",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
            ],
            1,
        ))
        .unwrap();
        assert_eq!(first.rows.len(), 1);
        assert_eq!(first.completeness, QueryCompleteness::More);
        let mut second_request = request(
            vec![
                record(
                    "claim-a",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
                record(
                    "claim-b",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
            ],
            1,
        );
        second_request.after = first.next_cursor.clone();
        let second = query_glioma_decision_context(&second_request).unwrap();
        assert_eq!(second.rows[0].record_id, "claim-b");
        assert_eq!(second.completeness, QueryCompleteness::Complete);
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn query_rejects_scope_outside_capability() {
        let mut query = request(
            vec![record(
                "claim-a",
                QueryField::Claim,
                QueryValueState::Measured,
                None,
            )],
            2,
        );
        query.scope_prefix = "actions/glioma".into();
        assert!(matches!(
            query_glioma_decision_context(&query),
            Err(DecisionContextQueryError::Unauthorized(_))
        ));
    }

    #[test]
    fn query_rejects_tampered_cursor() {
        let first = query_glioma_decision_context(&request(
            vec![
                record(
                    "claim-a",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
                record(
                    "claim-b",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
            ],
            1,
        ))
        .unwrap();
        let mut query = request(
            vec![
                record(
                    "claim-a",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
                record(
                    "claim-b",
                    QueryField::Claim,
                    QueryValueState::Measured,
                    None,
                ),
            ],
            1,
        );
        query.after = first.next_cursor;
        query
            .after
            .as_mut()
            .unwrap()
            .last_record_key
            .push_str("-tampered");
        assert!(matches!(
            query_glioma_decision_context(&query),
            Err(DecisionContextQueryError::InvalidCursor(_))
        ));
    }

    #[test]
    fn query_preserves_or_reports_omissions_by_capability() {
        let omitted = record(
            "missing-provenance",
            QueryField::Omission,
            QueryValueState::Omitted,
            Some("protected assay unavailable"),
        );
        let visible = query_glioma_decision_context(&request(vec![omitted.clone()], 2)).unwrap();
        assert_eq!(visible.rows[0].state, QueryValueState::Omitted);
        let mut hidden_request = request(vec![omitted], 2);
        hidden_request.capability = capability(false);
        let hidden = query_glioma_decision_context(&hidden_request).unwrap();
        assert!(hidden.rows.is_empty());
        assert_eq!(hidden.completeness, QueryCompleteness::Omitted);
        assert!(!hidden.negative_evidence_order.is_empty());
    }

    #[test]
    fn query_rejects_result_budget_above_capability() {
        let mut query = request(
            vec![record(
                "claim-a",
                QueryField::Claim,
                QueryValueState::Measured,
                None,
            )],
            2,
        );
        query.result_budget = 9;
        assert!(matches!(
            query_glioma_decision_context(&query),
            Err(DecisionContextQueryError::InvalidRequest(_))
        ));
    }
}
