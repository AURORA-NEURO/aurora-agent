//! Glioma instrument operations handlers for bounded fleet and acquisition governance.

use super::*;

impl Server {
    /// Assess instrument health trends while optionally masking site identity.
    pub(super) fn glioma_instrument_fleet_health(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: FleetHealthMonitorRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_instrument_fleet_health requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma instrument fleet-health request: {error}"))?;
        let assessment = monitor_glioma_instrument_fleet_health(&request).map_err(|error| {
            format!("glioma instrument fleet-health assessment refused: {error}")
        })?;
        serde_json::to_value(json!({
            "assessment": assessment,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "health drift, quality failures, downtime, and calibration pressure are assessed from replay-validated local summaries",
                "site identity can be masked while instrument-level evidence and alert reasons remain auditable",
                "an assessment never dispatches an instrument or changes fleet state"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument fleet-health assessment: {error}"))
    }

    /// Allocate approved acquisition demand across bounded local capacity and fairness limits.
    pub(super) fn glioma_acquisition_capacity_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AcquisitionCapacityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_acquisition_capacity_plan requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma acquisition-capacity request: {error}"))?;
        let plan = plan_glioma_acquisition_capacity(&request)
            .map_err(|error| format!("glioma acquisition-capacity planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "minimum service, campaign priority, fairness, resource capacity, operator capacity, maintenance reserve, deadline, approval, and cost remain bounded",
                "unallocated demand and fairness shortfalls remain explicit",
                "the plan sets dispatch_permitted false and does not reserve or consume physical capacity"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma acquisition-capacity plan: {error}"))
    }

    /// Schedule maintenance windows against service deadlines and existing instrument bookings.
    pub(super) fn glioma_instrument_maintenance_plan(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: MaintenanceWindowRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_maintenance_plan requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma instrument maintenance request: {error}"))?;
        let plan = plan_glioma_instrument_maintenance(&request)
            .map_err(|error| format!("glioma instrument maintenance planning refused: {error}"))?;
        serde_json::to_value(json!({
            "plan": plan,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "service due dates, calibration expiry, device health, operating horizon, and reservation conflicts are checked",
                "maintenance windows are proposals and never disable equipment or alter a reservation"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument maintenance plan: {error}"))
    }

    /// Audit instrument run identity, protocol, operator, clock, calibration, and artifact continuity.
    pub(super) fn glioma_assay_provenance_audit(&self, arguments: &Value) -> Result<Value, String> {
        let request: AssayProvenanceAuditRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_assay_provenance_audit requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma assay-provenance request: {error}"))?;
        let audit = audit_glioma_assay_provenance(&request)
            .map_err(|error| format!("glioma assay-provenance audit refused: {error}"))?;
        serde_json::to_value(json!({
            "audit": audit,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "sample scope, approved protocol, instrument calibration, operator authority, observation clock, and artifact chain are checked per run",
                "admission and exclusion decisions retain digest-bound evidence and do not modify source records"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma assay-provenance audit: {error}"))
    }

    /// Snapshot acquisition queue risk, device readiness, telemetry freshness, and reorder options.
    pub(super) fn glioma_acquisition_operations_snapshot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: AcquisitionOperationsRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_acquisition_operations_snapshot requires request".to_string()
            })?)
            .map_err(|error| format!("invalid glioma acquisition-operations request: {error}"))?;
        let snapshot = plan_glioma_acquisition_operations(&request)
            .map_err(|error| format!("glioma acquisition-operations snapshot refused: {error}"))?;
        serde_json::to_value(json!({
            "snapshot": snapshot,
            "dispatch": "not_started",
            "simulation_only": true,
            "guarantees": [
                "queue service, fairness, device availability, calibration horizon, operator capacity, telemetry staleness, and reorder proposals remain visible",
                "the snapshot sets dispatch_permitted false and does not reserve equipment or execute acquisitions"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma acquisition-operations snapshot: {error}"))
    }

    /// Compile a single-use operator approval decision bound to plan, sample, device, interlocks, and expiry.
    pub(super) fn glioma_instrument_operator_approval(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: OperatorApprovalRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_instrument_operator_approval requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma instrument operator-approval request: {error}")
            })?;
        let approval = approve_glioma_instrument_action(&request)
            .map_err(|error| format!("glioma instrument operator approval refused: {error}"))?;
        serde_json::to_value(json!({
            "approval": approval,
            "dispatch": "not_started",
            "guarantees": [
                "approval binds operator authority, plan digest, sample scope, device, effect order, live interlocks, uncertainty, expiry, and single-use policy",
                "approval compilation does not consume the approval or dispatch an instrument action"
            ]
        }))
        .map_err(|error| format!("cannot encode glioma instrument operator approval: {error}"))
    }
}
