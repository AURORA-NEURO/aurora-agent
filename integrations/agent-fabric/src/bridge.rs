//! Honest protocol bridges.
//!
//! These functions preserve the useful wire shape of A2A-style task messages, but they do not
//! claim to implement A2A discovery, authentication, streaming, or remote task lifecycle. ACP is
//! represented by a typed refusal until a real ACP contract is selected and tested.

use crate::capability::{Capability, CapabilitySet};
use crate::envelope::TaskEnvelope;
use crate::ids::TaskId;
use crate::json::Value;
use std::fmt;

pub const A2A_WIRE_PROFILE: &str = "aurora-agent-fabric/a2a-wire-shape-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireTask {
    pub task: TaskId,
    pub capabilities: CapabilitySet,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeError {
    Missing(&'static str),
    WrongType(&'static str),
    InvalidId,
    InvalidCapability(String),
    InvalidHex,
    Unsupported(&'static str),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing field {name}"),
            Self::WrongType(name) => write!(f, "field {name} has the wrong type"),
            Self::InvalidId => write!(f, "task id must be a nonzero integer"),
            Self::InvalidCapability(cap) => write!(f, "invalid capability {cap:?}"),
            Self::InvalidHex => write!(f, "payload_hex is not valid even-length hexadecimal"),
            Self::Unsupported(name) => write!(f, "unsupported bridge operation: {name}"),
        }
    }
}

impl std::error::Error for BridgeError {}

pub fn envelope_to_a2a(envelope: &TaskEnvelope) -> Value {
    Value::obj(vec![
        ("profile", Value::str(A2A_WIRE_PROFILE)),
        ("kind", Value::str("task")),
        ("task_id", Value::Uint(envelope.task().raw())),
        (
            "capabilities",
            Value::Arr(
                envelope
                    .capabilities()
                    .iter()
                    .map(|c| Value::str(c.as_str()))
                    .collect(),
            ),
        ),
        ("payload_hex", Value::str(hex_encode(envelope.payload()))),
        (
            "payload_digest",
            Value::str(envelope.payload_digest().hex()),
        ),
        ("wire_only", Value::Bool(true)),
    ])
}

pub fn a2a_to_wire_task(value: &Value) -> Result<WireTask, BridgeError> {
    let task = value
        .get("task_id")
        .and_then(Value::as_u64)
        .ok_or(BridgeError::Missing("task_id"))?;
    let task = TaskId::from_raw(task).ok_or(BridgeError::InvalidId)?;
    let caps = value
        .get("capabilities")
        .ok_or(BridgeError::Missing("capabilities"))?
        .as_arr()
        .ok_or(BridgeError::WrongType("capabilities"))?;
    let mut parsed = Vec::with_capacity(caps.len());
    for cap in caps {
        let raw = cap
            .as_str()
            .ok_or(BridgeError::WrongType("capabilities[]"))?;
        parsed.push(
            Capability::parse(raw).map_err(|_| BridgeError::InvalidCapability(raw.to_string()))?,
        );
    }
    let capabilities = CapabilitySet::from_caps(parsed);
    if capabilities.is_empty() {
        return Err(BridgeError::Missing("capabilities"));
    }
    let payload = value
        .get("payload_hex")
        .and_then(Value::as_str)
        .ok_or(BridgeError::Missing("payload_hex"))?;
    Ok(WireTask {
        task,
        capabilities,
        payload: hex_decode(payload)?,
    })
}

pub fn acp_refusal(reason: &str) -> Value {
    Value::obj(vec![
        ("supported", Value::Bool(false)),
        ("protocol", Value::str("ACP")),
        ("code", Value::str("acp_not_implemented")),
        ("reason", Value::str(reason)),
        ("action", Value::str("use MCP or the local fabric API")),
    ])
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn hex_decode(text: &str) -> Result<Vec<u8>, BridgeError> {
    if text.len() % 2 != 0 {
        return Err(BridgeError::InvalidHex);
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks_exact(2) {
        let hi = (pair[0] as char)
            .to_digit(16)
            .ok_or(BridgeError::InvalidHex)?;
        let lo = (pair[1] as char)
            .to_digit(16)
            .ok_or(BridgeError::InvalidHex)?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;

    #[test]
    fn a2a_shape_round_trips_bytes_and_capabilities() {
        let caps = CapabilitySet::one(Capability::parse("compute").expect("cap"));
        let env = TaskEnvelope::compose(
            TaskId::from_raw(3).expect("id"),
            b"abc".to_vec(),
            caps,
            None,
            0,
            1,
        );
        let wire = envelope_to_a2a(&env);
        let decoded = a2a_to_wire_task(&wire).expect("wire task");
        assert_eq!(decoded.task, env.task());
        assert_eq!(decoded.payload, b"abc");
    }

    #[test]
    fn acp_is_a_typed_refusal_not_a_support_claim() {
        assert_eq!(
            acp_refusal("not wired").get("supported"),
            Some(&Value::Bool(false))
        );
    }
}
