//! Trust across a federation, which is to say: trust not crossing one.
//!
//! Blueprint 10.18 states the rule in five words — *"Federation proves origin, not universal
//! trust"* — and then describes replicating signed catalog events, which is how the rule gets
//! broken in practice. A tier is metadata; metadata replicates; and one replication later the
//! `gold` badge that registry A computed is sitting in registry B's catalog looking exactly like a
//! badge B computed.
//!
//! # Trust does not transit, and the type says so
//!
//! [`TrustStanding`] carries the registry it holds in. There is no function anywhere in this crate
//! with the shape `fn (TrustStanding, RegistryId) -> TrustStanding`, and no `From` impl that turns
//! an [`Attestation`] into one. The two ways to obtain a standing are:
//!
//! - [`TrustStanding::earned`], which takes a `bioprism_registry::TierAssessment` — the tier is
//!   *read off* the assessment, never supplied by the caller. This extends
//!   `bioprism-registry`'s rule (a tier is computed from the pack document, never asserted) across
//!   the registry boundary rather than restating it.
//! - [`adopt`], which takes somebody else's [`Attestation`] plus an explicit [`AdoptionPolicy`]
//!   and produces a standing whose [`Basis`] records that it was adopted and from whom.
//!
//! # One hop, never two
//!
//! An adopted standing can be attested — and that attestation cannot be adopted.
//! [`FederationError::TrustWouldTransit`] refuses it. So A's assessment may become B's standing if
//! B has said in advance that it will accept A's word, and it can never become C's, because what
//! B holds is not an assessment but a decision to defer, and deferring to a deferral establishes
//! nothing. This is the runtime shadow of the type-level property: the constructor is missing, and
//! the one path that comes close is capped at a single hop.
//!
//! # The ceiling, and why it stops below the top
//!
//! An [`AdoptionPolicy`] names the peers a registry will defer to and the highest tier it will
//! grant on each one's word. A peer absent from the policy is not a peer: the empty policy — the
//! default — adopts nothing. And no ceiling may be `gold`, because `bioprism-registry` defines
//! gold as requiring "an approved independent review of this exact content", and a remote
//! registry's say-so is by construction not that. [`FederationError::CeilingTooHigh`] is that
//! sentence made into a check.
//!
//! # Not implemented
//!
//! No signatures, no key rotation, no delegation certificates, no revocation feed — 10.18 and
//! 10.20 want all four, and every one of them is transport. Nothing here verifies that an
//! attestation came from the registry named in it; that is the deployment's job, and this module
//! is what the deployment's answer is fed into. No visibility scopes, no residency rules, no
//! redaction: 10.19's public/organization/reviewer-group lattice is a policy engine, and its
//! inputs are not tiers.

use crate::name::{PackName, Version};
use crate::registry::RegistryId;
use crate::resolve::Resolved;
use bioprism_registry::{TierAssessment, TrustTier};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// What a name and version were at the moment a standing was established.
///
/// The digest is part of it because a standing is about *content*. A tier that survived a rebind
/// of the name would be a tier about a name, and names are not what anybody reviewed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Subject {
    pub name: PackName,
    pub version: Version,
    pub digest: String,
}

impl Subject {
    pub fn new(name: PackName, version: Version, digest: impl Into<String>) -> Self {
        Subject {
            name,
            version,
            digest: digest.into(),
        }
    }

    fn validate(&self) -> Result<(), FederationError> {
        if self.digest.trim().is_empty() {
            return Err(FederationError::InvalidState {
                detail: format!("subject {} has no digest", self.name),
            });
        }
        Ok(())
    }
}

impl From<&Resolved> for Subject {
    fn from(resolved: &Resolved) -> Self {
        Subject {
            name: resolved.name.clone(),
            version: resolved.version,
            digest: resolved.digest.clone(),
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{} ({})", self.name, self.version, self.digest)
    }
}

/// How a registry came to hold a standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// Computed here, from the pack document, by `bioprism-registry`.
    Earned,
    /// Deferred to a peer under a policy stated in advance. Not evidence about the pack; evidence
    /// about a decision this registry made.
    Adopted { from: RegistryId },
}

impl Basis {
    pub fn is_earned(&self) -> bool {
        matches!(self, Basis::Earned)
    }
}

impl fmt::Display for Basis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Basis::Earned => f.write_str("earned"),
            Basis::Adopted { from } => write!(f, "adopted from {from}"),
        }
    }
}

/// A tier a particular registry holds for a particular artifact.
///
/// The fields are private and there is no constructor taking a tier. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrustStanding {
    registry: RegistryId,
    subject: Subject,
    tier: TrustTier,
    basis: Basis,
}

#[derive(Debug, Deserialize)]
struct TrustStandingFields {
    registry: RegistryId,
    subject: Subject,
    tier: TrustTier,
    basis: Basis,
}

impl<'de> Deserialize<'de> for TrustStanding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = TrustStandingFields::deserialize(deserializer)?;
        let standing = TrustStanding {
            registry: fields.registry,
            subject: fields.subject,
            tier: fields.tier,
            basis: fields.basis,
        };
        standing
            .validate()
            .map(|()| standing)
            .map_err(D::Error::custom)
    }
}

impl TrustStanding {
    /// A standing computed here.
    ///
    /// The tier is taken from the assessment; there is no parameter for it. What this cannot check
    /// is that the assessment itself was honestly produced — that is `bioprism-registry`'s
    /// guarantee, which it gets by being a pure function of the pack bytes, and restating it here
    /// would be theatre.
    pub fn earned(registry: RegistryId, subject: Subject, assessment: &TierAssessment) -> Self {
        TrustStanding {
            registry,
            subject,
            tier: assessment.earned,
            basis: Basis::Earned,
        }
    }

    pub fn registry(&self) -> &RegistryId {
        &self.registry
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }

    pub fn tier(&self) -> TrustTier {
        self.tier
    }

    pub fn basis(&self) -> &Basis {
        &self.basis
    }

    fn validate(&self) -> Result<(), FederationError> {
        self.subject.validate()?;
        if let Basis::Adopted { from } = &self.basis {
            if from == &self.registry {
                return Err(FederationError::InvalidState {
                    detail: format!("{0} cannot hold an adoption from itself", self.registry),
                });
            }
            if self.tier >= TrustTier::Gold {
                return Err(FederationError::InvalidState {
                    detail: "an adopted standing cannot carry the gold tier".into(),
                });
            }
        }
        Ok(())
    }

    /// True when this registry worked the tier out rather than deferring to somebody.
    pub fn is_earned(&self) -> bool {
        self.basis.is_earned()
    }

    /// Whether this standing holds in a given registry. There is deliberately no way to make it
    /// hold in another one.
    pub fn holds_in(&self, registry: &RegistryId) -> bool {
        &self.registry == registry
    }

    /// Exports the standing for a peer to consider. An exported *adopted* standing is legal to
    /// produce and impossible to adopt; see [`adopt`].
    pub fn attest(&self) -> Attestation {
        Attestation {
            by: self.registry.clone(),
            subject: self.subject.clone(),
            tier: self.tier,
            basis: self.basis.clone(),
        }
    }
}

impl fmt::Display for TrustStanding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} holds {} at {} ({})",
            self.registry,
            self.subject,
            self.tier.as_str(),
            self.basis
        )
    }
}

/// What one registry says about an artifact, as another registry receives it.
///
/// Note what this is not: it is not a standing, it does not become one by being received, and
/// nothing about holding one entitles the holder to anything. It is a claim, with a name on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attestation {
    pub by: RegistryId,
    pub subject: Subject,
    pub tier: TrustTier,
    /// How the attesting registry came by the tier. This is the field that stops the second hop.
    pub basis: Basis,
}

#[derive(Debug, Deserialize)]
struct AttestationFields {
    by: RegistryId,
    subject: Subject,
    tier: TrustTier,
    basis: Basis,
}

impl<'de> Deserialize<'de> for Attestation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = AttestationFields::deserialize(deserializer)?;
        let attestation = Attestation {
            by: fields.by,
            subject: fields.subject,
            tier: fields.tier,
            basis: fields.basis,
        };
        attestation
            .validate()
            .map(|()| attestation)
            .map_err(D::Error::custom)
    }
}

impl Attestation {
    fn validate(&self) -> Result<(), FederationError> {
        self.subject.validate()
    }
}

impl fmt::Display for Attestation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} attests {} at {} ({})",
            self.by,
            self.subject,
            self.tier.as_str(),
            self.basis
        )
    }
}

/// The peers a registry will defer to, and how far.
///
/// Empty by default, which is the whole design: a registry that has said nothing about a peer
/// grants nothing on that peer's word, and a federation that grows by replication does not
/// thereby grow anybody's trust.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AdoptionPolicy {
    ceilings: BTreeMap<RegistryId, TrustTier>,
}

#[derive(Debug, Deserialize)]
struct AdoptionPolicyFields {
    ceilings: BTreeMap<RegistryId, TrustTier>,
}

impl<'de> Deserialize<'de> for AdoptionPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = AdoptionPolicyFields::deserialize(deserializer)?;
        let policy = AdoptionPolicy {
            ceilings: fields.ceilings,
        };
        policy.validate().map(|()| policy).map_err(D::Error::custom)
    }
}

impl AdoptionPolicy {
    pub fn new() -> Self {
        AdoptionPolicy::default()
    }

    /// Names a peer and the highest tier this registry will grant on its word.
    ///
    /// Refuses a ceiling of gold: `bioprism-registry` defines that rung as requiring an approved
    /// independent review of this exact content, and a peer's assertion is not one.
    pub fn recognising(
        mut self,
        peer: RegistryId,
        ceiling: TrustTier,
    ) -> Result<Self, FederationError> {
        if ceiling >= TrustTier::Gold {
            return Err(FederationError::CeilingTooHigh {
                peer,
                ceiling: ceiling.as_str(),
            });
        }
        self.ceilings.insert(peer, ceiling);
        Ok(self)
    }

    pub fn ceiling_for(&self, peer: &RegistryId) -> Option<TrustTier> {
        self.ceilings.get(peer).copied()
    }

    pub fn recognises(&self, peer: &RegistryId) -> bool {
        self.ceilings.contains_key(peer)
    }

    fn validate(&self) -> Result<(), FederationError> {
        if let Some((peer, ceiling)) = self
            .ceilings
            .iter()
            .find(|(_, ceiling)| **ceiling >= TrustTier::Gold)
        {
            return Err(FederationError::CeilingTooHigh {
                peer: peer.clone(),
                ceiling: ceiling.as_str(),
            });
        }
        Ok(())
    }
}

/// The record of one registry deciding to defer to another about one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Adoption {
    pub standing: TrustStanding,
    /// The tier the peer attested, when the local ceiling cut it down. `None` when nothing was
    /// lost. Kept because "we granted reviewed" and "they said gold and we granted reviewed" are
    /// different events and a registry should be able to show which one happened.
    pub capped_from: Option<TrustTier>,
}

#[derive(Debug, Deserialize)]
struct AdoptionFields {
    standing: TrustStanding,
    capped_from: Option<TrustTier>,
}

impl<'de> Deserialize<'de> for Adoption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = AdoptionFields::deserialize(deserializer)?;
        let adoption = Adoption {
            standing: fields.standing,
            capped_from: fields.capped_from,
        };
        adoption
            .validate()
            .map(|()| adoption)
            .map_err(D::Error::custom)
    }
}

impl Adoption {
    fn validate(&self) -> Result<(), FederationError> {
        self.standing.validate()?;
        if !matches!(&self.standing.basis, Basis::Adopted { .. }) {
            return Err(FederationError::InvalidState {
                detail: "an adoption must carry an adopted standing".into(),
            });
        }
        match (self.capped_from, self.standing.tier) {
            (Some(from), granted) if from > granted => Ok(()),
            (Some(_), _) => Err(FederationError::InvalidState {
                detail: "a recorded cap must be above the granted tier".into(),
            }),
            (None, _) => Ok(()),
        }
    }
}

/// Turns a peer's attestation into a local standing, or refuses to.
///
/// Refuses when: the peer is not in the policy; the attestation is itself adopted rather than
/// earned; or the adopting registry is the attesting one. The successful case still does not
/// produce an earned standing — [`Basis::Adopted`] names the peer, permanently.
pub fn adopt(
    into: &RegistryId,
    attestation: &Attestation,
    policy: &AdoptionPolicy,
) -> Result<Adoption, FederationError> {
    attestation.validate()?;
    policy.validate()?;
    if &attestation.by == into {
        return Err(FederationError::SelfAdoption {
            registry: into.clone(),
        });
    }
    if let Basis::Adopted { from } = &attestation.basis {
        return Err(FederationError::TrustWouldTransit {
            origin: from.clone(),
            via: attestation.by.clone(),
            into: into.clone(),
            subject: attestation.subject.to_string(),
        });
    }
    let ceiling =
        policy
            .ceiling_for(&attestation.by)
            .ok_or_else(|| FederationError::PeerNotRecognised {
                peer: attestation.by.clone(),
                into: into.clone(),
                subject: attestation.subject.to_string(),
            })?;

    let granted = attestation.tier.min(ceiling);
    let capped_from = (granted < attestation.tier).then_some(attestation.tier);
    Ok(Adoption {
        standing: TrustStanding {
            registry: into.clone(),
            subject: attestation.subject.clone(),
            tier: granted,
            basis: Basis::Adopted {
                from: attestation.by.clone(),
            },
        },
        capped_from,
    })
}

/// Why a trust claim did not cross a registry boundary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FederationError {
    #[error("invalid federation state: {detail}")]
    InvalidState { detail: String },

    #[error(
        "{into} does not recognise {peer} as an assessor, so {peer}'s attestation about {subject} \
         grants nothing here"
    )]
    PeerNotRecognised {
        peer: RegistryId,
        into: RegistryId,
        subject: String,
    },

    #[error(
        "{via} adopted its standing for {subject} from {origin}; adopting it again into {into} \
         would make trust transitive"
    )]
    TrustWouldTransit {
        origin: RegistryId,
        via: RegistryId,
        into: RegistryId,
        subject: String,
    },

    #[error("{registry} cannot adopt its own attestation")]
    SelfAdoption { registry: RegistryId },

    #[error(
        "a ceiling of {ceiling} for {peer} is not grantable on a peer's word: that rung requires \
         an approved independent review of this exact content"
    )]
    CeilingTooHigh {
        peer: RegistryId,
        ceiling: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_registry::RungAssessment;

    fn id(text: &str) -> RegistryId {
        RegistryId::parse(text).expect("parses")
    }

    fn subject() -> Subject {
        Subject::new(
            PackName::parse("bioprism/onco-tp53").expect("parses"),
            Version::new(1, 2, 0),
            "sha256:aa",
        )
    }

    fn assessed(tier: TrustTier) -> TierAssessment {
        TierAssessment {
            earned: tier,
            rungs: Vec::<RungAssessment>::new(),
        }
    }

    fn earned_in(registry: &str, tier: TrustTier) -> TrustStanding {
        TrustStanding::earned(id(registry), subject(), &assessed(tier))
    }

    fn policy(peer: &str, ceiling: TrustTier) -> AdoptionPolicy {
        AdoptionPolicy::new()
            .recognising(id(peer), ceiling)
            .expect("a ceiling below gold is grantable")
    }

    #[test]
    fn a_standing_earned_in_one_registry_does_not_hold_in_another() {
        let standing = earned_in("alpha", TrustTier::Reviewed);
        assert!(standing.holds_in(&id("alpha")));
        assert!(!standing.holds_in(&id("beta")));
    }

    #[test]
    fn an_attestation_is_not_a_standing_and_becomes_one_only_under_a_stated_policy() {
        let attestation = earned_in("alpha", TrustTier::Reviewed).attest();
        assert!(matches!(
            adopt(&id("beta"), &attestation, &AdoptionPolicy::new()),
            Err(FederationError::PeerNotRecognised { .. })
        ));
        let adoption = adopt(
            &id("beta"),
            &attestation,
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect("beta said in advance it would accept alpha's word");
        assert!(adoption.standing.holds_in(&id("beta")));
        assert_eq!(adoption.standing.tier(), TrustTier::Reviewed);
    }

    #[test]
    fn the_default_policy_adopts_nothing() {
        let empty = AdoptionPolicy::new();
        assert!(!empty.recognises(&id("alpha")));
        assert_eq!(empty.ceiling_for(&id("alpha")), None);
    }

    #[test]
    fn an_adopted_standing_records_that_it_was_adopted_and_from_whom() {
        let adoption = adopt(
            &id("beta"),
            &earned_in("alpha", TrustTier::Reviewed).attest(),
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect("adopts");
        assert!(!adoption.standing.is_earned());
        assert_eq!(
            adoption.standing.basis(),
            &Basis::Adopted { from: id("alpha") }
        );
    }

    #[test]
    fn trust_does_not_make_a_second_hop() {
        let adoption = adopt(
            &id("beta"),
            &earned_in("alpha", TrustTier::Reviewed).attest(),
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect("adopts");
        let relayed = adoption.standing.attest();

        let error = adopt(&id("gamma"), &relayed, &policy("beta", TrustTier::Reviewed))
            .expect_err("deferring to a deferral establishes nothing");
        assert!(matches!(
            error,
            FederationError::TrustWouldTransit { ref origin, ref via, .. }
                if origin == &id("alpha") && via == &id("beta")
        ));
    }

    #[test]
    fn a_registry_that_recognises_a_peer_still_caps_what_that_peer_can_confer() {
        let adoption = adopt(
            &id("beta"),
            &earned_in("alpha", TrustTier::Reviewed).attest(),
            &policy("alpha", TrustTier::GeneratedVerified),
        )
        .expect("adopts at the ceiling");
        assert_eq!(adoption.standing.tier(), TrustTier::GeneratedVerified);
        assert_eq!(adoption.capped_from, Some(TrustTier::Reviewed));
    }

    #[test]
    fn an_uncapped_adoption_records_no_cap() {
        let adoption = adopt(
            &id("beta"),
            &earned_in("alpha", TrustTier::Exploratory).attest(),
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect("adopts");
        assert_eq!(adoption.standing.tier(), TrustTier::Exploratory);
        assert_eq!(adoption.capped_from, None);
    }

    #[test]
    fn no_policy_may_grant_the_top_rung_on_a_peers_word() {
        let error = AdoptionPolicy::new()
            .recognising(id("alpha"), TrustTier::Gold)
            .expect_err("gold requires an independent review of this exact content");
        assert!(matches!(error, FederationError::CeilingTooHigh { .. }));
    }

    #[test]
    fn a_registry_cannot_adopt_its_own_attestation_to_launder_the_basis() {
        let error = adopt(
            &id("alpha"),
            &earned_in("alpha", TrustTier::Reviewed).attest(),
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect_err("a round trip through the federation is not evidence");
        assert!(matches!(error, FederationError::SelfAdoption { .. }));
    }

    #[test]
    fn the_tier_of_an_earned_standing_comes_from_the_assessment_and_not_from_the_caller() {
        let standing = TrustStanding::earned(
            id("alpha"),
            subject(),
            &assessed(TrustTier::GeneratedVerified),
        );
        assert_eq!(standing.tier(), TrustTier::GeneratedVerified);
        assert!(standing.is_earned());
    }

    #[test]
    fn a_standing_is_about_content_so_a_different_digest_is_a_different_subject() {
        let one = earned_in("alpha", TrustTier::Reviewed);
        let other = TrustStanding::earned(
            id("alpha"),
            Subject::new(
                PackName::parse("bioprism/onco-tp53").expect("parses"),
                Version::new(1, 2, 0),
                "sha256:bb",
            ),
            &assessed(TrustTier::Reviewed),
        );
        assert_ne!(one.subject(), other.subject());
    }

    #[test]
    fn an_attestation_survives_a_json_round_trip_with_its_basis_intact() {
        let adoption = adopt(
            &id("beta"),
            &earned_in("alpha", TrustTier::Reviewed).attest(),
            &policy("alpha", TrustTier::Reviewed),
        )
        .expect("adopts");
        let relayed = adoption.standing.attest();
        let text = serde_json::to_string(&relayed).expect("serialises");
        assert!(text.contains("adopted"));
        let back: Attestation = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, relayed);
        assert!(adopt(&id("gamma"), &back, &policy("beta", TrustTier::Reviewed)).is_err());
    }

    #[test]
    fn policy_deserialization_cannot_restore_a_gold_peer_ceiling() {
        let policy = policy("alpha", TrustTier::Reviewed);
        let mut value = serde_json::to_value(&policy).expect("serialises");
        value["ceilings"]["alpha"] = serde_json::json!("gold");
        assert!(serde_json::from_value::<AdoptionPolicy>(value).is_err());
    }

    #[test]
    fn standing_and_attestation_deserialization_recheck_content_identity() {
        let standing = earned_in("alpha", TrustTier::Reviewed);
        let mut value = serde_json::to_value(&standing).expect("serialises");
        value["subject"]["digest"] = serde_json::json!(" ");
        assert!(serde_json::from_value::<TrustStanding>(value).is_err());

        let attestation = standing.attest();
        let mut value = serde_json::to_value(&attestation).expect("serialises");
        value["subject"]["digest"] = serde_json::json!("");
        assert!(serde_json::from_value::<Attestation>(value).is_err());
    }
}
