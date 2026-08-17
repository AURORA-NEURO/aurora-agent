//! Append-only event and webhook-outbox state for the HTTP boundary.
//!
//! Events are deliberately sequence-based rather than wall-clock-based.  A consumer can resume
//! from an integer cursor, detect retention gaps, and replay the same evidence without trusting a
//! machine clock.  Webhooks are represented as signed, retryable outbox deliveries.  This crate
//! does not open arbitrary outbound sockets: an operator-owned delivery worker can poll the
//! outbox, send the signed envelope, and acknowledge it with the same idempotent cursor.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_EVENT_TYPE_BYTES: usize = 128;
pub const MAX_SUBSCRIPTION_ID_BYTES: usize = 128;
pub const MAX_ENDPOINT_BYTES: usize = 2048;
pub const MAX_SECRET_BYTES: usize = 4096;
pub const MAX_FILTERS: usize = 32;
pub const MAX_RETRY_ATTEMPTS: u32 = 10;
pub const EVENT_STATE_SCHEMA_VERSION: u64 = 5;
const LEGACY_EVENT_STATE_SCHEMA_VERSION: u64 = 1;
const EARLIEST_DURABLE_EVENT_STATE_SCHEMA_VERSION: u64 = 2;
const CONTENT_ADDRESSED_EVENT_STATE_SCHEMA_VERSION: u64 = 3;
const PREVIOUS_EVENT_STATE_SCHEMA_VERSION: u64 = 4;
pub const MAX_EVENT_STATE_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_DELIVERY_WORKER_BATCH: usize = 100;
pub const MAX_DELIVERY_ERROR_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiEvent {
    pub id: u64,
    pub event_type: String,
    pub subject: String,
    pub request_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPage {
    pub events: Vec<ApiEvent>,
    pub after: u64,
    pub next_after: u64,
    pub oldest: Option<u64>,
    pub newest: Option<u64>,
    pub gap: bool,
    pub dropped_events: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionView {
    pub id: String,
    pub endpoint: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at_sequence: u64,
    pub secret_bound: bool,
    pub rebind_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEnvelope {
    pub delivery_id: u64,
    pub subscription_id: String,
    pub attempt: u32,
    pub event: ApiEvent,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryView {
    pub delivery_id: u64,
    pub subscription_id: String,
    pub attempt: u32,
    pub state: String,
    pub last_error: Option<String>,
    pub last_error_retryable: Option<bool>,
    pub event_id: u64,
    pub event_type: String,
    pub signature: String,
    pub envelope: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryPage {
    pub deliveries: Vec<DeliveryView>,
    pub after: u64,
    pub next_after: u64,
    pub pending_count: usize,
    pub dropped_deliveries: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventMetrics {
    pub retained_events: usize,
    pub dropped_events: u64,
    pub subscriptions: usize,
    pub active_subscriptions: usize,
    pub pending_deliveries: usize,
    pub dropped_deliveries: u64,
    pub next_event_id: u64,
    pub next_delivery_id: u64,
    pub retained_delivery_attempts: usize,
    pub dropped_delivery_attempts: u64,
    pub next_attempt_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryAttempt {
    pub attempt_id: u64,
    pub delivery_id: u64,
    pub subscription_id: String,
    pub event_id: u64,
    pub event_type: String,
    pub attempt: u32,
    pub action: String,
    pub outcome: String,
    pub receiver_accepted: Option<bool>,
    pub retryable: Option<bool>,
    pub error: Option<String>,
    pub signature: String,
    #[serde(default)]
    pub receipt_id: Option<String>,
    #[serde(default)]
    pub receipt_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeliveryAttemptPage {
    pub attempts: Vec<DeliveryAttempt>,
    pub after: u64,
    pub next_after: u64,
    pub oldest: Option<u64>,
    pub newest: Option<u64>,
    pub gap: bool,
    pub dropped_attempts: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DeliveryFailure {
    pub delivery_id: u64,
    pub subscription_id: String,
    pub attempt: u32,
    pub retryable: bool,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DeliveryRunReport {
    pub attempted: usize,
    pub acknowledged: usize,
    pub retried: usize,
    pub failed: usize,
    pub exhausted: usize,
    pub blocked: usize,
    pub pending: usize,
    pub failures: Vec<DeliveryFailure>,
}

/// Network boundary supplied by an operator-owned delivery worker.
///
/// The API crate hands the transport an already-signed envelope and endpoint. Implementations
/// decide how to perform HTTP/TLS, classify retryability, and enforce their own egress policy.
pub trait DeliverySender {
    fn send(&mut self, endpoint: &str, envelope: &Value) -> Result<(), DeliverySendError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverySendError {
    pub retryable: bool,
    pub message: String,
}

impl DeliverySendError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            retryable: true,
            message: message.into(),
        }
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            retryable: false,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Subscription {
    view: SubscriptionView,
    secret: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingDelivery {
    envelope: WebhookEnvelope,
    last_error: Option<String>,
    last_error_retryable: Option<bool>,
}

pub struct EventLog {
    capacity: usize,
    delivery_capacity: usize,
    attempt_capacity: usize,
    next_event_id: u64,
    next_delivery_id: u64,
    next_attempt_id: u64,
    events: VecDeque<ApiEvent>,
    subscriptions: BTreeMap<String, Subscription>,
    deliveries: BTreeMap<u64, PendingDelivery>,
    attempts: VecDeque<DeliveryAttempt>,
    dropped_events: u64,
    dropped_deliveries: u64,
    dropped_attempts: u64,
}

impl EventLog {
    pub fn new(capacity: usize) -> Result<Self, String> {
        if capacity == 0 || capacity > 100_000 {
            return Err("event capacity must be between 1 and 100000".into());
        }
        Ok(Self {
            capacity,
            delivery_capacity: capacity.saturating_mul(8).clamp(1, 100_000),
            attempt_capacity: capacity.saturating_mul(32).clamp(1, 100_000),
            next_event_id: 1,
            next_delivery_id: 1,
            next_attempt_id: 1,
            events: VecDeque::with_capacity(capacity.min(1024)),
            subscriptions: BTreeMap::new(),
            deliveries: BTreeMap::new(),
            attempts: VecDeque::new(),
            dropped_events: 0,
            dropped_deliveries: 0,
            dropped_attempts: 0,
        })
    }

    /// Restore bounded event rows, non-secret subscription/outbox metadata, and delivery-attempt
    /// provenance from an optional checkpoint. Signing secrets are intentionally never restored;
    /// subscriptions remain paused until an operator explicitly rebinds each secret rather than
    /// silently resurrecting credentials from a JSON snapshot.
    pub fn from_checkpoint_path(
        capacity: usize,
        path: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        let Some(path) = path else {
            return Self::new(capacity);
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Self::new(capacity)
            }
            Err(error) => return Err(format!("event state snapshot could not be read: {error}")),
        };
        if bytes.len() > MAX_EVENT_STATE_FILE_BYTES {
            return Err(format!(
                "event state snapshot is {} bytes, above the {}-byte bound",
                bytes.len(),
                MAX_EVENT_STATE_FILE_BYTES
            ));
        }
        let document: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("event state snapshot is invalid JSON: {error}"))?;
        let schema_version = document
            .get("schema_version")
            .and_then(Value::as_u64)
            .ok_or_else(|| "event state snapshot has no schema_version".to_string())?;
        if schema_version != EVENT_STATE_SCHEMA_VERSION
            && schema_version != PREVIOUS_EVENT_STATE_SCHEMA_VERSION
            && schema_version != CONTENT_ADDRESSED_EVENT_STATE_SCHEMA_VERSION
            && schema_version != EARLIEST_DURABLE_EVENT_STATE_SCHEMA_VERSION
            && schema_version != LEGACY_EVENT_STATE_SCHEMA_VERSION
        {
            return Err(format!(
                "unsupported event state schema version {schema_version}; expected {LEGACY_EVENT_STATE_SCHEMA_VERSION}, {EARLIEST_DURABLE_EVENT_STATE_SCHEMA_VERSION}, {CONTENT_ADDRESSED_EVENT_STATE_SCHEMA_VERSION}, {PREVIOUS_EVENT_STATE_SCHEMA_VERSION}, or {EVENT_STATE_SCHEMA_VERSION}"
            ));
        }
        if schema_version == EVENT_STATE_SCHEMA_VERSION
            || schema_version == PREVIOUS_EVENT_STATE_SCHEMA_VERSION
            || schema_version == CONTENT_ADDRESSED_EVENT_STATE_SCHEMA_VERSION
        {
            verify_checkpoint_digest(&document)?;
        }
        let mut log = Self::new(capacity)?;
        log.next_event_id = document
            .get("next_event_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "event state snapshot has no next_event_id".to_string())?;
        log.dropped_events = document
            .get("dropped_events")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let events = document
            .get("events")
            .and_then(Value::as_array)
            .ok_or_else(|| "event state snapshot has no events array".to_string())?;
        let retained = events
            .iter()
            .rev()
            .take(capacity)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>();
        if retained.len() < events.len() {
            log.dropped_events = log
                .dropped_events
                .saturating_add((events.len() - retained.len()) as u64);
        }
        let mut previous_id = None;
        for value in retained {
            let event: ApiEvent = serde_json::from_value(value).map_err(|error| {
                format!("event state snapshot contains an invalid event: {error}")
            })?;
            validate_token(&event.event_type, MAX_EVENT_TYPE_BYTES, "event_type")?;
            validate_token(&event.subject, 256, "subject")?;
            validate_token(&event.request_id, 256, "request_id")?;
            if event.id == 0 || previous_id.is_some_and(|previous| event.id <= previous) {
                return Err("event state snapshot event ids must be strictly increasing".into());
            }
            previous_id = Some(event.id);
            log.events.push_back(event);
        }
        if let Some(last_id) = previous_id {
            log.next_event_id = log.next_event_id.max(last_id.saturating_add(1));
        }
        if log.next_event_id == 0 {
            return Err("event state snapshot next_event_id must not overflow".into());
        }
        if schema_version == EARLIEST_DURABLE_EVENT_STATE_SCHEMA_VERSION
            || schema_version == CONTENT_ADDRESSED_EVENT_STATE_SCHEMA_VERSION
            || schema_version == PREVIOUS_EVENT_STATE_SCHEMA_VERSION
            || schema_version == EVENT_STATE_SCHEMA_VERSION
        {
            if document
                .get("subscriptions_durable")
                .and_then(Value::as_bool)
                != Some(true)
                || document
                    .get("webhook_deliveries_durable")
                    .and_then(Value::as_bool)
                    != Some(true)
            {
                return Err(
                    "event state snapshot must explicitly declare durable subscription and outbox metadata"
                        .into(),
                );
            }
            if document.get("secrets_persisted").and_then(Value::as_bool) != Some(false) {
                return Err(
                    "event state snapshot must explicitly declare secrets_persisted=false".into(),
                );
            }
            log.next_delivery_id = document
                .get("next_delivery_id")
                .and_then(Value::as_u64)
                .ok_or_else(|| "event state snapshot has no next_delivery_id".to_string())?;
            log.dropped_deliveries = document
                .get("dropped_deliveries")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if log.next_delivery_id == 0 {
                return Err("event state snapshot next_delivery_id must not overflow".into());
            }
            let subscriptions = document
                .get("subscriptions")
                .and_then(Value::as_array)
                .ok_or_else(|| "event state snapshot has no subscriptions array".to_string())?;
            if subscriptions.len() > 256 {
                return Err("event state snapshot contains too many subscriptions".into());
            }
            for value in subscriptions {
                let mut view: SubscriptionView =
                    serde_json::from_value(value.clone()).map_err(|error| {
                        format!("event state snapshot contains an invalid subscription: {error}")
                    })?;
                validate_subscription_view(&view)?;
                if log.subscriptions.contains_key(&view.id) {
                    return Err(format!(
                        "event state snapshot contains duplicate subscription {:?}",
                        view.id
                    ));
                }
                // The persisted endpoint and filters remain inspectable, but the subscription is
                // paused until the operator explicitly supplies its secret again.
                view.active = false;
                view.secret_bound = false;
                view.rebind_required = true;
                log.subscriptions.insert(
                    view.id.clone(),
                    Subscription {
                        view,
                        secret: Vec::new(),
                    },
                );
            }
            let deliveries = document
                .get("deliveries")
                .and_then(Value::as_array)
                .ok_or_else(|| "event state snapshot has no deliveries array".to_string())?;
            if deliveries.len() > log.delivery_capacity {
                return Err(format!(
                    "event state snapshot contains {} deliveries above the {}-row bound",
                    deliveries.len(),
                    log.delivery_capacity
                ));
            }
            for value in deliveries {
                let pending: PendingDelivery =
                    serde_json::from_value(value.clone()).map_err(|error| {
                        format!("event state snapshot contains an invalid delivery: {error}")
                    })?;
                validate_pending_delivery(&pending)?;
                let subscription_id = &pending.envelope.subscription_id;
                if !log.subscriptions.contains_key(subscription_id) {
                    return Err(format!(
                        "event state snapshot delivery references unknown subscription {subscription_id:?}"
                    ));
                }
                if log
                    .deliveries
                    .insert(pending.envelope.delivery_id, pending)
                    .is_some()
                {
                    return Err("event state snapshot contains duplicate delivery ids".into());
                }
            }
            if let Some(last_id) = log.deliveries.keys().next_back().copied() {
                log.next_delivery_id = log.next_delivery_id.max(last_id.saturating_add(1));
            }
            if log.next_delivery_id == 0 {
                return Err("event state snapshot next_delivery_id must not overflow".into());
            }
            if schema_version == EVENT_STATE_SCHEMA_VERSION
                || schema_version == PREVIOUS_EVENT_STATE_SCHEMA_VERSION
            {
                if document
                    .get("delivery_attempts_durable")
                    .and_then(Value::as_bool)
                    != Some(true)
                {
                    return Err(
                        "event state snapshot must explicitly declare durable delivery attempts"
                            .into(),
                    );
                }
                if schema_version == EVENT_STATE_SCHEMA_VERSION
                    && document
                        .get("delivery_receipt_metadata_durable")
                        .and_then(Value::as_bool)
                        != Some(true)
                {
                    return Err(
                        "event state snapshot must explicitly declare durable delivery receipt metadata"
                            .into(),
                    );
                }
                log.next_attempt_id = document
                    .get("next_attempt_id")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "event state snapshot has no next_attempt_id".to_string())?;
                log.dropped_attempts = document
                    .get("dropped_attempts")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if log.next_attempt_id == 0 {
                    return Err("event state snapshot next_attempt_id must not overflow".into());
                }
                let attempts = document
                    .get("delivery_attempts")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        "event state snapshot has no delivery_attempts array".to_string()
                    })?;
                if attempts.len() > log.attempt_capacity {
                    return Err(format!(
                        "event state snapshot contains {} delivery attempts above the {}-row bound",
                        attempts.len(),
                        log.attempt_capacity
                    ));
                }
                for value in attempts {
                    let attempt: DeliveryAttempt =
                        serde_json::from_value(value.clone()).map_err(|error| {
                            format!(
                                "event state snapshot contains an invalid delivery attempt: {error}"
                            )
                        })?;
                    validate_delivery_attempt(&attempt)?;
                    if log
                        .attempts
                        .iter()
                        .any(|existing| existing.attempt_id == attempt.attempt_id)
                    {
                        return Err(
                            "event state snapshot contains duplicate delivery attempt ids".into(),
                        );
                    }
                    log.attempts.push_back(attempt);
                }
                if let Some(last_id) = log.attempts.back().map(|attempt| attempt.attempt_id) {
                    log.next_attempt_id = log.next_attempt_id.max(last_id.saturating_add(1));
                }
                if log.next_attempt_id == 0 {
                    return Err("event state snapshot next_attempt_id must not overflow".into());
                }
            }
        }
        Ok(log)
    }

    /// Atomically write a bounded checkpoint for events plus non-secret subscription/outbox state.
    /// Signing secrets never enter the snapshot; restored subscriptions are paused until an
    /// operator explicitly rebinds each secret in memory.
    pub fn checkpoint_to_path(&self, path: &std::path::Path) -> Result<usize, String> {
        let mut events = self.events.iter().cloned().collect::<Vec<_>>();
        let mut dropped_events = self.dropped_events;
        let mut dropped_attempts = self.dropped_attempts;
        let subscriptions = self
            .subscriptions
            .values()
            .map(|subscription| subscription.view.clone())
            .collect::<Vec<_>>();
        let deliveries = self.deliveries.values().cloned().collect::<Vec<_>>();
        let mut attempts = self.attempts.iter().cloned().collect::<Vec<_>>();
        loop {
            let mut document = json!({
                "schema_version": EVENT_STATE_SCHEMA_VERSION,
                "next_event_id": self.next_event_id,
                "next_delivery_id": self.next_delivery_id,
                "next_attempt_id": self.next_attempt_id,
                "dropped_events": dropped_events,
                "dropped_deliveries": self.dropped_deliveries,
                "dropped_attempts": dropped_attempts,
                "events": events,
                "subscriptions": subscriptions,
                "deliveries": deliveries,
                "delivery_attempts": attempts,
                "subscriptions_durable": true,
                "webhook_deliveries_durable": true,
                "delivery_attempts_durable": true,
                "delivery_receipt_metadata_durable": true,
                "secrets_persisted": false,
                "recovery_policy": "restored subscriptions are paused until explicit secret rebind; signed outbox rows remain inspectable and are re-signed only after rebind",
            });
            let state_digest = checkpoint_digest(&document)?;
            document
                .as_object_mut()
                .expect("event checkpoint document is an object")
                .insert("state_digest".into(), Value::String(state_digest));
            let bytes = serde_json::to_vec_pretty(&document)
                .map_err(|error| format!("event state could not be serialized: {error}"))?;
            if bytes.len() <= MAX_EVENT_STATE_FILE_BYTES {
                if let Some(parent) = path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    std::fs::create_dir_all(parent).map_err(|error| {
                        format!("event state directory could not be created: {error}")
                    })?;
                }
                let filename = path
                    .file_name()
                    .ok_or_else(|| "event_state_path must name a file".to_string())?
                    .to_string_lossy();
                let temporary = path.with_file_name(format!(".{filename}.tmp"));
                std::fs::write(&temporary, &bytes).map_err(|error| {
                    format!("event state temporary file could not be written: {error}")
                })?;
                if let Err(first_error) = std::fs::rename(&temporary, path) {
                    #[cfg(windows)]
                    {
                        let _ = std::fs::remove_file(path);
                        std::fs::rename(&temporary, path).map_err(|second_error| {
                            format!(
                                "event state could not replace the previous snapshot ({first_error}; retry: {second_error})"
                            )
                        })?;
                    }
                    #[cfg(not(windows))]
                    {
                        return Err(format!(
                            "event state snapshot could not be installed: {first_error}"
                        ));
                    }
                }
                return Ok(bytes.len());
            }
            if events.is_empty() {
                if !attempts.is_empty() {
                    attempts.remove(0);
                    dropped_attempts = dropped_attempts.saturating_add(1);
                    continue;
                }
                return Err(format!(
                    "event state snapshot cannot fit within the {}-byte bound",
                    MAX_EVENT_STATE_FILE_BYTES
                ));
            }
            events.remove(0);
            dropped_events = dropped_events.saturating_add(1);
        }
    }

    pub fn emit(
        &mut self,
        event_type: &str,
        subject: &str,
        request_id: &str,
        payload: Value,
    ) -> Result<ApiEvent, String> {
        validate_token(event_type, MAX_EVENT_TYPE_BYTES, "event_type")?;
        validate_token(subject, 256, "subject")?;
        validate_token(request_id, 256, "request_id")?;
        let event = ApiEvent {
            id: self.next_event_id,
            event_type: event_type.to_string(),
            subject: subject.to_string(),
            request_id: request_id.to_string(),
            payload,
        };
        self.next_event_id = self.next_event_id.saturating_add(1);
        if self.events.len() == self.capacity {
            self.events.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
        self.events.push_back(event.clone());

        let subscriptions: Vec<(String, Vec<u8>)> = self
            .subscriptions
            .values()
            .filter(|subscription| {
                subscription.view.active
                    && subscription.view.secret_bound
                    && subscription
                        .view
                        .events
                        .iter()
                        .any(|filter| filter == "*" || filter == &event.event_type)
            })
            .map(|subscription| (subscription.view.id.clone(), subscription.secret.clone()))
            .collect();
        for (subscription_id, secret) in subscriptions {
            self.enqueue_delivery(subscription_id, secret, event.clone(), 1)?;
        }
        Ok(event)
    }

    pub fn register_subscription(
        &mut self,
        id: Option<&str>,
        endpoint: &str,
        events: Option<&[String]>,
        secret: &str,
    ) -> Result<SubscriptionView, String> {
        validate_endpoint(endpoint)?;
        validate_secret(secret)?;
        let subscription_id = id
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("sub-{}", self.next_event_id));
        validate_token(
            &subscription_id,
            MAX_SUBSCRIPTION_ID_BYTES,
            "subscription id",
        )?;
        if self.subscriptions.contains_key(&subscription_id) {
            return Err(format!("subscription {subscription_id:?} already exists"));
        }
        if self.subscriptions.len() >= 256 {
            return Err("subscription limit of 256 has been reached".into());
        }
        let filters = events
            .map(|values| values.to_vec())
            .unwrap_or_else(|| vec!["*".into()]);
        if filters.is_empty() || filters.len() > MAX_FILTERS {
            return Err(format!(
                "events must contain between 1 and {MAX_FILTERS} filters"
            ));
        }
        for filter in &filters {
            validate_token(filter, MAX_EVENT_TYPE_BYTES, "event filter")?;
        }
        let view = SubscriptionView {
            id: subscription_id.clone(),
            endpoint: endpoint.to_string(),
            events: filters,
            active: true,
            created_at_sequence: self.next_event_id,
            secret_bound: true,
            rebind_required: false,
        };
        self.subscriptions.insert(
            subscription_id,
            Subscription {
                view: view.clone(),
                secret: secret.as_bytes().to_vec(),
            },
        );
        Ok(view)
    }

    pub fn subscriptions(&self) -> Vec<SubscriptionView> {
        self.subscriptions
            .values()
            .map(|subscription| subscription.view.clone())
            .collect()
    }

    /// Rebind a restored subscription's signing secret in memory.
    ///
    /// The secret is never returned or persisted. Existing pending envelopes are re-signed with
    /// the newly supplied secret before the subscription becomes active, so a restored outbox
    /// cannot accidentally send an envelope authenticated by an unavailable pre-restart secret.
    pub fn rebind_subscription(
        &mut self,
        id: &str,
        secret: &str,
    ) -> Result<(SubscriptionView, usize), String> {
        validate_secret(secret)?;
        let subscription = self
            .subscriptions
            .get_mut(id)
            .ok_or_else(|| format!("unknown subscription {id:?}"))?;
        subscription.secret = secret.as_bytes().to_vec();
        subscription.view.secret_bound = true;
        subscription.view.rebind_required = false;
        subscription.view.active = true;
        let mut resigned = 0;
        for delivery in self.deliveries.values_mut() {
            if delivery.envelope.subscription_id == id {
                delivery.envelope.signature =
                    sign_envelope(&subscription.secret, &delivery.envelope);
                resigned += 1;
            }
        }
        Ok((subscription.view.clone(), resigned))
    }

    pub fn remove_subscription(&mut self, id: &str) -> Result<bool, String> {
        if self.subscriptions.remove(id).is_none() {
            return Ok(false);
        }
        self.deliveries
            .retain(|_, delivery| delivery.envelope.subscription_id != id);
        Ok(true)
    }

    pub fn events(&self, after: u64, limit: usize) -> Result<EventPage, String> {
        self.events_matching(after, limit, |_| true)
    }

    /// Return only retained tool events whose capability-route-review response carries the
    /// requested content-addressed review id. The event cursor and retention evidence retain the
    /// same semantics as an unfiltered page; no separate mutable review index is introduced.
    pub fn events_for_review(
        &self,
        after: u64,
        limit: usize,
        review_id: &str,
    ) -> Result<EventPage, String> {
        validate_review_id(review_id)?;
        self.events_matching(after, limit, |event| {
            event.subject == "capability_route_review"
                && event_matches_review_id(&event.payload, review_id)
        })
    }

    /// Return retained operations gate-review events for one content-addressed review ID.
    /// Review records live in the same durable event log as tool evidence so replay exposes the
    /// exact cursor, retention gap, and checkpoint semantics used by the rest of the API.
    pub fn events_for_operations_gate_review(
        &self,
        after: u64,
        limit: usize,
        review_id: &str,
    ) -> Result<EventPage, String> {
        validate_review_id(review_id)?;
        self.events_matching(after, limit, |event| {
            event.subject == "operations_gate_review"
                && event
                    .payload
                    .get("review_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == review_id)
        })
    }

    /// Return retained operations gate-review events with an optional exact content-addressed
    /// filter. The unfiltered form is bounded by the same event cursor and retention window.
    pub fn operations_gate_reviews(
        &self,
        after: u64,
        limit: usize,
        review_id: Option<&str>,
    ) -> Result<EventPage, String> {
        if let Some(review_id) = review_id {
            return self.events_for_operations_gate_review(after, limit, review_id);
        }
        self.events_matching(after, limit, |event| {
            event.subject == "operations_gate_review"
        })
    }

    /// Return retained delivery-receipt tool events for one caller-supplied receipt identifier.
    ///
    /// The event log deliberately keeps this as a projection query rather than a second mutable
    /// index. A receipt event may retain the complete response or only its bounded projection when
    /// the response is large; both forms carry the same exact identifier and remain cursor-bound.
    pub fn events_for_receipt(
        &self,
        after: u64,
        limit: usize,
        receipt_id: &str,
    ) -> Result<EventPage, String> {
        validate_receipt_id(receipt_id)?;
        self.events_matching(after, limit, |event| {
            matches!(
                event.subject.as_str(),
                "developer_delivery_receipt" | "developer_delivery_receipt_verify"
            ) && event_matches_receipt_id(&event.payload, receipt_id)
        })
    }

    fn events_matching<F>(&self, after: u64, limit: usize, matches: F) -> Result<EventPage, String>
    where
        F: Fn(&ApiEvent) -> bool,
    {
        let limit = checked_limit(limit)?;
        let events = self
            .events
            .iter()
            .filter(|event| event.id > after && matches(event))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let oldest = self.events.front().map(|event| event.id);
        let newest = self.events.back().map(|event| event.id);
        let next_after = events.last().map_or(after, |event| event.id);
        let gap = oldest.is_some_and(|oldest| after.saturating_add(1) < oldest);
        Ok(EventPage {
            events,
            after,
            next_after,
            oldest,
            newest,
            gap,
            dropped_events: self.dropped_events,
        })
    }

    pub fn deliveries(
        &self,
        subscription_id: &str,
        after: u64,
        limit: usize,
    ) -> Result<DeliveryPage, String> {
        self.require_subscription(subscription_id)?;
        let limit = checked_limit(limit)?;
        let deliveries = self
            .deliveries
            .values()
            .filter(|delivery| {
                delivery.envelope.subscription_id == subscription_id
                    && delivery.envelope.delivery_id > after
            })
            .take(limit)
            .map(|delivery| {
                let secret_bound = self
                    .subscriptions
                    .get(&delivery.envelope.subscription_id)
                    .is_some_and(|subscription| subscription.view.secret_bound);
                delivery_view(delivery, secret_bound)
            })
            .collect::<Vec<_>>();
        let next_after = deliveries
            .last()
            .map_or(after, |delivery| delivery.delivery_id);
        Ok(DeliveryPage {
            deliveries,
            after,
            next_after,
            pending_count: self
                .deliveries
                .values()
                .filter(|delivery| delivery.envelope.subscription_id == subscription_id)
                .count(),
            dropped_deliveries: self.dropped_deliveries,
        })
    }

    pub fn delivery_attempts(
        &self,
        subscription_id: &str,
        after: u64,
        limit: usize,
    ) -> Result<DeliveryAttemptPage, String> {
        self.require_subscription(subscription_id)?;
        let limit = checked_limit(limit)?;
        let matching = self
            .attempts
            .iter()
            .filter(|attempt| attempt.subscription_id == subscription_id)
            .collect::<Vec<_>>();
        let oldest = matching.first().map(|attempt| attempt.attempt_id);
        let newest = matching.last().map(|attempt| attempt.attempt_id);
        let attempts = matching
            .into_iter()
            .filter(|attempt| attempt.attempt_id > after)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_after = attempts.last().map_or(after, |attempt| attempt.attempt_id);
        let gap = oldest.is_some_and(|oldest| after.saturating_add(1) < oldest);
        Ok(DeliveryAttemptPage {
            attempts,
            after,
            next_after,
            oldest,
            newest,
            gap,
            dropped_attempts: self.dropped_attempts,
        })
    }

    /// Return durable delivery-attempt provenance correlated to one content-addressed developer
    /// delivery receipt. The attempt cursor remains global so a caller can resume one stable
    /// sequence even when multiple subscriptions delivered the same receipt-bearing event.
    pub fn delivery_attempts_for_receipt(
        &self,
        receipt_id: &str,
        after: u64,
        limit: usize,
    ) -> Result<DeliveryAttemptPage, String> {
        validate_receipt_id(receipt_id)?;
        let limit = checked_limit(limit)?;
        let matching = self
            .attempts
            .iter()
            .filter(|attempt| attempt.receipt_id.as_deref() == Some(receipt_id))
            .collect::<Vec<_>>();
        let oldest = matching.first().map(|attempt| attempt.attempt_id);
        let newest = matching.last().map(|attempt| attempt.attempt_id);
        let attempts = matching
            .into_iter()
            .filter(|attempt| attempt.attempt_id > after)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_after = attempts.last().map_or(after, |attempt| attempt.attempt_id);
        let gap = oldest.is_some_and(|oldest| after.saturating_add(1) < oldest);
        Ok(DeliveryAttemptPage {
            attempts,
            after,
            next_after,
            oldest,
            newest,
            gap,
            dropped_attempts: self.dropped_attempts,
        })
    }

    pub fn acknowledge(&mut self, subscription_id: &str, ids: &[u64]) -> Result<usize, String> {
        self.require_subscription(subscription_id)?;
        if ids.len() > 1000 {
            return Err("a single acknowledgement may contain at most 1000 ids".into());
        }
        let mut acknowledged = 0;
        for id in ids {
            let Some(delivery) = self.deliveries.get(id).cloned() else {
                continue;
            };
            if delivery.envelope.subscription_id != subscription_id {
                continue;
            }
            self.record_attempt(
                &delivery.envelope,
                "acknowledge",
                "acknowledged",
                None,
                None,
                None,
            )?;
            if self.deliveries.remove(id).is_some() {
                acknowledged += 1;
            }
        }
        Ok(acknowledged)
    }

    pub fn retry(
        &mut self,
        subscription_id: &str,
        ids: &[u64],
    ) -> Result<Vec<DeliveryView>, String> {
        self.require_subscription(subscription_id)?;
        if ids.len() > 1000 {
            return Err("a single retry request may contain at most 1000 ids".into());
        }
        let secret = self.bound_secret(subscription_id)?;
        let mut retried = Vec::new();
        for id in ids {
            let Some(delivery) = self.deliveries.get_mut(id) else {
                continue;
            };
            if delivery.envelope.subscription_id != subscription_id
                || delivery.envelope.attempt >= MAX_RETRY_ATTEMPTS
            {
                continue;
            }
            delivery.envelope.attempt += 1;
            delivery.envelope.signature = sign_envelope(&secret, &delivery.envelope);
            let envelope = delivery.envelope.clone();
            let view = delivery_view(delivery, true);
            let previous_error = delivery.last_error.clone();
            self.record_attempt(&envelope, "retry", "scheduled", None, None, previous_error)?;
            retried.push(view);
        }
        Ok(retried)
    }

    /// Reset selected deliveries for an operator-approved replay.
    ///
    /// Replay keeps the delivery ID stable for receiver-side idempotency, resets the attempt
    /// budget to one, re-signs the envelope, and clears the previous transport failure. It never
    /// acknowledges the row or creates an unbounded copy in the outbox.
    pub fn replay(
        &mut self,
        subscription_id: &str,
        ids: &[u64],
    ) -> Result<Vec<DeliveryView>, String> {
        self.require_subscription(subscription_id)?;
        if ids.len() > 1000 {
            return Err("a single replay request may contain at most 1000 ids".into());
        }
        let secret = self.bound_secret(subscription_id)?;
        let mut replayed = Vec::new();
        for id in ids {
            let Some(delivery) = self.deliveries.get_mut(id) else {
                continue;
            };
            if delivery.envelope.subscription_id != subscription_id {
                continue;
            }
            delivery.envelope.attempt = 1;
            delivery.last_error = None;
            delivery.last_error_retryable = None;
            delivery.envelope.signature = sign_envelope(&secret, &delivery.envelope);
            let envelope = delivery.envelope.clone();
            let view = delivery_view(delivery, true);
            self.record_attempt(&envelope, "replay", "scheduled", None, None, None)?;
            replayed.push(view);
        }
        Ok(replayed)
    }

    pub fn metrics(&self) -> EventMetrics {
        EventMetrics {
            retained_events: self.events.len(),
            dropped_events: self.dropped_events,
            subscriptions: self.subscriptions.len(),
            active_subscriptions: self
                .subscriptions
                .values()
                .filter(|subscription| subscription.view.active && subscription.view.secret_bound)
                .count(),
            pending_deliveries: self.deliveries.len(),
            dropped_deliveries: self.dropped_deliveries,
            next_event_id: self.next_event_id,
            next_delivery_id: self.next_delivery_id,
            retained_delivery_attempts: self.attempts.len(),
            dropped_delivery_attempts: self.dropped_attempts,
            next_attempt_id: self.next_attempt_id,
        }
    }

    /// Execute one bounded delivery cycle through a caller-owned transport.
    ///
    /// Successful sends are acknowledged idempotently. Retryable failures advance the signed
    /// attempt and remain pending; permanent failures and exhausted attempts remain pending for
    /// operator inspection rather than being deleted as if delivery succeeded.
    pub fn deliver_once<S: DeliverySender>(
        &mut self,
        sender: &mut S,
        max_batch: usize,
    ) -> Result<DeliveryRunReport, String> {
        if !(1..=1000).contains(&max_batch) {
            return Err("delivery worker batch must be between 1 and 1000".into());
        }
        let subscriptions = self.subscriptions();
        let mut remaining = max_batch;
        let mut report = DeliveryRunReport {
            attempted: 0,
            acknowledged: 0,
            retried: 0,
            failed: 0,
            exhausted: 0,
            blocked: 0,
            pending: 0,
            failures: Vec::new(),
        };
        for subscription in subscriptions {
            if remaining == 0 {
                break;
            }
            let page = self.deliveries(&subscription.id, 0, remaining)?;
            if !subscription.secret_bound {
                report.blocked += page.deliveries.len();
                for delivery in &page.deliveries {
                    self.record_attempt_from_view(
                        delivery,
                        "send",
                        "secret_rebind_required",
                        None,
                        None,
                        Some("subscription requires an explicit secret rebind".into()),
                    )?;
                }
                remaining = remaining.saturating_sub(page.deliveries.len());
                continue;
            }
            for delivery in page.deliveries {
                if remaining == 0 {
                    break;
                }
                remaining -= 1;
                report.attempted += 1;
                match sender.send(&subscription.endpoint, &delivery.envelope) {
                    Ok(()) => {
                        self.record_attempt_from_view(
                            &delivery,
                            "send",
                            "accepted",
                            Some(true),
                            None,
                            None,
                        )?;
                        self.deliveries.remove(&delivery.delivery_id);
                        report.acknowledged += 1;
                    }
                    Err(error) => {
                        let failure = DeliveryFailure {
                            delivery_id: delivery.delivery_id,
                            subscription_id: subscription.id.clone(),
                            attempt: delivery.attempt,
                            retryable: error.retryable,
                            error: bounded_error(error.message),
                        };
                        let outcome = if failure.retryable && delivery.attempt < MAX_RETRY_ATTEMPTS
                        {
                            "retryable_failure"
                        } else if failure.retryable {
                            "exhausted"
                        } else {
                            "permanent_failure"
                        };
                        self.record_attempt_from_view(
                            &delivery,
                            "send",
                            outcome,
                            Some(false),
                            Some(failure.retryable),
                            Some(failure.error.clone()),
                        )?;
                        if let Some(pending) = self.deliveries.get_mut(&delivery.delivery_id) {
                            pending.last_error = Some(failure.error.clone());
                            pending.last_error_retryable = Some(failure.retryable);
                        }
                        if failure.retryable && delivery.attempt < MAX_RETRY_ATTEMPTS {
                            self.retry(&subscription.id, &[delivery.delivery_id])?;
                            report.retried += 1;
                        } else if failure.retryable {
                            report.exhausted += 1;
                        } else {
                            report.failed += 1;
                        }
                        report.failures.push(failure);
                    }
                }
            }
        }
        report.pending = self.deliveries.len();
        Ok(report)
    }

    pub fn sse(&self, page: &EventPage) -> Vec<u8> {
        let mut output = String::new();
        for event in &page.events {
            let data = serde_json::to_string(event).unwrap_or_else(|_| "{}".into());
            output.push_str(&format!(
                "id: {}\nevent: {}\ndata: {data}\n\n",
                event.id, event.event_type
            ));
        }
        if page.gap {
            output.push_str(&format!(
                "event: cursor_gap\ndata: {}\n\n",
                json!({
                    "after": page.after,
                    "oldest": page.oldest,
                    "dropped_events": page.dropped_events,
                })
            ));
        }
        output.into_bytes()
    }

    fn enqueue_delivery(
        &mut self,
        subscription_id: String,
        secret: Vec<u8>,
        event: ApiEvent,
        attempt: u32,
    ) -> Result<(), String> {
        let delivery_id = self.next_delivery_id;
        self.next_delivery_id = self.next_delivery_id.saturating_add(1);
        let mut envelope = WebhookEnvelope {
            delivery_id,
            subscription_id,
            attempt,
            event,
            signature: String::new(),
        };
        envelope.signature = sign_envelope(&secret, &envelope);
        if self.deliveries.len() >= self.delivery_capacity {
            if let Some(first) = self.deliveries.keys().next().copied() {
                self.deliveries.remove(&first);
                self.dropped_deliveries = self.dropped_deliveries.saturating_add(1);
            }
        }
        self.deliveries.insert(
            delivery_id,
            PendingDelivery {
                envelope: envelope.clone(),
                last_error: None,
                last_error_retryable: None,
            },
        );
        self.record_attempt(&envelope, "enqueue", "pending", None, None, None)?;
        Ok(())
    }

    fn record_attempt_from_view(
        &mut self,
        delivery: &DeliveryView,
        action: &str,
        outcome: &str,
        receiver_accepted: Option<bool>,
        retryable: Option<bool>,
        error: Option<String>,
    ) -> Result<(), String> {
        let envelope = serde_json::from_value::<WebhookEnvelope>(delivery.envelope.clone())
            .unwrap_or_else(|_| WebhookEnvelope {
                delivery_id: delivery.delivery_id,
                subscription_id: delivery.subscription_id.clone(),
                attempt: delivery.attempt,
                event: ApiEvent {
                    id: delivery.event_id,
                    event_type: delivery.event_type.clone(),
                    subject: String::new(),
                    request_id: String::new(),
                    payload: Value::Null,
                },
                signature: delivery.signature.clone(),
            });
        self.record_attempt(
            &envelope,
            action,
            outcome,
            receiver_accepted,
            retryable,
            error,
        )
    }

    fn record_attempt(
        &mut self,
        envelope: &WebhookEnvelope,
        action: &str,
        outcome: &str,
        receiver_accepted: Option<bool>,
        retryable: Option<bool>,
        error: Option<String>,
    ) -> Result<(), String> {
        validate_token(action, 64, "delivery attempt action")?;
        validate_token(outcome, 64, "delivery attempt outcome")?;
        if envelope.delivery_id == 0 || envelope.event.id == 0 {
            return Err("delivery attempt ids must be positive".into());
        }
        let (receipt_id, receipt_digest) = delivery_receipt_metadata(&envelope.event.payload)?;
        let bounded_error = error.map(bounded_error);
        let attempt = DeliveryAttempt {
            attempt_id: self.next_attempt_id,
            delivery_id: envelope.delivery_id,
            subscription_id: envelope.subscription_id.clone(),
            event_id: envelope.event.id,
            event_type: envelope.event.event_type.clone(),
            attempt: envelope.attempt,
            action: action.to_string(),
            outcome: outcome.to_string(),
            receiver_accepted,
            retryable,
            error: bounded_error,
            signature: envelope.signature.clone(),
            receipt_id,
            receipt_digest,
        };
        self.next_attempt_id = self.next_attempt_id.saturating_add(1);
        if self.attempts.len() >= self.attempt_capacity {
            self.attempts.pop_front();
            self.dropped_attempts = self.dropped_attempts.saturating_add(1);
        }
        self.attempts.push_back(attempt);
        Ok(())
    }

    fn require_subscription(&self, id: &str) -> Result<(), String> {
        if self.subscriptions.contains_key(id) {
            Ok(())
        } else {
            Err(format!("unknown subscription {id:?}"))
        }
    }

    fn bound_secret(&self, id: &str) -> Result<Vec<u8>, String> {
        let subscription = self
            .subscriptions
            .get(id)
            .ok_or_else(|| format!("unknown subscription {id:?}"))?;
        if !subscription.view.secret_bound || subscription.secret.is_empty() {
            return Err(format!(
                "subscription {id:?} requires an explicit secret rebind before retry or replay"
            ));
        }
        Ok(subscription.secret.clone())
    }
}

fn delivery_view(delivery: &PendingDelivery, secret_bound: bool) -> DeliveryView {
    DeliveryView {
        delivery_id: delivery.envelope.delivery_id,
        subscription_id: delivery.envelope.subscription_id.clone(),
        attempt: delivery.envelope.attempt,
        state: delivery_state(delivery, secret_bound),
        last_error: delivery.last_error.clone(),
        last_error_retryable: delivery.last_error_retryable,
        event_id: delivery.envelope.event.id,
        event_type: delivery.envelope.event.event_type.clone(),
        signature: delivery.envelope.signature.clone(),
        envelope: serde_json::to_value(&delivery.envelope).unwrap_or_else(|_| json!({})),
    }
}

fn delivery_state(delivery: &PendingDelivery, secret_bound: bool) -> String {
    if !secret_bound {
        return "secret_rebind_required".into();
    }
    match delivery.last_error_retryable {
        None => "pending".into(),
        Some(false) => "failed".into(),
        Some(true) if delivery.envelope.attempt >= MAX_RETRY_ATTEMPTS => "exhausted".into(),
        Some(true) => "retryable".into(),
    }
}

fn bounded_error(message: String) -> String {
    let sanitized = message
        .chars()
        .map(|character| {
            if character.is_control() {
                '�'
            } else {
                character
            }
        })
        .collect::<String>();
    if sanitized.len() <= MAX_DELIVERY_ERROR_BYTES {
        return sanitized;
    }
    let prefix_limit = MAX_DELIVERY_ERROR_BYTES.saturating_sub(16);
    let mut bounded = String::new();
    for character in sanitized.chars() {
        if bounded.len().saturating_add(character.len_utf8()) > prefix_limit {
            break;
        }
        bounded.push(character);
    }
    bounded.push_str("… [truncated]");
    bounded
}

fn sign_envelope(secret: &[u8], envelope: &WebhookEnvelope) -> String {
    let mut unsigned = envelope.clone();
    unsigned.signature.clear();
    let message = serde_json::to_vec(&unsigned).unwrap_or_default();
    format!("sha256={}", hex_digest(&hmac_sha256(secret, &message)))
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut normalized = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        normalized[..digest.len()].copy_from_slice(&digest);
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner = [0u8; BLOCK];
    let mut outer = [0u8; BLOCK];
    for index in 0..BLOCK {
        inner[index] = normalized[index] ^ 0x36;
        outer[index] = normalized[index] ^ 0x5c;
    }
    let mut inner_hasher = Sha256::new();
    inner_hasher.update(inner);
    inner_hasher.update(message);
    let inner_digest = inner_hasher.finalize();
    let mut outer_hasher = Sha256::new();
    outer_hasher.update(outer);
    outer_hasher.update(inner_digest);
    let digest = outer_hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&digest);
    output
}

fn checkpoint_digest(document: &Value) -> Result<String, String> {
    let bytes = serde_json::to_vec(document)
        .map_err(|error| format!("event state digest could not be serialized: {error}"))?;
    Ok(hex_digest(&Sha256::digest(&bytes)))
}

fn verify_checkpoint_digest(document: &Value) -> Result<(), String> {
    let stored = document
        .get("state_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| "content-addressed event state schema requires state_digest".to_string())?;
    if stored.len() != 64
        || !stored
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("event state state_digest must be 64 lowercase hexadecimal characters".into());
    }
    let mut unsigned = document.clone();
    unsigned
        .as_object_mut()
        .ok_or_else(|| "event state snapshot must be a JSON object".to_string())?
        .remove("state_digest");
    let computed = checkpoint_digest(&unsigned)?;
    if computed != stored {
        return Err(format!(
            "event state state_digest mismatch: expected {stored}, computed {computed}"
        ));
    }
    Ok(())
}

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn validate_subscription_view(view: &SubscriptionView) -> Result<(), String> {
    validate_token(&view.id, MAX_SUBSCRIPTION_ID_BYTES, "subscription id")?;
    validate_endpoint(&view.endpoint)?;
    if view.events.is_empty() || view.events.len() > MAX_FILTERS {
        return Err(format!(
            "subscription events must contain between 1 and {MAX_FILTERS} filters"
        ));
    }
    for filter in &view.events {
        validate_token(filter, MAX_EVENT_TYPE_BYTES, "event filter")?;
    }
    if view.created_at_sequence == 0 {
        return Err("subscription created_at_sequence must be positive".into());
    }
    Ok(())
}

fn validate_pending_delivery(delivery: &PendingDelivery) -> Result<(), String> {
    let envelope = &delivery.envelope;
    if envelope.delivery_id == 0 || envelope.event.id == 0 {
        return Err("delivery ids must be positive".into());
    }
    if !(1..=MAX_RETRY_ATTEMPTS).contains(&envelope.attempt) {
        return Err(format!(
            "delivery attempt must be between 1 and {MAX_RETRY_ATTEMPTS}"
        ));
    }
    validate_token(
        &envelope.subscription_id,
        MAX_SUBSCRIPTION_ID_BYTES,
        "delivery subscription id",
    )?;
    validate_token(
        &envelope.event.event_type,
        MAX_EVENT_TYPE_BYTES,
        "delivery event type",
    )?;
    validate_token(&envelope.event.subject, 256, "delivery event subject")?;
    validate_token(&envelope.event.request_id, 256, "delivery event request id")?;
    validate_token(&envelope.signature, 256, "delivery signature")?;
    if let Some(error) = &delivery.last_error {
        if error.len() > MAX_DELIVERY_ERROR_BYTES || error.bytes().any(|byte| byte < 0x20) {
            return Err("delivery last_error is unbounded or contains control bytes".into());
        }
    }
    Ok(())
}

fn validate_delivery_attempt(attempt: &DeliveryAttempt) -> Result<(), String> {
    if attempt.attempt_id == 0 || attempt.delivery_id == 0 || attempt.event_id == 0 {
        return Err("delivery attempt ids must be positive".into());
    }
    if !(1..=MAX_RETRY_ATTEMPTS).contains(&attempt.attempt) {
        return Err(format!(
            "delivery attempt number must be between 1 and {MAX_RETRY_ATTEMPTS}"
        ));
    }
    validate_token(
        &attempt.subscription_id,
        MAX_SUBSCRIPTION_ID_BYTES,
        "delivery attempt subscription id",
    )?;
    validate_token(
        &attempt.event_type,
        MAX_EVENT_TYPE_BYTES,
        "delivery attempt event type",
    )?;
    validate_token(&attempt.action, 64, "delivery attempt action")?;
    validate_token(&attempt.outcome, 64, "delivery attempt outcome")?;
    validate_token(&attempt.signature, 256, "delivery attempt signature")?;
    if let Some(error) = &attempt.error {
        if error.len() > MAX_DELIVERY_ERROR_BYTES || error.bytes().any(|byte| byte < 0x20) {
            return Err("delivery attempt error is unbounded or contains control bytes".into());
        }
    }
    if let Some(receipt_id) = &attempt.receipt_id {
        validate_receipt_id(receipt_id)?;
    }
    if let Some(receipt_digest) = &attempt.receipt_digest {
        validate_receipt_digest(receipt_digest)?;
        if attempt.receipt_id.is_none() {
            return Err("delivery attempt receipt_digest requires receipt_id".into());
        }
    }
    Ok(())
}

fn validate_token(value: &str, maximum: usize, label: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum || value.bytes().any(|byte| byte < 0x20) {
        return Err(format!(
            "{label} must be non-empty, bounded, and free of control bytes"
        ));
    }
    Ok(())
}

fn validate_secret(secret: &str) -> Result<(), String> {
    if secret.len() < 8 || secret.len() > MAX_SECRET_BYTES || secret.bytes().any(|byte| byte < 0x20)
    {
        return Err(format!(
            "webhook secret must contain 8..={MAX_SECRET_BYTES} printable bytes"
        ));
    }
    Ok(())
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    if endpoint.len() > MAX_ENDPOINT_BYTES || endpoint.bytes().any(|byte| byte <= 0x20) {
        return Err(format!(
            "endpoint must be at most {MAX_ENDPOINT_BYTES} visible bytes"
        ));
    }
    let Some(rest) = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
    else {
        return Err("endpoint must use http:// or https://".into());
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty()
        || authority.contains('@')
        || authority.contains('#')
        || authority.contains('?')
    {
        return Err("endpoint must contain a host without credentials, fragment, or query".into());
    }
    Ok(())
}

fn checked_limit(limit: usize) -> Result<usize, String> {
    if (1..=1000).contains(&limit) {
        Ok(limit)
    } else {
        Err("limit must be between 1 and 1000".into())
    }
}

fn validate_review_id(review_id: &str) -> Result<(), String> {
    if review_id.len() != 64
        || !review_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("review_id must be a 64-character hexadecimal content hash".into());
    }
    Ok(())
}

fn validate_receipt_id(receipt_id: &str) -> Result<(), String> {
    validate_token(receipt_id, 128, "receipt_id")
}

fn validate_receipt_digest(receipt_digest: &str) -> Result<(), String> {
    if receipt_digest.len() != 64
        || !receipt_digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("receipt_digest must be 64 lowercase hexadecimal characters".into());
    }
    Ok(())
}

fn delivery_receipt_metadata(payload: &Value) -> Result<(Option<String>, Option<String>), String> {
    let Some(projection) = payload.get("delivery_receipt") else {
        return Ok((None, None));
    };
    let receipt_id = projection
        .get("receipt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "delivery_receipt projection must contain a string receipt_id".to_string()
        })?;
    validate_receipt_id(receipt_id)?;
    let receipt_digest = projection
        .get("receipt_digest")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(digest) = receipt_digest.as_deref() {
        validate_receipt_digest(digest)?;
    }
    Ok((Some(receipt_id.to_owned()), receipt_digest))
}

fn event_matches_review_id(payload: &Value, review_id: &str) -> bool {
    payload
        .get("response")
        .is_some_and(|response| value_contains_review_id(response, review_id))
        || payload
            .get("review_id")
            .and_then(Value::as_str)
            .is_some_and(|value| value == review_id)
}

fn value_contains_review_id(value: &Value, review_id: &str) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("review_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value == review_id)
                || object
                    .values()
                    .any(|child| value_contains_review_id(child, review_id))
        }
        Value::Array(values) => values
            .iter()
            .any(|child| value_contains_review_id(child, review_id)),
        _ => false,
    }
}

fn event_matches_receipt_id(payload: &Value, receipt_id: &str) -> bool {
    payload
        .get("delivery_receipt")
        .is_some_and(|projection| value_contains_receipt_id(projection, receipt_id))
        || payload
            .get("response")
            .is_some_and(|response| value_contains_receipt_id(response, receipt_id))
}

fn value_contains_receipt_id(value: &Value, receipt_id: &str) -> bool {
    match value {
        Value::Object(object) => {
            object
                .get("receipt_id")
                .and_then(Value::as_str)
                .is_some_and(|value| value == receipt_id)
                || object
                    .values()
                    .any(|child| value_contains_receipt_id(child, receipt_id))
        }
        Value::Array(values) => values
            .iter()
            .any(|child| value_contains_receipt_id(child, receipt_id)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_cursors_report_retention_gaps_and_webhook_signatures() {
        let mut log = EventLog::new(2).unwrap();
        let subscription = log
            .register_subscription(
                Some("research"),
                "https://hooks.example.test/aurora",
                Some(&["tool.completed".into()]),
                "a-secret-key",
            )
            .unwrap();
        assert_eq!(subscription.events, ["tool.completed"]);
        log.emit(
            "tool.completed",
            "fiber_compile",
            "req-1",
            json!({"ok": true}),
        )
        .unwrap();
        log.emit(
            "tool.refused",
            "fiber_compile",
            "req-2",
            json!({"ok": false}),
        )
        .unwrap();
        log.emit(
            "tool.completed",
            "fiber_refine",
            "req-3",
            json!({"ok": true}),
        )
        .unwrap();
        let page = log.events(0, 10).unwrap();
        assert!(page.gap);
        assert_eq!(page.events.len(), 2);
        let deliveries = log.deliveries("research", 0, 10).unwrap();
        assert_eq!(deliveries.deliveries.len(), 2);
        assert!(deliveries.deliveries[0].signature.starts_with("sha256="));
        let attempts = log.delivery_attempts("research", 0, 10).unwrap();
        assert_eq!(attempts.attempts.len(), 2);
        assert_eq!(attempts.attempts[0].action, "enqueue");
        assert_eq!(attempts.attempts[0].outcome, "pending");
        assert_eq!(log.acknowledge("research", &[1]).unwrap(), 1);
        assert_eq!(log.retry("research", &[2]).unwrap()[0].attempt, 2);
        let attempts = log.delivery_attempts("research", 0, 10).unwrap();
        assert_eq!(attempts.attempts[2].outcome, "acknowledged");
        assert_eq!(attempts.attempts[3].outcome, "scheduled");
    }

    #[test]
    fn invalid_subscription_boundaries_fail_closed() {
        let mut log = EventLog::new(4).unwrap();
        assert!(log
            .register_subscription(None, "ftp://example.test", None, "a-secret-key")
            .is_err());
        assert!(log
            .register_subscription(None, "https://example.test", None, "short")
            .is_err());
        assert!(log.events(0, 0).is_err());
    }

    #[test]
    fn route_review_event_filter_is_exact_and_bounded() {
        let mut log = EventLog::new(4).unwrap();
        let first = "a".repeat(64);
        let second = "b".repeat(64);
        log.emit(
            "tool.completed",
            "capability_route_review",
            "req-1",
            json!({
                "tool": "capability_route_review",
                "response": {"result": {"structuredContent": {"review_id": first}}}
            }),
        )
        .unwrap();
        log.emit(
            "tool.completed",
            "capability_route_review",
            "req-2",
            json!({
                "tool": "capability_route_review",
                "response": {"result": {"structuredContent": {"review_id": second}}}
            }),
        )
        .unwrap();
        let page = log.events_for_review(0, 10, &first).unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].request_id, "req-1");
        assert!(log.events_for_review(0, 10, &"z".repeat(63)).is_err());
    }

    #[test]
    fn delivery_receipt_event_filter_accepts_projection_and_rejects_near_matches() {
        let mut log = EventLog::new(4).unwrap();
        log.emit(
            "tool.completed",
            "developer_delivery_receipt",
            "req-1",
            json!({
                "tool": "developer_delivery_receipt",
                "delivery_receipt": {
                    "workflow": "developer_delivery_receipt",
                    "receipt_id": "receipt-1",
                    "receipt_digest": "a".repeat(64)
                }
            }),
        )
        .unwrap();
        log.emit(
            "tool.completed",
            "developer_delivery_receipt",
            "req-2",
            json!({
                "tool": "developer_delivery_receipt",
                "delivery_receipt": { "receipt_id": "receipt-10" }
            }),
        )
        .unwrap();
        let page = log.events_for_receipt(0, 10, "receipt-1").unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].request_id, "req-1");
        assert!(log.events_for_receipt(0, 10, "receipt-1\n").is_err());
    }

    #[test]
    fn event_checkpoint_restores_cursor_continuity_without_delivery_secrets() {
        let path =
            std::env::temp_dir().join(format!("bioprism-event-state-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut log = EventLog::new(2).unwrap();
        log.emit("tool.completed", "one", "req-1", json!({"n": 1}))
            .unwrap();
        log.emit("tool.completed", "two", "req-2", json!({"n": 2}))
            .unwrap();
        log.emit("tool.completed", "three", "req-3", json!({"n": 3}))
            .unwrap();
        let bytes = log.checkpoint_to_path(&path).unwrap();
        assert!(bytes > 0);

        let restored = EventLog::from_checkpoint_path(2, Some(&path)).unwrap();
        let page = restored.events(0, 10).unwrap();
        assert!(page.gap);
        assert_eq!(page.events.len(), 2);
        assert_eq!(page.events[0].id, 2);
        assert_eq!(restored.metrics().next_event_id, 4);
        assert_eq!(restored.metrics().subscriptions, 0);
        assert_eq!(restored.metrics().pending_deliveries, 0);

        let mut schema_four =
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap();
        schema_four["schema_version"] = json!(4);
        schema_four
            .as_object_mut()
            .unwrap()
            .remove("delivery_receipt_metadata_durable");
        schema_four.as_object_mut().unwrap().remove("state_digest");
        let digest = checkpoint_digest(&schema_four).unwrap();
        schema_four
            .as_object_mut()
            .unwrap()
            .insert("state_digest".into(), Value::String(digest));
        std::fs::write(&path, serde_json::to_vec_pretty(&schema_four).unwrap()).unwrap();
        let migrated_four = EventLog::from_checkpoint_path(2, Some(&path)).unwrap();
        assert_eq!(migrated_four.metrics().next_event_id, 4);
        assert_eq!(migrated_four.metrics().retained_events, 2);

        let mut schema_three =
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap();
        schema_three["schema_version"] = json!(3);
        {
            let object = schema_three.as_object_mut().unwrap();
            object.remove("next_attempt_id");
            object.remove("dropped_attempts");
            object.remove("delivery_attempts");
            object.remove("delivery_attempts_durable");
            object.remove("state_digest");
        }
        let digest = checkpoint_digest(&schema_three).unwrap();
        schema_three
            .as_object_mut()
            .unwrap()
            .insert("state_digest".into(), Value::String(digest));
        std::fs::write(&path, serde_json::to_vec_pretty(&schema_three).unwrap()).unwrap();
        let migrated_three = EventLog::from_checkpoint_path(2, Some(&path)).unwrap();
        assert_eq!(migrated_three.metrics().next_event_id, 4);
        assert_eq!(migrated_three.metrics().retained_events, 2);

        let mut schema_two =
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap();
        schema_two["schema_version"] = json!(2);
        schema_two.as_object_mut().unwrap().remove("state_digest");
        std::fs::write(&path, serde_json::to_vec_pretty(&schema_two).unwrap()).unwrap();
        let migrated = EventLog::from_checkpoint_path(2, Some(&path)).unwrap();
        assert_eq!(migrated.metrics().next_event_id, 4);
        assert_eq!(migrated.metrics().retained_events, 2);
        let _ = std::fs::remove_file(path);
    }

    struct TestSender {
        calls: usize,
        fail_once: bool,
    }

    impl DeliverySender for TestSender {
        fn send(&mut self, _endpoint: &str, _envelope: &Value) -> Result<(), DeliverySendError> {
            self.calls += 1;
            if self.fail_once && self.calls == 1 {
                Err(DeliverySendError::retryable("temporary transport failure"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn delivery_worker_acknowledges_success_and_retries_only_retryable_failure() {
        let mut log = EventLog::new(4).unwrap();
        log.register_subscription(
            Some("worker"),
            "https://example.test/hook",
            Some(&["tool.completed".into()]),
            "a-secret-key",
        )
        .unwrap();
        log.emit(
            "tool.completed",
            "tool",
            "req",
            json!({
                "ok": true,
                "delivery_receipt": {
                    "receipt_id": "receipt-worker-1",
                    "receipt_digest": "a".repeat(64)
                }
            }),
        )
        .unwrap();
        let mut sender = TestSender {
            calls: 0,
            fail_once: true,
        };
        let first = log.deliver_once(&mut sender, 10).unwrap();
        assert_eq!(first.attempted, 1);
        assert_eq!(first.retried, 1);
        assert_eq!(first.acknowledged, 0);
        assert_eq!(
            log.deliveries("worker", 0, 10).unwrap().deliveries[0].attempt,
            2
        );
        let second = log.deliver_once(&mut sender, 10).unwrap();
        assert_eq!(second.acknowledged, 1);
        assert_eq!(log.deliveries("worker", 0, 10).unwrap().deliveries.len(), 0);
        let attempts = log.delivery_attempts("worker", 0, 10).unwrap();
        assert_eq!(attempts.attempts.len(), 4);
        assert_eq!(attempts.attempts[1].outcome, "retryable_failure");
        assert_eq!(attempts.attempts[2].action, "retry");
        assert_eq!(attempts.attempts[3].outcome, "accepted");
        assert_eq!(
            attempts.attempts[0].receipt_id.as_deref(),
            Some("receipt-worker-1")
        );
        let receipt_digest = "a".repeat(64);
        assert_eq!(
            attempts.attempts[3].receipt_digest.as_deref(),
            Some(receipt_digest.as_str())
        );
        let receipt_attempts = log
            .delivery_attempts_for_receipt("receipt-worker-1", 0, 10)
            .unwrap();
        assert_eq!(receipt_attempts.attempts.len(), 4);
    }

    #[test]
    fn delivery_worker_keeps_permanent_failure_pending_for_operator_review() {
        struct PermanentSender;
        impl DeliverySender for PermanentSender {
            fn send(
                &mut self,
                _endpoint: &str,
                _envelope: &Value,
            ) -> Result<(), DeliverySendError> {
                Err(DeliverySendError::permanent("certificate rejected"))
            }
        }
        let mut log = EventLog::new(2).unwrap();
        log.register_subscription(
            Some("permanent"),
            "https://example.test/hook",
            None,
            "a-secret-key",
        )
        .unwrap();
        log.emit("tool.completed", "tool", "req", json!({"ok": true}))
            .unwrap();
        let report = log.deliver_once(&mut PermanentSender, 1).unwrap();
        assert_eq!(report.failed, 1);
        assert_eq!(report.retried, 0);
        assert_eq!(report.pending, 1);
        let page = log.deliveries("permanent", 0, 10).unwrap();
        assert_eq!(page.deliveries.len(), 1);
        assert_eq!(page.deliveries[0].state, "failed");
        assert_eq!(
            page.deliveries[0].last_error,
            Some("certificate rejected".into())
        );
        assert_eq!(page.deliveries[0].last_error_retryable, Some(false));
        let replayed = log.replay("permanent", &[1]).unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].state, "pending");
        assert_eq!(replayed[0].attempt, 1);
        assert_eq!(replayed[0].last_error, None);
    }
}
