//! Layered configuration, secret references, and the deployment profile that may not change
//! meaning.
//!
//! Implements blueprint 40.10 (Configuration, Secrets and Feature Flags) apart from its feature-flag
//! half, which is [`crate::flags`], and the first of 40.38's four invariants — *deployment changes
//! providers, not object semantics* — which is the only one of the four that is a predicate over a
//! value rather than a property of infrastructure this workspace does not have.
//!
//! # A configuration is an input to a reproducible computation
//!
//! That single sentence generates everything below. If the compiled context, the certificate and
//! the result bundle are content-addressed, then whatever was on the command line is part of the
//! computation, and a run whose configuration cannot be recovered is a run nobody can replay. So
//! the effective configuration is hashable, twice and for two different questions:
//!
//! * [`EffectiveConfig::fingerprint`] covers **every** resolved setting with secrets standing as
//!   their references. This is the redacted fingerprint 40.10 lists as an output, and it answers
//!   "was this the same configuration".
//! * [`EffectiveConfig::emitted_fingerprint`] covers only the settings declared
//!   [`Influence::Emitted`]. It answers the sharper question: "could this configuration have
//!   changed the bytes".
//!
//! The two-axis split is the whole design. [`Influence`] records, per setting, whether the value
//! participates in what a computation emits or only in how it gets there, and every rule in this
//! module and in [`crate::flags`] is stated against that axis.
//!
//! # The rule that makes secrets structural rather than careful
//!
//! 40.10's first invariant is *secrets are referenced, not serialized*, and its second is *result
//! bundles include nonsecret effective settings*. Put them together and a third follows that the
//! blueprint does not state: **a secret may not participate in an emitted artifact.** If it did,
//! the artifact's digest would depend on a value the bundle is forbidden to record, and no reader
//! could ever reproduce it. [`SettingSpec::secret`] therefore fixes [`Influence::Operational`] and
//! there is no constructor that does otherwise, so the situation is not an error to be caught but a
//! shape that cannot be built. [`crate::OpsError::SecretInDigest`] exists anyway, because a
//! `SettingSpec` arriving through `serde` bypasses the constructors, and [`Schema::declare`] is
//! where that document meets the rule.
//!
//! There is no secret *value* type anywhere in this crate. [`Binding::Secret`] carries a
//! [`SecretRef`], and a `SecretRef` is a locator. [`SecretLease`] — the "scoped secret lease" 40.10
//! names as an output — carries no value either and has no `resolve` method, because resolving a
//! reference needs an environment, a vault or a file, and this crate touches none of the three. A
//! lease is a *permission to resolve at the execution boundary*, addressed to whoever owns the
//! boundary. It does not implement `Clone` or `Serialize`, for the reason
//! `bioprism_fiber::Budget` does not: a lease that can be copied is a lease that outlives its
//! scope.
//!
//! # Precedence, and the failure of not having one
//!
//! [`Layer`] is ordered and the order is fixed by the type, not by argument order. What 40.10's
//! `ambiguous precedence` failure actually describes is the case the order does not settle: two
//! sources at the *same* layer binding one key to different values. [`ConfigStack::resolve`] raises
//! it rather than picking, because the alternative is a run that depends on which environment
//! variable the reader happened to enumerate first.
//!
//! # What is deliberately not implemented
//!
//! * **No file loading, no environment reading, no vault client, no network, no process
//!   spawning.** A [`Source`] is a map somebody built. 40.10's "TOML/YAML project files" and
//!   "environment or vault references" are the caller's to produce; this crate resolves them and
//!   refuses to pretend it fetched anything.
//! * **No secret resolution, no encryption, no rotation, no revocation.** See above.
//!   `crates/safety` records that this workspace holds no key material at all.
//! * **No `config` CLI.** 40.10 names one under Interfaces; `bioprism-cli` owns command surfaces.
//! * **No schema validation beyond type and presence.** Ranges, patterns and enumerations are an
//!   ingest concern; what changes meaning between two runs is which key was bound and to what.
//! * **No provider bindings, health checks, readiness probes or deployment attestations.** Those
//!   are 40.38's other three invariants and they need infrastructure. [`DeploymentProfile`] here is
//!   a configuration layer with one rule attached, not a deployment.

use crate::error::{well_formed_name, OpsError};
use bioprism_ids::ContentHash;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

/// Where a binding came from, in ascending precedence.
///
/// The order is a property of the type. A caller cannot reorder layers, and adding a layer means
/// deciding where it sits relative to every other one at the point of definition rather than at the
/// point of use.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// Compiled-in defaults. The only layer at which a permission may not grant.
    Defaults,
    /// A checked-in project file.
    ProjectFile,
    /// A [`DeploymentProfile`]. Constrained to operational settings; see the module docs.
    DeploymentProfile,
    /// Process environment.
    Environment,
    /// Explicit invocation arguments.
    CommandLine,
}

impl Layer {
    pub const ASCENDING: [Layer; 5] = [
        Layer::Defaults,
        Layer::ProjectFile,
        Layer::DeploymentProfile,
        Layer::Environment,
        Layer::CommandLine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Defaults => "defaults",
            Layer::ProjectFile => "project_file",
            Layer::DeploymentProfile => "deployment_profile",
            Layer::Environment => "environment",
            Layer::CommandLine => "command_line",
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a setting's value participates in what a computation emits.
///
/// This is the axis the crate is built on, and it is deliberately the same distinction
/// `bioprism-governance` draws over schema fields with its `DigestRole`. That crate is not a
/// dependency here — it owns schema evolution and a second classifier is exactly what this
/// workspace must not grow — so the rule is restated rather than imported, and the restatement is
/// visible here so a reader can check the two against each other.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Influence {
    /// Changing this value changes the bytes of some emitted artifact, so it changes that
    /// artifact's digest.
    Emitted,
    /// Changing this value changes where, how fast or how loudly, and not what.
    Operational,
}

impl Influence {
    pub fn as_str(self) -> &'static str {
        match self {
            Influence::Emitted => "emitted",
            Influence::Operational => "operational",
        }
    }

    pub fn is_emitted(self) -> bool {
        self == Influence::Emitted
    }
}

impl fmt::Display for Influence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A dotted setting name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SettingKey(String);

impl SettingKey {
    pub fn parse(value: impl Into<String>) -> Result<Self, OpsError> {
        Ok(SettingKey(well_formed_name("setting key", &value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SettingKey {
    type Error = OpsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SettingKey::parse(value)
    }
}

impl From<SettingKey> for String {
    fn from(value: SettingKey) -> Self {
        value.0
    }
}

/// The four value shapes a setting may take.
///
/// No float. Configuration participates in digests, and a float that round-trips differently
/// between two serializers would move a fingerprint without anybody changing a setting — the exact
/// failure `bioprism-ids` had to fix for the certificate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingValue {
    Bool(bool),
    Integer(i64),
    Text(String),
    List(Vec<String>),
}

impl SettingValue {
    pub fn value_type(&self) -> ValueType {
        match self {
            SettingValue::Bool(_) => ValueType::Bool,
            SettingValue::Integer(_) => ValueType::Integer,
            SettingValue::Text(_) => ValueType::Text,
            SettingValue::List(_) => ValueType::List,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            SettingValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            SettingValue::Bool(value) => Value::Bool(*value),
            SettingValue::Integer(value) => Value::from(*value),
            SettingValue::Text(value) => Value::String(value.clone()),
            SettingValue::List(values) => {
                Value::Array(values.iter().cloned().map(Value::String).collect())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Bool,
    Integer,
    Text,
    List,
}

impl ValueType {
    pub fn as_str(self) -> &'static str {
        match self {
            ValueType::Bool => "bool",
            ValueType::Integer => "integer",
            ValueType::Text => "text",
            ValueType::List => "list",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a secret would be fetched from, if anything here fetched anything.
///
/// The distinction is load-bearing for 40.39's *no ambient credentials*:
/// [`SecretSource::Environment`] is readable by every line of code in the process, so a reference
/// to it is ambient unless a [`SecretLease`] scopes it. See [`crate::hardening`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretSource {
    /// Process environment. Ambient by construction.
    Environment,
    /// An external broker addressed by path.
    Vault,
    /// A file on the host.
    File,
}

impl SecretSource {
    pub fn as_str(self) -> &'static str {
        match self {
            SecretSource::Environment => "env",
            SecretSource::Vault => "vault",
            SecretSource::File => "file",
        }
    }

    /// Whether every line of code in the process can read it without asking anybody.
    pub fn is_process_wide(self) -> bool {
        self == SecretSource::Environment
    }
}

/// A locator for a secret. Never the secret.
///
/// A reference is safe to write into a result bundle, a log line and a fingerprint, which is
/// exactly why 40.10's first invariant is phrased as *referenced, not serialized*: the reference is
/// the serializable half.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SecretRef {
    source: SecretSource,
    locator: String,
}

impl SecretRef {
    pub fn new(source: SecretSource, locator: impl Into<String>) -> Result<Self, OpsError> {
        Ok(SecretRef {
            source,
            locator: well_formed_name("secret locator", &locator.into())?,
        })
    }

    pub fn source(&self) -> SecretSource {
        self.source
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.source.as_str(), self.locator)
    }
}

/// What a layer bound a key to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "snake_case")]
pub enum Binding {
    Value(SettingValue),
    Secret(SecretRef),
}

impl Binding {
    fn to_json(&self) -> Value {
        match self {
            Binding::Value(value) => value.to_json(),
            Binding::Secret(reference) => Value::String(reference.to_string()),
        }
    }
}

/// What kind of thing a setting is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingKind {
    /// An ordinary typed value.
    Value { value_type: ValueType },
    /// A boolean that grants something. Defaults to denied and may not be granted by the defaults
    /// layer.
    Permission,
    /// A credential, held as a reference.
    Secret,
}

/// One declared setting.
///
/// Fields are private and the three constructors are the whole vocabulary, because
/// [`SettingSpec::secret`] fixing [`Influence::Operational`] is how the secret-in-digest rule is
/// made unrepresentable rather than merely checked. `serde` can still produce a `SettingSpec` that
/// violates it — deserialization does not run constructors — which is why [`Schema::declare`]
/// checks anyway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingSpec {
    key: SettingKey,
    kind: SettingKind,
    influence: Influence,
}

impl SettingSpec {
    /// An ordinary value setting.
    pub fn value(key: SettingKey, value_type: ValueType, influence: Influence) -> Self {
        SettingSpec {
            key,
            kind: SettingKind::Value { value_type },
            influence,
        }
    }

    /// A permission. Absence denies; see [`ConfigStack::resolve`].
    pub fn permission(key: SettingKey, influence: Influence) -> Self {
        SettingSpec {
            key,
            kind: SettingKind::Permission,
            influence,
        }
    }

    /// A credential reference. Always [`Influence::Operational`], and no argument offers the
    /// alternative.
    pub fn secret(key: SettingKey) -> Self {
        SettingSpec {
            key,
            kind: SettingKind::Secret,
            influence: Influence::Operational,
        }
    }

    pub fn key(&self) -> &SettingKey {
        &self.key
    }

    pub fn kind(&self) -> SettingKind {
        self.kind
    }

    pub fn influence(&self) -> Influence {
        self.influence
    }

    pub fn is_secret(&self) -> bool {
        matches!(self.kind, SettingKind::Secret)
    }

    fn accepts(&self, binding: &Binding) -> Result<(), (String, String)> {
        match (self.kind, binding) {
            (SettingKind::Value { value_type }, Binding::Value(value)) => {
                if value.value_type() == value_type {
                    Ok(())
                } else {
                    Err((value_type.to_string(), value.value_type().to_string()))
                }
            }
            (SettingKind::Permission, Binding::Value(SettingValue::Bool(_))) => Ok(()),
            (SettingKind::Permission, Binding::Value(other)) => {
                Err(("bool".to_string(), other.value_type().to_string()))
            }
            (SettingKind::Secret, Binding::Secret(_)) => Ok(()),
            (SettingKind::Secret, Binding::Value(value)) => {
                Err(("secret".to_string(), value.value_type().to_string()))
            }
            (_, Binding::Secret(_)) => Err(("value".to_string(), "secret".to_string())),
        }
    }
}

/// The declared settings of a program.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    specs: BTreeMap<SettingKey, SettingSpec>,
}

impl Schema {
    pub fn new() -> Self {
        Schema::default()
    }

    /// Adds a spec, refusing one that would let a secret reach an emitted artifact.
    pub fn declare(&mut self, spec: SettingSpec) -> Result<(), OpsError> {
        if spec.is_secret() && spec.influence.is_emitted() {
            return Err(OpsError::SecretInDigest {
                key: spec.key.to_string(),
            });
        }
        self.specs.insert(spec.key.clone(), spec);
        Ok(())
    }

    pub fn with(mut self, spec: SettingSpec) -> Result<Self, OpsError> {
        self.declare(spec)?;
        Ok(self)
    }

    pub fn get(&self, key: &SettingKey) -> Option<&SettingSpec> {
        self.specs.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &SettingKey> {
        self.specs.keys()
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

/// One place bindings came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    layer: Layer,
    origin: String,
    bindings: BTreeMap<SettingKey, Binding>,
}

impl Source {
    pub fn new(layer: Layer, origin: impl Into<String>) -> Self {
        Source {
            layer,
            origin: origin.into(),
            bindings: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, key: SettingKey, binding: Binding) -> Self {
        self.bindings.insert(key, binding);
        self
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// A deployment profile, expressed as the one layer that may not change meaning.
///
/// 40.38's first invariant is *deployment changes providers, not object semantics*. Here that is a
/// checkable predicate: a profile may bind [`Influence::Operational`] settings and nothing else, so
/// switching profiles provably cannot move [`EffectiveConfig::emitted_fingerprint`]. The remaining
/// three invariants of 40.38 — local mode needing no external service, protected mode denying
/// egress, hosted metadata not implying artifact access — are properties of running infrastructure
/// and are not modelled anywhere in this workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentProfile {
    name: String,
    bindings: BTreeMap<SettingKey, Binding>,
}

impl DeploymentProfile {
    pub fn new(name: impl Into<String>) -> Self {
        DeploymentProfile {
            name: name.into(),
            bindings: BTreeMap::new(),
        }
    }

    pub fn bind(mut self, key: SettingKey, binding: Binding) -> Self {
        self.bindings.insert(key, binding);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The profile as a configuration source at [`Layer::DeploymentProfile`].
    pub fn as_source(&self) -> Source {
        Source {
            layer: Layer::DeploymentProfile,
            origin: self.name.clone(),
            bindings: self.bindings.clone(),
        }
    }
}

/// Which layer won a key, and where in that layer it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub binding: Binding,
    pub layer: Layer,
    pub origin: String,
}

/// The layers of a program's configuration, before resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigStack {
    schema: Schema,
    sources: Vec<Source>,
}

impl ConfigStack {
    pub fn new(schema: Schema) -> Self {
        ConfigStack {
            schema,
            sources: Vec::new(),
        }
    }

    pub fn push(mut self, source: Source) -> Self {
        self.sources.push(source);
        self
    }

    pub fn with_profile(self, profile: &DeploymentProfile) -> Self {
        self.push(profile.as_source())
    }

    /// Resolves the layers into an effective configuration.
    ///
    /// The failures raised here are 40.10's four, each in the form that makes it decidable:
    /// `unknown setting` when a layer binds an undeclared key, `ambiguous precedence` when one
    /// layer contains two disagreeing sources for a key, an unsafe default when the defaults layer
    /// grants a permission, and a type mismatch when a binding does not fit its declaration.
    /// 40.10's `secret unavailable` is not raised here: an unbound secret is a legitimate
    /// configuration until somebody asks for a lease, and [`EffectiveConfig::lease`] is where it
    /// becomes a failure.
    pub fn resolve(&self) -> Result<EffectiveConfig, OpsError> {
        let mut resolved: BTreeMap<SettingKey, Resolution> = BTreeMap::new();

        for layer in Layer::ASCENDING {
            let mut at_this_layer: BTreeMap<SettingKey, (Binding, String)> = BTreeMap::new();

            for source in self.sources.iter().filter(|s| s.layer == layer) {
                for (key, binding) in &source.bindings {
                    let spec = self.schema.get(key).ok_or_else(|| OpsError::UnknownSetting {
                        key: key.to_string(),
                        origin: source.origin.clone(),
                    })?;

                    if let Err((expected, actual)) = spec.accepts(binding) {
                        return Err(OpsError::TypeMismatch {
                            key: key.to_string(),
                            expected,
                            actual,
                            origin: source.origin.clone(),
                        });
                    }

                    if layer == Layer::Defaults
                        && matches!(spec.kind, SettingKind::Permission)
                        && binding_grants(binding)
                    {
                        return Err(OpsError::UnsafeDefault {
                            key: key.to_string(),
                        });
                    }

                    if layer == Layer::DeploymentProfile && spec.influence.is_emitted() {
                        return Err(OpsError::ProfileChangesSemantics {
                            profile: source.origin.clone(),
                            key: key.to_string(),
                        });
                    }

                    match at_this_layer.get(key) {
                        Some((existing, first_origin)) if existing != binding => {
                            return Err(OpsError::AmbiguousPrecedence {
                                key: key.to_string(),
                                layer: layer.to_string(),
                                first: first_origin.clone(),
                                second: source.origin.clone(),
                            });
                        }
                        _ => {
                            at_this_layer
                                .insert(key.clone(), (binding.clone(), source.origin.clone()));
                        }
                    }
                }
            }

            for (key, (binding, origin)) in at_this_layer {
                resolved.insert(key, Resolution { binding, layer, origin });
            }
        }

        for (key, spec) in &self.schema.specs {
            if resolved.contains_key(key) {
                continue;
            }
            match spec.kind {
                SettingKind::Permission => {
                    resolved.insert(
                        key.clone(),
                        Resolution {
                            binding: Binding::Value(SettingValue::Bool(false)),
                            layer: Layer::Defaults,
                            origin: "implicit-deny".to_string(),
                        },
                    );
                }
                SettingKind::Secret => {}
                SettingKind::Value { .. } => {
                    return Err(OpsError::MissingRequiredSetting {
                        key: key.to_string(),
                    })
                }
            }
        }

        Ok(EffectiveConfig {
            schema: self.schema.clone(),
            resolved,
        })
    }
}

fn binding_grants(binding: &Binding) -> bool {
    matches!(binding, Binding::Value(SettingValue::Bool(true)))
}

/// The resolved configuration of one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveConfig {
    schema: Schema,
    resolved: BTreeMap<SettingKey, Resolution>,
}

impl EffectiveConfig {
    pub fn get(&self, key: &SettingKey) -> Option<&Resolution> {
        self.resolved.get(key)
    }

    pub fn value(&self, key: &SettingKey) -> Option<&SettingValue> {
        match self.resolved.get(key).map(|r| &r.binding) {
            Some(Binding::Value(value)) => Some(value),
            _ => None,
        }
    }

    /// Whether a permission is granted. An absent permission is denied, never unknown.
    pub fn granted(&self, key: &SettingKey) -> bool {
        self.value(key).and_then(SettingValue::as_bool).unwrap_or(false)
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn keys(&self) -> impl Iterator<Item = &SettingKey> {
        self.resolved.keys()
    }

    /// Every secret reference the run will need, in key order.
    pub fn secret_references(&self) -> Vec<&SecretRef> {
        self.resolved
            .values()
            .filter_map(|r| match &r.binding {
                Binding::Secret(reference) => Some(reference),
                Binding::Value(_) => None,
            })
            .collect()
    }

    /// The nonsecret effective settings, for a result bundle. 40.10's second invariant.
    ///
    /// Secrets appear as their references, which is what makes the snapshot both complete and
    /// safe: a reader learns that `hub.token` came from `vault:kv/hub#token` and learns nothing
    /// about the token.
    pub fn snapshot(&self) -> Value {
        let mut map = Map::new();
        for (key, resolution) in &self.resolved {
            let mut entry = Map::new();
            entry.insert("value".to_string(), resolution.binding.to_json());
            entry.insert(
                "layer".to_string(),
                Value::String(resolution.layer.as_str().to_string()),
            );
            entry.insert(
                "origin".to_string(),
                Value::String(resolution.origin.clone()),
            );
            entry.insert(
                "influence".to_string(),
                Value::String(self.influence_of(key).as_str().to_string()),
            );
            map.insert(key.as_str().to_string(), Value::Object(entry));
        }
        Value::Object(map)
    }

    /// The redacted config fingerprint 40.10 lists as an output.
    ///
    /// Covers every setting including the origin it was resolved from, so two runs that agree on
    /// values but disagree on where they came from are distinguishable. That matters for
    /// reproduction: a value taken from the command line is not the same evidence as the same value
    /// taken from a checked-in file.
    pub fn fingerprint(&self) -> Result<ContentHash, OpsError> {
        ContentHash::of_value(&self.snapshot()).map_err(|error| OpsError::MalformedName {
            field: "config fingerprint".to_string(),
            value: error.to_string(),
        })
    }

    /// The fingerprint over only the settings that participate in emitted artifacts.
    ///
    /// Two configurations with the same emitted fingerprint cannot produce different artifact
    /// bytes *by way of configuration*. That is the property [`DeploymentProfile`] relies on, and
    /// it is tested rather than asserted.
    pub fn emitted_fingerprint(&self) -> Result<ContentHash, OpsError> {
        let mut map = Map::new();
        for (key, resolution) in &self.resolved {
            if !self.influence_of(key).is_emitted() {
                continue;
            }
            map.insert(key.as_str().to_string(), resolution.binding.to_json());
        }
        ContentHash::of_value(&Value::Object(map)).map_err(|error| OpsError::MalformedName {
            field: "emitted fingerprint".to_string(),
            value: error.to_string(),
        })
    }

    /// A lease on a secret, scoped to a named execution boundary and an epoch window.
    ///
    /// 40.10 says *lease secrets at the execution boundary*. The boundary name is required and not
    /// defaulted, because a lease with no boundary is indistinguishable from ambient access, which
    /// is what [`crate::hardening`] exists to find.
    pub fn lease(
        &self,
        key: &SettingKey,
        boundary: impl Into<String>,
        issued: Epoch,
        ttl: u64,
    ) -> Result<SecretLease, OpsError> {
        match self.resolved.get(key).map(|r| &r.binding) {
            Some(Binding::Secret(reference)) => Ok(SecretLease {
                reference: reference.clone(),
                boundary: boundary.into(),
                issued,
                expires: Epoch::new(issued.tick().saturating_add(ttl)),
            }),
            _ => Err(OpsError::SecretUnavailable {
                key: key.to_string(),
            }),
        }
    }

    fn influence_of(&self, key: &SettingKey) -> Influence {
        self.schema
            .get(key)
            .map(SettingSpec::influence)
            .unwrap_or(Influence::Operational)
    }
}

/// Permission to resolve one secret, at one boundary, for a bounded number of epochs.
///
/// Carries no value and has no `resolve` method; see the module docs. Deliberately not `Clone` and
/// deliberately not `Serialize`: a lease that can be copied into a bundle is a lease that has left
/// its boundary.
#[derive(Debug, PartialEq, Eq)]
pub struct SecretLease {
    reference: SecretRef,
    boundary: String,
    issued: Epoch,
    expires: Epoch,
}

impl SecretLease {
    pub fn reference(&self) -> &SecretRef {
        &self.reference
    }

    pub fn boundary(&self) -> &str {
        &self.boundary
    }

    pub fn issued(&self) -> Epoch {
        self.issued
    }

    pub fn expires(&self) -> Epoch {
        self.expires
    }

    /// Checks the lease at an epoch. 40.10's `secret expiry`, on a logical clock.
    pub fn check(&self, at: Epoch) -> Result<(), OpsError> {
        if at > self.expires {
            return Err(OpsError::LeaseExpired {
                reference: self.reference.to_string(),
                expires: self.expires.tick(),
                consulted: at.tick(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> SettingKey {
        SettingKey::parse(name).expect("well-formed")
    }

    fn base_schema() -> Schema {
        Schema::new()
            .with(SettingSpec::value(
                key("compile.max_hops"),
                ValueType::Integer,
                Influence::Emitted,
            ))
            .unwrap()
            .with(SettingSpec::value(
                key("store.root"),
                ValueType::Text,
                Influence::Operational,
            ))
            .unwrap()
            .with(SettingSpec::permission(
                key("allow.unreviewed_packs"),
                Influence::Emitted,
            ))
            .unwrap()
            .with(SettingSpec::secret(key("hub.token")))
            .unwrap()
    }

    fn defaults() -> Source {
        Source::new(Layer::Defaults, "built-in")
            .bind(
                key("compile.max_hops"),
                Binding::Value(SettingValue::Integer(3)),
            )
            .bind(
                key("store.root"),
                Binding::Value(SettingValue::Text("./.bioprism".into())),
            )
    }

    #[test]
    fn a_secret_setting_cannot_be_declared_as_entering_an_emitted_artifact() {
        let spec = SettingSpec::secret(key("hub.token"));
        assert_eq!(spec.influence(), Influence::Operational);

        let hostile: SettingSpec = serde_json::from_value(serde_json::json!({
            "key": "hub.token",
            "kind": { "kind": "secret" },
            "influence": "emitted"
        }))
        .expect("serde bypasses constructors");
        let error = Schema::new().declare(hostile).unwrap_err();
        assert!(matches!(error, OpsError::SecretInDigest { .. }));
    }

    #[test]
    fn a_permission_absent_from_every_layer_resolves_to_denied_rather_than_unknown() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .resolve()
            .expect("resolves");
        assert!(!effective.granted(&key("allow.unreviewed_packs")));
        assert_eq!(
            effective.get(&key("allow.unreviewed_packs")).unwrap().origin,
            "implicit-deny"
        );
    }

    #[test]
    fn the_defaults_layer_may_not_grant_a_permission() {
        let error = ConfigStack::new(base_schema())
            .push(defaults().bind(
                key("allow.unreviewed_packs"),
                Binding::Value(SettingValue::Bool(true)),
            ))
            .resolve()
            .unwrap_err();
        assert!(matches!(error, OpsError::UnsafeDefault { .. }));
    }

    #[test]
    fn a_higher_layer_may_grant_a_permission_the_defaults_deny() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .push(Source::new(Layer::CommandLine, "--allow-unreviewed-packs").bind(
                key("allow.unreviewed_packs"),
                Binding::Value(SettingValue::Bool(true)),
            ))
            .resolve()
            .expect("resolves");
        assert!(effective.granted(&key("allow.unreviewed_packs")));
    }

    #[test]
    fn two_sources_in_one_layer_disagreeing_about_a_key_is_a_failure_not_a_coin_flip() {
        let error = ConfigStack::new(base_schema())
            .push(defaults())
            .push(
                Source::new(Layer::Environment, "BIOPRISM_STORE")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/a".into()))),
            )
            .push(
                Source::new(Layer::Environment, "BIOPRISM_STORE_ROOT")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/b".into()))),
            )
            .resolve()
            .unwrap_err();
        match error {
            OpsError::AmbiguousPrecedence { key, .. } => assert_eq!(key, "store.root"),
            other => panic!("expected ambiguous precedence, got {other}"),
        }
    }

    #[test]
    fn two_sources_in_one_layer_agreeing_about_a_key_resolve_without_complaint() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .push(
                Source::new(Layer::Environment, "BIOPRISM_STORE")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/a".into()))),
            )
            .push(
                Source::new(Layer::Environment, "BIOPRISM_STORE_ROOT")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/a".into()))),
            )
            .resolve()
            .expect("agreement is not ambiguity");
        assert_eq!(
            effective.value(&key("store.root")),
            Some(&SettingValue::Text("/a".into()))
        );
    }

    #[test]
    fn precedence_follows_the_type_order_not_the_order_layers_were_pushed() {
        let effective = ConfigStack::new(base_schema())
            .push(
                Source::new(Layer::CommandLine, "--store-root")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/cli".into()))),
            )
            .push(defaults())
            .push(
                Source::new(Layer::Environment, "BIOPRISM_STORE")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/env".into()))),
            )
            .resolve()
            .expect("resolves");
        assert_eq!(
            effective.value(&key("store.root")),
            Some(&SettingValue::Text("/cli".into()))
        );
        assert_eq!(effective.get(&key("store.root")).unwrap().layer, Layer::CommandLine);
    }

    #[test]
    fn a_deployment_profile_cannot_bind_a_setting_that_enters_an_emitted_artifact() {
        let profile = DeploymentProfile::new("hosted").bind(
            key("compile.max_hops"),
            Binding::Value(SettingValue::Integer(9)),
        );
        let error = ConfigStack::new(base_schema())
            .push(defaults())
            .with_profile(&profile)
            .resolve()
            .unwrap_err();
        match error {
            OpsError::ProfileChangesSemantics { profile, key } => {
                assert_eq!(profile, "hosted");
                assert_eq!(key, "compile.max_hops");
            }
            other => panic!("expected a semantics violation, got {other}"),
        }
    }

    #[test]
    fn switching_deployment_profiles_cannot_move_the_emitted_fingerprint() {
        let local = DeploymentProfile::new("local").bind(
            key("store.root"),
            Binding::Value(SettingValue::Text("./local".into())),
        );
        let hosted = DeploymentProfile::new("hosted").bind(
            key("store.root"),
            Binding::Value(SettingValue::Text("s3://bucket".into())),
        );

        let under = |profile: &DeploymentProfile| {
            ConfigStack::new(base_schema())
                .push(defaults())
                .with_profile(profile)
                .resolve()
                .expect("resolves")
        };

        let a = under(&local);
        let b = under(&hosted);
        assert_ne!(a.fingerprint().unwrap(), b.fingerprint().unwrap());
        assert_eq!(
            a.emitted_fingerprint().unwrap(),
            b.emitted_fingerprint().unwrap(),
            "40.38 invariant 1: a profile changes providers, not object semantics"
        );
    }

    #[test]
    fn changing_a_setting_that_enters_an_emitted_artifact_moves_the_emitted_fingerprint() {
        let base = ConfigStack::new(base_schema())
            .push(defaults())
            .resolve()
            .unwrap();
        let deeper = ConfigStack::new(base_schema())
            .push(defaults())
            .push(Source::new(Layer::CommandLine, "--max-hops").bind(
                key("compile.max_hops"),
                Binding::Value(SettingValue::Integer(5)),
            ))
            .resolve()
            .unwrap();
        assert_ne!(
            base.emitted_fingerprint().unwrap(),
            deeper.emitted_fingerprint().unwrap()
        );
    }

    #[test]
    fn the_same_value_from_a_different_origin_is_a_different_configuration() {
        let from_file = ConfigStack::new(base_schema())
            .push(defaults())
            .push(
                Source::new(Layer::ProjectFile, "bioprism.toml")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/x".into()))),
            )
            .resolve()
            .unwrap();
        let from_cli = ConfigStack::new(base_schema())
            .push(defaults())
            .push(
                Source::new(Layer::CommandLine, "--store-root")
                    .bind(key("store.root"), Binding::Value(SettingValue::Text("/x".into()))),
            )
            .resolve()
            .unwrap();
        assert_ne!(from_file.fingerprint().unwrap(), from_cli.fingerprint().unwrap());
        assert_eq!(
            from_file.emitted_fingerprint().unwrap(),
            from_cli.emitted_fingerprint().unwrap()
        );
    }

    #[test]
    fn a_snapshot_carries_the_secret_reference_and_never_a_secret() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .push(Source::new(Layer::Environment, "BIOPRISM_HUB_TOKEN").bind(
                key("hub.token"),
                Binding::Secret(SecretRef::new(SecretSource::Vault, "kv/hub#token").unwrap()),
            ))
            .resolve()
            .unwrap();
        let snapshot = effective.snapshot();
        let rendered = serde_json::to_string(&snapshot).unwrap();
        assert!(rendered.contains("vault:kv/hub#token"));
        assert_eq!(effective.secret_references().len(), 1);
    }

    #[test]
    fn an_unbound_secret_resolves_and_fails_only_when_a_lease_is_asked_for() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .resolve()
            .expect("an unbound secret is a legitimate configuration");
        let error = effective
            .lease(&key("hub.token"), "oracle-sandbox", Epoch::new(1), 4)
            .unwrap_err();
        assert!(matches!(error, OpsError::SecretUnavailable { .. }));
    }

    #[test]
    fn a_lease_expires_on_the_logical_clock_and_names_the_boundary_it_was_issued_to() {
        let effective = ConfigStack::new(base_schema())
            .push(defaults())
            .push(Source::new(Layer::Environment, "vault-ref").bind(
                key("hub.token"),
                Binding::Secret(SecretRef::new(SecretSource::Vault, "kv/hub#token").unwrap()),
            ))
            .resolve()
            .unwrap();
        let lease = effective
            .lease(&key("hub.token"), "publish-boundary", Epoch::new(2), 3)
            .unwrap();
        assert_eq!(lease.boundary(), "publish-boundary");
        assert!(lease.check(Epoch::new(5)).is_ok());
        assert!(matches!(
            lease.check(Epoch::new(6)).unwrap_err(),
            OpsError::LeaseExpired { .. }
        ));
    }

    #[test]
    fn a_value_setting_nobody_bound_is_a_failure_rather_than_an_implicit_empty() {
        let error = ConfigStack::new(base_schema())
            .push(Source::new(Layer::Defaults, "built-in").bind(
                key("compile.max_hops"),
                Binding::Value(SettingValue::Integer(3)),
            ))
            .resolve()
            .unwrap_err();
        match error {
            OpsError::MissingRequiredSetting { key } => assert_eq!(key, "store.root"),
            other => panic!("expected a missing setting, got {other}"),
        }
    }

    #[test]
    fn a_layer_binding_an_undeclared_key_names_the_source_that_did_it() {
        let error = ConfigStack::new(base_schema())
            .push(defaults())
            .push(
                Source::new(Layer::Environment, "BIOPRISM_TYPO")
                    .bind(key("store.rooot"), Binding::Value(SettingValue::Text("/x".into()))),
            )
            .resolve()
            .unwrap_err();
        match error {
            OpsError::UnknownSetting { key, origin } => {
                assert_eq!(key, "store.rooot");
                assert_eq!(origin, "BIOPRISM_TYPO");
            }
            other => panic!("expected an unknown setting, got {other}"),
        }
    }

    #[test]
    fn a_binding_of_the_wrong_type_is_refused_rather_than_coerced() {
        let error = ConfigStack::new(base_schema())
            .push(defaults())
            .push(Source::new(Layer::CommandLine, "--max-hops").bind(
                key("compile.max_hops"),
                Binding::Value(SettingValue::Text("three".into())),
            ))
            .resolve()
            .unwrap_err();
        assert!(matches!(error, OpsError::TypeMismatch { .. }));
    }

    #[test]
    fn a_fingerprint_is_the_same_on_two_resolutions_of_the_same_layers() {
        let build = || {
            ConfigStack::new(base_schema())
                .push(defaults())
                .resolve()
                .unwrap()
                .fingerprint()
                .unwrap()
        };
        assert_eq!(build(), build());
    }
}
