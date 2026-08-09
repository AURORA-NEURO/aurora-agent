//! Feature flags, and the rule that a flag which changes what a compile emits is a version.
//!
//! Implements the feature-flag half of blueprint 40.10 (Configuration, Secrets and Feature Flags):
//! its third invariant, *feature flags are versioned and auditable*, and its fourth declared
//! failure, *flag changes during a pinned run*.
//!
//! # One rule, borrowed from a neighbour and paid for in a different currency
//!
//! `bioprism-governance` decides whether a schema change is breaking, and the rule that outranks
//! every other rule there is that a change moving an artifact's canonical bytes is breaking however
//! innocent it looks field by field. A feature flag is the same problem wearing different clothes.
//! A flag that selects between two compiler paths does not toggle behaviour at runtime; it selects
//! which artifact the run produces, and two runs under different values of it are not the same
//! computation under a different setting — they are different computations.
//!
//! So this module has two constructors and no third. [`Flag::toggle`] fixes
//! [`Influence::Operational`]; [`Flag::variant`] fixes [`Influence::Emitted`] and *requires* an
//! artifact version string, because a variant with no version is a fork nobody can name. There is
//! no `Flag::new` taking both a shape and an influence, so a toggle that moves a digest is a shape
//! that cannot be built rather than a mistake to be caught. [`OpsError::ToggleMovesEmittedArtifact`]
//! exists for the one route that bypasses constructors — a declaration arriving through `serde` —
//! and [`FlagRegistry::declare`] is where that route is closed.
//!
//! `bioprism-governance` is not a dependency of this crate and the classification here is not a
//! second implementation of its classifier: [`classify`] compares two flag *declarations* and
//! answers one question, whether the change can move an emitted artifact. It has no notion of a
//! field, a schema, a document or a version bump. Where governance would be asked, it should be
//! asked directly.
//!
//! # Pinning is what makes a run reproducible, and it fails in two ways
//!
//! 40.10 lists `flag changes during pinned run` as a failure and stops there. There are two
//! failures inside it and they have different causes. The loud one is a flag whose value moved
//! mid-run. The quiet one is a flag the pin never contained: nothing changed during the run, and
//! the run is still irreproducible, because the next run is free to take the other branch.
//! [`PinnedRun`] raises [`OpsError::FlagChangedDuringPinnedRun`] and [`OpsError::FlagNotPinned`]
//! separately for that reason.
//!
//! # What is deliberately not implemented
//!
//! * **No rollout, no percentage, no targeting, no experiment assignment, no remote flag
//!   service.** A flag here has a value somebody decided; nothing samples, buckets or phones home.
//! * **No clock.** A decision carries a caller-supplied [`Epoch`], as everything else in this
//!   workspace does.
//! * **No version arithmetic.** [`FlagChangeClass`] is two states. Semantic-version bumps are
//!   `bioprism_governance::VersionBump`'s and are not reimplemented on a flag.
//! * **No storage.** [`DecisionLog`] is a `Vec`. 40.10's "flag registry" is a map in memory.

use crate::config::Influence;
use crate::error::{well_formed_name, OpsError};
use bioprism_ids::ContentHash;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

/// A flag name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FlagId(String);

impl FlagId {
    pub fn parse(value: impl Into<String>) -> Result<Self, OpsError> {
        Ok(FlagId(well_formed_name("flag id", &value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FlagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for FlagId {
    type Error = OpsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        FlagId::parse(value)
    }
}

impl From<FlagId> for String {
    fn from(value: FlagId) -> Self {
        value.0
    }
}

/// What a flag is, structurally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum FlagShape {
    /// A runtime switch over behaviour that leaves emitted bytes alone.
    Toggle,
    /// A selection between artifact variants. The version names the variant an artifact was
    /// produced under, so a reader of the artifact can find out which branch made it.
    Variant { artifact_version: String },
}

impl FlagShape {
    pub fn as_str(&self) -> &'static str {
        match self {
            FlagShape::Toggle => "toggle",
            FlagShape::Variant { .. } => "variant",
        }
    }
}

/// One declared flag.
///
/// Fields are private. The pairing of [`FlagShape`] with [`Influence`] is the invariant, and the
/// two constructors are the only way to establish it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flag {
    id: FlagId,
    shape: FlagShape,
    influence: Influence,
    default: bool,
    declared_in: String,
}

impl Flag {
    /// A runtime toggle. Operational by construction: there is no argument that would make it
    /// otherwise.
    pub fn toggle(id: FlagId, default: bool, declared_in: impl Into<String>) -> Self {
        Flag {
            id,
            shape: FlagShape::Toggle,
            influence: Influence::Operational,
            default,
            declared_in: declared_in.into(),
        }
    }

    /// An artifact variant. Emitted by construction, and the version is required.
    pub fn variant(
        id: FlagId,
        artifact_version: impl Into<String>,
        default: bool,
        declared_in: impl Into<String>,
    ) -> Result<Self, OpsError> {
        let artifact_version = well_formed_name("artifact version", &artifact_version.into())?;
        Ok(Flag {
            id,
            shape: FlagShape::Variant { artifact_version },
            influence: Influence::Emitted,
            default,
            declared_in: declared_in.into(),
        })
    }

    pub fn id(&self) -> &FlagId {
        &self.id
    }

    pub fn shape(&self) -> &FlagShape {
        &self.shape
    }

    pub fn influence(&self) -> Influence {
        self.influence
    }

    pub fn default(&self) -> bool {
        self.default
    }

    /// Where the declaration lives, so a reader can go and check it — the same field
    /// `bioprism_safety::Mitigation::DeclaredOnly` carries, for the same reason.
    pub fn declared_in(&self) -> &str {
        &self.declared_in
    }

    pub fn artifact_version(&self) -> Option<&str> {
        match &self.shape {
            FlagShape::Variant { artifact_version } => Some(artifact_version),
            FlagShape::Toggle => None,
        }
    }

    fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".to_string(), Value::String(self.id.0.clone()));
        map.insert(
            "shape".to_string(),
            Value::String(self.shape.as_str().to_string()),
        );
        map.insert(
            "influence".to_string(),
            Value::String(self.influence.as_str().to_string()),
        );
        map.insert("default".to_string(), Value::Bool(self.default));
        if let Some(version) = self.artifact_version() {
            map.insert(
                "artifact_version".to_string(),
                Value::String(version.to_string()),
            );
        }
        Value::Object(map)
    }
}

/// Whether a change to a flag declaration can move an emitted artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagChangeClass {
    /// No artifact that already exists was produced under a rule this change alters.
    Compatible,
    /// Some artifact that already exists was produced under a rule this change alters, so its
    /// provenance no longer describes it.
    Breaking,
}

impl FlagChangeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FlagChangeClass::Compatible => "compatible",
            FlagChangeClass::Breaking => "breaking",
        }
    }
}

impl fmt::Display for FlagChangeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A change to one flag declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum FlagChange {
    Added { flag: Flag },
    Removed { flag: Flag },
    Redeclared { before: Flag, after: Flag },
}

impl FlagChange {
    pub fn flag_id(&self) -> &FlagId {
        match self {
            FlagChange::Added { flag } | FlagChange::Removed { flag } => &flag.id,
            FlagChange::Redeclared { after, .. } => &after.id,
        }
    }
}

/// Whether a change can move the bytes of an artifact that already exists.
///
/// The asymmetries are the substance, and each is the flag-shaped image of one in
/// `bioprism_governance::affects_digest`:
///
/// - **Adding a variant** moves nothing. No existing artifact was produced under a branch that did
///   not exist, which is the same reason an added optional field with no default is digest-safe.
/// - **Removing a variant** always moves something, because artifacts stamped with its version
///   exist and nothing now describes what produced them.
/// - **Changing a variant's artifact version** always moves something, for the same reason.
/// - **Changing a variant's default** moves something: the branch a run takes when nobody chooses
///   changes, so two runs of the same command emit different bytes.
/// - **Changing a toggle's default** moves nothing, because a toggle is operational by
///   construction.
/// - **Promoting a toggle to a variant, or demoting a variant to a toggle**, always moves
///   something: in one direction previously operational runs become artifact-bearing, in the other
///   the version that stamped existing artifacts stops existing.
pub fn affects_emitted_artifact(change: &FlagChange) -> bool {
    match change {
        FlagChange::Added { .. } => false,
        FlagChange::Removed { flag } => flag.influence.is_emitted(),
        FlagChange::Redeclared { before, after } => {
            if before.influence != after.influence {
                return true;
            }
            if !after.influence.is_emitted() {
                return false;
            }
            before.artifact_version() != after.artifact_version() || before.default != after.default
        }
    }
}

/// One change, classified, with the reason in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagVerdict {
    pub change: FlagChange,
    pub class: FlagChangeClass,
    pub affects_emitted_artifact: bool,
    pub rationale: String,
}

/// Classifies one change. The class is the digest predicate and nothing else.
pub fn classify(change: FlagChange) -> FlagVerdict {
    let moves = affects_emitted_artifact(&change);
    let rationale = match (&change, moves) {
        (FlagChange::Added { .. }, _) => {
            "no artifact that already exists was produced under a branch that did not exist".into()
        }
        (FlagChange::Removed { flag }, true) => format!(
            "artifacts stamped {} exist and nothing would describe what produced them",
            flag.artifact_version().unwrap_or("(unversioned)")
        ),
        (FlagChange::Removed { .. }, false) => {
            "an operational toggle stamps nothing, so removing it describes no artifact".into()
        }
        (FlagChange::Redeclared { before, after }, true) => format!(
            "{} moved from {} to {}",
            after.id,
            describe(before),
            describe(after)
        ),
        (FlagChange::Redeclared { .. }, false) => {
            "an operational toggle's declaration does not enter any artifact".into()
        }
    };
    FlagVerdict {
        change,
        class: if moves {
            FlagChangeClass::Breaking
        } else {
            FlagChangeClass::Compatible
        },
        affects_emitted_artifact: moves,
        rationale,
    }
}

fn describe(flag: &Flag) -> String {
    match flag.artifact_version() {
        Some(version) => format!("{} {version} default={}", flag.shape.as_str(), flag.default),
        None => format!("{} default={}", flag.shape.as_str(), flag.default),
    }
}

impl FlagVerdict {
    /// Holds an author's claim against the derived class.
    ///
    /// Refuses rather than warns, for the reason `bioprism_governance::Classification::assert_class`
    /// refuses: a warning about a digest that moved is read once and then filtered.
    pub fn assert_class(&self, declared: FlagChangeClass) -> Result<(), OpsError> {
        if declared == self.class {
            return Ok(());
        }
        Err(OpsError::FlagChangeMisclassified {
            flag: self.change.flag_id().to_string(),
            declared: declared.to_string(),
            derived: self.class.to_string(),
            reason: self.rationale.clone(),
        })
    }
}

/// The declared flags of a program.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagRegistry {
    flags: BTreeMap<FlagId, Flag>,
}

impl FlagRegistry {
    pub fn new() -> Self {
        FlagRegistry::default()
    }

    /// Adds a declaration, refusing one whose shape and influence disagree.
    pub fn declare(&mut self, flag: Flag) -> Result<(), OpsError> {
        let consistent = match flag.shape {
            FlagShape::Toggle => flag.influence == Influence::Operational,
            FlagShape::Variant { .. } => flag.influence == Influence::Emitted,
        };
        if !consistent {
            return Err(OpsError::ToggleMovesEmittedArtifact {
                flag: flag.id.to_string(),
            });
        }
        self.flags.insert(flag.id.clone(), flag);
        Ok(())
    }

    pub fn with(mut self, flag: Flag) -> Result<Self, OpsError> {
        self.declare(flag)?;
        Ok(self)
    }

    pub fn get(&self, id: &FlagId) -> Option<&Flag> {
        self.flags.get(id)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// The flags that stamp artifacts, in id order.
    pub fn emitted(&self) -> Vec<&Flag> {
        self.flags
            .values()
            .filter(|flag| flag.influence.is_emitted())
            .collect()
    }

    /// Freezes the registry at chosen values.
    ///
    /// Every declared flag must be given a value. A pin that omits a flag is not a pin: the omitted
    /// flag takes whatever the next run's declaration says, which is exactly the case
    /// [`OpsError::FlagNotPinned`] describes.
    pub fn pin(&self, values: impl IntoIterator<Item = (FlagId, bool)>) -> Result<FlagPin, OpsError> {
        let chosen: BTreeMap<FlagId, bool> = values.into_iter().collect();
        let mut pinned = BTreeMap::new();
        for (id, flag) in &self.flags {
            let value = chosen.get(id).copied().unwrap_or(flag.default);
            pinned.insert(id.clone(), value);
        }
        for id in chosen.keys() {
            if !self.flags.contains_key(id) {
                return Err(OpsError::FlagNotPinned {
                    flag: id.to_string(),
                    pin: "registry".to_string(),
                });
            }
        }
        let digest = pin_digest(self, &pinned)?;
        Ok(FlagPin {
            values: pinned,
            digest,
        })
    }
}

fn pin_digest(registry: &FlagRegistry, values: &BTreeMap<FlagId, bool>) -> Result<ContentHash, OpsError> {
    let mut map = Map::new();
    for (id, value) in values {
        let mut entry = Map::new();
        entry.insert("value".to_string(), Value::Bool(*value));
        if let Some(flag) = registry.get(id) {
            entry.insert("declaration".to_string(), flag.to_json());
        }
        map.insert(id.as_str().to_string(), Value::Object(entry));
    }
    ContentHash::of_value(&Value::Object(map)).map_err(|error| OpsError::MalformedName {
        field: "flag pin".to_string(),
        value: error.to_string(),
    })
}

/// A frozen set of flag values, addressed by digest.
///
/// The digest covers the *declarations* as well as the values, so redeclaring a variant under a new
/// artifact version moves the pin even when every value is unchanged. A pin that covered only
/// values would let the same pin identify two different computations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagPin {
    values: BTreeMap<FlagId, bool>,
    digest: ContentHash,
}

impl FlagPin {
    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    pub fn get(&self, id: &FlagId) -> Option<bool> {
        self.values.get(id).copied()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// One consultation of one flag. 40.10's "feature decision events".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagDecision {
    pub flag: FlagId,
    pub value: bool,
    pub influence: Influence,
    pub pin: ContentHash,
    pub epoch: u64,
}

/// A run frozen at a pin, recording every flag it consults.
///
/// This is where 40.10's fourth failure becomes two failures. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedRun {
    pin: FlagPin,
    decisions: Vec<FlagDecision>,
}

impl PinnedRun {
    pub fn new(pin: FlagPin) -> Self {
        PinnedRun {
            pin,
            decisions: Vec::new(),
        }
    }

    /// Consults a flag, recording the decision.
    ///
    /// `observed` is what the caller's own configuration says the flag is, which is how a value
    /// that moved underneath a pinned run becomes visible rather than silently winning.
    pub fn decide(
        &mut self,
        registry: &FlagRegistry,
        id: &FlagId,
        observed: bool,
        epoch: Epoch,
    ) -> Result<bool, OpsError> {
        let pinned = self.pin.get(id).ok_or_else(|| OpsError::FlagNotPinned {
            flag: id.to_string(),
            pin: self.pin.digest.to_string(),
        })?;
        if pinned != observed {
            return Err(OpsError::FlagChangedDuringPinnedRun {
                flag: id.to_string(),
                pinned: pinned.to_string(),
                observed: observed.to_string(),
            });
        }
        let influence = registry
            .get(id)
            .map(Flag::influence)
            .unwrap_or(Influence::Operational);
        self.decisions.push(FlagDecision {
            flag: id.clone(),
            value: pinned,
            influence,
            pin: self.pin.digest.clone(),
            epoch: epoch.tick(),
        });
        Ok(pinned)
    }

    pub fn decisions(&self) -> &[FlagDecision] {
        &self.decisions
    }

    /// The decisions that stamped an artifact, which are the ones a result bundle must carry.
    pub fn emitting_decisions(&self) -> Vec<&FlagDecision> {
        self.decisions
            .iter()
            .filter(|decision| decision.influence.is_emitted())
            .collect()
    }

    pub fn pin(&self) -> &FlagPin {
        &self.pin
    }
}

/// An append-only record of decisions across runs. 40.10's flag registry, on the audit side.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLog {
    entries: Vec<FlagDecision>,
}

impl DecisionLog {
    pub fn new() -> Self {
        DecisionLog::default()
    }

    pub fn extend_from(&mut self, run: &PinnedRun) {
        self.entries.extend(run.decisions.iter().cloned());
    }

    pub fn entries(&self) -> &[FlagDecision] {
        &self.entries
    }

    /// Every distinct pin the log has seen, in first-appearance order.
    ///
    /// More than one pin over a set of results that are being compared means the comparison spans
    /// two computations, which is the question this log exists to make answerable.
    pub fn distinct_pins(&self) -> Vec<&ContentHash> {
        let mut seen: Vec<&ContentHash> = Vec::new();
        for entry in &self.entries {
            if !seen.iter().any(|pin| **pin == entry.pin) {
                seen.push(&entry.pin);
            }
        }
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> FlagId {
        FlagId::parse(name).expect("well-formed")
    }

    fn registry() -> FlagRegistry {
        FlagRegistry::new()
            .with(Flag::toggle(id("log.verbose"), false, "40.10"))
            .unwrap()
            .with(Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10").unwrap())
            .unwrap()
    }

    #[test]
    fn a_flag_that_moves_an_artifact_digest_is_a_version_not_a_toggle() {
        let variant = Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10")
            .expect("a variant carries a version");
        assert_eq!(variant.influence(), Influence::Emitted);
        assert_eq!(variant.artifact_version(), Some("ranking-v2"));

        let toggle = Flag::toggle(id("log.verbose"), false, "40.10");
        assert_eq!(toggle.influence(), Influence::Operational);
        assert_eq!(toggle.artifact_version(), None);
    }

    #[test]
    fn a_declaration_that_calls_itself_a_toggle_and_stamps_artifacts_is_refused() {
        let hostile: Flag = serde_json::from_value(serde_json::json!({
            "id": "compile.graph_ranking",
            "shape": { "shape": "toggle" },
            "influence": "emitted",
            "default": false,
            "declared_in": "hostile"
        }))
        .expect("serde bypasses constructors");
        let error = FlagRegistry::new().declare(hostile).unwrap_err();
        assert!(matches!(error, OpsError::ToggleMovesEmittedArtifact { .. }));
    }

    #[test]
    fn adding_a_variant_is_compatible_because_no_existing_artifact_took_that_branch() {
        let verdict = classify(FlagChange::Added {
            flag: Flag::variant(id("compile.new_path"), "path-v1", false, "40.10").unwrap(),
        });
        assert_eq!(verdict.class, FlagChangeClass::Compatible);
        assert!(verdict.assert_class(FlagChangeClass::Compatible).is_ok());
    }

    #[test]
    fn removing_a_variant_is_breaking_because_artifacts_stamped_with_it_still_exist() {
        let verdict = classify(FlagChange::Removed {
            flag: Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10").unwrap(),
        });
        assert_eq!(verdict.class, FlagChangeClass::Breaking);
    }

    #[test]
    fn removing_a_toggle_is_compatible_because_a_toggle_stamps_nothing() {
        let verdict = classify(FlagChange::Removed {
            flag: Flag::toggle(id("log.verbose"), false, "40.10"),
        });
        assert_eq!(verdict.class, FlagChangeClass::Compatible);
    }

    #[test]
    fn changing_a_variants_default_is_breaking_and_changing_a_toggles_default_is_not() {
        let variant_change = classify(FlagChange::Redeclared {
            before: Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10")
                .unwrap(),
            after: Flag::variant(id("compile.graph_ranking"), "ranking-v2", true, "40.10").unwrap(),
        });
        assert_eq!(variant_change.class, FlagChangeClass::Breaking);

        let toggle_change = classify(FlagChange::Redeclared {
            before: Flag::toggle(id("log.verbose"), false, "40.10"),
            after: Flag::toggle(id("log.verbose"), true, "40.10"),
        });
        assert_eq!(toggle_change.class, FlagChangeClass::Compatible);
    }

    #[test]
    fn demoting_a_variant_to_a_toggle_is_breaking_in_both_directions() {
        let promote = classify(FlagChange::Redeclared {
            before: Flag::toggle(id("compile.graph_ranking"), false, "40.10"),
            after: Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10").unwrap(),
        });
        let demote = classify(FlagChange::Redeclared {
            before: Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10")
                .unwrap(),
            after: Flag::toggle(id("compile.graph_ranking"), false, "40.10"),
        });
        assert_eq!(promote.class, FlagChangeClass::Breaking);
        assert_eq!(demote.class, FlagChangeClass::Breaking);
    }

    #[test]
    fn an_author_who_labels_a_breaking_flag_change_compatible_is_contradicted_with_evidence() {
        let verdict = classify(FlagChange::Redeclared {
            before: Flag::variant(id("compile.graph_ranking"), "ranking-v2", false, "40.10")
                .unwrap(),
            after: Flag::variant(id("compile.graph_ranking"), "ranking-v3", false, "40.10").unwrap(),
        });
        let error = verdict.assert_class(FlagChangeClass::Compatible).unwrap_err();
        match error {
            OpsError::FlagChangeMisclassified { derived, reason, .. } => {
                assert_eq!(derived, "breaking");
                assert!(reason.contains("ranking-v3"));
            }
            other => panic!("expected a misclassification, got {other}"),
        }
    }

    #[test]
    fn a_pin_covers_declarations_so_a_new_artifact_version_moves_it_with_no_value_change() {
        let before = registry().pin([(id("log.verbose"), false)]).unwrap();
        let after = FlagRegistry::new()
            .with(Flag::toggle(id("log.verbose"), false, "40.10"))
            .unwrap()
            .with(Flag::variant(id("compile.graph_ranking"), "ranking-v3", false, "40.10").unwrap())
            .unwrap()
            .pin([(id("log.verbose"), false)])
            .unwrap();
        assert_eq!(before.get(&id("compile.graph_ranking")), after.get(&id("compile.graph_ranking")));
        assert_ne!(before.digest(), after.digest());
    }

    #[test]
    fn a_pinned_run_refuses_a_flag_whose_value_moved_underneath_it() {
        let registry = registry();
        let pin = registry.pin([(id("compile.graph_ranking"), true)]).unwrap();
        let mut run = PinnedRun::new(pin);
        assert!(run
            .decide(&registry, &id("compile.graph_ranking"), true, Epoch::new(1))
            .is_ok());
        let error = run
            .decide(&registry, &id("compile.graph_ranking"), false, Epoch::new(2))
            .unwrap_err();
        assert!(matches!(error, OpsError::FlagChangedDuringPinnedRun { .. }));
    }

    #[test]
    fn a_flag_the_pin_never_contained_is_a_distinct_failure_from_one_that_moved() {
        let registry = registry();
        let pin = registry.pin([]).unwrap();
        let mut run = PinnedRun::new(pin);
        let error = run
            .decide(&registry, &id("compile.unknown_path"), true, Epoch::new(1))
            .unwrap_err();
        assert!(matches!(error, OpsError::FlagNotPinned { .. }));
    }

    #[test]
    fn pinning_a_flag_the_registry_does_not_declare_is_refused() {
        let error = registry().pin([(id("compile.ghost"), true)]).unwrap_err();
        assert!(matches!(error, OpsError::FlagNotPinned { .. }));
    }

    #[test]
    fn a_result_bundle_needs_only_the_decisions_that_stamped_an_artifact() {
        let registry = registry();
        let pin = registry.pin([]).unwrap();
        let mut run = PinnedRun::new(pin);
        run.decide(&registry, &id("log.verbose"), false, Epoch::new(1))
            .unwrap();
        run.decide(&registry, &id("compile.graph_ranking"), false, Epoch::new(1))
            .unwrap();
        assert_eq!(run.decisions().len(), 2);
        assert_eq!(run.emitting_decisions().len(), 1);
        assert_eq!(
            run.emitting_decisions()[0].flag.as_str(),
            "compile.graph_ranking"
        );
    }

    #[test]
    fn a_decision_log_reports_more_than_one_pin_when_results_span_two_computations() {
        let registry = registry();
        let mut log = DecisionLog::new();

        let mut first = PinnedRun::new(registry.pin([(id("compile.graph_ranking"), false)]).unwrap());
        first
            .decide(&registry, &id("compile.graph_ranking"), false, Epoch::new(1))
            .unwrap();
        log.extend_from(&first);

        let mut second = PinnedRun::new(registry.pin([(id("compile.graph_ranking"), true)]).unwrap());
        second
            .decide(&registry, &id("compile.graph_ranking"), true, Epoch::new(2))
            .unwrap();
        log.extend_from(&second);

        assert_eq!(log.distinct_pins().len(), 2);
    }
}
