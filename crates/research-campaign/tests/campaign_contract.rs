use bioprism_autopilot::{
    plan_next_action, AttemptKind, AttemptRecord, AutonomyGrant, DriveHistory, NextAction,
};
use bioprism_brain::{AutonomousPlanRequest, PlanEffect, PlanStep};
use bioprism_ids::{to_canonical_string, ContentHash};
use bioprism_research::{run_research, ResearchRequest, WorldFamily};
use bioprism_research_campaign::{
    restore_campaign, seal_campaign_checkpoint, seal_campaign_reconciliation_receipt,
    start_campaign, validate_campaign_checkpoint, verify_campaign_reconciliation,
    CampaignActionKind, CampaignAuthorizationClaim, CampaignCheckpointCoordinator,
    CampaignCheckpointHead, CampaignError, CampaignExecutionJournal, CampaignReceiptDisposition,
    CampaignReconciliationAuthorityDocument, CampaignReconciliationDecisionDocument,
    CampaignReconciliationQuery, CampaignReconciliationReceiptDocument,
    CampaignReconciliationResult, CampaignStageDocument, CampaignStatus, ResearchCampaign,
    ResearchCampaignSpec, ResearchCampaignSpecDocument, ValidatedCampaignCheckpoint,
    ValidatedCampaignReconciliationReceipt, VerifiedCampaignReceipt, MAX_CAMPAIGN_ACTIONS,
};
use serde_json::{json, Value};
use std::sync::Mutex;

#[cfg(not(feature = "neurosurgery-adapter"))]
use bioprism_research_campaign::CampaignAdapterAvailability;

fn digest(value: &Value) -> String {
    ContentHash::of_value(value)
        .expect("test value canonicalises")
        .to_string()
}

fn authority(label: &str) -> CampaignReconciliationAuthorityDocument {
    CampaignReconciliationAuthorityDocument {
        authority_id: "test-execution-journal".to_owned(),
        protocol_version: "0.1".to_owned(),
        config_digest: digest(&json!({ "journal_configuration": label })),
    }
}

fn stage(
    stage_id: &str,
    kind: CampaignActionKind,
    input_digest: &str,
    depends_on: &[&str],
) -> CampaignStageDocument {
    CampaignStageDocument {
        stage_id: stage_id.to_owned(),
        kind,
        input_digest: input_digest.to_owned(),
        depends_on: depends_on.iter().map(|value| (*value).to_owned()).collect(),
    }
}

fn spec_with(
    campaign_id: &str,
    objective: &str,
    stages: Vec<CampaignStageDocument>,
) -> ResearchCampaignSpec {
    let stage_count = stages.len() as u16;
    spec_with_max(campaign_id, objective, stages, stage_count)
}

fn spec_with_max(
    campaign_id: &str,
    objective: &str,
    stages: Vec<CampaignStageDocument>,
    max_actions: u16,
) -> ResearchCampaignSpec {
    ResearchCampaignSpec::try_from(ResearchCampaignSpecDocument {
        campaign_id: campaign_id.to_owned(),
        objective: objective.to_owned(),
        reconciliation_authority: authority("default"),
        stages,
        max_actions,
    })
    .expect("test campaign specification validates")
}

struct AcceptingJournal;

impl CampaignExecutionJournal for AcceptingJournal {
    fn verify_reconciliation(
        &self,
        query: &CampaignReconciliationQuery,
        receipt: &CampaignReconciliationReceiptDocument,
    ) -> Result<(), String> {
        if receipt.authorization_digest != query.authorization_digest() {
            return Err("journal row does not match the requested authorization".to_owned());
        }
        Ok(())
    }
}

struct EmptyJournal;

impl CampaignExecutionJournal for EmptyJournal {
    fn verify_reconciliation(
        &self,
        _query: &CampaignReconciliationQuery,
        _receipt: &CampaignReconciliationReceiptDocument,
    ) -> Result<(), String> {
        Err("no journal row is not positive absence evidence".to_owned())
    }
}

struct AcceptingCoordinator;

impl CampaignCheckpointCoordinator for AcceptingCoordinator {
    fn compare_and_store_authorization(
        &self,
        expected_head: Option<&CampaignCheckpointHead>,
        candidate: &ValidatedCampaignCheckpoint,
        claim: &CampaignAuthorizationClaim,
    ) -> Result<(), String> {
        if claim.expected_checkpoint_head() != expected_head {
            return Err("authorization claim does not bind the expected head".to_owned());
        }
        if claim.candidate_checkpoint_head() != &candidate.head() {
            return Err("authorization claim does not bind the candidate checkpoint".to_owned());
        }
        Ok(())
    }
}

struct CoordinatorState {
    current_head: Option<CampaignCheckpointHead>,
    stored_checkpoint: Option<Value>,
}

struct OneShotCoordinator {
    state: Mutex<CoordinatorState>,
}

impl OneShotCoordinator {
    fn for_head(expected_head: CampaignCheckpointHead) -> Self {
        Self {
            state: Mutex::new(CoordinatorState {
                current_head: Some(expected_head),
                stored_checkpoint: None,
            }),
        }
    }
}

impl CampaignCheckpointCoordinator for OneShotCoordinator {
    fn compare_and_store_authorization(
        &self,
        expected_head: Option<&CampaignCheckpointHead>,
        candidate: &ValidatedCampaignCheckpoint,
        claim: &CampaignAuthorizationClaim,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "test coordinator lock was poisoned".to_owned())?;
        if state.current_head.as_ref() != expected_head {
            return Err("checkpoint head is no longer current".to_owned());
        }
        if claim.expected_checkpoint_head() != expected_head {
            return Err("authorization claim does not bind the expected head".to_owned());
        }
        let candidate_head = candidate.head();
        if claim.candidate_checkpoint_head() != &candidate_head {
            return Err("authorization claim does not bind the candidate checkpoint".to_owned());
        }
        state.stored_checkpoint = Some(candidate.as_value().clone());
        state.current_head = Some(candidate_head);
        Ok(())
    }
}

fn reconciliation_value(
    campaign: &ResearchCampaign,
    decision: CampaignReconciliationDecisionDocument,
) -> Value {
    let query = campaign
        .reconciliation_query()
        .expect("campaign exposes its fenced authorization");
    seal_campaign_reconciliation_receipt(&query, observation_digest("journal snapshot"), decision)
        .expect("reconciliation receipt seals")
}

fn verified_reconciliation(
    campaign: &ResearchCampaign,
    decision: CampaignReconciliationDecisionDocument,
) -> ValidatedCampaignReconciliationReceipt {
    let query = campaign
        .reconciliation_query()
        .expect("campaign exposes its fenced authorization");
    let value = seal_campaign_reconciliation_receipt(
        &query,
        observation_digest("journal snapshot"),
        decision,
    )
    .expect("reconciliation receipt seals");
    verify_campaign_reconciliation(&query, &value, &AcceptingJournal)
        .expect("configured journal accepts the receipt")
}

fn restore_after_lost_acknowledgement(spec: &ResearchCampaignSpec) -> ResearchCampaign {
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    let checkpoint =
        seal_campaign_checkpoint(&mut campaign).expect("in-flight campaign checkpoints");
    drop(authorization);
    restore_campaign(spec.clone(), &checkpoint, &checkpoint.head(), Vec::new())
        .expect("campaign restores behind reconciliation fence")
}

fn observation_digest(label: &str) -> String {
    digest(&json!({ "observation": label }))
}

fn research_fixture() -> (ResearchRequest, Value) {
    let request: ResearchRequest = serde_json::from_value(json!({
        "research_id": "campaign-small",
        "question": "Does the reference-like panel contain a negative result?",
        "family": WorldFamily::ReferenceLike,
        "distractor_points": [40],
        "seed": 11,
    }))
    .expect("research request validates");
    let dossier = run_research(&request).expect("small deterministic research run completes");
    (request, dossier)
}

fn autopilot_report(input_digest: &str, final_status: &str) -> Value {
    let mut report = json!({
        "schema": bioprism_autopilot::AUTOPILOT_REPORT_SCHEMA_VERSION,
        "base_mission_digest": input_digest,
        "attempts": [],
        "final_status": final_status,
        "limitations": bioprism_autopilot::REQUIRED_LIMITATIONS,
    });
    let report_digest = digest(&report);
    report["report_sha256"] = json!(report_digest);
    report
}

fn one_step_autopilot_history() -> (AutonomyGrant, DriveHistory, String) {
    let base_mission = json!({
        "mission_id": "campaign-autopilot",
        "goal": "produce one planner-verified terminal history",
        "steps": [{
            "id": "measure",
            "domain": "metrics",
            "capability": "analytics",
            "objective": "measure once",
            "tool": "measure_once",
            "arguments": {},
            "depends_on": [],
            "bindings": [],
            "required": true,
        }],
    });
    let input_digest = digest(&base_mission);
    let grant: AutonomyGrant = serde_json::from_value(json!({
        "allowed_tools": ["measure_once"],
        "max_attempts": 1,
        "require_reconciliation_complete": false,
    }))
    .expect("test grant validates");
    let mut history = DriveHistory::new(base_mission).expect("test mission validates");
    let dispatched = match plan_next_action(&grant, &history).expect("first action plans") {
        NextAction::DispatchFull { mission, .. } => mission,
        other => panic!("expected a full dispatch, got {other:?}"),
    };
    let report = json!({
        "schema_version": "bioprism-devplat-mission/0.1",
        "plan": {
            "schema_version": "bioprism-devplat-mission/0.1",
            "mission_id": "campaign-autopilot",
            "goal": "produce one planner-verified terminal history",
            "digest": digest(&dispatched),
            "step_count": 1,
            "ordered_steps": ["measure"],
            "waves": [["measure"]],
            "critical_path_length": 1,
            "steps": [{
                "id": "measure",
                "domain": "metrics",
                "capability": "analytics",
                "objective": "measure once",
                "tool": "measure_once",
                "depends_on": [],
                "bindings": [],
                "required": true,
                "wave": 0,
            }],
            "execution": "authorized",
            "execution_mode": "serial",
            "max_parallelism": 1,
            "guarantees": [],
            "limitations": [],
        },
        "execution": "executed",
        "mission_status": "succeeded",
        "succeeded": 1,
        "refused": 0,
        "blocked": 0,
        "cancelled": 0,
        "required_failures": 0,
        "returned_bytes": 0,
        "results": [{
            "id": "measure",
            "tool": "measure_once",
            "status": "succeeded",
            "required": true,
            "arguments_digest": "1".repeat(64),
            "bytes": 0,
            "wire": null,
            "error": null,
        }],
        "execution_trace_schema_version": "bioprism-devplat-mission-trace/0.1",
        "execution_trace": [],
        "claim_requests": [],
        "claim_lineage": {},
        "guarantees": [],
        "limitations": [],
    });
    history.push(
        AttemptRecord::delivered(
            AttemptKind::Full,
            dispatched,
            report,
            None,
            Some("the test grant does not require reconciliation".to_owned()),
        )
        .expect("delivered attempt validates"),
    );
    (grant, history, input_digest)
}

fn restamp_snapshot(value: &mut Value) {
    let mut body = value.clone();
    body.as_object_mut()
        .expect("checkpoint is an object")
        .remove("snapshot_digest");
    value["snapshot_digest"] = json!(digest(&body));
}

fn restamp_reconciliation_receipt(value: &mut Value) {
    let mut body = value.clone();
    body.as_object_mut()
        .expect("reconciliation receipt is an object")
        .remove("receipt_digest");
    value["receipt_digest"] = json!(digest(&body));
}

fn restamp_event_chain(value: &mut Value) {
    let events = value["events"]
        .as_array_mut()
        .expect("checkpoint events are an array");
    let mut previous: Option<String> = None;
    for (index, event) in events.iter_mut().enumerate() {
        event["ordinal"] = json!(index + 1);
        event["previous_event_digest"] = previous
            .as_ref()
            .map_or(Value::Null, |digest| json!(digest));
        let mut body = event.clone();
        body.as_object_mut()
            .expect("event is an object")
            .remove("event_digest");
        let event_digest = digest(&body);
        event["event_digest"] = json!(event_digest);
        previous = Some(event_digest);
    }
    value["event_chain_digest"] = json!(previous.expect("test retains at least one event"));
    restamp_snapshot(value);
}

fn succeeded_decision(receipt: &VerifiedCampaignReceipt) -> CampaignReconciliationDecisionDocument {
    CampaignReconciliationDecisionDocument::Succeeded {
        journal_receipt_digest: observation_digest("journal success row"),
        artifact_digest: receipt.artifact_digest().to_owned(),
        native_receipt_digest: receipt
            .projection_digest()
            .expect("native receipt projection digests"),
    }
}

#[test]
fn a_spec_rejects_unknown_dependencies_cycles_and_an_impossible_action_ceiling() {
    let input = "a".repeat(64);
    let unknown = ResearchCampaignSpec::try_from(ResearchCampaignSpecDocument {
        campaign_id: "unknown-dependency".into(),
        objective: "bounded objective".into(),
        reconciliation_authority: authority("default"),
        stages: vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input,
            &["absent"],
        )],
        max_actions: 1,
    });
    assert!(matches!(unknown, Err(CampaignError::InvalidSpec { .. })));

    let cycle = ResearchCampaignSpec::try_from(ResearchCampaignSpecDocument {
        campaign_id: "cycle".into(),
        objective: "bounded objective".into(),
        reconciliation_authority: authority("default"),
        stages: vec![
            stage("a", CampaignActionKind::BrainPlan, &input, &["b"]),
            stage("b", CampaignActionKind::AutopilotDrive, &input, &["a"]),
        ],
        max_actions: 2,
    });
    assert!(matches!(cycle, Err(CampaignError::InvalidSpec { .. })));

    let too_small = ResearchCampaignSpec::try_from(ResearchCampaignSpecDocument {
        campaign_id: "ceiling".into(),
        objective: "bounded objective".into(),
        reconciliation_authority: authority("default"),
        stages: vec![
            stage("a", CampaignActionKind::BrainPlan, &input, &[]),
            stage("b", CampaignActionKind::AutopilotDrive, &input, &["a"]),
        ],
        max_actions: 1,
    });
    assert!(matches!(too_small, Err(CampaignError::InvalidSpec { .. })));
    assert_eq!(MAX_CAMPAIGN_ACTIONS, 64);
}

#[test]
fn semantically_identical_dags_have_one_canonical_spec_identity() {
    let input = "9".repeat(64);
    let left = spec_with(
        "canonical-dag",
        "ordering is representation, not campaign meaning",
        vec![
            stage(
                "observe",
                CampaignActionKind::SyntheticResearch,
                &input,
                &[],
            ),
            stage("plan", CampaignActionKind::BrainPlan, &input, &["observe"]),
            stage(
                "drive",
                CampaignActionKind::AutopilotDrive,
                &input,
                &["plan", "observe"],
            ),
        ],
    );
    let right = spec_with(
        "canonical-dag",
        "ordering is representation, not campaign meaning",
        vec![
            stage(
                "drive",
                CampaignActionKind::AutopilotDrive,
                &input,
                &["observe", "plan"],
            ),
            stage("plan", CampaignActionKind::BrainPlan, &input, &["observe"]),
            stage(
                "observe",
                CampaignActionKind::SyntheticResearch,
                &input,
                &[],
            ),
        ],
    );

    assert_eq!(left.spec_digest(), right.spec_digest());
    assert_eq!(
        left.stages()
            .map(|stage| stage.stage_id())
            .collect::<Vec<_>>(),
        right
            .stages()
            .map(|stage| stage.stage_id())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        left.stage("drive").expect("drive exists").depends_on(),
        &["observe".to_owned(), "plan".to_owned()]
    );
}

#[test]
fn the_reconciliation_authority_is_part_of_the_canonical_spec_identity() {
    let input = "8".repeat(64);
    let mut left_document = ResearchCampaignSpecDocument {
        campaign_id: "authority-binding".to_owned(),
        objective: "bind the only journal allowed to resolve uncertain execution".to_owned(),
        reconciliation_authority: authority("journal-a"),
        stages: vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        max_actions: 1,
    };
    let left = ResearchCampaignSpec::try_from(left_document.clone()).expect("left spec validates");
    left_document.reconciliation_authority = authority("journal-b");
    let right = ResearchCampaignSpec::try_from(left_document).expect("right spec validates");

    assert_ne!(left.spec_digest(), right.spec_digest());
    assert_ne!(
        left.reconciliation_authority().config_digest,
        right.reconciliation_authority().config_digest
    );
}

#[test]
fn the_next_ready_stage_is_deterministic_and_only_one_authorization_can_be_live() {
    let input = "b".repeat(64);
    let spec = spec_with(
        "ordering",
        "choose one ready action",
        vec![
            stage("z", CampaignActionKind::BrainPlan, &input, &[]),
            stage("a", CampaignActionKind::AutopilotDrive, &input, &[]),
        ],
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("first ready action is authorized");
    assert_eq!(authorization.stage_id(), "a");
    assert_eq!(authorization.action_ordinal(), 1);
    assert!(matches!(
        campaign.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::ActionAlreadyInFlight)
    ));
}

#[cfg(not(feature = "neurosurgery-adapter"))]
#[test]
fn a_disabled_neurosurgery_adapter_refuses_before_mutating_or_completing() {
    let input = "c".repeat(64);
    assert_eq!(
        CampaignActionKind::NeurosurgeryResearch.adapter_availability(),
        CampaignAdapterAvailability::FeatureDisabled {
            required_feature: "neurosurgery-adapter"
        }
    );
    let spec = spec_with(
        "neuro-disabled",
        "domain research still requires its native verifier",
        vec![stage(
            "domain",
            CampaignActionKind::NeurosurgeryResearch,
            &input,
            &[],
        )],
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    assert!(matches!(
        campaign.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::AdapterUnavailable { .. })
    ));
    assert_eq!(campaign.status(), CampaignStatus::Planned);
    assert_eq!(campaign.actions_used(), 0);
}

#[test]
fn missing_input_and_unknown_completion_enter_distinct_fail_closed_states() {
    let input = "d".repeat(64);
    let base_stage = stage("plan", CampaignActionKind::BrainPlan, &input, &[]);

    let missing_spec = spec_with(
        "missing",
        "distinguish absent input",
        vec![base_stage.clone()],
    );
    let missing_receipt = VerifiedCampaignReceipt::missing_input(
        missing_spec.stage("plan").expect("stage exists"),
        observation_digest("source was absent"),
    )
    .expect("missing observation validates");
    let mut missing = start_campaign(missing_spec).expect("campaign starts");
    let authorization = missing
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    missing
        .apply_receipt(authorization, missing_receipt)
        .expect("missing receipt settles");
    assert_eq!(missing.status(), CampaignStatus::NeedsInput);

    let unknown_spec = spec_with("unknown", "distinguish uncertain effects", vec![base_stage]);
    let unknown_receipt = VerifiedCampaignReceipt::unknown_completion(
        unknown_spec.stage("plan").expect("stage exists"),
        observation_digest("dispatch acknowledgement was lost"),
    )
    .expect("unknown observation validates");
    let mut unknown = start_campaign(unknown_spec).expect("campaign starts");
    let authorization = unknown
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    unknown
        .apply_receipt(authorization, unknown_receipt)
        .expect("unknown receipt records uncertainty");
    assert_eq!(unknown.status(), CampaignStatus::ReconciliationRequired);
    assert_eq!(unknown.active_stage_id(), Some("plan"));
    assert!(unknown.reconciliation_query().is_ok());
    assert!(matches!(
        unknown.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::ActionAlreadyInFlight)
    ));
}

#[test]
fn a_verified_research_dossier_preserves_negative_findings_in_the_checkpoint() {
    let (request, dossier) = research_fixture();
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "negative-result",
        "retain a falsifying observation",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );
    let receipt = VerifiedCampaignReceipt::from_research_dossier(
        spec.stage("measure").expect("stage exists"),
        &dossier,
    )
    .expect("native verifier accepts the dossier");
    assert_eq!(
        receipt.disposition(),
        CampaignReceiptDisposition::CompletedWithNegativeFindings
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("verified receipt settles");
    assert_eq!(campaign.status(), CampaignStatus::Completed);
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("checkpoint seals");
    assert_eq!(
        checkpoint.as_value()["stages"][0]["receipt"]["disposition"],
        json!("completed_with_negative_findings")
    );
}

#[test]
fn a_restamped_empty_research_dossier_cannot_vacuously_succeed_the_campaign() {
    let (request, mut dossier) = research_fixture();
    dossier["steps"] = json!([]);
    dossier["findings"] = json!([]);
    dossier
        .as_object_mut()
        .expect("dossier is an object")
        .remove("dossier_sha256");
    dossier["dossier_sha256"] = json!(digest(&dossier));
    assert_eq!(
        bioprism_research::verify_dossier(&dossier)
            .expect("upstream verifier returns a projection")["valid"],
        json!(true),
        "the regression fixture must reach the campaign adapter's stronger gate"
    );
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "empty-dossier",
        "vacuous arrays are not completed research",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );

    assert!(matches!(
        VerifiedCampaignReceipt::from_research_dossier(
            spec.stage("measure").expect("stage exists"),
            &dossier,
        ),
        Err(CampaignError::InvalidReceipt { .. })
    ));
}

#[test]
fn a_restamped_nonempty_but_unexecuted_research_dossier_cannot_succeed() {
    let (request, mut dossier) = research_fixture();
    dossier["steps"] = json!([{
        "outcome": "completed",
        "outputs": [{ "sha256": "not-an-artifact-digest" }],
    }]);
    dossier["findings"] = json!([{
        "level": "observation",
        "supported_by": ["not-an-artifact-digest"],
        "negative": false,
    }]);
    dossier
        .as_object_mut()
        .expect("dossier is an object")
        .remove("dossier_sha256");
    dossier["dossier_sha256"] = json!(digest(&dossier));
    assert_eq!(
        bioprism_research::verify_dossier(&dossier)
            .expect("upstream verifier returns a projection")["valid"],
        json!(true),
        "the adversarial fixture must pass shape and self-digest verification"
    );
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "nonempty-hollow-dossier",
        "nonempty self-asserted rows are not deterministic research replay",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );

    assert!(matches!(
        VerifiedCampaignReceipt::from_research_dossier(
            spec.stage("measure").expect("stage exists"),
            &dossier,
        ),
        Err(CampaignError::InvalidReceipt { .. })
    ));
}

#[test]
fn an_effectful_brain_plan_stops_for_review_instead_of_claiming_execution() {
    let request = AutonomousPlanRequest {
        objective: "prepare a bounded write proposal".into(),
        steps: vec![PlanStep {
            id: "propose".into(),
            objective: "prepare but do not execute".into(),
            tool: "proposal.write".into(),
            arguments: json!({ "private": "transient" }),
            depends_on: Vec::new(),
            effect: PlanEffect::ExternalWrite,
            estimated_cost: 1,
        }],
        allowed_tools: vec!["proposal.write".into()],
        max_cost: 1,
        require_approval_for_effects: true,
        max_parallelism: 1,
    };
    let input_digest = digest(&serde_json::to_value(&request).expect("request serializes"));
    let spec = spec_with(
        "brain-review",
        "planning is not execution",
        vec![stage(
            "plan",
            CampaignActionKind::BrainPlan,
            &input_digest,
            &[],
        )],
    );
    let receipt = VerifiedCampaignReceipt::from_brain_plan(
        spec.stage("plan").expect("stage exists"),
        &request,
    )
    .expect("brain planner receipt validates");
    assert_eq!(
        receipt.disposition(),
        CampaignReceiptDisposition::AwaitingHumanReview
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("receipt settles");
    assert_eq!(campaign.status(), CampaignStatus::AwaitingHumanReview);
}

#[test]
fn an_exhausted_autopilot_report_exhausts_the_campaign_instead_of_completing_it() {
    let input_digest = "e".repeat(64);
    let spec = spec_with(
        "autopilot-exhausted",
        "propagate the native stop state",
        vec![stage(
            "drive",
            CampaignActionKind::AutopilotDrive,
            &input_digest,
            &[],
        )],
    );
    let report = autopilot_report(&input_digest, "exhausted");
    let receipt = VerifiedCampaignReceipt::from_autopilot_report(
        spec.stage("drive").expect("stage exists"),
        &report,
    )
    .expect("autopilot report verifies");
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("receipt settles");
    assert_eq!(campaign.status(), CampaignStatus::Exhausted);
}

#[test]
fn a_succeeded_autopilot_report_without_an_attempt_cannot_complete_the_campaign() {
    let input_digest = "7".repeat(64);
    let spec = spec_with(
        "empty-autopilot-success",
        "success must name work that actually ran",
        vec![stage(
            "drive",
            CampaignActionKind::AutopilotDrive,
            &input_digest,
            &[],
        )],
    );
    let report = autopilot_report(&input_digest, "succeeded");
    assert_eq!(
        bioprism_autopilot::verify_autopilot_report(&report)
            .expect("upstream verifier returns a projection")["valid"],
        json!(true),
        "the regression fixture must reach the campaign adapter's stronger gate"
    );

    assert!(matches!(
        VerifiedCampaignReceipt::from_autopilot_report(
            spec.stage("drive").expect("stage exists"),
            &report,
        ),
        Err(CampaignError::InvalidReceipt { .. })
    ));
}

#[test]
fn a_restamped_nonempty_bogus_autopilot_report_cannot_mint_success() {
    let input_digest = "8".repeat(64);
    let spec = spec_with(
        "bogus-autopilot-success",
        "nonempty rows are not terminal planner evidence",
        vec![stage(
            "drive",
            CampaignActionKind::AutopilotDrive,
            &input_digest,
            &[],
        )],
    );
    let mut report = autopilot_report(&input_digest, "succeeded");
    report["attempts"] = json!([{}]);
    report
        .as_object_mut()
        .expect("report is an object")
        .remove("report_sha256");
    report["report_sha256"] = json!(digest(&report));
    assert_eq!(
        bioprism_autopilot::verify_autopilot_report(&report)
            .expect("upstream verifier returns a projection")["valid"],
        json!(true),
        "the adversarial fixture must pass top-level integrity verification"
    );

    assert!(matches!(
        VerifiedCampaignReceipt::from_autopilot_report(
            spec.stage("drive").expect("stage exists"),
            &report,
        ),
        Err(CampaignError::InvalidReceipt { .. })
    ));
}

#[test]
fn terminal_autopilot_history_rebuilds_the_only_success_receipt() {
    let (grant, history, input_digest) = one_step_autopilot_history();
    assert!(matches!(
        plan_next_action(&grant, &history).expect("terminal history replans"),
        NextAction::StopSuccess { .. }
    ));
    let spec = spec_with(
        "planner-backed-autopilot-success",
        "success is derived from terminal grant and history",
        vec![stage(
            "drive",
            CampaignActionKind::AutopilotDrive,
            &input_digest,
            &[],
        )],
    );

    let receipt = VerifiedCampaignReceipt::from_autopilot_terminal_history(
        spec.stage("drive").expect("stage exists"),
        &grant,
        &history,
    )
    .expect("terminal history rebuilds a receipt");
    assert_eq!(receipt.disposition(), CampaignReceiptDisposition::Succeeded);
}

#[test]
fn nonterminal_autopilot_history_cannot_settle_a_campaign_stage() {
    let (grant, _, input_digest) = one_step_autopilot_history();
    let base_mission = json!({
        "mission_id": "campaign-autopilot",
        "goal": "produce one planner-verified terminal history",
        "steps": [{
            "id": "measure",
            "domain": "metrics",
            "capability": "analytics",
            "objective": "measure once",
            "tool": "measure_once",
            "arguments": {},
            "depends_on": [],
            "bindings": [],
            "required": true,
        }],
    });
    assert_eq!(digest(&base_mission), input_digest);
    let history = DriveHistory::new(base_mission).expect("test mission validates");
    let spec = spec_with(
        "nonterminal-autopilot-history",
        "a pending dispatch is not a terminal receipt",
        vec![stage(
            "drive",
            CampaignActionKind::AutopilotDrive,
            &input_digest,
            &[],
        )],
    );

    assert!(matches!(
        VerifiedCampaignReceipt::from_autopilot_terminal_history(
            spec.stage("drive").expect("stage exists"),
            &grant,
            &history,
        ),
        Err(CampaignError::InvalidReceipt { .. })
    ));
}

#[test]
fn checkpointing_retains_no_objective_prompt_arguments_output_evidence_or_credentials() {
    let sentinel = "PRIVATE-OBJECTIVE-PROMPT-ARGUMENT-OUTPUT-EVIDENCE-CREDENTIAL";
    let spec = spec_with(
        "metadata-only",
        sentinel,
        vec![stage(
            "plan",
            CampaignActionKind::BrainPlan,
            &observation_digest("private input"),
            &[],
        )],
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("checkpoint seals");
    let encoded = to_canonical_string(checkpoint.as_value()).expect("checkpoint canonicalizes");
    assert!(!encoded.contains(sentinel));
    assert_eq!(
        checkpoint.as_value()["secret_material"],
        json!("never_returned")
    );
}

#[test]
fn deleting_an_event_breaks_the_inner_chain_even_after_the_snapshot_is_restamped() {
    let input = "f".repeat(64);
    let spec = spec_with(
        "event-chain",
        "detect deleted transitions",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let receipt = VerifiedCampaignReceipt::missing_input(
        spec.stage("plan").expect("stage exists"),
        observation_digest("missing"),
    )
    .expect("receipt validates");
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("receipt settles");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("checkpoint seals");
    let mut tampered = checkpoint.as_value().clone();
    tampered["events"]
        .as_array_mut()
        .expect("events array")
        .remove(0);
    restamp_snapshot(&mut tampered);
    assert!(matches!(
        validate_campaign_checkpoint(&tampered),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}

#[test]
fn an_unknown_checkpoint_field_is_refused_even_when_the_snapshot_is_restamped() {
    let input = "0".repeat(64);
    let spec = spec_with(
        "exact-schema",
        "unknown state must not be ignored",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("checkpoint seals");
    let mut extended = checkpoint.as_value().clone();
    extended["silently_ignored"] = json!(true);
    restamp_snapshot(&mut extended);
    assert!(matches!(
        validate_campaign_checkpoint(&extended),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}

#[test]
fn a_receipt_for_another_stage_cannot_advance_the_live_authorization() {
    let first_input = "3".repeat(64);
    let second_input = "4".repeat(64);
    let spec = spec_with(
        "receipt-binding",
        "bind receipts to one stage",
        vec![
            stage("first", CampaignActionKind::BrainPlan, &first_input, &[]),
            stage(
                "second",
                CampaignActionKind::AutopilotDrive,
                &second_input,
                &["first"],
            ),
        ],
    );
    let wrong_receipt = VerifiedCampaignReceipt::missing_input(
        spec.stage("second").expect("second stage exists"),
        observation_digest("wrong-stage observation"),
    )
    .expect("observation receipt validates");
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("first is authorized");
    assert!(matches!(
        campaign.apply_receipt(authorization, wrong_receipt),
        Err(CampaignError::StaleAuthorization)
    ));
    assert_eq!(campaign.status(), CampaignStatus::InFlight);
    assert_eq!(campaign.active_stage_id(), Some("first"));
}

#[test]
fn identical_campaign_states_seal_to_identical_bytes_and_generations_chain() {
    let input = "1".repeat(64);
    let spec = spec_with(
        "deterministic",
        "same state means same bytes",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut left = start_campaign(spec.clone()).expect("left starts");
    let mut right = start_campaign(spec).expect("right starts");
    let left_first = seal_campaign_checkpoint(&mut left).expect("left seals");
    let right_first = seal_campaign_checkpoint(&mut right).expect("right seals");
    assert_eq!(left_first.as_value(), right_first.as_value());

    let left_second = seal_campaign_checkpoint(&mut left).expect("second generation seals");
    assert_eq!(left_second.generation(), 2);
    assert_eq!(
        left_second.as_value()["previous_snapshot_digest"],
        json!(left_first.snapshot_digest())
    );
}

#[test]
fn two_workers_cannot_authorize_from_one_restored_checkpoint_head() {
    let input = "5".repeat(64);
    let spec = spec_with(
        "single-worker-claim",
        "one durable head authorizes at most one restored worker",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut original = start_campaign(spec.clone()).expect("campaign starts");
    let checkpoint = seal_campaign_checkpoint(&mut original).expect("planned state seals");
    let trusted_head = checkpoint.head();
    let mut first = restore_campaign(spec.clone(), &checkpoint, &trusted_head, Vec::new())
        .expect("first worker restores");
    let mut second = restore_campaign(spec, &checkpoint, &trusted_head, Vec::new())
        .expect("second worker may inspect the same state");
    let coordinator = OneShotCoordinator::for_head(trusted_head);

    let authorization = first
        .authorize_next_action(&coordinator)
        .expect("first worker atomically stores the in-flight checkpoint");
    assert_eq!(authorization.action_ordinal(), 1);
    assert!(matches!(
        second.authorize_next_action(&coordinator),
        Err(CampaignError::AuthorizationCheckpointRejected { .. })
    ));
    assert_eq!(second.status(), CampaignStatus::Planned);
    assert_eq!(second.actions_used(), 0);
    assert_eq!(second.active_stage_id(), None);
}

#[test]
fn a_valid_older_checkpoint_cannot_restore_against_a_newer_trusted_head() {
    let input = "4".repeat(64);
    let spec = spec_with(
        "trusted-head-rollback",
        "the caller's durable head defines current state",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let older = seal_campaign_checkpoint(&mut campaign).expect("generation one seals");
    let newer = seal_campaign_checkpoint(&mut campaign).expect("generation two seals");
    assert!(validate_campaign_checkpoint(older.as_value()).is_ok());

    assert!(matches!(
        restore_campaign(spec, &older, &newer.head(), Vec::new()),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}

#[test]
fn restore_requires_reverified_artifacts_and_rejects_spec_drift() {
    let (request, dossier) = research_fixture();
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "restore",
        "bind the original objective",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );
    let receipt = VerifiedCampaignReceipt::from_research_dossier(
        spec.stage("measure").expect("stage exists"),
        &dossier,
    )
    .expect("receipt verifies");
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt.clone())
        .expect("receipt settles");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("checkpoint seals");

    assert!(matches!(
        restore_campaign(spec.clone(), &checkpoint, &checkpoint.head(), Vec::new()),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
    let restored = restore_campaign(spec.clone(), &checkpoint, &checkpoint.head(), vec![receipt])
        .expect("reverified receipt restores state");
    assert_eq!(restored.status(), CampaignStatus::Completed);

    let drifted = spec_with(
        "restore",
        "a changed objective",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );
    let replacement = VerifiedCampaignReceipt::from_research_dossier(
        drifted.stage("measure").expect("stage exists"),
        &dossier,
    )
    .expect("artifact still verifies independently");
    assert!(matches!(
        restore_campaign(drifted, &checkpoint, &checkpoint.head(), vec![replacement]),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}

#[test]
fn restoring_an_in_flight_action_requires_reconciliation_and_never_redispatches() {
    let input = "2".repeat(64);
    let spec = spec_with(
        "in-flight",
        "do not retry an uncertain boundary",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("in-flight state seals");
    drop(authorization);

    let mut restored = restore_campaign(spec, &checkpoint, &checkpoint.head(), Vec::new())
        .expect("in-flight checkpoint restores behind a fence");
    assert_eq!(restored.status(), CampaignStatus::ReconciliationRequired);
    assert!(matches!(
        restored.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::ActionAlreadyInFlight)
    ));
    let next = seal_campaign_checkpoint(&mut restored).expect("fenced state seals");
    assert_eq!(next.generation(), 3);
    assert_eq!(next.as_value()["status"], json!("reconciliation_required"));
}

#[test]
fn a_lost_acknowledgement_reconciles_from_the_journal_without_redispatch() {
    let (request, dossier) = research_fixture();
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "lost-acknowledgement",
        "recover a completed action from its durable journal",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );
    let native_receipt = VerifiedCampaignReceipt::from_research_dossier(
        spec.stage("measure").expect("stage exists"),
        &dossier,
    )
    .expect("native dossier verifies");
    assert_eq!(
        native_receipt.disposition(),
        CampaignReceiptDisposition::CompletedWithNegativeFindings
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let reconciliation = verified_reconciliation(&campaign, succeeded_decision(&native_receipt));

    assert_eq!(
        campaign
            .reconcile_active_action(reconciliation, Some(native_receipt.clone()))
            .expect("journal-backed success settles"),
        CampaignReconciliationResult::Settled(CampaignStatus::Completed)
    );
    assert_eq!(campaign.actions_used(), 1);
    assert_eq!(campaign.active_stage_id(), None);
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("reconciled state seals");
    assert_eq!(
        checkpoint.as_value()["stages"][0]["receipt"]["disposition"],
        json!("completed_with_negative_findings"),
        "journal recovery must not relabel a native negative result"
    );
    let restored = restore_campaign(spec, &checkpoint, &checkpoint.head(), vec![native_receipt])
        .expect("reconciled success replays and rehydrates");
    assert_eq!(restored.status(), CampaignStatus::Completed);
}

#[test]
fn not_executed_requeues_with_a_new_action_ordinal_and_keeps_the_old_attempt_charged() {
    let input = "a".repeat(64);
    let spec = spec_with_max(
        "proved-absence",
        "retry only after positive absence evidence",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let old_query = campaign.reconciliation_query().expect("old query exists");
    let reconciliation = verified_reconciliation(
        &campaign,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest("journal proves no dispatch row"),
        },
    );

    assert_eq!(
        campaign
            .reconcile_active_action(reconciliation, None)
            .expect("positive absence evidence requeues"),
        CampaignReconciliationResult::Requeued
    );
    assert_eq!(campaign.status(), CampaignStatus::Ready);
    assert_eq!(campaign.actions_used(), 1);
    assert_eq!(campaign.active_stage_id(), None);
    let next = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("caller may explicitly authorize a fresh attempt");
    assert_eq!(next.action_ordinal(), 2);
    assert_ne!(
        next.authorization_digest(),
        old_query.authorization_digest()
    );
}

#[test]
fn not_executed_at_the_action_ceiling_exhausts_instead_of_claiming_ready() {
    let input = "b".repeat(64);
    let spec = spec_with(
        "absence-at-ceiling",
        "a charged lost attempt still consumes budget",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let reconciliation = verified_reconciliation(
        &campaign,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest("positive absence proof"),
        },
    );

    assert_eq!(
        campaign
            .reconcile_active_action(reconciliation, None)
            .expect("absence resolves the uncertainty"),
        CampaignReconciliationResult::Exhausted
    );
    assert_eq!(campaign.status(), CampaignStatus::Exhausted);
    assert_eq!(campaign.actions_used(), 1);
    assert!(matches!(
        campaign.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::ActionNotAvailable { .. })
    ));
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("exhausted state seals");
    assert!(validate_campaign_checkpoint(checkpoint.as_value()).is_ok());
}

#[test]
fn unknown_reconciliation_is_a_no_op_and_keeps_the_campaign_fenced() {
    let input = "c".repeat(64);
    let spec = spec_with(
        "still-unknown",
        "uncertainty is not absence",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let chain_before = campaign.event_chain_digest().to_owned();
    let reconciliation = verified_reconciliation(
        &campaign,
        CampaignReconciliationDecisionDocument::Unknown {
            uncertainty_evidence_digest: observation_digest("journal is inconclusive"),
        },
    );

    assert_eq!(
        campaign
            .reconcile_active_action(reconciliation, None)
            .expect("unknown remains a valid conclusion"),
        CampaignReconciliationResult::Unresolved
    );
    assert_eq!(campaign.status(), CampaignStatus::ReconciliationRequired);
    assert_eq!(campaign.active_stage_id(), Some("plan"));
    assert_eq!(campaign.actions_used(), 1);
    assert_eq!(campaign.event_chain_digest(), chain_before);
    assert!(matches!(
        campaign.authorize_next_action(&AcceptingCoordinator),
        Err(CampaignError::ActionAlreadyInFlight)
    ));
}

#[test]
fn an_explicit_unknown_completion_remains_reconcilable_after_restore() {
    let input = "d".repeat(64);
    let spec = spec_with_max(
        "durable-unknown",
        "retain the uncertain action lineage across restart",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let unknown_receipt = VerifiedCampaignReceipt::unknown_completion(
        spec.stage("plan").expect("stage exists"),
        observation_digest("acknowledgement lost"),
    )
    .expect("unknown observation validates");
    let mut campaign = start_campaign(spec.clone()).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, unknown_receipt)
        .expect("unknown completion is recorded");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("uncertain state seals");
    assert_eq!(
        checkpoint.as_value()["stages"][0]["state"],
        json!("uncertain")
    );
    let mut restored = restore_campaign(spec, &checkpoint, &checkpoint.head(), Vec::new())
        .expect("uncertain observation needs no private artifact rehydration");
    let reconciliation = verified_reconciliation(
        &restored,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest("journal later proves absence"),
        },
    );

    assert_eq!(
        restored
            .reconcile_active_action(reconciliation, None)
            .expect("restored uncertainty resolves"),
        CampaignReconciliationResult::Requeued
    );
    assert_eq!(restored.status(), CampaignStatus::Ready);
}

#[test]
fn succeeded_reconciliation_requires_the_exact_native_verified_receipt() {
    let (request, dossier) = research_fixture();
    let input_digest = request.digest().expect("request digests");
    let spec = spec_with(
        "native-receipt-binding",
        "journal success is not a substitute for native verification",
        vec![stage(
            "measure",
            CampaignActionKind::SyntheticResearch,
            &input_digest,
            &[],
        )],
    );
    let expected = VerifiedCampaignReceipt::from_research_dossier(
        spec.stage("measure").expect("stage exists"),
        &dossier,
    )
    .expect("dossier verifies");
    let wrong = VerifiedCampaignReceipt::missing_input(
        spec.stage("measure").expect("stage exists"),
        observation_digest("different native projection"),
    )
    .expect("different receipt validates independently");
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let reconciliation = verified_reconciliation(&campaign, succeeded_decision(&expected));

    assert!(matches!(
        campaign.reconcile_active_action(reconciliation, Some(wrong)),
        Err(CampaignError::InvalidReconciliationReceipt { .. })
    ));
    assert_eq!(campaign.status(), CampaignStatus::ReconciliationRequired);
    assert_eq!(campaign.active_stage_id(), Some("measure"));
    assert_eq!(campaign.actions_used(), 1);
}

#[test]
fn an_empty_journal_is_not_an_absence_proof() {
    let input = "e".repeat(64);
    let spec = spec_with_max(
        "empty-journal",
        "absence requires affirmative journal evidence",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let campaign = restore_after_lost_acknowledgement(&spec);
    let query = campaign.reconciliation_query().expect("query exists");
    let value = reconciliation_value(
        &campaign,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest("claimed absence"),
        },
    );

    assert!(matches!(
        verify_campaign_reconciliation(&query, &value, &EmptyJournal),
        Err(CampaignError::ReconciliationJournalRejected { .. })
    ));
}

#[test]
fn a_different_authority_cannot_resolve_the_campaign() {
    let input = "f".repeat(64);
    let spec = spec_with_max(
        "wrong-authority",
        "only the specification-bound journal may resolve execution",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let campaign = restore_after_lost_acknowledgement(&spec);
    let query = campaign.reconciliation_query().expect("query exists");
    let mut value = reconciliation_value(
        &campaign,
        CampaignReconciliationDecisionDocument::Unknown {
            uncertainty_evidence_digest: observation_digest("unknown"),
        },
    );
    value["authority"]["config_digest"] = json!(observation_digest("other journal"));
    restamp_reconciliation_receipt(&mut value);

    assert!(matches!(
        verify_campaign_reconciliation(&query, &value, &AcceptingJournal),
        Err(CampaignError::StaleReconciliationReceipt)
    ));
}

#[test]
fn a_receipt_with_the_wrong_authorization_predecessor_is_stale() {
    let input = "6".repeat(64);
    let spec = spec_with_max(
        "wrong-predecessor",
        "bind journal evidence to the exact authorization lineage",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let campaign = restore_after_lost_acknowledgement(&spec);
    let query = campaign.reconciliation_query().expect("query exists");
    let mut value = reconciliation_value(
        &campaign,
        CampaignReconciliationDecisionDocument::Unknown {
            uncertainty_evidence_digest: observation_digest("unknown"),
        },
    );
    value["authorization_predecessor_digest"] = json!(observation_digest("unrelated predecessor"));
    restamp_reconciliation_receipt(&mut value);

    assert!(matches!(
        verify_campaign_reconciliation(&query, &value, &AcceptingJournal),
        Err(CampaignError::StaleReconciliationReceipt)
    ));
}

#[test]
fn a_receipt_bound_to_a_previous_attempt_cannot_resolve_a_later_attempt() {
    let input = "0".repeat(64);
    let spec = spec_with_max(
        "stale-reconciliation",
        "each attempt has distinct lineage",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let old_decision = CampaignReconciliationDecisionDocument::NotExecuted {
        absence_evidence_digest: observation_digest("first attempt absent"),
    };
    let first = verified_reconciliation(&campaign, old_decision.clone());
    let stale_copy = verified_reconciliation(&campaign, old_decision);
    campaign
        .reconcile_active_action(first, None)
        .expect("first attempt requeues");
    let second_authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("second attempt is explicitly authorized");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("second attempt seals");
    drop(second_authorization);
    let mut restored = restore_campaign(spec, &checkpoint, &checkpoint.head(), Vec::new())
        .expect("second attempt restores behind a fence");

    assert!(matches!(
        restored.reconcile_active_action(stale_copy, None),
        Err(CampaignError::StaleReconciliationReceipt)
    ));
    assert_eq!(restored.active_stage_id(), Some("plan"));
    assert_eq!(restored.actions_used(), 2);
}

#[test]
fn deleting_a_reconciliation_predecessor_fails_even_after_restamping_every_digest() {
    let input = "1".repeat(64);
    let spec = spec_with_max(
        "reconciliation-replay",
        "replay must prove every state predecessor",
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let reconciliation = verified_reconciliation(
        &campaign,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest("absent"),
        },
    );
    campaign
        .reconcile_active_action(reconciliation, None)
        .expect("absence requeues");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("requeued state seals");
    let mut tampered = checkpoint.as_value().clone();
    tampered["events"]
        .as_array_mut()
        .expect("events are an array")
        .remove(0);
    restamp_event_chain(&mut tampered);

    assert!(matches!(
        validate_campaign_checkpoint(&tampered),
        Err(CampaignError::InvalidCheckpoint { .. })
    ));
}

#[test]
fn reconciliation_receipts_and_checkpoints_retain_no_raw_evidence_or_credentials() {
    let sentinel = "PRIVATE-RECONCILIATION-EVIDENCE-CREDENTIAL";
    let input = "2".repeat(64);
    let spec = spec_with_max(
        "reconciliation-retention",
        sentinel,
        vec![stage("plan", CampaignActionKind::BrainPlan, &input, &[])],
        2,
    );
    let mut campaign = restore_after_lost_acknowledgement(&spec);
    let value = reconciliation_value(
        &campaign,
        CampaignReconciliationDecisionDocument::NotExecuted {
            absence_evidence_digest: observation_digest(sentinel),
        },
    );
    let encoded = to_canonical_string(&value).expect("receipt canonicalizes");
    assert!(!encoded.contains(sentinel));
    assert_eq!(value["secret_material"], json!("never_returned"));
    let query = campaign.reconciliation_query().expect("query exists");
    let reconciliation = verify_campaign_reconciliation(&query, &value, &AcceptingJournal)
        .expect("receipt verifies");
    campaign
        .reconcile_active_action(reconciliation, None)
        .expect("receipt applies");
    let checkpoint = seal_campaign_checkpoint(&mut campaign).expect("state seals");
    let encoded = to_canonical_string(checkpoint.as_value()).expect("checkpoint canonicalizes");
    assert!(!encoded.contains(sentinel));
}

#[cfg(feature = "neurosurgery-adapter")]
#[test]
fn a_verified_neurosurgical_terminal_session_never_becomes_workflow_complete() {
    use bioprism_neurosurgery::{CaseRequest, NeurosurgicalAgent};

    let request: CaseRequest = serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("fixture request parses");
    let agent = NeurosurgicalAgent::default();
    let mut session = agent.start_session(&request, None).expect("session starts");
    let input_digest = session.request_digest.clone();
    while session.next_ordinal as usize <= session.route.len() {
        session = agent
            .advance_session(&session, &request, None)
            .expect("route advances");
    }
    let spec = spec_with(
        "neuro-review",
        "domain research remains non-clinical",
        vec![stage(
            "domain",
            CampaignActionKind::NeurosurgeryResearch,
            &input_digest,
            &[],
        )],
    );
    let receipt = VerifiedCampaignReceipt::from_neurosurgery_session(
        spec.stage("domain").expect("stage exists"),
        &agent,
        &session,
    )
    .expect("session integrity verifies");
    assert_eq!(
        receipt.disposition(),
        CampaignReceiptDisposition::AwaitingHumanReview
    );
    let mut campaign = start_campaign(spec).expect("campaign starts");
    let authorization = campaign
        .authorize_next_action(&AcceptingCoordinator)
        .expect("action authorized");
    campaign
        .apply_receipt(authorization, receipt)
        .expect("receipt settles");
    assert_eq!(campaign.status(), CampaignStatus::AwaitingHumanReview);
}
