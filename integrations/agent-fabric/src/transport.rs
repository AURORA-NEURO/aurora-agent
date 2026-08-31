//! Shared, bounded wire framing for the integration adapters.
//!
//! The fabric keeps framing separate from protocol semantics.  A malformed or oversized frame
//! is rejected before JSON parsing, and a complete frame reports how many bytes it consumed so a
//! caller can safely retain bytes from a subsequent request.

use crate::json::{self, Value};
use std::fmt;

pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_HEADER_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    EmptyFrame,
    FrameTooLarge { size: usize, limit: usize },
    HeaderTooLarge,
    MissingContentLength,
    InvalidContentLength,
    UnsupportedTransferEncoding,
    Incomplete,
    InvalidUtf8,
    Json(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFrame => write!(f, "empty frame"),
            Self::FrameTooLarge { size, limit } => {
                write!(f, "frame is {size} bytes; limit is {limit}")
            }
            Self::HeaderTooLarge => write!(f, "header block exceeds the limit"),
            Self::MissingContentLength => write!(f, "Content-Length is required"),
            Self::InvalidContentLength => write!(f, "Content-Length is invalid"),
            Self::UnsupportedTransferEncoding => write!(f, "Transfer-Encoding is not supported"),
            Self::Incomplete => write!(f, "frame is incomplete"),
            Self::InvalidUtf8 => write!(f, "frame is not UTF-8"),
            Self::Json(message) => write!(f, "JSON frame is invalid: {message}"),
        }
    }
}

impl std::error::Error for TransportError {}

pub fn encode_line(value: &Value) -> String {
    let mut line = json::to_string(value);
    line.push('\n');
    line
}

pub fn decode_line(line: &str) -> Result<Value, TransportError> {
    if line.len() > MAX_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge {
            size: line.len(),
            limit: MAX_FRAME_BYTES,
        });
    }
    let text = line.trim_end_matches(['\r', '\n']);
    if text.trim().is_empty() {
        return Err(TransportError::EmptyFrame);
    }
    json::parse(text).map_err(|e| TransportError::Json(e.to_string()))
}

pub fn encode_content_length(value: &Value) -> Vec<u8> {
    let body = json::to_string(value);
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(body.as_bytes());
    frame
}

/// Decodes one Content-Length framed JSON value. `Ok(None)` means more bytes are needed.
pub fn decode_content_length(input: &[u8]) -> Result<Option<(Value, usize)>, TransportError> {
    let Some(header_end) = input.windows(4).position(|w| w == b"\r\n\r\n") else {
        if input.len() > MAX_HEADER_BYTES {
            return Err(TransportError::HeaderTooLarge);
        }
        return Ok(None);
    };
    let header_len = header_end + 4;
    if header_end > MAX_HEADER_BYTES {
        return Err(TransportError::HeaderTooLarge);
    }
    let header =
        std::str::from_utf8(&input[..header_end]).map_err(|_| TransportError::InvalidUtf8)?;
    let mut content_length = None;
    for line in header.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            return Err(TransportError::InvalidContentLength);
        };
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(TransportError::InvalidContentLength);
            }
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| TransportError::InvalidContentLength)?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(TransportError::UnsupportedTransferEncoding);
        }
    }
    let length = content_length.ok_or(TransportError::MissingContentLength)?;
    if length > MAX_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge {
            size: length,
            limit: MAX_FRAME_BYTES,
        });
    }
    let end = header_len
        .checked_add(length)
        .ok_or(TransportError::FrameTooLarge {
            size: usize::MAX,
            limit: MAX_FRAME_BYTES,
        })?;
    if input.len() < end {
        return Ok(None);
    }
    let body =
        std::str::from_utf8(&input[header_len..end]).map_err(|_| TransportError::InvalidUtf8)?;
    let value = json::parse(body).map_err(|e| TransportError::Json(e.to_string()))?;
    Ok(Some((value, end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_framing_is_compact_and_round_trips() {
        let line = encode_line(&Value::obj(vec![("ok", Value::Bool(true))]));
        assert_eq!(
            decode_line(&line).expect("frame"),
            Value::obj(vec![("ok", Value::Bool(true))])
        );
    }

    #[test]
    fn content_length_reports_incomplete_without_parsing_partial_json() {
        let frame = encode_content_length(&Value::obj(vec![("n", Value::Uint(1))]));
        assert!(decode_content_length(&frame[..frame.len() - 1])
            .expect("incomplete")
            .is_none());
        let (value, used) = decode_content_length(&frame)
            .expect("decode")
            .expect("complete");
        assert_eq!(used, frame.len());
        assert_eq!(value.get("n").and_then(Value::as_u64), Some(1));
    }

    #[test]
    fn duplicate_lengths_and_chunked_transfer_are_rejected() {
        let duplicate = b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(
            decode_content_length(duplicate),
            Err(TransportError::InvalidContentLength)
        );
        let chunked = b"Transfer-Encoding: chunked\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(
            decode_content_length(chunked),
            Err(TransportError::UnsupportedTransferEncoding)
        );
    }
}
