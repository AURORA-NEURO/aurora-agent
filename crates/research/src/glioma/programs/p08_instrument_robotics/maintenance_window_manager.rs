//! Deterministic maintenance-window planning for preclinical glioma instrument fleets.
//!
//! Maintenance is a scheduling constraint, not an after-the-fact warning.  This planner searches
//! the earliest reservation-free service interval before calibration expiry, blocks overdue or
//! low-health devices, and returns explicit conflicts when no safe window exists.  It never
//! mutates bookings or contacts hardware; the resulting plan is consumed by fleet scheduling and
//! instrument preflight.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentMaintenancePlan1@1";
pub const MAX_DEVICES: usize = 2_048;
pub const MAX_RESERVATIONS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentReservation {
    pub reservation_id: String,
    pub instrument_id: String,
    pub start_tick: u64,
    pub end_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceDevice {
    pub instrument_id: String,
    pub service_due_tick: u64,
    pub calibration_valid_until_tick: u64,
    pub maintenance_duration_ticks: u64,
    pub health_score_milli: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceWindowRequest {
    pub current_tick: u64,
    pub horizon_end_tick: u64,
    pub minimum_health_milli: u16,
    pub devices: Vec<MaintenanceDevice>,
    pub reservations: Vec<InstrumentReservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceWindowDisposition {
    Scheduled,
    Overdue,
    Blocked,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentMaintenanceWindow {
    pub instrument_id: String,
    pub disposition: MaintenanceWindowDisposition,
    pub window_start_tick: Option<u64>,
    pub window_end_tick: Option<u64>,
    pub calibration_valid_until_tick: u64,
    pub health_score_milli: u16,
    pub conflict_order: Vec<String>,
    pub reason_order: Vec<String>,
    pub availability_after_window: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentMaintenancePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub current_tick: u64,
    pub horizon_end_tick: u64,
    pub instrument_order: Vec<String>,
    pub windows: Vec<InstrumentMaintenanceWindow>,
    pub blocked_instrument_order: Vec<String>,
    pub maintenance_lock_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MaintenanceWindowError {
    #[error("maintenance-window request is invalid: {0}")]
    InvalidRequest(String),
    #[error("maintenance-window output is invalid: {0}")]
    InvalidOutput(String),
    #[error("maintenance-window digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_body(plan: &InstrumentMaintenancePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "current_tick": plan.current_tick,
        "horizon_end_tick": plan.horizon_end_tick,
        "instrument_order": plan.instrument_order,
        "windows": plan.windows,
        "blocked_instrument_order": plan.blocked_instrument_order,
        "maintenance_lock_order": plan.maintenance_lock_order,
    })
}

fn validate_request(request: &MaintenanceWindowRequest) -> Result<(), MaintenanceWindowError> {
    if request.current_tick == 0
        || request.horizon_end_tick <= request.current_tick
        || request.minimum_health_milli > 1_000
        || request.devices.is_empty()
        || request.devices.len() > MAX_DEVICES
        || request.reservations.len() > MAX_RESERVATIONS
    {
        return Err(MaintenanceWindowError::InvalidRequest(
            "positive current/horizon bounds, health threshold, and bounded device/reservation sets are required".into(),
        ));
    }
    let mut instruments = BTreeSet::new();
    for device in &request.devices {
        if !safe_text(&device.instrument_id)
            || !instruments.insert(device.instrument_id.clone())
            || device.service_due_tick == 0
            || device.calibration_valid_until_tick <= request.current_tick
            || device.maintenance_duration_ticks == 0
            || device.maintenance_duration_ticks > request.horizon_end_tick - request.current_tick
            || device.health_score_milli > 1_000
        {
            return Err(MaintenanceWindowError::InvalidRequest(
                "devices require unique ids, future calibration expiry, positive bounded service duration, and health scores".into(),
            ));
        }
    }
    let mut reservations = BTreeSet::new();
    for reservation in &request.reservations {
        if !safe_text(&reservation.reservation_id)
            || !safe_text(&reservation.instrument_id)
            || !instruments.contains(&reservation.instrument_id)
            || reservation.start_tick >= reservation.end_tick
            || reservation.end_tick <= request.current_tick
            || reservation.start_tick >= request.horizon_end_tick
            || !reservations.insert(reservation.reservation_id.clone())
        {
            return Err(MaintenanceWindowError::InvalidRequest(
                "reservations require unique in-horizon ids and known instruments".into(),
            ));
        }
    }
    Ok(())
}

impl InstrumentMaintenancePlan {
    pub fn validate(&self) -> Result<(), MaintenanceWindowError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.current_tick == 0
            || self.horizon_end_tick <= self.current_tick
            || self.instrument_order.len() != self.windows.len()
            || !canonical(&self.instrument_order)
            || !unique(&self.instrument_order)
            || !canonical(&self.blocked_instrument_order)
            || !canonical(&self.maintenance_lock_order)
            || self.windows.iter().any(|window| {
                !safe_text(&window.instrument_id)
                    || !canonical(&window.conflict_order)
                    || !canonical(&window.reason_order)
                    || window.health_score_milli > 1_000
                    || window.calibration_valid_until_tick == 0
                    || window
                        .window_start_tick
                        .zip(window.window_end_tick)
                        .map(|(start, end)| start >= end)
                        .unwrap_or(false)
            })
            || !valid_hash(&self.digest)
        {
            return Err(MaintenanceWindowError::InvalidOutput(
                "maintenance plan identity, ordering, window bounds, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| MaintenanceWindowError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MaintenanceWindowError::InvalidOutput(
                "maintenance plan digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Find the earliest safe maintenance window for every local instrument.
pub fn plan_glioma_instrument_maintenance(
    request: &MaintenanceWindowRequest,
) -> Result<InstrumentMaintenancePlan, MaintenanceWindowError> {
    validate_request(request)?;
    let mut devices = request.devices.clone();
    devices.sort_by(|left, right| left.instrument_id.cmp(&right.instrument_id));
    let mut windows = Vec::new();
    for device in &devices {
        let mut reasons = BTreeSet::new();
        let mut conflicts = BTreeSet::new();
        let disposition;
        let mut window_start = None;
        let mut window_end = None;
        if !device.enabled {
            disposition = MaintenanceWindowDisposition::Disabled;
            reasons.insert("device-disabled".into());
        } else if device.health_score_milli < request.minimum_health_milli {
            disposition = MaintenanceWindowDisposition::Blocked;
            reasons.insert("health-below-maintenance-floor".into());
        } else {
            let mut reservations = request
                .reservations
                .iter()
                .filter(|reservation| reservation.instrument_id == device.instrument_id)
                .collect::<Vec<_>>();
            reservations.sort_by_key(|reservation| (reservation.start_tick, reservation.end_tick));
            let earliest = request.current_tick.max(device.service_due_tick);
            let calibration_limit = device
                .calibration_valid_until_tick
                .min(request.horizon_end_tick);
            let mut candidate = earliest;
            while candidate + device.maintenance_duration_ticks <= calibration_limit {
                let end = candidate + device.maintenance_duration_ticks;
                let overlapping = reservations.iter().find(|reservation| {
                    reservation.start_tick < end && reservation.end_tick > candidate
                });
                if let Some(reservation) = overlapping {
                    conflicts.insert(reservation.reservation_id.clone());
                    candidate = reservation.end_tick.max(candidate + 1);
                } else {
                    window_start = Some(candidate);
                    window_end = Some(end);
                    break;
                }
            }
            if window_start.is_none() {
                disposition = MaintenanceWindowDisposition::Blocked;
                reasons.insert("no-reservation-free-window-before-calibration-expiry".into());
            } else if device.service_due_tick <= request.current_tick {
                disposition = MaintenanceWindowDisposition::Overdue;
                reasons.insert("service-overdue-at-current-tick".into());
            } else {
                disposition = MaintenanceWindowDisposition::Scheduled;
            }
            if device.calibration_valid_until_tick <= device.service_due_tick {
                reasons.insert("calibration-expiry-near-service-due".into());
            }
        }
        windows.push(InstrumentMaintenanceWindow {
            instrument_id: device.instrument_id.clone(),
            disposition,
            window_start_tick: window_start,
            window_end_tick: window_end,
            calibration_valid_until_tick: device.calibration_valid_until_tick,
            health_score_milli: device.health_score_milli,
            conflict_order: conflicts.into_iter().collect(),
            reason_order: reasons.into_iter().collect(),
            availability_after_window: matches!(
                disposition,
                MaintenanceWindowDisposition::Scheduled | MaintenanceWindowDisposition::Overdue
            ),
        });
    }
    let instrument_order = windows
        .iter()
        .map(|window| window.instrument_id.clone())
        .collect::<Vec<_>>();
    let blocked = windows
        .iter()
        .filter(|window| {
            matches!(
                window.disposition,
                MaintenanceWindowDisposition::Blocked | MaintenanceWindowDisposition::Disabled
            )
        })
        .map(|window| window.instrument_id.clone())
        .collect::<Vec<_>>();
    let locks = windows
        .iter()
        .filter(|window| window.window_start_tick.is_some())
        .map(|window| window.instrument_id.clone())
        .collect::<Vec<_>>();
    let mut plan = InstrumentMaintenancePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        current_tick: request.current_tick,
        horizon_end_tick: request.horizon_end_tick,
        instrument_order,
        windows,
        blocked_instrument_order: blocked,
        maintenance_lock_order: locks,
        digest: ContentHash::of_bytes(b"unsealed-glioma-maintenance-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_body(&plan))
        .map_err(|error| MaintenanceWindowError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, due: u64, expiry: u64, health: u16) -> MaintenanceDevice {
        MaintenanceDevice {
            instrument_id: id.into(),
            service_due_tick: due,
            calibration_valid_until_tick: expiry,
            maintenance_duration_ticks: 2,
            health_score_milli: health,
            enabled: true,
        }
    }

    fn request(
        devices: Vec<MaintenanceDevice>,
        reservations: Vec<InstrumentReservation>,
    ) -> MaintenanceWindowRequest {
        MaintenanceWindowRequest {
            current_tick: 10,
            horizon_end_tick: 30,
            minimum_health_milli: 500,
            devices,
            reservations,
        }
    }

    #[test]
    fn finds_earliest_gap_after_a_reservation() {
        let plan = plan_glioma_instrument_maintenance(&request(
            vec![device("scope-1", 12, 25, 900)],
            vec![InstrumentReservation {
                reservation_id: "run-a".into(),
                instrument_id: "scope-1".into(),
                start_tick: 12,
                end_tick: 16,
            }],
        ))
        .unwrap();
        assert_eq!(plan.windows[0].window_start_tick, Some(16));
        assert_eq!(plan.windows[0].window_end_tick, Some(18));
        assert_eq!(
            plan.windows[0].disposition,
            MaintenanceWindowDisposition::Scheduled
        );
    }

    #[test]
    fn overdue_service_is_explicit_and_locked() {
        let plan = plan_glioma_instrument_maintenance(&request(
            vec![device("scope-1", 5, 25, 900)],
            vec![],
        ))
        .unwrap();
        assert_eq!(
            plan.windows[0].disposition,
            MaintenanceWindowDisposition::Overdue
        );
        assert!(plan.maintenance_lock_order.contains(&"scope-1".into()));
        assert!(plan.windows[0]
            .reason_order
            .contains(&"service-overdue-at-current-tick".into()));
    }

    #[test]
    fn conflicts_until_expiry_block_the_instrument() {
        let plan = plan_glioma_instrument_maintenance(&request(
            vec![device("scope-1", 12, 18, 900)],
            vec![InstrumentReservation {
                reservation_id: "run-a".into(),
                instrument_id: "scope-1".into(),
                start_tick: 12,
                end_tick: 18,
            }],
        ))
        .unwrap();
        assert_eq!(
            plan.windows[0].disposition,
            MaintenanceWindowDisposition::Blocked
        );
        assert!(plan.blocked_instrument_order.contains(&"scope-1".into()));
    }

    #[test]
    fn low_health_and_disabled_devices_fail_closed() {
        let plan = plan_glioma_instrument_maintenance(&request(
            vec![
                device("scope-a", 12, 25, 100),
                MaintenanceDevice {
                    enabled: false,
                    ..device("scope-b", 12, 25, 900)
                },
            ],
            vec![],
        ))
        .unwrap();
        assert_eq!(
            plan.windows[0].disposition,
            MaintenanceWindowDisposition::Blocked
        );
        assert_eq!(
            plan.windows[1].disposition,
            MaintenanceWindowDisposition::Disabled
        );
    }
}
