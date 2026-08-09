//! Minimal JSON-RPC 2.0 over newline-delimited stdio.
//!
//! Hand-rolled for the same reason the CLI parser is: the wire contract is small, and owning it
//! keeps the server dependency-free and its framing exactly specified.

use serde_json::{json, Value};

pub const JSONRPC: &str = "2.0";

/// Error codes. The first four are JSON-RPC standard; the last is ours.
pub mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}

#[derive(Debug, Clone)]
pub struct Request {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn parse(line: &str) -> Result<Request, Box<Response>> {
        let value: Value = serde_json::from_str(line).map_err(|e| {
            Box::new(Response::error(
                None,
                code::PARSE_ERROR,
                format!("invalid JSON: {e}"),
                None,
            ))
        })?;

        let method = value
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Box::new(Response::error(
                    value.get("id").cloned(),
                    code::INVALID_REQUEST,
                    "missing method".to_string(),
                    None,
                ))
            })?
            .to_string();

        Ok(Request {
            id: value.get("id").cloned(),
            method,
            params: value.get("params").cloned().unwrap_or(Value::Null),
        })
    }

    /// A notification carries no id and must not be answered.
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    pub fn param_str(&self, name: &str) -> Option<&str> {
        self.params.get(name).and_then(Value::as_str)
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub id: Option<Value>,
    pub payload: Result<Value, RpcError>,
}

#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl Response {
    pub fn result(id: Option<Value>, value: Value) -> Response {
        Response {
            id,
            payload: Ok(value),
        }
    }

    pub fn error(id: Option<Value>, code: i64, message: String, data: Option<Value>) -> Response {
        Response {
            id,
            payload: Err(RpcError {
                code,
                message,
                data,
            }),
        }
    }

    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("jsonrpc".into(), json!(JSONRPC));
        map.insert("id".into(), self.id.clone().unwrap_or(Value::Null));
        match &self.payload {
            Ok(value) => {
                map.insert("result".into(), value.clone());
            }
            Err(error) => {
                let mut body = serde_json::Map::new();
                body.insert("code".into(), json!(error.code));
                body.insert("message".into(), json!(error.message));
                if let Some(data) = &error.data {
                    body.insert("data".into(), data.clone());
                }
                map.insert("error".into(), Value::Object(body));
            }
        }
        Value::Object(map)
    }
}
