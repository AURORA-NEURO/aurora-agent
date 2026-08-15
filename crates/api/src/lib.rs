//! A bounded HTTP/REST and event integration layer for the AURORA/Prism MCP kernel.
//!
//! The crate implements the network-facing half of the developer platform without duplicating
//! domain logic.  `ApiRouter` exposes the same 114 (and future) MCP tools as REST routes, keeps
//! JSON-RPC available for clients that already speak MCP, and records successful/refused tool
//! calls in a cursor-addressable event log.  Webhook subscriptions produce signed outbox
//! envelopes; an operator-owned worker polls, retries, sends, and acknowledges those envelopes.
//!
//! This is intentionally explicit about what it does not provide: HTTP/2 gRPC, TLS termination,
//! external delivery workers, durable storage, and authentication providers remain deployment
//! concerns.  The local boundary still enforces body/header limits, root confinement through the
//! MCP server, constant-time bearer comparison, deterministic cursors, retention-gap reporting,
//! HMAC-SHA256 signatures, and idempotent acknowledgement.
//!
//! The implementation addresses the executable portion of the developer-platform REST/event
//! contract (`11.08` and `11.09`); TypeScript and gRPC clients remain separate artifacts.

pub mod events;
pub mod http;
pub mod router;

pub use events::{
    ApiEvent, DeliveryPage, DeliveryView, EventLog, EventMetrics, EventPage, SubscriptionView,
    WebhookEnvelope,
};
pub use http::{read_request, HttpError, HttpRequest, HttpResponse};
pub use router::{
    ApiConfig, ApiRouter, API_VERSION, DEFAULT_EVENT_CAPACITY, DEFAULT_MAX_BODY_BYTES,
    DEFAULT_MAX_HEADER_BYTES,
};

use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Serves one bounded request per accepted connection until the listener fails.
pub fn serve(listener: TcpListener, router: Arc<Mutex<ApiRouter>>) -> std::io::Result<()> {
    for incoming in listener.incoming() {
        let stream = incoming?;
        let router = Arc::clone(&router);
        std::thread::spawn(move || {
            if let Err(error) = serve_connection(stream, router) {
                eprintln!("bioprism-api connection error: {error}");
            }
        });
    }
    Ok(())
}

fn serve_connection(stream: TcpStream, router: Arc<Mutex<ApiRouter>>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let (maximum_header_bytes, maximum_body_bytes) = {
        let router = router
            .lock()
            .map_err(|_| std::io::Error::other("API router lock poisoned"))?;
        router.limits()
    };
    let writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let request = match read_request(&mut reader, maximum_header_bytes, maximum_body_bytes) {
        Ok(request) => request,
        Err(error) => {
            let response = HttpResponse::json(
                match error {
                    HttpError::BodyTooLarge { .. } => 413,
                    HttpError::HeaderTooLarge { .. } => 413,
                    _ => 400,
                },
                &serde_json::json!({ "ok": false, "error": error.to_string() }),
            );
            let mut writer = writer;
            response.write_to(&mut writer)?;
            writer.flush()?;
            return Ok(());
        }
    };
    let response = {
        let mut router = router
            .lock()
            .map_err(|_| std::io::Error::other("API router lock poisoned"))?;
        router.handle(request)
    };
    let mut writer = writer;
    response.write_to(&mut writer)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn wire_round_trip_uses_the_same_bounded_router() {
        let mut router =
            ApiRouter::new(std::env::current_dir().unwrap(), ApiConfig::default()).unwrap();
        let raw = b"GET /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = read_request(
            &mut Cursor::new(raw),
            DEFAULT_MAX_HEADER_BYTES,
            DEFAULT_MAX_BODY_BYTES,
        )
        .unwrap();
        let response = router.handle(request);
        assert_eq!(response.status, 200);
        assert!(String::from_utf8(response.body)
            .unwrap()
            .contains("\"ready\":true"));
    }
}
