use crate::error::{invalid_reconciliation_receipt, CampaignError};
use crate::kernel::digest_value;
use crate::model::{
    CampaignActionKind, CampaignReconciliationAuthorityDocument, MAX_CAMPAIGN_ID_BYTES,
    MAX_RECONCILIATION_AUTHORITY_ID_BYTES, MAX_RECONCILIATION_AUTHORITY_VERSION_BYTES,
    MAX_STAGE_ID_BYTES,
};
use bioprism_ids::{to_canonical_string, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const RESEARCH_CAMPAIGN_RECONCILIATION_SCHEMA: &str =
    "bioprism-research-campaign-reconciliation/0.1";
pub const RESEARCH_CAMPAIGN_RECONCILIATION_RETENTION: &str =
    "metadata_only_reconciliation;objective_artifact_evidence_provider_output_credentials_not_retained";
pub const MAX_CAMPAIGN_RECONCILIATION_RECEIPT_BYTES: usize = 65_536;

/// Journal conclusion for one uncertain action. Separate variants prevent absence evidence from
/// being omitted or an unknown execution from being represented as safe to retry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum CampaignReconciliationDecisionDocument {
    NotExecuted {
        absence_evidence_digest: String,
    },
    Succeeded {
        journal_receipt_digest: String,
        artifact_digest: String,
        native_receipt_digest: String,
    },
    Unknown {
        uncertainty_evidence_digest: String,
    },
}

/// Digest-bound journal query minted from the campaign's currently fenced authorization.
///
/// It is serializable for an out-of-process journal adapter, but its private fields mean callers
/// cannot manufacture one without a [`crate::ResearchCampaign`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampaignReconciliationQuery {
    campaign_id: String,
    spec_digest: String,
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    action_ordinal: u16,
    authorization_digest: String,
    authorization_predecessor_digest: String,
    authority: CampaignReconciliationAuthorityDocument,
}

impl CampaignReconciliationQuery {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        campaign_id: String,
        spec_digest: String,
        stage_id: String,
        kind: CampaignActionKind,
        input_digest: String,
        action_ordinal: u16,
        authorization_digest: String,
        authorization_predecessor_digest: String,
        authority: CampaignReconciliationAuthorityDocument,
    ) -> Self {
        Self {
            campaign_id,
            spec_digest,
            stage_id,
            kind,
            input_digest,
            action_ordinal,
            authorization_digest,
            authorization_predecessor_digest,
            authority,
        }
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn spec_digest(&self) -> &str {
        &self.spec_digest
    }

    pub fn stage_id(&self) -> &str {
        &self.stage_id
    }

    pub fn kind(&self) -> CampaignActionKind {
        self.kind
    }

    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }

    pub fn action_ordinal(&self) -> u16 {
        self.action_ordinal
    }

    pub fn authorization_digest(&self) -> &str {
        &self.authorization_digest
    }

    pub fn authorization_predecessor_digest(&self) -> &str {
        &self.authorization_predecessor_digest
    }

    pub fn authority(&self) -> &CampaignReconciliationAuthorityDocument {
        &self.authority
    }
}

/// Portable, self-digested journal receipt. The authority document selects the journal
/// configuration committed by the campaign specification; it does not itself authenticate this
/// document. Authentication and absence-proof policy belong to [`CampaignExecutionJournal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignReconciliationReceiptDocument {
    pub schema: String,
    pub campaign_id: String,
    pub spec_digest: String,
    pub stage_id: String,
    pub kind: CampaignActionKind,
    pub input_digest: String,
    pub action_ordinal: u16,
    pub authorization_digest: String,
    pub authorization_predecessor_digest: String,
    pub authority: CampaignReconciliationAuthorityDocument,
    pub journal_snapshot_digest: String,
    pub decision: CampaignReconciliationDecisionDocument,
    pub retention: String,
    pub secret_material: String,
    pub receipt_digest: String,
}

/// Caller-owned verifier for the execution journal selected by the campaign specification.
///
/// Implementations must verify the named snapshot and evidence against their durable journal.
/// In particular, an empty lookup is not evidence for `NotExecuted`; that variant requires a
/// positive, journal-specific absence proof named by `absence_evidence_digest`.
pub trait CampaignExecutionJournal {
    fn verify_reconciliation(
        &self,
        query: &CampaignReconciliationQuery,
        receipt: &CampaignReconciliationReceiptDocument,
    ) -> Result<(), String>;
}

/// A structurally valid, query-bound receipt accepted by the configured execution journal.
/// This token is intentionally neither cloneable nor deserializable and is consumed by the
/// campaign transition.
#[derive(Debug)]
pub struct ValidatedCampaignReconciliationReceipt {
    document: CampaignReconciliationReceiptDocument,
}

/// State transition produced by consuming a verified reconciliation receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignReconciliationResult {
    /// The journal still cannot distinguish execution from non-execution; no state changed.
    Unresolved,
    /// Positive absence evidence returned the stage to pending without refunding its attempt.
    Requeued,
    /// Absence was proved, but the charged attempt reached the campaign action ceiling.
    Exhausted,
    /// A matching native receipt settled the stage and produced this campaign status.
    Settled(crate::model::CampaignStatus),
}

impl ValidatedCampaignReconciliationReceipt {
    pub fn decision(&self) -> &CampaignReconciliationDecisionDocument {
        &self.document.decision
    }

    pub fn receipt_digest(&self) -> &str {
        &self.document.receipt_digest
    }

    pub(crate) fn document(&self) -> &CampaignReconciliationReceiptDocument {
        &self.document
    }
}

/// Build the canonical portable document a journal adapter may persist and return. Sealing is
/// content addressing only; [`verify_campaign_reconciliation`] still requires the configured
/// journal to accept the evidence before a campaign can consume it.
pub fn seal_campaign_reconciliation_receipt(
    query: &CampaignReconciliationQuery,
    journal_snapshot_digest: impl Into<String>,
    decision: CampaignReconciliationDecisionDocument,
) -> Result<Value, CampaignError> {
    let document = CampaignReconciliationReceiptDocument {
        schema: RESEARCH_CAMPAIGN_RECONCILIATION_SCHEMA.to_owned(),
        campaign_id: query.campaign_id.clone(),
        spec_digest: query.spec_digest.clone(),
        stage_id: query.stage_id.clone(),
        kind: query.kind,
        input_digest: query.input_digest.clone(),
        action_ordinal: query.action_ordinal,
        authorization_digest: query.authorization_digest.clone(),
        authorization_predecessor_digest: query.authorization_predecessor_digest.clone(),
        authority: query.authority.clone(),
        journal_snapshot_digest: journal_snapshot_digest.into(),
        decision,
        retention: RESEARCH_CAMPAIGN_RECONCILIATION_RETENTION.to_owned(),
        secret_material: "never_returned".to_owned(),
        receipt_digest: String::new(),
    };
    let mut value =
        serde_json::to_value(document).map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })?;
    let body = value
        .as_object_mut()
        .expect("reconciliation receipt serializes as an object");
    body.remove("receipt_digest");
    let receipt_digest = digest_value(&value)?;
    value
        .as_object_mut()
        .expect("reconciliation receipt body is an object")
        .insert("receipt_digest".to_owned(), Value::String(receipt_digest));
    validate_reconciliation_document(&value).map(|_| value)
}

/// Validate exact schema, bounds, content digest, live query lineage, and the caller-owned
/// execution journal. A syntactically valid receipt is not enough to authorize a transition.
pub fn verify_campaign_reconciliation<J: CampaignExecutionJournal>(
    query: &CampaignReconciliationQuery,
    value: &Value,
    journal: &J,
) -> Result<ValidatedCampaignReconciliationReceipt, CampaignError> {
    let document = validate_reconciliation_document(value)?;
    if document.campaign_id != query.campaign_id
        || document.spec_digest != query.spec_digest
        || document.stage_id != query.stage_id
        || document.kind != query.kind
        || document.input_digest != query.input_digest
        || document.action_ordinal != query.action_ordinal
        || document.authorization_digest != query.authorization_digest
        || document.authorization_predecessor_digest != query.authorization_predecessor_digest
        || document.authority != query.authority
    {
        return Err(CampaignError::StaleReconciliationReceipt);
    }
    journal
        .verify_reconciliation(query, &document)
        .map_err(|reason| CampaignError::ReconciliationJournalRejected { reason })?;
    Ok(ValidatedCampaignReconciliationReceipt { document })
}

fn validate_reconciliation_document(
    value: &Value,
) -> Result<CampaignReconciliationReceiptDocument, CampaignError> {
    let canonical =
        to_canonical_string(value).map_err(|error| CampaignError::Canonicalisation {
            reason: error.to_string(),
        })?;
    if canonical.len() > MAX_CAMPAIGN_RECONCILIATION_RECEIPT_BYTES {
        return Err(invalid_reconciliation_receipt(
            "receipt exceeds its byte ceiling",
        ));
    }
    let document: CampaignReconciliationReceiptDocument = serde_json::from_value(value.clone())
        .map_err(|error| {
            invalid_reconciliation_receipt(format!(
                "document does not match the exact schema: {error}"
            ))
        })?;
    if document.schema != RESEARCH_CAMPAIGN_RECONCILIATION_SCHEMA
        || document.retention != RESEARCH_CAMPAIGN_RECONCILIATION_RETENTION
        || document.secret_material != "never_returned"
    {
        return Err(invalid_reconciliation_receipt(
            "schema or metadata-retention markers are invalid",
        ));
    }
    bounded_text(&document.campaign_id, "campaign_id", MAX_CAMPAIGN_ID_BYTES)?;
    bounded_text(&document.stage_id, "stage_id", MAX_STAGE_ID_BYTES)?;
    bounded_text(
        &document.authority.authority_id,
        "authority.authority_id",
        MAX_RECONCILIATION_AUTHORITY_ID_BYTES,
    )?;
    bounded_text(
        &document.authority.protocol_version,
        "authority.protocol_version",
        MAX_RECONCILIATION_AUTHORITY_VERSION_BYTES,
    )?;
    if document.action_ordinal == 0 {
        return Err(invalid_reconciliation_receipt(
            "action_ordinal must be positive",
        ));
    }
    for (digest, field) in [
        (&document.spec_digest, "spec_digest"),
        (&document.input_digest, "input_digest"),
        (&document.authorization_digest, "authorization_digest"),
        (
            &document.authorization_predecessor_digest,
            "authorization_predecessor_digest",
        ),
        (&document.authority.config_digest, "authority.config_digest"),
        (&document.journal_snapshot_digest, "journal_snapshot_digest"),
        (&document.receipt_digest, "receipt_digest"),
    ] {
        require_digest(digest, field)?;
    }
    match &document.decision {
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest,
        } => require_digest(absence_evidence_digest, "absence_evidence_digest")?,
        CampaignReconciliationDecisionDocument::Succeeded {
            journal_receipt_digest,
            artifact_digest,
            native_receipt_digest,
        } => {
            require_digest(journal_receipt_digest, "journal_receipt_digest")?;
            require_digest(artifact_digest, "artifact_digest")?;
            require_digest(native_receipt_digest, "native_receipt_digest")?;
        }
        CampaignReconciliationDecisionDocument::Unknown {
            uncertainty_evidence_digest,
        } => require_digest(uncertainty_evidence_digest, "uncertainty_evidence_digest")?,
    }

    let mut body = value.clone();
    body.as_object_mut()
        .ok_or_else(|| invalid_reconciliation_receipt("receipt must be an object"))?
        .remove("receipt_digest");
    if digest_value(&body)? != document.receipt_digest {
        return Err(invalid_reconciliation_receipt(
            "receipt_digest does not match the receipt body",
        ));
    }
    Ok(document)
}

fn require_digest(value: &str, field: &str) -> Result<(), CampaignError> {
    ContentHash::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| {
            invalid_reconciliation_receipt(format!("{field} must be a lowercase SHA-256 digest"))
        })
}

fn bounded_text(value: &str, field: &str, maximum: usize) -> Result<(), CampaignError> {
    if value.trim().is_empty() || value.contains('\0') || value.len() > maximum {
        return Err(invalid_reconciliation_receipt(format!(
            "{field} must be non-empty, NUL-free text of at most {maximum} bytes"
        )));
    }
    Ok(())
}
