//! Resumable, audience- and locality-gated exchange planning for preclinical glioma objects.
//!
//! The exchange API reconciles a signed manifest with caller-supplied chunk acknowledgements. It
//! is idempotent across retries, detects gaps and conflicting chunks, and never treats an active
//! transfer as publication. Network I/O is intentionally outside this deterministic contract.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectExchange1@1";
pub const MAX_CHUNKS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeManifest {
    pub object_id: String,
    pub version_id: String,
    pub content_digest: ContentHash,
    pub total_bytes: u64,
    pub chunk_size: u32,
    pub audience_scope: String,
    pub locality_region: String,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeChunk {
    pub index: u32,
    pub content_digest: ContentHash,
    pub byte_len: u32,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangePolicy {
    pub recipient_id: String,
    pub permitted_audience_scope: String,
    pub permitted_region: String,
    pub max_bytes: u64,
    pub grant_active: bool,
    pub require_signed_manifest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectExchangeRequest {
    pub manifest: ExchangeManifest,
    pub chunks: Vec<ExchangeChunk>,
    pub policy: ExchangePolicy,
    pub resume_cursor: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObjectExchangeRecord {
    pub feature_id: String,
    pub output_schema: String,
    pub object_id: String,
    pub version_id: String,
    pub recipient_id: String,
    pub disposition: ExchangeDisposition,
    pub next_chunk: u32,
    pub acknowledged_order: Vec<u32>,
    pub missing_order: Vec<u32>,
    pub duplicate_order: Vec<u32>,
    pub blocking_order: Vec<String>,
    pub verified_bytes: u64,
    pub final_digest: Option<ContentHash>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExchangeApiError {
    #[error("research-object exchange request is invalid: {0}")]
    InvalidRequest(String),
    #[error("research-object exchange output is invalid: {0}")]
    InvalidOutput(String),
    #[error("research-object exchange digest failed: {0}")]
    Digest(String),
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn digest_input(record: &ResearchObjectExchangeRecord) -> serde_json::Value {
    serde_json::json!({"feature_id":record.feature_id,"output_schema":record.output_schema,"object_id":record.object_id,"version_id":record.version_id,"recipient_id":record.recipient_id,"disposition":record.disposition,"next_chunk":record.next_chunk,"acknowledged_order":record.acknowledged_order,"missing_order":record.missing_order,"duplicate_order":record.duplicate_order,"blocking_order":record.blocking_order,"verified_bytes":record.verified_bytes,"final_digest":record.final_digest})
}
fn validate_request(request: &ResearchObjectExchangeRequest) -> Result<(), ExchangeApiError> {
    let manifest = &request.manifest;
    let policy = &request.policy;
    if !valid_id(&manifest.object_id)
        || !valid_id(&manifest.version_id)
        || !valid_digest(&manifest.content_digest)
        || manifest.total_bytes == 0
        || manifest.chunk_size == 0
        || !valid_id(&manifest.audience_scope)
        || !valid_id(&manifest.locality_region)
        || !valid_id(&policy.recipient_id)
        || !valid_id(&policy.permitted_audience_scope)
        || !valid_id(&policy.permitted_region)
        || policy.max_bytes == 0
        || request.chunks.is_empty()
        || request.chunks.len() > MAX_CHUNKS
        || request.chunks.iter().any(|chunk| {
            !valid_digest(&chunk.content_digest)
                || chunk.byte_len == 0
                || chunk.index as usize >= MAX_CHUNKS
        })
    {
        return Err(ExchangeApiError::InvalidRequest(
            "bounded manifest, policy, and chunk fields are required".into(),
        ));
    }
    Ok(())
}
impl ResearchObjectExchangeRecord {
    pub fn validate(&self) -> Result<(), ExchangeApiError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_id(&self.object_id)
            || !valid_id(&self.version_id)
            || !valid_id(&self.recipient_id)
        {
            return Err(ExchangeApiError::InvalidOutput(
                "exchange identity is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ExchangeApiError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ExchangeApiError::InvalidOutput(
                "exchange record digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Reconcile a resumable exchange cursor and emit a deterministic transfer record.
pub fn plan_glioma_research_object_exchange(
    request: &ResearchObjectExchangeRequest,
) -> Result<ResearchObjectExchangeRecord, ExchangeApiError> {
    validate_request(request)?;
    let manifest = &request.manifest;
    let policy = &request.policy;
    let mut blocking = BTreeSet::new();
    if !manifest.signed && policy.require_signed_manifest {
        blocking.insert("unsigned-manifest".into());
    }
    if !policy.grant_active {
        blocking.insert("transfer-grant-inactive".into());
    }
    if manifest.audience_scope != policy.permitted_audience_scope {
        blocking.insert("audience-scope-denied".into());
    }
    if manifest.locality_region != policy.permitted_region {
        blocking.insert("locality-region-denied".into());
    }
    if manifest.total_bytes > policy.max_bytes {
        blocking.insert("size-policy-exceeded".into());
    }
    let expected_chunks = manifest.total_bytes.div_ceil(manifest.chunk_size as u64) as u32;
    if expected_chunks as usize > MAX_CHUNKS {
        blocking.insert("chunk-count-exceeded".into());
    }
    let mut seen = BTreeSet::new();
    let mut acknowledged = Vec::new();
    let mut duplicates = Vec::new();
    let mut verified_bytes = 0_u64;
    for chunk in &request.chunks {
        if !seen.insert(chunk.index) {
            duplicates.push(chunk.index);
            blocking.insert(format!("duplicate-chunk-{}", chunk.index));
            continue;
        }
        if chunk.index >= expected_chunks {
            blocking.insert(format!("chunk-{}-out-of-range", chunk.index));
            continue;
        }
        if chunk.acknowledged {
            acknowledged.push(chunk.index);
            verified_bytes = verified_bytes.saturating_add(chunk.byte_len as u64);
        }
    }
    acknowledged.sort();
    duplicates.sort();
    let missing = (0..expected_chunks)
        .filter(|index| !acknowledged.contains(index))
        .collect::<Vec<_>>();
    let next_chunk = missing
        .iter()
        .copied()
        .next()
        .unwrap_or(expected_chunks)
        .max(request.resume_cursor);
    if acknowledged.iter().map(|_| 0_u64).sum::<u64>() == u64::MAX {
        blocking.insert("unreachable-overflow".into());
    }
    let disposition = if !blocking.is_empty() {
        ExchangeDisposition::Blocked
    } else if missing.is_empty() && verified_bytes >= manifest.total_bytes {
        ExchangeDisposition::Ready
    } else {
        ExchangeDisposition::Partial
    };
    let final_digest =
        matches!(disposition, ExchangeDisposition::Ready).then(|| manifest.content_digest.clone());
    let mut record = ResearchObjectExchangeRecord {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        object_id: manifest.object_id.clone(),
        version_id: manifest.version_id.clone(),
        recipient_id: policy.recipient_id.clone(),
        disposition,
        next_chunk,
        acknowledged_order: acknowledged,
        missing_order: missing,
        duplicate_order: duplicates,
        blocking_order: blocking.into_iter().collect(),
        verified_bytes,
        final_digest,
        digest: ContentHash::of_bytes(b"unsealed-glioma-exchange"),
    };
    record.digest = ContentHash::of_value(&digest_input(&record))
        .map_err(|error| ExchangeApiError::Digest(error.to_string()))?;
    record.validate()?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }
    fn request() -> ResearchObjectExchangeRequest {
        ResearchObjectExchangeRequest {
            manifest: ExchangeManifest {
                object_id: "object".into(),
                version_id: "v1".into(),
                content_digest: hash("object"),
                total_bytes: 8,
                chunk_size: 4,
                audience_scope: "consortium".into(),
                locality_region: "us".into(),
                signed: true,
            },
            chunks: vec![
                ExchangeChunk {
                    index: 0,
                    content_digest: hash("c0"),
                    byte_len: 4,
                    acknowledged: true,
                },
                ExchangeChunk {
                    index: 1,
                    content_digest: hash("c1"),
                    byte_len: 4,
                    acknowledged: true,
                },
            ],
            policy: ExchangePolicy {
                recipient_id: "archive".into(),
                permitted_audience_scope: "consortium".into(),
                permitted_region: "us".into(),
                max_bytes: 16,
                grant_active: true,
                require_signed_manifest: true,
            },
            resume_cursor: 0,
        }
    }
    #[test]
    fn complete_exchange_is_ready() {
        let record = plan_glioma_research_object_exchange(&request()).unwrap();
        assert_eq!(record.disposition, ExchangeDisposition::Ready);
        assert_eq!(record.final_digest, Some(hash("object")));
        record.validate().unwrap();
    }
    #[test]
    fn missing_chunk_is_partial_and_resumable() {
        let mut request = request();
        request.chunks[1].acknowledged = false;
        let record = plan_glioma_research_object_exchange(&request).unwrap();
        assert_eq!(record.disposition, ExchangeDisposition::Partial);
        assert_eq!(record.next_chunk, 1);
    }
}
