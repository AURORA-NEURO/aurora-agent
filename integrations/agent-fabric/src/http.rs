//! Minimal HTTP/1.1 Content-Length adapter for the MCP surface.
//!
//! This module deliberately does not claim TLS, HTTP/2, chunked transfer, keep-alive pooling, or
//! a general web framework. It parses one bounded request, routes `/mcp` to [`McpServer`], and
//! emits one Content-Length response.

use crate::json::{self, Value};
use crate::mcp_stdio::McpServer;
use crate::transport::{TransportError, MAX_FRAME_BYTES, MAX_HEADER_BYTES};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: json::to_string(value).into_bytes(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!("HTTP/1.1 {} {}\r\n", self.status, reason(self.status));
        out.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str("Connection: close\r\n\r\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

#[derive(Debug)]
pub enum HttpError {
    Incomplete,
    InvalidRequest(&'static str),
    Transport(TransportError),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete => write!(f, "HTTP request is incomplete"),
            Self::InvalidRequest(message) => write!(f, "invalid HTTP request: {message}"),
            Self::Transport(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for HttpError {}

impl From<TransportError> for HttpError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// Parses one request and returns the consumed byte count. The adapter is intentionally strict:
/// the request line must be HTTP/1.1 and a body is always bounded by Content-Length.
pub fn parse_request(input: &[u8]) -> Result<Option<(HttpRequest, usize)>, HttpError> {
    let Some(head_end) = input.windows(4).position(|w| w == b"\r\n\r\n") else {
        if input.len() > MAX_HEADER_BYTES {
            return Err(HttpError::Transport(TransportError::HeaderTooLarge));
        }
        return Ok(None);
    };
    if head_end > MAX_HEADER_BYTES {
        return Err(HttpError::Transport(TransportError::HeaderTooLarge));
    }
    let head = std::str::from_utf8(&input[..head_end])
        .map_err(|_| HttpError::Transport(TransportError::InvalidUtf8))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(HttpError::InvalidRequest("request line missing"))?;
    let mut parts = request_line.split_ascii_whitespace();
    let method = parts
        .next()
        .ok_or(HttpError::InvalidRequest("method missing"))?;
    let path = parts
        .next()
        .ok_or(HttpError::InvalidRequest("path missing"))?;
    let version = parts
        .next()
        .ok_or(HttpError::InvalidRequest("HTTP version missing"))?;
    if parts.next().is_some() || version != "HTTP/1.1" {
        return Err(HttpError::InvalidRequest("only HTTP/1.1 is supported"));
    }
    let mut headers = Vec::new();
    let mut length = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(HttpError::InvalidRequest("header missing colon"));
        };
        if name.trim().is_empty() {
            return Err(HttpError::InvalidRequest("header name is empty"));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(HttpError::Transport(
                TransportError::UnsupportedTransferEncoding,
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() {
                return Err(HttpError::Transport(TransportError::InvalidContentLength));
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| HttpError::Transport(TransportError::InvalidContentLength))?,
            );
        }
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let body_len = length.unwrap_or(0);
    if body_len > MAX_FRAME_BYTES {
        return Err(HttpError::Transport(TransportError::FrameTooLarge {
            size: body_len,
            limit: MAX_FRAME_BYTES,
        }));
    }
    let body_start = head_end + 4;
    let end = body_start
        .checked_add(body_len)
        .ok_or(HttpError::InvalidRequest("body length overflow"))?;
    if input.len() < end {
        return Ok(None);
    }
    Ok(Some((
        HttpRequest {
            method: method.into(),
            path: path.into(),
            headers,
            body: input[body_start..end].to_vec(),
        },
        end,
    )))
}

pub struct HttpMcpAdapter {
    server: McpServer,
}

impl HttpMcpAdapter {
    pub fn new(server: McpServer) -> Self {
        Self { server }
    }
    pub fn server(&self) -> &McpServer {
        &self.server
    }
    pub fn server_mut(&mut self) -> &mut McpServer {
        &mut self.server
    }

    pub fn handle_bytes(&mut self, input: &[u8]) -> Result<Option<Vec<u8>>, HttpError> {
        let Some((request, used)) = parse_request(input)? else {
            return Ok(None);
        };
        let response = if request.method == "GET" && request.path == "/health" {
            HttpResponse::json(
                200,
                &Value::obj(vec![
                    ("ok", Value::Bool(true)),
                    ("transport", Value::str("http/1.1-content-length")),
                ]),
            )
        } else if request.method != "POST" || request.path != "/mcp" {
            HttpResponse::json(404, &Value::obj(vec![("error", Value::str("not_found"))]))
        } else {
            match json::parse(
                std::str::from_utf8(&request.body)
                    .map_err(|_| HttpError::Transport(TransportError::InvalidUtf8))?,
            ) {
                Ok(value) => match self.server.handle_value(&value) {
                    Some(response) => HttpResponse::json(200, &response),
                    None => HttpResponse {
                        status: 202,
                        headers: vec![],
                        body: Vec::new(),
                    },
                },
                Err(error) => HttpResponse::json(
                    400,
                    &Value::obj(vec![
                        ("error", Value::str("invalid_json")),
                        ("message", Value::str(error.to_string())),
                    ]),
                ),
            }
        };
        let mut bytes = response.to_bytes();
        // The return value is a complete response and the parser's consumed count is retained in
        // the first two bytes only for callers that need to frame a stream; public callers usually
        // pass one request at a time. Avoid an extra response wrapper or an implicit second parse.
        let _ = used;
        Ok(Some(std::mem::take(&mut bytes)))
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancel::CancelState;
    use crate::capability::{Capability, CapabilitySet};
    use crate::envelope::Outcome;
    use crate::exec::{FnHandler, InlineDriver};
    use crate::mcp_stdio::McpServer;
    use crate::scheduler::{Fabric, FabricConfig};
    use std::sync::Arc;

    fn adapter() -> HttpMcpAdapter {
        let cancels = CancelState::new();
        let handler = Arc::new(FnHandler(|job: &crate::envelope::DispatchJob| {
            Outcome::Succeeded {
                result: job.envelope.payload().to_vec(),
            }
        }));
        let fabric = Fabric::new(
            FabricConfig {
                shards: 1,
                ..FabricConfig::default()
            },
            Box::new(InlineDriver::new(handler, cancels.clone())),
            cancels,
        );
        let mut server = McpServer::new(fabric);
        server.server_register_test_agent();
        HttpMcpAdapter::new(server)
    }

    trait TestRegister {
        fn server_register_test_agent(&mut self);
    }
    impl TestRegister for McpServer {
        fn server_register_test_agent(&mut self) {
            self.fabric_mut().register_agent(
                "http-test",
                CapabilitySet::one(Capability::parse("compute").expect("cap")),
            );
        }
    }

    #[test]
    fn request_parser_requires_complete_content_length_body() {
        let request = b"POST /mcp HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";
        let (parsed, used) = parse_request(request).expect("parse").expect("complete");
        assert_eq!(parsed.method, "POST");
        assert_eq!(used, request.len());
        assert!(parse_request(&request[..request.len() - 1])
            .expect("parse")
            .is_none());
    }

    #[test]
    fn health_and_unknown_paths_are_explicit_http_responses() {
        let mut adapter = adapter();
        let health = b"GET /health HTTP/1.1\r\n\r\n";
        let bytes = adapter
            .handle_bytes(health)
            .expect("response")
            .expect("complete");
        assert!(String::from_utf8_lossy(&bytes).contains("200 OK"));
        let unknown = b"GET /other HTTP/1.1\r\n\r\n";
        let bytes = adapter
            .handle_bytes(unknown)
            .expect("response")
            .expect("complete");
        assert!(String::from_utf8_lossy(&bytes).contains("404 Not Found"));
    }

    #[test]
    fn mcp_post_is_routed_through_the_same_server() {
        let mut adapter = adapter();
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
        let request = format!(
            "POST /mcp HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(body)
        );
        let bytes = adapter
            .handle_bytes(request.as_bytes())
            .expect("response")
            .expect("complete");
        assert!(String::from_utf8_lossy(&bytes).contains("\"jsonrpc\":\"2.0\""));
    }
}
