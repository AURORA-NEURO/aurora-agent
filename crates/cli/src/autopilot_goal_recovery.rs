//! Durable, caller-controlled continuation for goal-level CLI steps.
//!
//! The goal report and checkpoint are metadata-only. The latest Autopilot report is retained in
//! a separate private file, and an intent marker is flushed before mission dispatch so a process
//! interruption cannot silently repeat work whose outcome is unknown.

use bioprism_autopilot::{
    validate_goal_control_checkpoint, verify_autopilot_report, verify_goal_control_report,
    GoalControlBudget,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const IDENTITY_SCHEMA: &str = "bioprism-autopilot-cli-goal-identity/0.1";
const HEAD_SCHEMA: &str = "bioprism-autopilot-cli-goal-head/0.1";
const PRIVATE_REPORT_SCHEMA: &str = "bioprism-autopilot-cli-goal-private-report/0.1";
const PENDING_SCHEMA: &str = "bioprism-autopilot-cli-goal-pending/0.1";
const TERMINAL_SCHEMA: &str = "bioprism-autopilot-cli-goal-terminal/0.1";
const MAX_GENERATIONS: usize = 256;
const MAX_CHECKPOINT_BYTES: usize = 2_000_000;
const MAX_PRIVATE_REPORT_BYTES: usize = 20_000_000;
const MAX_TOTAL_PRIVATE_REPORT_BYTES: usize = 80_000_000;
const MAX_TERMINAL_BYTES: usize = 2_000_000;

const IDENTITY_KEYS: &[&str] = &[
    "schema",
    "goal_id",
    "grant_sha256",
    "budget",
    "identity_sha256",
];
const HEAD_KEYS: &[&str] = &[
    "schema",
    "generation",
    "checkpoint_digest",
    "previous_head_digest",
    "private_report_generation",
    "private_report_digest",
    "committed_pending_digest",
    "head_digest",
];
const PRIVATE_REPORT_KEYS: &[&str] = &["schema", "generation", "autopilot_report", "record_digest"];
const PENDING_KEYS: &[&str] = &[
    "schema",
    "generation",
    "mission_digest",
    "previous_head_digest",
    "pending_digest",
];
const TERMINAL_KEYS: &[&str] = &[
    "schema",
    "goal_id",
    "grant_sha256",
    "previous_head_digest",
    "goal_control_report",
    "terminal_digest",
];

#[derive(Debug)]
pub struct GoalControlRecoveryStore {
    root: PathBuf,
    goal_id: String,
    grant_sha256: String,
    budget: GoalControlBudget,
    generation: usize,
    checkpoint: Option<Value>,
    latest_report_generation: Option<usize>,
    latest_report_digest: Option<String>,
    latest_report: Option<Value>,
    head_digest: Option<String>,
    pending_written: bool,
    total_private_report_bytes: usize,
}

#[derive(Debug)]
pub struct RestoredGoalControlState {
    pub store: GoalControlRecoveryStore,
    pub checkpoint: Option<Value>,
}

impl GoalControlRecoveryStore {
    pub fn create(
        root: impl Into<PathBuf>,
        goal_id: &str,
        grant_sha256: &str,
        budget: GoalControlBudget,
    ) -> Result<Self, String> {
        let root = root.into();
        if root.exists() {
            let mut entries = fs::read_dir(&root)
                .map_err(|error| format!("cannot read {}: {error}", root.display()))?;
            if let Some(entry) = entries.next() {
                entry.map_err(|error| {
                    format!(
                        "cannot inspect recovery entry in {}: {error}",
                        root.display()
                    )
                })?;
                return Err("a new goal recovery directory must be empty".into());
            }
        } else {
            fs::create_dir_all(&root)
                .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
        }

        let mut identity = json!({
            "schema": IDENTITY_SCHEMA,
            "goal_id": goal_id,
            "grant_sha256": grant_sha256,
            "budget": budget_json(budget),
        });
        seal(&mut identity, "identity_sha256")?;
        write_new_json(&identity_path(&root), &identity, 64_000)?;

        Ok(Self {
            root,
            goal_id: goal_id.to_owned(),
            grant_sha256: grant_sha256.to_owned(),
            budget,
            generation: 0,
            checkpoint: None,
            latest_report_generation: None,
            latest_report_digest: None,
            latest_report: None,
            head_digest: None,
            pending_written: false,
            total_private_report_bytes: 0,
        })
    }

    pub fn restore(
        root: impl Into<PathBuf>,
        goal_id: &str,
        grant_sha256: &str,
    ) -> Result<RestoredGoalControlState, String> {
        let root = root.into();
        let identity = read_json(&identity_path(&root), 64_000)?;
        exact_keys(&identity, IDENTITY_KEYS, "goal recovery identity")?;
        if identity["schema"] != IDENTITY_SCHEMA {
            return Err("goal recovery identity schema is unsupported".into());
        }
        validate_seal(&identity, "identity_sha256")?;
        if identity["goal_id"].as_str() != Some(goal_id)
            || identity["grant_sha256"].as_str() != Some(grant_sha256)
        {
            return Err("goal recovery identity does not match this goal and grant".into());
        }
        let budget = parse_budget(&identity["budget"])?;

        let generations = scan_generations(&root)?;
        if terminal_path(&root).exists() {
            let terminal = read_json(&terminal_path(&root), MAX_TERMINAL_BYTES)?;
            validate_terminal(&terminal, goal_id, grant_sha256)?;
            let expected_head = if generations == 0 {
                Value::Null
            } else {
                read_json(&head_path(&root, generations), 64_000)?["head_digest"].clone()
            };
            if terminal["previous_head_digest"] != expected_head
                || terminal["goal_control_report"]["budget"] != budget_json(budget)
            {
                return Err("terminal goal record is not chained to this recovery state".into());
            }
            return Err("goal recovery is terminal and cannot resume".into());
        }
        if generations == 0 {
            if pending_path(&root).exists() {
                let pending = read_json(&pending_path(&root), 64_000)?;
                validate_pending(&pending, 1)?;
                return Err(
                    "goal recovery has a dispatch intent without a committed safe stop; outcome is unknown and automatic replay is refused"
                        .into(),
                );
            }
            // The identity is flushed before any controller or dispatcher call. With no pending
            // intent and no generation artifacts, no mission could have crossed the side-effect
            // boundary; reusing the stored grant/budget is safe and avoids wedging a run that
            // stopped before its first dispatch.
            return Ok(RestoredGoalControlState {
                store: Self {
                    root,
                    goal_id: goal_id.to_owned(),
                    grant_sha256: grant_sha256.to_owned(),
                    budget,
                    generation: 0,
                    checkpoint: None,
                    latest_report_generation: None,
                    latest_report_digest: None,
                    latest_report: None,
                    head_digest: None,
                    pending_written: false,
                    total_private_report_bytes: 0,
                },
                checkpoint: None,
            });
        }

        let mut previous_head_digest: Option<String> = None;
        let mut previous_snapshot_digest: Option<String> = None;
        let mut previous_report_generation: Option<usize> = None;
        let mut previous_cycle_count = 0usize;
        let mut latest_checkpoint = None;
        let mut latest_report = None;
        let mut latest_report_digest = None;

        for generation in 1..=generations {
            let checkpoint = read_json(&checkpoint_path(&root, generation), MAX_CHECKPOINT_BYTES)?;
            let checkpoint = validate_goal_control_checkpoint(&checkpoint)
                .map_err(|error| format!("goal checkpoint {generation} is invalid: {error}"))?;
            let report = &checkpoint["goal_control_report"];
            verify_goal_control_report(report)
                .map_err(|error| format!("goal report {generation} is invalid: {error}"))?;
            let report_cycles = report["cycles"]
                .as_array()
                .ok_or_else(|| format!("goal report {generation} has no cycle list"))?;
            let head = read_json(&head_path(&root, generation), 64_000)?;
            validate_head(&head, generation)?;

            let checkpoint_digest = digest(&checkpoint)?;
            if checkpoint["generation"].as_u64() != Some(generation as u64)
                || checkpoint["previous_snapshot_digest"]
                    != previous_snapshot_digest
                        .as_deref()
                        .map(|value| Value::String(value.to_owned()))
                        .unwrap_or(Value::Null)
                || report["goal_id"].as_str() != Some(goal_id)
                || report["grant_sha256"].as_str() != Some(grant_sha256)
                || report["budget"] != budget_json(budget)
                || head["checkpoint_digest"].as_str() != Some(checkpoint_digest.as_str())
                || head["previous_head_digest"]
                    != previous_head_digest
                        .as_deref()
                        .map(|value| Value::String(value.to_owned()))
                        .unwrap_or(Value::Null)
            {
                return Err(format!(
                    "goal checkpoint {generation} is not contiguous with its identity and recovery chain"
                ));
            }

            let report_ref = parse_report_reference(&head, generation)?;
            let cycle_count = report_cycles.len();
            if cycle_count < previous_cycle_count || cycle_count > previous_cycle_count + 1 {
                return Err(format!(
                    "goal checkpoint {generation} changes the cycle count unexpectedly"
                ));
            }
            if cycle_count == 0 && report_ref.is_some() {
                return Err("goal recovery contains a private report without a cycle".into());
            }
            if cycle_count > 0 && report_ref.is_none() {
                return Err("goal recovery is missing the latest private mission report".into());
            }
            let referenced_generation = report_ref.as_ref().map(|(index, _)| *index);
            if (cycle_count == previous_cycle_count
                && referenced_generation != previous_report_generation)
                || (cycle_count > previous_cycle_count && referenced_generation != Some(generation))
            {
                return Err(format!(
                    "goal checkpoint {generation} has an inconsistent private report reference"
                ));
            }

            let mut resolved_report = None;
            let mut resolved_report_digest = None;
            if let Some((report_generation, report_digest)) = report_ref.as_ref() {
                let private = read_json(
                    &private_report_path(&root, *report_generation),
                    MAX_PRIVATE_REPORT_BYTES,
                )?;
                validate_private_report(&private, *report_generation)?;
                if digest(&private)? != *report_digest {
                    return Err(format!(
                        "private Autopilot report {report_generation} does not match its head"
                    ));
                }
                let raw_report = private["autopilot_report"].clone();
                verify_autopilot_report(&raw_report).map_err(|error| {
                    format!("private Autopilot report {report_generation} is invalid: {error}")
                })?;
                let expected_cycle_digest = report_cycles
                    .last()
                    .and_then(|cycle| cycle["autopilot_report_sha256"].as_str())
                    .ok_or_else(|| "last goal cycle has no Autopilot report digest".to_string())?;
                if digest(&raw_report)? != expected_cycle_digest {
                    return Err(
                        "private Autopilot report does not match the last committed goal cycle"
                            .into(),
                    );
                }
                resolved_report = Some(raw_report);
                resolved_report_digest = Some(report_digest.clone());
            }

            previous_head_digest = head["head_digest"].as_str().map(str::to_owned);
            previous_snapshot_digest = checkpoint["snapshot_digest"].as_str().map(str::to_owned);
            previous_report_generation = referenced_generation;
            previous_cycle_count = cycle_count;
            latest_report = resolved_report;
            latest_report_digest = resolved_report_digest;
            latest_checkpoint = Some(checkpoint);
        }

        let checkpoint = latest_checkpoint.expect("generation count was nonzero");
        let total_private_report_bytes = private_report_bytes(&root)?;
        if total_private_report_bytes > MAX_TOTAL_PRIVATE_REPORT_BYTES {
            return Err("private goal recovery reports exceed the 80 MB total bound".into());
        }
        let mut store = Self {
            root,
            goal_id: goal_id.to_owned(),
            grant_sha256: grant_sha256.to_owned(),
            budget,
            generation: generations,
            checkpoint: Some(checkpoint.clone()),
            latest_report_generation: previous_report_generation,
            latest_report_digest,
            latest_report,
            head_digest: previous_head_digest,
            pending_written: false,
            total_private_report_bytes,
        };
        store.reconcile_pending()?;
        Ok(RestoredGoalControlState {
            store,
            checkpoint: Some(checkpoint),
        })
    }

    pub fn budget(&self) -> GoalControlBudget {
        self.budget
    }

    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn last_report(&self) -> Option<&Value> {
        self.latest_report.as_ref()
    }

    pub fn checkpoint(&self) -> Option<&Value> {
        self.checkpoint.as_ref()
    }

    pub fn has_pending(&self) -> bool {
        self.pending_written || pending_path(&self.root).exists()
    }

    /// Multiple retries in one process share one unresolved generation marker. After a crash,
    /// that marker prevents resume until the caller handles the unknown outcome explicitly.
    pub fn mark_pending(&mut self, mission: &Value) -> Result<(), String> {
        self.verify_head()?;
        let path = pending_path(&self.root);
        if path.exists() {
            if !self.pending_written {
                return Err(
                    "a goal dispatch intent is already pending in this recovery directory".into(),
                );
            }
            let pending = read_json(&path, 64_000)?;
            validate_pending(&pending, self.generation + 1)?;
            if pending["previous_head_digest"]
                != self
                    .head_digest
                    .as_deref()
                    .map(|value| Value::String(value.to_owned()))
                    .unwrap_or(Value::Null)
            {
                return Err("pending goal dispatch belongs to a different checkpoint head".into());
            }
            return Ok(());
        }
        let mut pending = json!({
            "schema": PENDING_SCHEMA,
            "generation": self.generation + 1,
            "mission_digest": digest(mission)?,
            "previous_head_digest": self.head_digest,
        });
        seal(&mut pending, "pending_digest")?;
        write_new_json(&path, &pending, 64_000)?;
        self.pending_written = true;
        Ok(())
    }

    pub fn commit_safe_stop(
        &mut self,
        checkpoint: &Value,
        new_autopilot_report: Option<&Value>,
    ) -> Result<(), String> {
        self.verify_head()?;
        let generation = self.generation + 1;
        if generation > MAX_GENERATIONS {
            return Err("goal recovery generation exceeds its safety bound".into());
        }
        let checkpoint = validate_goal_control_checkpoint(checkpoint)
            .map_err(|error| format!("cannot commit invalid goal checkpoint: {error}"))?;
        let report = &checkpoint["goal_control_report"];
        verify_goal_control_report(report)
            .map_err(|error| format!("cannot commit invalid goal report: {error}"))?;
        if checkpoint["generation"].as_u64() != Some(generation as u64)
            || checkpoint["previous_snapshot_digest"]
                != self
                    .checkpoint
                    .as_ref()
                    .and_then(|value| value["snapshot_digest"].as_str())
                    .map(|value| Value::String(value.to_owned()))
                    .unwrap_or(Value::Null)
            || report["goal_id"].as_str() != Some(self.goal_id.as_str())
            || report["grant_sha256"].as_str() != Some(self.grant_sha256.as_str())
            || report["budget"] != budget_json(self.budget)
        {
            return Err("goal checkpoint does not continue this recovery identity".into());
        }

        let pending = if pending_path(&self.root).exists() {
            if !self.pending_written {
                return Err("goal dispatch intent is owned by another process".into());
            }
            let pending = read_json(&pending_path(&self.root), 64_000)?;
            validate_pending(&pending, generation)?;
            if pending["previous_head_digest"]
                != self
                    .head_digest
                    .as_deref()
                    .map(|value| Value::String(value.to_owned()))
                    .unwrap_or(Value::Null)
            {
                return Err("goal dispatch intent is not chained to the current head".into());
            }
            Some(pending)
        } else {
            None
        };

        let cycle_count = report["cycles"]
            .as_array()
            .map(Vec::len)
            .ok_or_else(|| "goal report has no cycle list".to_string())?;
        let mut private_report_generation = self.latest_report_generation;
        let mut private_report_digest = self.latest_report_digest.clone();
        let mut latest_report = self.latest_report.clone();
        if (cycle_count == 0 && (latest_report.is_some() || new_autopilot_report.is_some()))
            || (cycle_count > 0 && latest_report.is_none() && new_autopilot_report.is_none())
        {
            return Err("safe goal checkpoint has an incomplete private report binding".into());
        }

        if let Some(new_report) = new_autopilot_report {
            verify_autopilot_report(new_report)
                .map_err(|error| format!("new Autopilot report is invalid: {error}"))?;
            let expected = report["cycles"]
                .as_array()
                .and_then(|cycles| cycles.last())
                .and_then(|cycle| cycle["autopilot_report_sha256"].as_str())
                .ok_or_else(|| "goal report has no last Autopilot cycle digest".to_string())?;
            if digest(new_report)? != expected {
                return Err("new Autopilot report does not match the last goal cycle".into());
            }
            let mut private = json!({
                "schema": PRIVATE_REPORT_SCHEMA,
                "generation": generation,
                "autopilot_report": new_report,
            });
            seal(&mut private, "record_digest")?;
            let private_digest = digest(&private)?;
            let added_bytes = serde_json::to_vec_pretty(&private)
                .map_err(|error| format!("cannot size private goal report: {error}"))?
                .len()
                .saturating_add(1);
            let total_private_report_bytes = self
                .total_private_report_bytes
                .checked_add(added_bytes)
                .filter(|bytes| *bytes <= MAX_TOTAL_PRIVATE_REPORT_BYTES)
                .ok_or_else(|| "private goal reports exceed the 80 MB total bound".to_string())?;
            write_new_json(
                &private_report_path(&self.root, generation),
                &private,
                MAX_PRIVATE_REPORT_BYTES,
            )?;
            self.total_private_report_bytes = total_private_report_bytes;
            private_report_generation = Some(generation);
            private_report_digest = Some(private_digest);
            latest_report = Some(new_report.clone());
        }
        if (cycle_count > 0) != latest_report.is_some() {
            return Err("safe goal checkpoint has an incomplete private report binding".into());
        }

        write_new_json(
            &checkpoint_path(&self.root, generation),
            &checkpoint,
            MAX_CHECKPOINT_BYTES,
        )?;
        let mut head = json!({
            "schema": HEAD_SCHEMA,
            "generation": generation,
            "checkpoint_digest": digest(&checkpoint)?,
            "previous_head_digest": self.head_digest,
            "private_report_generation": private_report_generation,
            "private_report_digest": private_report_digest,
            "committed_pending_digest": pending
                .as_ref()
                .and_then(|value| value["pending_digest"].as_str()),
        });
        seal(&mut head, "head_digest")?;
        write_new_json(&head_path(&self.root, generation), &head, 64_000)?;

        self.generation = generation;
        self.head_digest = head["head_digest"].as_str().map(str::to_owned);
        self.latest_report_generation = private_report_generation;
        self.latest_report_digest = private_report_digest;
        self.latest_report = latest_report;
        self.checkpoint = Some(checkpoint);
        if pending.is_some() {
            fs::remove_file(pending_path(&self.root))
                .map_err(|error| format!("cannot clear committed goal dispatch marker: {error}"))?;
            self.pending_written = false;
        }
        Ok(())
    }

    pub fn commit_terminal(&mut self, goal_report: &Value) -> Result<(), String> {
        self.verify_head()?;
        if pending_path(&self.root).exists() {
            return Err(
                "goal has an unresolved dispatch intent and cannot be marked terminal".into(),
            );
        }
        verify_goal_control_report(goal_report)
            .map_err(|error| format!("cannot commit invalid terminal goal report: {error}"))?;
        if goal_report["goal_id"].as_str() != Some(self.goal_id.as_str())
            || goal_report["grant_sha256"].as_str() != Some(self.grant_sha256.as_str())
            || goal_report["budget"] != budget_json(self.budget)
        {
            return Err("terminal goal report does not match this recovery identity".into());
        }
        let mut terminal = json!({
            "schema": TERMINAL_SCHEMA,
            "goal_id": self.goal_id,
            "grant_sha256": self.grant_sha256,
            "previous_head_digest": self.head_digest,
            "goal_control_report": goal_report,
        });
        seal(&mut terminal, "terminal_digest")?;
        write_new_json(&terminal_path(&self.root), &terminal, MAX_TERMINAL_BYTES)
    }

    fn verify_head(&self) -> Result<(), String> {
        if scan_generations(&self.root)? != self.generation {
            return Err("goal recovery generation changed concurrently".into());
        }
        let observed = if self.generation == 0 {
            None
        } else {
            let head = read_json(&head_path(&self.root, self.generation), 64_000)?;
            validate_head(&head, self.generation)?;
            head["head_digest"].as_str().map(str::to_owned)
        };
        if observed != self.head_digest {
            return Err("goal recovery head changed concurrently".into());
        }
        Ok(())
    }

    fn reconcile_pending(&mut self) -> Result<(), String> {
        let path = pending_path(&self.root);
        if !path.exists() {
            return Ok(());
        }
        let pending = read_json(&path, 64_000)?;
        let generation = pending["generation"]
            .as_u64()
            .ok_or_else(|| "pending goal generation is missing".to_string())?
            as usize;
        validate_pending(&pending, generation)?;
        if generation != self.generation {
            return Err(
                "goal recovery has an uncommitted dispatch intent; outcome is unknown and automatic replay is refused"
                    .into(),
            );
        }
        let head = read_json(&head_path(&self.root, generation), 64_000)?;
        if head["committed_pending_digest"] != pending["pending_digest"]
            || head["previous_head_digest"] != pending["previous_head_digest"]
        {
            return Err("goal dispatch intent does not match its committed checkpoint".into());
        }
        fs::remove_file(&path)
            .map_err(|error| format!("cannot clear committed goal dispatch marker: {error}"))
    }
}

fn budget_json(budget: GoalControlBudget) -> Value {
    json!({
        "max_cycles": budget.max_cycles(),
        "max_total_dispatches": budget.max_total_dispatches(),
    })
}

fn parse_budget(value: &Value) -> Result<GoalControlBudget, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "goal recovery budget must be an object".to_string())?;
    if object.len() != 2
        || !object.contains_key("max_cycles")
        || !object.contains_key("max_total_dispatches")
    {
        return Err("goal recovery budget has missing or unsupported fields".into());
    }
    let max_cycles = object["max_cycles"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "max_cycles must be a positive integer".to_string())?;
    let max_total_dispatches = object["max_total_dispatches"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "max_total_dispatches must be a positive integer".to_string())?;
    GoalControlBudget::new(max_cycles, max_total_dispatches)
        .map_err(|error| format!("invalid goal recovery budget: {error}"))
}

fn identity_path(root: &Path) -> PathBuf {
    root.join("identity.json")
}

fn checkpoint_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("checkpoint-{generation:06}.json"))
}

fn head_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("head-{generation:06}.json"))
}

fn private_report_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("cycle-report-{generation:06}.private.json"))
}

fn pending_path(root: &Path) -> PathBuf {
    root.join("pending.json")
}

fn terminal_path(root: &Path) -> PathBuf {
    root.join("terminal.json")
}

fn private_report_bytes(root: &Path) -> Result<usize, String> {
    let mut total = 0usize;
    for entry in fs::read_dir(root)
        .map_err(|error| format!("cannot inspect goal recovery directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("cannot inspect goal recovery entry: {error}"))?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("cycle-report-")
        {
            let metadata = entry
                .metadata()
                .map_err(|error| format!("cannot inspect private goal report: {error}"))?;
            total = total
                .checked_add(usize::try_from(metadata.len()).unwrap_or(usize::MAX))
                .ok_or_else(|| "private goal report size overflowed".to_string())?;
        }
    }
    Ok(total)
}

fn scan_generations(root: &Path) -> Result<usize, String> {
    let mut checkpoints = Vec::new();
    let mut heads = Vec::new();
    let mut reports = Vec::new();
    for entry in fs::read_dir(root)
        .map_err(|error| format!("cannot inspect goal recovery directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("cannot inspect goal recovery entry: {error}"))?;
        if !entry
            .file_type()
            .map_err(|error| format!("cannot inspect goal recovery entry: {error}"))?
            .is_file()
        {
            return Err("goal recovery entries must be regular files".into());
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            "identity.json" | "pending.json" | "terminal.json"
        ) {
            continue;
        }
        let (prefix, suffix, target) = if name.starts_with("checkpoint-") {
            ("checkpoint-", ".json", &mut checkpoints)
        } else if name.starts_with("head-") {
            ("head-", ".json", &mut heads)
        } else if name.starts_with("cycle-report-") {
            ("cycle-report-", ".private.json", &mut reports)
        } else {
            return Err(format!("unexpected goal recovery entry {name:?}"));
        };
        let digits = name
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(suffix))
            .ok_or_else(|| format!("goal recovery entry {name:?} has an invalid name"))?;
        let generation = digits
            .parse::<usize>()
            .ok()
            .filter(|value| (1..=MAX_GENERATIONS).contains(value))
            .filter(|value| digits == format!("{value:06}"))
            .ok_or_else(|| format!("goal recovery entry {name:?} has an invalid generation"))?;
        target.push(generation);
    }
    checkpoints.sort_unstable();
    heads.sort_unstable();
    reports.sort_unstable();
    if heads.is_empty() {
        if !checkpoints.is_empty() || !reports.is_empty() {
            return Err("goal recovery artifacts exist without a committed head".into());
        }
        return Ok(0);
    }
    let maximum = *heads.last().expect("head list is non-empty");
    let expected = (1..=maximum).collect::<Vec<_>>();
    if heads != expected || checkpoints != expected || reports.iter().any(|value| *value > maximum)
    {
        return Err("goal recovery generations are missing, extra, or non-contiguous".into());
    }
    let mut report_origins = Vec::new();
    for generation in 1..=maximum {
        let head = read_json(&head_path(root, generation), 64_000)?;
        if head["private_report_generation"].as_u64() == Some(generation as u64) {
            report_origins.push(generation);
        }
    }
    if report_origins != reports {
        return Err("goal recovery contains an orphan or unreferenced private report".into());
    }
    Ok(maximum)
}

fn parse_report_reference(
    head: &Value,
    generation: usize,
) -> Result<Option<(usize, String)>, String> {
    let report_generation = match &head["private_report_generation"] {
        Value::Null => None,
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| (1..=generation).contains(value)),
        _ => None,
    };
    let report_digest = match &head["private_report_digest"] {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        _ => None,
    };
    match (report_generation, report_digest) {
        (None, None) => Ok(None),
        (Some(generation), Some(digest)) => {
            validate_digest(&digest, "private_report_digest")?;
            Ok(Some((generation, digest)))
        }
        _ => Err("goal recovery head has an incomplete private-report reference".into()),
    }
}

fn validate_head(head: &Value, generation: usize) -> Result<(), String> {
    exact_keys(head, HEAD_KEYS, "goal recovery head")?;
    if head["schema"] != HEAD_SCHEMA
        || head["generation"].as_u64() != Some(generation as u64)
        || !(1..=MAX_GENERATIONS).contains(&generation)
    {
        return Err("goal recovery head identity is invalid".into());
    }
    validate_digest(
        head["checkpoint_digest"]
            .as_str()
            .ok_or_else(|| "goal recovery checkpoint digest is missing".to_string())?,
        "checkpoint_digest",
    )?;
    for field in ["previous_head_digest", "committed_pending_digest"] {
        match &head[field] {
            Value::Null => {}
            Value::String(value) => validate_digest(value, field)?,
            _ => return Err(format!("{field} must be null or a digest")),
        }
    }
    validate_seal(head, "head_digest")
}

fn validate_private_report(value: &Value, generation: usize) -> Result<(), String> {
    exact_keys(value, PRIVATE_REPORT_KEYS, "private Autopilot report")?;
    if value["schema"] != PRIVATE_REPORT_SCHEMA
        || value["generation"].as_u64() != Some(generation as u64)
    {
        return Err("private Autopilot report identity is invalid".into());
    }
    validate_seal(value, "record_digest")
}

fn validate_pending(value: &Value, generation: usize) -> Result<(), String> {
    exact_keys(value, PENDING_KEYS, "pending goal dispatch")?;
    if value["schema"] != PENDING_SCHEMA
        || value["generation"].as_u64() != Some(generation as u64)
        || !(1..=MAX_GENERATIONS).contains(&generation)
    {
        return Err("pending goal dispatch identity is invalid".into());
    }
    validate_digest(
        value["mission_digest"]
            .as_str()
            .ok_or_else(|| "pending mission digest is missing".to_string())?,
        "mission_digest",
    )?;
    match &value["previous_head_digest"] {
        Value::Null => {}
        Value::String(value) => validate_digest(value, "previous_head_digest")?,
        _ => return Err("pending predecessor must be null or a digest".into()),
    }
    validate_seal(value, "pending_digest")
}

fn validate_terminal(value: &Value, goal_id: &str, grant_sha256: &str) -> Result<(), String> {
    exact_keys(value, TERMINAL_KEYS, "terminal goal record")?;
    if value["schema"] != TERMINAL_SCHEMA
        || value["goal_id"].as_str() != Some(goal_id)
        || value["grant_sha256"].as_str() != Some(grant_sha256)
    {
        return Err("terminal goal record identity is invalid".into());
    }
    match &value["previous_head_digest"] {
        Value::Null => {}
        Value::String(value) => validate_digest(value, "previous_head_digest")?,
        _ => return Err("terminal predecessor must be null or a digest".into()),
    }
    verify_goal_control_report(&value["goal_control_report"])
        .map_err(|error| format!("terminal goal report is invalid: {error}"))?;
    if value["goal_control_report"]["goal_id"].as_str() != Some(goal_id)
        || value["goal_control_report"]["grant_sha256"].as_str() != Some(grant_sha256)
    {
        return Err("terminal goal report does not match its recovery identity".into());
    }
    validate_seal(value, "terminal_digest")
}

fn exact_keys(value: &Value, expected: &[&str], name: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{name} must be an object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("{name} has missing or unsupported fields"));
    }
    Ok(())
}

fn validate_digest(value: &str, field: &str) -> Result<(), String> {
    ContentHash::parse(value.to_owned())
        .map(|_| ())
        .map_err(|_| format!("{field} is malformed"))
}

fn digest(value: &Value) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| format!("cannot digest goal recovery data: {error}"))
}

fn seal(value: &mut Value, field: &str) -> Result<(), String> {
    let hash = digest(value)?;
    value
        .as_object_mut()
        .ok_or_else(|| "sealed recovery value must be an object".to_string())?
        .insert(field.into(), Value::String(hash));
    Ok(())
}

fn validate_seal(value: &Value, field: &str) -> Result<(), String> {
    let claimed = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{field} must be a digest"))?;
    validate_digest(claimed, field)?;
    let mut unsigned = value
        .as_object()
        .cloned()
        .ok_or_else(|| "sealed recovery value must be an object".to_string())?;
    unsigned.remove(field);
    if digest(&Value::Object(unsigned))? != claimed {
        return Err(format!("{field} does not match its recovery record"));
    }
    Ok(())
}

fn read_json(path: &Path, maximum: usize) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(format!(
            "{} is not a regular file within its byte bound",
            path.display()
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} is invalid JSON: {error}", path.display()))
}

fn write_new_json(path: &Path, value: &Value, maximum: usize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize goal recovery artifact: {error}"))?;
    bytes.push(b'\n');
    if bytes.len() > maximum {
        return Err(format!("{} exceeds its byte bound", path.display()));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot persist {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_autopilot::{
        drive_goal, seal_goal_control_checkpoint, AutonomyGrant, GoalDecision, GoalStopReason,
    };
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock follows epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "autopilot-goal-recovery-{}-{stamp}",
            std::process::id()
        ))
    }

    fn sample() -> (AutonomyGrant, GoalControlBudget) {
        let grant = serde_json::from_value(json!({
            "allowed_tools": ["observe"],
            "max_attempts": 1,
            "require_reconciliation_complete": false
        }))
        .expect("valid grant");
        (grant, GoalControlBudget::new(3, 3).expect("valid budget"))
    }

    fn stopped_report(grant: &AutonomyGrant, budget: GoalControlBudget) -> Value {
        let mut controller = |_: &bioprism_autopilot::GoalControlContext<'_>| {
            Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork))
        };
        let mut dispatcher =
            |_: &Value| -> Result<Value, String> { Err("stop decision must not dispatch".into()) };
        drive_goal("goal", grant, budget, &mut controller, &mut dispatcher)
            .expect("a typed safe stop produces a report")
            .report
    }

    #[test]
    fn safe_stop_checkpoints_restore_without_private_reports_when_no_mission_ran() {
        let root = scratch();
        let (grant, budget) = sample();
        let grant_sha256 = grant.digest().unwrap().to_string();
        let mut store =
            GoalControlRecoveryStore::create(&root, "goal", &grant_sha256, budget).unwrap();
        let report = stopped_report(&grant, budget);
        let checkpoint = seal_goal_control_checkpoint(&report, 1, None).unwrap();
        store.commit_safe_stop(&checkpoint, None).unwrap();

        let restored = GoalControlRecoveryStore::restore(&root, "goal", &grant_sha256).unwrap();
        assert_eq!(restored.checkpoint, Some(checkpoint));
        assert_eq!(restored.store.generation(), 1);
        assert!(restored.store.last_report().is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_identity_only_recovery_directory_can_be_retried_before_any_dispatch() {
        let root = scratch();
        let (grant, budget) = sample();
        let grant_sha256 = grant.digest().unwrap().to_string();
        GoalControlRecoveryStore::create(&root, "goal", &grant_sha256, budget).unwrap();

        let restored = GoalControlRecoveryStore::restore(&root, "goal", &grant_sha256)
            .expect("no pending intent means no mission was dispatched");
        assert_eq!(restored.checkpoint, None);
        assert_eq!(restored.store.generation(), 0);
        assert!(restored.store.last_report().is_none());

        let report = stopped_report(&grant, budget);
        let checkpoint = seal_goal_control_checkpoint(&report, 1, None).unwrap();
        let mut store = restored.store;
        store
            .commit_safe_stop(&checkpoint, None)
            .expect("the retried run can create its first safe checkpoint");
        let resumed = GoalControlRecoveryStore::restore(&root, "goal", &grant_sha256)
            .expect("the first committed checkpoint restores");
        assert_eq!(resumed.checkpoint, Some(checkpoint));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unmatched_goal_dispatch_intents_are_refused_before_any_resume() {
        let root = scratch();
        let (grant, budget) = sample();
        let grant_sha256 = grant.digest().unwrap().to_string();
        let mut store =
            GoalControlRecoveryStore::create(&root, "goal", &grant_sha256, budget).unwrap();
        store
            .mark_pending(&json!({ "mission_id": "possibly-dispatched" }))
            .unwrap();
        let error = match GoalControlRecoveryStore::restore(&root, "goal", &grant_sha256) {
            Ok(_) => panic!("unknown dispatch outcome must not resume"),
            Err(error) => error,
        };
        assert!(error.contains("outcome is unknown and automatic replay is refused"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_terminal_goal_record_prevents_resuming_an_older_safe_checkpoint() {
        let root = scratch();
        let (grant, budget) = sample();
        let grant_sha256 = grant.digest().unwrap().to_string();
        let mut store =
            GoalControlRecoveryStore::create(&root, "goal", &grant_sha256, budget).unwrap();
        let report = stopped_report(&grant, budget);
        let checkpoint = seal_goal_control_checkpoint(&report, 1, None).unwrap();
        store.commit_safe_stop(&checkpoint, None).unwrap();
        store.commit_terminal(&report).unwrap();
        let error = GoalControlRecoveryStore::restore(&root, "goal", &grant_sha256)
            .expect_err("a terminal goal cannot be resumed");
        assert!(error.contains("terminal and cannot resume"));
        fs::remove_dir_all(root).unwrap();
    }
}
