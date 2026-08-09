//! Registration, conflict rules, and resolution that does not depend on order.
//!
//! Blueprint 40.16's execution path is "discover manifest, validate signatures/schema, install in
//! controlled environment, run conformance, register capability and status". Steps one and three
//! are absent here — this crate discovers nothing and installs nothing — and step four is somebody
//! else's job, arriving as [`crate::ConformanceEvidence`]. What is left, steps two and five, is
//! this module.
//!
//! # Conflicts are refusals, not tiebreaks
//!
//! Two plugins claiming the same capability at the same priority could be resolved by an
//! alphabetical tiebreak, and that would be worse than refusing. It would make the *outcome*
//! deterministic while leaving the *cause* invisible: whichever author picked the earlier name
//! wins, silently, forever, and nobody involved intended it. A refusal names both plugins and
//! makes somebody choose. `bioprism-fiber`'s rule that "provably cannot matter" and "nobody
//! checked" must not share a representation is the same rule one level up — a deliberate
//! precedence and an accidental one must not share a representation either.
//!
//! # Order independence, and the one place it needs help
//!
//! Once a set of manifests registers cleanly, [`PluginRegistry::resolution`] is a pure function of
//! the set: priorities are unique per capability, so the maximum is unique, and the entries live in
//! a `BTreeMap`. Registration order cannot change it.
//!
//! A *conflicting* set is the awkward case. Registering A then B fails on B; registering B then A
//! fails on A, so the incremental path reports different failures for the same set.
//! [`PluginRegistry::from_manifests`] exists for that: it sorts by identity before registering, so
//! one bad set produces one error whichever order it arrives in. The conflict error also names the
//! two plugins lexicographically rather than as incumbent-and-candidate, so even the message is
//! order-free.
//!
//! A revoked entry keeps its identity but releases its capability slot. It has to keep the
//! identity, or a withdrawn plugin could be re-registered as though nothing had happened; it has to
//! release the slot, because the slot exists to prevent ambiguous *resolution* and a revoked entry
//! resolves to nothing. Withdrawing a bad oracle and registering its replacement at the same
//! priority is the ordinary repair, not a conflict.
//!
//! # Registration is transactional
//!
//! [`PluginRegistry::register`] performs every check before it inserts anything. A rejected
//! manifest leaves the registry byte-identical to what it was, which matters because the rejection
//! reasons are computed against the current contents: a half-inserted plugin would change the
//! answer for the next one.

use crate::abi::{check_ask, HostAsk};
use crate::capability::{Capability, CapabilityKind, Priority};
use crate::discovery::{Availability, CapabilityCard, ProviderCard};
use crate::effect::EffectPolicy;
use crate::error::{RegistrationError, SelectionError};
use crate::evidence::{TrustAssessment, TrustLevel};
use crate::manifest::{PluginId, PluginManifest};
use crate::version::{negotiate, Negotiated, SchemaVersion, VersionSet};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The host's side of every registration decision.
///
/// Everything a host is allowed to be opinionated about lives here rather than being scattered
/// through the checks, so that two deployments differing only in policy can be compared by
/// diffing one value. `bioprism-registry` puts its thresholds in one `TierPolicy` for the same
/// reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryPolicy {
    /// Schema versions this host speaks. Empty means nothing can register.
    pub host_versions: VersionSet,
    /// Effects this host grants. Defaults to granting nothing.
    pub effects: EffectPolicy,
    /// The trust level a plugin must reach to be selected for load-bearing work.
    pub load_bearing_floor: TrustLevel,
    /// Whether two versions of one plugin may be registered at once.
    ///
    /// Off by default: the common case is that a duplicate name is a mistake, and 40.16's failure
    /// list opens with `dependency conflict`. On, it enables the "version coexistence" case 40.16's
    /// verification plan calls for, and the capability-conflict rule then keeps the two versions
    /// from silently competing for the same job.
    pub allow_version_coexistence: bool,
}

impl RegistryPolicy {
    /// The schema versions this workspace actually publishes, read from `bioprism-section`.
    ///
    /// Not a guess: if the section crate bumps `SECTION_SCHEMA_VERSION`, this set moves with it and
    /// plugins pinned to the old one start failing negotiation, which is the intended behaviour
    /// rather than a regression.
    pub fn workspace_versions() -> VersionSet {
        [
            bioprism_section::SECTION_SCHEMA_VERSION,
            bioprism_section::CERTIFICATE_SCHEMA_VERSION,
            bioprism_section::CERTIFICATE_SCHEMA_VERSION_EXTENDED,
        ]
        .into_iter()
        .filter_map(|text| SchemaVersion::parse(text).ok())
        .collect()
    }

    pub fn granting(mut self, effects: EffectPolicy) -> Self {
        self.effects = effects;
        self
    }

    pub fn speaking(mut self, versions: VersionSet) -> Self {
        self.host_versions = versions;
        self
    }

    pub fn with_load_bearing_floor(mut self, floor: TrustLevel) -> Self {
        self.load_bearing_floor = floor;
        self
    }

    pub fn allowing_version_coexistence(mut self) -> Self {
        self.allow_version_coexistence = true;
        self
    }
}

impl Default for RegistryPolicy {
    fn default() -> Self {
        RegistryPolicy {
            host_versions: RegistryPolicy::workspace_versions(),
            effects: EffectPolicy::deny_all(),
            load_bearing_floor: TrustLevel::SelfAttested,
            allow_version_coexistence: false,
        }
    }
}

/// Why a registered plugin was withdrawn (40.16 persists a revocation history).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revocation {
    pub plugin: PluginId,
    pub reason: String,
}

/// A plugin that passed registration, with everything computed at that moment.
///
/// The digests, the negotiated version and the trust assessment are stored rather than recomputed
/// on read, because they are answers about the manifest *as registered*. A manifest is immutable
/// once here, so there is nothing to go stale — and a caller that recomputed them would be able to
/// disagree with the registry about what it registered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registration {
    pub manifest: PluginManifest,
    pub digest: ContentHash,
    pub core_digest: ContentHash,
    pub negotiated: Negotiated,
    pub trust: TrustAssessment,
    pub revoked: Option<Revocation>,
}

impl Registration {
    pub fn id(&self) -> PluginId {
        self.manifest.id()
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked.is_some()
    }

    /// Whether this plugin may be selected for work whose result is load-bearing.
    ///
    /// The floor is the host's, the level is the evidence's, and neither is the plugin's to set.
    pub fn is_selectable_for_load_bearing(&self, floor: TrustLevel) -> bool {
        !self.is_revoked() && self.trust.level >= floor
    }

    /// Whether the declared ABI grade covers `ask` (23.49).
    pub fn can(&self, ask: HostAsk) -> bool {
        self.manifest.abi_grade.can(ask)
    }

    pub fn capability(&self, kind: CapabilityKind) -> Option<&Capability> {
        self.manifest.capability(kind)
    }

    fn priority_for(&self, kind: CapabilityKind) -> Option<Priority> {
        self.capability(kind).map(|capability| capability.priority)
    }
}

/// The registered set, and the resolution rules over it.
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    policy: RegistryPolicy,
    entries: BTreeMap<PluginId, Registration>,
    revocations: Vec<Revocation>,
}

impl PluginRegistry {
    pub fn new(policy: RegistryPolicy) -> Self {
        PluginRegistry {
            policy,
            entries: BTreeMap::new(),
            revocations: Vec::new(),
        }
    }

    /// Registers a whole set at once, sorted by identity first.
    ///
    /// The order-independent entry point. See the module docs on why the incremental path cannot
    /// be order-independent for a set that conflicts with itself.
    pub fn from_manifests<I>(
        policy: RegistryPolicy,
        manifests: I,
    ) -> Result<Self, RegistrationError>
    where
        I: IntoIterator<Item = PluginManifest>,
    {
        let mut sorted: Vec<PluginManifest> = manifests.into_iter().collect();
        sorted.sort_by_key(|manifest| manifest.id());
        let mut registry = PluginRegistry::new(policy);
        for manifest in sorted {
            registry.register(manifest)?;
        }
        Ok(registry)
    }

    pub fn policy(&self) -> &RegistryPolicy {
        &self.policy
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: &PluginId) -> Option<&Registration> {
        self.entries.get(id)
    }

    /// Every registration, ordered by identity.
    pub fn registrations(&self) -> impl Iterator<Item = &Registration> {
        self.entries.values()
    }

    pub fn revocation_history(&self) -> &[Revocation] {
        &self.revocations
    }

    /// Validates, negotiates, checks conflicts, and only then inserts.
    pub fn register(
        &mut self,
        manifest: PluginManifest,
    ) -> Result<&Registration, RegistrationError> {
        manifest.validate()?;
        let id = manifest.id();

        if !self.policy.allow_version_coexistence {
            if let Some(incumbent) = self
                .entries
                .keys()
                .find(|existing| existing.name == id.name && existing.version != id.version)
            {
                return Err(RegistrationError::DuplicateName {
                    name: id.name.clone(),
                    incumbent_version: incumbent.version.clone(),
                });
            }
        }

        if let Some(incumbent) = self.entries.get(&id) {
            return Err(RegistrationError::DuplicateIdentity {
                name: id.name.clone(),
                version: id.version.clone(),
                incumbent: incumbent.digest.to_string(),
            });
        }

        let negotiated = negotiate(&self.policy.host_versions, &manifest.speaks)?;

        if let Some(effect) = self.policy.refusals(&manifest).into_iter().next() {
            return Err(RegistrationError::EffectNotPermitted {
                plugin: id.to_string(),
                effect,
            });
        }

        for capability in &manifest.capabilities {
            if let Some(existing) = self.entries.values().find(|entry| {
                !entry.is_revoked()
                    && entry.priority_for(capability.kind) == Some(capability.priority)
            }) {
                let (first, second) = order_pair(id.to_string(), existing.id().to_string());
                return Err(RegistrationError::CapabilityConflict {
                    kind: capability.kind.to_string(),
                    priority: capability.priority.value(),
                    first,
                    second,
                });
            }
        }

        let digest = manifest.digest()?;
        let core_digest = manifest.core_digest()?;
        let trust = manifest.trust()?;

        let registration = Registration {
            manifest,
            digest,
            core_digest,
            negotiated,
            trust,
            revoked: None,
        };
        Ok(self.entries.entry(id).or_insert(registration))
    }

    /// Marks a plugin revoked. It stays in the registry, unresolvable, with its reason recorded.
    ///
    /// Deleting it would lose the history 40.16 asks to persist, and would let the same identity be
    /// re-registered as if nothing had happened.
    pub fn revoke(
        &mut self,
        id: &PluginId,
        reason: impl Into<String>,
    ) -> Result<(), SelectionError> {
        let reason = reason.into();
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| SelectionError::UnknownPlugin {
                plugin: id.to_string(),
            })?;
        let revocation = Revocation {
            plugin: id.clone(),
            reason,
        };
        entry.revoked = Some(revocation.clone());
        self.revocations.push(revocation);
        Ok(())
    }

    /// The winning plugin for each capability kind: a pure function of the registered set.
    pub fn resolution(&self) -> BTreeMap<CapabilityKind, PluginId> {
        let mut resolved = BTreeMap::new();
        for kind in CapabilityKind::ALL {
            if let Some(registration) = self.best(kind, |_| true) {
                resolved.insert(kind, registration.id());
            }
        }
        resolved
    }

    /// The highest-priority live provider of `kind`, regardless of trust.
    pub fn resolve(&self, kind: CapabilityKind) -> Result<&Registration, SelectionError> {
        self.best(kind, |_| true)
            .ok_or_else(|| SelectionError::NoProviderFor {
                kind: kind.to_string(),
            })
    }

    /// The highest-priority live provider of `kind` that handles `subject`.
    pub fn resolve_subject(
        &self,
        kind: CapabilityKind,
        subject: &str,
    ) -> Result<&Registration, SelectionError> {
        self.best(kind, |capability| capability.handles(subject))
            .ok_or_else(|| {
                if self.best(kind, |_| true).is_some() {
                    SelectionError::NoProviderForSubject {
                        kind: kind.to_string(),
                        subject: subject.to_string(),
                    }
                } else {
                    SelectionError::NoProviderFor {
                        kind: kind.to_string(),
                    }
                }
            })
    }

    /// The highest-priority provider of `kind` that also clears the host's trust floor.
    ///
    /// Priority orders candidates; trust filters them. A plugin cannot buy its way past the floor by
    /// declaring a high priority, and a trusted plugin is not shadowed by an untrusted one that
    /// declared a higher one. When nothing clears the floor the error names the best candidate and
    /// what its evidence was missing, because "no provider" would send the operator looking for a
    /// plugin that is sitting right there.
    pub fn resolve_load_bearing(
        &self,
        kind: CapabilityKind,
    ) -> Result<&Registration, SelectionError> {
        let floor = self.policy.load_bearing_floor;
        if let Some(registration) = self.best_where(
            kind,
            |_| true,
            |entry| entry.is_selectable_for_load_bearing(floor),
        ) {
            return Ok(registration);
        }
        match self.best(kind, |_| true) {
            Some(best) => Err(SelectionError::NotSelectableForLoadBearing {
                plugin: best.id().to_string(),
                trust: best.trust.level.to_string(),
                reasons: best.trust.describe_reasons(),
            }),
            None => Err(SelectionError::NoProviderFor {
                kind: kind.to_string(),
            }),
        }
    }

    /// Checks that a named plugin is live and its declared ABI grade covers `ask` (23.49).
    pub fn ask(&self, id: &PluginId, ask: HostAsk) -> Result<&Registration, SelectionError> {
        let registration = self
            .entries
            .get(id)
            .ok_or_else(|| SelectionError::UnknownPlugin {
                plugin: id.to_string(),
            })?;
        if let Some(revocation) = &registration.revoked {
            return Err(SelectionError::Revoked {
                plugin: id.to_string(),
                reason: revocation.reason.clone(),
            });
        }
        check_ask(&id.to_string(), registration.manifest.abi_grade, ask)?;
        Ok(registration)
    }

    /// One card per capability kind, including the kinds nothing provides.
    ///
    /// 43.35 requires unsupported capabilities to stay discoverable as unavailable, so this never
    /// omits a kind. See [`crate::discovery`] for why an absent card would be worse than an
    /// unavailable one.
    pub fn capability_cards(&self) -> Vec<CapabilityCard> {
        CapabilityKind::ALL
            .into_iter()
            .map(|kind| CapabilityCard {
                kind,
                availability: self.availability_of(kind),
            })
            .collect()
    }

    fn availability_of(&self, kind: CapabilityKind) -> Availability {
        match self.best(kind, |_| true) {
            Some(entry) => {
                let capability = entry
                    .capability(kind)
                    .expect("the winning entry declares the kind it won");
                Availability::Available(Box::new(ProviderCard {
                    plugin: entry.id(),
                    digest: entry.digest.clone(),
                    priority: capability.priority,
                    subjects: capability.subjects.clone(),
                    speaks: entry.negotiated.agreed.clone(),
                    trust: entry.trust.level,
                    load_bearing: entry
                        .is_selectable_for_load_bearing(self.policy.load_bearing_floor),
                    abi_grade: entry.manifest.abi_grade,
                    effects: entry.manifest.effects.clone(),
                    isolation: entry.manifest.sandbox.isolation,
                    isolation_enforcement: entry.manifest.sandbox.enforcement,
                    semantic_loss: entry.manifest.loss.clone(),
                }))
            }
            None => {
                let revoked = self
                    .entries
                    .values()
                    .filter(|entry| entry.is_revoked() && entry.capability(kind).is_some())
                    .count();
                let reason = if revoked > 0 {
                    format!("{revoked} provider(s) registered, all revoked")
                } else {
                    "no plugin registered for this capability".to_string()
                };
                Availability::Unavailable { reason }
            }
        }
    }

    /// The trust assessment recorded for a plugin at registration.
    pub fn trust_of(&self, id: &PluginId) -> Result<&TrustAssessment, SelectionError> {
        self.entries
            .get(id)
            .map(|entry| &entry.trust)
            .ok_or_else(|| SelectionError::UnknownPlugin {
                plugin: id.to_string(),
            })
    }

    fn best<F>(&self, kind: CapabilityKind, matches: F) -> Option<&Registration>
    where
        F: Fn(&Capability) -> bool,
    {
        self.best_where(kind, matches, |_| true)
    }

    fn best_where<F, G>(
        &self,
        kind: CapabilityKind,
        matches: F,
        eligible: G,
    ) -> Option<&Registration>
    where
        F: Fn(&Capability) -> bool,
        G: Fn(&Registration) -> bool,
    {
        self.entries
            .values()
            .filter(|entry| !entry.is_revoked() && eligible(entry))
            .filter_map(|entry| {
                entry
                    .capability(kind)
                    .filter(|capability| matches(capability))
                    .map(|capability| (capability.priority, entry))
            })
            .max_by_key(|(priority, _)| *priority)
            .map(|(_, entry)| entry)
    }
}

impl RegistryPolicy {
    fn refusals(&self, manifest: &PluginManifest) -> Vec<String> {
        self.effects
            .refusals(manifest.effects.iter())
            .into_iter()
            .map(ToString::to_string)
            .collect()
    }
}

/// Sorts two identities so an error message does not depend on which arrived first.
fn order_pair(a: String, b: String) -> (String, String) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::effect::{Effect, EffectClass};
    use crate::evidence::ConformanceEvidence;
    use crate::loss::SemanticLossDeclaration;

    fn host_policy() -> RegistryPolicy {
        RegistryPolicy::default()
    }

    fn speaks() -> VersionSet {
        RegistryPolicy::workspace_versions()
    }

    fn oracle(name: &str, priority: Priority) -> PluginManifest {
        PluginManifest::new(name, "1.0.0", "acme")
            .speaking(speaks())
            .providing(Capability::new(CapabilityKind::Oracle).at(priority))
    }

    #[test]
    fn the_default_host_speaks_the_schema_versions_this_workspace_publishes() {
        let versions = RegistryPolicy::workspace_versions();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(
            &SchemaVersion::parse(bioprism_section::SECTION_SCHEMA_VERSION).expect("parses")
        ));
    }

    #[test]
    fn a_plugin_speaking_an_unknown_wire_version_is_refused_rather_than_tried() {
        let mut registry = PluginRegistry::new(host_policy());
        let manifest = PluginManifest::new("stranger", "1.0.0", "acme")
            .speaking(
                VersionSet::new()
                    .with(SchemaVersion::parse("some-other-protocol/3.0").expect("parses")),
            )
            .providing(Capability::new(CapabilityKind::Oracle));
        let error = registry.register(manifest).expect_err("no common version");
        let rendered = error.to_string();
        assert!(rendered.contains("some-other-protocol/3.0"), "{rendered}");
        assert!(
            rendered.contains("fiber-decision-section/0.1"),
            "{rendered}"
        );
        assert!(registry.is_empty());
    }

    #[test]
    fn two_plugins_claiming_one_capability_at_one_priority_is_an_error_not_a_tiebreak() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("first", Priority(50)))
            .expect("first registers");
        let error = registry
            .register(oracle("second", Priority(50)))
            .expect_err("same kind, same priority");
        match error {
            RegistrationError::CapabilityConflict {
                first,
                second,
                priority,
                ..
            } => {
                assert_eq!(priority, 50);
                assert_eq!(
                    (first.as_str(), second.as_str()),
                    ("first@1.0.0", "second@1.0.0")
                );
            }
            other => panic!("expected a capability conflict, got {other}"),
        }
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn a_conflicting_set_reports_the_same_error_whichever_order_it_arrives_in() {
        let forwards = PluginRegistry::from_manifests(
            host_policy(),
            [oracle("alpha", Priority(50)), oracle("beta", Priority(50))],
        )
        .expect_err("conflicting set");
        let backwards = PluginRegistry::from_manifests(
            host_policy(),
            [oracle("beta", Priority(50)), oracle("alpha", Priority(50))],
        )
        .expect_err("conflicting set");
        assert_eq!(forwards, backwards);
    }

    #[test]
    fn registering_the_same_name_twice_is_refused_by_default() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("dup", Priority(50)))
            .expect("first registers");
        let mut second = oracle("dup", Priority(60));
        second.version = "2.0.0".into();
        assert!(matches!(
            registry.register(second),
            Err(RegistrationError::DuplicateName { .. })
        ));
    }

    #[test]
    fn version_coexistence_is_allowed_only_when_the_policy_says_so() {
        let mut registry = PluginRegistry::new(host_policy().allowing_version_coexistence());
        registry
            .register(oracle("dual", Priority(50)))
            .expect("v1 registers");
        let mut second = oracle("dual", Priority(60));
        second.version = "2.0.0".into();
        registry.register(second).expect("v2 coexists");
        assert_eq!(registry.len(), 2);
        assert_eq!(
            registry
                .resolve(CapabilityKind::Oracle)
                .expect("resolves")
                .id(),
            PluginId::new("dual", "2.0.0")
        );
    }

    #[test]
    fn registering_the_identical_identity_twice_is_refused_rather_than_overwriting() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("same", Priority(50)))
            .expect("first registers");
        assert!(matches!(
            registry.register(oracle("same", Priority(50))),
            Err(RegistrationError::DuplicateIdentity { .. })
        ));
    }

    #[test]
    fn a_rejected_registration_leaves_the_registry_untouched() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("kept", Priority(50)))
            .expect("registers");
        let before = registry.resolution();
        let _ = registry.register(oracle("rejected", Priority(50)));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.resolution(), before);
    }

    #[test]
    fn resolution_does_not_depend_on_the_order_a_clean_set_was_registered_in() {
        let manifests = || {
            vec![
                oracle("low", Priority(10)),
                oracle("high", Priority(90)),
                PluginManifest::new("csv", "1.0.0", "acme")
                    .speaking(speaks())
                    .providing(Capability::new(CapabilityKind::Adapter).at(Priority(20)))
                    .declaring_loss(SemanticLossDeclaration::admitting(["unmapped_column"])),
            ]
        };

        let mut orders = Vec::new();
        for rotation in 0..3 {
            let mut set = manifests();
            set.rotate_left(rotation);
            let mut registry = PluginRegistry::new(host_policy());
            for manifest in set {
                registry.register(manifest).expect("clean set registers");
            }
            orders.push(registry.resolution());
        }

        assert!(orders.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(
            orders[0][&CapabilityKind::Oracle],
            PluginId::new("high", "1.0.0")
        );
    }

    #[test]
    fn a_plugin_needing_an_ungranted_effect_is_refused_at_registration() {
        let mut registry = PluginRegistry::new(host_policy());
        let manifest = oracle("networked", Priority(50)).needing(Effect::Network {
            domain: "api.example".into(),
        });
        match registry.register(manifest) {
            Err(RegistrationError::EffectNotPermitted { effect, .. }) => {
                assert!(effect.contains("api.example"), "{effect}")
            }
            other => panic!("expected an effect refusal, got {other:?}"),
        }
    }

    #[test]
    fn granting_the_effect_class_lets_the_same_plugin_register() {
        let policy =
            host_policy().granting(EffectPolicy::deny_all().granting_class(EffectClass::Network));
        let mut registry = PluginRegistry::new(policy);
        let manifest = oracle("networked", Priority(50)).needing(Effect::Network {
            domain: "api.example".into(),
        });
        assert!(registry.register(manifest).is_ok());
    }

    #[test]
    fn a_plugin_with_no_conformance_evidence_is_not_selectable_for_load_bearing_work() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("bare", Priority(50)))
            .expect("registers despite having no evidence");

        assert!(registry.resolve(CapabilityKind::Oracle).is_ok());
        match registry.resolve_load_bearing(CapabilityKind::Oracle) {
            Err(SelectionError::NotSelectableForLoadBearing { trust, reasons, .. }) => {
                assert_eq!(trust, "unverified");
                assert!(reasons.contains("no conformance evidence"), "{reasons}");
            }
            other => panic!("expected a trust refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_untrusted_plugin_at_a_higher_priority_does_not_shadow_a_trusted_one() {
        let attested = {
            let base = oracle("attested", Priority(10));
            let core = base.core_digest().expect("digestible");
            base.attesting(
                ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "acme")
                    .passing(7)
                    .covering_core(core),
            )
        };
        let mut registry = PluginRegistry::new(host_policy());
        registry.register(attested).expect("registers");
        registry
            .register(oracle("loud", Priority(900)))
            .expect("registers");

        assert_eq!(
            registry
                .resolve(CapabilityKind::Oracle)
                .expect("resolves")
                .id(),
            PluginId::new("loud", "1.0.0")
        );
        assert_eq!(
            registry
                .resolve_load_bearing(CapabilityKind::Oracle)
                .expect("the attested one clears the floor")
                .id(),
            PluginId::new("attested", "1.0.0")
        );
    }

    #[test]
    fn a_revoked_plugin_stops_resolving_but_keeps_its_history() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("doomed", Priority(50)))
            .expect("registers");
        let id = PluginId::new("doomed", "1.0.0");
        registry
            .revoke(&id, "shipped a poisoned fixture")
            .expect("revokes");

        assert!(matches!(
            registry.resolve(CapabilityKind::Oracle),
            Err(SelectionError::NoProviderFor { .. })
        ));
        assert!(matches!(
            registry.ask(&id, HostAsk::ExchangeActs),
            Err(SelectionError::Revoked { .. })
        ));
        assert_eq!(registry.revocation_history().len(), 1);
        assert!(registry.get(&id).expect("still present").is_revoked());
    }

    #[test]
    fn revoking_a_plugin_releases_its_capability_slot_but_not_its_identity() {
        let mut registry = PluginRegistry::new(host_policy().allowing_version_coexistence());
        registry
            .register(oracle("faulty", Priority(50)))
            .expect("registers");
        registry
            .revoke(&PluginId::new("faulty", "1.0.0"), "wrong verdicts")
            .expect("revokes");

        registry
            .register(oracle("replacement", Priority(50)))
            .expect("the slot is free once nothing resolves to it");
        assert!(matches!(
            registry.register(oracle("faulty", Priority(70))),
            Err(RegistrationError::DuplicateIdentity { .. })
        ));
    }

    #[test]
    fn revoking_a_plugin_that_was_never_registered_is_an_error() {
        let mut registry = PluginRegistry::new(host_policy());
        assert!(matches!(
            registry.revoke(&PluginId::new("ghost", "1.0.0"), "n/a"),
            Err(SelectionError::UnknownPlugin { .. })
        ));
    }

    #[test]
    fn resolution_by_subject_distinguishes_a_missing_kind_from_a_missing_format() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(
                PluginManifest::new("csv", "1.0.0", "acme")
                    .speaking(speaks())
                    .providing(
                        Capability::new(CapabilityKind::Adapter)
                            .at(Priority(20))
                            .handling(["text/csv"]),
                    )
                    .declaring_loss(SemanticLossDeclaration::admitting(["unmapped_column"])),
            )
            .expect("registers");

        assert!(registry
            .resolve_subject(CapabilityKind::Adapter, "text/csv")
            .is_ok());
        assert!(matches!(
            registry.resolve_subject(CapabilityKind::Adapter, "application/dicom"),
            Err(SelectionError::NoProviderForSubject { .. })
        ));
        assert!(matches!(
            registry.resolve_subject(CapabilityKind::Oracle, "anything"),
            Err(SelectionError::NoProviderFor { .. })
        ));
    }

    #[test]
    fn a_p0_plugin_registered_successfully_is_still_refused_a_continuation() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("flat", Priority(50)))
            .expect("registers at the default P0 grade");
        let id = PluginId::new("flat", "1.0.0");
        assert!(registry.ask(&id, HostAsk::ExchangeActs).is_ok());
        assert!(matches!(
            registry.ask(&id, HostAsk::ResumeContinuation),
            Err(SelectionError::AbiGradeTooLow { .. })
        ));
    }

    #[test]
    fn a_capability_nothing_provides_is_still_published_as_a_card() {
        let registry = PluginRegistry::new(host_policy());
        let cards = registry.capability_cards();
        assert_eq!(cards.len(), CapabilityKind::ALL.len());
        assert!(cards.iter().all(|card| !card.is_available()));
        let mutation = cards
            .iter()
            .find(|card| card.kind == CapabilityKind::MutationOperator)
            .expect("every kind gets a card");
        match &mutation.availability {
            Availability::Unavailable { reason } => {
                assert!(reason.contains("no plugin registered"), "{reason}")
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_card_distinguishes_nothing_registered_from_everything_revoked() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("only", Priority(50)))
            .expect("registers");
        registry
            .revoke(&PluginId::new("only", "1.0.0"), "withdrawn")
            .expect("revokes");

        let cards = registry.capability_cards();
        let oracle_card = cards
            .iter()
            .find(|card| card.kind == CapabilityKind::Oracle)
            .expect("card exists");
        match &oracle_card.availability {
            Availability::Unavailable { reason } => {
                assert!(reason.contains("all revoked"), "{reason}")
            }
            other => panic!("expected unavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_provider_card_publishes_the_host_verdict_not_just_the_trust_level() {
        let mut registry = PluginRegistry::new(host_policy());
        registry
            .register(oracle("bare", Priority(50)))
            .expect("registers");
        let cards = registry.capability_cards();
        let provider = cards
            .iter()
            .find(|card| card.kind == CapabilityKind::Oracle)
            .and_then(CapabilityCard::provider)
            .expect("an available oracle card");
        assert_eq!(provider.trust, TrustLevel::Unverified);
        assert!(!provider.load_bearing);
        assert_eq!(
            provider.isolation_enforcement,
            crate::sandbox::Enforcement::DeclaredOnly
        );
    }

    #[test]
    fn the_negotiated_version_is_recorded_on_the_registration_not_recomputed_by_callers() {
        let mut registry = PluginRegistry::new(host_policy());
        let manifest = PluginManifest::new("narrow", "1.0.0", "acme")
            .speaking(VersionSet::new().with(
                SchemaVersion::parse(bioprism_section::SECTION_SCHEMA_VERSION).expect("parses"),
            ))
            .providing(Capability::new(CapabilityKind::Oracle));
        let registration = registry.register(manifest).expect("registers");
        assert_eq!(
            registration.negotiated.agreed.to_string(),
            bioprism_section::SECTION_SCHEMA_VERSION
        );
        assert!(
            registration.negotiated.is_lossy(),
            "the host speaks two profiles this plugin does not"
        );
    }
}
