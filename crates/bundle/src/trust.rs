//! Offline key lifecycle, delegation, and authorization policy.
//!
//! [`crate::signature`] proves that a private Ed25519 key signed canonical bytes. This module
//! supplies the missing, explicitly separate policy layer: a caller can carry a deterministic
//! registry snapshot containing trust anchors, attenuating delegations, revocations, and rotations,
//! then evaluate a bundle against that snapshot without contacting a service or reading a clock.
//!
//! The registry is intentionally a value rather than a singleton. A snapshot is only authoritative
//! to the process that chose it; importing it does not create an identity provider, a transparency
//! log, an HSM, or a timestamp authority. Those boundaries remain visible in [`TrustReport`].
//!
//! The policy is fail-closed in four important ways:
//!
//! * a key must be present in the snapshot and its public material must verify the attestation;
//! * delegation can only narrow roles, producer identity, and validity windows;
//! * revocation and rotation are evaluated at an explicit signing or evaluation instant; and
//! * purpose, role, producer binding, and root/delegated status are independent checks.

use crate::attestation::{AttestationPurpose, KeyHolderAuthenticated};
use crate::mac::KeyIdentity;
use crate::signature::{
    Ed25519PublicKey, Ed25519Signature, KeyValidity, PublicKeyAttestation, SigningKey,
    VerificationKey,
};
use bioprism_ids::CanonicalError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Wire version for registry snapshots and their signed lifecycle records.
pub const TRUST_REGISTRY_SCHEMA_VERSION: &str = "bioprism-key-registry/0.1";
/// Maximum number of registered keys in one offline snapshot.
pub const MAX_TRUST_KEYS: usize = 4096;
/// Maximum number of lifecycle records in one offline snapshot.
pub const MAX_TRUST_EVENTS: usize = 8192;
/// Maximum supported delegation chain depth.
pub const MAX_DELEGATION_DEPTH: usize = 32;

/// Roles that a registry can bind to a key. Roles are deliberately narrower than an arbitrary
/// permission string so purpose separation remains reviewable and deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRole {
    Publisher,
    Builder,
    Hub,
    Auditor,
    Reproducer,
    Delegator,
    Revoker,
    Rotator,
}

impl KeyRole {
    /// The default role required by a purpose-specific attestation policy.
    pub fn for_purpose(purpose: AttestationPurpose) -> Self {
        match purpose {
            AttestationPurpose::PublisherManifest => Self::Publisher,
            AttestationPurpose::BuilderProvenance => Self::Builder,
            AttestationPurpose::HubPublicationReceipt => Self::Hub,
            AttestationPurpose::AuditCheckpoint => Self::Auditor,
            AttestationPurpose::IndependentReproduction => Self::Reproducer,
        }
    }
}

impl fmt::Display for KeyRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Publisher => "publisher",
            Self::Builder => "builder",
            Self::Hub => "hub",
            Self::Auditor => "auditor",
            Self::Reproducer => "reproducer",
            Self::Delegator => "delegator",
            Self::Revoker => "revoker",
            Self::Rotator => "rotator",
        };
        f.write_str(value)
    }
}

/// A key record carried by a [`KeyRegistry`].
///
/// `producer` is meaningful only because a trusted snapshot assigned it. It is not derived from
/// the public key, and it is not a claim that the key was stored safely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredKey {
    pub identity: KeyIdentity,
    pub public_key: Ed25519PublicKey,
    pub validity: KeyValidity,
    pub producer: String,
    pub roles: BTreeSet<KeyRole>,
    pub delegable_roles: BTreeSet<KeyRole>,
    pub issuer: Option<KeyIdentity>,
}

impl RegisteredKey {
    pub fn root(
        identity: KeyIdentity,
        public_key: Ed25519PublicKey,
        validity: KeyValidity,
        producer: impl Into<String>,
        roles: BTreeSet<KeyRole>,
        delegable_roles: BTreeSet<KeyRole>,
    ) -> Result<Self, TrustError> {
        let key = Self {
            identity,
            public_key,
            validity,
            producer: producer.into(),
            roles,
            delegable_roles,
            issuer: None,
        };
        key.validate_shape()?;
        Ok(key)
    }

    pub fn delegated(
        identity: KeyIdentity,
        public_key: Ed25519PublicKey,
        validity: KeyValidity,
        producer: impl Into<String>,
        roles: BTreeSet<KeyRole>,
        delegable_roles: BTreeSet<KeyRole>,
        issuer: KeyIdentity,
    ) -> Result<Self, TrustError> {
        let key = Self {
            identity,
            public_key,
            validity,
            producer: producer.into(),
            roles,
            delegable_roles,
            issuer: Some(issuer),
        };
        key.validate_shape()?;
        Ok(key)
    }

    pub fn verification_key(&self) -> VerificationKey {
        VerificationKey::new(
            self.identity.clone(),
            self.public_key,
            self.validity.clone(),
        )
    }

    fn validate_shape(&self) -> Result<(), TrustError> {
        validate_identity(&self.identity)?;
        validate_text("registered producer", &self.producer)?;
        if let (Some(before), Some(after)) = (self.validity.not_before, self.validity.not_after) {
            if before > after {
                return Err(TrustError::MalformedRegistry {
                    detail: format!(
                        "key `{}` has an inverted validity window: {before} > {after}",
                        self.identity
                    ),
                });
            }
        }
        if self.roles.is_empty() {
            return Err(TrustError::MalformedRegistry {
                detail: format!("key `{}` has no roles", self.identity),
            });
        }
        if !self.delegable_roles.is_subset(&self.roles) {
            return Err(TrustError::MalformedRegistry {
                detail: format!(
                    "key `{}` delegates a role it does not itself hold",
                    self.identity
                ),
            });
        }
        if self.issuer.as_ref() == Some(&self.identity) {
            return Err(TrustError::DelegationCycle {
                identity: self.identity.clone(),
            });
        }
        Ok(())
    }
}

/// A signed, attenuating certificate from one registered key to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDelegation {
    pub schema_version: String,
    pub issuer: KeyIdentity,
    pub subject: KeyIdentity,
    pub subject_public_key: Ed25519PublicKey,
    pub subject_validity: KeyValidity,
    pub subject_producer: String,
    pub subject_roles: BTreeSet<KeyRole>,
    pub subject_delegable_roles: BTreeSet<KeyRole>,
    pub issued_at: u64,
    pub signature: Ed25519Signature,
}

impl KeyDelegation {
    pub fn produce(
        issuer: &SigningKey,
        subject: &VerificationKey,
        producer: impl Into<String>,
        roles: BTreeSet<KeyRole>,
        delegable_roles: BTreeSet<KeyRole>,
        issued_at: u64,
    ) -> Result<Self, TrustError> {
        let producer = producer.into();
        validate_text("delegated producer", &producer)?;
        let delegation = Self {
            schema_version: TRUST_REGISTRY_SCHEMA_VERSION.to_string(),
            issuer: issuer.identity().clone(),
            subject: subject.identity().clone(),
            subject_public_key: subject.public_key(),
            subject_validity: subject.validity().clone(),
            subject_producer: producer,
            subject_roles: roles,
            subject_delegable_roles: delegable_roles,
            issued_at,
            signature: Ed25519Signature::from_bytes([0; 64]),
        };
        let preimage = delegation_preimage(&delegation)?;
        Ok(Self {
            signature: issuer.sign_bytes(&preimage),
            ..delegation
        })
    }
}

/// A signed revocation statement. `revoked_at` is the effective status time; `signed_at` is when
/// the authority key signed this statement and is checked against that authority's own validity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRevocation {
    pub schema_version: String,
    pub target: KeyIdentity,
    pub authority: KeyIdentity,
    pub revoked_at: u64,
    pub signed_at: u64,
    pub reason: String,
    pub signature: Ed25519Signature,
}

impl KeyRevocation {
    pub fn produce(
        authority: &SigningKey,
        target: KeyIdentity,
        revoked_at: u64,
        signed_at: u64,
        reason: impl Into<String>,
    ) -> Result<Self, TrustError> {
        if signed_at > revoked_at {
            return Err(TrustError::MalformedEvent {
                detail: "revocation signed_at cannot be after revoked_at".into(),
            });
        }
        validate_identity(&target)?;
        let reason = reason.into();
        validate_text("revocation reason", &reason)?;
        let event = Self {
            schema_version: TRUST_REGISTRY_SCHEMA_VERSION.to_string(),
            target,
            authority: authority.identity().clone(),
            revoked_at,
            signed_at,
            reason,
            signature: Ed25519Signature::from_bytes([0; 64]),
        };
        let preimage = revocation_preimage(&event)?;
        Ok(Self {
            signature: authority.sign_bytes(&preimage),
            ..event
        })
    }
}

/// A signed successor relationship. Rotation never silently changes historical validity: a key
/// remains usable for attestations before `effective_at`, while later policy checks use the new key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRotation {
    pub schema_version: String,
    pub predecessor: KeyIdentity,
    pub successor: KeyIdentity,
    pub successor_public_key: Ed25519PublicKey,
    pub effective_at: u64,
    pub signed_at: u64,
    pub signature: Ed25519Signature,
}

impl KeyRotation {
    pub fn produce(
        predecessor: &SigningKey,
        successor: &VerificationKey,
        effective_at: u64,
        signed_at: u64,
    ) -> Result<Self, TrustError> {
        if signed_at > effective_at {
            return Err(TrustError::MalformedEvent {
                detail: "rotation signed_at cannot be after effective_at".into(),
            });
        }
        if predecessor.identity() == successor.identity() {
            return Err(TrustError::MalformedEvent {
                detail: "rotation predecessor and successor must differ".into(),
            });
        }
        let event = Self {
            schema_version: TRUST_REGISTRY_SCHEMA_VERSION.to_string(),
            predecessor: predecessor.identity().clone(),
            successor: successor.identity().clone(),
            successor_public_key: successor.public_key(),
            effective_at,
            signed_at,
            signature: Ed25519Signature::from_bytes([0; 64]),
        };
        let preimage = rotation_preimage(&event)?;
        Ok(Self {
            signature: predecessor.sign_bytes(&preimage),
            ..event
        })
    }
}

/// A policy supplied by the verifier, never inferred from a producer string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustPolicy {
    pub purpose: AttestationPurpose,
    #[serde(default)]
    pub required_role: Option<KeyRole>,
    #[serde(default)]
    pub expected_producer: Option<String>,
    #[serde(default = "default_true")]
    pub require_producer_binding: bool,
    #[serde(default = "default_true")]
    pub require_signed_at: bool,
    #[serde(default)]
    pub require_delegated: bool,
    #[serde(default = "default_true")]
    pub allow_root: bool,
    #[serde(default)]
    pub as_of: Option<u64>,
    #[serde(default = "default_max_delegation_depth")]
    pub max_delegation_depth: usize,
}

impl TrustPolicy {
    pub fn for_purpose(purpose: AttestationPurpose) -> Self {
        Self {
            purpose,
            required_role: Some(KeyRole::for_purpose(purpose)),
            expected_producer: None,
            require_producer_binding: true,
            require_signed_at: true,
            require_delegated: false,
            allow_root: true,
            as_of: None,
            max_delegation_depth: MAX_DELEGATION_DEPTH,
        }
    }
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self::for_purpose(AttestationPurpose::PublisherManifest)
    }
}

/// A successful registry-backed trust result. The report states both the positive checks and the
/// boundaries that a local snapshot cannot establish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustReport {
    pub schema_version: String,
    pub verdict: TrustVerdict,
    pub key_identity: KeyIdentity,
    pub purpose: AttestationPurpose,
    pub signed_at: Option<u64>,
    pub evaluated_at: Option<u64>,
    pub producer: String,
    pub roles: Vec<KeyRole>,
    pub delegation_chain: Vec<KeyIdentity>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustVerdict {
    Trusted,
}

/// A deterministic in-memory trust snapshot. The maps are public so the snapshot can be persisted
/// or transported as JSON, but all mutation helpers validate a candidate copy before committing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRegistry {
    pub schema_version: String,
    pub keys: BTreeMap<KeyIdentity, RegisteredKey>,
    pub delegations: BTreeMap<KeyIdentity, KeyDelegation>,
    pub rotations: BTreeMap<KeyIdentity, KeyRotation>,
    pub revocations: BTreeMap<KeyIdentity, KeyRevocation>,
}

impl Default for KeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyRegistry {
    pub fn new() -> Self {
        Self {
            schema_version: TRUST_REGISTRY_SCHEMA_VERSION.to_string(),
            keys: BTreeMap::new(),
            delegations: BTreeMap::new(),
            rotations: BTreeMap::new(),
            revocations: BTreeMap::new(),
        }
    }

    pub fn register_root(&mut self, key: RegisteredKey) -> Result<(), TrustError> {
        if key.issuer.is_some() {
            return Err(TrustError::MalformedRegistry {
                detail: format!("key `{}` is not a root", key.identity),
            });
        }
        if self.keys.contains_key(&key.identity) {
            return Err(TrustError::DuplicateKey {
                identity: key.identity,
            });
        }
        let mut candidate = self.clone();
        candidate.keys.insert(key.identity.clone(), key);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn import_delegation(&mut self, delegation: KeyDelegation) -> Result<(), TrustError> {
        if self.keys.contains_key(&delegation.subject) {
            return Err(TrustError::DuplicateKey {
                identity: delegation.subject,
            });
        }
        let subject = RegisteredKey::delegated(
            delegation.subject.clone(),
            delegation.subject_public_key,
            delegation.subject_validity.clone(),
            delegation.subject_producer.clone(),
            delegation.subject_roles.clone(),
            delegation.subject_delegable_roles.clone(),
            delegation.issuer.clone(),
        )?;
        let mut candidate = self.clone();
        candidate
            .delegations
            .insert(delegation.subject.clone(), delegation);
        candidate.keys.insert(subject.identity.clone(), subject);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn import_rotation(&mut self, rotation: KeyRotation) -> Result<(), TrustError> {
        if self.rotations.contains_key(&rotation.predecessor) {
            return Err(TrustError::DuplicateRotation {
                identity: rotation.predecessor,
            });
        }
        let mut candidate = self.clone();
        candidate
            .rotations
            .insert(rotation.predecessor.clone(), rotation);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn import_revocation(&mut self, revocation: KeyRevocation) -> Result<(), TrustError> {
        if self.revocations.contains_key(&revocation.target) {
            return Err(TrustError::DuplicateRevocation {
                identity: revocation.target,
            });
        }
        let mut candidate = self.clone();
        candidate
            .revocations
            .insert(revocation.target.clone(), revocation);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn registered(&self, identity: &KeyIdentity) -> Option<&RegisteredKey> {
        self.keys.get(identity)
    }

    /// Validates every relationship and signature in the snapshot.
    pub fn validate(&self) -> Result<(), TrustError> {
        if self.schema_version != TRUST_REGISTRY_SCHEMA_VERSION {
            return Err(TrustError::MalformedRegistry {
                detail: format!("unsupported registry schema `{}`", self.schema_version),
            });
        }
        if self.keys.len() > MAX_TRUST_KEYS {
            return Err(TrustError::MalformedRegistry {
                detail: format!("registry contains more than {MAX_TRUST_KEYS} keys"),
            });
        }
        if self.delegations.len() + self.rotations.len() + self.revocations.len() > MAX_TRUST_EVENTS
        {
            return Err(TrustError::MalformedRegistry {
                detail: format!("registry contains more than {MAX_TRUST_EVENTS} lifecycle events"),
            });
        }
        for (identity, key) in &self.keys {
            if identity != &key.identity {
                return Err(TrustError::MalformedRegistry {
                    detail: format!(
                        "key map entry `{identity}` disagrees with record `{}`",
                        key.identity
                    ),
                });
            }
            key.validate_shape()?;
        }
        self.validate_delegation_graph()?;
        for (subject, delegation) in &self.delegations {
            if subject != &delegation.subject {
                return Err(TrustError::MalformedRegistry {
                    detail: format!(
                        "delegation map entry `{subject}` disagrees with subject `{}`",
                        delegation.subject
                    ),
                });
            }
            self.validate_delegation(delegation)?;
        }
        for (predecessor, rotation) in &self.rotations {
            if predecessor != &rotation.predecessor {
                return Err(TrustError::MalformedRegistry {
                    detail: format!(
                        "rotation map entry `{predecessor}` disagrees with predecessor `{}`",
                        rotation.predecessor
                    ),
                });
            }
            self.validate_rotation(rotation)?;
        }
        for (target, revocation) in &self.revocations {
            if target != &revocation.target {
                return Err(TrustError::MalformedRegistry {
                    detail: format!(
                        "revocation map entry `{target}` disagrees with target `{}`",
                        revocation.target
                    ),
                });
            }
            self.validate_revocation(revocation)?;
        }
        Ok(())
    }

    /// Verify an attestation and apply the explicit role, producer, validity, rotation and
    /// revocation policy. The returned key-holder authentication is still the cryptographic fact;
    /// the report is the separate registry-backed interpretation.
    pub fn verify_attestation(
        &self,
        attestation: &PublicKeyAttestation,
        policy: &TrustPolicy,
    ) -> Result<(KeyHolderAuthenticated, TrustReport), TrustError> {
        self.validate()?;
        if let Some(expected) = &policy.expected_producer {
            if expected.is_empty() || expected.chars().any(char::is_control) {
                return Err(TrustError::MalformedPolicy {
                    detail: "expected_producer must be non-empty and contain no control characters"
                        .into(),
                });
            }
        }
        if policy.max_delegation_depth == 0 || policy.max_delegation_depth > MAX_DELEGATION_DEPTH {
            return Err(TrustError::MalformedPolicy {
                detail: format!(
                    "max_delegation_depth must be between 1 and {MAX_DELEGATION_DEPTH}"
                ),
            });
        }
        if attestation.purpose != policy.purpose {
            return Err(TrustError::PurposeNotAuthorized {
                attested: attestation.purpose,
                requested: policy.purpose,
            });
        }
        let key =
            self.keys
                .get(&attestation.key_identity)
                .ok_or_else(|| TrustError::UnknownKey {
                    identity: attestation.key_identity.clone(),
                })?;
        let signed_at = attestation.signed_at;
        if let (Some(evaluated_at), Some(signed_at)) = (policy.as_of, signed_at) {
            if evaluated_at < signed_at {
                return Err(TrustError::MalformedPolicy {
                    detail: format!(
                        "as_of {evaluated_at} cannot precede attestation signed_at {signed_at}"
                    ),
                });
            }
        }
        if policy.require_signed_at && signed_at.is_none() {
            return Err(TrustError::MissingSigningTime {
                identity: key.identity.clone(),
            });
        }
        if let Some(at) = signed_at {
            ensure_valid_at(&key.validity, at, &key.identity)?;
        } else if key.validity.not_before.is_some() || key.validity.not_after.is_some() {
            return Err(TrustError::MissingSigningTime {
                identity: key.identity.clone(),
            });
        }
        let chain = self.delegation_chain(&key.identity)?;
        let depth = chain.len().saturating_sub(1);
        if depth > policy.max_delegation_depth {
            return Err(TrustError::DelegationTooDeep { depth });
        }
        if policy.require_delegated && key.issuer.is_none() {
            return Err(TrustError::RootNotAllowed {
                identity: key.identity.clone(),
            });
        }
        if !policy.allow_root && key.issuer.is_none() {
            return Err(TrustError::RootNotAllowed {
                identity: key.identity.clone(),
            });
        }
        let required_role = policy
            .required_role
            .unwrap_or_else(|| KeyRole::for_purpose(policy.purpose));
        if !key.roles.contains(&required_role) {
            return Err(TrustError::RoleNotAuthorized {
                identity: key.identity.clone(),
                role: required_role,
                purpose: policy.purpose,
            });
        }
        if policy.require_producer_binding
            && key.producer != attestation.claimed_producer.claimed_name()
        {
            return Err(TrustError::ProducerMismatch {
                identity: key.identity.clone(),
                registered: key.producer.clone(),
                claimed: attestation.claimed_producer.claimed_name().to_string(),
            });
        }
        if let Some(expected) = &policy.expected_producer {
            if expected != &key.producer || expected != attestation.claimed_producer.claimed_name()
            {
                return Err(TrustError::ExpectedProducerMismatch {
                    expected: expected.clone(),
                    registered: key.producer.clone(),
                    claimed: attestation.claimed_producer.claimed_name().to_string(),
                });
            }
        }
        let status_at = policy.as_of.or(signed_at);
        if let Some(at) = status_at {
            match self.status_at(&key.identity, at)? {
                KeyStatus::Active => {}
                KeyStatus::Revoked { revoked_at, reason } => {
                    return Err(TrustError::KeyRevoked {
                        identity: key.identity.clone(),
                        at,
                        revoked_at,
                        reason,
                    })
                }
                KeyStatus::Superseded {
                    successor,
                    effective_at,
                } => {
                    return Err(TrustError::KeySuperseded {
                        identity: key.identity.clone(),
                        successor,
                        at,
                        effective_at,
                    })
                }
            }
        }
        let authenticated = attestation
            .verify_for_or_error(&key.verification_key(), policy.purpose)
            .map_err(|error| TrustError::Cryptographic {
                detail: error.to_string(),
            })?;
        let report = TrustReport {
            schema_version: TRUST_REGISTRY_SCHEMA_VERSION.to_string(),
            verdict: TrustVerdict::Trusted,
            key_identity: key.identity.clone(),
            purpose: policy.purpose,
            signed_at,
            evaluated_at: status_at,
            producer: key.producer.clone(),
            roles: key.roles.iter().copied().collect(),
            delegation_chain: chain,
            guarantees: vec![
                "the attestation signature verifies under public material registered for this key identity".into(),
                "the requested attestation purpose is authorized by policy".into(),
                format!("the key holds the `{required_role}` role"),
                "delegations, if present, were checked for attenuation and valid issuer signatures".into(),
                "the evaluated revocation and rotation snapshot was active at the requested instant".into(),
            ],
            limitations: vec![
                "the snapshot is caller-supplied and is not an external identity or timestamp authority".into(),
                "private-key custody, compromise history, hardware protection, and legal attribution remain outside this report".into(),
                "referenced bundle entries are still verified only by their carried digest".into(),
            ],
        };
        Ok((authenticated, report))
    }

    fn validate_delegation_graph(&self) -> Result<(), TrustError> {
        for key in self.keys.values() {
            let mut seen = BTreeSet::new();
            let mut current = key.issuer.clone();
            let mut depth = 0;
            while let Some(identity) = current {
                if !seen.insert(identity.clone()) {
                    return Err(TrustError::DelegationCycle { identity });
                }
                depth += 1;
                if depth > MAX_DELEGATION_DEPTH {
                    return Err(TrustError::DelegationTooDeep { depth });
                }
                current = self
                    .keys
                    .get(&identity)
                    .ok_or_else(|| TrustError::UnknownIssuer {
                        identity: identity.clone(),
                    })?
                    .issuer
                    .clone();
            }
        }
        Ok(())
    }

    fn validate_delegation(&self, delegation: &KeyDelegation) -> Result<(), TrustError> {
        if delegation.schema_version != TRUST_REGISTRY_SCHEMA_VERSION {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "unsupported delegation schema `{}`",
                    delegation.schema_version
                ),
            });
        }
        validate_identity(&delegation.issuer)?;
        validate_identity(&delegation.subject)?;
        validate_text("delegated producer", &delegation.subject_producer)?;
        let issuer =
            self.keys
                .get(&delegation.issuer)
                .ok_or_else(|| TrustError::UnknownIssuer {
                    identity: delegation.issuer.clone(),
                })?;
        let subject = self
            .keys
            .get(&delegation.subject)
            .ok_or_else(|| TrustError::UnknownKey {
                identity: delegation.subject.clone(),
            })?;
        if subject.public_key != delegation.subject_public_key
            || subject.validity != delegation.subject_validity
            || subject.producer != delegation.subject_producer
            || subject.roles != delegation.subject_roles
            || subject.delegable_roles != delegation.subject_delegable_roles
        {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "delegation for `{}` disagrees with its registered subject",
                    delegation.subject
                ),
            });
        }
        if !issuer.roles.contains(&KeyRole::Delegator) {
            return Err(TrustError::IssuerLacksRole {
                identity: issuer.identity.clone(),
                role: KeyRole::Delegator,
            });
        }
        if !delegation.subject_roles.is_subset(&issuer.delegable_roles)
            || !delegation
                .subject_delegable_roles
                .is_subset(&issuer.delegable_roles)
        {
            return Err(TrustError::DelegationWidensAuthority {
                issuer: issuer.identity.clone(),
                subject: subject.identity.clone(),
            });
        }
        ensure_valid_at(&issuer.validity, delegation.issued_at, &issuer.identity)?;
        if !matches!(
            self.status_at(&issuer.identity, delegation.issued_at)?,
            KeyStatus::Active
        ) {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "delegation issuer `{}` was not active when it delegated",
                    issuer.identity
                ),
            });
        }
        ensure_narrower(
            &issuer.validity,
            &delegation.subject_validity,
            &issuer.identity,
            &subject.identity,
        )?;
        let preimage = delegation_preimage(delegation)?;
        issuer
            .verification_key()
            .verify_bytes(&preimage, &delegation.signature)
            .map_err(|error| TrustError::InvalidEventSignature {
                event: format!("delegation to `{}`", delegation.subject),
                detail: error.to_string(),
            })?;
        Ok(())
    }

    fn validate_rotation(&self, rotation: &KeyRotation) -> Result<(), TrustError> {
        if rotation.schema_version != TRUST_REGISTRY_SCHEMA_VERSION {
            return Err(TrustError::MalformedEvent {
                detail: format!("unsupported rotation schema `{}`", rotation.schema_version),
            });
        }
        let predecessor =
            self.keys
                .get(&rotation.predecessor)
                .ok_or_else(|| TrustError::UnknownKey {
                    identity: rotation.predecessor.clone(),
                })?;
        let successor =
            self.keys
                .get(&rotation.successor)
                .ok_or_else(|| TrustError::UnknownKey {
                    identity: rotation.successor.clone(),
                })?;
        if successor.public_key != rotation.successor_public_key {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "rotation successor `{}` has different public material",
                    successor.identity
                ),
            });
        }
        if !predecessor.roles.contains(&KeyRole::Rotator) {
            return Err(TrustError::IssuerLacksRole {
                identity: predecessor.identity.clone(),
                role: KeyRole::Rotator,
            });
        }
        if rotation.signed_at > rotation.effective_at {
            return Err(TrustError::MalformedEvent {
                detail: "rotation signed_at cannot be after effective_at".into(),
            });
        }
        ensure_valid_at(
            &predecessor.validity,
            rotation.signed_at,
            &predecessor.identity,
        )?;
        if !matches!(
            self.status_at(&predecessor.identity, rotation.signed_at)?,
            KeyStatus::Active
        ) {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "rotation predecessor `{}` was not active when it rotated",
                    predecessor.identity
                ),
            });
        }
        if let Some(not_before) = successor.validity.not_before {
            if not_before > rotation.effective_at {
                return Err(TrustError::MalformedEvent {
                    detail: format!(
                        "successor `{}` activates after its rotation takes effect",
                        successor.identity
                    ),
                });
            }
        }
        if !successor.roles.is_subset(&predecessor.roles)
            || !successor
                .delegable_roles
                .is_subset(&predecessor.delegable_roles)
        {
            return Err(TrustError::DelegationWidensAuthority {
                issuer: predecessor.identity.clone(),
                subject: successor.identity.clone(),
            });
        }
        let preimage = rotation_preimage(rotation)?;
        predecessor
            .verification_key()
            .verify_bytes(&preimage, &rotation.signature)
            .map_err(|error| TrustError::InvalidEventSignature {
                event: format!("rotation to `{}`", rotation.successor),
                detail: error.to_string(),
            })?;
        Ok(())
    }

    fn validate_revocation(&self, revocation: &KeyRevocation) -> Result<(), TrustError> {
        if revocation.schema_version != TRUST_REGISTRY_SCHEMA_VERSION {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "unsupported revocation schema `{}`",
                    revocation.schema_version
                ),
            });
        }
        if revocation.signed_at > revocation.revoked_at {
            return Err(TrustError::MalformedEvent {
                detail: "revocation signed_at cannot be after revoked_at".into(),
            });
        }
        let authority =
            self.keys
                .get(&revocation.authority)
                .ok_or_else(|| TrustError::UnknownIssuer {
                    identity: revocation.authority.clone(),
                })?;
        if !authority.roles.contains(&KeyRole::Revoker) {
            return Err(TrustError::IssuerLacksRole {
                identity: authority.identity.clone(),
                role: KeyRole::Revoker,
            });
        }
        ensure_valid_at(
            &authority.validity,
            revocation.signed_at,
            &authority.identity,
        )?;
        if !matches!(
            self.status_at(&authority.identity, revocation.signed_at)?,
            KeyStatus::Active
        ) {
            return Err(TrustError::MalformedEvent {
                detail: format!(
                    "revocation authority `{}` was not active when it signed",
                    authority.identity
                ),
            });
        }
        let target = self
            .keys
            .get(&revocation.target)
            .ok_or_else(|| TrustError::UnknownKey {
                identity: revocation.target.clone(),
            })?;
        validate_text("revocation reason", &revocation.reason)?;
        let preimage = revocation_preimage(revocation)?;
        authority
            .verification_key()
            .verify_bytes(&preimage, &revocation.signature)
            .map_err(|error| TrustError::InvalidEventSignature {
                event: format!("revocation of `{}`", target.identity),
                detail: error.to_string(),
            })?;
        Ok(())
    }

    fn delegation_chain(&self, identity: &KeyIdentity) -> Result<Vec<KeyIdentity>, TrustError> {
        let mut chain = Vec::new();
        let mut current = Some(identity.clone());
        while let Some(next) = current {
            let key = self.keys.get(&next).ok_or_else(|| TrustError::UnknownKey {
                identity: next.clone(),
            })?;
            chain.push(next);
            current = key.issuer.clone();
            if chain.len() > MAX_DELEGATION_DEPTH + 1 {
                return Err(TrustError::DelegationTooDeep { depth: chain.len() });
            }
        }
        chain.reverse();
        Ok(chain)
    }

    fn status_at(&self, identity: &KeyIdentity, at: u64) -> Result<KeyStatus, TrustError> {
        if let Some(revocation) = self.revocations.get(identity) {
            if at >= revocation.revoked_at {
                return Ok(KeyStatus::Revoked {
                    revoked_at: revocation.revoked_at,
                    reason: revocation.reason.clone(),
                });
            }
        }
        if let Some(rotation) = self.rotations.get(identity) {
            if at >= rotation.effective_at {
                return Ok(KeyStatus::Superseded {
                    successor: rotation.successor.clone(),
                    effective_at: rotation.effective_at,
                });
            }
        }
        Ok(KeyStatus::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KeyStatus {
    Active,
    Revoked {
        revoked_at: u64,
        reason: String,
    },
    Superseded {
        successor: KeyIdentity,
        effective_at: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrustError {
    #[error("trust registry is malformed: {detail}")]
    MalformedRegistry { detail: String },
    #[error("trust policy is malformed: {detail}")]
    MalformedPolicy { detail: String },
    #[error("trust lifecycle event is malformed: {detail}")]
    MalformedEvent { detail: String },
    #[error("key `{identity}` is already registered")]
    DuplicateKey { identity: KeyIdentity },
    #[error("rotation for key `{identity}` is already present")]
    DuplicateRotation { identity: KeyIdentity },
    #[error("revocation for key `{identity}` is already present")]
    DuplicateRevocation { identity: KeyIdentity },
    #[error("key `{identity}` is not present in the trust registry")]
    UnknownKey { identity: KeyIdentity },
    #[error("issuer `{identity}` is not present in the trust registry")]
    UnknownIssuer { identity: KeyIdentity },
    #[error("delegation graph contains a cycle at `{identity}`")]
    DelegationCycle { identity: KeyIdentity },
    #[error("delegation chain depth {depth} exceeds the supported bound")]
    DelegationTooDeep { depth: usize },
    #[error("issuer `{identity}` does not hold the `{role}` role")]
    IssuerLacksRole {
        identity: KeyIdentity,
        role: KeyRole,
    },
    #[error("delegation from `{issuer}` to `{subject}` widens roles or delegation authority")]
    DelegationWidensAuthority {
        issuer: KeyIdentity,
        subject: KeyIdentity,
    },
    #[error("lifecycle event `{event}` has an invalid signature: {detail}")]
    InvalidEventSignature { event: String, detail: String },
    #[error(
        "attestation purpose `{attested}` is not authorized for requested purpose `{requested}`"
    )]
    PurposeNotAuthorized {
        attested: AttestationPurpose,
        requested: AttestationPurpose,
    },
    #[error("key `{identity}` has no signing instant for registry policy evaluation")]
    MissingSigningTime { identity: KeyIdentity },
    #[error("key `{identity}` is not valid at {at}: {detail}")]
    KeyNotValid {
        identity: KeyIdentity,
        at: u64,
        detail: String,
    },
    #[error(
        "key `{identity}` is revoked at evaluation instant {at} (effective {revoked_at}): {reason}"
    )]
    KeyRevoked {
        identity: KeyIdentity,
        at: u64,
        revoked_at: u64,
        reason: String,
    },
    #[error("key `{identity}` is superseded by `{successor}` at evaluation instant {at} (effective {effective_at})")]
    KeySuperseded {
        identity: KeyIdentity,
        successor: KeyIdentity,
        at: u64,
        effective_at: u64,
    },
    #[error("key `{identity}` is not authorized for role `{role}` and purpose `{purpose}`")]
    RoleNotAuthorized {
        identity: KeyIdentity,
        role: KeyRole,
        purpose: AttestationPurpose,
    },
    #[error("root key `{identity}` is not allowed by this policy")]
    RootNotAllowed { identity: KeyIdentity },
    #[error("registered producer for `{identity}` is `{registered}`, but the attestation claims `{claimed}`")]
    ProducerMismatch {
        identity: KeyIdentity,
        registered: String,
        claimed: String,
    },
    #[error("expected producer `{expected}` but registry has `{registered}` and attestation claims `{claimed}`")]
    ExpectedProducerMismatch {
        expected: String,
        registered: String,
        claimed: String,
    },
    #[error("cryptographic attestation verification failed: {detail}")]
    Cryptographic { detail: String },
    #[error("canonical lifecycle serialization failed: {0}")]
    Canonical(#[from] CanonicalError),
}

fn default_true() -> bool {
    true
}

fn default_max_delegation_depth() -> usize {
    MAX_DELEGATION_DEPTH
}

fn validate_identity(identity: &KeyIdentity) -> Result<(), TrustError> {
    validate_text("key identity", identity.as_str())
}

fn validate_text(label: &str, value: &str) -> Result<(), TrustError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(TrustError::MalformedEvent {
            detail: format!("{label} must be non-empty and contain no control characters"),
        });
    }
    Ok(())
}

fn ensure_valid_at(
    validity: &KeyValidity,
    at: u64,
    identity: &KeyIdentity,
) -> Result<(), TrustError> {
    if let (Some(before), Some(after)) = (validity.not_before, validity.not_after) {
        if before > after {
            return Err(TrustError::KeyNotValid {
                identity: identity.clone(),
                at,
                detail: format!("validity window is inverted: {before} > {after}"),
            });
        }
    }
    if let Some(before) = validity.not_before {
        if at < before {
            return Err(TrustError::KeyNotValid {
                identity: identity.clone(),
                at,
                detail: format!("key activates at {before}"),
            });
        }
    }
    if let Some(after) = validity.not_after {
        if at > after {
            return Err(TrustError::KeyNotValid {
                identity: identity.clone(),
                at,
                detail: format!("key expired at {after}"),
            });
        }
    }
    Ok(())
}

fn ensure_narrower(
    parent: &KeyValidity,
    child: &KeyValidity,
    issuer: &KeyIdentity,
    subject: &KeyIdentity,
) -> Result<(), TrustError> {
    if let Some(parent_before) = parent.not_before {
        if child.not_before.is_none_or(|value| value < parent_before) {
            return Err(TrustError::DelegationWidensAuthority {
                issuer: issuer.clone(),
                subject: subject.clone(),
            });
        }
    }
    if let Some(parent_after) = parent.not_after {
        if child.not_after.is_none_or(|value| value > parent_after) {
            return Err(TrustError::DelegationWidensAuthority {
                issuer: issuer.clone(),
                subject: subject.clone(),
            });
        }
    }
    Ok(())
}

fn delegation_preimage(event: &KeyDelegation) -> Result<Vec<u8>, TrustError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(&event.schema_version));
    map.insert("event_kind".into(), json!("key_delegation"));
    map.insert("issuer".into(), json!(event.issuer.as_str()));
    map.insert("subject".into(), json!(event.subject.as_str()));
    map.insert("subject_public_key".into(), json!(event.subject_public_key));
    map.insert("subject_validity".into(), json!(&event.subject_validity));
    map.insert("subject_producer".into(), json!(&event.subject_producer));
    map.insert("subject_roles".into(), json!(&event.subject_roles));
    map.insert(
        "subject_delegable_roles".into(),
        json!(&event.subject_delegable_roles),
    );
    map.insert("issued_at".into(), json!(event.issued_at));
    Ok(bioprism_ids::to_canonical_bytes(&Value::Object(map))?)
}

fn revocation_preimage(event: &KeyRevocation) -> Result<Vec<u8>, TrustError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(&event.schema_version));
    map.insert("event_kind".into(), json!("key_revocation"));
    map.insert("target".into(), json!(event.target.as_str()));
    map.insert("authority".into(), json!(event.authority.as_str()));
    map.insert("revoked_at".into(), json!(event.revoked_at));
    map.insert("signed_at".into(), json!(event.signed_at));
    map.insert("reason".into(), json!(&event.reason));
    Ok(bioprism_ids::to_canonical_bytes(&Value::Object(map))?)
}

fn rotation_preimage(event: &KeyRotation) -> Result<Vec<u8>, TrustError> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!(&event.schema_version));
    map.insert("event_kind".into(), json!("key_rotation"));
    map.insert("predecessor".into(), json!(event.predecessor.as_str()));
    map.insert("successor".into(), json!(event.successor.as_str()));
    map.insert(
        "successor_public_key".into(),
        json!(event.successor_public_key),
    );
    map.insert("effective_at".into(), json!(event.effective_at));
    map.insert("signed_at".into(), json!(event.signed_at));
    Ok(bioprism_ids::to_canonical_bytes(&Value::Object(map))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::{AttestationPurpose, ClaimedProducer};
    use crate::signature::KeyValidity;
    use bioprism_ids::ContentHash;

    fn roles(values: &[KeyRole]) -> BTreeSet<KeyRole> {
        values.iter().copied().collect()
    }

    fn root_signer() -> SigningKey {
        SigningKey::new(KeyIdentity::new("root"), [0x11; 32])
    }

    fn child_signer() -> SigningKey {
        SigningKey::new(KeyIdentity::new("publisher-2026"), [0x22; 32])
    }

    fn root_record() -> RegisteredKey {
        RegisteredKey::root(
            KeyIdentity::new("root"),
            root_signer()
                .verification_key(KeyValidity::unbounded())
                .public_key(),
            KeyValidity::unbounded(),
            "AURORA Trust Root",
            roles(&[
                KeyRole::Delegator,
                KeyRole::Revoker,
                KeyRole::Rotator,
                KeyRole::Publisher,
            ]),
            roles(&[KeyRole::Publisher, KeyRole::Rotator]),
        )
        .expect("root record")
    }

    fn registry_with_root() -> KeyRegistry {
        let mut registry = KeyRegistry::new();
        registry
            .register_root(root_record())
            .expect("register root");
        registry
    }

    fn delegated_registry() -> KeyRegistry {
        let mut registry = registry_with_root();
        let child = child_signer()
            .verification_key(KeyValidity::bounded(Some(100), Some(500)).expect("window"));
        let delegation = KeyDelegation::produce(
            &root_signer(),
            &child,
            "AURORA Publisher",
            roles(&[KeyRole::Publisher, KeyRole::Rotator]),
            BTreeSet::new(),
            100,
        )
        .expect("delegation");
        registry
            .import_delegation(delegation)
            .expect("import delegation");
        registry
    }

    fn attestation(signed_at: u64) -> PublicKeyAttestation {
        PublicKeyAttestation::produce_with(
            AttestationPurpose::PublisherManifest,
            ContentHash::of_bytes(b"manifest"),
            &child_signer(),
            ClaimedProducer::new("AURORA Publisher"),
            Some("n".into()),
            Some("2026-08-18T00:00:00Z".into()),
            Some(signed_at),
        )
        .expect("attestation")
    }

    #[test]
    fn a_delegated_key_verifies_only_when_role_producer_and_time_all_bind() {
        let registry = delegated_registry();
        let (authenticated, report) = registry
            .verify_attestation(
                &attestation(200),
                &TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest),
            )
            .expect("trusted");
        assert_eq!(authenticated.key_identity().as_str(), "publisher-2026");
        assert_eq!(report.verdict, TrustVerdict::Trusted);
        assert_eq!(
            report
                .delegation_chain
                .iter()
                .map(KeyIdentity::as_str)
                .collect::<Vec<_>>(),
            vec!["root", "publisher-2026"]
        );
    }

    #[test]
    fn unknown_keys_are_not_accepted_by_a_caller_supplied_registry() {
        let error = registry_with_root()
            .verify_attestation(
                &attestation(200),
                &TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest),
            )
            .expect_err("unknown key");
        assert!(matches!(error, TrustError::UnknownKey { .. }));
    }

    #[test]
    fn delegation_cannot_widen_parent_roles_or_validity() {
        let mut registry = registry_with_root();
        let child = child_signer().verification_key(KeyValidity::unbounded());
        let delegation = KeyDelegation::produce(
            &root_signer(),
            &child,
            "AURORA Publisher",
            roles(&[KeyRole::Publisher, KeyRole::Revoker]),
            BTreeSet::new(),
            10,
        )
        .expect("delegation material");
        let error = registry
            .import_delegation(delegation)
            .expect_err("widening denied");
        assert!(matches!(
            error,
            TrustError::DelegationWidensAuthority { .. }
        ));
    }

    #[test]
    fn revocation_applies_at_effective_time_but_preserves_historical_verification() {
        let mut registry = delegated_registry();
        let revocation = KeyRevocation::produce(
            &root_signer(),
            KeyIdentity::new("publisher-2026"),
            300,
            300,
            "compromise reported",
        )
        .expect("revocation");
        registry
            .import_revocation(revocation)
            .expect("import revocation");
        registry
            .verify_attestation(
                &attestation(200),
                &TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest),
            )
            .expect("historical signature remains valid");
        let mut policy = TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest);
        policy.as_of = Some(300);
        let error = registry
            .verify_attestation(&attestation(200), &policy)
            .expect_err("current revocation");
        assert!(matches!(error, TrustError::KeyRevoked { .. }));
    }

    #[test]
    fn rotation_supersedes_only_later_policy_evaluations() {
        let mut registry = delegated_registry();
        let replacement = SigningKey::new(KeyIdentity::new("publisher-2027"), [0x33; 32]);
        let replacement_key =
            replacement.verification_key(KeyValidity::bounded(Some(400), None).expect("window"));
        let delegation = KeyDelegation::produce(
            &root_signer(),
            &replacement_key,
            "AURORA Publisher",
            roles(&[KeyRole::Publisher]),
            BTreeSet::new(),
            400,
        )
        .expect("replacement delegation");
        registry
            .import_delegation(delegation)
            .expect("import replacement");
        let rotation =
            KeyRotation::produce(&child_signer(), &replacement_key, 400, 350).expect("rotation");
        registry.import_rotation(rotation).expect("import rotation");
        registry
            .verify_attestation(
                &attestation(200),
                &TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest),
            )
            .expect("old signature before rotation");
        let mut policy = TrustPolicy::for_purpose(AttestationPurpose::PublisherManifest);
        policy.as_of = Some(400);
        assert!(matches!(
            registry.verify_attestation(&attestation(200), &policy),
            Err(TrustError::KeySuperseded { successor, .. }) if successor.as_str() == "publisher-2027"
        ));
    }

    #[test]
    fn a_registry_round_trips_without_changing_canonical_lifecycle_material() {
        let registry = delegated_registry();
        let wire = serde_json::to_string(&registry).expect("serialise");
        let restored: KeyRegistry = serde_json::from_str(&wire).expect("deserialise");
        assert_eq!(restored, registry);
        restored.validate().expect("valid restored registry");
    }
}
