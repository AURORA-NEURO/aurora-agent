//! The compile introspection record: the developer's view of what the compiler decided.
//!
//! Blueprint 43.16 asks for "explain-plan output modeled after database systems" and 11.25 asks
//! for diagnostics that include "effective config" and a "bundle verifier". `bioprism-cli`'s
//! `explain` module already *renders* a compile for a person to read. This module is the other
//! half: the same information as a queryable record, so an agent can answer **why was this fact
//! omitted** without re-running the compiler, and can tell the difference between an answer and
//! the absence of one.
//!
//! # The distinction this module is forbidden to weaken
//!
//! `bioprism-section`'s omission manifest exists to keep *provably cannot matter*
//! ([`InfluenceClass::Zero`]) apart from *nobody checked* ([`InfluenceClass::Unknown`]). A
//! developer view that renders both as "omitted" would destroy the property the certificate was
//! built to carry, so [`OmissionAnswer`] keeps the class verbatim and
//! [`OmissionEntry::developer_label`] is injective over [`InfluenceClass`] — asserted by a test,
//! not by care.
//!
//! There is a third state, and it is this module's own: **the record does not say**. An omission
//! manifest carries counts and *representative* members, never the whole list, so a subject that
//! appears in no group's examples has not been shown to be selected. [`OmissionAnswer::NotRecorded`]
//! is that answer. Collapsing it into `Zero` would be exactly the failure `Zero` versus `Unknown`
//! guards against, one level down, and collapsing it into an error would let a caller
//! `unwrap_or_default()` it into silence.
//!
//! # What is deliberately not here
//!
//! No compiler. This module never runs a pass; it reads a record of passes that ran elsewhere. It
//! therefore cannot detect a pass that was omitted from the record entirely — if a compiler forgets
//! to declare a pass, nothing here will notice, and [`CompileRecord::coverage`] says so by
//! answering only questions the record's own contents support.
//!
//! No storage, no clock, no run identity beyond the ids the record carries.

use crate::diagnostic::{Certainty, Site};
use crate::error::IntrospectError;
use bioprism_ids::ContentHash;
use bioprism_section::{InfluenceClass, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What happened to one pass.
///
/// Four cases rather than a boolean, because "did not run" hides three different situations that
/// call for three different developer actions. 43.16's deferred-pass list conflates them today;
/// separating them here is this module's addition, and it is why a deferred pass carries a reason
/// and a skipped pass carries the precondition that was not met.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PassOutcome {
    /// The pass ran to completion.
    Ran { considered: usize, retained: usize },
    /// The pass exists but has no implementation on this path. The reason names what is missing.
    Deferred { reason: String },
    /// The pass is implemented, and a precondition for running it was not satisfied by the input.
    Skipped { unmet_precondition: String },
    /// The pass ran and failed. The diagnostic code is the join key into the catalogue.
    Failed { diagnostic_code: String },
}

impl PassOutcome {
    pub fn ran(&self) -> bool {
        matches!(self, PassOutcome::Ran { .. })
    }

    /// Why this pass contributed nothing, when it contributed nothing.
    pub fn absence_reason(&self) -> Option<&str> {
        match self {
            PassOutcome::Ran { .. } => None,
            PassOutcome::Deferred { reason } => Some(reason),
            PassOutcome::Skipped { unmet_precondition } => Some(unmet_precondition),
            PassOutcome::Failed { diagnostic_code } => Some(diagnostic_code),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PassOutcome::Ran { .. } => "ran",
            PassOutcome::Deferred { .. } => "deferred",
            PassOutcome::Skipped { .. } => "skipped",
            PassOutcome::Failed { .. } => "failed",
        }
    }
}

/// One pass, its outcome, and what it decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassRecord {
    pub name: String,
    pub outcome: PassOutcome,
    /// What the pass decided, in a sentence a reader can check against the artefact. Not a log
    /// line: "kept every fact reachable from the target within depth 2" rather than "done".
    pub decision: String,
    /// Subjects this pass removed, by id. The field that makes an omission attributable to a pass
    /// rather than to the compiler in general.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
}

impl PassRecord {
    pub fn new(name: impl Into<String>, outcome: PassOutcome, decision: impl Into<String>) -> Self {
        PassRecord {
            name: name.into(),
            outcome,
            decision: decision.into(),
            removed: Vec::new(),
        }
    }

    pub fn removing(mut self, subject: impl Into<String>) -> Self {
        self.removed.push(subject.into());
        self
    }
}

/// A developer-facing view of one omission group, with the influence class carried through intact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmissionEntry {
    pub reason: String,
    pub influence: InfluenceClass,
    pub count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
    pub examples: Vec<String>,
    /// Whether this group may participate in a sufficiency claim, taken from the class.
    pub supports_sufficiency: bool,
    /// What would have to change for this group to stop blocking a sufficiency claim.
    ///
    /// `None` when the group already supports one. This is the [`crate::diagnostic::Remedy`] idea
    /// applied to an omission: a manifest that says "unknown" and stops leaves a developer with
    /// nothing to do about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

impl OmissionEntry {
    /// A distinct label per influence class.
    ///
    /// Injective on purpose: two classes sharing a label would erase the distinction the manifest
    /// exists to carry, and a rendering layer downstream would have no way to recover it.
    pub fn developer_label(influence: InfluenceClass) -> &'static str {
        match influence {
            InfluenceClass::Zero => "cannot affect the decision (proved)",
            InfluenceClass::Bounded => "can affect the decision by at most a stated bound",
            InfluenceClass::InaccessibleByPolicy => "withheld by policy or consent",
            InfluenceClass::DeferredAcquisition => "not available at the temporal cut",
            InfluenceClass::Unknown => "not analysed; nobody checked",
        }
    }

    /// The remedy for a group that blocks a sufficiency claim.
    pub fn remedy_for(influence: InfluenceClass) -> Option<&'static str> {
        match influence {
            InfluenceClass::Zero | InfluenceClass::Bounded => None,
            InfluenceClass::InaccessibleByPolicy => Some(
                "obtain the access grant or re-scope the query away from the withheld variables; \
                 the gap cannot be closed by recompiling",
            ),
            InfluenceClass::DeferredAcquisition => Some(
                "move the temporal cut later than the governing event, or accept the gap and state \
                 it in the decision",
            ),
            InfluenceClass::Unknown => Some(
                "run the influence analysis over this group; until it runs, no sufficiency claim \
                 may be made on this context",
            ),
        }
    }

    pub fn label(&self) -> &'static str {
        OmissionEntry::developer_label(self.influence)
    }
}

/// The answer to "why was this subject omitted".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum OmissionAnswer {
    /// It was not omitted. The record lists it as selected.
    Selected,
    /// It was omitted, and the record says why.
    Omitted {
        influence: InfluenceClass,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bound: Option<f64>,
        /// The pass that removed it, when a pass claimed it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attributed_to: Option<String>,
        /// How confidently the attribution is asserted. A subject named in a group's examples is
        /// observed; one attributed only by a pass's removal list without a matching group is
        /// inferred.
        certainty: Certainty,
    },
    /// The record does not say.
    ///
    /// Not an error and not an omission with unknown influence: those are claims about the
    /// *world*, this is a claim about the *record*. An omission manifest carries representatives,
    /// not membership, so silence here is silence and nothing more.
    NotRecorded { because: String },
}

impl OmissionAnswer {
    pub fn is_answer_about_the_world(&self) -> bool {
        !matches!(self, OmissionAnswer::NotRecorded { .. })
    }
}

/// A question a developer asks of a compile after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeveloperQuestion {
    WhichPassesRan,
    WhichPassesDidNotRunAndWhy,
    WhatEachPassDecided,
    WhyWasASubjectOmitted,
    WhetherTheContextClaimsSufficiency,
    WhatBindsTheResultToItsInputs,
}

impl DeveloperQuestion {
    pub const ALL: [DeveloperQuestion; 6] = [
        DeveloperQuestion::WhichPassesRan,
        DeveloperQuestion::WhichPassesDidNotRunAndWhy,
        DeveloperQuestion::WhatEachPassDecided,
        DeveloperQuestion::WhyWasASubjectOmitted,
        DeveloperQuestion::WhetherTheContextClaimsSufficiency,
        DeveloperQuestion::WhatBindsTheResultToItsInputs,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DeveloperQuestion::WhichPassesRan => "which_passes_ran",
            DeveloperQuestion::WhichPassesDidNotRunAndWhy => "which_passes_did_not_run_and_why",
            DeveloperQuestion::WhatEachPassDecided => "what_each_pass_decided",
            DeveloperQuestion::WhyWasASubjectOmitted => "why_was_a_subject_omitted",
            DeveloperQuestion::WhetherTheContextClaimsSufficiency => {
                "whether_the_context_claims_sufficiency"
            }
            DeveloperQuestion::WhatBindsTheResultToItsInputs => "what_binds_the_result_to_its_inputs",
        }
    }
}

/// Which questions a record can answer, and why not where it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntrospectionCoverage {
    pub answerable: Vec<DeveloperQuestion>,
    /// Questions the record cannot answer, each with the missing field named.
    pub unanswerable: Vec<(DeveloperQuestion, String)>,
}

impl IntrospectionCoverage {
    pub fn answers(&self, question: DeveloperQuestion) -> bool {
        self.answerable.contains(&question)
    }
}

/// Everything the compiler decided, in a form a later reader can query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompileRecord {
    pub world_id: String,
    pub query_id: String,
    pub passes: Vec<PassRecord>,
    /// The selected subjects, by id. Membership here is authoritative; absence is not.
    pub selected: BTreeSet<String>,
    /// The omission manifest exactly as `bioprism-section` produced it.
    pub manifest: OmissionManifest,
    /// The certificate digest binding the result to its inputs, when one was produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_digest: Option<String>,
    /// Limitations the compiler declared about its own output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl CompileRecord {
    pub fn new(world_id: impl Into<String>, query_id: impl Into<String>) -> Self {
        CompileRecord {
            world_id: world_id.into(),
            query_id: query_id.into(),
            passes: Vec::new(),
            selected: BTreeSet::new(),
            manifest: OmissionManifest::default(),
            certificate_digest: None,
            limitations: Vec::new(),
        }
    }

    pub fn with_pass(mut self, pass: PassRecord) -> Self {
        self.passes.push(pass);
        self
    }

    pub fn selecting(mut self, subject: impl Into<String>) -> Self {
        self.selected.insert(subject.into());
        self
    }

    pub fn with_manifest(mut self, manifest: OmissionManifest) -> Self {
        self.manifest = manifest;
        self
    }

    pub fn bound_by(mut self, digest: impl Into<String>) -> Self {
        self.certificate_digest = Some(digest.into());
        self
    }

    pub fn limited_by(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    /// Passes that ran.
    pub fn ran(&self) -> Vec<&PassRecord> {
        self.passes.iter().filter(|p| p.outcome.ran()).collect()
    }

    /// Passes that did not run, whatever the reason.
    pub fn did_not_run(&self) -> Vec<&PassRecord> {
        self.passes.iter().filter(|p| !p.outcome.ran()).collect()
    }

    /// A pass by name, or a typed refusal.
    pub fn pass(&self, name: &str) -> Result<&PassRecord, IntrospectError> {
        let matches: Vec<&PassRecord> = self.passes.iter().filter(|p| p.name == name).collect();
        match matches.len() {
            0 => Err(IntrospectError::UnknownPass {
                pass: name.to_string(),
            }),
            1 => Ok(matches[0]),
            count => Err(IntrospectError::DuplicatePass {
                pass: name.to_string(),
                count,
            }),
        }
    }

    /// The developer view over the omission manifest.
    ///
    /// One entry per group, in manifest order, with the influence class carried through unchanged.
    pub fn omissions(&self) -> Vec<OmissionEntry> {
        self.manifest
            .groups
            .iter()
            .map(|group| OmissionEntry {
                reason: group.reason.clone(),
                influence: group.influence,
                count: group.count,
                bound: group.bound,
                examples: group.examples.clone(),
                supports_sufficiency: group.influence.supports_sufficiency(),
                remedy: OmissionEntry::remedy_for(group.influence).map(str::to_string),
            })
            .collect()
    }

    /// Whether the context may be presented as sufficient.
    ///
    /// Delegates to the manifest. It is a delegation rather than a recomputation so the view
    /// cannot drift into claiming a sufficiency the certificate denies.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.manifest.supports_sufficiency_claim()
    }

    /// Groups that block a sufficiency claim, with their remedies.
    pub fn blocking_omissions(&self) -> Vec<OmissionEntry> {
        self.omissions()
            .into_iter()
            .filter(|entry| !entry.supports_sufficiency)
            .collect()
    }

    /// Why a subject is not in the compiled context.
    ///
    /// Three sources are consulted in order, and the order is the point: the selected set is
    /// authoritative, a group example is an observation, a pass removal list without a matching
    /// group is an inference, and everything else is [`OmissionAnswer::NotRecorded`].
    pub fn why_omitted(&self, subject: &str) -> OmissionAnswer {
        if self.selected.contains(subject) {
            return OmissionAnswer::Selected;
        }

        let attributed = self
            .passes
            .iter()
            .find(|pass| pass.removed.iter().any(|s| s == subject))
            .map(|pass| pass.name.clone());

        if let Some(group) = self
            .manifest
            .groups
            .iter()
            .find(|group| group.examples.iter().any(|s| s == subject))
        {
            return OmissionAnswer::Omitted {
                influence: group.influence,
                reason: group.reason.clone(),
                bound: group.bound,
                attributed_to: attributed,
                certainty: Certainty::Observed,
            };
        }

        if let Some(pass_name) = attributed {
            let pass = self
                .passes
                .iter()
                .find(|p| p.name == pass_name)
                .expect("the pass was found by iterating the same list");
            return OmissionAnswer::Omitted {
                influence: InfluenceClass::Unknown,
                reason: format!(
                    "removed by pass {pass_name} ({}); no omission group in the manifest claims it, \
                     so its influence class was never assigned",
                    pass.decision
                ),
                bound: None,
                attributed_to: Some(pass_name),
                certainty: Certainty::Inferred,
            };
        }

        OmissionAnswer::NotRecorded {
            because: format!(
                "{subject} is neither in the selected set nor named by any pass or omission group; \
                 omission groups carry counts and representative members, not membership, so this \
                 record cannot decide whether {subject} was omitted or never existed"
            ),
        }
    }

    /// The site to cite when reporting something about this compile.
    pub fn site(&self) -> Site {
        match &self.certificate_digest {
            Some(digest) => Site::Digest {
                digest: digest.clone(),
            },
            None => Site::Artifact {
                node_kind: "compile".to_string(),
                id: format!("{}::{}", self.world_id, self.query_id),
            },
        }
    }

    /// The certificate digest, parsed, or a typed refusal naming the question that needed it.
    pub fn verified_digest(&self, question: &str) -> Result<ContentHash, IntrospectError> {
        let raw = self
            .certificate_digest
            .as_ref()
            .ok_or_else(|| IntrospectError::NoCertificate {
                question: question.to_string(),
            })?;
        ContentHash::parse(raw.clone()).map_err(|_| IntrospectError::NoCertificate {
            question: format!("{question} (the recorded digest is not a sha256 hex string)"),
        })
    }

    /// Which developer questions this record answers.
    pub fn coverage(&self) -> IntrospectionCoverage {
        let mut answerable = Vec::new();
        let mut unanswerable = Vec::new();

        if self.passes.is_empty() {
            unanswerable.push((
                DeveloperQuestion::WhichPassesRan,
                "the record lists no passes".to_string(),
            ));
            unanswerable.push((
                DeveloperQuestion::WhichPassesDidNotRunAndWhy,
                "the record lists no passes".to_string(),
            ));
            unanswerable.push((
                DeveloperQuestion::WhatEachPassDecided,
                "the record lists no passes".to_string(),
            ));
        } else {
            answerable.push(DeveloperQuestion::WhichPassesRan);
            answerable.push(DeveloperQuestion::WhichPassesDidNotRunAndWhy);
            let silent: Vec<&str> = self
                .passes
                .iter()
                .filter(|p| p.decision.trim().is_empty())
                .map(|p| p.name.as_str())
                .collect();
            if silent.is_empty() {
                answerable.push(DeveloperQuestion::WhatEachPassDecided);
            } else {
                unanswerable.push((
                    DeveloperQuestion::WhatEachPassDecided,
                    format!("these passes recorded no decision: {}", silent.join(", ")),
                ));
            }
        }

        if self.manifest.groups.is_empty() && self.selected.is_empty() {
            unanswerable.push((
                DeveloperQuestion::WhyWasASubjectOmitted,
                "the record carries neither a selected set nor an omission manifest, so every \
                 subject answers not_recorded"
                    .to_string(),
            ));
        } else {
            answerable.push(DeveloperQuestion::WhyWasASubjectOmitted);
        }

        answerable.push(DeveloperQuestion::WhetherTheContextClaimsSufficiency);

        match &self.certificate_digest {
            Some(_) => answerable.push(DeveloperQuestion::WhatBindsTheResultToItsInputs),
            None => unanswerable.push((
                DeveloperQuestion::WhatBindsTheResultToItsInputs,
                "the record carries no certificate digest".to_string(),
            )),
        }

        answerable.sort();
        unanswerable.sort();
        IntrospectionCoverage {
            answerable,
            unanswerable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_section::OmissionGroup;

    fn record() -> CompileRecord {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: "no dependency path reaches the target".into(),
            influence: InfluenceClass::Zero,
            count: 12,
            bound: None,
            examples: vec!["fact:unreachable-1".into()],
        });
        manifest.push(OmissionGroup {
            reason: "not analysed".into(),
            influence: InfluenceClass::Unknown,
            count: 3,
            bound: None,
            examples: vec!["fact:unchecked-1".into()],
        });
        CompileRecord::new("world-1", "query-1")
            .selecting("fact:kept-1")
            .with_manifest(manifest)
            .with_pass(
                PassRecord::new(
                    "protected_closure",
                    PassOutcome::Ran {
                        considered: 40,
                        retained: 18,
                    },
                    "unioned every fact carrying a protected tag into the selection",
                )
                .removing("fact:dropped-by-pass"),
            )
            .with_pass(PassRecord::new(
                "abstract_interpretation",
                PassOutcome::Deferred {
                    reason: "fiber-world/0.1 carries no abstract-domain registry".into(),
                },
                "no domain was available to over-approximate in",
            ))
    }

    #[test]
    fn the_developer_label_is_injective_over_the_five_influence_classes() {
        let classes = [
            InfluenceClass::Zero,
            InfluenceClass::Bounded,
            InfluenceClass::InaccessibleByPolicy,
            InfluenceClass::DeferredAcquisition,
            InfluenceClass::Unknown,
        ];
        let mut labels: Vec<&str> = classes
            .iter()
            .map(|c| OmissionEntry::developer_label(*c))
            .collect();
        let before = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(before, labels.len(), "two influence classes share a label");
    }

    #[test]
    fn zero_influence_and_unknown_influence_stay_distinct_in_the_developer_view() {
        let entries = record().omissions();
        let zero = entries
            .iter()
            .find(|e| e.influence == InfluenceClass::Zero)
            .expect("zero group present");
        let unknown = entries
            .iter()
            .find(|e| e.influence == InfluenceClass::Unknown)
            .expect("unknown group present");
        assert!(zero.supports_sufficiency);
        assert!(!unknown.supports_sufficiency);
        assert_ne!(zero.label(), unknown.label());
        assert!(zero.remedy.is_none());
        assert!(unknown.remedy.is_some());
    }

    #[test]
    fn a_subject_absent_from_the_record_is_not_reported_as_zero_influence() {
        let answer = record().why_omitted("fact:never-mentioned");
        match answer {
            OmissionAnswer::NotRecorded { because } => {
                assert!(because.contains("representative members"));
            }
            other => panic!("expected not_recorded, got {other:?}"),
        }
    }

    #[test]
    fn a_subject_named_in_a_group_example_answers_with_that_groups_influence_class() {
        match record().why_omitted("fact:unchecked-1") {
            OmissionAnswer::Omitted {
                influence,
                certainty,
                ..
            } => {
                assert_eq!(influence, InfluenceClass::Unknown);
                assert_eq!(certainty, Certainty::Observed);
            }
            other => panic!("expected omitted, got {other:?}"),
        }
    }

    #[test]
    fn a_subject_removed_by_a_pass_but_claimed_by_no_group_is_unknown_and_only_inferred() {
        match record().why_omitted("fact:dropped-by-pass") {
            OmissionAnswer::Omitted {
                influence,
                attributed_to,
                certainty,
                ..
            } => {
                assert_eq!(influence, InfluenceClass::Unknown);
                assert_eq!(attributed_to.as_deref(), Some("protected_closure"));
                assert_eq!(certainty, Certainty::Inferred);
            }
            other => panic!("expected omitted, got {other:?}"),
        }
    }

    #[test]
    fn a_selected_subject_is_never_reported_as_omitted() {
        assert_eq!(record().why_omitted("fact:kept-1"), OmissionAnswer::Selected);
    }

    #[test]
    fn the_view_never_claims_a_sufficiency_the_manifest_denies() {
        let record = record();
        assert!(!record.manifest.supports_sufficiency_claim());
        assert!(!record.supports_sufficiency_claim());
        assert_eq!(record.blocking_omissions().len(), 1);
    }

    #[test]
    fn a_record_without_a_certificate_refuses_the_binding_question_by_name() {
        let record = record();
        let error = record
            .verified_digest("which world produced this")
            .expect_err("no digest recorded");
        assert!(matches!(error, IntrospectError::NoCertificate { .. }));
        assert!(!record
            .coverage()
            .answers(DeveloperQuestion::WhatBindsTheResultToItsInputs));
    }

    #[test]
    fn a_recorded_digest_that_is_not_a_hash_is_refused_rather_than_returned() {
        let record = record().bound_by("not-a-digest");
        assert!(record.verified_digest("binding").is_err());
        assert!(record
            .coverage()
            .answers(DeveloperQuestion::WhatBindsTheResultToItsInputs));
    }

    #[test]
    fn a_pass_that_recorded_no_decision_makes_that_question_unanswerable() {
        let record = record().with_pass(PassRecord::new(
            "silent_pass",
            PassOutcome::Ran {
                considered: 1,
                retained: 1,
            },
            "",
        ));
        let coverage = record.coverage();
        assert!(!coverage.answers(DeveloperQuestion::WhatEachPassDecided));
        assert!(coverage
            .unanswerable
            .iter()
            .any(|(_, why)| why.contains("silent_pass")));
    }

    #[test]
    fn an_unknown_pass_name_is_a_typed_refusal_not_an_empty_option() {
        let record = record();
        assert!(matches!(
            record.pass("no_such_pass"),
            Err(IntrospectError::UnknownPass { .. })
        ));
        assert_eq!(record.ran().len(), 1);
        assert_eq!(record.did_not_run().len(), 1);
    }

    #[test]
    fn a_deferred_pass_states_what_is_missing_rather_than_that_it_is_missing() {
        let record = record();
        let pass = record.pass("abstract_interpretation").expect("pass exists");
        let reason = pass.outcome.absence_reason().expect("deferred passes explain");
        assert!(reason.contains("abstract-domain registry"));
        assert_eq!(pass.outcome.as_str(), "deferred");
    }

    #[test]
    fn a_record_round_trips_through_json() {
        let record = record();
        let encoded = serde_json::to_string(&record).expect("serialises");
        let decoded: CompileRecord = serde_json::from_str(&encoded).expect("parses back");
        assert_eq!(record, decoded);
    }
}
