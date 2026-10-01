//! Webhook subscription and delivery lifecycle routes.
//!
//! These handlers mutate delivery state through the router-owned event log so subscription,
//! retry, and replay operations retain one policy and persistence boundary.

use super::*;

impl ApiRouter {
    pub(super) fn list_subscriptions(&self) -> HttpResponse {
        HttpResponse::json(
            200,
            &json!({
                "ok": true,
                "subscriptions": self
                    .events
                    .lock()
                    .map(|events| events.subscriptions())
                    .unwrap_or_default(),
                "secret_policy": "secrets are never returned; delivery signatures are computed over the unsigned envelope"
            }),
        )
    }

    pub(super) fn create_subscription(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let endpoint = match body.get("endpoint").and_then(Value::as_str) {
            Some(value) => value,
            None => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "endpoint is required",
                    request_id,
                )
            }
        };
        let secret = match body.get("secret").and_then(Value::as_str) {
            Some(value) => value,
            None => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "secret is required",
                    request_id,
                )
            }
        };
        let filters = match body.get("events") {
            None => None,
            Some(Value::Array(values)) => {
                let mut filters = Vec::with_capacity(values.len());
                for value in values {
                    let Some(value) = value.as_str() else {
                        return self.error(
                            422,
                            "invalid_subscription",
                            "events must contain strings",
                            request_id,
                        );
                    };
                    filters.push(value.to_string());
                }
                Some(filters)
            }
            Some(_) => {
                return self.error(
                    422,
                    "invalid_subscription",
                    "events must be an array",
                    request_id,
                )
            }
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.register_subscription(
            body.get("id").and_then(Value::as_str),
            endpoint,
            filters.as_deref(),
            secret,
        ) {
            Ok(subscription) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(
                    201,
                    &json!({
                        "ok": true,
                        "subscription": subscription,
                        "delivery": {
                            "mode": "signed_outbox",
                            "poll": "/v1/webhooks/subscriptions/{id}/deliveries",
                            "ack": "/v1/webhooks/subscriptions/{id}/ack",
                            "retry": "/v1/webhooks/subscriptions/{id}/retry",
                            "replay": "/v1/webhooks/subscriptions/{id}/replay",
                            "rebind": "/v1/webhooks/subscriptions/{id}/rebind"
                        }
                    }),
                )
            }
            Err(error) => self.error(422, "invalid_subscription", &error, request_id),
        }
    }

    pub(super) fn delete_subscription(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), None) else {
            return self.error(
                404,
                "not_found",
                "subscription route does not exist",
                request_id,
            );
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.remove_subscription(&id) {
            Ok(true) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(200, &json!({ "ok": true, "deleted": id }))
            }
            Ok(false) => self.error(404, "not_found", "subscription does not exist", request_id),
            Err(error) => self.error(409, "subscription_error", &error, request_id),
        }
    }

    pub(super) fn rebind_subscription(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("rebind")) else {
            return self.error(
                404,
                "not_found",
                "subscription rebind route does not exist",
                request_id,
            );
        };
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(secret) = body.get("secret").and_then(Value::as_str) else {
            return self.error(
                422,
                "invalid_subscription_secret",
                "secret is required for an in-memory subscription rebind",
                request_id,
            );
        };
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.rebind_subscription(&id, secret) {
            Ok((subscription, resigned_deliveries)) => {
                drop(events);
                let _ = self.event_persistence.persist();
                HttpResponse::json(
                    200,
                    &json!({
                        "ok": true,
                        "subscription": subscription,
                        "resigned_deliveries": resigned_deliveries,
                        "secret_policy": "the supplied secret is held in memory only and is never returned or persisted"
                    }),
                )
            }
            Err(error) => self.error(404, "subscription_rebind_failed", &error, request_id),
        }
    }

    pub(super) fn list_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("deliveries")) else {
            return self.error(
                404,
                "not_found",
                "delivery route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.deliveries(&id, after, limit) {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(404, "not_found", &error, request_id),
        }
    }

    pub(super) fn list_delivery_attempts(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(id) = subscription_id(&request.path_segments(), Some("attempts")) else {
            return self.error(
                404,
                "not_found",
                "delivery attempt route does not exist",
                request_id,
            );
        };
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), request_id),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, request_id),
        };
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        match events.delivery_attempts(&id, after, limit) {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(404, "not_found", &error, request_id),
        }
    }

    pub(super) fn ack_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, false, false)
    }

    pub(super) fn retry_deliveries(&self, request: &HttpRequest, request_id: &str) -> HttpResponse {
        self.delivery_mutation(request, request_id, true, false)
    }

    pub(super) fn replay_deliveries(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        self.delivery_mutation(request, request_id, false, true)
    }

    pub(super) fn delivery_mutation(
        &self,
        request: &HttpRequest,
        request_id: &str,
        retry: bool,
        replay: bool,
    ) -> HttpResponse {
        let operation = if retry {
            "retry"
        } else if replay {
            "replay"
        } else {
            "ack"
        };
        let Some(id) = subscription_id(&request.path_segments(), Some(operation)) else {
            return self.error(
                404,
                "not_found",
                "delivery route does not exist",
                request_id,
            );
        };
        let body = match self.json_object(request) {
            Ok(body) => body,
            Err(error) => return self.error(400, "invalid_json", &error, request_id),
        };
        let Some(values) = body.get("delivery_ids").and_then(Value::as_array) else {
            return self.error(
                422,
                "invalid_delivery_ids",
                "delivery_ids must be an array",
                request_id,
            );
        };
        let mut ids = Vec::with_capacity(values.len());
        for value in values {
            let Some(id) = value.as_u64() else {
                return self.error(
                    422,
                    "invalid_delivery_ids",
                    "delivery_ids must contain integers",
                    request_id,
                );
            };
            ids.push(id);
        }
        let mut events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    request_id,
                )
            }
        };
        if retry {
            match events.retry(&id, &ids) {
                Ok(deliveries) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "retried": deliveries }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        } else if replay {
            match events.replay(&id, &ids) {
                Ok(deliveries) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "replayed": deliveries }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        } else {
            match events.acknowledge(&id, &ids) {
                Ok(acknowledged) => {
                    drop(events);
                    let _ = self.event_persistence.persist();
                    HttpResponse::json(200, &json!({ "ok": true, "acknowledged": acknowledged }))
                }
                Err(error) => self.error(404, "not_found", &error, request_id),
            }
        }
    }
}
