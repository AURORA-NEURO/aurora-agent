use crate::checkpoint::ValidatedCampaignCheckpoint;
use crate::model::CampaignActionKind;
use serde::{Deserialize, Serialize};

/// Caller-trusted durable campaign head. Supplying this separately from the checkpoint prevents
/// an older, otherwise valid snapshot from silently becoming current again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignCheckpointHead {
    campaign_id: String,
    spec_digest: String,
    generation: u64,
    snapshot_digest: String,
}

impl CampaignCheckpointHead {
    pub(crate) fn new(
        campaign_id: String,
        spec_digest: String,
        generation: u64,
        snapshot_digest: String,
    ) -> Self {
        Self {
            campaign_id,
            spec_digest,
            generation,
            snapshot_digest,
        }
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn spec_digest(&self) -> &str {
        &self.spec_digest
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn snapshot_digest(&self) -> &str {
        &self.snapshot_digest
    }
}

/// Exact atomic persistence claim a worker must acquire before an authorization may escape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CampaignAuthorizationClaim {
    expected_checkpoint_head: Option<CampaignCheckpointHead>,
    candidate_checkpoint_head: CampaignCheckpointHead,
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    action_ordinal: u16,
    authorization_digest: String,
    authorization_predecessor_digest: String,
}

impl CampaignAuthorizationClaim {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        expected_checkpoint_head: Option<CampaignCheckpointHead>,
        candidate_checkpoint_head: CampaignCheckpointHead,
        stage_id: String,
        kind: CampaignActionKind,
        input_digest: String,
        action_ordinal: u16,
        authorization_digest: String,
        authorization_predecessor_digest: String,
    ) -> Self {
        Self {
            expected_checkpoint_head,
            candidate_checkpoint_head,
            stage_id,
            kind,
            input_digest,
            action_ordinal,
            authorization_digest,
            authorization_predecessor_digest,
        }
    }

    /// Head that must still be current. `None` means atomic first creation.
    pub fn expected_checkpoint_head(&self) -> Option<&CampaignCheckpointHead> {
        self.expected_checkpoint_head.as_ref()
    }

    /// Head of the exact in-flight checkpoint that must be stored before dispatch.
    pub fn candidate_checkpoint_head(&self) -> &CampaignCheckpointHead {
        &self.candidate_checkpoint_head
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
}

/// Shared atomic persistence boundary for campaign dispatch authorization.
///
/// `compare_and_store_authorization` must atomically compare the current durable head with
/// `expected_head`, store the exact `candidate` payload, and advance the durable head to the
/// candidate head. `None` is an atomic create-if-absent operation, so two independently started
/// workers cannot both authorize the first action. The method must reject every later call from
/// the same predecessor, including a byte-identical candidate.
///
/// Returning `Err` never releases a dispatch token. If storage succeeded but its acknowledgement
/// was lost, the implementation may leave the candidate durable and still return `Err`; the
/// caller must then discard local state and restore that in-flight checkpoint, which enters
/// reconciliation instead of redispatching. Implementations must never turn an ambiguous write
/// into a second successful authorization response.
///
/// A process-local implementation coordinates only that process; multi-process callers must back
/// this trait with one shared durable transaction over payload and head.
pub trait CampaignCheckpointCoordinator {
    fn compare_and_store_authorization(
        &self,
        expected_head: Option<&CampaignCheckpointHead>,
        candidate: &ValidatedCampaignCheckpoint,
        claim: &CampaignAuthorizationClaim,
    ) -> Result<(), String>;
}
