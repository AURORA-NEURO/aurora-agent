//! The deliberately small HTTP/1.1 boundary used by `bioprism-api`.
//!
//! The API is an integration boundary, not a general-purpose web framework.  Owning the parser
//! here makes the important limits visible: one request per connection, no chunked encoding, one
//! unambiguous `Content-Length`, and a hard header/body bound before JSON parsing starts.  A reverse
//! proxy can provide HTTP/2, TLS, compression, and connection pooling in front of this boundary;
//! none of those are silently implied by the in-process router.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead, Write};

#[derive(Debug)]
pub enum HttpError {
    Io(io::Error),
    Invalid(String),
    HeaderTooLarge { maximum: usize },
    BodyTooLarge { maximum: usize },
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpError::Io(error) => write!(formatter, "I/O error: {error}"),
            HttpError::Invalid(message) => write!(formatter, "invalid HTTP request: {message}"),
            HttpError::HeaderTooLarge { maximum } => {
                write!(formatter, "HTTP headers exceed the {maximum}-byte bound")
            }
            HttpError::BodyTooLarge { maximum } => {
                write!(formatter, "HTTP body exceeds the {maximum}-byte bound")
            }
        }
    }
}

impl std::error::Error for HttpError {}

impl From<io::Error> for HttpError {
    fn from(error: io::Error) -> Self {
        HttpError::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub target: String,
    pub version: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }

    pub fn query(&self) -> Result<BTreeMap<String, String>, HttpError> {
        let Some((_, query)) = self.target.split_once('?') else {
            return Ok(BTreeMap::new());
        };
        let mut values = BTreeMap::new();
        if query.is_empty() {
            return Ok(values);
        }
        for item in query.split('&') {
            let (raw_key, raw_value) = item.split_once('=').unwrap_or((item, ""));
            let key = percent_decode(raw_key)?;
            let value = percent_decode(raw_value)?;
            if key.is_empty() || values.insert(key.clone(), value).is_some() {
                return Err(HttpError::Invalid(format!(
                    "query keys must be unique and non-empty: {key:?}"
                )));
            }
        }
        Ok(values)
    }

    pub fn path_segments(&self) -> Result<Vec<String>, HttpError> {
        self.path()
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(percent_decode)
            .collect()
    }
}

/// Reads exactly one HTTP request.  The caller owns the connection lifetime and should close it
/// after writing the response; this keeps request smuggling and pipelining outside this minimal
/// server's contract.
pub fn read_request<R: BufRead>(
    reader: &mut R,
    maximum_header_bytes: usize,
    maximum_body_bytes: usize,
) -> Result<HttpRequest, HttpError> {
    let mut header_bytes = 0usize;
    let mut lines = Vec::new();
    loop {
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            return Err(HttpError::Invalid("request ended before headers".into()));
        }
        header_bytes = header_bytes.saturating_add(read);
        if header_bytes > maximum_header_bytes {
            return Err(HttpError::HeaderTooLarge {
                maximum: maximum_header_bytes,
            });
        }
        if !line.ends_with(b"\n") {
            return Err(HttpError::Invalid("header line has no newline".into()));
        }
        let end = line
            .strip_suffix(b"\n")
            .and_then(|line| line.strip_suffix(b"\r"))
            .unwrap_or(line.as_slice());
        if end.is_empty() {
            break;
        }
        lines.push(line);
    }

    let request_line = std::str::from_utf8(
        lines
            .first()
            .ok_or_else(|| HttpError::Invalid("missing request line".into()))?,
    )
    .map_err(|_| HttpError::Invalid("request line is not UTF-8".into()))?
    .trim();
    let mut request_parts = request_line.split_ascii_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| HttpError::Invalid("missing method".into()))?;
    let target = request_parts
        .next()
        .ok_or_else(|| HttpError::Invalid("missing request target".into()))?;
    let version = request_parts
        .next()
        .ok_or_else(|| HttpError::Invalid("missing HTTP version".into()))?;
    if request_parts.next().is_some() {
        return Err(HttpError::Invalid("request line has extra fields".into()));
    }
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(HttpError::Invalid(format!(
            "unsupported HTTP version {version:?}"
        )));
    }
    if method.bytes().any(|byte| !byte.is_ascii_uppercase()) {
        return Err(HttpError::Invalid(
            "method must use uppercase token bytes".into(),
        ));
    }
    if target.is_empty() || !target.starts_with('/') {
        return Err(HttpError::Invalid(
            "request target must be an origin-form path".into(),
        ));
    }

    let mut headers = BTreeMap::new();
    for raw_line in lines.iter().skip(1) {
        let text = std::str::from_utf8(raw_line)
            .map_err(|_| HttpError::Invalid("header is not UTF-8".into()))?
            .trim();
        let (raw_name, raw_value) = text
            .split_once(':')
            .ok_or_else(|| HttpError::Invalid("header has no colon".into()))?;
        let name = raw_name.trim().to_ascii_lowercase();
        let value = raw_value.trim();
        if name.is_empty()
            || name
                .bytes()
                .any(|byte| !(byte.is_ascii_alphanumeric() || byte == b'-'))
            || value.bytes().any(|byte| byte == b'\r' || byte == b'\n')
        {
            return Err(HttpError::Invalid(format!("invalid header {raw_name:?}")));
        }
        if headers.insert(name.clone(), value.to_string()).is_some() {
            return Err(HttpError::Invalid(format!(
                "duplicate header {name:?} is refused"
            )));
        }
    }

    if headers.contains_key("transfer-encoding") {
        return Err(HttpError::Invalid(
            "Transfer-Encoding is unsupported; send a bounded Content-Length".into(),
        ));
    }
    let content_length = match headers.get("content-length") {
        None => 0,
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| HttpError::Invalid("Content-Length must be a decimal integer".into()))?,
    };
    if content_length > maximum_body_bytes {
        return Err(HttpError::BodyTooLarge {
            maximum: maximum_body_bytes,
        });
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    Ok(HttpRequest {
        method: method.to_string(),
        target: target.to_string(),
        version: version.to_string(),
        headers,
        body,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec());
        let mut response = Self::new(status, body);
        response.headers.insert(
            "content-type".into(),
            "application/json; charset=utf-8".into(),
        );
        response
    }

    pub fn text(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        let mut response = Self::new(status, body.into());
        response
            .headers
            .insert("content-type".into(), content_type.to_string());
        response
    }

    pub fn empty(status: u16) -> Self {
        Self::new(status, Vec::new())
    }

    fn new(status: u16, body: Vec<u8>) -> Self {
        HttpResponse {
            status,
            headers: BTreeMap::new(),
            body,
        }
    }

    pub fn with_header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.insert(name.to_ascii_lowercase(), value.into());
        self
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let reason = reason_phrase(self.status);
        write!(writer, "HTTP/1.1 {} {}\r\n", self.status, reason)?;
        for (name, value) in &self.headers {
            write!(writer, "{}: {}\r\n", canonical_header_name(name), value)?;
        }
        write!(writer, "Content-Length: {}\r\n", self.body.len())?;
        write!(writer, "Connection: close\r\n\r\n")?;
        writer.write_all(&self.body)
    }
}

fn canonical_header_name(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn percent_decode(value: &str) -> Result<String, HttpError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(HttpError::Invalid("truncated percent escape".into()));
            }
            let high = hex(bytes[index + 1])?;
            let low = hex(bytes[index + 2])?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output)
        .map_err(|_| HttpError::Invalid("percent-decoded text is not UTF-8".into()))
}

fn hex(value: u8) -> Result<u8, HttpError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(HttpError::Invalid("invalid percent escape".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn parser_enforces_unique_headers_and_bounded_body() {
        let raw = b"POST /v1/tools/foo?a=1&b=two%20words HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\n\r\n{\"x\":1}";
        let request = read_request(&mut Cursor::new(raw), 1024, 32).expect("request parses");
        assert_eq!(request.path(), "/v1/tools/foo");
        assert_eq!(request.query().unwrap()["b"], "two words");
        assert_eq!(request.body, b"{\"x\":1}");
        let duplicate = b"GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\n\r\n";
        assert!(read_request(&mut Cursor::new(duplicate), 1024, 32).is_err());
    }

    #[test]
    fn parser_rejects_chunked_and_over_limit_requests() {
        let chunked = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n";
        assert!(read_request(&mut Cursor::new(chunked), 1024, 32).is_err());
        let too_large = b"POST / HTTP/1.1\r\nContent-Length: 33\r\n\r\n";
        assert!(matches!(
            read_request(&mut Cursor::new(too_large), 1024, 32),
            Err(HttpError::BodyTooLarge { maximum: 32 })
        ));
    }

    #[test]
    fn response_has_explicit_length_and_close_semantics() {
        let response = HttpResponse::json(200, &serde_json::json!({"ok": true}));
        let mut bytes = Vec::new();
        response.write_to(&mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("Content-Length: 11\r\n"));
        assert!(text.contains("Connection: close\r\n"));
    }
}
