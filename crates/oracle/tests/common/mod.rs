#![allow(dead_code)]

//! Fixtures shared by the invariant suites.
//!
//! Judgements are built directly from manifests rather than by running an oracle, so that a test
//! about combination can put any tier beside any position without needing an oracle that would
//! plausibly produce it. Tests about the shipped oracles themselves live in `oracles.rs` and go
//! through [`bioprism_oracle::Oracle::evaluate`].

use bioprism_oracle::{
    Confidence, Evidence, EvidenceTier, Independence, Judgement, OracleId, OracleManifest,
    OracleRef, OracleVersion, Plane, Position, SharedResource, UtcTimestamp, ValidityWindow,
};

pub const NOW: &str = "2026-08-08T12:00:00Z";

pub fn ts(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("fixture timestamp is well formed")
}

pub fn now() -> UtcTimestamp {
    ts(NOW)
}

/// A window that opened long ago and never closes.
pub fn always() -> ValidityWindow {
    ValidityWindow::open_ended(ts("2000-01-01T00:00:00Z"))
}

pub fn oracle_ref(name: &str, major: u32) -> OracleRef {
    OracleRef::new(
        OracleId::parse(format!("test:{name}")).expect("fixture oracle id is well formed"),
        OracleVersion::new(major, 0, 0),
    )
}

pub fn manifest(name: &str, tier: EvidenceTier) -> OracleManifest {
    manifest_versioned(name, 1, tier, plane_for(tier))
}

pub fn manifest_versioned(
    name: &str,
    major: u32,
    tier: EvidenceTier,
    plane: Plane,
) -> OracleManifest {
    OracleManifest::new(oracle_ref(name, major), tier, [plane], [], always())
        .expect("fixture manifest declares a plane and disclaims nothing")
        .disclaiming_the_rest()
}

/// The plane a fixture oracle at each rung plausibly speaks to.
pub fn plane_for(tier: EvidenceTier) -> Plane {
    match tier {
        EvidenceTier::Deterministic => Plane::Artifact,
        EvidenceTier::Execution | EvidenceTier::Property => Plane::Analytical,
        EvidenceTier::Statistical => Plane::Measurement,
        EvidenceTier::Judge => Plane::Policy,
    }
}

pub fn confidence(value: f64) -> Confidence {
    Confidence::new(value).expect("fixture confidence is a probability")
}

pub fn judgement(name: &str, tier: EvidenceTier, position: Position, weight: f64) -> Judgement {
    Judgement::from_manifest(&manifest(name, tier), &now(), position, confidence(weight))
}

/// A judgement from an oracle that shares training data with the system it evaluates, so 31.01's
/// independence demotion applies.
pub fn circular_judgement(
    name: &str,
    tier: EvidenceTier,
    position: Position,
    weight: f64,
) -> Judgement {
    let manifest = manifest(name, tier)
        .with_independence(Independence::sharing([SharedResource::TrainingData]));
    Judgement::from_manifest(&manifest, &now(), position, confidence(weight))
}

pub fn evidence(subject: &str) -> Evidence {
    Evidence::new(subject, now())
}
