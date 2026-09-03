//! Provider-free, append-only execution surface for bounded research campaigns.
//!
//! The MCP audit log records tool arguments and API gateways may retain tool results. This module
//! therefore accepts paths rather than private campaign documents and returns metadata rather
//! than objectives, questions, plan arguments, dossiers, or planner reports. The linear campaign
//! authorization never crosses the Rust boundary.

use bioprism_brain::{plan_autonomous, AutonomousPlanRequest};
use bioprism_ids::{to_canonical_bytes, ContentHash};
use bioprism_research::{run_research, ResearchRequest};
use bioprism_research_campaign::{
    seal_campaign_checkpoint, start_campaign, CampaignActionKind, CampaignAuthorizationClaim,
    CampaignCheckpointCoordinator, CampaignCheckpointHead, CampaignReceiptDisposition,
    CampaignStatus, ResearchCampaignSpec, ValidatedCampaignCheckpoint, VerifiedCampaignReceipt,
    RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub(crate) const OFFLINE_CAMPAIGN_SCHEMA: &str = "bioprism-mcp/research-campaign-offline-run/0.1";
const OFFLINE_CAMPAIGN_MANIFEST_SCHEMA: &str =
    "bioprism-mcp/research-campaign-offline-manifest/0.1";
const AUTHORIZATION_ENVELOPE_SCHEMA: &str =
    "bioprism-mcp/research-campaign-authorization-envelope/0.1";
const TERMINAL_ENVELOPE_SCHEMA: &str = "bioprism-mcp/research-campaign-terminal-envelope/0.1";
const MAX_OFFLINE_STAGES: usize = 8;
const MAX_AUTHORITY_ENTRIES: usize = MAX_OFFLINE_STAGES + 1;
const MAX_AUTHORITY_SCAN_ENTRIES: usize = MAX_AUTHORITY_ENTRIES + 1;
const MAX_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;
const MAX_SPEC_BYTES: u64 = 1_000_000;
const MAX_STAGE_INPUT_BYTES: u64 = 2_000_000;
const MAX_TOTAL_ARTIFACT_BYTES: usize = 32_000_000;
const RETENTION: &str = "metadata_only_response; objectives, questions, plan arguments, dossiers, and planner reports remain only in caller-owned files";
const LIMITATIONS: [&str; 4] = [
    "supports only synthetic_research and brain_plan campaign stages",
    "synthetic_research measures seeded repository fixtures and does not search external literature",
    "brain_plan validates and orders a plan but never executes its steps",
    "this first slice has no resume or execution-journal reconciliation; an interrupted output directory must be inspected rather than retried",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfflineCampaignArgs {
    spec_path: String,
    stage_input_paths: BTreeMap<String, String>,
    output_dir: String,
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug)]
enum PreparedInput {
    SyntheticResearch(ResearchRequest),
    BrainPlan(AutonomousPlanRequest),
}

#[derive(Debug)]
struct PreparedStage {
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    artifact_locator: String,
    input: PreparedInput,
}

#[derive(Debug)]
struct PreparedCampaign {
    spec: ResearchCampaignSpec,
    stages: Vec<PreparedStage>,
    output_dir: PathBuf,
    output_display: String,
    confirm: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum OfflineExecution {
    NotStarted,
    Completed,
    AwaitingHumanReview { stage_id: String },
    Refused { stage_id: String },
    NeedsInput { stage_id: String },
    Exhausted { stage_id: String },
    ReconciliationRequired { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum OfflineStageOutcome {
    NotStarted {
        stage_id: String,
        kind: CampaignActionKind,
        input_digest: String,
        artifact_locator: String,
    },
    Settled {
        stage_id: String,
        kind: CampaignActionKind,
        input_digest: String,
        action_ordinal: u16,
        disposition: CampaignReceiptDisposition,
        artifact_digest: String,
        receipt_digest: String,
        artifact_locator: String,
        file_sha256: String,
    },
    ReconciliationRequired {
        stage_id: String,
        kind: CampaignActionKind,
        input_digest: String,
        action_ordinal: u16,
        authorization_digest: String,
        artifact_locator: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointSummary {
    locator: String,
    schema: String,
    generation: u64,
    snapshot_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedHeadSummary {
    locator: String,
    campaign_id: String,
    spec_digest: String,
    generation: u64,
    snapshot_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSummary {
    locator: String,
    digest: String,
    file_sha256: String,
}

#[derive(Debug, Serialize)]
struct OfflineCampaignResponse {
    schema: &'static str,
    workflow: &'static str,
    execution: OfflineExecution,
    campaign_id: String,
    spec_digest: String,
    campaign_status: CampaignStatus,
    actions_used: u16,
    stages: Vec<OfflineStageOutcome>,
    checkpoint: Option<CheckpointSummary>,
    trusted_head: Option<TrustedHeadSummary>,
    manifest: Option<ManifestSummary>,
    written: Vec<String>,
    limitations: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct CampaignManifest<'a> {
    schema: &'static str,
    campaign_id: &'a str,
    spec_digest: &'a str,
    campaign_status: CampaignStatus,
    actions_used: u16,
    stages: &'a [OfflineStageOutcome],
    checkpoint: &'a CheckpointSummary,
    trusted_head: &'a TrustedHeadSummary,
    retention: &'static str,
    limitations: &'static [&'static str],
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCampaignManifest {
    schema: String,
    campaign_id: String,
    spec_digest: String,
    campaign_status: CampaignStatus,
    actions_used: u16,
    stages: Vec<OfflineStageOutcome>,
    checkpoint: CheckpointSummary,
    trusted_head: TrustedHeadSummary,
    retention: String,
    limitations: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AuthorizationEnvelope<'a> {
    schema: &'static str,
    expected_checkpoint_head: Option<&'a CampaignCheckpointHead>,
    candidate_checkpoint_head: CampaignCheckpointHead,
    claim: &'a CampaignAuthorizationClaim,
    checkpoint: &'a Value,
    retention: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAuthorizationEnvelope {
    schema: String,
    expected_checkpoint_head: Option<CampaignCheckpointHead>,
    candidate_checkpoint_head: CampaignCheckpointHead,
    claim: StoredAuthorizationClaim,
    checkpoint: Value,
    retention: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAuthorizationClaim {
    expected_checkpoint_head: Option<CampaignCheckpointHead>,
    candidate_checkpoint_head: CampaignCheckpointHead,
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    action_ordinal: u16,
    authorization_digest: String,
    authorization_predecessor_digest: String,
}

#[derive(Debug, Serialize)]
struct TerminalEnvelope<'a> {
    schema: &'static str,
    expected_checkpoint_head: &'a CampaignCheckpointHead,
    candidate_checkpoint_head: CampaignCheckpointHead,
    checkpoint: &'a Value,
    retention: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTerminalEnvelope {
    schema: String,
    expected_checkpoint_head: CampaignCheckpointHead,
    candidate_checkpoint_head: CampaignCheckpointHead,
    checkpoint: Value,
    retention: String,
}

struct VerifiedAuthorizationEnvelope {
    locator: String,
    checkpoint: ValidatedCampaignCheckpoint,
}

struct VerifiedTerminalEnvelope {
    locator: String,
    checkpoint: ValidatedCampaignCheckpoint,
}

#[derive(Default)]
struct AuthorityChainInspection {
    authorizations: Vec<VerifiedAuthorizationEnvelope>,
    terminal: Option<VerifiedTerminalEnvelope>,
    failure: Option<String>,
}

#[derive(Debug, Default)]
struct CoordinatorState {
    head: Option<CampaignCheckpointHead>,
    written: Vec<String>,
}

/// One-worker durable authorization boundary. The output directory itself is claimed atomically,
/// and every authorization stores checkpoint, head, and claim in one create-new canonical file
/// before the linear token is released.
struct AppendOnlyFileCoordinator {
    authority_dir: PathBuf,
    output_display: String,
    state: Mutex<CoordinatorState>,
}

impl AppendOnlyFileCoordinator {
    fn create(output_dir: &Path, output_display: &str) -> Result<Self, String> {
        fs::create_dir(output_dir).map_err(|error| {
            format!(
                "cannot atomically claim the requested output_dir: {error}; existing campaign output is never overwritten or blindly resumed"
            )
        })?;
        let authority_dir = output_dir.join("authority");
        fs::create_dir(&authority_dir).map_err(|error| {
            format!("cannot create the append-only authority directory: {error}")
        })?;
        fs::create_dir(output_dir.join("artifacts"))
            .map_err(|error| format!("cannot create the campaign artifact directory: {error}"))?;
        Ok(Self {
            authority_dir,
            output_display: output_display.to_owned(),
            state: Mutex::new(CoordinatorState::default()),
        })
    }

    fn store_terminal(
        &self,
        checkpoint: &ValidatedCampaignCheckpoint,
    ) -> Result<CampaignCheckpointHead, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "campaign checkpoint coordinator lock is poisoned".to_string())?;
        let expected = state.head.as_ref().ok_or_else(|| {
            "cannot store a terminal checkpoint before an authorization checkpoint".to_string()
        })?;
        if checkpoint.generation() != expected.generation().saturating_add(1)
            || checkpoint
                .as_value()
                .get("previous_snapshot_digest")
                .and_then(Value::as_str)
                != Some(expected.snapshot_digest())
        {
            return Err(
                "terminal checkpoint does not extend the durable authorization head".into(),
            );
        }
        let head = checkpoint.head();
        let envelope = TerminalEnvelope {
            schema: TERMINAL_ENVELOPE_SCHEMA,
            expected_checkpoint_head: expected,
            candidate_checkpoint_head: head.clone(),
            checkpoint: checkpoint.as_value(),
            retention: RETENTION,
        };
        let value = serde_json::to_value(envelope)
            .map_err(|error| format!("cannot encode terminal campaign envelope: {error}"))?;
        let name = format!("{:04}-terminal.json", head.generation());
        write_new_canonical(&self.authority_dir.join(&name), &value)?;
        state.head = Some(head.clone());
        state.written.push(join_display(
            &self.output_display,
            &format!("authority/{name}"),
        ));
        Ok(head)
    }

    fn written(&self) -> Result<Vec<String>, String> {
        self.state
            .lock()
            .map(|state| state.written.clone())
            .map_err(|_| "campaign checkpoint coordinator lock is poisoned".to_string())
    }
}

impl CampaignCheckpointCoordinator for AppendOnlyFileCoordinator {
    fn compare_and_store_authorization(
        &self,
        expected_head: Option<&CampaignCheckpointHead>,
        candidate: &ValidatedCampaignCheckpoint,
        claim: &CampaignAuthorizationClaim,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "campaign checkpoint coordinator lock is poisoned".to_string())?;
        if state.head.as_ref() != expected_head
            || claim.expected_checkpoint_head() != expected_head
            || claim.candidate_checkpoint_head() != &candidate.head()
        {
            return Err(
                "authorization compare-and-store rejected a stale or mismatched checkpoint head"
                    .into(),
            );
        }
        let candidate_head = candidate.head();
        let envelope = AuthorizationEnvelope {
            schema: AUTHORIZATION_ENVELOPE_SCHEMA,
            expected_checkpoint_head: expected_head,
            candidate_checkpoint_head: candidate_head.clone(),
            claim,
            checkpoint: candidate.as_value(),
            retention: RETENTION,
        };
        let value = serde_json::to_value(envelope)
            .map_err(|error| format!("cannot encode campaign authorization envelope: {error}"))?;
        let name = format!("{:04}-authorization.json", candidate_head.generation());
        write_new_canonical(&self.authority_dir.join(&name), &value)?;
        state.head = Some(candidate_head);
        state.written.push(join_display(
            &self.output_display,
            &format!("authority/{name}"),
        ));
        Ok(())
    }
}

pub(crate) fn run_offline<F>(arguments: &Value, resolve: F) -> Result<Value, String>
where
    F: Fn(&str) -> Result<PathBuf, String>,
{
    let prepared = prepare(arguments, &resolve)?;
    let response = if prepared.output_dir.exists() {
        if prepared.confirm {
            inspect_existing(prepared)?
        } else {
            return Err(
                "the requested output_dir already exists; preview cannot promise a fresh append-only target, and existing campaign output is never overwritten"
                    .into(),
            );
        }
    } else if prepared.confirm {
        execute(prepared)?
    } else {
        preview(prepared)
    };
    serde_json::to_value(response)
        .map_err(|error| format!("cannot encode offline campaign response: {error}"))
}

fn prepare<F>(arguments: &Value, resolve: &F) -> Result<PreparedCampaign, String>
where
    F: Fn(&str) -> Result<PathBuf, String>,
{
    let encoded = serde_json::to_vec(arguments)
        .map_err(|error| format!("cannot encode offline campaign arguments: {error}"))?;
    if encoded.len() > MAX_TOOL_ARGUMENT_BYTES {
        return Err(format!(
            "offline campaign arguments exceed the {MAX_TOOL_ARGUMENT_BYTES}-byte bound"
        ));
    }
    let args: OfflineCampaignArgs = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("invalid offline campaign arguments: {error}"))?;
    validate_relative_label(&args.spec_path, "spec_path")?;
    validate_relative_label(&args.output_dir, "output_dir")?;
    if args.stage_input_paths.is_empty() || args.stage_input_paths.len() > MAX_OFFLINE_STAGES {
        return Err(format!(
            "stage_input_paths must contain 1..={MAX_OFFLINE_STAGES} entries"
        ));
    }

    let spec_path = resolve(&args.spec_path)?;
    let spec_value = read_json_bounded(&spec_path, MAX_SPEC_BYTES, "campaign spec")?;
    let spec = ResearchCampaignSpec::parse(&spec_value)
        .map_err(|error| format!("campaign spec was refused: {error}"))?;
    if spec.stages().count() > MAX_OFFLINE_STAGES
        || usize::from(spec.max_actions()) > MAX_OFFLINE_STAGES
    {
        return Err(format!(
            "offline campaigns are limited to {MAX_OFFLINE_STAGES} stages and actions"
        ));
    }
    for stage in spec.stages() {
        if !matches!(
            stage.kind(),
            CampaignActionKind::SyntheticResearch | CampaignActionKind::BrainPlan
        ) {
            return Err(format!(
                "stage {:?} uses unsupported offline kind {}; no action was authorized",
                stage.stage_id(),
                stage.kind().as_str()
            ));
        }
    }

    let expected_ids = spec
        .stages()
        .map(|stage| stage.stage_id().to_owned())
        .collect::<BTreeSet<_>>();
    let supplied_ids = args
        .stage_input_paths
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_ids != supplied_ids {
        let missing = expected_ids
            .difference(&supplied_ids)
            .cloned()
            .collect::<Vec<_>>();
        let extra = supplied_ids
            .difference(&expected_ids)
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "stage_input_paths must exactly match campaign stages; missing={missing:?}, extra={extra:?}"
        ));
    }

    let mut stages = Vec::with_capacity(expected_ids.len());
    for (index, stage) in spec.stages().enumerate() {
        let relative = args
            .stage_input_paths
            .get(stage.stage_id())
            .expect("exact stage input map contains every stage");
        validate_relative_label(relative, "stage_input_paths value")?;
        let path = resolve(relative)?;
        let value = read_json_bounded(&path, MAX_STAGE_INPUT_BYTES, "campaign stage input")?;
        let (input_digest, input) = match stage.kind() {
            CampaignActionKind::SyntheticResearch => {
                let request: ResearchRequest = serde_json::from_value(value).map_err(|error| {
                    format!(
                        "synthetic_research input for stage {:?} does not match the native request schema: {error}",
                        stage.stage_id()
                    )
                })?;
                let digest = request.digest().map_err(|error| {
                    format!(
                        "cannot digest synthetic_research input for stage {:?}: {error}",
                        stage.stage_id()
                    )
                })?;
                (digest, PreparedInput::SyntheticResearch(request))
            }
            CampaignActionKind::BrainPlan => {
                let request: AutonomousPlanRequest = serde_json::from_value(value).map_err(|error| {
                    format!(
                        "brain_plan input for stage {:?} does not match the native request schema: {error}",
                        stage.stage_id()
                    )
                })?;
                let request_value = serde_json::to_value(&request).map_err(|error| {
                    format!(
                        "cannot canonicalize brain_plan input for stage {:?}: {error}",
                        stage.stage_id()
                    )
                })?;
                let digest = ContentHash::of_value(&request_value)
                    .map_err(|error| {
                        format!(
                            "cannot digest brain_plan input for stage {:?}: {error}",
                            stage.stage_id()
                        )
                    })?
                    .to_string();
                (digest, PreparedInput::BrainPlan(request))
            }
            CampaignActionKind::AutopilotDrive | CampaignActionKind::NeurosurgeryResearch => {
                unreachable!("unsupported kinds were rejected before input loading")
            }
        };
        if input_digest != stage.input_digest() {
            return Err(format!(
                "stage {:?} input digest does not match the campaign spec",
                stage.stage_id()
            ));
        }
        let suffix = match stage.kind() {
            CampaignActionKind::SyntheticResearch => "research-dossier",
            CampaignActionKind::BrainPlan => "brain-plan-report",
            CampaignActionKind::AutopilotDrive | CampaignActionKind::NeurosurgeryResearch => {
                unreachable!("unsupported kinds were rejected before locator construction")
            }
        };
        stages.push(PreparedStage {
            stage_id: stage.stage_id().to_owned(),
            kind: stage.kind(),
            input_digest,
            artifact_locator: format!("artifacts/{:04}-{suffix}.json", index + 1),
            input,
        });
    }

    let output_dir = resolve(&args.output_dir)?;
    if output_dir.exists() {
        if !output_dir.is_dir() {
            return Err("the requested output_dir exists but is not a directory".into());
        }
    } else {
        let parent = output_dir
            .parent()
            .ok_or_else(|| "output_dir has no parent inside the server root".to_string())?;
        if !parent.is_dir() {
            return Err(
                "the requested output_dir parent must already exist so the campaign directory can be claimed atomically"
                    .into(),
            );
        }
    }

    Ok(PreparedCampaign {
        spec,
        stages,
        output_dir,
        output_display: normalize_display_path(&args.output_dir),
        confirm: args.confirm,
    })
}

fn preview(prepared: PreparedCampaign) -> OfflineCampaignResponse {
    OfflineCampaignResponse {
        schema: OFFLINE_CAMPAIGN_SCHEMA,
        workflow: "research_campaign_run_offline",
        execution: OfflineExecution::NotStarted,
        campaign_id: prepared.spec.campaign_id().to_owned(),
        spec_digest: prepared.spec.spec_digest().to_owned(),
        campaign_status: CampaignStatus::Planned,
        actions_used: 0,
        stages: prepared
            .stages
            .into_iter()
            .map(not_started_outcome)
            .collect(),
        checkpoint: None,
        trusted_head: None,
        manifest: None,
        written: Vec::new(),
        limitations: &LIMITATIONS,
    }
}

fn inspect_authority_chain(
    prepared: &PreparedCampaign,
) -> Result<AuthorityChainInspection, String> {
    let authority_dir = prepared.output_dir.join("authority");
    if !authority_dir.is_dir() {
        return Ok(AuthorityChainInspection::default());
    }

    let mut authorization_entries = Vec::new();
    let mut terminal_entries = Vec::new();
    let mut unexpected_entry_count = 0_usize;
    let entries = fs::read_dir(&authority_dir)
        .map_err(|error| format!("cannot inspect authority directory: {error}"))?;
    for (index, entry) in entries.take(MAX_AUTHORITY_SCAN_ENTRIES).enumerate() {
        if index == MAX_AUTHORITY_ENTRIES {
            return Ok(AuthorityChainInspection {
                failure: Some(format!(
                    "authority directory exceeds the {MAX_AUTHORITY_ENTRIES}-entry bound"
                )),
                ..AuthorityChainInspection::default()
            });
        }
        let entry = entry
            .map_err(|error| format!("cannot inspect an authority directory entry: {error}"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "authority entry name is not valid Unicode".to_string())?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect authority entry type: {error}"))?;
        if !file_type.is_file() {
            unexpected_entry_count += 1;
        } else if name.ends_with("-authorization.json") {
            authorization_entries.push((name, entry.path()));
        } else if name.ends_with("-terminal.json") {
            terminal_entries.push((name, entry.path()));
        } else {
            unexpected_entry_count += 1;
        }
    }
    authorization_entries.sort_by(|left, right| left.0.cmp(&right.0));
    terminal_entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut inspection = AuthorityChainInspection::default();
    if unexpected_entry_count != 0 {
        append_authority_failure(
            &mut inspection.failure,
            "authority directory contains unexpected entries".into(),
        );
    }
    if authorization_entries.len() > MAX_OFFLINE_STAGES {
        append_authority_failure(
            &mut inspection.failure,
            format!(
                "authority directory contains more than {MAX_OFFLINE_STAGES} authorization envelopes"
            ),
        );
    }

    let mut previous_head: Option<CampaignCheckpointHead> = None;
    for (index, (name, path)) in authorization_entries.iter().enumerate() {
        let expected_generation = u64::try_from(index + 1)
            .map_err(|_| "authorization generation overflowed".to_string())?;
        match verify_authorization_envelope(
            prepared,
            name,
            path,
            expected_generation,
            previous_head.as_ref(),
        ) {
            Ok(checkpoint) => {
                previous_head = Some(checkpoint.head());
                inspection
                    .authorizations
                    .push(VerifiedAuthorizationEnvelope {
                        locator: format!("authority/{name}"),
                        checkpoint,
                    });
            }
            Err(error) => {
                append_authority_failure(
                    &mut inspection.failure,
                    format!(
                        "authorization envelope at ordinal {expected_generation} is not trusted ({error})"
                    ),
                );
                break;
            }
        }
    }

    if terminal_entries.len() > 1 {
        append_authority_failure(
            &mut inspection.failure,
            "authority directory contains more than one terminal envelope".into(),
        );
    } else if authorization_entries.len() == inspection.authorizations.len() {
        if let Some((name, path)) = terminal_entries.first() {
            match previous_head.as_ref() {
                Some(expected_head) => {
                    match verify_terminal_envelope(prepared, name, path, expected_head) {
                        Ok(checkpoint) => {
                            inspection.terminal = Some(VerifiedTerminalEnvelope {
                                locator: format!("authority/{name}"),
                                checkpoint,
                            });
                        }
                        Err(error) => append_authority_failure(
                            &mut inspection.failure,
                            format!("terminal envelope is not trusted ({error})"),
                        ),
                    }
                }
                None => append_authority_failure(
                    &mut inspection.failure,
                    "terminal envelope has no preceding authorization envelope".into(),
                ),
            }
        }
    }
    Ok(inspection)
}

fn verify_authorization_envelope(
    prepared: &PreparedCampaign,
    name: &str,
    path: &Path,
    expected_generation: u64,
    previous_head: Option<&CampaignCheckpointHead>,
) -> Result<ValidatedCampaignCheckpoint, String> {
    let value = read_json_bounded(
        path,
        MAX_SPEC_BYTES.saturating_mul(3),
        "authorization envelope",
    )
    .map_err(|_| "authorization envelope cannot be read as bounded JSON".to_string())?;
    let stored: StoredAuthorizationEnvelope = serde_json::from_value(value)
        .map_err(|error| format!("authorization envelope schema mismatch: {error}"))?;
    if stored.schema != AUTHORIZATION_ENVELOPE_SCHEMA || stored.retention != RETENTION {
        return Err("schema or retention marker is invalid".into());
    }
    let checkpoint = bioprism_research_campaign::validate_campaign_checkpoint(&stored.checkpoint)
        .map_err(|error| format!("authorization checkpoint was refused: {error}"))?;
    verify_checkpoint_preflight(prepared, checkpoint.as_value())?;
    let head = checkpoint.head();
    let expected_name = format!("{:04}-authorization.json", checkpoint.generation());
    if name != expected_name || checkpoint.generation() != expected_generation {
        return Err(format!(
            "filename/generation is not the contiguous canonical authorization {expected_generation:04}"
        ));
    }
    let expected_previous_snapshot = previous_head
        .map(|head| Value::String(head.snapshot_digest().to_owned()))
        .unwrap_or(Value::Null);
    if checkpoint.as_value().get("previous_snapshot_digest") != Some(&expected_previous_snapshot) {
        return Err("checkpoint does not extend the preceding snapshot digest".into());
    }
    if stored.expected_checkpoint_head.as_ref() != previous_head
        || stored.candidate_checkpoint_head != head
        || stored.claim.expected_checkpoint_head.as_ref() != previous_head
        || stored.claim.candidate_checkpoint_head != head
    {
        return Err("envelope and claim checkpoint heads do not bind the stored checkpoint".into());
    }
    if head.campaign_id() != prepared.spec.campaign_id()
        || head.spec_digest() != prepared.spec.spec_digest()
    {
        return Err(
            "authorization checkpoint identifies a different campaign specification".into(),
        );
    }
    if checkpoint.as_value().get("status").and_then(Value::as_str) != Some("in_flight")
        || checkpoint
            .as_value()
            .get("actions_used")
            .and_then(Value::as_u64)
            != Some(u64::from(stored.claim.action_ordinal))
        || u64::from(stored.claim.action_ordinal) != expected_generation
        || checkpoint
            .as_value()
            .get("active_stage_id")
            .and_then(Value::as_str)
            != Some(&stored.claim.stage_id)
    {
        return Err("authorization claim does not identify the active checkpoint action".into());
    }

    let prepared_stage = prepared
        .stages
        .iter()
        .find(|stage| stage.stage_id == stored.claim.stage_id)
        .ok_or_else(|| "authorization claim names a stage absent from preflight".to_string())?;
    if stored.claim.kind != prepared_stage.kind
        || stored.claim.input_digest != prepared_stage.input_digest
    {
        return Err("authorization claim stage kind or input digest differs from preflight".into());
    }
    let checkpoint_stage = checkpoint
        .as_value()
        .get("stages")
        .and_then(Value::as_array)
        .and_then(|stages| {
            stages.iter().find(|stage| {
                stage.get("stage_id").and_then(Value::as_str)
                    == Some(stored.claim.stage_id.as_str())
            })
        })
        .ok_or_else(|| "authorization checkpoint has no claimed stage projection".to_string())?;
    let claim_kind = serde_json::to_value(stored.claim.kind)
        .map_err(|error| format!("cannot encode authorization claim kind: {error}"))?;
    if checkpoint_stage.get("kind") != Some(&claim_kind)
        || checkpoint_stage.get("input_digest").and_then(Value::as_str)
            != Some(&stored.claim.input_digest)
        || checkpoint_stage.get("state").and_then(Value::as_str) != Some("in_flight")
        || checkpoint_stage
            .get("action_ordinal")
            .and_then(Value::as_u64)
            != Some(u64::from(stored.claim.action_ordinal))
        || checkpoint_stage
            .get("authorization_digest")
            .and_then(Value::as_str)
            != Some(&stored.claim.authorization_digest)
    {
        return Err("authorization claim does not bind the active stage projection".into());
    }

    let latest_event = checkpoint
        .as_value()
        .get("events")
        .and_then(Value::as_array)
        .and_then(|events| events.last())
        .ok_or_else(|| "authorization checkpoint has no latest authorization event".to_string())?;
    if latest_event.get("transition").and_then(Value::as_str) != Some("authorized")
        || latest_event.get("stage_id").and_then(Value::as_str) != Some(&stored.claim.stage_id)
        || latest_event.get("kind") != Some(&claim_kind)
        || latest_event.get("input_digest").and_then(Value::as_str)
            != Some(&stored.claim.input_digest)
        || latest_event.get("action_ordinal").and_then(Value::as_u64)
            != Some(u64::from(stored.claim.action_ordinal))
        || latest_event
            .get("authorization_digest")
            .and_then(Value::as_str)
            != Some(&stored.claim.authorization_digest)
    {
        return Err("authorization claim does not bind the latest checkpoint event".into());
    }
    let expected_authorization_predecessor = match latest_event.get("previous_event_digest") {
        Some(Value::String(digest)) => digest.clone(),
        Some(Value::Null) => ContentHash::of_value(&serde_json::json!({
            "campaign_id": prepared.spec.campaign_id(),
            "spec_digest": prepared.spec.spec_digest(),
            "events": [],
        }))
        .map_err(|error| format!("cannot derive initial event-chain digest: {error}"))?
        .to_string(),
        _ => return Err("latest authorization event has an invalid predecessor".into()),
    };
    if stored.claim.authorization_predecessor_digest != expected_authorization_predecessor {
        return Err(
            "authorization claim predecessor does not bind the event-chain boundary".into(),
        );
    }
    Ok(checkpoint)
}

fn verify_terminal_envelope(
    prepared: &PreparedCampaign,
    name: &str,
    path: &Path,
    expected_head: &CampaignCheckpointHead,
) -> Result<ValidatedCampaignCheckpoint, String> {
    let value = read_json_bounded(path, MAX_SPEC_BYTES.saturating_mul(3), "terminal envelope")
        .map_err(|_| "terminal envelope cannot be read as bounded JSON".to_string())?;
    let stored: StoredTerminalEnvelope = serde_json::from_value(value)
        .map_err(|error| format!("terminal envelope schema mismatch: {error}"))?;
    if stored.schema != TERMINAL_ENVELOPE_SCHEMA || stored.retention != RETENTION {
        return Err("schema or retention marker is invalid".into());
    }
    let checkpoint = bioprism_research_campaign::validate_campaign_checkpoint(&stored.checkpoint)
        .map_err(|error| format!("terminal checkpoint was refused: {error}"))?;
    verify_checkpoint_preflight(prepared, checkpoint.as_value())?;
    let head = checkpoint.head();
    let expected_generation = expected_head
        .generation()
        .checked_add(1)
        .ok_or_else(|| "terminal generation overflowed".to_string())?;
    let expected_name = format!("{expected_generation:04}-terminal.json");
    if name != expected_name || checkpoint.generation() != expected_generation {
        return Err("filename/generation is not the canonical terminal successor".into());
    }
    if stored.expected_checkpoint_head != *expected_head
        || stored.candidate_checkpoint_head != head
        || checkpoint
            .as_value()
            .get("previous_snapshot_digest")
            .and_then(Value::as_str)
            != Some(expected_head.snapshot_digest())
    {
        return Err("terminal envelope does not extend the last authorization head".into());
    }
    if matches!(
        checkpoint.as_value().get("status").and_then(Value::as_str),
        Some("planned" | "ready" | "in_flight") | None
    ) {
        return Err("terminal checkpoint retains a non-terminal campaign status".into());
    }
    Ok(checkpoint)
}

fn verify_checkpoint_preflight(
    prepared: &PreparedCampaign,
    checkpoint: &Value,
) -> Result<(), String> {
    if checkpoint.get("campaign_id").and_then(Value::as_str) != Some(prepared.spec.campaign_id())
        || checkpoint.get("spec_digest").and_then(Value::as_str)
            != Some(prepared.spec.spec_digest())
        || checkpoint.get("max_actions").and_then(Value::as_u64)
            != Some(u64::from(prepared.spec.max_actions()))
    {
        return Err("checkpoint campaign identity or action ceiling differs from preflight".into());
    }
    let stored_stages = checkpoint
        .get("stages")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated checkpoint has no stage projections".to_string())?;
    if stored_stages.len() != prepared.stages.len() {
        return Err("checkpoint stage count differs from preflight".into());
    }
    for ((stored, prepared_stage), spec_stage) in stored_stages
        .iter()
        .zip(&prepared.stages)
        .zip(prepared.spec.stages())
    {
        let expected_kind = serde_json::to_value(prepared_stage.kind)
            .map_err(|error| format!("cannot encode preflight stage kind: {error}"))?;
        let expected_dependencies = serde_json::to_value(spec_stage.depends_on())
            .map_err(|error| format!("cannot encode preflight stage dependencies: {error}"))?;
        if stored.get("stage_id").and_then(Value::as_str) != Some(&prepared_stage.stage_id)
            || stored.get("kind") != Some(&expected_kind)
            || stored.get("input_digest").and_then(Value::as_str)
                != Some(&prepared_stage.input_digest)
            || stored.get("depends_on") != Some(&expected_dependencies)
        {
            return Err(format!(
                "checkpoint stage {:?} differs from the preflighted specification",
                prepared_stage.stage_id
            ));
        }
    }
    Ok(())
}

fn append_authority_failure(failure: &mut Option<String>, detail: String) {
    match failure {
        Some(existing) => {
            existing.push_str("; ");
            existing.push_str(&detail);
        }
        None => *failure = Some(detail),
    }
}

fn inspect_existing(prepared: PreparedCampaign) -> Result<OfflineCampaignResponse, String> {
    let committed = prepared.output_dir.join("campaign.head.json").is_file()
        && prepared
            .output_dir
            .join("campaign.checkpoint.json")
            .is_file()
        && prepared.output_dir.join("campaign.manifest.json").is_file();
    if committed {
        match inspect_committed(&prepared) {
            Ok(response) => return Ok(response),
            Err(reason) => {
                return inspect_partial(
                    prepared,
                    format!(
                        "the apparent commit marker did not verify ({reason}); no action will be retried"
                    ),
                )
            }
        }
    }
    inspect_partial(
        prepared,
        "the append-only output has no complete checkpoint/manifest/head commit".into(),
    )
}

fn inspect_committed(prepared: &PreparedCampaign) -> Result<OfflineCampaignResponse, String> {
    let manifest_path = prepared.output_dir.join("campaign.manifest.json");
    let manifest_bytes = read_bytes_bounded(&manifest_path, MAX_SPEC_BYTES, "campaign manifest")?;
    let manifest_value: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("invalid JSON in campaign manifest: {error}"))?;
    let manifest: StoredCampaignManifest = serde_json::from_value(manifest_value)
        .map_err(|error| format!("campaign manifest does not match its exact schema: {error}"))?;
    let expected_limitations = LIMITATIONS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    if manifest.schema != OFFLINE_CAMPAIGN_MANIFEST_SCHEMA
        || manifest.retention != RETENTION
        || manifest.limitations != expected_limitations
        || manifest.campaign_id != prepared.spec.campaign_id()
        || manifest.spec_digest != prepared.spec.spec_digest()
    {
        return Err("campaign manifest identity, schema, or retention markers do not match".into());
    }
    if manifest.checkpoint.locator != "campaign.checkpoint.json"
        || manifest.trusted_head.locator != "campaign.head.json"
    {
        return Err("campaign manifest uses unexpected checkpoint or head locators".into());
    }

    let checkpoint_value = read_json_bounded(
        &prepared.output_dir.join("campaign.checkpoint.json"),
        MAX_SPEC_BYTES.saturating_mul(2),
        "terminal campaign checkpoint",
    )?;
    let checkpoint = bioprism_research_campaign::validate_campaign_checkpoint(&checkpoint_value)
        .map_err(|error| format!("terminal campaign checkpoint was refused: {error}"))?;
    let head_value = read_json_bounded(
        &prepared.output_dir.join("campaign.head.json"),
        MAX_SPEC_BYTES,
        "trusted campaign head",
    )?;
    let head: CampaignCheckpointHead = serde_json::from_value(head_value)
        .map_err(|error| format!("trusted campaign head was refused: {error}"))?;
    let checkpoint_head = checkpoint.head();
    if head != checkpoint_head
        || manifest.checkpoint.generation != checkpoint.generation()
        || manifest.checkpoint.snapshot_digest != checkpoint.snapshot_digest()
        || manifest.checkpoint.schema != RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA
        || manifest.trusted_head.campaign_id != head.campaign_id()
        || manifest.trusted_head.spec_digest != head.spec_digest()
        || manifest.trusted_head.generation != head.generation()
        || manifest.trusted_head.snapshot_digest != head.snapshot_digest()
    {
        return Err("checkpoint, trusted head, and manifest do not name one exact snapshot".into());
    }
    if checkpoint_value.get("status")
        != Some(
            &serde_json::to_value(manifest.campaign_status)
                .map_err(|error| format!("cannot encode stored campaign status: {error}"))?,
        )
        || checkpoint_value.get("actions_used").and_then(Value::as_u64)
            != Some(u64::from(manifest.actions_used))
    {
        return Err("manifest status or action count does not match the checkpoint".into());
    }

    let authority = inspect_authority_chain(prepared)?;
    if let Some(failure) = authority.failure {
        return Err(format!(
            "append-only authority chain did not verify: {failure}"
        ));
    }
    if authority.authorizations.len() != usize::from(manifest.actions_used) {
        return Err("authorization envelope count does not match actions_used".into());
    }
    let terminal = authority
        .terminal
        .ok_or_else(|| "committed output has no verified terminal envelope".to_string())?;
    let expected_terminal_locator =
        format!("authority/{:04}-terminal.json", checkpoint.generation());
    if terminal.locator != expected_terminal_locator
        || terminal.checkpoint.as_value() != &checkpoint_value
        || terminal.checkpoint.head() != head
    {
        return Err(
            "terminal envelope, committed checkpoint, and trusted head are not identical".into(),
        );
    }

    verify_stored_outcomes(prepared, &manifest.stages, manifest.actions_used)?;
    verify_outcomes_against_checkpoint(&manifest.stages, &checkpoint_value)?;
    let last_stage_id = manifest.stages.iter().rev().find_map(outcome_stage_id);
    let execution = execution_for_status(manifest.campaign_status, last_stage_id)?;
    let written = expected_committed_written(
        prepared,
        &manifest.stages,
        manifest.actions_used,
        checkpoint.generation(),
    )?;
    let manifest_digest = ContentHash::of_bytes(&manifest_bytes).to_string();

    Ok(OfflineCampaignResponse {
        schema: OFFLINE_CAMPAIGN_SCHEMA,
        workflow: "research_campaign_run_offline",
        execution,
        campaign_id: manifest.campaign_id,
        spec_digest: manifest.spec_digest,
        campaign_status: manifest.campaign_status,
        actions_used: manifest.actions_used,
        stages: manifest.stages,
        checkpoint: Some(manifest.checkpoint),
        trusted_head: Some(manifest.trusted_head),
        manifest: Some(ManifestSummary {
            locator: "campaign.manifest.json".into(),
            digest: manifest_digest.clone(),
            file_sha256: manifest_digest,
        }),
        written,
        limitations: &LIMITATIONS,
    })
}

fn inspect_partial(
    prepared: PreparedCampaign,
    initial_reason: String,
) -> Result<OfflineCampaignResponse, String> {
    let authority = inspect_authority_chain(&prepared)?;
    let mut reason = initial_reason;
    if let Some(failure) = authority.failure.as_deref() {
        reason = format!(
            "{reason}; append-only authority chain could not be fully trusted ({failure}), so no later action will be inferred"
        );
    }
    if authority.terminal.is_some() {
        reason = format!(
            "{reason}; a terminal authority envelope exists without one complete verified public commit"
        );
    }
    let valid_authorization_locators = authority
        .authorizations
        .iter()
        .map(|record| record.locator.clone())
        .collect::<Vec<_>>();
    let latest = authority
        .authorizations
        .last()
        .map(|record| (record.locator.clone(), record.checkpoint.clone()));

    let Some((locator, checkpoint)) = latest else {
        return Ok(OfflineCampaignResponse {
            schema: OFFLINE_CAMPAIGN_SCHEMA,
            workflow: "research_campaign_run_offline",
            execution: OfflineExecution::ReconciliationRequired { reason },
            campaign_id: prepared.spec.campaign_id().to_owned(),
            spec_digest: prepared.spec.spec_digest().to_owned(),
            campaign_status: CampaignStatus::ReconciliationRequired,
            actions_used: 0,
            stages: prepared
                .stages
                .into_iter()
                .map(not_started_outcome)
                .collect(),
            checkpoint: None,
            trusted_head: None,
            manifest: None,
            written: Vec::new(),
            limitations: &LIMITATIONS,
        });
    };

    let checkpoint_value = checkpoint.as_value();
    let actions_used = checkpoint_value
        .get("actions_used")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| "validated partial checkpoint has no bounded actions_used".to_string())?;
    let stages = partial_outcomes_from_checkpoint(&prepared, checkpoint_value)?;
    let head = checkpoint.head();
    let written = recognized_partial_written(&prepared, &stages, &valid_authorization_locators);
    reason = format!(
        "{reason}; durable authorization generation {} is fenced and requires execution-journal reconciliation",
        checkpoint.generation()
    );
    Ok(OfflineCampaignResponse {
        schema: OFFLINE_CAMPAIGN_SCHEMA,
        workflow: "research_campaign_run_offline",
        execution: OfflineExecution::ReconciliationRequired { reason },
        campaign_id: prepared.spec.campaign_id().to_owned(),
        spec_digest: prepared.spec.spec_digest().to_owned(),
        campaign_status: CampaignStatus::ReconciliationRequired,
        actions_used,
        stages,
        checkpoint: Some(CheckpointSummary {
            locator: format!("{locator}#/checkpoint"),
            schema: RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA.into(),
            generation: checkpoint.generation(),
            snapshot_digest: checkpoint.snapshot_digest().to_owned(),
        }),
        trusted_head: Some(TrustedHeadSummary {
            locator: format!("{locator}#/candidate_checkpoint_head"),
            campaign_id: head.campaign_id().to_owned(),
            spec_digest: head.spec_digest().to_owned(),
            generation: head.generation(),
            snapshot_digest: head.snapshot_digest().to_owned(),
        }),
        manifest: None,
        written,
        limitations: &LIMITATIONS,
    })
}

fn verify_stored_outcomes(
    prepared: &PreparedCampaign,
    outcomes: &[OfflineStageOutcome],
    actions_used: u16,
) -> Result<(), String> {
    if outcomes.len() != prepared.stages.len() {
        return Err("stored manifest stage count does not match the campaign spec".into());
    }
    let mut ordinals = BTreeSet::new();
    for (prepared_stage, outcome) in prepared.stages.iter().zip(outcomes) {
        match outcome {
            OfflineStageOutcome::NotStarted {
                stage_id,
                kind,
                input_digest,
                artifact_locator,
            } => verify_stage_identity(
                prepared_stage,
                stage_id,
                *kind,
                input_digest,
                artifact_locator,
            )?,
            OfflineStageOutcome::Settled {
                stage_id,
                kind,
                input_digest,
                action_ordinal,
                disposition,
                artifact_digest,
                receipt_digest,
                artifact_locator,
                file_sha256,
            } => {
                verify_stage_identity(
                    prepared_stage,
                    stage_id,
                    *kind,
                    input_digest,
                    artifact_locator,
                )?;
                if !ordinals.insert(*action_ordinal) {
                    return Err("stored manifest repeats an action ordinal".into());
                }
                let bytes = read_bytes_bounded(
                    &prepared.output_dir.join(artifact_locator),
                    MAX_TOTAL_ARTIFACT_BYTES as u64,
                    "campaign artifact",
                )?;
                if ContentHash::of_bytes(&bytes).to_string() != *file_sha256 {
                    return Err(format!(
                        "stored artifact for stage {stage_id:?} changed bytes"
                    ));
                }
                let artifact: Value = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("stored stage artifact is invalid JSON: {error}"))?;
                let stage = prepared
                    .spec
                    .stage(stage_id)
                    .ok_or_else(|| "stored outcome names an unknown campaign stage".to_string())?;
                let receipt = match &prepared_stage.input {
                    PreparedInput::SyntheticResearch(_) => {
                        VerifiedCampaignReceipt::from_research_dossier(stage, &artifact)
                    }
                    PreparedInput::BrainPlan(request) => {
                        let expected = serde_json::to_value(plan_autonomous(request).map_err(|_| {
                            format!("stored brain_plan stage {stage_id:?} no longer replays")
                        })?)
                        .map_err(|error| format!("cannot encode replayed brain plan: {error}"))?;
                        if expected != artifact {
                            return Err(format!(
                                "stored brain_plan artifact for stage {stage_id:?} does not exactly replay"
                            ));
                        }
                        VerifiedCampaignReceipt::from_brain_plan(stage, request)
                    }
                }
                .map_err(|_| format!("stored artifact for stage {stage_id:?} failed native verification"))?;
                if receipt.disposition() != *disposition
                    || receipt.artifact_digest() != artifact_digest
                    || receipt
                        .projection_digest()
                        .map_err(|error| format!("cannot digest replayed receipt: {error}"))?
                        != *receipt_digest
                {
                    return Err(format!(
                        "stored receipt metadata for stage {stage_id:?} does not match native replay"
                    ));
                }
            }
            OfflineStageOutcome::ReconciliationRequired {
                stage_id,
                kind,
                input_digest,
                action_ordinal,
                authorization_digest,
                artifact_locator,
                ..
            } => {
                verify_stage_identity(
                    prepared_stage,
                    stage_id,
                    *kind,
                    input_digest,
                    artifact_locator,
                )?;
                ContentHash::parse(authorization_digest.clone()).map_err(|_| {
                    format!(
                        "stored reconciliation stage {stage_id:?} has an invalid authorization digest"
                    )
                })?;
                if !ordinals.insert(*action_ordinal) {
                    return Err("stored manifest repeats an action ordinal".into());
                }
            }
        }
    }
    let expected_ordinals = (1..=actions_used).collect::<BTreeSet<_>>();
    if ordinals != expected_ordinals {
        return Err("stored action ordinals do not exactly cover actions_used".into());
    }
    Ok(())
}

fn verify_stage_identity(
    prepared: &PreparedStage,
    stage_id: &str,
    kind: CampaignActionKind,
    input_digest: &str,
    artifact_locator: &str,
) -> Result<(), String> {
    if stage_id != prepared.stage_id
        || kind != prepared.kind
        || input_digest != prepared.input_digest
        || artifact_locator != prepared.artifact_locator
    {
        return Err(format!(
            "stored outcome for stage {:?} does not match preflighted metadata",
            prepared.stage_id
        ));
    }
    Ok(())
}

fn verify_outcomes_against_checkpoint(
    outcomes: &[OfflineStageOutcome],
    checkpoint: &Value,
) -> Result<(), String> {
    let checkpoint_stages = checkpoint
        .get("stages")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated checkpoint has no stages array".to_string())?;
    if checkpoint_stages.len() != outcomes.len() {
        return Err("manifest and checkpoint stage counts differ".into());
    }
    for (outcome, stored) in outcomes.iter().zip(checkpoint_stages) {
        let stored_id = stored.get("stage_id").and_then(Value::as_str);
        let expected_state = match outcome {
            OfflineStageOutcome::NotStarted { stage_id, .. } => {
                if stored_id != Some(stage_id) {
                    return Err("manifest and checkpoint stage identities differ".into());
                }
                "pending"
            }
            OfflineStageOutcome::Settled {
                stage_id,
                action_ordinal,
                disposition,
                artifact_digest,
                ..
            } => {
                if stored_id != Some(stage_id)
                    || stored.get("action_ordinal").and_then(Value::as_u64)
                        != Some(u64::from(*action_ordinal))
                    || stored.pointer("/receipt/disposition")
                        != Some(&serde_json::to_value(disposition).map_err(|error| {
                            format!("cannot encode stored disposition: {error}")
                        })?)
                    || stored
                        .pointer("/receipt/artifact_digest")
                        .and_then(Value::as_str)
                        != Some(artifact_digest)
                {
                    return Err("manifest settled outcome does not match checkpoint receipt".into());
                }
                "settled"
            }
            OfflineStageOutcome::ReconciliationRequired {
                stage_id,
                action_ordinal,
                authorization_digest,
                ..
            } => {
                if stored_id != Some(stage_id)
                    || stored.get("action_ordinal").and_then(Value::as_u64)
                        != Some(u64::from(*action_ordinal))
                    || stored.get("authorization_digest").and_then(Value::as_str)
                        != Some(authorization_digest)
                {
                    return Err(
                        "manifest reconciliation outcome does not match checkpoint action".into(),
                    );
                }
                "uncertain"
            }
        };
        if stored.get("state").and_then(Value::as_str) != Some(expected_state) {
            return Err("manifest and checkpoint stage states differ".into());
        }
    }
    Ok(())
}

fn expected_committed_written(
    prepared: &PreparedCampaign,
    outcomes: &[OfflineStageOutcome],
    actions_used: u16,
    terminal_generation: u64,
) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for generation in 1..=u64::from(actions_used) {
        let locator = format!("authority/{generation:04}-authorization.json");
        if !prepared.output_dir.join(&locator).is_file() {
            return Err(format!("committed output is missing {locator}"));
        }
        written.push(join_display(&prepared.output_display, &locator));
    }
    let terminal = format!("authority/{terminal_generation:04}-terminal.json");
    if !prepared.output_dir.join(&terminal).is_file() {
        return Err(format!("committed output is missing {terminal}"));
    }
    written.push(join_display(&prepared.output_display, &terminal));
    for outcome in outcomes {
        if let OfflineStageOutcome::Settled {
            artifact_locator, ..
        } = outcome
        {
            written.push(join_display(&prepared.output_display, artifact_locator));
        }
    }
    for locator in [
        "campaign.checkpoint.json",
        "campaign.manifest.json",
        "campaign.head.json",
    ] {
        written.push(join_display(&prepared.output_display, locator));
    }
    written.sort();
    Ok(written)
}

fn partial_outcomes_from_checkpoint(
    prepared: &PreparedCampaign,
    checkpoint: &Value,
) -> Result<Vec<OfflineStageOutcome>, String> {
    let stored = checkpoint
        .get("stages")
        .and_then(Value::as_array)
        .ok_or_else(|| "validated partial checkpoint has no stages array".to_string())?;
    if stored.len() != prepared.stages.len() {
        return Err("partial checkpoint stage count does not match preflight".into());
    }
    let mut outcomes = Vec::with_capacity(stored.len());
    for (prepared_stage, stage) in prepared.stages.iter().zip(stored) {
        if stage.get("stage_id").and_then(Value::as_str) != Some(&prepared_stage.stage_id)
            || stage.get("input_digest").and_then(Value::as_str)
                != Some(&prepared_stage.input_digest)
        {
            return Err("partial checkpoint stage identity does not match preflight".into());
        }
        match stage.get("state").and_then(Value::as_str) {
            Some("pending") => outcomes.push(prepared_not_started_outcome(prepared_stage)),
            Some("settled") => {
                let action_ordinal = stage
                    .get("action_ordinal")
                    .and_then(Value::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or_else(|| "partial checkpoint action ordinal is invalid".to_string())?;
                let receipt_value = stage.get("receipt").ok_or_else(|| {
                    "settled partial checkpoint stage has no receipt projection".to_string()
                })?;
                let campaign_stage = prepared
                    .spec
                    .stage(&prepared_stage.stage_id)
                    .ok_or_else(|| "partial checkpoint names an unknown stage".to_string())?;
                let replayed = execute_prepared_stage(prepared_stage, campaign_stage).map_err(|_| {
                    format!(
                        "previously settled stage {:?} no longer produces a verified native artifact",
                        prepared_stage.stage_id
                    )
                })?;
                let artifact_path = prepared.output_dir.join(&prepared_stage.artifact_locator);
                let stored_artifact = read_bytes_bounded(
                    &artifact_path,
                    MAX_TOTAL_ARTIFACT_BYTES as u64,
                    "previously settled campaign artifact",
                )?;
                if stored_artifact != replayed.bytes
                    || receipt_value.get("artifact_digest").and_then(Value::as_str)
                        != Some(&replayed.artifact_digest)
                    || ContentHash::of_value(receipt_value)
                        .map_err(|error| {
                            format!("cannot digest partial checkpoint receipt: {error}")
                        })?
                        .to_string()
                        != replayed.receipt_digest
                {
                    return Err(format!(
                        "previously settled stage {:?} does not match its native replay",
                        prepared_stage.stage_id
                    ));
                }
                outcomes.push(OfflineStageOutcome::Settled {
                    stage_id: prepared_stage.stage_id.clone(),
                    kind: prepared_stage.kind,
                    input_digest: prepared_stage.input_digest.clone(),
                    action_ordinal,
                    disposition: replayed.disposition,
                    artifact_digest: replayed.artifact_digest,
                    receipt_digest: replayed.receipt_digest,
                    artifact_locator: prepared_stage.artifact_locator.clone(),
                    file_sha256: replayed.file_sha256,
                });
            }
            Some("in_flight") | Some("uncertain") => {
                let action_ordinal = stage
                    .get("action_ordinal")
                    .and_then(Value::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or_else(|| "partial checkpoint action ordinal is invalid".to_string())?;
                let authorization_digest = stage
                    .get("authorization_digest")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "partial checkpoint authorization digest is absent".to_string())?
                    .to_owned();
                outcomes.push(OfflineStageOutcome::ReconciliationRequired {
                    stage_id: prepared_stage.stage_id.clone(),
                    kind: prepared_stage.kind,
                    input_digest: prepared_stage.input_digest.clone(),
                    action_ordinal,
                    authorization_digest,
                    artifact_locator: prepared_stage.artifact_locator.clone(),
                    reason: "the latest trusted authorization chain does not end in a committed campaign head".into(),
                });
            }
            _ => return Err("partial checkpoint has an unknown stage state".into()),
        }
    }
    if outcomes
        .iter()
        .filter(|outcome| matches!(outcome, OfflineStageOutcome::ReconciliationRequired { .. }))
        .count()
        != 1
    {
        return Err(
            "partial authorization checkpoint must contain exactly one active reconciliation stage"
                .into(),
        );
    }
    Ok(outcomes)
}

fn recognized_partial_written(
    prepared: &PreparedCampaign,
    outcomes: &[OfflineStageOutcome],
    authorization_locators: &[String],
) -> Vec<String> {
    let mut written = authorization_locators
        .iter()
        .map(|locator| join_display(&prepared.output_display, locator))
        .collect::<Vec<_>>();
    for outcome in outcomes {
        let locator = match outcome {
            OfflineStageOutcome::Settled {
                artifact_locator, ..
            }
            | OfflineStageOutcome::ReconciliationRequired {
                artifact_locator, ..
            } => artifact_locator,
            OfflineStageOutcome::NotStarted { .. } => continue,
        };
        if prepared.output_dir.join(locator).is_file() {
            written.push(join_display(&prepared.output_display, locator));
        }
    }
    written.sort();
    written
}

fn outcome_stage_id(outcome: &OfflineStageOutcome) -> Option<&str> {
    match outcome {
        OfflineStageOutcome::NotStarted { .. } => None,
        OfflineStageOutcome::Settled { stage_id, .. }
        | OfflineStageOutcome::ReconciliationRequired { stage_id, .. } => Some(stage_id),
    }
}

struct VerifiedStageArtifact {
    bytes: Vec<u8>,
    receipt: VerifiedCampaignReceipt,
    disposition: CampaignReceiptDisposition,
    artifact_digest: String,
    receipt_digest: String,
    file_sha256: String,
}

fn execute_prepared_stage(
    prepared: &PreparedStage,
    campaign_stage: &bioprism_research_campaign::CampaignStageSpec,
) -> Result<VerifiedStageArtifact, String> {
    let (artifact, receipt) = match &prepared.input {
        PreparedInput::SyntheticResearch(request) => {
            let dossier = run_research(request).map_err(|_| {
                "the native synthetic research runner did not produce a dossier".to_string()
            })?;
            let receipt = VerifiedCampaignReceipt::from_research_dossier(campaign_stage, &dossier)
                .map_err(|_| {
                    "the synthetic research dossier failed exact native replay".to_string()
                })?;
            (dossier, receipt)
        }
        PreparedInput::BrainPlan(request) => {
            let report = plan_autonomous(request)
                .map_err(|_| "the native brain planner did not produce a report".to_string())?;
            let artifact = serde_json::to_value(report)
                .map_err(|_| "the native brain planning report could not be encoded".to_string())?;
            let receipt = VerifiedCampaignReceipt::from_brain_plan(campaign_stage, request)
                .map_err(|_| "the brain planning report failed exact native replay".to_string())?;
            (artifact, receipt)
        }
    };
    let bytes = to_canonical_bytes(&artifact)
        .map_err(|_| "the verified stage artifact could not be canonicalized".to_string())?;
    let disposition = receipt.disposition();
    let artifact_digest = receipt.artifact_digest().to_owned();
    let receipt_digest = receipt
        .projection_digest()
        .map_err(|_| "the verified stage receipt could not be digested".to_string())?;
    let file_sha256 = ContentHash::of_bytes(&bytes).to_string();
    Ok(VerifiedStageArtifact {
        bytes,
        receipt,
        disposition,
        artifact_digest,
        receipt_digest,
        file_sha256,
    })
}

fn execute(prepared: PreparedCampaign) -> Result<OfflineCampaignResponse, String> {
    let coordinator =
        AppendOnlyFileCoordinator::create(&prepared.output_dir, &prepared.output_display)?;
    let mut campaign = start_campaign(prepared.spec.clone())
        .map_err(|error| format!("cannot start offline campaign: {error}"))?;
    let mut outcomes = prepared
        .stages
        .iter()
        .map(prepared_not_started_outcome)
        .collect::<Vec<_>>();
    let mut artifact_written = Vec::new();
    let mut total_artifact_bytes = 0_usize;
    let mut last_stage_id = None;

    while matches!(
        campaign.status(),
        CampaignStatus::Planned | CampaignStatus::Ready
    ) {
        let authorization = campaign
            .authorize_next_action(&coordinator)
            .map_err(|error| format!("campaign action authorization refused: {error}"))?;
        let stage_id = authorization.stage_id().to_owned();
        let stage_index = prepared
            .stages
            .iter()
            .position(|stage| stage.stage_id == stage_id)
            .ok_or_else(|| "campaign authorized a stage absent from preflight".to_string())?;
        let prepared_stage = &prepared.stages[stage_index];
        if authorization.kind() != prepared_stage.kind
            || authorization.input_digest() != prepared_stage.input_digest
        {
            return Err("campaign authorization does not match preflighted stage input".into());
        }
        let campaign_stage = campaign
            .spec()
            .stage(&stage_id)
            .cloned()
            .ok_or_else(|| "campaign authorization names an unknown stage".to_string())?;
        let action_ordinal = authorization.action_ordinal();
        let authorization_digest = authorization.authorization_digest().to_owned();
        let attempt =
            execute_prepared_stage(prepared_stage, &campaign_stage).and_then(|artifact| {
                let next_total = total_artifact_bytes
                    .checked_add(artifact.bytes.len())
                    .ok_or_else(|| "offline campaign artifact byte count overflowed".to_string())?;
                if next_total > MAX_TOTAL_ARTIFACT_BYTES {
                    return Err(format!(
                    "offline campaign artifacts exceed the {MAX_TOTAL_ARTIFACT_BYTES}-byte bound"
                ));
                }
                let artifact_path = prepared.output_dir.join(&prepared_stage.artifact_locator);
                write_new_bytes(&artifact_path, &artifact.bytes).map_err(|_| {
                    "the verified stage artifact could not be durably stored".to_string()
                })?;
                Ok((artifact, next_total))
            });
        let (artifact, next_total) = match attempt {
            Ok(value) => value,
            Err(reason) => {
                let observation = ContentHash::of_value(&serde_json::json!({
                    "schema": "bioprism-mcp/research-campaign-stage-observation/0.1",
                    "campaign_id": campaign.spec().campaign_id(),
                    "spec_digest": campaign.spec().spec_digest(),
                    "stage_id": stage_id,
                    "action_ordinal": action_ordinal,
                    "authorization_digest": authorization_digest,
                    "classification": "verified_completion_not_available"
                }))
                .map_err(|error| format!("cannot digest stage failure observation: {error}"))?
                .to_string();
                let unknown =
                    VerifiedCampaignReceipt::unknown_completion(&campaign_stage, observation)
                        .map_err(|error| {
                            format!("cannot record unknown stage completion: {error}")
                        })?;
                campaign
                    .apply_receipt(authorization, unknown)
                    .map_err(|error| format!("cannot fence unknown stage completion: {error}"))?;
                outcomes[stage_index] = OfflineStageOutcome::ReconciliationRequired {
                    stage_id: stage_id.clone(),
                    kind: prepared_stage.kind,
                    input_digest: prepared_stage.input_digest.clone(),
                    action_ordinal,
                    authorization_digest,
                    artifact_locator: prepared_stage.artifact_locator.clone(),
                    reason,
                };
                last_stage_id = Some(stage_id);
                break;
            }
        };
        total_artifact_bytes = next_total;
        artifact_written.push(join_display(
            &prepared.output_display,
            &prepared_stage.artifact_locator,
        ));
        campaign
            .apply_receipt(authorization, artifact.receipt)
            .map_err(|error| format!("cannot apply verified campaign receipt: {error}"))?;
        outcomes[stage_index] = OfflineStageOutcome::Settled {
            stage_id: stage_id.clone(),
            kind: prepared_stage.kind,
            input_digest: prepared_stage.input_digest.clone(),
            action_ordinal,
            disposition: artifact.disposition,
            artifact_digest: artifact.artifact_digest,
            receipt_digest: artifact.receipt_digest,
            artifact_locator: prepared_stage.artifact_locator.clone(),
            file_sha256: artifact.file_sha256,
        };
        last_stage_id = Some(stage_id);
    }

    let execution = execution_for_status(campaign.status(), last_stage_id.as_deref())?;
    let checkpoint = seal_campaign_checkpoint(&mut campaign)
        .map_err(|error| format!("cannot seal terminal campaign checkpoint: {error}"))?;
    let trusted_head = coordinator.store_terminal(&checkpoint)?;
    let checkpoint_summary = CheckpointSummary {
        locator: "campaign.checkpoint.json".into(),
        schema: RESEARCH_CAMPAIGN_CHECKPOINT_SCHEMA.into(),
        generation: checkpoint.generation(),
        snapshot_digest: checkpoint.snapshot_digest().to_owned(),
    };
    let trusted_head_summary = TrustedHeadSummary {
        locator: "campaign.head.json".into(),
        campaign_id: trusted_head.campaign_id().to_owned(),
        spec_digest: trusted_head.spec_digest().to_owned(),
        generation: trusted_head.generation(),
        snapshot_digest: trusted_head.snapshot_digest().to_owned(),
    };

    write_new_canonical(
        &prepared.output_dir.join(&checkpoint_summary.locator),
        checkpoint.as_value(),
    )?;
    let manifest_value = serde_json::to_value(CampaignManifest {
        schema: OFFLINE_CAMPAIGN_MANIFEST_SCHEMA,
        campaign_id: campaign.spec().campaign_id(),
        spec_digest: campaign.spec().spec_digest(),
        campaign_status: campaign.status(),
        actions_used: campaign.actions_used(),
        stages: &outcomes,
        checkpoint: &checkpoint_summary,
        trusted_head: &trusted_head_summary,
        retention: RETENTION,
        limitations: &LIMITATIONS,
    })
    .map_err(|error| format!("cannot encode offline campaign manifest: {error}"))?;
    let manifest_bytes = to_canonical_bytes(&manifest_value)
        .map_err(|error| format!("cannot canonicalize offline campaign manifest: {error}"))?;
    let manifest_digest = ContentHash::of_bytes(&manifest_bytes).to_string();
    write_new_bytes(
        &prepared.output_dir.join("campaign.manifest.json"),
        &manifest_bytes,
    )?;
    let head_value = serde_json::to_value(&trusted_head)
        .map_err(|error| format!("cannot encode trusted campaign head: {error}"))?;
    write_new_canonical(
        &prepared.output_dir.join(&trusted_head_summary.locator),
        &head_value,
    )?;

    let mut written = coordinator.written()?;
    written.append(&mut artifact_written);
    written.push(join_display(
        &prepared.output_display,
        &checkpoint_summary.locator,
    ));
    written.push(join_display(
        &prepared.output_display,
        "campaign.manifest.json",
    ));
    written.push(join_display(
        &prepared.output_display,
        &trusted_head_summary.locator,
    ));
    written.sort();

    Ok(OfflineCampaignResponse {
        schema: OFFLINE_CAMPAIGN_SCHEMA,
        workflow: "research_campaign_run_offline",
        execution,
        campaign_id: campaign.spec().campaign_id().to_owned(),
        spec_digest: campaign.spec().spec_digest().to_owned(),
        campaign_status: campaign.status(),
        actions_used: campaign.actions_used(),
        stages: outcomes,
        checkpoint: Some(checkpoint_summary),
        trusted_head: Some(trusted_head_summary),
        manifest: Some(ManifestSummary {
            locator: "campaign.manifest.json".into(),
            digest: manifest_digest.clone(),
            file_sha256: manifest_digest,
        }),
        written,
        limitations: &LIMITATIONS,
    })
}

fn execution_for_status(
    status: CampaignStatus,
    last_stage_id: Option<&str>,
) -> Result<OfflineExecution, String> {
    let stage_id = || {
        last_stage_id
            .map(str::to_owned)
            .ok_or_else(|| format!("campaign reached {status:?} without a settled stage"))
    };
    match status {
        CampaignStatus::Completed => Ok(OfflineExecution::Completed),
        CampaignStatus::AwaitingHumanReview => Ok(OfflineExecution::AwaitingHumanReview {
            stage_id: stage_id()?,
        }),
        CampaignStatus::Refused => Ok(OfflineExecution::Refused {
            stage_id: stage_id()?,
        }),
        CampaignStatus::NeedsInput => Ok(OfflineExecution::NeedsInput {
            stage_id: stage_id()?,
        }),
        CampaignStatus::Exhausted => Ok(OfflineExecution::Exhausted {
            stage_id: stage_id()?,
        }),
        CampaignStatus::ReconciliationRequired => {
            Ok(OfflineExecution::ReconciliationRequired {
                reason: "the campaign has an uncertain authorized action and requires a caller-owned execution journal".into(),
            })
        }
        CampaignStatus::Planned | CampaignStatus::Ready | CampaignStatus::InFlight => Err(format!(
            "offline campaign stopped in non-terminal status {}",
            status.as_str()
        )),
    }
}

fn prepared_not_started_outcome(stage: &PreparedStage) -> OfflineStageOutcome {
    OfflineStageOutcome::NotStarted {
        stage_id: stage.stage_id.clone(),
        kind: stage.kind,
        input_digest: stage.input_digest.clone(),
        artifact_locator: stage.artifact_locator.clone(),
    }
}

fn not_started_outcome(stage: PreparedStage) -> OfflineStageOutcome {
    OfflineStageOutcome::NotStarted {
        stage_id: stage.stage_id,
        kind: stage.kind,
        input_digest: stage.input_digest,
        artifact_locator: stage.artifact_locator,
    }
}

fn validate_relative_label(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be a non-empty relative path"));
    }
    if value.len() > 4096 {
        return Err(format!("{field} exceeds the 4096-byte path bound"));
    }
    Ok(())
}

fn read_json_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Value, String> {
    let bytes = read_bytes_bounded(path, max_bytes, label)?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid JSON in {label}: {error}"))
}

fn read_bytes_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("cannot inspect {label}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} is not a file"));
    }
    if metadata.len() > max_bytes {
        return Err(format!("{label} exceeds the {max_bytes}-byte bound"));
    }
    fs::read(path).map_err(|error| format!("cannot read {label}: {error}"))
}

fn write_new_canonical(path: &Path, value: &Value) -> Result<String, String> {
    let bytes = to_canonical_bytes(value)
        .map_err(|error| format!("cannot canonicalize append-only campaign content: {error}"))?;
    write_new_bytes(path, &bytes)?;
    Ok(ContentHash::of_bytes(&bytes).to_string())
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create append-only campaign file: {error}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot durably write append-only campaign file: {error}"))
}

fn normalize_display_path(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_owned()
}

fn join_display(prefix: &str, suffix: &str) -> String {
    format!("{prefix}/{suffix}")
}
