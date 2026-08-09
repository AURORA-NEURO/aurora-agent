//! Effect types, resource scopes and irreversibility classes.
//!
//! Blueprint 23.14 (effect taxonomy, static and runtime checks, irreversibility classes, policy
//! composition). The information-flow half of 23.14 is [`crate::flow`]; this module is the
//! "what a participant may do to the world" half.
//!
//! # Three decisions worth stating
//!
//! **An undeclared scope is not an unlimited one, and it is not a bounded one either.** 23.14
//! writes `filesystem.write<repo-x/branch/42/**>` and says effects "are parameterized by resource
//! and scope", but every example in §23 also writes bare `filesystem.read` with no parameter. The
//! tempting readings are both wrong: treating a bare effect as *all resources* makes every
//! contract maximally dangerous, and treating it as *no resources* makes every contract vacuously
//! safe. [`Scope::Undeclared`] is neither. Containment against it returns
//! [`Containment::Undecided`], which a gate treats as refusal and a report treats as a hole.
//!
//! **Containment is three-valued all the way up.** [`EffectSet::includes`] returns
//! [`Inclusion`], not `bool`, so `B.E ⊆ A.E` in 23.41's substitution relation can come back
//! *undecided* rather than silently true. This is the same discipline
//! `bioprism_choreography::Verdict::Inconclusive` applies to model checking: a procedure that did
//! not settle the question must not be typed as one that settled it affirmatively.
//!
//! **The irreversibility floor is this crate's, not the blueprint's.** 23.14 lists classes E0–E4
//! and lists eighteen effect kinds and never connects them. [`EffectKind::irreversibility_floor`]
//! is the connection, written here so a disagreement is a disagreement about one table.
//!
//! Nothing here performs an effect. There is no filesystem access, no network, no process
//! execution; an [`Effect`] is a *declaration* that something may happen, and the whole module is
//! a calculus over declarations.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The closed effect taxonomy of 23.14.
///
/// Closed on purpose. An open string vocabulary would let a participant declare an effect no
/// policy anticipated and no gate recognised, which is precisely the escalation path 23.14's
/// evaluation section asks to be tested for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    PureCompute,
    ArtifactRead,
    ArtifactWrite,
    FilesystemRead,
    FilesystemWrite,
    ProcessExecute,
    NetworkRead,
    NetworkWrite,
    MessageSend,
    AgentSpawn,
    AgentDelegate,
    BudgetSpend,
    SecretUse,
    PolicyChange,
    ExternalPublish,
    HumanContact,
    ClinicalOutput,
    IrreversibleEffect,
}

impl EffectKind {
    /// The taxonomy exactly as 23.14 lists it, in its order.
    pub const TAXONOMY: [EffectKind; 18] = [
        EffectKind::PureCompute,
        EffectKind::ArtifactRead,
        EffectKind::ArtifactWrite,
        EffectKind::FilesystemRead,
        EffectKind::FilesystemWrite,
        EffectKind::ProcessExecute,
        EffectKind::NetworkRead,
        EffectKind::NetworkWrite,
        EffectKind::MessageSend,
        EffectKind::AgentSpawn,
        EffectKind::AgentDelegate,
        EffectKind::BudgetSpend,
        EffectKind::SecretUse,
        EffectKind::PolicyChange,
        EffectKind::ExternalPublish,
        EffectKind::HumanContact,
        EffectKind::ClinicalOutput,
        EffectKind::IrreversibleEffect,
    ];

    /// The dotted name 23.14 writes, e.g. `filesystem.write`.
    pub fn as_str(&self) -> &'static str {
        match self {
            EffectKind::PureCompute => "pure.compute",
            EffectKind::ArtifactRead => "artifact.read",
            EffectKind::ArtifactWrite => "artifact.write",
            EffectKind::FilesystemRead => "filesystem.read",
            EffectKind::FilesystemWrite => "filesystem.write",
            EffectKind::ProcessExecute => "process.execute",
            EffectKind::NetworkRead => "network.read",
            EffectKind::NetworkWrite => "network.write",
            EffectKind::MessageSend => "message.send",
            EffectKind::AgentSpawn => "agent.spawn",
            EffectKind::AgentDelegate => "agent.delegate",
            EffectKind::BudgetSpend => "budget.spend",
            EffectKind::SecretUse => "secret.use",
            EffectKind::PolicyChange => "policy.change",
            EffectKind::ExternalPublish => "external.publish",
            EffectKind::HumanContact => "human.contact",
            EffectKind::ClinicalOutput => "clinical.output",
            EffectKind::IrreversibleEffect => "irreversible.effect",
        }
    }

    /// Parse a dotted name from the taxonomy. Unknown names are refused rather than wrapped in an
    /// `Other(String)` escape hatch, which would reopen the closed set through the back door.
    pub fn parse(name: &str) -> Result<Self, EffectError> {
        EffectKind::TAXONOMY
            .iter()
            .copied()
            .find(|k| k.as_str() == name)
            .ok_or_else(|| EffectError::UnknownEffectKind(name.to_string()))
    }

    /// The lowest irreversibility class this kind may honestly be declared at.
    ///
    /// **This mapping is not in the blueprint.** 23.14 defines E0–E4 and defines the taxonomy and
    /// never relates them, so a policy written against "class ≥ E3 requires quorum" has nothing to
    /// evaluate. The floor below is this crate's reading. A declaration *above* the floor is
    /// always allowed — a caller who knows its `artifact.write` targets a published corpus may
    /// declare E3 — but a declaration below it is [`EffectError::ClassBelowFloor`].
    pub fn irreversibility_floor(&self) -> Irreversibility {
        match self {
            EffectKind::PureCompute
            | EffectKind::ArtifactRead
            | EffectKind::FilesystemRead
            | EffectKind::NetworkRead => Irreversibility::E0,
            EffectKind::ArtifactWrite | EffectKind::FilesystemWrite | EffectKind::ProcessExecute => {
                Irreversibility::E1
            }
            EffectKind::MessageSend
            | EffectKind::AgentSpawn
            | EffectKind::AgentDelegate
            | EffectKind::PolicyChange => Irreversibility::E2,
            EffectKind::NetworkWrite | EffectKind::BudgetSpend | EffectKind::SecretUse => {
                Irreversibility::E3
            }
            EffectKind::ExternalPublish
            | EffectKind::HumanContact
            | EffectKind::ClinicalOutput
            | EffectKind::IrreversibleEffect => Irreversibility::E4,
        }
    }

    /// Whether the kind mutates anything a peer could observe. Drives the write-set disjointness
    /// side condition of 23.41's parallel commutativity law.
    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            EffectKind::PureCompute
                | EffectKind::ArtifactRead
                | EffectKind::FilesystemRead
                | EffectKind::NetworkRead
        )
    }
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 23.14's irreversibility classes. Ordered, because "policies may require increasing
/// verification, quorum, or human approval by class" is only meaningful if the classes compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Irreversibility {
    /// Pure or read-only.
    E0,
    /// Local reversible mutation.
    E1,
    /// Shared reversible mutation.
    E2,
    /// Externally visible or costly action.
    E3,
    /// Irreversible, safety-critical, legal, financial or clinical action.
    E4,
}

impl Irreversibility {
    /// True when an action of this class cannot be undone by a compensating action, which is what
    /// makes 23.41's sequential associativity law fail: reassociating changes where the
    /// compensation scope boundary falls, and a boundary that cannot be crossed backwards is
    /// observable.
    pub fn is_irreversible(&self) -> bool {
        *self == Irreversibility::E4
    }
}

/// A resource pattern, e.g. `repo-x/branch/42/**`.
///
/// Segments split on `/`. A `*` segment matches exactly one segment; a trailing `**` matches one
/// or more remaining segments. `**` anywhere but last is refused, because `a/**/b` has two
/// plausible readings and picking one silently is how two implementations diverge.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ResourcePattern {
    raw: String,
}

impl ResourcePattern {
    pub fn parse(raw: impl Into<String>) -> Result<Self, EffectError> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(EffectError::EmptyResourcePattern);
        }
        let segments: Vec<&str> = raw.split('/').collect();
        for (index, segment) in segments.iter().enumerate() {
            if *segment == "**" && index + 1 != segments.len() {
                return Err(EffectError::InteriorRecursiveWildcard(raw.clone()));
            }
            if segment.is_empty() {
                return Err(EffectError::EmptySegment(raw.clone()));
            }
        }
        Ok(ResourcePattern { raw })
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    fn segments(&self) -> Vec<&str> {
        self.raw.split('/').collect()
    }

    /// Whether every resource matched by `inner` is matched by `self`.
    ///
    /// Decided structurally over patterns rather than by enumerating resources, because the
    /// resource universe is not available to a static check and 23.14 asks for exactly that check
    /// ("static checks compute possible effects from the Weave program").
    pub fn contains(&self, inner: &ResourcePattern) -> bool {
        contains_segments(&self.segments(), &inner.segments())
    }
}

fn contains_segments(outer: &[&str], inner: &[&str]) -> bool {
    match (outer.first(), inner.first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(&"**"), _) if outer.len() == 1 => !inner.is_empty(),
        (Some(_), None) => false,
        (Some(&"**"), Some(_)) => false,
        (Some(&"*"), Some(head)) => *head != "**" && contains_segments(&outer[1..], &inner[1..]),
        (Some(o), Some(i)) => o == i && contains_segments(&outer[1..], &inner[1..]),
    }
}

impl fmt::Display for ResourcePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl From<ResourcePattern> for String {
    fn from(value: ResourcePattern) -> Self {
        value.raw
    }
}

impl TryFrom<String> for ResourcePattern {
    type Error = EffectError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ResourcePattern::parse(value)
    }
}

/// The resource parameter of an effect.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope", content = "pattern")]
pub enum Scope {
    /// `filesystem.write<repo-x/branch/42/**>`.
    Resource(ResourcePattern),
    /// The effect was declared with no resource parameter.
    ///
    /// Deliberately not a synonym for "everything" and not a synonym for "nothing". See the module
    /// documentation.
    Undeclared,
}

/// The three-valued answer to "does this contain that".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Containment {
    Contains,
    DoesNotContain,
    /// One side's scope is undeclared and the other's is not, so no structural comparison exists.
    Undecided,
}

impl Scope {
    /// Whether every resource this scope permits is permitted by `self`.
    ///
    /// Two undeclared scopes on the same effect kind are [`Containment::Contains`]: the
    /// *declarations* are identical, so the inner side demands nothing the outer side did not
    /// already demand. Comparing an undeclared scope with a bounded one is
    /// [`Containment::Undecided`] in both directions.
    pub fn contains(&self, inner: &Scope) -> Containment {
        match (self, inner) {
            (Scope::Undeclared, Scope::Undeclared) => Containment::Contains,
            (Scope::Undeclared, Scope::Resource(_)) | (Scope::Resource(_), Scope::Undeclared) => {
                Containment::Undecided
            }
            (Scope::Resource(outer), Scope::Resource(inner)) => {
                if outer.contains(inner) {
                    Containment::Contains
                } else {
                    Containment::DoesNotContain
                }
            }
        }
    }

    pub fn resource(pattern: impl Into<String>) -> Result<Self, EffectError> {
        Ok(Scope::Resource(ResourcePattern::parse(pattern)?))
    }
}

/// One declared effect: a kind, its resource scope, and the irreversibility class the declarer
/// commits to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Effect {
    pub kind: EffectKind,
    pub scope: Scope,
    pub class: Irreversibility,
}

impl Effect {
    /// Declare an effect at its floor class.
    pub fn new(kind: EffectKind, scope: Scope) -> Self {
        Effect {
            class: kind.irreversibility_floor(),
            kind,
            scope,
        }
    }

    /// Declare an effect at an explicit class, refusing anything below the floor.
    pub fn at_class(
        kind: EffectKind,
        scope: Scope,
        class: Irreversibility,
    ) -> Result<Self, EffectError> {
        let floor = kind.irreversibility_floor();
        if class < floor {
            return Err(EffectError::ClassBelowFloor {
                kind,
                declared: class,
                floor,
            });
        }
        Ok(Effect { kind, scope, class })
    }

    /// Whether `self` permits everything `inner` does: same kind, containing scope, and a class at
    /// least as high, since a lower declared class is a weaker promise about what may happen.
    pub fn permits(&self, inner: &Effect) -> Containment {
        if self.kind != inner.kind {
            return Containment::DoesNotContain;
        }
        match self.scope.contains(&inner.scope) {
            Containment::Contains if self.class >= inner.class => Containment::Contains,
            Containment::Contains => Containment::DoesNotContain,
            other => other,
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.scope {
            Scope::Resource(pattern) => write!(f, "{}<{}>", self.kind, pattern),
            Scope::Undeclared => write!(f, "{}", self.kind),
        }
    }
}

/// A set of declared effects. `E` in 23.41's contract tuple.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectSet {
    effects: BTreeSet<Effect>,
}

/// The three-valued answer to `B.E ⊆ A.E`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "inclusion")]
pub enum Inclusion {
    Holds,
    /// At least one effect on the inner side is permitted by nothing on the outer side.
    Fails { witnesses: Vec<Effect> },
    /// No effect definitively escapes, but at least one comparison involved an undeclared scope.
    Undecided { witnesses: Vec<Effect> },
}

impl Inclusion {
    /// A gate treats undecided as refusal. A report does not, which is why the distinction is in
    /// the type rather than in a `bool` returned by a function named `is_ok`.
    pub fn admitted(&self) -> bool {
        matches!(self, Inclusion::Holds)
    }
}

impl EffectSet {
    pub fn new() -> Self {
        EffectSet::default()
    }

    pub fn from_effects(effects: impl IntoIterator<Item = Effect>) -> Self {
        EffectSet {
            effects: effects.into_iter().collect(),
        }
    }

    pub fn with(mut self, effect: Effect) -> Self {
        self.effects.insert(effect);
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.effects.iter()
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn contains_kind(&self, kind: EffectKind) -> bool {
        self.effects.iter().any(|e| e.kind == kind)
    }

    /// The highest irreversibility class in the set, or [`Irreversibility::E0`] when empty.
    pub fn peak_class(&self) -> Irreversibility {
        self.effects
            .iter()
            .map(|e| e.class)
            .max()
            .unwrap_or(Irreversibility::E0)
    }

    /// The effects that mutate something. Used by 23.41's parallel commutativity side condition.
    pub fn write_set(&self) -> BTreeSet<Effect> {
        self.effects
            .iter()
            .filter(|e| e.kind.is_write())
            .cloned()
            .collect()
    }

    /// Whether `self` permits everything in `inner`.
    pub fn includes(&self, inner: &EffectSet) -> Inclusion {
        let mut failures = Vec::new();
        let mut undecided = Vec::new();
        for effect in &inner.effects {
            let mut best = Containment::DoesNotContain;
            for outer in &self.effects {
                match outer.permits(effect) {
                    Containment::Contains => {
                        best = Containment::Contains;
                        break;
                    }
                    Containment::Undecided => best = Containment::Undecided,
                    Containment::DoesNotContain => {}
                }
            }
            match best {
                Containment::Contains => {}
                Containment::Undecided => undecided.push(effect.clone()),
                Containment::DoesNotContain => failures.push(effect.clone()),
            }
        }
        if !failures.is_empty() {
            Inclusion::Fails {
                witnesses: failures,
            }
        } else if !undecided.is_empty() {
            Inclusion::Undecided {
                witnesses: undecided,
            }
        } else {
            Inclusion::Holds
        }
    }

    pub fn union(&self, other: &EffectSet) -> EffectSet {
        EffectSet {
            effects: self.effects.union(&other.effects).cloned().collect(),
        }
    }

    /// Effects present in `self` that `outer` does not permit. The escalation witness of 23.14's
    /// "effect escalation through a composite molecule" evaluation case.
    pub fn escalation_over(&self, outer: &EffectSet) -> Vec<Effect> {
        match outer.includes(self) {
            Inclusion::Holds => Vec::new(),
            Inclusion::Fails { witnesses } | Inclusion::Undecided { witnesses } => witnesses,
        }
    }
}

/// One policy layer. 23.14: policies come from "user, organization, thread, molecule, data owner,
/// and runtime".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicySource {
    User,
    Organization,
    Thread,
    Molecule,
    DataOwner,
    Runtime,
}

/// A single layer's opinion about effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPolicy {
    pub source: PolicySource,
    pub allowed: EffectSet,
    pub prohibited: EffectSet,
    /// Classes at or above this require an approval transition the effect checker cannot grant.
    pub approval_required_from: Option<Irreversibility>,
}

impl EffectPolicy {
    pub fn new(source: PolicySource) -> Self {
        EffectPolicy {
            source,
            allowed: EffectSet::new(),
            prohibited: EffectSet::new(),
            approval_required_from: None,
        }
    }

    pub fn allowing(mut self, effect: Effect) -> Self {
        self.allowed = std::mem::take(&mut self.allowed).with(effect);
        self
    }

    pub fn prohibiting(mut self, effect: Effect) -> Self {
        self.prohibited = std::mem::take(&mut self.prohibited).with(effect);
        self
    }

    pub fn requiring_approval_from(mut self, class: Irreversibility) -> Self {
        self.approval_required_from = Some(class);
        self
    }
}

/// The composition of several policy layers.
///
/// 23.14's reference rule verbatim: "deny-by-default with intersection of allowed effects and
/// union of prohibitions". Intersection of *allowed* is implemented as "permitted by every layer",
/// not as set intersection of the literal declarations, because a layer allowing
/// `filesystem.write<repo/**>` and another allowing `filesystem.write<repo/src/**>` intersect to
/// the narrower pattern and their literal sets intersect to nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposedPolicy {
    layers: Vec<EffectPolicy>,
}

/// What a composed policy says about one requested effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "gate")]
pub enum Gate {
    Permitted,
    /// A layer prohibits it outright.
    Prohibited { by: PolicySource, rule: Effect },
    /// No layer prohibits it and at least one layer does not allow it. Deny-by-default.
    NotAllowed { by: PolicySource },
    /// Allowed by every layer but its class needs an approval transition.
    ApprovalRequired {
        by: PolicySource,
        threshold: Irreversibility,
    },
    /// Every comparison that could refuse it was undecided.
    Undecided { by: PolicySource },
}

impl ComposedPolicy {
    pub fn compose(layers: impl IntoIterator<Item = EffectPolicy>) -> Self {
        let mut layers: Vec<EffectPolicy> = layers.into_iter().collect();
        layers.sort_by_key(|layer| layer.source);
        ComposedPolicy { layers }
    }

    pub fn layers(&self) -> &[EffectPolicy] {
        &self.layers
    }

    /// Prohibitions win over permissions, and the check runs in a fixed layer order so two runs
    /// naming different sources for the same refusal is impossible.
    pub fn gate(&self, effect: &Effect) -> Gate {
        for layer in &self.layers {
            if let Inclusion::Holds = layer.prohibited.includes(&EffectSet::new().with(effect.clone()))
            {
                if let Some(rule) = layer
                    .prohibited
                    .iter()
                    .find(|candidate| candidate.permits(effect) == Containment::Contains)
                {
                    return Gate::Prohibited {
                        by: layer.source,
                        rule: rule.clone(),
                    };
                }
            }
        }
        let mut undecided: Option<PolicySource> = None;
        for layer in &self.layers {
            match layer.allowed.includes(&EffectSet::new().with(effect.clone())) {
                Inclusion::Holds => {}
                Inclusion::Undecided { .. } => {
                    if undecided.is_none() {
                        undecided = Some(layer.source);
                    }
                }
                Inclusion::Fails { .. } => return Gate::NotAllowed { by: layer.source },
            }
        }
        if let Some(source) = undecided {
            return Gate::Undecided { by: source };
        }
        for layer in &self.layers {
            if let Some(threshold) = layer.approval_required_from {
                if effect.class >= threshold {
                    return Gate::ApprovalRequired {
                        by: layer.source,
                        threshold,
                    };
                }
            }
        }
        Gate::Permitted
    }

    /// The reasons a statically permitted effect may still fail at run time. 23.14 lists six and
    /// this crate implements none of them, because every one needs a runtime this crate does not
    /// have. They are enumerated so a caller building a runtime has the vocabulary.
    pub fn runtime_deferral_reasons() -> &'static [RuntimeDeferral] {
        &[
            RuntimeDeferral::GrantExpired,
            RuntimeDeferral::BudgetExhausted,
            RuntimeDeferral::ResourceLabelChanged,
            RuntimeDeferral::SafetyMonitorPaused,
            RuntimeDeferral::Revoked,
            RuntimeDeferral::HumanApprovalRequired,
        ]
    }
}

/// Why a statically allowed effect may nonetheless be refused at run time (23.14).
///
/// Recorded, never evaluated here: this crate has no clock, no grants in flight and no monitors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDeferral {
    GrantExpired,
    BudgetExhausted,
    ResourceLabelChanged,
    SafetyMonitorPaused,
    Revoked,
    HumanApprovalRequired,
}

/// An effect-polymorphic molecule parameter, 23.14's `molecule data-auditor<R: readable-resource>`.
///
/// The parameter is a *bound*, and instantiating it substitutes a concrete scope into every effect
/// mentioning the variable. Instantiation with a resource outside the bound is refused; a molecule
/// that could widen its own effect scope by choosing its argument is not polymorphic, it is
/// unbounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectParameter {
    pub variable: String,
    pub bound: ResourcePattern,
    pub permitted_kinds: BTreeSet<EffectKind>,
}

impl EffectParameter {
    pub fn new(variable: impl Into<String>, bound: ResourcePattern) -> Self {
        EffectParameter {
            variable: variable.into(),
            bound,
            permitted_kinds: BTreeSet::new(),
        }
    }

    pub fn permitting(mut self, kind: EffectKind) -> Self {
        self.permitted_kinds.insert(kind);
        self
    }

    /// Substitute a concrete resource for the variable across a polymorphic effect set.
    pub fn instantiate(
        &self,
        template: &EffectSet,
        argument: &ResourcePattern,
    ) -> Result<EffectSet, EffectError> {
        if !self.bound.contains(argument) {
            return Err(EffectError::ArgumentOutsideBound {
                variable: self.variable.clone(),
                bound: self.bound.clone(),
                argument: argument.clone(),
            });
        }
        let marker = format!("${}", self.variable);
        let mut out = EffectSet::new();
        for effect in template.iter() {
            let scope = match &effect.scope {
                Scope::Resource(pattern) if pattern.as_str() == marker => {
                    if !self.permitted_kinds.is_empty()
                        && !self.permitted_kinds.contains(&effect.kind)
                    {
                        return Err(EffectError::KindOutsideParameter {
                            variable: self.variable.clone(),
                            kind: effect.kind,
                        });
                    }
                    Scope::Resource(argument.clone())
                }
                other => other.clone(),
            };
            out = out.with(Effect {
                kind: effect.kind,
                scope,
                class: effect.class,
            });
        }
        Ok(out)
    }
}

/// A static effect account for a composite: what each part contributes and what the whole may do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectAccount {
    pub per_part: BTreeMap<String, EffectSet>,
    pub total: EffectSet,
}

impl EffectAccount {
    /// Total is the union of parts. Composition never manufactures an effect, which is the effect
    /// analogue of 23.41's authority attenuation law and the reason `total` is computed here
    /// rather than supplied by a caller.
    pub fn of(parts: impl IntoIterator<Item = (String, EffectSet)>) -> Self {
        let per_part: BTreeMap<String, EffectSet> = parts.into_iter().collect();
        let total = per_part
            .values()
            .fold(EffectSet::new(), |acc, set| acc.union(set));
        EffectAccount { per_part, total }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EffectError {
    #[error("{0:?} is not in the 23.14 effect taxonomy")]
    UnknownEffectKind(String),

    #[error("resource pattern is empty")]
    EmptyResourcePattern,

    #[error("resource pattern {0:?} has an empty segment")]
    EmptySegment(String),

    #[error("resource pattern {0:?} uses ** somewhere other than the last segment")]
    InteriorRecursiveWildcard(String),

    #[error("{kind} declared at {declared:?} but its floor is {floor:?}")]
    ClassBelowFloor {
        kind: EffectKind,
        declared: Irreversibility,
        floor: Irreversibility,
    },

    #[error("argument {argument} is outside the bound {bound} of effect parameter {variable}")]
    ArgumentOutsideBound {
        variable: String,
        bound: ResourcePattern,
        argument: ResourcePattern,
    },

    #[error("effect parameter {variable} does not permit {kind}")]
    KindOutsideParameter { variable: String, kind: EffectKind },
}
