//! Glioma research release handlers grouped by release workflow.

use super::*;

impl Server {
    /// Evaluate transitive release rights, locality, embargo, and audience constraints.
    pub(super) fn glioma_release_shareability_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseShareabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_shareability_check requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release shareability request: {error}"))?;
        let decision = evaluate_glioma_release_shareability(&request)
            .map_err(|error| format!("glioma release shareability evaluation refused: {error}"))?;
        Ok(json!({
            "decision": decision,
            "dispatch": "not_started",
            "guarantees": [
                "transitive dependencies, field classifications, license terms, audience, embargo, rights, and locality are checked together",
                "unknown or restricted dependencies fail closed and remain visible in the decision",
                "a shareability decision authorizes no upload, publication, or artifact transfer"
            ]
        }))
    }
}
