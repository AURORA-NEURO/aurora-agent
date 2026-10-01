//! Event, metrics, and review-history routes over the shared API state.
//!
//! Keeping these projections together makes cursor and retention behavior reviewable without
//! splitting their event log ownership from the main router.

use super::*;

impl ApiRouter {
    pub(super) fn metrics(&self) -> HttpResponse {
        HttpResponse::json(200, &json!({ "ok": true, "metrics": self.event_metrics() }))
    }

    pub(super) fn events(&self, request: &HttpRequest) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), "query"),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let receipt_id = query.get("receipt_id").map(String::as_str);
        if review_id.is_some() && receipt_id.is_some() {
            return self.error(
                400,
                "invalid_query",
                "review_id and receipt_id are mutually exclusive event filters",
                "query",
            );
        }
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    "query",
                )
            }
        };
        let page = match (review_id, receipt_id) {
            (Some(review_id), None) => events.events_for_review(after, limit, review_id),
            (None, Some(receipt_id)) => events.events_for_receipt(after, limit, receipt_id),
            (None, None) => events.events(after, limit),
            (Some(_), Some(_)) => {
                return self.error(
                    400,
                    "invalid_query",
                    "review_id and receipt_id are mutually exclusive event filters",
                    "query",
                )
            }
        };
        match page {
            Ok(page) => HttpResponse::json(200, &json!({ "ok": true, "page": page })),
            Err(error) => self.error(400, "invalid_query", &error, "query"),
        }
    }

    pub(super) fn event_stream(&self, request: &HttpRequest) -> HttpResponse {
        let query = match request.query() {
            Ok(query) => query,
            Err(error) => return self.error(400, "invalid_query", &error.to_string(), "query"),
        };
        let after = match query_u64(&query, "after", 0) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let limit = match query_usize(&query, "limit", 100) {
            Ok(value) => value,
            Err(error) => return self.error(400, "invalid_query", &error, "query"),
        };
        let review_id = query.get("review_id").map(String::as_str);
        let receipt_id = query.get("receipt_id").map(String::as_str);
        if review_id.is_some() && receipt_id.is_some() {
            return self.error(
                400,
                "invalid_query",
                "review_id and receipt_id are mutually exclusive event filters",
                "query",
            );
        }
        let events = match self.events.lock() {
            Ok(events) => events,
            Err(_) => {
                return self.error(
                    500,
                    "event_log_unavailable",
                    "event log is unavailable",
                    "query",
                )
            }
        };
        let page = match (review_id, receipt_id) {
            (Some(review_id), None) => events.events_for_review(after, limit, review_id),
            (None, Some(receipt_id)) => events.events_for_receipt(after, limit, receipt_id),
            (None, None) => events.events(after, limit),
            (Some(_), Some(_)) => {
                return self.error(
                    400,
                    "invalid_query",
                    "review_id and receipt_id are mutually exclusive event filters",
                    "query",
                )
            }
        };
        match page {
            Ok(page) => {
                HttpResponse::text(200, "text/event-stream; charset=utf-8", events.sse(&page))
                    .with_header("x-next-after", page.next_after.to_string())
            }
            Err(error) => self.error(400, "invalid_query", &error, "query"),
        }
    }

    pub(super) fn delivery_receipt_events(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(receipt_id) = delivery_receipt_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "delivery-receipt event route does not exist",
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
        match events.events_for_receipt(after, limit, &receipt_id) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "developer_delivery_receipt_events",
                    "receipt_id": receipt_id,
                    "found": !page.events.is_empty(),
                    "page": page,
                    "guarantees": [
                        "evidence is limited to retained developer delivery receipt events with an exact receipt_id match",
                        "after is an exclusive event cursor and next_after is the last returned event id",
                        "retention gaps are reported instead of silently presented as complete history",
                        "an empty result means no matching retained event was found in the requested cursor window"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }

    pub(super) fn delivery_receipt_attempts(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(receipt_id) = delivery_receipt_attempts_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "delivery-receipt attempt route does not exist",
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
        match events.delivery_attempts_for_receipt(&receipt_id, after, limit) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "developer_delivery_receipt_attempts",
                    "receipt_id": receipt_id,
                    "found": !page.attempts.is_empty(),
                    "page": page,
                    "guarantees": [
                        "provenance is limited to retained attempt rows with an exact receipt_id match",
                        "receiver acceptance is reported only when the operator-owned sender returned success",
                        "retention gaps are reported instead of silently presented as complete history"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }

    pub(super) fn route_review_evidence(
        &self,
        request: &HttpRequest,
        request_id: &str,
    ) -> HttpResponse {
        let Some(review_id) = route_review_id(&request.path_segments()) else {
            return self.error(
                404,
                "not_found",
                "route-review evidence route does not exist",
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
        match events.events_for_review(after, limit, &review_id) {
            Ok(page) => HttpResponse::json(
                200,
                &json!({
                    "ok": true,
                    "workflow": "capability_route_review_evidence",
                    "review_id": review_id,
                    "found": !page.events.is_empty(),
                    "page": page,
                    "guarantees": [
                        "evidence is limited to retained capability_route_review tool events with an exact review_id match",
                        "after is an exclusive event cursor and next_after is the last returned event id",
                        "retention gaps are reported instead of silently presented as complete history",
                        "an empty result means no matching retained event was found in the requested cursor window"
                    ]
                }),
            ),
            Err(error) => self.error(400, "invalid_query", &error, request_id),
        }
    }
}
