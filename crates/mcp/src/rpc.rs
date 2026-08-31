//! Minimal JSON-RPC 2.0 over newline-delimited stdio.
//!
//! Hand-rolled for the same reason the CLI parser is: the wire contract is small, and owning it
//! keeps the server dependency-free and its framing exactly specified.

use serde_json::{json, Value};

pub const JSONRPC: &str = "2.0";
/// Maximum encoded size of one newline-delimited JSON-RPC request.
///
/// Individual tools apply tighter, domain-specific limits where appropriate.  This
/// framing limit keeps the parser from turning an unbounded stdio line into an
/// arbitrarily large `serde_json::Value` before a tool can enforce its own bound.
pub const MAX_REQUEST_BYTES: usize = 20_000_000;

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
        if line.len() > MAX_REQUEST_BYTES {
            return Err(Box::new(Response::error(
                None,
                code::INVALID_REQUEST,
                format!("JSON-RPC request exceeds the {MAX_REQUEST_BYTES}-byte safety bound"),
                None,
            )));
        }

        let value: Value = serde_json::from_str(line).map_err(|e| {
            Box::new(Response::error(
                None,
                code::PARSE_ERROR,
                format!("invalid JSON: {e}"),
                None,
            ))
        })?;

        let Some(object) = value.as_object() else {
            return Err(Box::new(Response::error(
                None,
                code::INVALID_REQUEST,
                "JSON-RPC request must be an object".to_string(),
                None,
            )));
        };

        if object.get("jsonrpc") != Some(&Value::String(JSONRPC.to_string())) {
            return Err(Box::new(Response::error(
                response_id(&value),
                code::INVALID_REQUEST,
                "jsonrpc must be exactly \"2.0\"".to_string(),
                None,
            )));
        }

        if let Some(id) = object.get("id") {
            if !(id.is_null() || id.is_string() || id.is_number()) {
                return Err(Box::new(Response::error(
                    None,
                    code::INVALID_REQUEST,
                    "id must be a string, number, or null".to_string(),
                    None,
                )));
            }
        }

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

        if method.is_empty() {
            return Err(Box::new(Response::error(
                response_id(&value),
                code::INVALID_REQUEST,
                "method must not be empty".to_string(),
                None,
            )));
        }

        if let Some(params) = object.get("params") {
            if !(params.is_object() || params.is_array()) {
                return Err(Box::new(Response::error(
                    response_id(&value),
                    code::INVALID_REQUEST,
                    "params must be an object or array".to_string(),
                    None,
                )));
            }
        }

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

fn response_id(value: &Value) -> Option<Value> {
    value
        .get("id")
        .and_then(|id| (id.is_null() || id.is_string() || id.is_number()).then(|| id.clone()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_notification_has_no_id_and_is_not_answered() {
        let request = Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .expect("notification parses");
        assert!(request.is_notification());
    }

    #[test]
    fn an_invalid_envelope_is_rejected_before_method_dispatch() {
        for document in [
            r#"{"id":1,"method":"ping"}"#,
            r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":true,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":false}"#,
        ] {
            let error = Request::parse(document).unwrap_err().to_json();
            assert_eq!(error["error"]["code"], json!(code::INVALID_REQUEST));
        }
    }

    #[test]
    fn response_serialisation_never_leaks_a_result_into_an_error() {
        let response = Response::error(
            Some(json!(7)),
            code::INVALID_PARAMS,
            "bad parameters".into(),
            Some(json!({ "field": "layer" })),
        )
        .to_json();
        assert!(response.get("result").is_none());
        assert_eq!(response["error"]["data"]["field"], json!("layer"));
    }

    #[test]
    fn oversized_request_is_rejected_before_json_deserialization() {
        let line = "x".repeat(MAX_REQUEST_BYTES + 1);
        let error = Request::parse(&line).unwrap_err().to_json();
        assert_eq!(error["error"]["code"], json!(code::INVALID_REQUEST));
        assert!(error["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("safety bound")));
    }
}
