//! Shared fixtures for the fabric test suites.
//!
//! Every contract here is deliberately small and named for the property it exists to exercise.

#![allow(dead_code)]

use bioprism_fabric::contract::{
    AgentContract, AssuranceProfile, EpistemicContract, FieldType, InterfaceType,
    ResourceEnvelope, UncertaintySemantics,
};
use bioprism_fabric::effect::{Effect, EffectKind, EffectSet, Scope};
use bioprism_fabric::reputation::EvidenceLayer;

pub fn record(name: &str, fields: &[(&str, FieldType)]) -> InterfaceType {
    fields
        .iter()
        .fold(InterfaceType::new(name), |acc, (field, ty)| {
            acc.field(*field, ty.clone())
        })
}

pub fn analysis() -> InterfaceType {
    record("Analysis", &[("rows", FieldType::Number)])
}

pub fn report() -> InterfaceType {
    record(
        "Report",
        &[("verdict", FieldType::Text), ("evidence", FieldType::Text)],
    )
}

pub fn thin_report() -> InterfaceType {
    record("Report", &[("verdict", FieldType::Text)])
}

pub fn effect(kind: EffectKind, pattern: &str) -> Effect {
    Effect::new(kind, Scope::resource(pattern).expect("valid pattern"))
}

pub fn effects(items: &[(EffectKind, &str)]) -> EffectSet {
    EffectSet::from_effects(items.iter().map(|(kind, pattern)| effect(*kind, pattern)))
}

/// A calibrated, provenance-complete component with a bounded read effect.
pub fn verifier(id: &str) -> AgentContract {
    AgentContract::new(
        id,
        analysis(),
        report(),
        EpistemicContract::new(UncertaintySemantics::Calibrated)
            .emitting("verdict")
            .with_complete_provenance(),
        AssuranceProfile::at(EvidenceLayer::PrismEvaluated).with_lower_bound_bp(8_000),
    )
    .with_effects(effects(&[(EffectKind::ArtifactRead, "corpus/**")]))
    .with_envelope(ResourceEnvelope::new().tokens(1_000).cost(100).latency(10))
}

/// Same interface, weaker epistemics: 23.41's worked counterexample.
pub fn cheaper_verifier(id: &str) -> AgentContract {
    AgentContract::new(
        id,
        analysis(),
        report(),
        EpistemicContract::new(UncertaintySemantics::PointEstimate).emitting("verdict"),
        AssuranceProfile::at(EvidenceLayer::SelfDeclared),
    )
    .with_effects(effects(&[(EffectKind::ArtifactRead, "corpus/**")]))
    .with_envelope(ResourceEnvelope::new().tokens(500).cost(20).latency(5))
}

pub fn extractor(id: &str) -> AgentContract {
    AgentContract::new(
        id,
        record("Paper", &[("doi", FieldType::Text)]),
        analysis(),
        EpistemicContract::new(UncertaintySemantics::Calibrated)
            .emitting("extraction")
            .with_complete_provenance(),
        AssuranceProfile::at(EvidenceLayer::PrismEvaluated).with_lower_bound_bp(7_000),
    )
    .with_effects(effects(&[(EffectKind::ArtifactRead, "corpus/**")]))
    .with_envelope(ResourceEnvelope::new().tokens(800).cost(60).latency(8))
}

pub fn permissive_envelope() -> ResourceEnvelope {
    ResourceEnvelope::new()
        .tokens(1_000_000)
        .tool_calls(1_000)
        .cost(1_000_000)
        .latency(1_000_000)
}
