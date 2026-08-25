//! Capability Molecules and virtual composite agents.
//!
//! Blueprint 23.11.
//!
//! A molecule is a versioned composition that exposes one stable agent interface. The mechanism is
//! [`crate::algebra::fuse`]; what this module adds is everything around it — the published card,
//! the six composition operations, the version-bump rule, and the invariant that makes the whole
//! idea acceptable.
//!
//! # The key invariant, as a type
//!
//! 23.11: "A composite agent may present one interface, but it must never erase internal
//! accountability. Every output remains traceable to roles, participants, artifacts, commitments,
//! and verifier evidence."
//!
//! [`Molecule::attribute`] is that sentence. It returns `Result`, and the failure case
//! [`MoleculeError::UnattributableOutput`] exists precisely so that a molecule which cannot say
//! which participant produced an output is a broken molecule rather than an opaque one.
//! [`Encapsulation`] is what a caller may inspect *without* authorisation, and it is a
//! deliberately short list; the internal transcript is not on it, and attribution is.
//!
//! # Self-described and verified are separate fields
//!
//! 23.11 says so in one line and it is easy to implement wrongly. [`MoleculeCard`] holds
//! `declared_capabilities` and `verified_capabilities` in separate fields of different types —
//! `String` and [`crate::reputation::Attestation`] — so a serialiser cannot merge them and a reader
//! cannot mistake one for the other.
//!
//! # Not implemented
//!
//! No registry, no packaging, no signing, no deployment bundle, no conformance runner. 23.11's
//! marketplace admission list is eight requirements and this crate can check exactly one of them
//! (explicit effects). The rest need artifacts that live outside a type system, and
//! [`admission_gaps`] names them rather than letting a caller believe a card that passes
//! [`MoleculeCard::validate`] is admissible.

use crate::algebra::{Composition, SubstitutionContext};
use crate::contract::{AgentContract, ComponentId, InterfaceType};
use crate::effect::{EffectSet, Inclusion};
use crate::reputation::{Attestation, EvidenceLayer, LogicalTime};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// A semantic version, compared field by field.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Version {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// One internal binding: a role name and the participant contract filling it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub role: String,
    pub participant: AgentContract,
}

/// 23.11's `guarantees { }` block.
///
/// `forbidden_effects` is checked against the composite's actual effect account rather than
/// trusted, because a guarantee nobody verifies is marketing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guarantees {
    pub provenance_complete: bool,
    pub private_reasoning_not_exported: bool,
    pub effect_ceiling: EffectSet,
    pub forbidden_effects: EffectSet,
    pub prohibited_claims: BTreeSet<String>,
}

impl Guarantees {
    pub fn new() -> Self {
        Guarantees::default()
    }

    pub fn with_effect_ceiling(mut self, ceiling: EffectSet) -> Self {
        self.effect_ceiling = ceiling;
        self
    }

    pub fn forbidding(mut self, effects: EffectSet) -> Self {
        self.forbidden_effects = effects;
        self
    }
}

/// A versioned composition exposing one interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Molecule {
    pub name: String,
    pub version: Version,
    pub bindings: Vec<Binding>,
    pub composition: Composition,
    pub exported: InterfaceType,
    pub guarantees: Guarantees,
    /// Which role each exported output came from. Populated at construction, because a molecule
    /// that cannot answer this is refused rather than published.
    attribution: BTreeMap<String, String>,
}

impl Molecule {
    /// Build a molecule from a fused composition.
    ///
    /// Refuses when a guarantee does not hold, when a binding has no role in the composition, or
    /// when an exported output field has no attribution.
    pub fn assemble(
        name: impl Into<String>,
        version: Version,
        bindings: Vec<Binding>,
        composition: Composition,
        exported: InterfaceType,
        guarantees: Guarantees,
        attribution: BTreeMap<String, String>,
    ) -> Result<Self, MoleculeError> {
        let name = name.into();
        let roles: BTreeSet<String> = bindings.iter().map(|b| b.role.clone()).collect();

        if !guarantees.effect_ceiling.is_empty() {
            if let Inclusion::Fails { witnesses } | Inclusion::Undecided { witnesses } = guarantees
                .effect_ceiling
                .includes(&composition.contract.effects)
            {
                return Err(MoleculeError::GuaranteeViolated {
                    molecule: name,
                    detail: format!("effects escape the declared ceiling: {witnesses:?}"),
                });
            }
        }
        let forbidden: Vec<crate::effect::Effect> = composition
            .contract
            .effects
            .iter()
            .filter(|effect| {
                matches!(
                    guarantees
                        .forbidden_effects
                        .includes(&EffectSet::new().with((*effect).clone())),
                    Inclusion::Holds
                )
            })
            .cloned()
            .collect();
        if !forbidden.is_empty() {
            return Err(MoleculeError::GuaranteeViolated {
                molecule: name,
                detail: format!("forbidden effects present: {forbidden:?}"),
            });
        }
        if guarantees.provenance_complete && !composition.contract.epistemic.provenance_complete {
            return Err(MoleculeError::GuaranteeViolated {
                molecule: name,
                detail: "provenance.complete is guaranteed and at least one participant does not \
                         provide it"
                    .to_string(),
            });
        }
        let prohibited: BTreeSet<String> = composition
            .contract
            .epistemic
            .claims_emitted
            .intersection(&guarantees.prohibited_claims)
            .cloned()
            .collect();
        if !prohibited.is_empty() {
            return Err(MoleculeError::GuaranteeViolated {
                molecule: name,
                detail: format!("prohibited claims emitted: {prohibited:?}"),
            });
        }

        for field in exported.fields.keys() {
            let role = attribution
                .get(field)
                .ok_or_else(|| MoleculeError::UnattributableOutput {
                    molecule: name.clone(),
                    output: field.clone(),
                })?;
            if !roles.contains(role) {
                return Err(MoleculeError::AttributionToUnboundRole {
                    molecule: name.clone(),
                    output: field.clone(),
                    role: role.clone(),
                });
            }
        }

        Ok(Molecule {
            name,
            version,
            bindings,
            composition,
            exported,
            guarantees,
            attribution,
        })
    }

    /// Which participant produced an exported output.
    pub fn attribute(&self, output: &str) -> Result<&Binding, MoleculeError> {
        let role = self
            .attribution
            .get(output)
            .ok_or_else(|| MoleculeError::UnattributableOutput {
                molecule: self.name.clone(),
                output: output.to_string(),
            })?;
        self.bindings
            .iter()
            .find(|b| &b.role == role)
            .ok_or_else(|| MoleculeError::AttributionToUnboundRole {
                molecule: self.name.clone(),
                output: output.to_string(),
                role: role.clone(),
            })
    }

    /// A content-derived identity over composition, policy and choreography, which is 23.46's
    /// "Molecule identity: hash of composition, policy, and choreography".
    pub fn identity(&self) -> Result<String, MoleculeError> {
        let payload = json!({
            "name": self.name,
            "version": self.version.to_string(),
            "roles": self.bindings.iter().map(|b| json!({
                "role": b.role,
                "participant": b.participant.id.as_str(),
            })).collect::<Vec<_>>(),
            "operator": serde_json::to_value(&self.composition.operator)
                .map_err(|e| MoleculeError::Encoding(e.to_string()))?,
            "guarantees": serde_json::to_value(&self.guarantees)
                .map_err(|e| MoleculeError::Encoding(e.to_string()))?,
        });
        Ok(ContentHash::of_value(&payload)
            .map_err(|e| MoleculeError::Encoding(e.to_string()))?
            .to_string())
    }

    /// What a caller may inspect without an audit authorisation (23.11's encapsulation list).
    pub fn encapsulation(&self) -> Encapsulation {
        Encapsulation {
            public_commitments: self
                .composition
                .contract
                .commitments
                .iter()
                .map(|c| c.id.clone())
                .collect(),
            requested_input: self.composition.contract.input.name.clone(),
            outputs: self.exported.fields.keys().cloned().collect(),
            attribution: self.attribution.clone(),
            declared_dissent: Vec::new(),
            unresolved_risk: Vec::new(),
        }
    }
}

/// What a molecule exposes without revealing its internal transcript.
///
/// `attribution` is on this list on purpose. Encapsulation hides *how*; it never hides *who*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Encapsulation {
    pub public_commitments: BTreeSet<String>,
    pub requested_input: String,
    pub outputs: BTreeSet<String>,
    pub attribution: BTreeMap<String, String>,
    pub declared_dissent: Vec<String>,
    pub unresolved_risk: Vec<String>,
}

/// The published card of 23.11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoleculeCard {
    pub name: String,
    pub version: Version,
    pub interfaces: BTreeSet<String>,
    pub required_effects: EffectSet,
    pub forbidden_effects: EffectSet,
    /// What the provider says about itself.
    pub declared_capabilities: BTreeSet<String>,
    /// What somebody else measured. A different type, so the two cannot be merged by accident.
    pub verified_capabilities: Vec<Attestation>,
    pub choreography_hash: Option<String>,
    pub policy_hash: Option<String>,
    /// Declared, not observed. Same caveat as [`crate::contract::ResourceEnvelope`].
    pub expected_latency_units: u64,
    pub expected_cost_minor: u64,
}

impl MoleculeCard {
    pub fn of(molecule: &Molecule) -> Result<Self, MoleculeError> {
        Ok(MoleculeCard {
            name: molecule.name.clone(),
            version: molecule.version,
            interfaces: [molecule.exported.name.clone()].into_iter().collect(),
            required_effects: molecule.composition.contract.effects.clone(),
            forbidden_effects: molecule.guarantees.forbidden_effects.clone(),
            declared_capabilities: BTreeSet::new(),
            verified_capabilities: Vec::new(),
            choreography_hash: match &molecule.composition.operator {
                crate::algebra::Operator::ChoreographedFusion {
                    choreography_digest,
                } => Some(choreography_digest.clone()),
                _ => None,
            },
            policy_hash: Some(molecule.identity()?),
            expected_latency_units: molecule.composition.contract.envelope.declared_latency_units,
            expected_cost_minor: molecule.composition.contract.envelope.declared_cost_minor,
        })
    }

    pub fn declaring(mut self, capability: impl Into<String>) -> Self {
        self.declared_capabilities.insert(capability.into());
        self
    }

    pub fn verified_by(mut self, attestation: Attestation) -> Self {
        self.verified_capabilities.push(attestation);
        self
    }

    /// Capabilities the card claims and nobody has verified.
    ///
    /// The gap between the two fields, which is the number a reader of a marketplace listing
    /// actually wants and which no listing shows.
    pub fn unverified_claims(&self, as_of: LogicalTime) -> BTreeSet<String> {
        let verified: BTreeSet<String> = self
            .verified_capabilities
            .iter()
            .filter(|a| {
                a.status(as_of) == crate::reputation::AttestationStatus::Valid
                    && a.layer.is_third_party()
            })
            .map(|a| a.claim.clone())
            .collect();
        self.declared_capabilities
            .difference(&verified)
            .cloned()
            .collect()
    }

    /// The one admission requirement of 23.11's marketplace list this crate can evaluate.
    pub fn validate(&self) -> Result<(), MoleculeError> {
        if self.required_effects.is_empty() {
            return Err(MoleculeError::CardIncomplete {
                name: self.name.clone(),
                field: "required_effects: a molecule that declares no effects has either not been \
                        analysed or is claiming to be pure"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// The seven admission requirements of 23.11 this crate cannot check, and why.
///
/// Returned as data rather than left in prose so a caller building an admission pipeline can
/// enumerate what it still has to do.
pub fn admission_gaps() -> &'static [(&'static str, &'static str)] {
    &[
        ("buildable package", "no build system in this crate"),
        ("signed provenance", "no cryptography in this crate"),
        (
            "conformance tests",
            "a conformance suite is an artifact, not a type",
        ),
        (
            "reproducible PRISM profile",
            "profiles are produced by evaluation runs this crate never performs",
        ),
        (
            "security review",
            "a human process with no machine-checkable output here",
        ),
        ("clear license", "metadata this crate does not model"),
        (
            "no misleading capability claims",
            "MoleculeCard::unverified_claims reports the gap; deciding whether a gap is misleading \
             is a judgement",
        ),
    ]
}

/// 23.11's six composition operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionOperation {
    Nest,
    Specialize,
    Adapt,
    Fuse,
    Split,
    Substitute,
}

/// An adapter's declared semantic loss (23.11's `Adapt`, 23.01's adapter principle).
///
/// There is no variant meaning "lossless and unexamined". A caller that has not looked must say
/// `unsupported` or `approximated`, not nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptationLoss {
    pub supported: BTreeSet<String>,
    pub approximated: BTreeSet<String>,
    pub unsupported: BTreeSet<String>,
}

impl AdaptationLoss {
    pub fn is_lossless(&self) -> bool {
        self.approximated.is_empty() && self.unsupported.is_empty()
    }
}

/// Bind additional policy or fix a participant without changing the interface (`Specialize`).
///
/// Refuses anything that changes the exported interface, because 23.11 defines specialisation as
/// "preserving the interface" and a specialisation that broadens it is a new molecule.
pub fn specialize(
    molecule: &Molecule,
    role: &str,
    fixed: AgentContract,
    context: &SubstitutionContext,
) -> Result<Molecule, MoleculeError> {
    let binding = molecule
        .bindings
        .iter()
        .find(|b| b.role == role)
        .ok_or_else(|| MoleculeError::NoSuchRole {
            molecule: molecule.name.clone(),
            role: role.to_string(),
        })?;
    let verdict = crate::algebra::substitutable(&fixed, &binding.participant, context);
    if !verdict.admitted() {
        return Err(MoleculeError::SpecializationRefused {
            molecule: molecule.name.clone(),
            role: role.to_string(),
            verdict: Box::new(verdict),
        });
    }
    let composition = crate::algebra::substitute(
        &molecule.composition,
        &binding.participant.id,
        &fixed,
        context,
    )
    .map_err(|e| MoleculeError::Composition(e.to_string()))?;
    let mut bindings = molecule.bindings.clone();
    for existing in &mut bindings {
        if existing.role == role {
            existing.participant = fixed.clone();
        }
    }
    Ok(Molecule {
        bindings,
        composition,
        version: Version::new(
            molecule.version.major,
            molecule.version.minor + 1,
            0,
        ),
        ..molecule.clone()
    })
}

/// Expose one internal role as an independent capability (`Split`).
///
/// Returns the participant's own contract. Splitting does not clone the molecule's authority onto
/// the fragment: the fragment carries what it always carried.
pub fn split(molecule: &Molecule, role: &str) -> Result<AgentContract, MoleculeError> {
    molecule
        .bindings
        .iter()
        .find(|b| b.role == role)
        .map(|b| b.participant.clone())
        .ok_or_else(|| MoleculeError::NoSuchRole {
            molecule: molecule.name.clone(),
            role: role.to_string(),
        })
}

/// Use a molecule as one participant inside another (`Nest`).
///
/// The nested molecule enters as its exported contract, not as its parts. That is the whole point
/// of the abstraction, and it is also why [`Molecule::attribute`] has to keep working through a
/// nesting: the outer molecule can name the inner one, and the inner one can name its role.
pub fn nest(molecule: &Molecule) -> AgentContract {
    AgentContract {
        id: ComponentId::new(format!("{}@{}", molecule.name, molecule.version)),
        output: molecule.exported.clone(),
        ..molecule.composition.contract.clone()
    }
}

/// The seven changes 23.11 says require a major version increment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakingChange {
    PublicTypeContract,
    EffectRequirements,
    SecurityPolicy,
    CommitmentSemantics,
    ResultInterpretation,
    CancellationBehavior,
    MinimumAssurance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionBump {
    Patch,
    Minor,
    Major,
}

/// Which bump two molecule versions require.
///
/// 23.11: "Internal model changes may remain a patch version only when PRISM regression gates pass
/// and the public behavioral contract is unchanged." The regression-gate half is not checkable
/// here, so [`required_bump`] answers the contract half and a caller that skips the gates and ships
/// a patch has skipped something this function never claimed to check.
pub fn required_bump(old: &Molecule, new: &Molecule) -> (VersionBump, BTreeSet<BreakingChange>) {
    let mut breaking = BTreeSet::new();
    if old.exported != new.exported {
        breaking.insert(BreakingChange::PublicTypeContract);
    }
    if old.composition.contract.effects != new.composition.contract.effects {
        breaking.insert(BreakingChange::EffectRequirements);
    }
    if old.guarantees != new.guarantees {
        breaking.insert(BreakingChange::SecurityPolicy);
    }
    if old.composition.contract.commitments != new.composition.contract.commitments {
        breaking.insert(BreakingChange::CommitmentSemantics);
    }
    if old.composition.contract.epistemic.claims_emitted
        != new.composition.contract.epistemic.claims_emitted
    {
        breaking.insert(BreakingChange::ResultInterpretation);
    }
    if old.composition.contract.failure.cancellable != new.composition.contract.failure.cancellable
    {
        breaking.insert(BreakingChange::CancellationBehavior);
    }
    if minimum_rung(old) != minimum_rung(new) {
        breaking.insert(BreakingChange::MinimumAssurance);
    }
    let bump = if !breaking.is_empty() {
        VersionBump::Major
    } else if old.bindings.len() != new.bindings.len() {
        VersionBump::Minor
    } else {
        VersionBump::Patch
    };
    (bump, breaking)
}

fn minimum_rung(molecule: &Molecule) -> EvidenceLayer {
    molecule
        .bindings
        .iter()
        .map(|b| b.participant.assurance.verified_at)
        .min()
        .unwrap_or(EvidenceLayer::SelfDeclared)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoleculeError {
    #[error("{molecule} exports {output} and cannot say which role produced it")]
    UnattributableOutput { molecule: String, output: String },

    #[error("{molecule} attributes {output} to role {role}, which nothing is bound to")]
    AttributionToUnboundRole {
        molecule: String,
        output: String,
        role: String,
    },

    #[error("{molecule} does not hold its guarantees: {detail}")]
    GuaranteeViolated { molecule: String, detail: String },

    #[error("{molecule} has no role {role}")]
    NoSuchRole { molecule: String, role: String },

    #[error("{molecule} cannot fix role {role}: {verdict:?}")]
    SpecializationRefused {
        molecule: String,
        role: String,
        verdict: Box<crate::algebra::SubstitutionVerdict>,
    },

    #[error("card for {name} is incomplete: {field}")]
    CardIncomplete { name: String, field: String },

    #[error("composition failed: {0}")]
    Composition(String),

    #[error("canonical encoding failed: {0}")]
    Encoding(String),
}
