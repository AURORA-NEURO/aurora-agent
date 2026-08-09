//! Capability cards: what a host publishes about its extension points.
//!
//! 40.16 lists a `capability card` among the outputs of registration, alongside the registered
//! version and the conformance result. 43.35 adds the requirement that makes the card more than a
//! listing: **"Unsupported capabilities remain discoverable as unavailable."**
//!
//! # An absent capability is a card, not a missing card
//!
//! [`PluginRegistry::capability_cards`](crate::PluginRegistry::capability_cards) emits one card per
//! [`CapabilityKind`], always, whether or not anything provides it. A client that receives five
//! cards, one of them [`Availability::Unavailable`], learns that the host knows about mutation
//! operators and has none. A client that receives four cards has to guess whether the fifth is
//! unsupported, unimplemented, or filtered — and every one of those guesses is a different bug
//! report.
//!
//! # The card repeats the caveats
//!
//! [`ProviderCard`] carries the declared effects and the isolation *request* next to
//! [`Enforcement::DeclaredOnly`], and it carries `load_bearing` — the host's verdict — rather than
//! leaving a reader to compare a trust level against a floor they cannot see. A card is the surface
//! most likely to be read by somebody who has not read this crate, so it states what it means
//! rather than what it stores.

use crate::abi::AbiGrade;
use crate::capability::{CapabilityKind, Priority};
use crate::effect::Effect;
use crate::evidence::TrustLevel;
use crate::loss::SemanticLossDeclaration;
use crate::manifest::PluginId;
use crate::sandbox::{Enforcement, IsolationClass};
use crate::version::SchemaVersion;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Whether anything currently answers for a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
pub enum Availability {
    Available(Box<ProviderCard>),
    /// Discoverable and unavailable, with the reason spelled out: "no provider" and "the only
    /// provider was revoked" send an operator to different places.
    Unavailable {
        reason: String,
    },
}

impl Availability {
    pub fn provider(&self) -> Option<&ProviderCard> {
        match self {
            Availability::Available(card) => Some(card),
            Availability::Unavailable { .. } => None,
        }
    }
}

/// The plugin that currently wins a capability, as published to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCard {
    pub plugin: PluginId,
    /// The immutable version identity: the digest of the whole manifest.
    pub digest: ContentHash,
    pub priority: Priority,
    /// Concrete subjects handled. Empty means every subject of the kind.
    pub subjects: BTreeSet<String>,
    /// The version the handshake settled on, not the set the plugin offered.
    pub speaks: SchemaVersion,
    pub trust: TrustLevel,
    /// The host's verdict, not a level to be compared against an unpublished floor.
    pub load_bearing: bool,
    pub abi_grade: AbiGrade,
    pub effects: BTreeSet<Effect>,
    pub isolation: IsolationClass,
    /// Always [`Enforcement::DeclaredOnly`]; published so a client cannot mistake the isolation
    /// field for a guarantee.
    pub isolation_enforcement: Enforcement,
    pub semantic_loss: Option<SemanticLossDeclaration>,
}

/// One capability kind and its current state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCard {
    pub kind: CapabilityKind,
    pub availability: Availability,
}

impl CapabilityCard {
    pub fn is_available(&self) -> bool {
        matches!(self.availability, Availability::Available(_))
    }

    pub fn provider(&self) -> Option<&ProviderCard> {
        self.availability.provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unavailable_card_carries_a_reason_and_no_provider() {
        let card = CapabilityCard {
            kind: CapabilityKind::MutationOperator,
            availability: Availability::Unavailable {
                reason: "no plugin registered for this capability".into(),
            },
        };
        assert!(!card.is_available());
        assert!(card.provider().is_none());
        let json = serde_json::to_string(&card).expect("serialises");
        assert!(json.contains("unavailable"), "{json}");
        assert!(json.contains("no plugin registered"), "{json}");
    }
}
