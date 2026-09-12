use bioprism_ids::ContentHash;
use bioprism_research_campaign::{
    restore_campaign, seal_campaign_checkpoint, start_campaign, validate_campaign_checkpoint,
    CampaignActionKind, CampaignAuthorizationClaim, CampaignCheckpointCoordinator,
    CampaignCheckpointHead, CampaignError, CampaignReconciliationAuthorityDocument,
    CampaignStageDocument, CampaignStatus, ResearchCampaignSpec, ResearchCampaignSpecDocument,
    ValidatedCampaignCheckpoint, VerifiedCampaignReceipt,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

fn digest(value: &Value) -> String {
    ContentHash::of_value(value)
        .expect("test value canonicalises")
        .to_string()
}

fn spec(campaign_id: &str, stages: Vec<CampaignStageDocument>) -> ResearchCampaignSpec {
    let max_actions = stages.len() as u16;
    ResearchCampaignSpec::try_from(ResearchCampaignSpecDocument {
        campaign_id: campaign_id.to_owned(),
        objective: "exercise the durable authorization boundary".to_owned(),
        reconciliation_authority: CampaignReconciliationAuthorityDocument {
            authority_id: "test-journal".to_owned(),
            protocol_version: "0.1".to_owned(),
            config_digest: digest(&json!({ "journal": "coordination-recovery" })),
        },
        stages,
        max_actions,
    })
    .expect("test spec validates")
}

fn stage(stage_id: &str, input_digest: &str) -> CampaignStageDocument {
    CampaignStageDocument {
        stage_id: stage_id.to_owned(),
        kind: CampaignActionKind::BrainPlan,
        input_digest: input_digest.to_owned(),
        depends_on: Vec::new(),
    }
}

#[derive(Default)]
struct DurableCoordinator {
    checkpoint: Mutex<Option<ValidatedCampaignCheckpoint>>,
    claims: Mutex<Vec<Value>>,
    lose_acknowledgement: AtomicBool,
    reject_before_write: AtomicBool,
}

impl DurableCoordinator {
    fn losing_acknowledgement() -> Self {
        Self {
            lose_acknowledgement: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn rejecting() -> Self {
        Self {
            reject_before_write: AtomicBool::new(true),
            ..Self::default()
        }
    }

    fn checkpoint(&self) -> Option<ValidatedCampaignCheckpoint> {
        self.checkpoint
            .lock()
            .expect("test checkpoint lock is healthy")
            .clone()
    }

    fn claims(&self) -> Vec<Value> {
        self.claims
            .lock()
            .expect("test claim lock is healthy")
            .clone()
    }
}

impl CampaignCheckpointCoordinator for DurableCoordinator {
    fn compare_and_store_authorization(
        &self,
        expected_head: Option<&CampaignCheckpointHead>,
        candidate: &ValidatedCampaignCheckpoint,
        claim: &CampaignAuthorizationClaim,
    ) -> Result<(), String> {
        if claim.expected_checkpoint_head() != expected_head
            || claim.candidate_checkpoint_head() != &candidate.head()
        {
            return Err("claim does not describe the requested checkpoint transition".to_owned());
        }
        if self.reject_before_write.load(Ordering::SeqCst) {
            return Err("injected persistence rejection".to_owned());
        }
        let mut current = self
            .checkpoint
            .lock()
            .map_err(|_| "test checkpoint lock was poisoned".to_owned())?;
        let observed_head = current.as_ref().map(ValidatedCampaignCheckpoint::head);
        if observed_head.as_ref() != expected_head {
            return Err("durable checkpoint head changed".to_owned());
        }
        self.claims
            .lock()
            .map_err(|_| "test claim lock was poisoned".to_owned())?
            .push(
                serde_json::to_value(claim)
                    .map_err(|error| format!("claim did not serialize: {error}"))?,
            );
        *current = Some(candidate.clone());
        if self.lose_acknowledgement.swap(false, Ordering::SeqCst) {
            return Err("injected acknowledgement loss after durable commit".to_owned());
        }
        Ok(())
    }
}

#[test]
fn two_fresh_workers_cannot_both_create_the_first_in_flight_checkpoint() {
    let input = "a".repeat(64);
    let spec = spec("fresh-worker-fence", vec![stage("plan", &input)]);
    let mut first = start_campaign(spec.clone()).expect("first campaign starts");
    let mut second = start_campaign(spec).expect("second campaign starts");
    let coordinator = DurableCoordinator::default();

    let authorization = first
        .authorize_next_action(&coordinator)
        .expect("first worker atomically stores its authorization");
    assert_eq!(authorization.stage_id(), "plan");
    assert_eq!(first.status(), CampaignStatus::InFlight);
    assert!(matches!(
        second.authorize_next_action(&coordinator),
        Err(CampaignError::AuthorizationCheckpointRejected { .. })
    ));
    assert_eq!(second.status(), CampaignStatus::Planned);
    assert_eq!(second.actions_used(), 0);

    let durable = coordinator
        .checkpoint()
        .expect("winner checkpoint is durable");
    assert_eq!(durable.as_value()["status"], json!("in_flight"));
    assert_eq!(durable.generation(), 1);
    let claims = coordinator.claims();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0]["expected_checkpoint_head"], Value::Null);
    assert_eq!(
        claims[0]["candidate_checkpoint_head"],
        json!(durable.head())
    );
    assert_eq!(claims[0]["stage_id"], json!("plan"));
    assert_eq!(claims[0]["kind"], json!("brain_plan"));
    assert_eq!(claims[0]["input_digest"], json!(input));
    assert_eq!(claims[0]["action_ordinal"], json!(1));
    assert_eq!(
        claims[0]["authorization_digest"],
        json!(authorization.authorization_digest())
    );
    assert_eq!(
        claims[0]["authorization_predecessor_digest"],
        json!(digest(&json!({
            "campaign_id": "fresh-worker-fence",
            "spec_digest": first.spec().spec_digest(),
            "events": [],
        })))
    );
}

#[test]
fn lost_storage_acknowledgement_releases_no_token_and_restores_only_for_reconciliation() {
    let input = "b".repeat(64);
    let spec = spec("lost-store-ack", vec![stage("plan", &input)]);
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let coordinator = DurableCoordinator::losing_acknowledgement();

    assert!(matches!(
        campaign.authorize_next_action(&coordinator),
        Err(CampaignError::AuthorizationCheckpointRejected { .. })
    ));
    assert_eq!(campaign.status(), CampaignStatus::Planned);
    assert_eq!(campaign.actions_used(), 0);
    assert_eq!(campaign.active_stage_id(), None);

    let durable = coordinator
        .checkpoint()
        .expect("the injected lost acknowledgement happened after durable storage");
    assert_eq!(durable.as_value()["status"], json!("in_flight"));
    let restored = restore_campaign(spec, &durable, &durable.head(), Vec::new())
        .expect("durable candidate restores");
    assert_eq!(restored.status(), CampaignStatus::ReconciliationRequired);
    assert_eq!(restored.active_stage_id(), Some("plan"));
}

#[test]
fn rejected_checkpoint_transaction_leaves_no_authorization_or_local_mutation() {
    let input = "c".repeat(64);
    let spec = spec("rejected-store", vec![stage("plan", &input)]);
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let coordinator = DurableCoordinator::rejecting();

    assert!(matches!(
        campaign.authorize_next_action(&coordinator),
        Err(CampaignError::AuthorizationCheckpointRejected { .. })
    ));
    assert_eq!(campaign.status(), CampaignStatus::Planned);
    assert_eq!(campaign.actions_used(), 0);
    assert_eq!(campaign.active_stage_id(), None);
    assert!(coordinator.checkpoint().is_none());
    assert!(coordinator.claims().is_empty());
}

fn restamp_checkpoint(value: &mut Value) {
    value
        .as_object_mut()
        .expect("checkpoint is an object")
        .remove("snapshot_digest");
    let stamped = digest(value);
    value["snapshot_digest"] = json!(stamped);
}

#[test]
fn replay_rejects_an_authorization_after_a_missing_input_pause_even_when_restamped() {
    let first_input = "d".repeat(64);
    let second_input = "e".repeat(64);
    let spec = spec(
        "terminal-replay-fence",
        vec![stage("blocked", &first_input), stage("next", &second_input)],
    );
    let coordinator = DurableCoordinator::default();
    let receipt = VerifiedCampaignReceipt::missing_input(
        spec.stage("blocked").expect("stage exists"),
        digest(&json!({ "observation": "source absent" })),
    )
    .expect("missing-input receipt validates");
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&coordinator)
        .expect("first action is durably authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("missing input pauses the campaign");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("paused checkpoint seals");
    let mut forged = checkpoint.as_value().clone();
    let predecessor = forged["event_chain_digest"]
        .as_str()
        .expect("chain digest is text")
        .to_owned();
    let spec_digest = forged["spec_digest"]
        .as_str()
        .expect("spec digest is text")
        .to_owned();
    let authorization_digest = digest(&json!({
        "campaign_id": "terminal-replay-fence",
        "spec_digest": spec_digest,
        "stage_id": "next",
        "kind": "brain_plan",
        "input_digest": second_input,
        "action_ordinal": 2,
        "preceding_event_chain_digest": predecessor,
    }));
    let mut forged_event = json!({
        "ordinal": 3,
        "action_ordinal": 2,
        "stage_id": "next",
        "kind": "brain_plan",
        "input_digest": second_input,
        "transition": "authorized",
        "authorization_digest": authorization_digest,
        "disposition": null,
        "artifact_digest": null,
        "detail_digest": null,
        "reconciliation_receipt_digest": null,
        "previous_event_digest": predecessor,
    });
    let event_digest = digest(&forged_event);
    forged_event["event_digest"] = json!(event_digest);
    forged["events"]
        .as_array_mut()
        .expect("events are an array")
        .push(forged_event);
    let stage = forged["stages"]
        .as_array_mut()
        .expect("stages are an array")
        .iter_mut()
        .find(|row| row["stage_id"] == json!("next"))
        .expect("next stage exists");
    stage["state"] = json!("in_flight");
    stage["action_ordinal"] = json!(2);
    stage["authorization_digest"] = json!(authorization_digest);
    forged["actions_used"] = json!(2);
    forged["active_stage_id"] = json!("next");
    forged["event_chain_digest"] = json!(event_digest);
    forged["status"] = json!("in_flight");
    restamp_checkpoint(&mut forged);

    assert!(matches!(
        validate_campaign_checkpoint(&forged),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}
