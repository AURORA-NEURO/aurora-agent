//! Quorum-aware consortium publication and correction coordination for preclinical glioma objects.
//!
//! Each institution keeps authority over its own signed decision. This planner reconciles only
//! explicit, digest-bound approvals, rejections, abstentions, and pending responses; it preserves
//! dissent and never rewrites a site's result or publishes bytes.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaConsortiumPublicationState1@1";
pub const MAX_SITES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitePublicationDecision {
    Approve,
    Reject,
    Abstain,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SitePublicationInput {
    pub site_id: String,
    pub object_digest: ContentHash,
    pub decision: SitePublicationDecision,
    pub authority_active: bool,
    pub authority_revoked: bool,
    pub signed: bool,
    pub dissent_reason: Option<String>,
    pub correction_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsortiumPublicationRequest {
    pub candidate_id: String,
    pub object_digest: ContentHash,
    pub sites: Vec<SitePublicationInput>,
    pub required_quorum: u16,
    pub allow_partial_publication: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsortiumPublicationDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsortiumSiteDecision {
    pub site_id: String,
    pub decision: SitePublicationDecision,
    pub counted_for_quorum: bool,
    pub accepted_digest: bool,
    pub dissent_preserved: bool,
    pub finding_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsortiumPublicationState {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_id: String,
    pub site_decisions: Vec<ConsortiumSiteDecision>,
    pub approval_order: Vec<String>,
    pub rejection_order: Vec<String>,
    pub pending_order: Vec<String>,
    pub dissent_order: Vec<String>,
    pub correction_digest_order: Vec<ContentHash>,
    pub quorum: u16,
    pub disposition: ConsortiumPublicationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConsortiumPublicationError {
    #[error("consortium publication request is invalid: {0}")]
    InvalidRequest(String),
    #[error("consortium publication output is invalid: {0}")]
    InvalidOutput(String),
    #[error("consortium publication digest failed: {0}")]
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
fn digest_input(state: &ConsortiumPublicationState) -> serde_json::Value {
    serde_json::json!({"feature_id":state.feature_id,"output_schema":state.output_schema,"candidate_id":state.candidate_id,"site_decisions":state.site_decisions,"approval_order":state.approval_order,"rejection_order":state.rejection_order,"pending_order":state.pending_order,"dissent_order":state.dissent_order,"correction_digest_order":state.correction_digest_order,"quorum":state.quorum,"disposition":state.disposition})
}
fn validate_request(
    request: &ConsortiumPublicationRequest,
) -> Result<(), ConsortiumPublicationError> {
    if !valid_id(&request.candidate_id)
        || !valid_digest(&request.object_digest)
        || request.sites.is_empty()
        || request.sites.len() > MAX_SITES
        || request.required_quorum == 0
        || request.required_quorum as usize > request.sites.len()
        || request.sites.iter().any(|site| {
            !valid_id(&site.site_id)
                || !valid_digest(&site.object_digest)
                || site
                    .correction_digest
                    .as_ref()
                    .is_some_and(|digest| !valid_digest(digest))
                || site
                    .dissent_reason
                    .as_ref()
                    .is_some_and(|reason| reason.is_empty())
        })
    {
        return Err(ConsortiumPublicationError::InvalidRequest(
            "bounded candidate, site, digest, quorum, and dissent fields are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if request
        .sites
        .iter()
        .any(|site| !ids.insert(site.site_id.clone()))
    {
        return Err(ConsortiumPublicationError::InvalidRequest(
            "site identifiers must be unique".into(),
        ));
    }
    Ok(())
}
impl ConsortiumPublicationState {
    pub fn validate(&self) -> Result<(), ConsortiumPublicationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_id(&self.candidate_id)
            || self
                .site_decisions
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
        {
            return Err(ConsortiumPublicationError::InvalidOutput(
                "consortium state identity or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ConsortiumPublicationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ConsortiumPublicationError::InvalidOutput(
                "consortium state digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Reconcile independent site publication decisions into a quorum/audit state.
pub fn steward_glioma_consortium_publication(
    request: &ConsortiumPublicationRequest,
) -> Result<ConsortiumPublicationState, ConsortiumPublicationError> {
    validate_request(request)?;
    let mut site_decisions = Vec::new();
    let mut approvals = Vec::new();
    let mut rejections = Vec::new();
    let mut pending = Vec::new();
    let mut dissent = Vec::new();
    let mut corrections = Vec::new();
    for site in &request.sites {
        let mut findings = BTreeSet::new();
        let accepted_digest = site.object_digest == request.object_digest;
        if !accepted_digest {
            findings.insert("object-digest-mismatch".into());
        }
        if !site.authority_active || site.authority_revoked {
            findings.insert("authority-revoked-or-inactive".into());
        }
        if !site.signed {
            findings.insert("unsigned-site-decision".into());
        }
        let counted = accepted_digest
            && site.authority_active
            && !site.authority_revoked
            && site.signed
            && matches!(site.decision, SitePublicationDecision::Approve);
        if counted {
            approvals.push(site.site_id.clone());
        }
        if matches!(site.decision, SitePublicationDecision::Reject) {
            rejections.push(site.site_id.clone());
        }
        if matches!(
            site.decision,
            SitePublicationDecision::Pending | SitePublicationDecision::Abstain
        ) {
            pending.push(site.site_id.clone());
        }
        let dissent_preserved = site.dissent_reason.is_some()
            || matches!(
                site.decision,
                SitePublicationDecision::Reject | SitePublicationDecision::Abstain
            );
        if dissent_preserved {
            dissent.push(site.site_id.clone());
        }
        if let Some(correction) = &site.correction_digest {
            corrections.push(correction.clone());
        }
        site_decisions.push(ConsortiumSiteDecision {
            site_id: site.site_id.clone(),
            decision: site.decision,
            counted_for_quorum: counted,
            accepted_digest,
            dissent_preserved,
            finding_order: findings.into_iter().collect(),
        });
    }
    site_decisions.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    approvals.sort();
    rejections.sort();
    pending.sort();
    dissent.sort();
    corrections.sort();
    corrections.dedup();
    let disposition = if approvals.len() >= request.required_quorum as usize
        && (pending.is_empty() || request.allow_partial_publication)
    {
        if pending.is_empty() {
            ConsortiumPublicationDisposition::Ready
        } else {
            ConsortiumPublicationDisposition::Partial
        }
    } else {
        ConsortiumPublicationDisposition::Blocked
    };
    let mut state = ConsortiumPublicationState {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_id: request.candidate_id.clone(),
        site_decisions,
        approval_order: approvals,
        rejection_order: rejections,
        pending_order: pending,
        dissent_order: dissent,
        correction_digest_order: corrections,
        quorum: request.required_quorum,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-consortium-publication"),
    };
    state.digest = ContentHash::of_value(&digest_input(&state))
        .map_err(|error| ConsortiumPublicationError::Digest(error.to_string()))?;
    state.validate()?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }
    #[test]
    fn quorum_and_dissent_are_preserved() {
        let digest = hash("object");
        let request = ConsortiumPublicationRequest {
            candidate_id: "candidate".into(),
            object_digest: digest.clone(),
            sites: vec![
                SitePublicationInput {
                    site_id: "site-a".into(),
                    object_digest: digest.clone(),
                    decision: SitePublicationDecision::Approve,
                    authority_active: true,
                    authority_revoked: false,
                    signed: true,
                    dissent_reason: None,
                    correction_digest: None,
                },
                SitePublicationInput {
                    site_id: "site-b".into(),
                    object_digest: digest,
                    decision: SitePublicationDecision::Reject,
                    authority_active: true,
                    authority_revoked: false,
                    signed: true,
                    dissent_reason: Some("replication concern".into()),
                    correction_digest: None,
                },
            ],
            required_quorum: 1,
            allow_partial_publication: false,
        };
        let state = steward_glioma_consortium_publication(&request).unwrap();
        assert_eq!(state.disposition, ConsortiumPublicationDisposition::Ready);
        assert_eq!(state.dissent_order, vec!["site-b"]);
        state.validate().unwrap();
    }
    #[test]
    fn revoked_approval_cannot_count() {
        let digest = hash("object");
        let mut site = SitePublicationInput {
            site_id: "site-a".into(),
            object_digest: digest.clone(),
            decision: SitePublicationDecision::Approve,
            authority_active: true,
            authority_revoked: true,
            signed: true,
            dissent_reason: None,
            correction_digest: None,
        };
        let request = ConsortiumPublicationRequest {
            candidate_id: "candidate".into(),
            object_digest: digest,
            sites: vec![site.clone()],
            required_quorum: 1,
            allow_partial_publication: false,
        };
        let state = steward_glioma_consortium_publication(&request).unwrap();
        assert_eq!(state.disposition, ConsortiumPublicationDisposition::Blocked);
        site.authority_revoked = false;
    }
}
