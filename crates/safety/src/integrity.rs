//! Benchmark poisoning and reward hacking, reported as witnesses.
//!
//! Implements blueprint 13.14 (evaluator attacks, benchmark poisoning and reward hacking). This is
//! the attack class an evaluation platform is most exposed to, because unlike sandbox escape it
//! needs no exploit: the agent does exactly what it was asked, the grader says yes, and the number
//! is wrong.
//!
//! # Witnesses, not scores
//!
//! Every check here returns [`IntegrityWitness`] values — "the reference answer for task T-3
//! appears verbatim in its own context segment `ctx-1`" — following
//! `bioprism_section::verdict::LeakageWitness`. A witness is checkable by hand and either true or
//! false. A contamination *score* would be a number nobody can act on, and would let a pack with
//! one fatal leak average out against a hundred clean tasks.
//!
//! # Underdetermined is a verdict
//!
//! [`IntegrityStatus::Underdetermined`] exists because most of these checks need evidence the
//! caller has to supply — probe outputs, operator labels, a declared asset placement — and running
//! a check with none of it must not return "clean". [`IntegrityVerdict::underdetermined`] is the
//! constructor for that, [`IntegrityReport::is_clean`] refuses to be true while any verdict is
//! underdetermined, and [`check_oracle_degeneracy`] with an empty probe set returns it rather than
//! [`IntegrityStatus::NoWitness`].
//!
//! # What the checks can and cannot see
//!
//! * [`check_answer_containment`] finds **exact** containment of a reference answer in the task's
//!   own context. A paraphrase, a translation, a rounded number or an answer split across two
//!   segments is not found. The witness list is a lower bound and reporting it as a clean bill of
//!   health would be wrong.
//! * [`check_oracle_degeneracy`] finds a constant output the oracle accepts across several tasks.
//!   The caller runs the oracle; this crate executes nothing. An oracle satisfiable by a
//!   *task-dependent* shortcut is invisible to it.
//! * [`check_grader_surface_feature`] finds a contradiction inside operator-supplied labels: an
//!   output the grader accepted that the operator says does not solve the task. It does not know
//!   whether an output solves a task, and cannot.
//! * [`check_identifier_label_leakage`] finds a prefix of the identifier that determines the label.
//!   Other encodings — suffixes, checksums, ordering, length — are not searched for.
//!
//! # What is deliberately not implemented
//!
//! No execution of anything: no oracle is run, no grader is invoked, no agent output is produced.
//! No statistical contamination estimate, no n-gram overlap, no embedding similarity — those are
//! the score-shaped answers this module exists to avoid. No canary generation (13.14's "benchmark
//! canaries"), because a canary is only meaningful once someone can observe whether it came back,
//! and nothing here observes anything.

use crate::boundary::{BoundaryModel, TrustZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A concrete, checkable integrity failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IntegrityWitness {
    /// The benchmark ships its own answer in the context it hands the agent.
    AnswerInOwnContext {
        task: String,
        answer: String,
        segment: String,
    },
    /// One constant output satisfies the oracle across several distinct tasks, so the oracle can be
    /// passed without solving anything.
    ConstantOutputSatisfiesOracle {
        oracle: String,
        output: String,
        tasks: Vec<String>,
    },
    /// A prefix of the instance identifier determines the label.
    IdentifierPredictsLabel {
        field: String,
        prefix_length: usize,
        mapping: BTreeMap<String, String>,
    },
    /// The grader accepted an output the operator says does not solve the task.
    GraderAcceptedNonSolution {
        grader: String,
        feature: String,
        outputs: Vec<String>,
    },
    /// The oracle checks only that a path exists, and the agent can create that path.
    ExpectedPathIsAgentCreatable { task: String, path: String },
    /// A hidden asset is declared in a zone the agent can influence or read.
    HiddenAssetReachableFromAgent {
        asset: String,
        zone: String,
        path: Vec<String>,
    },
    /// The party that authored a task also submitted the result being compared on it.
    AuthorIsAlsoSubmitter { pack: String, principal: String },
}

impl IntegrityWitness {
    pub fn kind(&self) -> &'static str {
        match self {
            IntegrityWitness::AnswerInOwnContext { .. } => "answer_in_own_context",
            IntegrityWitness::ConstantOutputSatisfiesOracle { .. } => {
                "constant_output_satisfies_oracle"
            }
            IntegrityWitness::IdentifierPredictsLabel { .. } => "identifier_predicts_label",
            IntegrityWitness::GraderAcceptedNonSolution { .. } => "grader_accepted_non_solution",
            IntegrityWitness::ExpectedPathIsAgentCreatable { .. } => {
                "expected_path_is_agent_creatable"
            }
            IntegrityWitness::HiddenAssetReachableFromAgent { .. } => {
                "hidden_asset_reachable_from_agent"
            }
            IntegrityWitness::AuthorIsAlsoSubmitter { .. } => "author_is_also_submitter",
        }
    }

    /// Whether this witness is against the benchmark's authors or against the benchmarked agent.
    ///
    /// 13.14 separates "Agent attacks" from "Contributor attacks" and they have different owners:
    /// one is fixed by rebuilding the pack, the other by invalidating a result.
    pub fn blames(&self) -> Blame {
        match self {
            IntegrityWitness::AnswerInOwnContext { .. }
            | IntegrityWitness::ConstantOutputSatisfiesOracle { .. }
            | IntegrityWitness::IdentifierPredictsLabel { .. }
            | IntegrityWitness::ExpectedPathIsAgentCreatable { .. }
            | IntegrityWitness::HiddenAssetReachableFromAgent { .. } => Blame::BenchmarkDesign,
            IntegrityWitness::GraderAcceptedNonSolution { .. } => Blame::Grader,
            IntegrityWitness::AuthorIsAlsoSubmitter { .. } => Blame::Contributor,
        }
    }
}

/// Who has to fix a witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blame {
    /// The pack is exploitable as designed; the agent did nothing wrong.
    BenchmarkDesign,
    /// The grader rewards the wrong thing.
    Grader,
    /// A person gamed the process.
    Contributor,
}

impl fmt::Display for Blame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Blame::BenchmarkDesign => "benchmark_design",
            Blame::Grader => "grader",
            Blame::Contributor => "contributor",
        })
    }
}

/// The outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStatus {
    /// The check ran on sufficient evidence and produced nothing.
    NoWitness,
    /// At least one witness.
    Exploitable,
    /// The check could not run: no probes, no labels, nothing to look at. Never clean.
    Underdetermined,
}

impl IntegrityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrityStatus::NoWitness => "no_witness",
            IntegrityStatus::Exploitable => "exploitable",
            IntegrityStatus::Underdetermined => "underdetermined",
        }
    }
}

impl fmt::Display for IntegrityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One check's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityVerdict {
    pub check: String,
    pub status: IntegrityStatus,
    pub witnesses: Vec<IntegrityWitness>,
    /// Present only when underdetermined: why the check could not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl IntegrityVerdict {
    /// The check ran. Empty witnesses means clean *for this check*.
    pub fn checked(check: impl Into<String>, witnesses: Vec<IntegrityWitness>) -> Self {
        let status = if witnesses.is_empty() {
            IntegrityStatus::NoWitness
        } else {
            IntegrityStatus::Exploitable
        };
        IntegrityVerdict {
            check: check.into(),
            status,
            witnesses,
            reason: None,
        }
    }

    /// The check could not run.
    pub fn underdetermined(check: impl Into<String>, reason: impl Into<String>) -> Self {
        IntegrityVerdict {
            check: check.into(),
            status: IntegrityStatus::Underdetermined,
            witnesses: Vec::new(),
            reason: Some(reason.into()),
        }
    }

    pub fn witness_kinds(&self) -> Vec<&'static str> {
        self.witnesses.iter().map(IntegrityWitness::kind).collect()
    }
}

/// Every check run against one pack.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub verdicts: Vec<IntegrityVerdict>,
}

impl IntegrityReport {
    pub fn push(&mut self, verdict: IntegrityVerdict) {
        self.verdicts.push(verdict);
    }

    pub fn witnesses(&self) -> Vec<&IntegrityWitness> {
        self.verdicts.iter().flat_map(|v| &v.witnesses).collect()
    }

    /// True only when every check ran and every check found nothing.
    ///
    /// An underdetermined check keeps this false. That is the whole point: a pack nobody probed is
    /// not a pack that passed.
    pub fn is_clean(&self) -> bool {
        !self.verdicts.is_empty()
            && self
                .verdicts
                .iter()
                .all(|v| v.status == IntegrityStatus::NoWitness)
    }

    pub fn underdetermined_checks(&self) -> Vec<&IntegrityVerdict> {
        self.verdicts
            .iter()
            .filter(|v| v.status == IntegrityStatus::Underdetermined)
            .collect()
    }

    pub fn by_blame(&self, blame: Blame) -> Vec<&IntegrityWitness> {
        self.witnesses()
            .into_iter()
            .filter(|w| w.blames() == blame)
            .collect()
    }
}

/// How an oracle decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleCheck {
    /// The named path exists. Satisfiable by `touch`.
    PathExists,
    /// The named path hashes to a declared digest.
    ContentDigest,
    /// A program was run and its behaviour observed.
    Execution,
    /// A model judged it.
    ModelJudgement,
}

/// One benchmark task, in the shape these checks need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub id: String,
    /// The answer the oracle compares against.
    pub reference_answer: String,
    /// `(segment id, text)` pairs the agent is given.
    pub context: Vec<(String, String)>,
    /// Paths the oracle looks for, and how it checks them.
    pub expected_paths: Vec<(String, OracleCheck)>,
    /// Paths the agent may write to.
    pub agent_writable_prefixes: Vec<String>,
}

impl TaskSpec {
    pub fn new(id: impl Into<String>, reference_answer: impl Into<String>) -> Self {
        TaskSpec {
            id: id.into(),
            reference_answer: reference_answer.into(),
            context: Vec::new(),
            expected_paths: Vec::new(),
            agent_writable_prefixes: Vec::new(),
        }
    }

    pub fn with_context(mut self, id: impl Into<String>, text: impl Into<String>) -> Self {
        self.context.push((id.into(), text.into()));
        self
    }

    pub fn expecting(mut self, path: impl Into<String>, check: OracleCheck) -> Self {
        self.expected_paths.push((path.into(), check));
        self
    }

    pub fn writable(mut self, prefix: impl Into<String>) -> Self {
        self.agent_writable_prefixes.push(prefix.into());
        self
    }
}

/// A constant output and the tasks whose oracle accepted it. Supplied by the caller, which ran them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegenerateProbe {
    pub output: String,
    pub accepted_by: BTreeSet<String>,
}

impl DegenerateProbe {
    pub fn new(output: impl Into<String>) -> Self {
        DegenerateProbe {
            output: output.into(),
            accepted_by: BTreeSet::new(),
        }
    }

    pub fn accepted_by(mut self, task: impl Into<String>) -> Self {
        self.accepted_by.insert(task.into());
        self
    }
}

/// One grader probe, with the operator's own judgement attached.
///
/// `solves_task` is the operator's label. This module treats it as given and looks for a
/// contradiction between it and `accepted`; it has no way to check it and does not pretend to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraderProbe {
    pub output: String,
    pub has_feature: bool,
    pub accepted: bool,
    pub solves_task: bool,
}

/// A labelled instance, for the identifier-leakage check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelledInstance {
    pub identifier: String,
    pub label: String,
}

impl LabelledInstance {
    pub fn new(identifier: impl Into<String>, label: impl Into<String>) -> Self {
        LabelledInstance {
            identifier: identifier.into(),
            label: label.into(),
        }
    }
}

/// Does the benchmark ship its own answer inside the context it hands the agent?
///
/// Exact containment only. See the module docs on why the result is a lower bound.
pub fn check_answer_containment(tasks: &[TaskSpec]) -> IntegrityVerdict {
    if tasks.is_empty() {
        return IntegrityVerdict::underdetermined("answer_containment", "no tasks supplied");
    }
    let mut witnesses = Vec::new();
    for task in tasks {
        if task.reference_answer.is_empty() {
            continue;
        }
        for (segment, text) in &task.context {
            if text.contains(&task.reference_answer) {
                witnesses.push(IntegrityWitness::AnswerInOwnContext {
                    task: task.id.clone(),
                    answer: task.reference_answer.clone(),
                    segment: segment.clone(),
                });
            }
        }
    }
    IntegrityVerdict::checked("answer_containment", witnesses)
}

/// Is there a constant output that satisfies this oracle across several tasks?
///
/// `min_tasks` is the number of distinct tasks a single output must satisfy before it counts. One
/// is not enough: some task's correct answer is `"0"`.
pub fn check_oracle_degeneracy(
    oracle: &str,
    probes: &[DegenerateProbe],
    min_tasks: usize,
) -> IntegrityVerdict {
    if probes.is_empty() {
        return IntegrityVerdict::underdetermined(
            "oracle_degeneracy",
            format!("no probes were run against {oracle}; an unprobed oracle is not a sound one"),
        );
    }
    let witnesses = probes
        .iter()
        .filter(|probe| probe.accepted_by.len() >= min_tasks.max(2))
        .map(|probe| IntegrityWitness::ConstantOutputSatisfiesOracle {
            oracle: oracle.to_string(),
            output: probe.output.clone(),
            tasks: probe.accepted_by.iter().cloned().collect(),
        })
        .collect();
    IntegrityVerdict::checked("oracle_degeneracy", witnesses)
}

/// Did the grader accept something the operator says does not solve the task?
pub fn check_grader_surface_feature(
    grader: &str,
    feature: &str,
    probes: &[GraderProbe],
) -> IntegrityVerdict {
    if probes.is_empty() {
        return IntegrityVerdict::underdetermined(
            "grader_surface_feature",
            format!("no probes were run against {grader}"),
        );
    }
    let accepted_non_solutions: Vec<String> = probes
        .iter()
        .filter(|probe| probe.accepted && !probe.solves_task && probe.has_feature)
        .map(|probe| probe.output.clone())
        .collect();
    let witnesses = if accepted_non_solutions.is_empty() {
        Vec::new()
    } else {
        vec![IntegrityWitness::GraderAcceptedNonSolution {
            grader: grader.to_string(),
            feature: feature.to_string(),
            outputs: accepted_non_solutions,
        }]
    };
    IntegrityVerdict::checked("grader_surface_feature", witnesses)
}

/// Does a prefix of the identifier determine the label?
///
/// Reports the shortest prefix length for which the mapping is a function and at least two labels
/// are distinguished. Shortest, because a prefix as long as the identifier is always a function and
/// says nothing.
pub fn check_identifier_label_leakage(
    field: &str,
    instances: &[LabelledInstance],
) -> IntegrityVerdict {
    let labels: BTreeSet<&str> = instances.iter().map(|i| i.label.as_str()).collect();
    if instances.len() < 2 || labels.len() < 2 {
        return IntegrityVerdict::underdetermined(
            "identifier_label_leakage",
            "fewer than two instances or fewer than two distinct labels",
        );
    }
    let longest = instances
        .iter()
        .map(|i| i.identifier.chars().count())
        .max()
        .unwrap_or(0);
    for prefix_length in 1..=longest {
        let mut mapping: BTreeMap<String, String> = BTreeMap::new();
        let mut is_function = true;
        for instance in instances {
            let prefix: String = instance.identifier.chars().take(prefix_length).collect();
            match mapping.get(&prefix) {
                Some(existing) if existing != &instance.label => {
                    is_function = false;
                    break;
                }
                Some(_) => {}
                None => {
                    mapping.insert(prefix, instance.label.clone());
                }
            }
        }
        if is_function && mapping.len() < instances.len() {
            return IntegrityVerdict::checked(
                "identifier_label_leakage",
                vec![IntegrityWitness::IdentifierPredictsLabel {
                    field: field.to_string(),
                    prefix_length,
                    mapping,
                }],
            );
        }
    }
    IntegrityVerdict::checked("identifier_label_leakage", Vec::new())
}

/// Does the oracle check only that a path exists, on a path the agent can create?
pub fn check_expected_path_creatable(tasks: &[TaskSpec]) -> IntegrityVerdict {
    if tasks.is_empty() {
        return IntegrityVerdict::underdetermined("expected_path_creatable", "no tasks supplied");
    }
    let mut witnesses = Vec::new();
    for task in tasks {
        for (path, check) in &task.expected_paths {
            if *check != OracleCheck::PathExists {
                continue;
            }
            if task
                .agent_writable_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
            {
                witnesses.push(IntegrityWitness::ExpectedPathIsAgentCreatable {
                    task: task.id.clone(),
                    path: path.clone(),
                });
            }
        }
    }
    IntegrityVerdict::checked("expected_path_creatable", witnesses)
}

/// Can the agent reach the zone a hidden asset is declared in?
///
/// Uses the influence graph, so the answer comes with the path that makes it true.
pub fn check_hidden_asset_placement(
    model: &BoundaryModel,
    assets: &[(String, TrustZone)],
) -> IntegrityVerdict {
    if assets.is_empty() {
        return IntegrityVerdict::underdetermined(
            "hidden_asset_placement",
            "no hidden assets were declared; a pack with hidden state and no declaration is worse",
        );
    }
    let mut witnesses = Vec::new();
    for (asset, zone) in assets {
        let paths = model.influence_paths(TrustZone::AgentSandbox, *zone);
        if let Some(path) = paths.first() {
            witnesses.push(IntegrityWitness::HiddenAssetReachableFromAgent {
                asset: asset.clone(),
                zone: zone.to_string(),
                path: path.iter().map(|zone| zone.to_string()).collect(),
            });
        }
    }
    IntegrityVerdict::checked("hidden_asset_placement", witnesses)
}

/// Did the same principal author the task and submit the result?
pub fn check_author_submitter_separation(
    pack: &str,
    author: &str,
    submitters: &[String],
) -> IntegrityVerdict {
    if submitters.is_empty() {
        return IntegrityVerdict::underdetermined(
            "author_submitter_separation",
            format!("no submitters recorded for {pack}"),
        );
    }
    let witnesses = submitters
        .iter()
        .filter(|submitter| submitter.as_str() == author)
        .map(|submitter| IntegrityWitness::AuthorIsAlsoSubmitter {
            pack: pack.to_string(),
            principal: submitter.clone(),
        })
        .collect();
    IntegrityVerdict::checked("author_submitter_separation", witnesses)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary::Channel;

    #[test]
    fn a_task_that_ships_its_own_answer_in_its_context_yields_a_witness_naming_the_segment() {
        let tasks = vec![TaskSpec::new("T-3", "EGFR")
            .with_context("ctx-0", "The patient has a lung adenocarcinoma.")
            .with_context("ctx-1", "Prior report lists an EGFR exon 19 deletion.")];
        let verdict = check_answer_containment(&tasks);
        assert_eq!(verdict.status, IntegrityStatus::Exploitable);
        assert_eq!(
            verdict.witnesses[0],
            IntegrityWitness::AnswerInOwnContext {
                task: "T-3".into(),
                answer: "EGFR".into(),
                segment: "ctx-1".into(),
            }
        );
    }

    #[test]
    fn an_oracle_nobody_probed_is_underdetermined_rather_than_clean() {
        let verdict = check_oracle_degeneracy("exact-match", &[], 2);
        assert_eq!(verdict.status, IntegrityStatus::Underdetermined);
        assert!(verdict.witnesses.is_empty());
        assert!(verdict.reason.is_some());
    }

    #[test]
    fn one_constant_output_accepted_by_two_tasks_is_a_degenerate_oracle() {
        let probes = vec![DegenerateProbe::new("{}")
            .accepted_by("T-1")
            .accepted_by("T-2")];
        let verdict = check_oracle_degeneracy("json-shape", &probes, 2);
        assert_eq!(verdict.witness_kinds(), vec!["constant_output_satisfies_oracle"]);
    }

    #[test]
    fn an_output_accepted_by_a_single_task_is_not_yet_a_degenerate_oracle() {
        let probes = vec![DegenerateProbe::new("0").accepted_by("T-1")];
        let verdict = check_oracle_degeneracy("numeric", &probes, 2);
        assert_eq!(verdict.status, IntegrityStatus::NoWitness);
    }

    #[test]
    fn a_grader_that_accepted_a_non_solution_carrying_the_feature_is_witnessed() {
        let probes = vec![
            GraderProbe {
                output: "Answer: 42. Reasoning: ...".into(),
                has_feature: true,
                accepted: true,
                solves_task: false,
            },
            GraderProbe {
                output: "42".into(),
                has_feature: false,
                accepted: false,
                solves_task: true,
            },
        ];
        let verdict = check_grader_surface_feature("judge", "answer_prefix", &probes);
        assert_eq!(verdict.status, IntegrityStatus::Exploitable);
        assert_eq!(verdict.witnesses[0].blames(), Blame::Grader);
    }

    #[test]
    fn a_grader_probe_set_with_no_contradiction_produces_no_witness() {
        let probes = vec![GraderProbe {
            output: "42".into(),
            has_feature: true,
            accepted: true,
            solves_task: true,
        }];
        assert_eq!(
            check_grader_surface_feature("judge", "answer_prefix", &probes).status,
            IntegrityStatus::NoWitness
        );
    }

    #[test]
    fn an_identifier_prefix_that_determines_the_label_is_reported_with_the_mapping() {
        let instances = vec![
            LabelledInstance::new("pos-001", "positive"),
            LabelledInstance::new("pos-002", "positive"),
            LabelledInstance::new("neg-001", "negative"),
            LabelledInstance::new("neg-002", "negative"),
        ];
        let verdict = check_identifier_label_leakage("instance_id", &instances);
        match &verdict.witnesses[0] {
            IntegrityWitness::IdentifierPredictsLabel {
                prefix_length,
                mapping,
                ..
            } => {
                assert_eq!(*prefix_length, 1);
                assert_eq!(mapping.get("p"), Some(&"positive".to_string()));
                assert_eq!(mapping.get("n"), Some(&"negative".to_string()));
            }
            other => panic!("expected an identifier witness, got {other:?}"),
        }
    }

    #[test]
    fn identifiers_that_do_not_encode_the_label_produce_no_witness() {
        let instances = vec![
            LabelledInstance::new("a1", "positive"),
            LabelledInstance::new("a2", "negative"),
            LabelledInstance::new("b1", "negative"),
            LabelledInstance::new("b2", "positive"),
        ];
        assert_eq!(
            check_identifier_label_leakage("instance_id", &instances).status,
            IntegrityStatus::NoWitness
        );
    }

    #[test]
    fn a_single_label_makes_the_identifier_check_underdetermined_not_clean() {
        let instances = vec![
            LabelledInstance::new("x1", "positive"),
            LabelledInstance::new("x2", "positive"),
        ];
        assert_eq!(
            check_identifier_label_leakage("instance_id", &instances).status,
            IntegrityStatus::Underdetermined
        );
    }

    #[test]
    fn an_existence_check_on_an_agent_writable_path_is_satisfiable_by_touch() {
        let tasks = vec![TaskSpec::new("T-9", "")
            .writable("/workspace/")
            .expecting("/workspace/out/report.json", OracleCheck::PathExists)
            .expecting("/workspace/out/model.bin", OracleCheck::ContentDigest)];
        let verdict = check_expected_path_creatable(&tasks);
        assert_eq!(verdict.witnesses.len(), 1, "only the existence check leaks");
        assert_eq!(
            verdict.witnesses[0],
            IntegrityWitness::ExpectedPathIsAgentCreatable {
                task: "T-9".into(),
                path: "/workspace/out/report.json".into(),
            }
        );
    }

    #[test]
    fn a_hidden_asset_in_a_zone_the_agent_reaches_is_reported_with_the_reaching_path() {
        let model = BoundaryModel::evaluation_model();
        let verdict = check_hidden_asset_placement(
            &model,
            &[
                ("holdout-labels".into(), TrustZone::ArtifactService),
                ("oracle-key".into(), TrustZone::TrustedReview),
            ],
        );
        assert_eq!(verdict.witnesses.len(), 1);
        match &verdict.witnesses[0] {
            IntegrityWitness::HiddenAssetReachableFromAgent { asset, path, .. } => {
                assert_eq!(asset, "holdout-labels");
                assert_eq!(path.first().map(String::as_str), Some("agent_sandbox"));
            }
            other => panic!("expected a placement witness, got {other:?}"),
        }
    }

    #[test]
    fn a_pack_author_who_also_submits_results_is_a_contributor_finding() {
        let verdict = check_author_submitter_separation(
            "onco-v1",
            "org:acme",
            &["org:acme".into(), "org:globex".into()],
        );
        assert_eq!(verdict.witnesses.len(), 1);
        assert_eq!(verdict.witnesses[0].blames(), Blame::Contributor);
    }

    #[test]
    fn a_report_with_an_underdetermined_check_is_never_clean() {
        let mut report = IntegrityReport::default();
        report.push(IntegrityVerdict::checked("answer_containment", Vec::new()));
        report.push(IntegrityVerdict::underdetermined(
            "oracle_degeneracy",
            "no probes",
        ));
        assert!(!report.is_clean());
        assert_eq!(report.underdetermined_checks().len(), 1);
    }

    #[test]
    fn an_empty_report_is_not_clean_because_nothing_was_checked() {
        assert!(!IntegrityReport::default().is_clean());
    }

    #[test]
    fn witnesses_separate_a_broken_benchmark_from_a_gaming_contributor() {
        let mut report = IntegrityReport::default();
        report.push(check_answer_containment(&[TaskSpec::new("T-1", "yes")
            .with_context("c", "the answer is yes")]));
        report.push(check_author_submitter_separation(
            "p",
            "org:a",
            &["org:a".into()],
        ));
        assert_eq!(report.by_blame(Blame::BenchmarkDesign).len(), 1);
        assert_eq!(report.by_blame(Blame::Contributor).len(), 1);
        assert!(report.by_blame(Blame::Grader).is_empty());
    }

    #[test]
    fn the_hidden_asset_check_uses_the_same_edge_map_the_boundary_model_publishes() {
        let sealed = BoundaryModel::new().allow(
            TrustZone::ControlPlane,
            TrustZone::EvaluatorSandbox,
            Channel::HiddenOracleMount,
        );
        let verdict =
            check_hidden_asset_placement(&sealed, &[("holdout".into(), TrustZone::EvaluatorSandbox)]);
        assert_eq!(
            verdict.status,
            IntegrityStatus::NoWitness,
            "an agent with no edge out cannot reach the evaluator's mount"
        );
    }
}
