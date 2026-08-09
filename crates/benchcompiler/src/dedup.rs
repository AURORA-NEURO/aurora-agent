//! Deduplication, contamination and leakage.
//!
//! Blueprint 06.14. Two distinct jobs that share a mechanism: stopping scale from being inflated by
//! duplicates, and stopping an agent from obtaining the answer through a channel that is not the
//! capability under test.
//!
//! ## Renaming is not a new benchmark
//!
//! `bioprism_mutation::lineage::content_digest` learned this the hard way and its comment says why:
//! it hashes facts, factors and events but **not** `world_id` or `description`, because a generator
//! that could defeat deduplication by renaming would inflate the instance count for free. The same
//! rule is applied here at two levels:
//!
//! - [`content_fingerprint`] hashes what an instance *tests* — world digest, query digest, accepted
//!   verdicts, required witnesses — and never its id, title or description.
//! - [`structural_fingerprint`] goes further and defeats renaming *inside* the content. Every
//!   declared identifier is replaced by its rank in canonical traversal order before hashing, so
//!   `SAMPLE_A/SAMPLE_B` and `specimen_q/specimen_r` collapse to the same fingerprint when they are
//!   used the same way. What survives is the shape of the instance, which is what a duplicate
//!   shares with its original.
//!
//! Identifiers must be *declared* by the caller rather than guessed. A heuristic that decided which
//! strings were meaningless would eventually decide a semantically loaded one was, and quietly
//! merge two instances that test different things — a false duplicate is worse than a missed one,
//! because it deletes evidence.
//!
//! ## Contamination
//!
//! Exposure is recorded, never inferred: this crate has no clock and no network, so it cannot
//! discover that an answer became searchable. It can enforce what follows from a record, and it can
//! enforce leak-probe outcomes — 06.14's "run agents with no task content, metadata-only access,
//! filename inspection, and grader access". A probe that *solves* the instance proves the answer is
//! reachable without the capability, and that is a fact about the instance rather than about the
//! agent that found it.
//!
//! Holdout assignment is a deterministic function of the content fingerprint, not a random draw:
//! the same instance lands in the same panel on every machine, which is what makes a private
//! holdout reproducible by a second site (35.08 gate 8).
//!
//! ## What is deliberately not implemented
//!
//! No embedding or entailment similarity. 06.14 lists "semantic similarity" and "behavioral
//! fingerprints across reference agents" as dedup layers; both need a model this offline workspace
//! does not have, and a token-overlap score presented as semantic similarity would be a weaker
//! signal wearing a stronger name. Behavioural fingerprints are supported *if the caller supplies
//! them* — [`Instance::behavioural_signature`] participates in oracle-equivalence grouping — but
//! nothing here produces one.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A benchmark instance as the deduplicator sees it.
///
/// The `instance_id` is carried for reporting and takes no part in any fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub instance_id: String,
    /// The instance's semantic content: the world, the query, whatever the caller considers
    /// load-bearing. Labels inside it are neutralised by [`structural_fingerprint`].
    pub content: Value,
    /// Verdicts the instance accepts.
    pub acceptable_verdicts: BTreeSet<String>,
    pub required_witnesses: BTreeSet<String>,
    /// Strings in `content` that are names rather than meanings.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub identifiers: BTreeSet<String>,
    /// An optional behavioural fingerprint the caller measured across reference agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavioural_signature: Option<String>,
}

impl Instance {
    pub fn new(instance_id: impl Into<String>, content: Value) -> Self {
        Instance {
            instance_id: instance_id.into(),
            content,
            acceptable_verdicts: BTreeSet::new(),
            required_witnesses: BTreeSet::new(),
            identifiers: BTreeSet::new(),
            behavioural_signature: None,
        }
    }

    pub fn accepting(mut self, verdict: impl Into<String>) -> Self {
        self.acceptable_verdicts.insert(verdict.into());
        self
    }

    pub fn requiring_witness(mut self, witness: impl Into<String>) -> Self {
        self.required_witnesses.insert(witness.into());
        self
    }

    /// Declares a string as a name with no semantic content.
    pub fn naming(mut self, identifier: impl Into<String>) -> Self {
        self.identifiers.insert(identifier.into());
        self
    }

    pub fn behaving_like(mut self, signature: impl Into<String>) -> Self {
        self.behavioural_signature = Some(signature.into());
        self
    }

    /// The oracle-equivalence key: what the instance tests, regardless of how it is worded.
    pub fn oracle_signature(&self) -> String {
        format!(
            "{}|{}|{}",
            self.acceptable_verdicts
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            self.required_witnesses
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            self.behavioural_signature.as_deref().unwrap_or("")
        )
    }
}

/// Hashes an instance's content and contract, excluding every label.
pub fn content_fingerprint(instance: &Instance) -> String {
    let body = serde_json::json!({
        "content": instance.content,
        "acceptable_verdicts": instance.acceptable_verdicts,
        "required_witnesses": instance.required_witnesses,
    });
    ContentHash::of_value(&body)
        .expect("instance content is finite JSON")
        .as_str()
        .to_string()
}

/// Rewrites every declared identifier to its rank in canonical traversal order.
///
/// The rank is assigned by first appearance in a deterministic walk of the document — object keys
/// in sorted order, array elements in position — so it depends on where a name is used and not on
/// what it is called.
fn alpha_rename(value: &Value, identifiers: &BTreeSet<String>) -> Value {
    let mut ranks: BTreeMap<String, usize> = BTreeMap::new();
    fn walk(value: &Value, identifiers: &BTreeSet<String>, ranks: &mut BTreeMap<String, usize>) {
        match value {
            Value::String(text) => {
                if identifiers.contains(text) && !ranks.contains_key(text) {
                    let next = ranks.len();
                    ranks.insert(text.clone(), next);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, identifiers, ranks);
                }
            }
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    if identifiers.contains(key.as_str()) && !ranks.contains_key(key.as_str()) {
                        let next = ranks.len();
                        ranks.insert(key.clone(), next);
                    }
                    walk(&map[key], identifiers, ranks);
                }
            }
            _ => {}
        }
    }
    walk(value, identifiers, &mut ranks);

    fn rewrite(value: &Value, ranks: &BTreeMap<String, usize>) -> Value {
        match value {
            Value::String(text) => match ranks.get(text) {
                Some(rank) => Value::String(format!("#{rank}")),
                None => value.clone(),
            },
            Value::Array(items) => {
                Value::Array(items.iter().map(|item| rewrite(item, ranks)).collect())
            }
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (key, entry) in map {
                    let key = match ranks.get(key) {
                        Some(rank) => format!("#{rank}"),
                        None => key.clone(),
                    };
                    out.insert(key, rewrite(entry, ranks));
                }
                Value::Object(out)
            }
            _ => value.clone(),
        }
    }
    rewrite(value, &ranks)
}

/// Hashes an instance's *shape*, with every declared identifier neutralised.
///
/// Two instances with the same structural fingerprint differ only in names, and renaming is not a
/// contribution to benchmark scale.
pub fn structural_fingerprint(instance: &Instance) -> String {
    let renamed = alpha_rename(&instance.content, &instance.identifiers);
    let body = serde_json::json!({
        "content": renamed,
        "acceptable_verdicts": instance.acceptable_verdicts,
        "required_witnesses": instance.required_witnesses,
    });
    ContentHash::of_value(&body)
        .expect("renamed content is finite JSON")
        .as_str()
        .to_string()
}

/// Which dedup layer merged a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateLayer {
    /// Byte-identical content and contract.
    Content,
    /// Identical once names are neutralised. The rename-proof layer.
    Structural,
    /// Different content, but the same verdict, witness and behavioural contract. These may be
    /// legitimately distinct instances; the layer flags them for review rather than deleting them.
    OracleEquivalent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub layer: DuplicateLayer,
    pub fingerprint: String,
    /// The instance kept, chosen as the lexicographically first id so the choice is reproducible.
    pub representative: String,
    pub duplicates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupReport {
    pub examined: usize,
    /// Instances that survive after content and structural duplicates are removed.
    pub distinct: usize,
    pub groups: Vec<DuplicateGroup>,
    pub caveat: String,
}

impl DedupReport {
    /// Ids removed by the content and structural layers.
    ///
    /// Oracle-equivalent groups are excluded: they are a review signal, and dropping them
    /// automatically would delete instances that legitimately test the same contract from different
    /// states, which 06.09's contrastive pairs depend on.
    pub fn removed(&self) -> Vec<String> {
        let mut removed: BTreeSet<String> = BTreeSet::new();
        for group in &self.groups {
            if group.layer == DuplicateLayer::OracleEquivalent {
                continue;
            }
            for id in &group.duplicates {
                removed.insert(id.clone());
            }
        }
        removed.into_iter().collect()
    }
}

/// Groups instances by every dedup layer this crate can compute.
pub fn deduplicate(instances: &[Instance]) -> DedupReport {
    let mut groups = Vec::new();

    let mut by_content: BTreeMap<String, Vec<&Instance>> = BTreeMap::new();
    for instance in instances {
        by_content
            .entry(content_fingerprint(instance))
            .or_default()
            .push(instance);
    }
    let mut content_duplicates: BTreeSet<&str> = BTreeSet::new();
    for (fingerprint, members) in &by_content {
        if members.len() < 2 {
            continue;
        }
        let mut ids: Vec<&str> = members.iter().map(|i| i.instance_id.as_str()).collect();
        ids.sort_unstable();
        for id in &ids[1..] {
            content_duplicates.insert(id);
        }
        groups.push(DuplicateGroup {
            layer: DuplicateLayer::Content,
            fingerprint: fingerprint.clone(),
            representative: ids[0].to_string(),
            duplicates: ids[1..].iter().map(|id| (*id).to_string()).collect(),
        });
    }

    let mut by_structure: BTreeMap<String, Vec<&Instance>> = BTreeMap::new();
    for instance in instances {
        if content_duplicates.contains(instance.instance_id.as_str()) {
            continue;
        }
        by_structure
            .entry(structural_fingerprint(instance))
            .or_default()
            .push(instance);
    }
    let mut structural_duplicates: BTreeSet<&str> = BTreeSet::new();
    for (fingerprint, members) in &by_structure {
        if members.len() < 2 {
            continue;
        }
        let mut ids: Vec<&str> = members.iter().map(|i| i.instance_id.as_str()).collect();
        ids.sort_unstable();
        for id in &ids[1..] {
            structural_duplicates.insert(id);
        }
        groups.push(DuplicateGroup {
            layer: DuplicateLayer::Structural,
            fingerprint: fingerprint.clone(),
            representative: ids[0].to_string(),
            duplicates: ids[1..].iter().map(|id| (*id).to_string()).collect(),
        });
    }

    let mut by_oracle: BTreeMap<String, Vec<&Instance>> = BTreeMap::new();
    for instance in instances {
        if content_duplicates.contains(instance.instance_id.as_str())
            || structural_duplicates.contains(instance.instance_id.as_str())
        {
            continue;
        }
        by_oracle
            .entry(instance.oracle_signature())
            .or_default()
            .push(instance);
    }
    for (signature, members) in &by_oracle {
        if members.len() < 2 {
            continue;
        }
        let mut ids: Vec<&str> = members.iter().map(|i| i.instance_id.as_str()).collect();
        ids.sort_unstable();
        groups.push(DuplicateGroup {
            layer: DuplicateLayer::OracleEquivalent,
            fingerprint: signature.clone(),
            representative: ids[0].to_string(),
            duplicates: ids[1..].iter().map(|id| (*id).to_string()).collect(),
        });
    }

    groups.sort_by(|a, b| {
        a.layer
            .cmp(&b.layer)
            .then_with(|| a.representative.cmp(&b.representative))
    });

    DedupReport {
        examined: instances.len(),
        distinct: instances.len() - content_duplicates.len() - structural_duplicates.len(),
        groups,
        caveat: "Content and structural layers are exact after canonical renaming. No semantic \
                 similarity is computed; two instances that mean the same thing in different words \
                 will not be caught here."
            .to_string(),
    }
}

/// A channel through which an agent might obtain the answer without doing the task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakChannel {
    /// The agent was given no task content at all.
    NoTaskContent,
    /// Metadata only: tags, ids, pack membership.
    MetadataOnly,
    /// Filenames only.
    FilenameOnly,
    /// The agent could read the grader.
    GraderAccess,
    /// A web search returned the answer.
    PublicSearch,
}

impl LeakChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            LeakChannel::NoTaskContent => "no_task_content",
            LeakChannel::MetadataOnly => "metadata_only",
            LeakChannel::FilenameOnly => "filename_only",
            LeakChannel::GraderAccess => "grader_access",
            LeakChannel::PublicSearch => "public_search",
        }
    }
}

/// One probe result. `solved` is the finding; everything else is context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakProbe {
    pub channel: LeakChannel,
    pub solved: bool,
    pub note: String,
}

impl LeakProbe {
    pub fn new(channel: LeakChannel, solved: bool, note: impl Into<String>) -> Self {
        LeakProbe {
            channel,
            solved,
            note: note.into(),
        }
    }
}

/// What is on the record about an instance's public exposure.
///
/// Every field is asserted by the caller. Nothing here is discovered, and an empty ledger means
/// "nobody checked", not "not exposed" — which is why [`ContaminationRisk::Unassessed`] exists.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExposureLedger {
    pub published: bool,
    /// Where the instance or its answer appears.
    pub repositories: Vec<String>,
    /// Whether the answer is known to be retrievable by search.
    pub answer_searchable: bool,
    /// When it was published, as the caller recorded it. This crate has no clock and does not
    /// parse or compare these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_published: Option<String>,
    /// Whether anyone has actually looked. Distinguishes a clean ledger from an empty one.
    pub assessed: bool,
}

/// Which panel an instance belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "holdout", rename_all = "snake_case")]
pub enum Holdout {
    Public,
    Private,
    Rotating { panel: usize },
}

/// Assigns an instance to a panel deterministically from its content fingerprint.
///
/// A hash bucket rather than a draw: the same instance lands in the same panel on every machine and
/// in every process, which is what lets a second site reproduce a private-holdout result without
/// being handed the split.
pub fn assign_holdout(instance: &Instance, private_share: u8, rotating_panels: usize) -> Holdout {
    let fingerprint = content_fingerprint(instance);
    let bucket = fingerprint
        .as_bytes()
        .iter()
        .rev()
        .take(8)
        .fold(0u32, |accumulator, byte| {
            accumulator.wrapping_mul(31).wrapping_add(*byte as u32)
        })
        % 100;
    if bucket < private_share.min(100) as u32 {
        return Holdout::Private;
    }
    if rotating_panels > 0 && bucket % 2 == 0 {
        return Holdout::Rotating {
            panel: (bucket as usize) % rotating_panels,
        };
    }
    Holdout::Public
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "risk", rename_all = "snake_case")]
pub enum ContaminationRisk {
    /// A probe solved the instance without the capability under test. Blocking.
    LeaksThroughChannel { channel: LeakChannel, note: String },
    /// The answer is on the record as publicly retrievable.
    AnswerSearchable { repositories: Vec<String> },
    /// Published, with no probe run against it.
    PublishedAndUnprobed,
    /// Nobody assessed exposure. Not the same as "clean".
    Unassessed,
    Clean,
}

impl ContaminationRisk {
    /// Whether the instance may appear in a primary count.
    pub fn admissible(&self) -> bool {
        matches!(self, ContaminationRisk::Clean)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContaminationReport {
    pub instance_id: String,
    pub risk: ContaminationRisk,
    pub probes_run: usize,
}

/// Assesses one instance against its ledger and probe results.
///
/// The order of checks is the order of severity: a demonstrated leak outranks a recorded exposure,
/// which outranks the absence of evidence. Nothing is downgraded because a later check passed.
pub fn assess_contamination(
    instance: &Instance,
    ledger: &ExposureLedger,
    probes: &[LeakProbe],
) -> ContaminationReport {
    let risk = if let Some(probe) = probes.iter().find(|probe| probe.solved) {
        ContaminationRisk::LeaksThroughChannel {
            channel: probe.channel,
            note: probe.note.clone(),
        }
    } else if ledger.answer_searchable {
        ContaminationRisk::AnswerSearchable {
            repositories: ledger.repositories.clone(),
        }
    } else if !ledger.assessed {
        ContaminationRisk::Unassessed
    } else if ledger.published && probes.is_empty() {
        ContaminationRisk::PublishedAndUnprobed
    } else {
        ContaminationRisk::Clean
    };

    ContaminationReport {
        instance_id: instance.instance_id.clone(),
        risk,
        probes_run: probes.len(),
    }
}
