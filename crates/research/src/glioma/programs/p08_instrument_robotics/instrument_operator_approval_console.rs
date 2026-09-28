//! Single-use human authorization for preclinical glioma instrument actions.
//!
//! This is a typed approval decision, not a UI log.  It binds an operator confirmation to the
//! exact plan, device, opaque sample scope, expiry, interlocks, uncertainty budget, and emergency
//! stop path.  Any change or revocation invalidates the approval before a local gateway can use it.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentApproval1@1";
pub const MAX_EFFECTS: usize = 128;
pub const MAX_INTERLOCKS: usize = 64;
pub const MAX_STOP_STEPS: usize = 32;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterlockState {
    Clear,
    Failed,
    Unmeasured,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterlockObservation {
    pub interlock_id: String,
    pub state: InterlockState,
    pub observed_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorApprovalRequest {
    pub approval_id: String,
    pub plan_digest: ContentHash,
    pub device_id: String,
    pub sample_scope_digest: ContentHash,
    pub operator_id: String,
    pub operator_authority_digest: Option<ContentHash>,
    pub issued_tick: u64,
    pub expires_tick: u64,
    pub current_tick: u64,
    pub effect_order: Vec<String>,
    pub interlock_order: Vec<InterlockObservation>,
    pub uncertainty_milli: u32,
    pub maximum_uncertainty_milli: u32,
    pub operator_confirmed: bool,
    pub revoked: bool,
    pub already_consumed: bool,
    pub single_use: bool,
    pub stop_path_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDisposition {
    Approved,
    Denied,
    Expired,
    Revoked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorApproval {
    pub feature_id: String,
    pub output_schema: String,
    pub approval_id: String,
    pub disposition: ApprovalDisposition,
    pub plan_digest: ContentHash,
    pub device_id: String,
    pub sample_scope_digest: ContentHash,
    pub operator_id: String,
    pub issued_tick: u64,
    pub expires_tick: u64,
    pub reason_order: Vec<String>,
    pub stop_path_order: Vec<String>,
    pub dispatch_permitted: bool,
    pub single_use_token_digest: Option<ContentHash>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OperatorApprovalError {
    #[error("operator approval request is invalid: {0}")]
    InvalidRequest(String),
    #[error("operator approval output is invalid: {0}")]
    InvalidOutput(String),
    #[error("operator approval digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String], maximum: usize) -> bool {
    values.len() <= maximum
        && values.iter().all(|value| safe_text(value))
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_unique_sequence(values: &[String], maximum: usize) -> bool {
    values.len() <= maximum
        && values.iter().all(|value| safe_text(value))
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| !values[..index].contains(value))
}

fn body(request: &OperatorApprovalRequest) -> serde_json::Value {
    serde_json::json!({
        "approval_id": request.approval_id,
        "plan_digest": request.plan_digest,
        "device_id": request.device_id,
        "sample_scope_digest": request.sample_scope_digest,
        "operator_id": request.operator_id,
        "operator_authority_digest": request.operator_authority_digest,
        "issued_tick": request.issued_tick,
        "expires_tick": request.expires_tick,
        "current_tick": request.current_tick,
        "effect_order": request.effect_order,
        "interlock_order": request.interlock_order,
        "uncertainty_milli": request.uncertainty_milli,
        "maximum_uncertainty_milli": request.maximum_uncertainty_milli,
        "operator_confirmed": request.operator_confirmed,
        "revoked": request.revoked,
        "already_consumed": request.already_consumed,
        "single_use": request.single_use,
        "stop_path_order": request.stop_path_order,
    })
}

fn result_body(result: &OperatorApproval) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "approval_id": result.approval_id,
        "disposition": result.disposition,
        "plan_digest": result.plan_digest,
        "device_id": result.device_id,
        "sample_scope_digest": result.sample_scope_digest,
        "operator_id": result.operator_id,
        "issued_tick": result.issued_tick,
        "expires_tick": result.expires_tick,
        "reason_order": result.reason_order,
        "stop_path_order": result.stop_path_order,
        "dispatch_permitted": result.dispatch_permitted,
        "single_use_token_digest": result.single_use_token_digest,
    })
}

fn validate_request(request: &OperatorApprovalRequest) -> Result<(), OperatorApprovalError> {
    if !safe_text(&request.approval_id)
        || !safe_text(&request.device_id)
        || !safe_text(&request.operator_id)
        || !valid_hash(&request.plan_digest)
        || !valid_hash(&request.sample_scope_digest)
        || request
            .operator_authority_digest
            .as_ref()
            .is_some_and(|hash| !valid_hash(hash))
        || request.operator_authority_digest.is_none()
        || request.issued_tick == 0
        || request.expires_tick <= request.issued_tick
        || request.current_tick == 0
        || request.current_tick < request.issued_tick
        || request.uncertainty_milli > 1_000_000
        || request.maximum_uncertainty_milli > 1_000_000
        || !canonical(&request.effect_order, MAX_EFFECTS)
        || request.effect_order.is_empty()
        || request.interlock_order.is_empty()
        || request.interlock_order.len() > MAX_INTERLOCKS
        || !bounded_unique_sequence(&request.stop_path_order, MAX_STOP_STEPS)
        || request.stop_path_order.is_empty()
        || request.interlock_order.windows(2).any(|pair| {
            pair[0].interlock_id >= pair[1].interlock_id
                || !safe_text(&pair[0].interlock_id)
                || !safe_text(&pair[1].interlock_id)
        })
        || request
            .interlock_order
            .iter()
            .any(|interlock| interlock.observed_tick == 0)
    {
        return Err(OperatorApprovalError::InvalidRequest(
            "approval identity, authority, expiry, bounded effects, interlocks, uncertainty, and stop path are required".into(),
        ));
    }
    let expected = ContentHash::of_value(&body(request))
        .map_err(|error| OperatorApprovalError::Digest(error.to_string()))?;
    if expected.as_str().len() != 64 {
        return Err(OperatorApprovalError::Digest(
            "approval body did not produce a content hash".into(),
        ));
    }
    Ok(())
}

impl OperatorApproval {
    pub fn validate(&self) -> Result<(), OperatorApprovalError> {
        if self.feature_id != FEATURE_ID || self.output_schema != OUTPUT_SCHEMA {
            return Err(OperatorApprovalError::InvalidOutput(
                "feature or output schema identity is incorrect".into(),
            ));
        }
        if !safe_text(&self.approval_id)
            || !safe_text(&self.device_id)
            || !safe_text(&self.operator_id)
            || !valid_hash(&self.plan_digest)
            || !valid_hash(&self.sample_scope_digest)
            || self.reason_order.len() > MAX_EFFECTS
            || self.stop_path_order.is_empty()
            || self.stop_path_order.len() > MAX_STOP_STEPS
            || self.dispatch_permitted != (self.disposition == ApprovalDisposition::Approved)
            || (self.dispatch_permitted && self.single_use_token_digest.is_none())
            || self
                .single_use_token_digest
                .as_ref()
                .is_some_and(|hash| !valid_hash(hash))
        {
            return Err(OperatorApprovalError::InvalidOutput(
                "approval output bounds or dispatch invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&result_body(self))
            .map_err(|error| OperatorApprovalError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(OperatorApprovalError::InvalidOutput(
                "approval digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

pub fn approve_glioma_instrument_action(
    request: &OperatorApprovalRequest,
) -> Result<OperatorApproval, OperatorApprovalError> {
    validate_request(request)?;
    let mut reasons = Vec::new();
    let disposition = if request.revoked {
        reasons.push("approval-is-revoked".into());
        ApprovalDisposition::Revoked
    } else if request.current_tick >= request.expires_tick {
        reasons.push("approval-is-expired".into());
        ApprovalDisposition::Expired
    } else {
        if request.operator_authority_digest.is_none() {
            reasons.push("operator-authority-is-missing".into());
        }
        if !request.operator_confirmed {
            reasons.push("operator-confirmation-is-missing".into());
        }
        if request.already_consumed {
            reasons.push("single-use-approval-was-already-consumed".into());
        }
        if request.uncertainty_milli > request.maximum_uncertainty_milli {
            reasons.push("uncertainty-exceeds-approval-budget".into());
        }
        for interlock in &request.interlock_order {
            if interlock.state != InterlockState::Clear {
                reasons.push(format!(
                    "interlock-{}-is-{:?}",
                    interlock.interlock_id, interlock.state
                ));
            }
        }
        if reasons.is_empty() {
            ApprovalDisposition::Approved
        } else if request
            .interlock_order
            .iter()
            .any(|interlock| interlock.state == InterlockState::Unmeasured)
        {
            ApprovalDisposition::Unresolved
        } else {
            ApprovalDisposition::Denied
        }
    };
    let dispatch_permitted = disposition == ApprovalDisposition::Approved;
    let single_use_token_digest = dispatch_permitted.then(|| {
        ContentHash::of_value(&serde_json::json!({
            "approval_id": request.approval_id,
            "plan_digest": request.plan_digest,
            "device_id": request.device_id,
            "sample_scope_digest": request.sample_scope_digest,
            "operator_id": request.operator_id,
            "expires_tick": request.expires_tick,
        }))
        .expect("approval token body is hashable")
    });
    let mut output = OperatorApproval {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        approval_id: request.approval_id.clone(),
        disposition,
        plan_digest: request.plan_digest.clone(),
        device_id: request.device_id.clone(),
        sample_scope_digest: request.sample_scope_digest.clone(),
        operator_id: request.operator_id.clone(),
        issued_tick: request.issued_tick,
        expires_tick: request.expires_tick,
        reason_order: reasons,
        stop_path_order: request.stop_path_order.clone(),
        dispatch_permitted,
        single_use_token_digest,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-approval"),
    };
    output.digest = ContentHash::of_value(&result_body(&output))
        .map_err(|error| OperatorApprovalError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn request() -> OperatorApprovalRequest {
        OperatorApprovalRequest {
            approval_id: "approval-a".into(),
            plan_digest: hash("plan"),
            device_id: "device-a".into(),
            sample_scope_digest: hash("scope"),
            operator_id: "operator-a".into(),
            operator_authority_digest: Some(hash("authority")),
            issued_tick: 10,
            expires_tick: 30,
            current_tick: 12,
            effect_order: vec!["capture".into(), "save".into()],
            interlock_order: vec![InterlockObservation {
                interlock_id: "guard".into(),
                state: InterlockState::Clear,
                observed_tick: 12,
            }],
            uncertainty_milli: 100,
            maximum_uncertainty_milli: 500,
            operator_confirmed: true,
            revoked: false,
            already_consumed: false,
            single_use: true,
            stop_path_order: vec!["stop-gateway".into(), "notify-operator".into()],
        }
    }

    #[test]
    fn exact_confirmed_plan_gets_single_use_approval() {
        let output = approve_glioma_instrument_action(&request()).unwrap();
        assert_eq!(output.disposition, ApprovalDisposition::Approved);
        assert!(output.dispatch_permitted);
        assert!(output.single_use_token_digest.is_some());
        assert!(output.validate().is_ok());
    }

    #[test]
    fn failed_interlock_denies_dispatch() {
        let mut input = request();
        input.interlock_order[0].state = InterlockState::Failed;
        let output = approve_glioma_instrument_action(&input).unwrap();
        assert_eq!(output.disposition, ApprovalDisposition::Denied);
        assert!(!output.dispatch_permitted);
        assert!(output
            .reason_order
            .iter()
            .any(|reason| reason.contains("interlock")));
    }

    #[test]
    fn unmeasured_interlock_is_unresolved_not_a_pass() {
        let mut input = request();
        input.interlock_order[0].state = InterlockState::Unmeasured;
        let output = approve_glioma_instrument_action(&input).unwrap();
        assert_eq!(output.disposition, ApprovalDisposition::Unresolved);
        assert!(!output.dispatch_permitted);
    }

    #[test]
    fn expiry_revocation_and_reuse_are_terminal() {
        let mut expired = request();
        expired.current_tick = expired.expires_tick;
        assert_eq!(
            approve_glioma_instrument_action(&expired)
                .unwrap()
                .disposition,
            ApprovalDisposition::Expired
        );
        let mut revoked = request();
        revoked.revoked = true;
        assert_eq!(
            approve_glioma_instrument_action(&revoked)
                .unwrap()
                .disposition,
            ApprovalDisposition::Revoked
        );
        let mut reused = request();
        reused.already_consumed = true;
        assert_eq!(
            approve_glioma_instrument_action(&reused)
                .unwrap()
                .disposition,
            ApprovalDisposition::Denied
        );
    }
}
