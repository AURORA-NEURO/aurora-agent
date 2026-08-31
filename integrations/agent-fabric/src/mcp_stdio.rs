//! Bounded MCP-over-stdio adapter.
//!
//! This is intentionally a small JSON-RPC surface: initialize, ping, tools/list, tools/call for
//! `fabric.submit` and `fabric.cancel`, and silent handling of notifications. It uses newline
//! framing, never shells out, and delegates admission to the same [`crate::scheduler::Fabric`]
//! instance used by local callers.

use crate::capability::{Capability, CapabilitySet};
use crate::ids::{IdempotencyKey, TaskId};
use crate::json::{self, Value};
use crate::scheduler::{Fabric, Submission};
use crate::transport::{self, TransportError};
use std::fmt;
use std::io::{self, BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug)]
pub enum McpError {
    Transport(TransportError),
    InvalidRequest(&'static str),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(f),
            Self::InvalidRequest(message) => write!(f, "invalid MCP request: {message}"),
        }
    }
}

impl std::error::Error for McpError {}

impl From<TransportError> for McpError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

pub struct McpServer {
    fabric: Fabric,
    initialized: bool,
}

impl McpServer {
    pub fn new(fabric: Fabric) -> Self {
        Self {
            fabric,
            initialized: false,
        }
    }

    pub fn fabric(&self) -> &Fabric {
        &self.fabric
    }
    pub fn fabric_mut(&mut self) -> &mut Fabric {
        &mut self.fabric
    }

    pub fn handle_line(&mut self, line: &str) -> Result<Option<String>, McpError> {
        let request = transport::decode_line(line)?;
        Ok(self
            .handle_value(&request)
            .map(|response| transport::encode_line(&response)))
    }

    /// Handle one parsed JSON-RPC request. Notifications are applied but return `None`.
    pub fn handle_value(&mut self, request: &Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str);
        let Some(method) = method else {
            return Some(error(id, -32600, "request.method is required"));
        };
        let response = match method {
            "initialize" => {
                self.initialized = true;
                let capabilities = Value::obj(vec![("tools", Value::obj(vec![]))]);
                let server_info = Value::obj(vec![
                    ("name", Value::str("aurora-agent-fabric")),
                    ("version", Value::str(crate::VERSION)),
                ]);
                Ok(Value::obj(vec![
                    ("protocolVersion", Value::str(PROTOCOL_VERSION)),
                    ("capabilities", capabilities),
                    ("serverInfo", server_info),
                ]))
            }
            "notifications/initialized" => return None,
            "ping" => Ok(Value::obj(vec![])),
            "tools/list" => Ok(tool_list()),
            "tools/call" => self.call_tool(request),
            _ => Err((-32601, "method not found")),
        };
        let id = id?;
        Some(match response {
            Ok(result) => success(id, result),
            Err((code, message)) => error(Some(id), code, message),
        })
    }

    fn call_tool(&mut self, request: &Value) -> Result<Value, (i64, &'static str)> {
        let params = request
            .get("params")
            .ok_or((-32602, "params is required"))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((-32602, "params.name is required"))?;
        let empty_arguments = Value::Obj(vec![]);
        let arguments = params.get("arguments").unwrap_or(&empty_arguments);
        match name {
            "fabric.submit" => self.submit(arguments),
            "fabric.cancel" => self.cancel(arguments),
            _ => Err((-32602, "unknown fabric tool")),
        }
    }

    fn submit(&mut self, args: &Value) -> Result<Value, (i64, &'static str)> {
        let payload = args
            .get("payload")
            .and_then(Value::as_str)
            .ok_or((-32602, "payload must be a string"))?;
        let caps = args
            .get("capabilities")
            .and_then(Value::as_arr)
            .ok_or((-32602, "capabilities must be an array"))?;
        let mut parsed = Vec::with_capacity(caps.len());
        for item in caps {
            let raw = item
                .as_str()
                .ok_or((-32602, "capabilities must contain strings"))?;
            parsed.push(Capability::parse(raw).map_err(|_| (-32602, "invalid capability"))?);
        }
        let capabilities = CapabilitySet::from_caps(parsed);
        if capabilities.is_empty() {
            return Err((-32602, "capabilities must not be empty"));
        }
        let key = match args.get("idempotency_key_hex") {
            None | Some(Value::Null) => None,
            Some(Value::Str(text)) => Some(IdempotencyKey::from_bytes(
                parse_key(text)
                    .map_err(|_| (-32602, "idempotency key must be 32 hex characters"))?,
            )),
            _ => return Err((-32602, "idempotency_key_hex must be a string")),
        };
        let submission = self
            .fabric
            .submit(payload.as_bytes().to_vec(), capabilities, key);
        self.fabric.step_to(self.fabric.now());
        let value = match submission {
            Submission::Accepted { task } => Value::obj(vec![
                ("status", Value::str("accepted")),
                ("task_id", Value::Uint(task.raw())),
            ]),
            Submission::Duplicate { task } => Value::obj(vec![
                ("status", Value::str("duplicate")),
                ("task_id", Value::Uint(task.raw())),
            ]),
            Submission::Rejected { pressure } => Value::obj(vec![
                ("status", Value::str("rejected")),
                ("capacity", Value::Uint(pressure.capacity as u64)),
            ]),
        };
        Ok(tool_result(value))
    }

    fn cancel(&mut self, args: &Value) -> Result<Value, (i64, &'static str)> {
        let raw = args
            .get("task_id")
            .and_then(Value::as_u64)
            .ok_or((-32602, "task_id must be a nonzero integer"))?;
        let task = TaskId::from_raw(raw).ok_or((-32602, "task_id must be a nonzero integer"))?;
        let cancelled = self.fabric.cancel(task);
        self.fabric.step_to(self.fabric.now());
        Ok(tool_result(Value::obj(vec![
            ("cancelled", Value::Bool(cancelled)),
            ("task_id", Value::Uint(raw)),
        ])))
    }
}

pub fn serve<R: BufRead, W: Write>(
    server: &mut McpServer,
    reader: R,
    mut writer: W,
) -> io::Result<()> {
    for line in reader.lines() {
        let line = line?;
        match server.handle_line(&line) {
            Ok(Some(response)) => writer.write_all(response.as_bytes())?,
            Ok(None) => {}
            Err(parse_error) => writer.write_all(
                transport::encode_line(&error(None, -32700, &parse_error.to_string())).as_bytes(),
            )?,
        }
        writer.flush()?;
    }
    Ok(())
}

fn tool_list() -> Value {
    Value::obj(vec![(
        "tools",
        Value::Arr(vec![
            Value::obj(vec![
                ("name", Value::str("fabric.submit")),
                (
                    "description",
                    Value::str("Admit one bounded task into the local fabric."),
                ),
            ]),
            Value::obj(vec![
                ("name", Value::str("fabric.cancel")),
                (
                    "description",
                    Value::str("Request cooperative cancellation for a task."),
                ),
            ]),
        ]),
    )])
}

fn tool_result(value: Value) -> Value {
    Value::obj(vec![
        (
            "content",
            Value::Arr(vec![Value::obj(vec![
                ("type", Value::str("text")),
                ("text", Value::str(json::to_string(&value))),
            ])]),
        ),
        ("structuredContent", value),
    ])
}

fn success(id: Value, result: Value) -> Value {
    Value::obj(vec![
        ("jsonrpc", Value::str("2.0")),
        ("id", id),
        ("result", result),
    ])
}

fn error(id: Option<Value>, code: i64, message: &str) -> Value {
    Value::obj(vec![
        ("jsonrpc", Value::str("2.0")),
        ("id", id.unwrap_or(Value::Null)),
        (
            "error",
            Value::obj(vec![
                ("code", Value::Int(code)),
                ("message", Value::str(message)),
            ]),
        ),
    ])
}

fn parse_key(text: &str) -> Result<[u8; 16], ()> {
    if text.len() != 32 {
        return Err(());
    }
    let mut key = [0u8; 16];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16).ok_or(())?;
        let lo = (pair[1] as char).to_digit(16).ok_or(())?;
        key[index] = ((hi << 4) | lo) as u8;
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancel::CancelState;
    use crate::capability::Capability;
    use crate::envelope::Outcome;
    use crate::exec::{FnHandler, InlineDriver};
    use crate::scheduler::FabricConfig;
    use std::sync::Arc;

    fn server() -> McpServer {
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
        server.fabric_mut().register_agent(
            "test",
            CapabilitySet::one(Capability::parse("compute").expect("cap")),
        );
        server
    }

    #[test]
    fn initialize_and_tools_list_are_json_rpc_responses() {
        let mut server = server();
        let request =
            json::parse(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).expect("request");
        assert_eq!(
            server
                .handle_value(&request)
                .expect("response")
                .get("id")
                .and_then(Value::as_u64),
            Some(1)
        );
        let list =
            json::parse(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).expect("request");
        assert!(
            json::to_string(&server.handle_value(&list).expect("response"))
                .contains("fabric.submit")
        );
    }

    #[test]
    fn submit_and_cancel_use_the_fabric_boundary() {
        let mut server = server();
        let request = json::parse(r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fabric.submit","arguments":{"payload":"abc","capabilities":["compute"]}}}"#).expect("request");
        let response = server.handle_value(&request).expect("response");
        assert!(json::to_string(&response).contains("accepted"));
    }

    #[test]
    fn malformed_requests_and_notifications_are_distinct() {
        let mut server = server();
        assert!(server.handle_line("not-json").is_err());
        let notification = json::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .expect("request");
        assert!(server.handle_value(&notification).is_none());
    }
}
