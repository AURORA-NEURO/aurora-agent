//! Which registry may answer for which name, and on whose behalf.
//!
//! Blueprint 10.18 (Private Registries and Federation) states the rule this module encodes —
//! *"Federation proves origin, not universal trust"* and *"No global root has unilateral control"*
//! — and 10.19 repeats it as *"There is no global write authority"*. Neither says how a consumer
//! decides whether the registry that just answered was entitled to. That decision is here.
//!
//! # Authority is per namespace, and it is exclusive
//!
//! A registry declares two disjoint sets: the namespaces it **owns** and the namespaces it
//! **carries** on behalf of a named origin. Owning is the claim that this registry decides what
//! `bioprism/onco-tp53` means. Carrying is the claim that it holds a copy of somebody else's
//! decision. A registry that answers for a namespace in neither set has produced
//! [`AuthorityError::OutsideAuthority`] — an error, not a caveat on an otherwise usable answer,
//! because an answer from a registry with no standing for the name is not a weaker answer, it is a
//! different question being answered.
//!
//! Within one federation, ownership is exclusive: two registries both claiming `bioprism` is
//! [`AuthorityError::Contested`], and a carrier whose named origin does not actually own the
//! namespace is [`AuthorityError::DelegationUnbacked`]. The second is the one that matters
//! offline, because a mirror is exactly the component a consumer cannot cross-check by asking
//! somebody else.
//!
//! # Not implemented
//!
//! No keys, no signatures, no delegation certificates. 10.18 wants "stable publisher identifiers,
//! signed delegations, key rotation"; a [`RegistryId`] here is a name, and a carrying declaration
//! is a statement in a document rather than a proof. What this module supplies is the *predicate*
//! a verified delegation would be checked against, and the shape of the answer once it is.
//! Nothing here reaches a network or discovers a registry; a federation is a value somebody built.

use crate::name::{NameError, Namespace, PackName};
use bioprism_ids::IdError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The name of a registry within a federation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RegistryId(String);

impl RegistryId {
    pub const KIND: &'static str = "registry id";

    pub fn parse(value: impl Into<String>) -> Result<Self, NameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(NameError::Id(IdError::Empty {
                kind: RegistryId::KIND,
            }));
        }
        if let Some(bad) = value.chars().find(|c| c.is_control()) {
            return Err(NameError::IllegalCharacter {
                kind: RegistryId::KIND,
                value,
                character: bad,
            });
        }
        Ok(RegistryId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RegistryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<RegistryId> for String {
    fn from(value: RegistryId) -> Self {
        value.0
    }
}

impl TryFrom<String> for RegistryId {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RegistryId::parse(value)
    }
}

/// What a registry declares it may answer for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authority {
    registry: RegistryId,
    owned: BTreeSet<Namespace>,
    carried: BTreeMap<Namespace, RegistryId>,
}

impl Authority {
    /// A registry that owns nothing and carries nothing. It can answer for no name at all, which
    /// is the correct starting point: authority is added deliberately.
    pub fn new(registry: RegistryId) -> Self {
        Authority {
            registry,
            owned: BTreeSet::new(),
            carried: BTreeMap::new(),
        }
    }

    pub fn registry(&self) -> &RegistryId {
        &self.registry
    }

    /// Declares this registry the origin for a namespace.
    pub fn owning(mut self, namespace: Namespace) -> Result<Self, AuthorityError> {
        if let Some(origin) = self.carried.get(&namespace) {
            return Err(AuthorityError::OwnsAndCarries {
                registry: self.registry.clone(),
                namespace: namespace.to_string(),
                origin: origin.clone(),
            });
        }
        self.owned.insert(namespace);
        Ok(self)
    }

    /// Declares this registry a carrier of somebody else's namespace.
    ///
    /// The declaration is not self-validating: [`Federation::standing_for`] is what decides whether
    /// `origin` actually owns the namespace. A carrier that names an origin which does not is the
    /// exact failure a consumer offline cannot detect by asking around, so it gets its own error.
    pub fn carrying(
        mut self,
        namespace: Namespace,
        origin: RegistryId,
    ) -> Result<Self, AuthorityError> {
        if self.owned.contains(&namespace) {
            return Err(AuthorityError::OwnsAndCarries {
                registry: self.registry.clone(),
                namespace: namespace.to_string(),
                origin,
            });
        }
        if origin == self.registry {
            return Err(AuthorityError::CarriesItself {
                registry: self.registry.clone(),
                namespace: namespace.to_string(),
            });
        }
        self.carried.insert(namespace, origin);
        Ok(self)
    }

    pub fn owns(&self, namespace: &Namespace) -> bool {
        self.owned.contains(namespace)
    }

    pub fn carries(&self, namespace: &Namespace) -> Option<&RegistryId> {
        self.carried.get(namespace)
    }

    pub fn owned_namespaces(&self) -> impl Iterator<Item = &Namespace> {
        self.owned.iter()
    }

    pub fn carried_namespaces(&self) -> impl Iterator<Item = (&Namespace, &RegistryId)> {
        self.carried.iter()
    }

    /// The standing this registry has for one name, or the reason it has none.
    ///
    /// This is a check against the registry's own declaration only. It cannot tell a carrier from
    /// a liar; [`Federation::standing_for`] can, because it can see the named origin.
    pub fn standing_for(&self, name: &PackName) -> Result<NameAuthority, AuthorityError> {
        let namespace = name.namespace();
        if self.owned.contains(namespace) {
            return Ok(NameAuthority::Authoritative {
                registry: self.registry.clone(),
            });
        }
        if let Some(origin) = self.carried.get(namespace) {
            return Ok(NameAuthority::Carried {
                mirror: self.registry.clone(),
                origin: origin.clone(),
            });
        }
        Err(AuthorityError::OutsideAuthority {
            registry: self.registry.clone(),
            name: name.to_string(),
            namespace: namespace.to_string(),
        })
    }
}

/// The standing a registry has for a particular name.
///
/// Every answer this crate produces carries one of these. There is no third variant for "answered
/// anyway": a registry with no standing does not produce an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "authority", rename_all = "snake_case")]
pub enum NameAuthority {
    /// This registry decides what the name means.
    Authoritative { registry: RegistryId },
    /// This registry holds a copy of what `origin` decided. The answer may still be correct and
    /// current; it is simply not the registry's to make.
    Carried {
        mirror: RegistryId,
        origin: RegistryId,
    },
}

impl NameAuthority {
    /// The registry that produced the answer, whether or not it was the authority for it.
    pub fn answered_by(&self) -> &RegistryId {
        match self {
            NameAuthority::Authoritative { registry } => registry,
            NameAuthority::Carried { mirror, .. } => mirror,
        }
    }

    /// The registry whose decision the answer reports.
    pub fn authority(&self) -> &RegistryId {
        match self {
            NameAuthority::Authoritative { registry } => registry,
            NameAuthority::Carried { origin, .. } => origin,
        }
    }

    /// True only when the registry that answered is the one that decides.
    pub fn is_authoritative(&self) -> bool {
        matches!(self, NameAuthority::Authoritative { .. })
    }
}

impl fmt::Display for NameAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameAuthority::Authoritative { registry } => {
                write!(f, "{registry} (authoritative)")
            }
            NameAuthority::Carried { mirror, origin } => {
                write!(f, "{mirror} (carrying {origin})")
            }
        }
    }
}

/// The set of registries a consumer is willing to consider, and their declarations.
///
/// A federation is a local value. Nothing joins it by announcing itself; somebody built this map,
/// which is the offline analogue of an allowlist and is the only trust-establishing act in the
/// whole crate that is not checked against evidence — because it cannot be. Everything downstream
/// of it is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Federation {
    members: BTreeMap<RegistryId, Authority>,
}

impl Federation {
    pub fn new() -> Self {
        Federation::default()
    }

    /// Admits a registry, refusing a declaration that contradicts one already admitted.
    pub fn admit(&mut self, authority: Authority) -> Result<(), AuthorityError> {
        let id = authority.registry.clone();
        if self.members.contains_key(&id) {
            return Err(AuthorityError::DuplicateMember { registry: id });
        }
        for namespace in authority.owned_namespaces() {
            if let Some(other) = self.owner_of(namespace) {
                return Err(AuthorityError::Contested {
                    namespace: namespace.to_string(),
                    claimants: [other.clone(), id],
                });
            }
        }
        self.members.insert(id, authority);
        Ok(())
    }

    pub fn member(&self, registry: &RegistryId) -> Option<&Authority> {
        self.members.get(registry)
    }

    pub fn members(&self) -> impl Iterator<Item = &Authority> {
        self.members.values()
    }

    pub fn owner_of(&self, namespace: &Namespace) -> Option<&RegistryId> {
        self.members
            .values()
            .find(|authority| authority.owns(namespace))
            .map(Authority::registry)
    }

    /// The standing `registry` has for `name`, checked against the federation rather than against
    /// the registry's own word.
    pub fn standing_for(
        &self,
        registry: &RegistryId,
        name: &PackName,
    ) -> Result<NameAuthority, AuthorityError> {
        let authority = self
            .members
            .get(registry)
            .ok_or_else(|| AuthorityError::NotAMember {
                registry: registry.clone(),
            })?;
        let standing = authority.standing_for(name)?;
        if let NameAuthority::Carried { mirror, origin } = &standing {
            let backed = self
                .members
                .get(origin)
                .is_some_and(|declared| declared.owns(name.namespace()));
            if !backed {
                return Err(AuthorityError::DelegationUnbacked {
                    mirror: mirror.clone(),
                    origin: origin.clone(),
                    namespace: name.namespace().to_string(),
                });
            }
        }
        Ok(standing)
    }

    /// Every declaration in the federation that is internally inconsistent.
    ///
    /// Admission checks each registry as it arrives, but a carrier may name an origin that is
    /// admitted later or never. This walks the finished federation and reports what is still
    /// dangling, so a deployment can run it once at startup instead of discovering it during a
    /// resolution.
    pub fn audit(&self) -> Vec<AuthorityError> {
        let mut findings = Vec::new();
        for authority in self.members.values() {
            for (namespace, origin) in authority.carried_namespaces() {
                match self.members.get(origin) {
                    Some(declared) if declared.owns(namespace) => {}
                    _ => findings.push(AuthorityError::DelegationUnbacked {
                        mirror: authority.registry.clone(),
                        origin: origin.clone(),
                        namespace: namespace.to_string(),
                    }),
                }
            }
        }
        findings
    }
}

/// Why a registry may not answer for a name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthorityError {
    #[error(
        "{registry} has no standing for {name}: it neither owns nor carries namespace {namespace}"
    )]
    OutsideAuthority {
        registry: RegistryId,
        name: String,
        namespace: String,
    },

    #[error(
        "{registry} declares namespace {namespace} both owned and carried on behalf of {origin}"
    )]
    OwnsAndCarries {
        registry: RegistryId,
        namespace: String,
        origin: RegistryId,
    },

    #[error("{registry} declares it carries namespace {namespace} on behalf of itself")]
    CarriesItself {
        registry: RegistryId,
        namespace: String,
    },

    #[error("namespace {namespace} is claimed by both {} and {}", claimants[0], claimants[1])]
    Contested {
        namespace: String,
        claimants: [RegistryId; 2],
    },

    #[error("{mirror} carries namespace {namespace} on behalf of {origin}, which does not own it")]
    DelegationUnbacked {
        mirror: RegistryId,
        origin: RegistryId,
        namespace: String,
    },

    #[error("{registry} is not a member of this federation")]
    NotAMember { registry: RegistryId },

    #[error("{registry} is already a member of this federation")]
    DuplicateMember { registry: RegistryId },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns(text: &str) -> Namespace {
        Namespace::parse(text).expect("parses")
    }

    fn id(text: &str) -> RegistryId {
        RegistryId::parse(text).expect("parses")
    }

    fn name(text: &str) -> PackName {
        PackName::parse(text).expect("parses")
    }

    fn origin() -> Authority {
        Authority::new(id("origin"))
            .owning(ns("bioprism"))
            .expect("owns")
    }

    fn mirror() -> Authority {
        Authority::new(id("site-mirror"))
            .carrying(ns("bioprism"), id("origin"))
            .expect("carries")
    }

    fn federation() -> Federation {
        let mut federation = Federation::new();
        federation.admit(origin()).expect("admitted");
        federation.admit(mirror()).expect("admitted");
        federation
    }

    #[test]
    fn a_registry_with_no_declaration_answers_for_nothing() {
        let bare = Authority::new(id("empty"));
        let error = bare
            .standing_for(&name("bioprism/onco-tp53"))
            .expect_err("authority is added deliberately, never assumed");
        assert!(matches!(error, AuthorityError::OutsideAuthority { .. }));
    }

    #[test]
    fn a_mirror_answering_outside_its_authority_is_an_error_and_not_a_warning() {
        let federation = federation();
        let error = federation
            .standing_for(&id("site-mirror"), &name("elsewhere/other-pack"))
            .expect_err("the mirror carries bioprism and nothing else");
        assert!(matches!(
            error,
            AuthorityError::OutsideAuthority { ref namespace, .. } if namespace == "elsewhere"
        ));
    }

    #[test]
    fn an_answer_names_the_registry_that_answered_and_the_one_that_decided() {
        let federation = federation();
        let carried = federation
            .standing_for(&id("site-mirror"), &name("bioprism/onco-tp53"))
            .expect("the mirror carries this namespace");
        assert_eq!(carried.answered_by(), &id("site-mirror"));
        assert_eq!(carried.authority(), &id("origin"));
        assert!(!carried.is_authoritative());

        let direct = federation
            .standing_for(&id("origin"), &name("bioprism/onco-tp53"))
            .expect("the origin owns this namespace");
        assert_eq!(direct.answered_by(), direct.authority());
        assert!(direct.is_authoritative());
    }

    #[test]
    fn a_carrier_naming_an_origin_that_does_not_own_the_namespace_is_refused() {
        let mut federation = Federation::new();
        federation
            .admit(
                Authority::new(id("impostor"))
                    .owning(ns("other"))
                    .expect("owns"),
            )
            .expect("admitted");
        federation
            .admit(
                Authority::new(id("mirror"))
                    .carrying(ns("bioprism"), id("impostor"))
                    .expect("declares"),
            )
            .expect("admitted");

        let error = federation
            .standing_for(&id("mirror"), &name("bioprism/onco-tp53"))
            .expect_err("a delegation is only as good as the origin behind it");
        assert!(matches!(error, AuthorityError::DelegationUnbacked { .. }));
        assert_eq!(federation.audit().len(), 1);
    }

    #[test]
    fn two_registries_cannot_both_own_a_namespace() {
        let mut federation = Federation::new();
        federation.admit(origin()).expect("admitted");
        let error = federation
            .admit(
                Authority::new(id("second"))
                    .owning(ns("bioprism"))
                    .expect("declares"),
            )
            .expect_err("ownership is exclusive inside one federation");
        assert!(matches!(error, AuthorityError::Contested { .. }));
    }

    #[test]
    fn a_registry_cannot_both_own_and_carry_the_same_namespace() {
        let error = Authority::new(id("confused"))
            .owning(ns("bioprism"))
            .expect("owns")
            .carrying(ns("bioprism"), id("origin"))
            .expect_err("deciding and copying a decision are different claims");
        assert!(matches!(error, AuthorityError::OwnsAndCarries { .. }));
    }

    #[test]
    fn a_registry_cannot_declare_itself_the_origin_it_carries_for() {
        let error = Authority::new(id("loop"))
            .carrying(ns("bioprism"), id("loop"))
            .expect_err("a self-referential delegation proves nothing");
        assert!(matches!(error, AuthorityError::CarriesItself { .. }));
    }

    #[test]
    fn a_registry_outside_the_federation_answers_for_nothing_within_it() {
        let federation = federation();
        let error = federation
            .standing_for(&id("stranger"), &name("bioprism/onco-tp53"))
            .expect_err("membership is the one thing that is granted, not computed");
        assert!(matches!(error, AuthorityError::NotAMember { .. }));
    }

    #[test]
    fn a_clean_federation_audits_clean() {
        assert!(federation().audit().is_empty());
    }
}
