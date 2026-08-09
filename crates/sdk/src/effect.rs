//! Declared effects and the determinism claim they have to be consistent with.
//!
//! Blueprint 40.16's second invariant is that "plugins declare effects, data classes, resources,
//! and determinism", and its fourth failure class is `undeclared effect`. 23.49 carries the same
//! idea as effect types and permissions; 43.35 states the rule this crate leans on hardest:
//! **permissions are not inferred from protocol connection.**
//!
//! # This module records, it does not enforce
//!
//! Nothing here prevents a plugin from doing anything. See [`crate::sandbox`] for the full
//! statement of what is and is not a boundary. What a declaration buys is a *comparison*: the host
//! can refuse a plugin whose declared effects exceed what it grants, and an operator can read the
//! registry and see that an oracle asked for network access before wondering why a score moved.
//!
//! # What determinism means here
//!
//! "Same declared inputs produce the same outputs." Reading a file that is part of the declared
//! input is therefore compatible with [`Determinism::Deterministic`]; reading the wall clock is
//! not, because the clock is an undeclared input. That is the same line `bioprism-adapter` draws
//! when it requires an adapter to be pure with respect to its source.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// One thing a plugin needs to reach outside itself.
///
/// Parameterised where the parameter is the interesting part — a plugin that needs
/// `network(pubmed.ncbi.nlm.nih.gov)` has told the operator something; one that needs "the
/// network" has not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    FilesystemRead {
        root: String,
    },
    FilesystemWrite {
        root: String,
    },
    Network {
        domain: String,
    },
    Subprocess {
        program: String,
    },
    /// Reads wall-clock time. Named separately from randomness because a plugin that timestamps
    /// its output is nondeterministic for a reason that looks harmless in review.
    Clock,
    Randomness,
    EnvironmentRead {
        name: String,
    },
    /// 11.12 lists secret references in the manifest. The reference is recorded; no secret is.
    SecretAccess {
        reference: String,
    },
}

impl Effect {
    pub fn class(&self) -> EffectClass {
        match self {
            Effect::FilesystemRead { .. } => EffectClass::FilesystemRead,
            Effect::FilesystemWrite { .. } => EffectClass::FilesystemWrite,
            Effect::Network { .. } => EffectClass::Network,
            Effect::Subprocess { .. } => EffectClass::Subprocess,
            Effect::Clock => EffectClass::Clock,
            Effect::Randomness => EffectClass::Randomness,
            Effect::EnvironmentRead { .. } => EffectClass::EnvironmentRead,
            Effect::SecretAccess { .. } => EffectClass::SecretAccess,
        }
    }

    /// Whether repeating the run with the same declared inputs can be expected to repeat the
    /// output.
    ///
    /// Filesystem access is reproducible in this sense and network access is not, which is not a
    /// statement about which is safer. It is a statement about what the *inputs* are: a declared
    /// file is one, a remote endpoint that may answer differently tomorrow is not.
    pub fn is_reproducible(&self) -> bool {
        match self {
            Effect::FilesystemRead { .. } | Effect::FilesystemWrite { .. } => true,
            Effect::Randomness => false,
            Effect::Network { .. }
            | Effect::Subprocess { .. }
            | Effect::Clock
            | Effect::EnvironmentRead { .. }
            | Effect::SecretAccess { .. } => false,
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::FilesystemRead { root } => write!(f, "filesystem-read({root})"),
            Effect::FilesystemWrite { root } => write!(f, "filesystem-write({root})"),
            Effect::Network { domain } => write!(f, "network({domain})"),
            Effect::Subprocess { program } => write!(f, "subprocess({program})"),
            Effect::Clock => f.write_str("clock"),
            Effect::Randomness => f.write_str("randomness"),
            Effect::EnvironmentRead { name } => write!(f, "environment-read({name})"),
            Effect::SecretAccess { reference } => write!(f, "secret-access({reference})"),
        }
    }
}

/// An effect with its parameter erased, for blanket grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    FilesystemRead,
    FilesystemWrite,
    Network,
    Subprocess,
    Clock,
    Randomness,
    EnvironmentRead,
    SecretAccess,
}

impl fmt::Display for EffectClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            EffectClass::FilesystemRead => "filesystem-read",
            EffectClass::FilesystemWrite => "filesystem-write",
            EffectClass::Network => "network",
            EffectClass::Subprocess => "subprocess",
            EffectClass::Clock => "clock",
            EffectClass::Randomness => "randomness",
            EffectClass::EnvironmentRead => "environment-read",
            EffectClass::SecretAccess => "secret-access",
        };
        f.write_str(text)
    }
}

/// How reproducible a plugin claims to be.
///
/// Three levels rather than a boolean because the middle one is the common honest answer. A
/// mutation operator that draws from a seeded generator is reproducible given its seed and not
/// otherwise, and collapsing it into either neighbour loses the condition under which replay works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    /// Same declared inputs, same outputs, no further conditions.
    Deterministic,
    /// Same declared inputs *and the recorded seed*, same outputs.
    SeedDeterministic,
    /// No reproducibility claim. Not a defect; an admission.
    Nondeterministic,
}

impl Determinism {
    /// Whether this claim survives the plugin also declaring `effect`.
    ///
    /// [`Determinism::SeedDeterministic`] tolerates [`Effect::Randomness`] and nothing else that
    /// is irreproducible: a seed explains a generator, it does not explain a clock.
    pub fn tolerates(self, effect: &Effect) -> bool {
        match self {
            Determinism::Nondeterministic => true,
            Determinism::SeedDeterministic => {
                effect.is_reproducible() || matches!(effect, Effect::Randomness)
            }
            Determinism::Deterministic => effect.is_reproducible(),
        }
    }
}

impl fmt::Display for Determinism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Determinism::Deterministic => "deterministic",
            Determinism::SeedDeterministic => "seed-deterministic",
            Determinism::Nondeterministic => "nondeterministic",
        };
        f.write_str(text)
    }
}

/// What a host is willing to grant.
///
/// Two granularities, and the coarse one is labelled coarse. An exact grant names the parameter;
/// a class grant is a blanket permission for every parameter of that class, which is what a
/// development host will realistically use and what a production host should not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPolicy {
    pub granted: BTreeSet<Effect>,
    pub granted_classes: BTreeSet<EffectClass>,
}

impl EffectPolicy {
    /// Grants nothing. The default a host should have to move away from deliberately.
    pub fn deny_all() -> Self {
        EffectPolicy::default()
    }

    pub fn granting(mut self, effect: Effect) -> Self {
        self.granted.insert(effect);
        self
    }

    pub fn granting_class(mut self, class: EffectClass) -> Self {
        self.granted_classes.insert(class);
        self
    }

    pub fn permits(&self, effect: &Effect) -> bool {
        self.granted_classes.contains(&effect.class()) || self.granted.contains(effect)
    }

    /// Every declared effect this policy does not grant, in a stable order.
    pub fn refusals<'a, I>(&self, declared: I) -> Vec<&'a Effect>
    where
        I: IntoIterator<Item = &'a Effect>,
    {
        declared
            .into_iter()
            .filter(|effect| !self.permits(effect))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deterministic_claim_does_not_survive_a_clock_effect() {
        assert!(!Determinism::Deterministic.tolerates(&Effect::Clock));
        assert!(!Determinism::SeedDeterministic.tolerates(&Effect::Clock));
        assert!(Determinism::Nondeterministic.tolerates(&Effect::Clock));
    }

    #[test]
    fn a_seed_explains_randomness_but_not_the_network() {
        assert!(Determinism::SeedDeterministic.tolerates(&Effect::Randomness));
        assert!(!Determinism::SeedDeterministic.tolerates(&Effect::Network {
            domain: "example.org".into()
        }));
        assert!(!Determinism::Deterministic.tolerates(&Effect::Randomness));
    }

    #[test]
    fn reading_a_declared_file_is_compatible_with_determinism() {
        assert!(
            Determinism::Deterministic.tolerates(&Effect::FilesystemRead {
                root: "/data/cohort".into()
            })
        );
    }

    #[test]
    fn an_empty_policy_grants_nothing_and_names_every_refusal() {
        let declared = vec![
            Effect::Clock,
            Effect::Network {
                domain: "a.example".into(),
            },
        ];
        let refused = EffectPolicy::deny_all().refusals(&declared);
        assert_eq!(refused.len(), 2);
    }

    #[test]
    fn an_exact_grant_does_not_extend_to_a_sibling_parameter() {
        let policy = EffectPolicy::deny_all().granting(Effect::Network {
            domain: "a.example".into(),
        });
        assert!(policy.permits(&Effect::Network {
            domain: "a.example".into()
        }));
        assert!(!policy.permits(&Effect::Network {
            domain: "b.example".into()
        }));
    }

    #[test]
    fn a_class_grant_is_a_blanket_grant_for_that_class_only() {
        let policy = EffectPolicy::deny_all().granting_class(EffectClass::Network);
        assert!(policy.permits(&Effect::Network {
            domain: "anything.example".into()
        }));
        assert!(!policy.permits(&Effect::Subprocess {
            program: "sh".into()
        }));
    }
}
