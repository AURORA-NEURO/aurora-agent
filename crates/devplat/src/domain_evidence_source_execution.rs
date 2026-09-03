//! Bounded execution for the external evidence source-plan seam.
//!
//! The planner deliberately does not fetch anything. This module is the small, auditable
//! execution kernel that can consume a retained plan without turning a locator into provenance:
//! local files are confined to a caller-owned root, plain HTTP is opt-in and host-allowlisted,
//! redirects and HTTPS are refused because this offline workspace has no TLS client, and every
//! accepted byte stream receives both a raw-byte digest and a bounded JSON response projection.
//! Unsupported connector families become explicit `refused` outcomes instead of pretending that
//! a future provider adapter ran.

use crate::domain_evidence_source::{
    validate_domain_evidence_source_plan, DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

pub const DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-evidence-source-execution/0.1";
pub const DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW: &str = "domain_evidence_source_execute";
pub const MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_PREVIEW_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainEvidenceSourceExecutionError {
    #[error("domain evidence source execution plan is invalid: {0}")]
    InvalidPlan(String),
    #[error("domain evidence source execution plan field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain evidence source execution could not be canonicalised: {0}")]
    Canonicalisation(String),
}

#[derive(Debug)]
enum FetchResult {
    Content {
        bytes: Vec<u8>,
        content_type: String,
        http_status: Option<u16>,
        outcome: &'static str,
    },
    Refused {
        reason: String,
    },
    Error {
        reason: String,
    },
}

/// Execute one retained source plan against a caller-owned root.
///
/// The function returns a structured refusal or transport error for connector work that could
/// not proceed. It only returns `Err` for malformed retained plans; callers can therefore retain
/// policy refusals and transport failures as explicit intake outcomes rather than collapsing them
/// into an absent result.
pub fn execute_domain_evidence_source(
    root: &Path,
    plan: &Value,
) -> Result<Value, DomainEvidenceSourceExecutionError> {
    validate_domain_evidence_source_plan(plan)
        .map_err(|error| DomainEvidenceSourceExecutionError::InvalidPlan(error.to_string()))?;
    let object = plan
        .as_object()
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("plan".into()))?;
    let connector_kind = object
        .get("connector_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("connector_kind".into()))?;
    let locator_kind = object
        .get("locator_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("locator_kind".into()))?;
    let locator = object
        .get("locator")
        .and_then(Value::as_str)
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("locator".into()))?;
    let retrieval_mode = object
        .get("retrieval_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("retrieval_mode".into()))?;
    let policy = object
        .get("retrieval_policy")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainEvidenceSourceExecutionError::InvalidField("retrieval_policy".into())
        })?;
    let max_bytes = policy
        .get("max_bytes")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            DomainEvidenceSourceExecutionError::InvalidField("retrieval_policy.max_bytes".into())
        })?;
    let timeout_ms = policy
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            DomainEvidenceSourceExecutionError::InvalidField("retrieval_policy.timeout_ms".into())
        })?;
    let plan_digest = object
        .get("plan_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| DomainEvidenceSourceExecutionError::InvalidField("plan_digest".into()))?;
    let expected_content_digest = object
        .get("expected_content_digest")
        .and_then(Value::as_str);

    let fetch = if retrieval_mode != "content" {
        FetchResult::Refused {
            reason: format!(
                "in-process connectors require retrieval_mode=content; {retrieval_mode:?} remains caller-managed"
            ),
        }
    } else {
        match (connector_kind, locator_kind) {
            ("file", "path") => fetch_file(root, locator, max_bytes),
            ("generic_http", "uri") => fetch_http(locator, policy, max_bytes, timeout_ms),
            ("file", _) => FetchResult::Refused {
                reason: "file connector requires locator_kind=path".into(),
            },
            ("generic_http", _) => FetchResult::Refused {
                reason: "generic_http connector requires locator_kind=uri".into(),
            },
            _ => FetchResult::Refused {
                reason: format!(
                    "connector kind {connector_kind:?} has no in-process adapter; use a caller-managed connector"
                ),
            },
        }
    };

    let (outcome, execution, response, raw_content_digest, byte_length, content_type, http_status) =
        match fetch {
            FetchResult::Content {
                bytes,
                content_type,
                http_status,
                outcome,
            } => {
                let raw_content_digest = ContentHash::of_bytes(&bytes).to_string();
                let byte_length = bytes.len();
                let digest_mismatch =
                    expected_content_digest.filter(|expected| *expected != raw_content_digest);
                let response = if let Some(expected) = digest_mismatch {
                    digest_mismatch_response(
                        plan_digest,
                        expected,
                        &raw_content_digest,
                        byte_length,
                        &content_type,
                    )
                } else {
                    content_response(
                        plan_digest,
                        outcome,
                        byte_length,
                        &raw_content_digest,
                        &content_type,
                        &bytes,
                    )
                };
                let outcome = if digest_mismatch.is_some() {
                    "refused"
                } else {
                    outcome
                };
                (
                    outcome,
                    "completed",
                    response,
                    Some(raw_content_digest),
                    Some(byte_length),
                    Some(content_type),
                    http_status,
                )
            }
            FetchResult::Refused { reason } => {
                let response = status_response(plan_digest, "refused", &reason);
                ("refused", "refused", response, None, None, None, None)
            }
            FetchResult::Error { reason } => {
                let response = status_response(plan_digest, "error", &reason);
                ("error", "completed", response, None, None, None, None)
            }
        };
    let response_digest = ContentHash::of_value(&response)
        .map_err(|error| DomainEvidenceSourceExecutionError::Canonicalisation(error.to_string()))?;
    Ok(json!({
        "schema": DOMAIN_EVIDENCE_SOURCE_EXECUTION_SCHEMA_VERSION,
        "workflow": DOMAIN_EVIDENCE_SOURCE_EXECUTION_WORKFLOW,
        "source_plan_schema": DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
        "source_plan_digest": plan_digest,
        "group_id": object.get("group_id"),
        "domains": object.get("domains"),
        "subject_id": object.get("subject_id"),
        "connector_kind": connector_kind,
        "locator_kind": locator_kind,
        "retrieval_mode": retrieval_mode,
        "expected_content_digest": expected_content_digest,
        "outcome": outcome,
        "retrieval_status": outcome,
        "execution": execution,
        "http_status": http_status,
        "content_type": content_type,
        "byte_length": byte_length,
        "raw_content_digest": raw_content_digest,
        "response_digest": response_digest.to_string(),
        "response": response,
        "readiness_claimed": false,
        "guarantees": [
            "accepted local bytes were read under the retained max_bytes bound and confined to the caller-owned root",
            "accepted network bytes used an explicit enabled policy, exact host allow-list, bounded timeout, and no redirect following",
            "raw content and the bounded JSON response projection have separate exact SHA-256 identities"
        ],
        "does_not_claim": [
            "a successful read proves that a locator is authentic, current, scientifically valid, clinically valid, or provenance-complete",
            "a transport response proves the named capability-group tool executed or interpreted the content",
            "unsupported connectors, policy refusals, and transport errors are equivalent to an observed result"
        ]
    }))
}

fn fetch_file(root: &Path, locator: &str, max_bytes: usize) -> FetchResult {
    let path = match resolve_file(root, locator) {
        Ok(path) => path,
        Err(reason) => return FetchResult::Refused { reason },
    };
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            return FetchResult::Refused {
                reason: format!("file locator is not a regular file: {}", path.display()),
            }
        }
        Err(error) => {
            return FetchResult::Error {
                reason: format!("cannot inspect file {}: {error}", path.display()),
            }
        }
    };
    if metadata.len() > max_bytes as u64 {
        return FetchResult::Refused {
            reason: format!(
                "file is {} bytes, above the planned {}-byte bound",
                metadata.len(),
                max_bytes
            ),
        };
    }
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(error) => {
            return FetchResult::Error {
                reason: format!("cannot open file {}: {error}", path.display()),
            }
        }
    };
    let Some(bytes) = read_bounded(&mut file, max_bytes) else {
        return FetchResult::Refused {
            reason: format!("file changed while reading and exceeded the {max_bytes}-byte bound"),
        };
    };
    FetchResult::Content {
        content_type: file_content_type(&path, &bytes),
        bytes,
        http_status: None,
        outcome: "observed",
    }
}

fn resolve_file(root: &Path, locator: &str) -> Result<PathBuf, String> {
    let relative = Path::new(locator);
    if relative.is_absolute() {
        return Err("absolute file locators are refused".into());
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("file locator traversal or drive prefixes are refused".into());
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("source root cannot be canonicalised: {error}"))?;
    let candidate = canonical_root.join(relative);
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("file locator cannot be resolved: {error}"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err("file locator resolves outside the source root".into());
    }
    Ok(canonical)
}

fn fetch_http(
    locator: &str,
    policy: &serde_json::Map<String, Value>,
    max_bytes: usize,
    timeout_ms: u64,
) -> FetchResult {
    if policy.get("network").and_then(Value::as_str) != Some("enabled") {
        return FetchResult::Refused {
            reason: "network connector requires retrieval_policy.network=enabled".into(),
        };
    }
    let (host, port, target, tls) = match parse_http_locator(locator) {
        Ok(value) => value,
        Err(reason) => return FetchResult::Refused { reason },
    };
    if tls {
        return FetchResult::Refused {
            reason: "https locators are refused because the offline connector has no TLS client"
                .into(),
        };
    }
    let allowed_hosts = policy
        .get("allowed_hosts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if !allowed_hosts.iter().any(|allowed| *allowed == host) {
        return FetchResult::Refused {
            reason: format!("HTTP host {host:?} is not in retrieval_policy.allowed_hosts"),
        };
    }
    let timeout = Duration::from_millis(timeout_ms);
    let address = match (host.as_str(), port).to_socket_addrs() {
        Ok(mut addresses) => match addresses.next() {
            Some(address) => address,
            None => {
                return FetchResult::Error {
                    reason: "HTTP host resolved to no address".into(),
                }
            }
        },
        Err(error) => {
            return FetchResult::Error {
                reason: format!("HTTP host resolution failed: {error}"),
            }
        }
    };
    let mut stream = match TcpStream::connect_timeout(&address, timeout) {
        Ok(stream) => stream,
        Err(error) => {
            return FetchResult::Error {
                reason: format!("HTTP connection failed: {error}"),
            }
        }
    };
    if let Err(error) = stream.set_read_timeout(Some(timeout)) {
        return FetchResult::Error {
            reason: format!("HTTP read timeout could not be configured: {error}"),
        };
    }
    if let Err(error) = stream.set_write_timeout(Some(timeout)) {
        return FetchResult::Error {
            reason: format!("HTTP write timeout could not be configured: {error}"),
        };
    }
    let request = format!(
        "GET {target} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json, text/plain, */*\r\nConnection: close\r\n\r\n"
    );
    if let Err(error) = stream.write_all(request.as_bytes()) {
        return FetchResult::Error {
            reason: format!("HTTP request write failed: {error}"),
        };
    }
    let (headers, mut body) = match read_http_headers(&mut stream) {
        Ok(value) => value,
        Err(reason) => return FetchResult::Error { reason },
    };
    let status = match headers
        .first()
        .and_then(|line| parse_http_status(line).ok())
    {
        Some(status) => status,
        None => {
            return FetchResult::Error {
                reason: "HTTP response status line is invalid".into(),
            }
        }
    };
    let transfer_encoding = header(&headers, "transfer-encoding");
    if transfer_encoding
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"))
    {
        return FetchResult::Refused {
            reason: "HTTP transfer encodings other than identity are refused".into(),
        };
    }
    let content_length = match header(&headers, "content-length") {
        None => None,
        Some(value) => match parse_http_content_length(&value) {
            Ok(value) => Some(value),
            Err(_) => {
                return FetchResult::Error {
                    reason: "HTTP Content-Length is invalid".into(),
                }
            }
        },
    };
    if content_length.is_some_and(|length| length > max_bytes) {
        return FetchResult::Refused {
            reason: format!("HTTP response exceeds the planned {max_bytes}-byte bound"),
        };
    }
    if content_length.is_some_and(|length| body.len() > length) {
        return FetchResult::Refused {
            reason: "HTTP response contains bytes beyond its declared Content-Length".into(),
        };
    }
    let body_result = read_http_body(&mut stream, &mut body, content_length, max_bytes);
    let Some(body) = body_result else {
        return FetchResult::Refused {
            reason: format!("HTTP response exceeded the planned {max_bytes}-byte bound"),
        };
    };
    let content_type = header(&headers, "content-type")
        .unwrap_or_else(|| "application/octet-stream".into())
        .chars()
        .take(512)
        .collect::<String>();
    if content_type.chars().any(char::is_control) {
        return FetchResult::Error {
            reason: "HTTP Content-Type contains control characters".into(),
        };
    }
    if (200..300).contains(&status) {
        FetchResult::Content {
            bytes: body,
            content_type,
            http_status: Some(status),
            outcome: if status == 206 { "partial" } else { "observed" },
        }
    } else if (300..400).contains(&status) {
        FetchResult::Refused {
            reason: format!("HTTP redirect status {status} was not followed"),
        }
    } else {
        FetchResult::Error {
            reason: format!("HTTP source returned status {status}"),
        }
    }
}

fn parse_http_locator(locator: &str) -> Result<(String, u16, String, bool), String> {
    let (tls, prefix) = if let Some(rest) = locator.strip_prefix("http://") {
        (false, rest)
    } else if let Some(rest) = locator.strip_prefix("https://") {
        (true, rest)
    } else {
        return Err("HTTP locator must use http:// or https://".into());
    };
    let authority_end = prefix.find(['/', '?', '#']).unwrap_or(prefix.len());
    let authority = &prefix[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Err("HTTP locator authority is empty or contains credentials".into());
    }
    let target = if authority_end == prefix.len() {
        "/".into()
    } else {
        let target = &prefix[authority_end..];
        if target.contains('#') {
            return Err("HTTP locator fragments are refused".into());
        } else if target.starts_with('?') {
            format!("/{target}")
        } else {
            target.to_string()
        }
    };
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() || port.is_empty() {
            return Err("HTTP locator has an invalid host or port".into());
        }
        let port = port
            .parse::<u16>()
            .map_err(|_| "HTTP locator port is invalid".to_string())?;
        if port == 0 {
            return Err("HTTP locator port must be non-zero".into());
        }
        (host.to_ascii_lowercase(), port)
    } else {
        (authority.to_ascii_lowercase(), if tls { 443 } else { 80 })
    };
    let host = host.trim_end_matches('.').to_string();
    if host.is_empty()
        || host.contains(['/', '?', '#', ' ', '[', ']', ':', '\\'])
        || host.chars().any(char::is_control)
    {
        return Err("HTTP locator host is invalid".into());
    }
    if target.chars().any(char::is_control) || target.contains('\\') || !target.starts_with('/') {
        return Err("HTTP locator target is invalid".into());
    }
    Ok((host, port, target, tls))
}

fn parse_http_status(status_line: &str) -> Result<u16, String> {
    let mut fields = status_line.splitn(3, ' ');
    let version = fields.next().unwrap_or_default();
    let status = fields.next().unwrap_or_default();
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || status.len() != 3
        || !status.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("HTTP response status line is invalid".into());
    }
    status
        .parse::<u16>()
        .map_err(|_| "HTTP response status line is invalid".into())
}

fn parse_http_content_length(value: &str) -> Result<usize, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("HTTP Content-Length is invalid".into());
    }
    value
        .parse::<usize>()
        .map_err(|_| "HTTP Content-Length is invalid".into())
}

fn validate_http_headers(lines: &[String]) -> Result<(), String> {
    let status_line = lines
        .first()
        .ok_or_else(|| "HTTP response status line is invalid".to_string())?;
    parse_http_status(status_line)?;
    let mut seen = std::collections::BTreeSet::new();
    for line in lines.iter().skip(1) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "HTTP response header is missing a colon".to_string())?;
        if name.is_empty()
            || name != name.trim()
            || !name.bytes().all(is_http_token_byte)
            || value.chars().any(char::is_control)
        {
            return Err("HTTP response contains an invalid header".into());
        }
        let normalized_name = name.to_ascii_lowercase();
        if matches!(
            normalized_name.as_str(),
            "content-length" | "transfer-encoding" | "content-type" | "location"
        ) && !seen.insert(normalized_name)
        {
            return Err("HTTP response contains a duplicate security-sensitive header".into());
        }
    }
    Ok(())
}

fn is_http_token_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9'
            | b'a'..=b'z'
            | b'A'..=b'Z'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
    )
}

fn read_http_headers(stream: &mut TcpStream) -> Result<(Vec<String>, Vec<u8>), String> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|error| format!("HTTP response read failed: {error}"))?;
        if read == 0 {
            return Err("HTTP response ended before headers".into());
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_bytes = &bytes[..index];
            let remainder = bytes[index + 4..].to_vec();
            if header_bytes.len() > MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_HEADER_BYTES {
                return Err("HTTP response headers exceed the safety bound".into());
            }
            let text = String::from_utf8(header_bytes.to_vec())
                .map_err(|_| "HTTP response headers are not valid UTF-8".to_string())?;
            let lines = text.split("\r\n").map(str::to_string).collect::<Vec<_>>();
            validate_http_headers(&lines)?;
            return Ok((lines, remainder));
        }
        if bytes.len() > MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_HEADER_BYTES {
            return Err("HTTP response headers exceed the safety bound".into());
        }
    }
}

fn read_http_body(
    stream: &mut TcpStream,
    body: &mut Vec<u8>,
    content_length: Option<usize>,
    max_bytes: usize,
) -> Option<Vec<u8>> {
    if let Some(length) = content_length {
        while body.len() < length {
            let mut chunk = [0_u8; 8192];
            let read = stream
                .read(&mut chunk)
                .ok()?
                .min(length.saturating_sub(body.len()));
            if read == 0 {
                return None;
            }
            body.extend_from_slice(&chunk[..read]);
        }
        body.truncate(length);
        return (body.len() <= max_bytes).then(|| std::mem::take(body));
    }
    loop {
        if body.len() > max_bytes {
            return None;
        }
        let mut chunk = [0_u8; 8192];
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return Some(std::mem::take(body));
        }
        body.extend_from_slice(&chunk[..read]);
        if body.len() > max_bytes {
            return None;
        }
    }
}

fn header(lines: &[String], wanted: &str) -> Option<String> {
    lines.iter().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case(wanted)
            .then(|| value.trim().to_string())
    })
}

fn read_bounded(reader: &mut impl Read, max_bytes: usize) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let remaining = max_bytes.saturating_add(1).saturating_sub(bytes.len());
        if remaining == 0 {
            return None;
        }
        let mut chunk = vec![0_u8; remaining.min(8192)];
        let read = reader.read(&mut chunk).ok()?;
        if read == 0 {
            return Some(bytes);
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > max_bytes {
            return None;
        }
    }
}

fn file_content_type(path: &Path, bytes: &[u8]) -> String {
    if serde_json::from_slice::<Value>(bytes).is_ok() {
        return "application/json".into();
    }
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("csv") => "text/csv".into(),
        Some("tsv") => "text/tab-separated-values".into(),
        Some("txt" | "md" | "rs" | "py" | "ts") => "text/plain".into(),
        _ => "application/octet-stream".into(),
    }
}

fn content_response(
    plan_digest: &str,
    outcome: &str,
    byte_length: usize,
    raw_content_digest: &str,
    content_type: &str,
    bytes: &[u8],
) -> Value {
    let mut retrieval = json!({
        "status": outcome,
        "byte_length": byte_length,
        "raw_content_digest": raw_content_digest,
        "content_type": content_type,
        "body_encoding": "omitted",
        "body_truncated": false
    });
    if bytes.is_empty() {
        retrieval["body_encoding"] = json!("empty");
    } else if bytes.len() <= MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_PREVIEW_BYTES {
        if let Ok(value) = serde_json::from_slice::<Value>(bytes) {
            retrieval["body_encoding"] = json!("json");
            retrieval["body"] = value;
        } else if let Ok(text) = std::str::from_utf8(bytes) {
            retrieval["body_encoding"] = json!("utf8");
            retrieval["body"] = json!(text);
        } else {
            retrieval["body_encoding"] = json!("binary");
        }
    } else {
        retrieval["body_truncated"] = json!(true);
        if let Ok(text) =
            std::str::from_utf8(&bytes[..MAX_DOMAIN_EVIDENCE_SOURCE_EXECUTION_PREVIEW_BYTES])
        {
            retrieval["body_encoding"] = json!("utf8_preview");
            retrieval["body_preview"] = json!(text);
        } else {
            retrieval["body_encoding"] = json!("binary");
        }
    }
    json!({
        "source_plan_digest": plan_digest,
        "retrieval": retrieval
    })
}

fn status_response(plan_digest: &str, status: &str, reason: &str) -> Value {
    json!({
        "source_plan_digest": plan_digest,
        "retrieval": {
            "status": status,
            "reason": reason,
            "byte_length": null,
            "raw_content_digest": null,
            "body_encoding": "omitted",
            "body_truncated": false
        }
    })
}

fn digest_mismatch_response(
    plan_digest: &str,
    expected_content_digest: &str,
    raw_content_digest: &str,
    byte_length: usize,
    content_type: &str,
) -> Value {
    json!({
        "source_plan_digest": plan_digest,
        "retrieval": {
            "status": "refused",
            "reason": "retrieved bytes do not match expected_content_digest",
            "expected_content_digest": expected_content_digest,
            "raw_content_digest": raw_content_digest,
            "byte_length": byte_length,
            "content_type": content_type,
            "body_encoding": "omitted",
            "body_truncated": false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_evidence_source::plan_domain_evidence_source;
    use std::fs;

    fn plan(locator_kind: &str, connector_kind: &str, locator: &str) -> Value {
        plan_domain_evidence_source(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-test",
            "source_tool": "modality_catalog",
            "connector_kind": connector_kind,
            "locator_kind": locator_kind,
            "locator": locator,
            "retrieval_mode": "content",
            "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "no_cache"},
            "does_not_claim": ["source truth", "scientific validity"]
        }))
        .unwrap()
    }

    #[test]
    fn file_execution_is_bounded_digest_addressed_and_json_projected() {
        let root =
            std::env::temp_dir().join(format!("bioprism-source-execution-{}", std::process::id()));
        let _ = fs::create_dir_all(&root);
        let path = root.join("response.json");
        fs::write(&path, br#"{"status":"bounded","items":[1,2]}"#).unwrap();
        let plan = plan("path", "file", "response.json");
        let first = execute_domain_evidence_source(&root, &plan).unwrap();
        let second = execute_domain_evidence_source(&root, &plan).unwrap();
        assert_eq!(first["response"], second["response"]);
        assert_eq!(first["outcome"], "observed");
        assert_eq!(first["response"]["retrieval"]["body_encoding"], "json");
        assert_eq!(first["raw_content_digest"].as_str().unwrap().len(), 64);
        assert_eq!(first["response_digest"].as_str().unwrap().len(), 64);
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn file_execution_refuses_traversal_and_unsupported_connectors_without_error_collapse() {
        let root = std::env::temp_dir();
        let traversal = plan("path", "file", "../outside.json");
        let refused = execute_domain_evidence_source(&root, &traversal).unwrap();
        assert_eq!(refused["outcome"], "refused");
        assert!(refused["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("traversal"));
        let unsupported = plan("uri", "literature", "https://example.org/article");
        let unsupported = execute_domain_evidence_source(&root, &unsupported).unwrap();
        assert_eq!(unsupported["outcome"], "refused");
        assert!(unsupported["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("no in-process adapter"));
    }

    #[test]
    fn file_execution_refuses_expected_content_digest_mismatch_with_actual_digest() {
        let root = std::env::temp_dir().join(format!(
            "bioprism-source-execution-expected-digest-{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&root);
        let path = root.join("response.json");
        fs::write(&path, br#"{"status":"unexpected"}"#).unwrap();
        let plan = plan_domain_evidence_source(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-expected-digest",
            "source_tool": "modality_catalog",
            "connector_kind": "file",
            "locator_kind": "path",
            "locator": "response.json",
            "retrieval_mode": "content",
            "expected_content_digest": "f".repeat(64),
            "retrieval_policy": {"network": "disabled", "max_bytes": 4096},
            "does_not_claim": ["source truth"]
        }))
        .unwrap();
        let result = execute_domain_evidence_source(&root, &plan).unwrap();
        assert_eq!(result["outcome"], "refused");
        assert_eq!(result["response"]["retrieval"]["status"], "refused");
        assert!(result["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("expected_content_digest"));
        assert_eq!(
            result["response"]["retrieval"]["raw_content_digest"],
            result["raw_content_digest"]
        );
        assert_ne!(result["raw_content_digest"], "f".repeat(64));
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn https_and_non_enabled_networks_are_refused_before_io() {
        let mut source = json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-http",
            "source_tool": "modality_catalog",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": "https://example.org/data",
            "retrieval_mode": "content",
            "retrieval_policy": {"network": "enabled", "allowed_hosts": ["example.org"], "max_bytes": 4096},
            "does_not_claim": ["source truth"]
        });
        source = plan_domain_evidence_source(&source).unwrap();
        let result = execute_domain_evidence_source(Path::new("."), &source).unwrap();
        assert_eq!(result["outcome"], "refused");
        assert!(result["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("TLS"));
    }

    #[test]
    fn http_locator_parser_rejects_ambiguous_authorities_and_targets() {
        for locator in [
            "http://example.org:0/data",
            "http://example.org:80:90/data",
            "http://example.org\\data",
            "http://example.org/data\\more",
            "http://example.org/data#fragment",
            "http://example.org#fragment",
        ] {
            assert!(parse_http_locator(locator).is_err(), "accepted {locator:?}");
        }
        assert_eq!(
            parse_http_locator("http://example.org/data").unwrap(),
            ("example.org".into(), 80, "/data".into(), false)
        );
        assert_eq!(
            parse_http_locator("http://EXAMPLE.ORG.:80/data").unwrap(),
            ("example.org".into(), 80, "/data".into(), false)
        );
    }

    #[test]
    fn http_response_headers_reject_ambiguous_or_noncanonical_metadata() {
        assert!(validate_http_headers(&[
            "HTTP/1.1 200 OK".into(),
            "Content-Length: 1".into(),
            "content-length: 1".into(),
        ])
        .is_err());
        assert!(
            validate_http_headers(&["HTTP/2 200 OK".into(), "Content-Length: 1".into(),]).is_err()
        );
        assert!(
            validate_http_headers(&["HTTP/1.1 200 OK".into(), "Content-Length : 1".into(),])
                .is_err()
        );
        assert!(parse_http_content_length("+1").is_err());
        assert!(parse_http_content_length("1.0").is_err());
        assert_eq!(parse_http_content_length("001").unwrap(), 1);
    }

    #[test]
    fn allowlisted_plain_http_is_bounded_and_digest_addressed_without_redirects() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 30\r\nConnection: close\r\n\r\n{\"status\":\"allowlisted\",\"n\":1}",
                )
                .unwrap();
        });
        let plan = plan_domain_evidence_source(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-http-success",
            "source_tool": "modality_catalog",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": format!("http://127.0.0.1:{}/payload", address.port()),
            "retrieval_mode": "content",
            "retrieval_policy": {
                "network": "enabled",
                "allowed_hosts": ["127.0.0.1"],
                "max_bytes": 4096,
                "timeout_ms": 2000,
                "cache": "no_cache"
            },
            "does_not_claim": ["source truth"]
        }))
        .unwrap();
        let result = execute_domain_evidence_source(Path::new("."), &plan).unwrap();
        worker.join().unwrap();
        assert_eq!(result["outcome"], "observed");
        assert_eq!(result["http_status"], 200);
        assert_eq!(result["content_type"], "application/json");
        assert_eq!(result["response"]["retrieval"]["body_encoding"], "json");
        assert_eq!(result["raw_content_digest"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn http_content_length_overrun_is_refused_instead_of_truncated() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nab")
                .unwrap();
        });
        let plan = plan_domain_evidence_source(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "source-execution-overrun",
            "source_tool": "modality_catalog",
            "connector_kind": "generic_http",
            "locator_kind": "uri",
            "locator": format!("http://127.0.0.1:{}/payload", address.port()),
            "retrieval_mode": "content",
            "retrieval_policy": {
                "network": "enabled",
                "allowed_hosts": ["127.0.0.1"],
                "max_bytes": 4096,
                "timeout_ms": 2000,
                "cache": "no_cache"
            },
            "does_not_claim": ["source truth"]
        }))
        .unwrap();
        let result = execute_domain_evidence_source(Path::new("."), &plan).unwrap();
        worker.join().unwrap();
        assert_eq!(result["outcome"], "refused");
        assert!(result["response"]["retrieval"]["reason"]
            .as_str()
            .unwrap()
            .contains("Content-Length"));
    }
}
