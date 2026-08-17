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
pub const EVENT_STATE_SCHEMA_VERSION: u64 = 1;
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

#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionView {
    pub id: String,
    pub endpoint: String,
    pub events: Vec<String>,
    pub active: bool,
    pub created_at_sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone)]
struct PendingDelivery {
    envelope: WebhookEnvelope,
    last_error: Option<String>,
    last_error_retryable: Option<bool>,
}

pub struct EventLog {
    capacity: usize,
    delivery_capacity: usize,
    next_event_id: u64,
    next_delivery_id: u64,
    events: VecDeque<ApiEvent>,
    subscriptions: BTreeMap<String, Subscription>,
    deliveries: BTreeMap<u64, PendingDelivery>,
    dropped_events: u64,
    dropped_deliveries: u64,
}

impl EventLog {
    pub fn new(capacity: usize) -> Result<Self, String> {
        if capacity == 0 || capacity > 100_000 {
            return Err("event capacity must be between 1 and 100000".into());
        }
        Ok(Self {
            capacity,
            delivery_capacity: capacity.saturating_mul(8).clamp(1, 100_000),
            next_event_id: 1,
            next_delivery_id: 1,
            events: VecDeque::with_capacity(capacity.min(1024)),
            subscriptions: BTreeMap::new(),
            deliveries: BTreeMap::new(),
            dropped_events: 0,
            dropped_deliveries: 0,
        })
    }

    /// Restore only the retained event cursor from an optional bounded checkpoint.
    ///
    /// Subscriptions, signing secrets, and pending deliveries are intentionally not restored.
    /// They are operator-owned delivery state and must be re-established explicitly after a
    /// process restart rather than silently resurrecting credentials from a JSON snapshot.
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
        if schema_version != EVENT_STATE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported event state schema version {schema_version}; expected {EVENT_STATE_SCHEMA_VERSION}"
            ));
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
        Ok(log)
    }

    /// Atomically write a bounded event-only checkpoint. Secrets and delivery state never enter it.
    pub fn checkpoint_to_path(&self, path: &std::path::Path) -> Result<usize, String> {
        let mut events = self.events.iter().cloned().collect::<Vec<_>>();
        let mut dropped_events = self.dropped_events;
        loop {
            let document = json!({
                "schema_version": EVENT_STATE_SCHEMA_VERSION,
                "next_event_id": self.next_event_id,
                "dropped_events": dropped_events,
                "events": events,
                "subscriptions_durable": false,
                "webhook_deliveries_durable": false,
            });
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
            .map(delivery_view)
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

    pub fn acknowledge(&mut self, subscription_id: &str, ids: &[u64]) -> Result<usize, String> {
        self.require_subscription(subscription_id)?;
        if ids.len() > 1000 {
            return Err("a single acknowledgement may contain at most 1000 ids".into());
        }
        let mut acknowledged = 0;
        for id in ids {
            let belongs = self
                .deliveries
                .get(id)
                .is_some_and(|delivery| delivery.envelope.subscription_id == subscription_id);
            if belongs && self.deliveries.remove(id).is_some() {
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
        let secret = self
            .subscriptions
            .get(subscription_id)
            .map(|subscription| subscription.secret.clone())
            .ok_or_else(|| format!("unknown subscription {subscription_id:?}"))?;
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
            retried.push(delivery_view(delivery));
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
        let secret = self
            .subscriptions
            .get(subscription_id)
            .map(|subscription| subscription.secret.clone())
            .ok_or_else(|| format!("unknown subscription {subscription_id:?}"))?;
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
            replayed.push(delivery_view(delivery));
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
                .filter(|subscription| subscription.view.active)
                .count(),
            pending_deliveries: self.deliveries.len(),
            dropped_deliveries: self.dropped_deliveries,
            next_event_id: self.next_event_id,
            next_delivery_id: self.next_delivery_id,
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
            pending: 0,
            failures: Vec::new(),
        };
        for subscription in subscriptions {
            if remaining == 0 {
                break;
            }
            let page = self.deliveries(&subscription.id, 0, remaining)?;
            for delivery in page.deliveries {
                if remaining == 0 {
                    break;
                }
                remaining -= 1;
                report.attempted += 1;
                match sender.send(&subscription.endpoint, &delivery.envelope) {
                    Ok(()) => {
                        self.acknowledge(&subscription.id, &[delivery.delivery_id])?;
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
                envelope,
                last_error: None,
                last_error_retryable: None,
            },
        );
        Ok(())
    }

    fn require_subscription(&self, id: &str) -> Result<(), String> {
        if self.subscriptions.contains_key(id) {
            Ok(())
        } else {
            Err(format!("unknown subscription {id:?}"))
        }
    }
}

fn delivery_view(delivery: &PendingDelivery) -> DeliveryView {
    DeliveryView {
        delivery_id: delivery.envelope.delivery_id,
        subscription_id: delivery.envelope.subscription_id.clone(),
        attempt: delivery.envelope.attempt,
        state: delivery_state(delivery),
        last_error: delivery.last_error.clone(),
        last_error_retryable: delivery.last_error_retryable,
        event_id: delivery.envelope.event.id,
        event_type: delivery.envelope.event.event_type.clone(),
        signature: delivery.envelope.signature.clone(),
        envelope: serde_json::to_value(&delivery.envelope).unwrap_or_else(|_| json!({})),
    }
}

fn delivery_state(delivery: &PendingDelivery) -> String {
    match delivery.last_error_retryable {
        None => "pending".into(),
        Some(false) => "failed".into(),
        Some(true) if delivery.envelope.attempt >= MAX_RETRY_ATTEMPTS => "exhausted".into(),
        Some(true) => "retryable".into(),
    }
}

fn bounded_error(message: String) -> String {
    if message.len() <= MAX_DELIVERY_ERROR_BYTES {
        return message;
    }
    let prefix_limit = MAX_DELIVERY_ERROR_BYTES.saturating_sub(16);
    let mut bounded = String::new();
    for character in message.chars() {
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

fn hex_digest(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
        assert_eq!(log.acknowledge("research", &[1]).unwrap(), 1);
        assert_eq!(log.retry("research", &[2]).unwrap()[0].attempt, 2);
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
        log.emit("tool.completed", "tool", "req", json!({"ok": true}))
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
