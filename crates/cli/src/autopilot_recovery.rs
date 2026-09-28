//! Caller-visible, restart-safe persistence for the CLI autopilot.
//!
//! Checkpoints contain only bounded metadata. Each rehydration record is stored in a separate
//! explicitly private file because it contains dispatched mission arguments and tool reports.
//! The append-only generation files and pending-dispatch marker make an interrupted write or
//! an interrupted external dispatch fail closed instead of silently replaying work.

use bioprism_autopilot::{
    restore_drive_history, seal_autopilot_checkpoint, validate_autopilot_checkpoint, AttemptKind,
    AttemptRecord, AutonomyGrant, AutopilotError, DriveHistory,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const HEAD_SCHEMA: &str = "bioprism-autopilot-cli-recovery-head/0.1";
const PRIVATE_ATTEMPT_SCHEMA: &str = "bioprism-autopilot-cli-private-attempt/0.1";
const PENDING_SCHEMA: &str = "bioprism-autopilot-cli-pending-dispatch/0.1";
const MAX_GENERATIONS: usize = 16;
const MAX_PRIVATE_ATTEMPT_BYTES: usize = 20_000_000;
const MAX_TOTAL_PRIVATE_BYTES: usize = 80_000_000;
const HEAD_KEYS: &[&str] = &[
    "schema",
    "generation",
    "checkpoint_digest",
    "private_attempt_digest",
    "previous_head_digest",
    "head_digest",
];
const PRIVATE_ATTEMPT_KEYS: &[&str] = &[
    "schema",
    "generation",
    "base_mission_digest",
    "attempt",
    "record_digest",
];
const DELIVERED_ATTEMPT_KEYS: &[&str] = &[
    "kind",
    "mission",
    "delivery",
    "report",
    "reconciliation",
    "reconciliation_note",
];
const UNDELIVERED_ATTEMPT_KEYS: &[&str] = &["kind", "mission", "delivery", "dispatch_error"];
const PENDING_KEYS: &[&str] = &[
    "schema",
    "generation",
    "mission_digest",
    "previous_head_digest",
    "pending_digest",
];

fn invalid(reason: impl Into<String>) -> AutopilotError {
    AutopilotError::InvalidCheckpoint {
        reason: reason.into(),
    }
}

fn persistence(reason: impl Into<String>) -> AutopilotError {
    AutopilotError::Persistence {
        reason: reason.into(),
    }
}

fn digest(value: &Value) -> Result<String, AutopilotError> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })
}

fn remove_digest(value: &Value, field: &str) -> Result<Value, AutopilotError> {
    let mut value = value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("recovery record must be a JSON object"))?;
    value.remove(field);
    Ok(Value::Object(value))
}

fn seal(value: &mut Value, field: &str) -> Result<(), AutopilotError> {
    let hash = digest(value)?;
    value
        .as_object_mut()
        .ok_or_else(|| invalid("recovery record must be a JSON object"))?
        .insert(field.into(), Value::String(hash));
    Ok(())
}

fn validate_seal(value: &Value, field: &str) -> Result<(), AutopilotError> {
    let claimed = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a digest string")))?;
    ContentHash::parse(claimed.to_owned()).map_err(|_| invalid(format!("{field} is malformed")))?;
    let expected = digest(&remove_digest(value, field)?)?;
    if claimed != expected {
        return Err(invalid(format!("{field} does not match its record")));
    }
    Ok(())
}

fn exact_keys(value: &Value, expected: &[&str], name: &str) -> Result<(), AutopilotError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{name} must be an object")))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(format!("{name} has unsupported or missing fields")));
    }
    Ok(())
}

fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>, AutopilotError> {
    let file = fs::File::open(path)
        .map_err(|error| persistence(format!("cannot open {}: {error}", path.display())))?;
    let metadata = file
        .metadata()
        .map_err(|error| persistence(format!("cannot inspect {}: {error}", path.display())))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(invalid(format!(
            "{} is not a regular file within its byte bound",
            path.display()
        )));
    }
    read_limited(file, maximum)
}

fn read_limited(reader: impl Read, maximum: usize) -> Result<Vec<u8>, AutopilotError> {
    let limit = u64::try_from(maximum)
        .ok()
        .and_then(|maximum| maximum.checked_add(1))
        .ok_or_else(|| invalid("configured recovery byte bound is too large"))?;
    let mut reader = reader.take(limit);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| persistence(format!("cannot read recovery artifact: {error}")))?;
    if bytes.len() > maximum {
        return Err(invalid(
            "recovery artifact grew beyond its configured byte bound while reading",
        ));
    }
    Ok(bytes)
}

/// Create a new recovery directory, refusing to rebind a non-empty directory.
pub struct AutopilotRecoveryStore {
    root: PathBuf,
    generation: usize,
    snapshot_digest: Option<String>,
    head_digest: Option<String>,
}

pub struct RecoveredAutopilotState {
    pub store: AutopilotRecoveryStore,
    pub checkpoint: Value,
    pub attempts: Vec<AttemptRecord>,
}

impl AutopilotRecoveryStore {
    pub fn create(root: impl Into<PathBuf>) -> Result<Self, AutopilotError> {
        let root = root.into();
        if root.exists() {
            let mut entries = fs::read_dir(&root)
                .map_err(|error| persistence(format!("cannot read {}: {error}", root.display())))?;
            if entries.next().is_some() {
                return Err(invalid("a new autopilot recovery directory must be empty"));
            }
        } else {
            fs::create_dir_all(&root).map_err(|error| {
                persistence(format!("cannot create {}: {error}", root.display()))
            })?;
        }
        Ok(Self {
            root,
            generation: 0,
            snapshot_digest: None,
            head_digest: None,
        })
    }

    /// Load and validate every committed generation before returning rehydration material.
    pub fn restore(
        root: impl Into<PathBuf>,
        grant: &AutonomyGrant,
        base_mission: &Value,
    ) -> Result<RecoveredAutopilotState, AutopilotError> {
        let root = root.into();
        let generation = scan_generation(&root)?;
        if generation == 0 {
            let pending_path = root.join("pending.json");
            if pending_path.exists() {
                let pending = read_json(&pending_path, 64_000)?;
                let pending_generation = pending
                    .get("generation")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("pending dispatch generation is missing"))?
                    as usize;
                validate_pending(&pending, pending_generation)?;
                return Err(invalid(
                    "recovery contains a dispatch intent without a completed receipt; outcome is unknown and automatic replay is refused",
                ));
            }
            return Err(invalid(
                "recovery directory contains no committed checkpoint",
            ));
        }
        let base_mission_digest = digest(base_mission)?;
        let expected_grant_digest = grant.digest()?;
        let mut previous_snapshot_digest: Option<String> = None;
        let mut previous_head_digest: Option<String> = None;
        let mut all_attempts = Vec::with_capacity(generation);
        let mut total_private_bytes = 0usize;
        let mut latest_checkpoint = Value::Null;

        for index in 1..=generation {
            let checkpoint_path = checkpoint_path(&root, index);
            let private_path = private_attempt_path(&root, index);
            let head_path = head_path(&root, index);
            let checkpoint = read_json(&checkpoint_path, 2_000_000)?;
            let checkpoint = validate_autopilot_checkpoint(&checkpoint)?;
            let private_bytes = bounded_read(&private_path, MAX_PRIVATE_ATTEMPT_BYTES)?;
            total_private_bytes = total_private_bytes
                .checked_add(private_bytes.len())
                .ok_or_else(|| invalid("private recovery material size overflowed"))?;
            if total_private_bytes > MAX_TOTAL_PRIVATE_BYTES {
                return Err(invalid(
                    "private recovery history exceeds its total byte bound",
                ));
            }
            let private: Value = serde_json::from_slice(&private_bytes).map_err(|error| {
                invalid(format!("private attempt record is invalid JSON: {error}"))
            })?;
            let head = read_json(&head_path, 64_000)?;
            validate_head(&head, index)?;
            let checkpoint_digest = digest(&checkpoint)?;
            let private_digest = digest(&private)?;
            if head["checkpoint_digest"] != checkpoint_digest
                || head["private_attempt_digest"] != private_digest
            {
                return Err(invalid(format!(
                    "recovery head {index} does not bind its checkpoint and private attempt"
                )));
            }
            let checkpoint_generation = checkpoint["generation"].as_u64();
            let attempts_used = checkpoint["attempts_used"].as_u64();
            if checkpoint_generation != Some(index as u64)
                || attempts_used != Some(index as u64)
                || checkpoint["grant_digest"] != expected_grant_digest
                || checkpoint["base_mission_digest"] != base_mission_digest
                || checkpoint["previous_snapshot_digest"]
                    != previous_snapshot_digest
                        .as_deref()
                        .map(|value| Value::String(value.to_owned()))
                        .unwrap_or(Value::Null)
            {
                return Err(invalid(format!(
                    "checkpoint {index} is not contiguous with this grant and mission"
                )));
            }
            if head["previous_head_digest"]
                != previous_head_digest
                    .as_deref()
                    .map(|value| Value::String(value.to_owned()))
                    .unwrap_or(Value::Null)
            {
                return Err(invalid(format!("recovery head {index} is not chained")));
            }
            let attempt = decode_private_attempt(&private, index, &base_mission_digest)?;
            let expected_kind = if index == 1 {
                AttemptKind::Full
            } else {
                AttemptKind::Repair
            };
            if attempt.kind() != expected_kind {
                return Err(invalid(format!(
                    "attempt {index} has the wrong full-or-repair classification"
                )));
            }
            all_attempts.push(attempt);
            restore_drive_history(
                grant,
                &checkpoint,
                base_mission.clone(),
                all_attempts.clone(),
            )?;
            previous_snapshot_digest = checkpoint
                .get("snapshot_digest")
                .and_then(Value::as_str)
                .map(str::to_owned);
            previous_head_digest = head
                .get("head_digest")
                .and_then(Value::as_str)
                .map(str::to_owned);
            latest_checkpoint = checkpoint;
        }

        let mut store = Self {
            root,
            generation,
            snapshot_digest: previous_snapshot_digest,
            head_digest: previous_head_digest,
        };
        store.refuse_or_reconcile_pending(&all_attempts)?;
        Ok(RecoveredAutopilotState {
            store,
            checkpoint: latest_checkpoint,
            attempts: all_attempts,
        })
    }

    /// Mark a mission as in flight before any tool can run. An unmatched marker is never replayed.
    pub fn mark_pending(&mut self, mission: &Value) -> Result<(), AutopilotError> {
        let path = self.root.join("pending.json");
        if path.exists() {
            return Err(invalid("an earlier dispatch intent is still pending"));
        }
        self.verify_committed_head()?;
        let mut pending = json!({
            "schema": PENDING_SCHEMA,
            "generation": self.generation + 1,
            "mission_digest": digest(mission)?,
            "previous_head_digest": self.head_digest.clone(),
        });
        seal(&mut pending, "pending_digest")?;
        write_new_json(&path, &pending, 64_000)?;
        if let Err(error) = self.verify_committed_head() {
            let _ = fs::remove_file(&path);
            return Err(error);
        }
        Ok(())
    }

    /// Persist the completed attempt, sealed checkpoint, and commit head in that order.
    pub fn checkpoint(
        &mut self,
        grant: &AutonomyGrant,
        history: &DriveHistory,
    ) -> Result<(), AutopilotError> {
        let generation = history.dispatches_used();
        if generation != self.generation + 1 || generation > MAX_GENERATIONS {
            return Err(invalid(
                "checkpoint generation is not contiguous or is over budget",
            ));
        }
        self.verify_committed_head()?;
        let attempt = history
            .attempts()
            .last()
            .ok_or_else(|| invalid("cannot checkpoint a history without a dispatch"))?;
        let pending_path = self.root.join("pending.json");
        let pending = read_json(&pending_path, 64_000)?;
        validate_pending(&pending, generation)?;
        if pending["mission_digest"] != attempt.mission_digest()
            || pending["previous_head_digest"]
                != self
                    .head_digest
                    .as_deref()
                    .map(|value| Value::String(value.to_owned()))
                    .unwrap_or(Value::Null)
        {
            return Err(invalid(
                "completed attempt does not match its pending dispatch intent",
            ));
        }

        let checkpoint = seal_autopilot_checkpoint(
            grant,
            history,
            generation as u64,
            self.snapshot_digest.as_deref(),
        )?;
        let base_mission_digest = digest(history.base_mission())?;
        let mut private = private_attempt_document(attempt, generation, &base_mission_digest)?;
        seal(&mut private, "record_digest")?;
        let checkpoint_digest = digest(&checkpoint)?;
        let private_attempt_digest = digest(&private)?;
        let mut head = json!({
            "schema": HEAD_SCHEMA,
            "generation": generation,
            "checkpoint_digest": checkpoint_digest,
            "private_attempt_digest": private_attempt_digest,
            "previous_head_digest": self.head_digest.clone(),
        });
        seal(&mut head, "head_digest")?;

        write_new_json(
            &private_attempt_path(&self.root, generation),
            &private,
            MAX_PRIVATE_ATTEMPT_BYTES,
        )?;
        write_new_json(
            &checkpoint_path(&self.root, generation),
            &checkpoint,
            2_000_000,
        )?;
        write_new_json(&head_path(&self.root, generation), &head, 64_000)?;
        let head_digest = head["head_digest"]
            .as_str()
            .ok_or_else(|| invalid("sealed recovery head has no digest"))?
            .to_owned();
        self.generation = generation;
        self.snapshot_digest = checkpoint["snapshot_digest"].as_str().map(str::to_owned);
        self.head_digest = Some(head_digest);
        fs::remove_file(&pending_path).map_err(|error| {
            persistence(format!("cannot clear committed dispatch marker: {error}"))
        })?;
        Ok(())
    }

    pub fn generation(&self) -> usize {
        self.generation
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn verify_committed_head(&self) -> Result<(), AutopilotError> {
        let generation = scan_generation(&self.root)?;
        let observed_head = if generation == 0 {
            None
        } else {
            let head = read_json(&head_path(&self.root, generation), 64_000)?;
            validate_head(&head, generation)?;
            head.get("head_digest")
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        if generation != self.generation || observed_head != self.head_digest {
            return Err(AutopilotError::CompareAndSwapConflict);
        }
        Ok(())
    }

    fn refuse_or_reconcile_pending(
        &mut self,
        attempts: &[AttemptRecord],
    ) -> Result<(), AutopilotError> {
        let path = self.root.join("pending.json");
        if !path.exists() {
            return Ok(());
        }
        let pending = read_json(&path, 64_000)?;
        let generation = pending
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("pending dispatch generation is missing"))?
            as usize;
        validate_pending(&pending, generation)?;
        let committed = generation <= self.generation
            && attempts
                .get(generation.saturating_sub(1))
                .is_some_and(|attempt| pending["mission_digest"] == attempt.mission_digest());
        if !committed {
            return Err(invalid(
                "a dispatch intent has no matching completed receipt; outcome is unknown and automatic replay is refused",
            ));
        }
        if pending["previous_head_digest"]
            != if generation == 1 {
                Value::Null
            } else {
                read_json(&head_path(&self.root, generation - 1), 64_000)?["head_digest"].clone()
            }
        {
            return Err(invalid(
                "committed pending dispatch is not chained to its prior head",
            ));
        }
        fs::remove_file(path).map_err(|error| {
            persistence(format!("cannot clear committed dispatch marker: {error}"))
        })?;
        Ok(())
    }
}

fn checkpoint_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("checkpoint-{generation:06}.json"))
}

fn private_attempt_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("attempt-{generation:06}.private.json"))
}

fn head_path(root: &Path, generation: usize) -> PathBuf {
    root.join(format!("head-{generation:06}.json"))
}

fn scan_generation(root: &Path) -> Result<usize, AutopilotError> {
    let entries = fs::read_dir(root)
        .map_err(|error| persistence(format!("cannot read recovery directory: {error}")))?;
    let mut head_generations = Vec::new();
    let mut checkpoint_generations = Vec::new();
    let mut attempt_generations = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| persistence(format!("cannot inspect recovery entry: {error}")))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "pending.json" {
            if !entry
                .file_type()
                .map_err(|error| persistence(format!("cannot inspect pending.json: {error}")))?
                .is_file()
            {
                return Err(invalid("pending.json is not a regular file"));
            }
            continue;
        }
        if !entry
            .file_type()
            .map_err(|error| persistence(format!("cannot inspect {name:?}: {error}")))?
            .is_file()
        {
            return Err(invalid(format!("recovery entry {name:?} is not a file")));
        }
        let parsed = if let Some(value) = name
            .strip_prefix("head-")
            .and_then(|value| value.strip_suffix(".json"))
        {
            Some((value, &mut head_generations))
        } else if let Some(value) = name
            .strip_prefix("checkpoint-")
            .and_then(|value| value.strip_suffix(".json"))
        {
            Some((value, &mut checkpoint_generations))
        } else if let Some(value) = name
            .strip_prefix("attempt-")
            .and_then(|value| value.strip_suffix(".private.json"))
        {
            Some((value, &mut attempt_generations))
        } else {
            None
        }
        .ok_or_else(|| invalid(format!("unexpected recovery entry {name:?}")))?;
        let generation = parsed
            .0
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0 && *value <= MAX_GENERATIONS)
            .ok_or_else(|| invalid(format!("recovery entry {name:?} has an invalid generation")))?;
        parsed.1.push(generation);
    }
    head_generations.sort_unstable();
    checkpoint_generations.sort_unstable();
    attempt_generations.sort_unstable();
    if head_generations.is_empty() {
        if !checkpoint_generations.is_empty() || !attempt_generations.is_empty() {
            return Err(invalid("recovery files exist without a committed head"));
        }
        return Ok(0);
    }
    let maximum = *head_generations.last().expect("not empty");
    let expected = (1..=maximum).collect::<Vec<_>>();
    if head_generations != expected
        || checkpoint_generations != expected
        || attempt_generations != expected
    {
        return Err(invalid("recovery head generations are not contiguous"));
    }
    Ok(maximum)
}

fn read_json(path: &Path, maximum: usize) -> Result<Value, AutopilotError> {
    let bytes = bounded_read(path, maximum)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("{} is invalid JSON: {error}", path.display())))
}

fn write_new_json(path: &Path, value: &Value, maximum: usize) -> Result<(), AutopilotError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| persistence(format!("cannot serialize recovery artifact: {error}")))?;
    if bytes.len() > maximum {
        return Err(invalid(format!(
            "{} exceeds its byte bound",
            path.display()
        )));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| persistence(format!("cannot create {}: {error}", path.display())))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| persistence(format!("cannot persist {}: {error}", path.display())))
}

fn parse_kind(value: &Value) -> Result<AttemptKind, AutopilotError> {
    match value.as_str() {
        Some("full") => Ok(AttemptKind::Full),
        Some("repair") => Ok(AttemptKind::Repair),
        _ => Err(invalid("private attempt kind must be full or repair")),
    }
}

fn private_attempt_document(
    attempt: &AttemptRecord,
    generation: usize,
    base_mission_digest: &str,
) -> Result<Value, AutopilotError> {
    let row = if let Some(report) = attempt.report() {
        json!({
            "kind": attempt.kind().as_str(),
            "mission": attempt.mission(),
            "delivery": "delivered",
            "report": report,
            "reconciliation": attempt.reconciliation(),
            "reconciliation_note": attempt.reconciliation_note(),
        })
    } else {
        let dispatch_error = attempt
            .dispatch_error()
            .ok_or_else(|| invalid("undelivered attempt is missing its dispatch error"))?;
        json!({
            "kind": attempt.kind().as_str(),
            "mission": attempt.mission(),
            "delivery": "undelivered",
            "dispatch_error": dispatch_error,
        })
    };
    let document = json!({
        "schema": PRIVATE_ATTEMPT_SCHEMA,
        "generation": generation,
        "base_mission_digest": base_mission_digest,
        "attempt": row,
    });
    if document.as_object().map(Map::len) != Some(PRIVATE_ATTEMPT_KEYS.len() - 1) {
        return Err(invalid("private attempt projection is malformed"));
    }
    Ok(document)
}

fn decode_private_attempt(
    document: &Value,
    generation: usize,
    base_mission_digest: &str,
) -> Result<AttemptRecord, AutopilotError> {
    exact_keys(document, PRIVATE_ATTEMPT_KEYS, "private attempt record")?;
    if document["schema"] != PRIVATE_ATTEMPT_SCHEMA
        || document["generation"].as_u64() != Some(generation as u64)
        || document["base_mission_digest"] != base_mission_digest
    {
        return Err(invalid(
            "private attempt record identity does not match its generation",
        ));
    }
    validate_seal(document, "record_digest")?;
    let row = document
        .get("attempt")
        .ok_or_else(|| invalid("private attempt record has no attempt"))?;
    let kind = parse_kind(
        row.get("kind")
            .ok_or_else(|| invalid("private attempt kind is missing"))?,
    )?;
    let mission = row
        .get("mission")
        .cloned()
        .ok_or_else(|| invalid("private attempt mission is missing"))?;
    match row.get("delivery").and_then(Value::as_str) {
        Some("delivered") => {
            exact_keys(row, DELIVERED_ATTEMPT_KEYS, "delivered attempt")?;
            let note = match row.get("reconciliation_note") {
                Some(Value::Null) => None,
                Some(Value::String(note)) if note.len() <= 16_384 && !note.contains('\0') => {
                    Some(note.clone())
                }
                _ => return Err(invalid("reconciliation_note must be null or bounded text")),
            };
            let reconciliation = match row.get("reconciliation") {
                Some(Value::Null) => None,
                Some(value) => Some(value.clone()),
                None => return Err(invalid("delivered attempt reconciliation field is missing")),
            };
            AttemptRecord::delivered(kind, mission, row["report"].clone(), reconciliation, note)
        }
        Some("undelivered") => {
            exact_keys(row, UNDELIVERED_ATTEMPT_KEYS, "undelivered attempt")?;
            let error = row["dispatch_error"]
                .as_str()
                .filter(|error| error.len() <= 16_384 && !error.contains('\0'))
                .ok_or_else(|| invalid("dispatch_error must be bounded text without NUL bytes"))?;
            AttemptRecord::undelivered(kind, mission, error.to_owned())
        }
        _ => Err(invalid(
            "private attempt delivery must be delivered or undelivered",
        )),
    }
}

fn validate_head(head: &Value, generation: usize) -> Result<(), AutopilotError> {
    exact_keys(head, HEAD_KEYS, "recovery head")?;
    if head["schema"] != HEAD_SCHEMA || head["generation"].as_u64() != Some(generation as u64) {
        return Err(invalid(
            "recovery head identity does not match its generation",
        ));
    }
    for field in ["checkpoint_digest", "private_attempt_digest"] {
        let value = head[field]
            .as_str()
            .ok_or_else(|| invalid(format!("recovery head {field} is missing")))?;
        ContentHash::parse(value.to_owned())
            .map_err(|_| invalid(format!("recovery head {field} is malformed")))?;
    }
    match &head["previous_head_digest"] {
        Value::Null => {}
        Value::String(value) => {
            ContentHash::parse(value.clone())
                .map_err(|_| invalid("recovery head predecessor digest is malformed"))?;
        }
        _ => {
            return Err(invalid(
                "recovery head predecessor must be null or a digest",
            ))
        }
    }
    validate_seal(head, "head_digest")
}

fn validate_pending(pending: &Value, generation: usize) -> Result<(), AutopilotError> {
    exact_keys(pending, PENDING_KEYS, "pending dispatch marker")?;
    if pending["schema"] != PENDING_SCHEMA
        || pending["generation"].as_u64() != Some(generation as u64)
        || !(1..=MAX_GENERATIONS).contains(&generation)
    {
        return Err(invalid("pending dispatch identity is malformed"));
    }
    let mission_digest = pending["mission_digest"]
        .as_str()
        .ok_or_else(|| invalid("pending dispatch mission digest is missing"))?;
    ContentHash::parse(mission_digest.to_owned())
        .map_err(|_| invalid("pending dispatch mission digest is malformed"))?;
    match &pending["previous_head_digest"] {
        Value::Null => {}
        Value::String(value) => {
            ContentHash::parse(value.clone())
                .map_err(|_| invalid("pending dispatch predecessor digest is malformed"))?;
        }
        _ => {
            return Err(invalid(
                "pending dispatch predecessor must be null or a digest",
            ))
        }
    }
    validate_seal(pending, "pending_digest")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct UnboundedReader;

    impl Read for UnboundedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(b'x');
            Ok(buffer.len())
        }
    }

    #[test]
    fn recovery_reads_remain_bounded_if_the_file_grows_after_metadata_inspection() {
        let error = read_limited(UnboundedReader, 8).unwrap_err();
        assert!(matches!(
            error,
            AutopilotError::InvalidCheckpoint { ref reason }
                if reason.contains("grew beyond its configured byte bound")
        ));
    }

    #[test]
    fn private_recovery_preserves_an_empty_dispatch_error_without_fabricating_text() {
        let mission = json!({
            "mission_id": "empty-dispatch-error",
            "goal": "round trip an empty executor diagnostic",
            "steps": [{
                "id": "capability",
                "domain": "metrics",
                "capability": "analytics",
                "objective": "inspect the workspace capability list",
                "tool": "workspace_capabilities",
                "arguments": {},
                "depends_on": [],
                "bindings": [],
                "required": true,
            }],
        });
        let base_mission_digest = digest(&mission).expect("mission digest");
        let attempt = AttemptRecord::undelivered(AttemptKind::Full, mission, String::new())
            .expect("an empty executor diagnostic is retained as supplied");
        let mut document = private_attempt_document(&attempt, 1, &base_mission_digest)
            .expect("private projection");
        seal(&mut document, "record_digest").expect("seal private projection");

        let restored = decode_private_attempt(&document, 1, &base_mission_digest)
            .expect("empty dispatch error remains valid private evidence");
        assert_eq!(restored.dispatch_error(), Some(""));
    }

    #[test]
    fn an_unmatched_dispatch_intent_blocks_resume_before_any_replay() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bioprism-autopilot-recovery-{}-{stamp}",
            std::process::id()
        ));
        let mut store = AutopilotRecoveryStore::create(&root).expect("create recovery journal");
        store
            .mark_pending(&json!({ "mission_id": "possibly-dispatched" }))
            .expect("persist dispatch intent before execution");
        let grant: AutonomyGrant = serde_json::from_value(json!({
            "allowed_tools": ["workspace_capabilities"],
            "max_attempts": 1
        }))
        .expect("valid grant");

        let result = AutopilotRecoveryStore::restore(
            &root,
            &grant,
            &json!({ "mission_id": "possibly-dispatched" }),
        );
        assert!(
            matches!(result, Err(AutopilotError::InvalidCheckpoint { ref reason }) if reason.contains("outcome is unknown")),
            "an intent without a saved receipt must not be replayed"
        );
        fs::remove_dir_all(root).expect("remove isolated test recovery directory");
    }

    #[test]
    fn a_malformed_delivered_report_survives_private_recovery_and_stays_non_retryable() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows the epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bioprism-autopilot-malformed-report-{}-{stamp}",
            std::process::id()
        ));
        let mission = json!({
            "mission_id": "malformed-report-recovery",
            "goal": "preserve a dispatch whose response is invalid",
            "steps": [{
                "id": "capability",
                "domain": "metrics",
                "capability": "analytics",
                "objective": "inspect the workspace capability list",
                "tool": "workspace_capabilities",
                "arguments": {},
                "depends_on": [],
                "bindings": [],
                "required": true,
            }],
        });
        let grant: AutonomyGrant = serde_json::from_value(json!({
            "allowed_tools": ["workspace_capabilities"],
            "max_attempts": 1,
        }))
        .expect("valid grant");
        let malformed = json!({ "unexpected": "not a mission report" });
        let attempt = AttemptRecord::delivered(
            AttemptKind::Full,
            mission.clone(),
            malformed.clone(),
            None,
            None,
        )
        .expect("malformed reports are retained as attempt state");
        assert!(attempt.report_validation_error().is_some());
        let mut history = DriveHistory::new(mission.clone()).expect("valid mission");
        history.push(attempt);

        let mut store = AutopilotRecoveryStore::create(&root).expect("create recovery directory");
        store
            .mark_pending(&mission)
            .expect("record dispatch intent before committing its malformed response");
        store
            .checkpoint(&grant, &history)
            .expect("retain malformed response and checkpoint");

        let restored = AutopilotRecoveryStore::restore(&root, &grant, &mission)
            .expect("private attempt rehydrates against its digest-only checkpoint");
        assert_eq!(restored.attempts.len(), 1);
        assert_eq!(restored.attempts[0].report(), Some(&malformed));
        assert!(restored.attempts[0]
            .report_validation_error()
            .is_some_and(|error| !error.is_empty()));
        assert_eq!(restored.attempts[0].dispatch_error(), None);
        fs::remove_dir_all(root).expect("remove isolated test recovery directory");
    }
}
