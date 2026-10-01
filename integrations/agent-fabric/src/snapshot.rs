//! Versioned, integrity-checked recovery checkpoints for the in-process scheduler.
//!
//! Checkpoints preserve queued and delayed tasks plus the scheduler state needed to resume them.
//! They deliberately refuse live leases: a checkpoint cannot decide whether an external effect
//! already happened. Storage, encryption, retention, and reconciliation of effects outside this
//! process remain caller-owned.

use crate::cancel::CancelState;
use crate::capability::{Capability, CapabilitySet};
use crate::digest::{sha256, Digest};
use crate::envelope::{Receipt, TaskEnvelope, Terminal};
use crate::exec::Driver;
use crate::ids::{AgentId, IdempotencyKey, ShardId, TaskId};
use crate::json::{self, Value};
use crate::queue::BoundedQueue;
use crate::quota::{Bucket, QuotaLedger, QuotaSpec};
use crate::retry::RetryPolicy;
use crate::router::{AgentState, Router};
use crate::scheduler::{AssignmentSpan, Fabric, FabricConfig, Metrics, QueueEntry, TaskMeta};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
use std::fmt;

const SCHEMA: &str = "aurora.agent-fabric.checkpoint";
const VERSION: u64 = 1;
/// Payload bytes are hex-encoded in a checkpoint, so the decoded task set is smaller than this.
pub const MAX_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotError {
    message: String,
}

impl SnapshotError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SnapshotError {}

type Fields<'a> = BTreeMap<&'a str, &'a Value>;
type RetryHeap = BinaryHeap<std::cmp::Reverse<(u64, u64, TaskId)>>;

fn fail(message: impl Into<String>) -> SnapshotError {
    SnapshotError::new(message)
}

fn object(fields: Vec<(&str, Value)>) -> Value {
    Value::Obj(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn parse_hex(value: &Value, bytes: usize, label: &str) -> Result<Vec<u8>, SnapshotError> {
    let text = value
        .as_str()
        .ok_or_else(|| fail(format!("{label} must be a hexadecimal string")))?;
    if text.len() != bytes * 2 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(fail(format!("{label} has an invalid hexadecimal encoding")));
    }
    let mut decoded = Vec::with_capacity(bytes);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| fail(format!("{label} has an invalid hexadecimal encoding")))?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| fail(format!("{label} has an invalid hexadecimal encoding")))?;
        decoded.push(((high << 4) | low) as u8);
    }
    Ok(decoded)
}

fn parse_payload(value: &Value) -> Result<Vec<u8>, SnapshotError> {
    let text = value
        .as_str()
        .ok_or_else(|| fail("task payload must be a hexadecimal string"))?;
    if text.len() % 2 != 0 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(fail("task payload has an invalid hexadecimal encoding"));
    }
    let mut decoded = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks_exact(2) {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| fail("task payload has an invalid hexadecimal encoding"))?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| fail("task payload has an invalid hexadecimal encoding"))?;
        decoded.push(((high << 4) | low) as u8);
    }
    Ok(decoded)
}

fn fields<'a>(value: &'a Value, expected: &[&str]) -> Result<Fields<'a>, SnapshotError> {
    fields_optional(value, expected, &[])
}

fn fields_optional<'a>(
    value: &'a Value,
    required: &[&str],
    optional: &[&str],
) -> Result<Fields<'a>, SnapshotError> {
    let Value::Obj(entries) = value else {
        return Err(fail("checkpoint value must be an object"));
    };
    let mut result = BTreeMap::new();
    for (key, value) in entries {
        if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
            return Err(fail(format!("checkpoint contains an unknown field: {key}")));
        }
        if result.insert(key.as_str(), value).is_some() {
            return Err(fail(format!("checkpoint repeats the field: {key}")));
        }
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(fail(format!("checkpoint is missing the field: {key}")));
        }
    }
    Ok(result)
}

fn uint(fields: &Fields<'_>, key: &str) -> Result<u64, SnapshotError> {
    fields
        .get(key)
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            fail(format!(
                "checkpoint field {key} must be an unsigned integer"
            ))
        })
}

fn usize_field(fields: &Fields<'_>, key: &str) -> Result<usize, SnapshotError> {
    usize::try_from(uint(fields, key)?).map_err(|_| {
        fail(format!(
            "checkpoint field {key} exceeds this platform's range"
        ))
    })
}

fn u32_field(fields: &Fields<'_>, key: &str) -> Result<u32, SnapshotError> {
    u32::try_from(uint(fields, key)?).map_err(|_| {
        fail(format!(
            "checkpoint field {key} exceeds its supported range"
        ))
    })
}

fn text<'a>(fields: &'a Fields<'_>, key: &str) -> Result<&'a str, SnapshotError> {
    fields
        .get(key)
        .and_then(|value| value.as_str())
        .ok_or_else(|| fail(format!("checkpoint field {key} must be a string")))
}

fn boolean(fields: &Fields<'_>, key: &str) -> Result<bool, SnapshotError> {
    match fields.get(key) {
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err(fail(format!("checkpoint field {key} must be a boolean"))),
    }
}

fn array<'a>(fields: &'a Fields<'_>, key: &str) -> Result<&'a [Value], SnapshotError> {
    fields
        .get(key)
        .and_then(|value| value.as_arr())
        .ok_or_else(|| fail(format!("checkpoint field {key} must be an array")))
}

fn nullable_u64(fields: &Fields<'_>, key: &str) -> Result<Option<u64>, SnapshotError> {
    match fields.get(key) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| fail(format!("checkpoint field {key} must be null or unsigned"))),
        None => Err(fail(format!("checkpoint is missing the field: {key}"))),
    }
}

fn id<T>(raw: u64, make: impl FnOnce(u64) -> Option<T>, label: &str) -> Result<T, SnapshotError> {
    make(raw).ok_or_else(|| fail(format!("checkpoint contains invalid {label} {raw}")))
}

fn caps_value(caps: &CapabilitySet) -> Value {
    Value::Arr(caps.iter().map(|cap| Value::str(cap.as_str())).collect())
}

fn parse_caps(value: &Value) -> Result<CapabilitySet, SnapshotError> {
    let values = value
        .as_arr()
        .ok_or_else(|| fail("capability set must be an array"))?;
    if values.is_empty() {
        return Err(fail("capability set must not be empty"));
    }
    let mut parsed = Vec::with_capacity(values.len());
    let mut prior: Option<String> = None;
    for value in values {
        let raw = value
            .as_str()
            .ok_or_else(|| fail("capability names must be strings"))?;
        let capability = Capability::parse(raw)
            .map_err(|error| fail(format!("checkpoint capability is invalid: {error}")))?;
        if capability.as_str() != raw
            || prior
                .as_deref()
                .is_some_and(|previous| previous >= capability.as_str())
        {
            return Err(fail(
                "checkpoint capabilities must be normalized, unique, and sorted",
            ));
        }
        prior = Some(capability.as_str().to_string());
        parsed.push(capability);
    }
    Ok(CapabilitySet::from_caps(parsed))
}

fn config_value(config: &FabricConfig) -> Value {
    object(vec![
        ("shards", Value::Uint(config.shards)),
        (
            "per_shard_queue_cap",
            Value::Uint(config.per_shard_queue_cap as u64),
        ),
        (
            "max_pending_tasks",
            Value::Uint(config.max_pending_tasks as u64),
        ),
        ("max_in_flight", Value::Uint(config.max_in_flight as u64)),
        (
            "default_lease_ttl_ticks",
            Value::Uint(config.default_lease_ttl_ticks),
        ),
        (
            "retry",
            object(vec![
                (
                    "max_attempts",
                    Value::Uint(u64::from(config.retry.max_attempts)),
                ),
                ("base_ticks", Value::Uint(config.retry.base_ticks)),
                ("cap_ticks", Value::Uint(config.retry.cap_ticks)),
            ]),
        ),
        (
            "quota",
            object(vec![
                ("burst", Value::Uint(u64::from(config.quota.burst))),
                ("refill_every", Value::Uint(config.quota.refill_every)),
                (
                    "refill_amount",
                    Value::Uint(u64::from(config.quota.refill_amount)),
                ),
            ]),
        ),
        (
            "receipt_retention",
            Value::Uint(config.receipt_retention as u64),
        ),
    ])
}

fn parse_config(value: &Value) -> Result<FabricConfig, SnapshotError> {
    let config = fields(
        value,
        &[
            "shards",
            "per_shard_queue_cap",
            "max_pending_tasks",
            "max_in_flight",
            "default_lease_ttl_ticks",
            "retry",
            "quota",
            "receipt_retention",
        ],
    )?;
    let retry_fields = fields(
        config["retry"],
        &["max_attempts", "base_ticks", "cap_ticks"],
    )?;
    let quota_fields = fields(config["quota"], &["burst", "refill_every", "refill_amount"])?;
    let parsed = FabricConfig {
        shards: uint(&config, "shards")?,
        per_shard_queue_cap: usize_field(&config, "per_shard_queue_cap")?,
        max_pending_tasks: usize_field(&config, "max_pending_tasks")?,
        max_in_flight: usize_field(&config, "max_in_flight")?,
        default_lease_ttl_ticks: uint(&config, "default_lease_ttl_ticks")?,
        retry: RetryPolicy {
            max_attempts: u32_field(&retry_fields, "max_attempts")?,
            base_ticks: uint(&retry_fields, "base_ticks")?,
            cap_ticks: uint(&retry_fields, "cap_ticks")?,
        },
        quota: QuotaSpec {
            burst: u32_field(&quota_fields, "burst")?,
            refill_every: uint(&quota_fields, "refill_every")?,
            refill_amount: u32_field(&quota_fields, "refill_amount")?,
        },
        receipt_retention: usize_field(&config, "receipt_retention")?,
    };
    if parsed.shards == 0
        || parsed.shards > 4096
        || parsed.per_shard_queue_cap == 0
        || parsed.max_pending_tasks == 0
        || parsed.default_lease_ttl_ticks == 0
        || parsed.retry.max_attempts == 0
        || parsed.quota.refill_every == 0
    {
        return Err(fail("checkpoint configuration violates scheduler bounds"));
    }
    Ok(parsed)
}

fn router_value(router: &Router) -> Value {
    let agents = router
        .agents
        .iter()
        .map(|(id, record)| {
            let shard = router
                .agent_shard
                .get(id)
                .expect("registered agent has a shard");
            object(vec![
                ("id", Value::Uint(id.raw())),
                ("name", Value::str(record.name.clone())),
                ("capabilities", caps_value(&record.caps)),
                ("state", Value::str(agent_state_name(record.state))),
                ("shard", Value::Uint(shard.raw())),
            ])
        })
        .collect();
    let cursors = router
        .cursors
        .iter()
        .map(|((shard, capability), cursor)| {
            object(vec![
                ("shard", Value::Uint(shard.raw())),
                ("capability", Value::str(capability.as_str())),
                ("cursor", Value::Uint(*cursor as u64)),
            ])
        })
        .collect();
    object(vec![
        ("shard_count", Value::Uint(router.shard_count)),
        ("next_agent", Value::Uint(router.next_agent)),
        ("agents", Value::Arr(agents)),
        ("cursors", Value::Arr(cursors)),
    ])
}

fn agent_state_name(state: AgentState) -> &'static str {
    match state {
        AgentState::Active => "active",
        AgentState::Draining => "draining",
        AgentState::Down => "down",
    }
}

fn parse_agent_state(raw: &str) -> Result<AgentState, SnapshotError> {
    match raw {
        "active" => Ok(AgentState::Active),
        "draining" => Ok(AgentState::Draining),
        "down" => Ok(AgentState::Down),
        _ => Err(fail("checkpoint contains an unknown agent state")),
    }
}

fn parse_router(value: &Value, config: &FabricConfig) -> Result<Router, SnapshotError> {
    let state = fields(value, &["shard_count", "next_agent", "agents", "cursors"])?;
    if uint(&state, "shard_count")? != config.shards {
        return Err(fail(
            "checkpoint router shard count differs from its configuration",
        ));
    }
    let mut router = Router::new(config.shards);
    for (expected_id, value) in (1u64..).zip(array(&state, "agents")?) {
        let record = fields(value, &["id", "name", "capabilities", "state", "shard"])?;
        let raw_id = uint(&record, "id")?;
        if raw_id != expected_id {
            return Err(fail("checkpoint agent ids must be contiguous and ordered"));
        }
        let name = text(&record, "name")?;
        let capabilities = parse_caps(record["capabilities"])?;
        let agent = router.register(name, capabilities);
        if agent.raw() != raw_id
            || router.shard_of(agent).map(ShardId::raw) != Some(uint(&record, "shard")?)
        {
            return Err(fail(
                "checkpoint agent placement does not match deterministic routing",
            ));
        }
        router
            .set_state(agent, parse_agent_state(text(&record, "state")?)?)
            .map_err(|_| fail("checkpoint agent registration could not be restored"))?;
    }
    if router.next_agent != uint(&state, "next_agent")? {
        return Err(fail(
            "checkpoint agent sequence does not match its registry",
        ));
    }
    for value in array(&state, "cursors")? {
        let cursor = fields(value, &["shard", "capability", "cursor"])?;
        let raw_shard = uint(&cursor, "shard")?;
        if raw_shard >= config.shards {
            return Err(fail("checkpoint fairness cursor names an unknown shard"));
        }
        let raw_capability = text(&cursor, "capability")?;
        let capability = Capability::parse(raw_capability).map_err(|error| {
            fail(format!(
                "checkpoint fairness capability is invalid: {error}"
            ))
        })?;
        if capability.as_str() != raw_capability {
            return Err(fail("checkpoint fairness capability is not normalized"));
        }
        let key = (ShardId::new(raw_shard), capability);
        if router
            .cursors
            .insert(key, usize_field(&cursor, "cursor")?)
            .is_some()
        {
            return Err(fail("checkpoint repeats a fairness cursor"));
        }
    }
    Ok(router)
}

fn quota_value(quotas: &QuotaLedger) -> Value {
    Value::Arr(
        quotas
            .buckets
            .iter()
            .map(|(agent, bucket)| {
                object(vec![
                    ("agent_id", Value::Uint(agent.raw())),
                    ("tokens", Value::Uint(bucket.tokens)),
                    ("last_synced_tick", Value::Uint(bucket.last_synced_tick)),
                ])
            })
            .collect(),
    )
}

fn parse_quotas(
    value: &Value,
    config: QuotaSpec,
    router: &Router,
    clock: u64,
) -> Result<QuotaLedger, SnapshotError> {
    let mut quotas = QuotaLedger::new(config);
    for agent in router.agents.keys() {
        quotas.register(*agent);
    }
    let mut seen = BTreeSet::new();
    for value in value
        .as_arr()
        .ok_or_else(|| fail("checkpoint quotas must be an array"))?
    {
        let bucket = fields(value, &["agent_id", "tokens", "last_synced_tick"])?;
        let agent = id(uint(&bucket, "agent_id")?, AgentId::from_raw, "agent id")?;
        let tokens = uint(&bucket, "tokens")?;
        let last_synced_tick = uint(&bucket, "last_synced_tick")?;
        if !router.agents.contains_key(&agent)
            || !seen.insert(agent)
            || tokens > u64::from(config.burst)
            || last_synced_tick > clock
        {
            return Err(fail(
                "checkpoint quota bucket is invalid for the restored agent or clock",
            ));
        }
        quotas.buckets.insert(
            agent,
            Bucket {
                tokens,
                last_synced_tick,
            },
        );
    }
    if seen.len() != router.agents.len() {
        return Err(fail(
            "checkpoint must carry one quota bucket per registered agent",
        ));
    }
    Ok(quotas)
}

fn task_value(task: TaskId, meta: &TaskMeta) -> Value {
    object(vec![
        ("id", Value::Uint(task.raw())),
        ("payload_hex", Value::str(hex(meta.env.payload()))),
        (
            "payload_digest",
            Value::str(meta.env.payload_digest().hex()),
        ),
        ("capabilities", caps_value(meta.env.capabilities())),
        ("idempotency_key", Value::str(meta.key.hex())),
        ("created_tick", Value::Uint(meta.env.created_tick())),
        (
            "max_attempts",
            Value::Uint(u64::from(meta.env.max_attempts())),
        ),
        ("submitted_tick", Value::Uint(meta.submitted_tick)),
        ("attempts_done", Value::Uint(u64::from(meta.attempts_done))),
        (
            "last_agent",
            meta.last_agent
                .map(|agent| Value::Uint(agent.raw()))
                .unwrap_or(Value::Null),
        ),
        ("cancel_requested", Value::Bool(meta.cancel_requested)),
    ])
}

fn parse_task(
    value: &Value,
    config: &FabricConfig,
    router: &Router,
    clock: u64,
) -> Result<(TaskId, TaskMeta), SnapshotError> {
    let task = fields(
        value,
        &[
            "id",
            "payload_hex",
            "payload_digest",
            "capabilities",
            "idempotency_key",
            "created_tick",
            "max_attempts",
            "submitted_tick",
            "attempts_done",
            "last_agent",
            "cancel_requested",
        ],
    )?;
    let task_id = id(uint(&task, "id")?, TaskId::from_raw, "task id")?;
    let payload = parse_payload(task["payload_hex"])?;
    let digest_bytes = parse_hex(task["payload_digest"], 32, "payload digest")?;
    let digest = Digest::from_hex(&hex(&digest_bytes)).expect("digest length was checked");
    let capabilities = parse_caps(task["capabilities"])?;
    let key_bytes = parse_hex(task["idempotency_key"], 16, "idempotency key")?;
    let key = IdempotencyKey::from_bytes(key_bytes.try_into().expect("key length was checked"));
    let created_tick = uint(&task, "created_tick")?;
    let submitted_tick = uint(&task, "submitted_tick")?;
    let max_attempts = u32_field(&task, "max_attempts")?;
    let attempts_done = u32_field(&task, "attempts_done")?;
    let last_agent = nullable_u64(&task, "last_agent")?
        .map(|raw| id(raw, AgentId::from_raw, "agent id"))
        .transpose()?;
    if created_tick != submitted_tick
        || submitted_tick > clock
        || max_attempts != config.retry.max_attempts
        || attempts_done > max_attempts
        || last_agent.is_some_and(|agent| !router.agents.contains_key(&agent))
    {
        return Err(fail(
            "checkpoint task metadata is inconsistent with its scheduler state",
        ));
    }
    let env = TaskEnvelope::compose(
        task_id,
        payload,
        capabilities,
        Some(key),
        created_tick,
        max_attempts,
    );
    if env.payload_digest() != digest {
        return Err(fail("checkpoint task payload does not match its digest"));
    }
    Ok((
        task_id,
        TaskMeta {
            env,
            submitted_tick,
            attempts_done,
            last_agent,
            cancel_requested: boolean(&task, "cancel_requested")?,
            key,
        },
    ))
}

fn metrics_value(metrics: &Metrics) -> Value {
    object(vec![
        ("submitted", Value::Uint(metrics.submitted)),
        (
            "duplicate_submissions",
            Value::Uint(metrics.duplicate_submissions),
        ),
        (
            "backpressure_rejections",
            Value::Uint(metrics.backpressure_rejections),
        ),
        (
            "pending_limit_rejections",
            Value::Uint(metrics.pending_limit_rejections),
        ),
        ("admitted", Value::Uint(metrics.admitted)),
        ("dispatched", Value::Uint(metrics.dispatched)),
        ("retried", Value::Uint(metrics.retried)),
        ("deferred_by_quota", Value::Uint(metrics.deferred_by_quota)),
        (
            "deferred_by_driver_full",
            Value::Uint(metrics.deferred_by_driver_full),
        ),
        ("unroutable_parks", Value::Uint(metrics.unroutable_parks)),
        ("lease_expiries", Value::Uint(metrics.lease_expiries)),
        ("succeeded", Value::Uint(metrics.succeeded)),
        ("failed_terminal", Value::Uint(metrics.failed_terminal)),
        (
            "cancelled_terminal",
            Value::Uint(metrics.cancelled_terminal),
        ),
        ("dropped_terminal", Value::Uint(metrics.dropped_terminal)),
        (
            "corrupted_settlements",
            Value::Uint(metrics.corrupted_settlements),
        ),
        ("receipts_evicted", Value::Uint(metrics.receipts_evicted)),
        (
            "idempotency_evictions",
            Value::Uint(metrics.idempotency_evictions),
        ),
        (
            "cancellations_requested",
            Value::Uint(metrics.cancellations_requested),
        ),
        (
            "ready_high_water",
            Value::Uint(metrics.ready_high_water as u64),
        ),
        (
            "pending_high_water",
            Value::Uint(metrics.pending_high_water as u64),
        ),
        (
            "in_flight_high_water",
            Value::Uint(metrics.in_flight_high_water as u64),
        ),
    ])
}

fn parse_metrics(value: &Value) -> Result<Metrics, SnapshotError> {
    let names = [
        "submitted",
        "duplicate_submissions",
        "backpressure_rejections",
        "pending_limit_rejections",
        "admitted",
        "dispatched",
        "retried",
        "deferred_by_quota",
        "deferred_by_driver_full",
        "unroutable_parks",
        "lease_expiries",
        "succeeded",
        "failed_terminal",
        "cancelled_terminal",
        "dropped_terminal",
        "corrupted_settlements",
        "receipts_evicted",
        "idempotency_evictions",
        "cancellations_requested",
        "ready_high_water",
        "pending_high_water",
        "in_flight_high_water",
    ];
    let metrics = fields(value, &names)?;
    Ok(Metrics {
        submitted: uint(&metrics, "submitted")?,
        duplicate_submissions: uint(&metrics, "duplicate_submissions")?,
        backpressure_rejections: uint(&metrics, "backpressure_rejections")?,
        pending_limit_rejections: uint(&metrics, "pending_limit_rejections")?,
        admitted: uint(&metrics, "admitted")?,
        dispatched: uint(&metrics, "dispatched")?,
        retried: uint(&metrics, "retried")?,
        deferred_by_quota: uint(&metrics, "deferred_by_quota")?,
        deferred_by_driver_full: uint(&metrics, "deferred_by_driver_full")?,
        unroutable_parks: uint(&metrics, "unroutable_parks")?,
        lease_expiries: uint(&metrics, "lease_expiries")?,
        succeeded: uint(&metrics, "succeeded")?,
        failed_terminal: uint(&metrics, "failed_terminal")?,
        cancelled_terminal: uint(&metrics, "cancelled_terminal")?,
        dropped_terminal: uint(&metrics, "dropped_terminal")?,
        corrupted_settlements: uint(&metrics, "corrupted_settlements")?,
        receipts_evicted: uint(&metrics, "receipts_evicted")?,
        idempotency_evictions: uint(&metrics, "idempotency_evictions")?,
        cancellations_requested: uint(&metrics, "cancellations_requested")?,
        ready_high_water: usize_field(&metrics, "ready_high_water")?,
        pending_high_water: usize_field(&metrics, "pending_high_water")?,
        in_flight_high_water: usize_field(&metrics, "in_flight_high_water")?,
    })
}

fn assignment_value(span: &AssignmentSpan) -> Value {
    object(vec![
        ("task", Value::Uint(span.task.raw())),
        ("agent", Value::Uint(span.agent.raw())),
        ("started_tick", Value::Uint(span.started_tick)),
        (
            "ended_tick",
            span.ended_tick.map(Value::Uint).unwrap_or(Value::Null),
        ),
    ])
}

fn parse_assignments(
    value: &Value,
    router: &Router,
    clock: u64,
) -> Result<Vec<AssignmentSpan>, SnapshotError> {
    let mut spans = Vec::new();
    for value in value
        .as_arr()
        .ok_or_else(|| fail("checkpoint assignments must be an array"))?
    {
        let span = fields(value, &["task", "agent", "started_tick", "ended_tick"])?;
        let task = id(uint(&span, "task")?, TaskId::from_raw, "task id")?;
        let agent = id(uint(&span, "agent")?, AgentId::from_raw, "agent id")?;
        let started_tick = uint(&span, "started_tick")?;
        let ended_tick = nullable_u64(&span, "ended_tick")?
            .ok_or_else(|| fail("checkpoint cannot retain an open assignment interval"))?;
        if task.raw() == 0
            || started_tick > ended_tick
            || ended_tick > clock
            || !router.agents.contains_key(&agent)
        {
            return Err(fail("checkpoint assignment interval is inconsistent"));
        }
        spans.push(AssignmentSpan {
            task,
            agent,
            started_tick,
            ended_tick: Some(ended_tick),
        });
    }
    Ok(spans)
}

fn parse_receipt(value: &Value, router: &Router, clock: u64) -> Result<Receipt, SnapshotError> {
    let receipt = fields_optional(
        value,
        &[
            "task_id",
            "agent_id",
            "attempts",
            "terminal",
            "payload_digest",
            "submitted_tick",
            "settled_tick",
            "cancel_requested",
        ],
        &["reason"],
    )?;
    let task = id(uint(&receipt, "task_id")?, TaskId::from_raw, "task id")?;
    let agent = nullable_u64(&receipt, "agent_id")?
        .map(|raw| id(raw, AgentId::from_raw, "agent id"))
        .transpose()?;
    if agent.is_some_and(|agent| !router.agents.contains_key(&agent)) {
        return Err(fail("checkpoint receipt names an unregistered agent"));
    }
    let attempts = u32_field(&receipt, "attempts")?;
    let terminal_text = text(&receipt, "terminal")?;
    let reason = receipt.get("reason").and_then(|value| value.as_str());
    let terminal = match terminal_text {
        "succeeded" if reason.is_none() => Terminal::Succeeded,
        "cancelled" if reason.is_none() => Terminal::Cancelled,
        "corrupted payload" if reason.is_none() => Terminal::CorruptedPayload,
        "result digest mismatch" if reason.is_none() => Terminal::ResultDigestMismatch,
        "dropped without a verdict" if reason.is_none() => Terminal::Dropped,
        value if value.starts_with("failed: ") => {
            let reason = reason.ok_or_else(|| fail("failed receipt is missing its reason"))?;
            if value != format!("failed: {reason}") {
                return Err(fail("failed receipt terminal and reason disagree"));
            }
            Terminal::Failed {
                reason: reason.to_string(),
            }
        }
        _ => {
            return Err(fail(
                "checkpoint contains an unknown or malformed receipt terminal",
            ))
        }
    };
    if attempts == 0 {
        return Err(fail("checkpoint receipt must record at least one attempt"));
    }
    let digest_bytes = parse_hex(receipt["payload_digest"], 32, "receipt payload digest")?;
    let digest = Digest::from_hex(&hex(&digest_bytes)).expect("digest length was checked");
    let submitted_tick = uint(&receipt, "submitted_tick")?;
    let settled_tick = uint(&receipt, "settled_tick")?;
    if settled_tick < submitted_tick || settled_tick > clock {
        return Err(fail(
            "checkpoint receipt ticks fall outside the checkpoint clock",
        ));
    }
    Ok(Receipt::new(
        task,
        agent,
        attempts,
        terminal,
        digest,
        submitted_tick,
        settled_tick,
        boolean(&receipt, "cancel_requested")?,
    ))
}

fn encode_state(fabric: &Fabric) -> Result<Value, SnapshotError> {
    if !fabric.in_flight.is_empty() || fabric.leases.live() != 0 || !fabric.busy_agents.is_empty() {
        return Err(fail("checkpoint state contains an active attempt or lease"));
    }
    if fabric.tasks.len() > fabric.cfg.max_pending_tasks
        || fabric.retry_heap.len() > fabric.cfg.max_pending_tasks
        || fabric.receipts.len() > fabric.cfg.receipt_retention
    {
        return Err(fail(
            "checkpoint state exceeds a configured scheduler bound",
        ));
    }
    let mut payload_bytes = 0usize;
    for meta in fabric.tasks.values() {
        payload_bytes = payload_bytes
            .checked_add(meta.env.payload().len())
            .ok_or_else(|| fail("checkpoint payload size overflowed"))?;
    }
    if payload_bytes > MAX_CHECKPOINT_BYTES / 2 {
        return Err(fail(
            "checkpoint payloads exceed the maximum checkpoint size",
        ));
    }

    let mut locations = BTreeSet::new();
    for queue in &fabric.queues {
        for entry in queue.iter() {
            let QueueEntry::Ready(task) = entry;
            if !fabric.tasks.contains_key(task) || !locations.insert(*task) {
                return Err(fail(
                    "checkpoint queues contain an unknown or repeated task",
                ));
            }
        }
    }
    for entry in &fabric.retry_heap {
        let std::cmp::Reverse((_, _, task)) = entry;
        if !fabric.tasks.contains_key(task) || !locations.insert(*task) {
            return Err(fail(
                "checkpoint retries contain an unknown or repeated task",
            ));
        }
    }
    if locations.len() != fabric.tasks.len() {
        return Err(fail(
            "every pending task must occur in exactly one queue or retry slot",
        ));
    }
    if fabric
        .assignments
        .iter()
        .any(|span| span.ended_tick.is_none())
    {
        return Err(fail(
            "checkpoint cannot contain an open assignment interval",
        ));
    }

    let queues = fabric
        .queues
        .iter()
        .map(|queue| {
            Value::Arr(
                queue
                    .iter()
                    .map(|entry| match entry {
                        QueueEntry::Ready(task) => Value::Uint(task.raw()),
                    })
                    .collect(),
            )
        })
        .collect();
    let mut retries: Vec<_> = fabric.retry_heap.iter().map(|entry| entry.0).collect();
    retries.sort_unstable();
    let retries = retries
        .into_iter()
        .map(|(tick, sequence, task)| {
            object(vec![
                ("tick", Value::Uint(tick)),
                ("sequence", Value::Uint(sequence)),
                ("task", Value::Uint(task.raw())),
            ])
        })
        .collect();
    let tasks = fabric
        .tasks
        .iter()
        .map(|(task, meta)| task_value(*task, meta))
        .collect();
    let receipts = fabric.receipts.iter().map(Receipt::to_json).collect();
    let assignments = fabric.assignments.iter().map(assignment_value).collect();

    Ok(object(vec![
        ("config", config_value(&fabric.cfg)),
        ("clock", Value::Uint(fabric.clock)),
        ("next_task", Value::Uint(fabric.next_task)),
        ("sequence", Value::Uint(fabric.seq)),
        ("shard_cursor", Value::Uint(fabric.shard_cursor as u64)),
        ("router", router_value(&fabric.router)),
        ("quotas", quota_value(&fabric.quotas)),
        ("queues", Value::Arr(queues)),
        ("retries", Value::Arr(retries)),
        ("tasks", Value::Arr(tasks)),
        ("receipts", Value::Arr(receipts)),
        ("metrics", metrics_value(&fabric.metrics)),
        ("assignments", Value::Arr(assignments)),
        ("lease_next_epoch", Value::Uint(fabric.leases.next_epoch)),
        ("record_assignments", Value::Bool(fabric.record_assignments)),
    ]))
}

pub(crate) fn encode(fabric: &Fabric) -> Result<String, SnapshotError> {
    let state = encode_state(fabric)?;
    let state_bytes = json::to_string(&state);
    let digest = sha256(state_bytes.as_bytes());
    let checkpoint = object(vec![
        ("schema", Value::str(SCHEMA)),
        ("version", Value::Uint(VERSION)),
        ("state", state),
        ("state_digest", Value::str(digest.hex())),
    ]);
    let serialized = json::to_string(&checkpoint);
    if serialized.len() > MAX_CHECKPOINT_BYTES {
        return Err(fail("checkpoint exceeds the maximum serialized size"));
    }
    Ok(serialized)
}

fn parse_retry_heap(
    value: &Value,
    tasks: &BTreeMap<TaskId, TaskMeta>,
    locations: &mut BTreeSet<TaskId>,
    max_sequence: u64,
) -> Result<RetryHeap, SnapshotError> {
    let mut heap = BinaryHeap::new();
    let mut sequences = BTreeSet::new();
    for value in value
        .as_arr()
        .ok_or_else(|| fail("checkpoint retries must be an array"))?
    {
        let retry = fields(value, &["tick", "sequence", "task"])?;
        let tick = uint(&retry, "tick")?;
        let sequence = uint(&retry, "sequence")?;
        let task = id(uint(&retry, "task")?, TaskId::from_raw, "task id")?;
        if !tasks.contains_key(&task)
            || !locations.insert(task)
            || sequence == 0
            || sequence > max_sequence
            || !sequences.insert(sequence)
        {
            return Err(fail("checkpoint retry entry is invalid or repeated"));
        }
        heap.push(std::cmp::Reverse((tick, sequence, task)));
    }
    Ok(heap)
}

fn parse_queues(
    value: &Value,
    config: &FabricConfig,
    tasks: &BTreeMap<TaskId, TaskMeta>,
    locations: &mut BTreeSet<TaskId>,
) -> Result<Vec<BoundedQueue<QueueEntry>>, SnapshotError> {
    let values = value
        .as_arr()
        .ok_or_else(|| fail("checkpoint queues must be an array"))?;
    if values.len() != config.shards as usize {
        return Err(fail(
            "checkpoint queue count differs from the configured shard count",
        ));
    }
    let mut queues = Vec::with_capacity(values.len());
    for value in values {
        let tasks_in_queue = value
            .as_arr()
            .ok_or_else(|| fail("each checkpoint queue must be an array"))?;
        if tasks_in_queue.len() > config.per_shard_queue_cap {
            return Err(fail(
                "checkpoint ready queue exceeds its configured capacity",
            ));
        }
        let mut queue = BoundedQueue::new(config.per_shard_queue_cap);
        for value in tasks_in_queue {
            let task = id(
                value
                    .as_u64()
                    .ok_or_else(|| fail("queued task id must be unsigned"))?,
                TaskId::from_raw,
                "task id",
            )?;
            if !tasks.contains_key(&task) || !locations.insert(task) {
                return Err(fail(
                    "checkpoint queue contains an unknown or repeated task",
                ));
            }
            queue
                .push(QueueEntry::Ready(task))
                .map_err(|_| fail("checkpoint ready queue exceeds its configured capacity"))?;
        }
        queues.push(queue);
    }
    Ok(queues)
}

fn decode_state(
    value: &Value,
    driver: Box<dyn Driver>,
    cancels: CancelState,
) -> Result<Fabric, SnapshotError> {
    let state = fields(
        value,
        &[
            "config",
            "clock",
            "next_task",
            "sequence",
            "shard_cursor",
            "router",
            "quotas",
            "queues",
            "retries",
            "tasks",
            "receipts",
            "metrics",
            "assignments",
            "lease_next_epoch",
            "record_assignments",
        ],
    )?;
    let config = parse_config(state["config"])?;
    let clock = uint(&state, "clock")?;
    let next_task = uint(&state, "next_task")?;
    let sequence = uint(&state, "sequence")?;
    let shard_cursor = usize_field(&state, "shard_cursor")?;
    if shard_cursor >= config.shards as usize {
        return Err(fail(
            "checkpoint shard cursor is outside the configured ring",
        ));
    }
    let router = parse_router(state["router"], &config)?;
    let quotas = parse_quotas(state["quotas"], config.quota, &router, clock)?;

    let mut tasks = BTreeMap::new();
    let task_values = array(&state, "tasks")?;
    if task_values.len() > config.max_pending_tasks {
        return Err(fail(
            "checkpoint pending task count exceeds its configured limit",
        ));
    }
    for value in task_values {
        let (task_id, meta) = parse_task(value, &config, &router, clock)?;
        if task_id.raw() > next_task || tasks.insert(task_id, meta).is_some() {
            return Err(fail(
                "checkpoint task id is repeated or exceeds the task sequence",
            ));
        }
    }

    let mut locations = BTreeSet::new();
    let queues = parse_queues(state["queues"], &config, &tasks, &mut locations)?;
    let retry_heap = parse_retry_heap(state["retries"], &tasks, &mut locations, sequence)?;
    if locations.len() != tasks.len() {
        return Err(fail(
            "every pending task must occur in exactly one queue or retry slot",
        ));
    }

    let mut receipts = VecDeque::new();
    let mut receipt_ids = BTreeSet::new();
    for value in array(&state, "receipts")? {
        let receipt = parse_receipt(value, &router, clock)?;
        if receipt.task().raw() > next_task
            || tasks.contains_key(&receipt.task())
            || !receipt_ids.insert(receipt.task())
        {
            return Err(fail("checkpoint receipt identity is invalid or repeated"));
        }
        receipts.push_back(receipt);
    }
    if receipts.len() > config.receipt_retention {
        return Err(fail(
            "checkpoint receipt ledger exceeds its retention bound",
        ));
    }

    let metrics = parse_metrics(state["metrics"])?;
    let total_queue_capacity = config
        .per_shard_queue_cap
        .checked_mul(config.shards as usize)
        .ok_or_else(|| fail("checkpoint aggregate queue capacity overflows this platform"))?;
    if metrics.ready_high_water > total_queue_capacity
        || metrics.pending_high_water > config.max_pending_tasks
        || metrics.in_flight_high_water > config.max_in_flight
        || metrics.pending_high_water < tasks.len()
        || metrics.pending_limit_rejections > metrics.backpressure_rejections
        || metrics.submitted
            != metrics
                .admitted
                .saturating_add(metrics.duplicate_submissions)
                .saturating_add(metrics.backpressure_rejections)
        || next_task != metrics.admitted
    {
        return Err(fail(
            "checkpoint metrics are inconsistent with scheduler history",
        ));
    }
    let assignments = parse_assignments(state["assignments"], &router, clock)?;
    if assignments.iter().any(|span| span.task.raw() > next_task) {
        return Err(fail(
            "checkpoint assignment references a task beyond the task sequence",
        ));
    }
    let lease_next_epoch = uint(&state, "lease_next_epoch")?;
    if lease_next_epoch < metrics.dispatched {
        // Every accepted dispatch must have minted a unique lease epoch. Driver-full grants can
        // make the epoch counter larger than the dispatch count, but never smaller.
        return Err(fail(
            "checkpoint lease epoch is behind its dispatch history",
        ));
    }
    let record_assignments = boolean(&state, "record_assignments")?;

    let mut fabric = Fabric::new(config, driver, cancels.clone());
    fabric.router = router;
    fabric.quotas = quotas;
    fabric.queues = queues;
    fabric.tasks = tasks;
    fabric.idem.clear();
    let mut cancelled = Vec::new();
    for (task, meta) in &fabric.tasks {
        if fabric.idem.insert(meta.key, *task).is_some() {
            return Err(fail("checkpoint pending tasks repeat an idempotency key"));
        }
        if meta.cancel_requested {
            cancelled.push(*task);
        }
    }
    fabric.cancels.restore_cancelled(&cancelled);
    fabric.retry_heap = retry_heap;
    fabric.receipts = receipts;
    fabric.metrics = metrics;
    fabric.assignments = assignments;
    fabric.leases.next_epoch = lease_next_epoch;
    fabric.clock = clock;
    fabric.next_task = next_task;
    fabric.seq = sequence;
    fabric.shard_cursor = shard_cursor;
    fabric.record_assignments = record_assignments;
    Ok(fabric)
}

pub(crate) fn decode(
    bytes: &str,
    driver: Box<dyn Driver>,
    cancels: CancelState,
) -> Result<Fabric, SnapshotError> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(fail("checkpoint exceeds the maximum serialized size"));
    }
    let root =
        json::parse(bytes).map_err(|error| fail(format!("checkpoint JSON is invalid: {error}")))?;
    let root = fields(&root, &["schema", "version", "state", "state_digest"])?;
    if text(&root, "schema")? != SCHEMA || uint(&root, "version")? != VERSION {
        return Err(fail("checkpoint schema or version is not supported"));
    }
    let state_bytes = json::to_string(root["state"]);
    let claimed = parse_hex(root["state_digest"], 32, "checkpoint state digest")?;
    if sha256(state_bytes.as_bytes()).as_bytes().as_slice() != claimed.as_slice() {
        return Err(fail("checkpoint state digest does not match its contents"));
    }
    decode_state(root["state"], driver, cancels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Outcome;
    use crate::exec::{Enqueue, FnHandler, InlineDriver};
    use std::sync::Arc;

    fn capability(name: &str) -> CapabilitySet {
        CapabilitySet::one(Capability::parse(name).expect("valid capability"))
    }

    fn echo_handler() -> Arc<FnHandler<impl Fn(&crate::envelope::DispatchJob) -> Outcome>> {
        Arc::new(FnHandler(|job: &crate::envelope::DispatchJob| {
            Outcome::Succeeded {
                result: job.envelope.payload().to_vec(),
            }
        }))
    }

    fn driver(cancels: CancelState) -> Box<dyn Driver> {
        Box::new(InlineDriver::new(echo_handler(), cancels))
    }

    fn failed_driver(cancels: CancelState) -> Box<dyn Driver> {
        Box::new(InlineDriver::new(
            Arc::new(FnHandler(|_: &crate::envelope::DispatchJob| {
                Outcome::Failed {
                    reason: "worker failed".into(),
                }
            })),
            cancels,
        ))
    }

    fn fabric(config: FabricConfig) -> (Fabric, CancelState) {
        let cancels = CancelState::new();
        let fabric = Fabric::new(config, driver(cancels.clone()), cancels.clone());
        (fabric, cancels)
    }

    fn restore(checkpoint: &str) -> Fabric {
        let cancels = CancelState::new();
        Fabric::from_checkpoint(checkpoint, driver(cancels.clone()), cancels)
            .expect("valid checkpoint restores")
    }

    #[test]
    fn checkpoint_round_trips_router_quota_retries_receipts_and_pending_idempotency() {
        let (mut original, _) = fabric(FabricConfig::default());
        let compute = original.register_agent("compute-worker", capability("compute"));
        let down = original.register_agent("offline-worker", capability("offline"));
        original
            .mark_agent(down, AgentState::Down)
            .expect("registered agent can be marked down");
        original.set_record_assignments(true);
        let completed = match original.submit(b"completed".to_vec(), capability("compute"), None) {
            crate::scheduler::Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        let pending_payload = b"wait for worker".to_vec();
        let pending_caps = capability("storage");
        let pending = match original.submit(pending_payload.clone(), pending_caps.clone(), None) {
            crate::scheduler::Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        original.step_to(0);
        assert_eq!(original.retry_heap.len(), 1);

        let checkpoint = original.checkpoint().expect("no attempt remains live");
        assert_eq!(original.receipt_count(), 1);
        assert_eq!(original.assignments().len(), 1);
        assert!(original.assignments()[0].ended_tick.is_some());
        let quota_tokens = original.quotas.buckets[&compute].tokens;

        let mut recovered = restore(&checkpoint);
        assert_eq!(recovered.receipt_count(), 1);
        assert_eq!(
            recovered.receipts().next().expect("receipt").task(),
            completed
        );
        assert_eq!(recovered.router().state_of(down), Some(AgentState::Down));
        assert_eq!(recovered.assignments().len(), 1);
        assert_eq!(recovered.quotas.buckets[&compute].tokens, quota_tokens);
        assert_eq!(recovered.memory_stats().retry_pending, 1);
        assert_eq!(
            recovered
                .checkpoint()
                .expect("recovered state is quiescent"),
            checkpoint,
            "restoration must preserve deterministic checkpoint bytes"
        );
        assert!(matches!(
            recovered.submit(pending_payload, pending_caps.clone(), None),
            crate::scheduler::Submission::Duplicate { task } if task == pending
        ));

        recovered.register_agent("storage-worker", pending_caps);
        recovered.step_to(2);
        recovered.step_to(2);
        assert!(!recovered.has_pending_work());
        assert_eq!(recovered.receipt_count(), 2);
        assert_eq!(recovered.metrics().succeeded, 2);
    }

    #[test]
    fn checkpoint_preserves_ready_queue_order_without_dispatching_on_restore() {
        let (mut original, _) = fabric(FabricConfig {
            shards: 1,
            per_shard_queue_cap: 4,
            ..FabricConfig::default()
        });
        original.submit(b"first".to_vec(), capability("compute"), None);
        original.submit(b"second".to_vec(), capability("compute"), None);

        let checkpoint = original
            .checkpoint()
            .expect("queued work is checkpointable");
        let mut recovered = restore(&checkpoint);

        assert_eq!(recovered.memory_stats().queued_ready, 2);
        assert_eq!(recovered.memory_stats().retry_pending, 0);
        assert_eq!(
            recovered.checkpoint().expect("stable round trip"),
            checkpoint
        );
    }

    #[test]
    fn failed_receipt_preserves_its_reason_across_a_checkpoint() {
        let cancels = CancelState::new();
        let mut fabric = Fabric::new(
            FabricConfig {
                shards: 1,
                retry: RetryPolicy {
                    max_attempts: 1,
                    ..RetryPolicy::default()
                },
                ..FabricConfig::default()
            },
            failed_driver(cancels.clone()),
            cancels,
        );
        fabric.register_agent("worker", capability("compute"));
        let task = match fabric.submit(b"fail".to_vec(), capability("compute"), None) {
            crate::scheduler::Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        fabric.step_to(0);

        let mut recovered = restore(&fabric.checkpoint().expect("settled failure is safe"));
        let receipt = recovered.receipts().next().expect("failed receipt");
        assert_eq!(receipt.task(), task);
        assert_eq!(
            receipt.terminal(),
            &Terminal::Failed {
                reason: "worker failed".into()
            }
        );
        assert_eq!(
            recovered.checkpoint().expect("round-trip stays stable"),
            fabric.checkpoint().expect("original stays stable")
        );
    }

    #[test]
    fn cancelled_queued_work_restores_as_cancelled_without_a_worker_call() {
        let (mut original, _) = fabric(FabricConfig {
            shards: 1,
            ..FabricConfig::default()
        });
        let task = match original.submit(b"cancel me".to_vec(), capability("compute"), None) {
            crate::scheduler::Submission::Accepted { task } => task,
            other => panic!("expected acceptance, got {other:?}"),
        };
        assert!(original.cancel(task));

        let mut recovered = restore(&original.checkpoint().expect("queued cancellation is safe"));
        recovered.step_to(0);

        let receipt = recovered.receipts().next().expect("cancelled receipt");
        assert_eq!(receipt.task(), task);
        assert_eq!(receipt.terminal(), &Terminal::Cancelled);
        assert!(receipt.cancel_requested());
        assert_eq!(recovered.metrics().dispatched, 0);
    }

    struct HoldingDriver;

    impl Driver for HoldingDriver {
        fn dispatch(&mut self, _job: crate::envelope::DispatchJob) -> Enqueue {
            Enqueue::Accepted
        }

        fn poll(&mut self) -> Vec<crate::envelope::Completion> {
            Vec::new()
        }
    }

    #[test]
    fn checkpoint_refuses_an_attempt_whose_external_effect_is_not_resolved() {
        let cancels = CancelState::new();
        let mut fabric = Fabric::new(
            FabricConfig {
                shards: 1,
                ..FabricConfig::default()
            },
            Box::new(HoldingDriver),
            cancels,
        );
        fabric.register_agent("worker", capability("compute"));
        fabric.submit(b"possibly executed".to_vec(), capability("compute"), None);
        fabric.step_to(0);

        let error = fabric.checkpoint().expect_err("active work is uncertain");

        assert!(error.to_string().contains("attempt or lease is active"));
    }

    #[test]
    fn checkpoint_checksum_rejects_state_tampering_before_restore() {
        let (mut fabric, _) = fabric(FabricConfig::default());
        fabric.submit(b"unchanged".to_vec(), capability("compute"), None);
        let mut value = json::parse(&fabric.checkpoint().expect("checkpoint")).expect("json");
        let Value::Obj(root) = &mut value else {
            panic!("checkpoint object");
        };
        let state = root
            .iter_mut()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value)
            .expect("state field");
        let Value::Obj(state) = state else {
            panic!("state object");
        };
        state.push(("unexpected".into(), Value::Bool(true)));

        let error = Fabric::from_checkpoint(
            &json::to_string(&value),
            driver(CancelState::new()),
            CancelState::new(),
        )
        .err()
        .expect("tampered state must not restore");

        assert!(error.to_string().contains("state digest"));
    }

    #[test]
    fn task_payload_digest_rejects_modified_payload_even_with_recomputed_checkpoint_digest() {
        let (mut fabric, _) = fabric(FabricConfig::default());
        fabric.submit(b"unchanged".to_vec(), capability("compute"), None);
        let mut value = json::parse(&fabric.checkpoint().expect("checkpoint")).expect("json");
        let Value::Obj(root) = &mut value else {
            panic!("checkpoint object");
        };
        let state = root
            .iter_mut()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value)
            .expect("state field");
        let Value::Obj(state) = state else {
            panic!("state object");
        };
        let tasks = state
            .iter_mut()
            .find(|(key, _)| key == "tasks")
            .map(|(_, value)| value)
            .expect("task list");
        let Value::Arr(tasks) = tasks else {
            panic!("task array");
        };
        let Value::Obj(task) = &mut tasks[0] else {
            panic!("task object");
        };
        let payload = task
            .iter_mut()
            .find(|(key, _)| key == "payload_hex")
            .map(|(_, value)| value)
            .expect("payload field");
        let original_payload = payload.as_str().expect("hex payload");
        let changed_payload = format!("00{}", &original_payload[2..]);
        *payload = Value::str(changed_payload);

        let state_digest = {
            let state = root
                .iter()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| value)
                .expect("state field");
            sha256(json::to_string(state).as_bytes()).hex()
        };
        let digest = root
            .iter_mut()
            .find(|(key, _)| key == "state_digest")
            .map(|(_, value)| value)
            .expect("state digest field");
        *digest = Value::str(state_digest);

        let error = Fabric::from_checkpoint(
            &json::to_string(&value),
            driver(CancelState::new()),
            CancelState::new(),
        )
        .err()
        .expect("payload mutation must not restore");

        assert!(error
            .to_string()
            .contains("payload does not match its digest"));
    }
}
