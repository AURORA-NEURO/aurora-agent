//! OncoWorld temporal context (39.15).
//!
//! 39.15 compiles a longitudinal capsule for a decision taken at a particular moment in a disease
//! course. Its four invariants are the four ways such a capsule goes wrong.
//!
//! # Future follow-up cannot decide the decision it followed
//!
//! [`TemporalFirewall`] admits events at or before its [`DecisionEpoch`] and refuses later ones
//! with [`TemporalError::FutureLeak`]. This is the section's most consequential invariant: a
//! response assessment that saw the next scan is not an assessment, it is a transcription, and
//! nothing downstream can detect the difference from the answer alone.
//!
//! Later evidence is not discarded — it goes to a [`RetrospectivePlane`], which the capsule builder
//! cannot read. Keeping it in a separate structure rather than behind a flag is deliberate: a flag
//! can be forgotten at one call site, a separate type cannot be passed to a function that does not
//! accept it.
//!
//! # An imaging interpretation detached from exposure is not interpretable
//!
//! The second invariant, and the one that is a type-level requirement here: an event of kind
//! [`EventKind::Imaging`] must carry [`TimelineEvent::clinical_context`] naming the treatment and
//! steroid exposure in force, or [`TemporalFirewall::admit`] refuses it. Pseudoprogression under
//! steroids and progression are the same picture; the difference is entirely in the context that a
//! compact capsule is most tempted to drop.
//!
//! # Historical wording is never overwritten
//!
//! [`DiagnosisHistory`] is append-only. A reclassification adds a record; it does not edit one.
//! [`DiagnosisHistory::as_of`] returns what was believed at an epoch, which is what a retrospective
//! reader of a historical decision needs, and [`DiagnosisHistory::current`] returns the latest,
//! which is what a present-day reader needs. Both exist because they are different questions.
//!
//! # Mixed and unresolved states are representable
//!
//! [`ResponseState`] has [`ResponseState::Mixed`] and [`ResponseState::Unresolved`] alongside the
//! clean categories. A response vocabulary with only clean categories forces a compiler to round,
//! and rounding is where a mixed response becomes a progression.
//!
//! # Not implemented
//!
//! No RANO or RAPNO rule engine, no measurement, no lesion tracking. The response categories here
//! are a vocabulary, not an assessment: nothing in this module decides what a scan showed. 39.15's
//! "RANO/RAPNO research rule adapters" interface is where that would attach.

use crate::error::TemporalError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The epoch a decision is being taken at.
///
/// An epoch rather than a timestamp, for the reason the rest of this crate uses epochs: there is no
/// clock, and a firewall whose cutoff moved with the host's date would admit different evidence on
/// different machines.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct DecisionEpoch(pub u64);

impl DecisionEpoch {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DecisionEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "epoch {}", self.0)
    }
}

/// What kind of thing happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// An imaging study. Requires clinical context to be interpretable.
    Imaging,
    /// A therapy administration or a change in it.
    Treatment,
    /// A specimen collection.
    Specimen,
    /// A laboratory or molecular result.
    Assay,
    /// A recorded clinical assessment.
    Clinical,
}

impl EventKind {
    fn requires_clinical_context(self) -> bool {
        matches!(self, EventKind::Imaging)
    }
}

/// The exposure context an imaging interpretation depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClinicalContext {
    /// Treatment in force at the time of the event.
    pub treatment_exposure: Vec<String>,
    /// Steroid exposure, called out separately because it is the specific confound 39.15 names.
    pub steroid_exposure: Option<String>,
}

impl ClinicalContext {
    pub fn on_treatment<I, S>(treatments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        ClinicalContext {
            treatment_exposure: treatments.into_iter().map(Into::into).collect(),
            steroid_exposure: None,
        }
    }

    /// An explicit statement that no treatment was in force. Different from an absent context: this
    /// is a recorded observation, that is a missing one.
    pub fn untreated() -> Self {
        ClinicalContext {
            treatment_exposure: Vec::new(),
            steroid_exposure: None,
        }
    }

    pub fn with_steroids(mut self, regimen: impl Into<String>) -> Self {
        self.steroid_exposure = Some(regimen.into());
        self
    }
}

/// One event on a subject's timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub event_id: String,
    pub subject: String,
    pub occurred_at: DecisionEpoch,
    pub kind: EventKind,
    /// The specimen or lesion identity this event is about, for the identity closure of 39.08.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clinical_context: Option<ClinicalContext>,
}

impl TimelineEvent {
    pub fn new(
        event_id: impl Into<String>,
        subject: impl Into<String>,
        occurred_at: DecisionEpoch,
        kind: EventKind,
    ) -> Self {
        TimelineEvent {
            event_id: event_id.into(),
            subject: subject.into(),
            occurred_at,
            kind,
            identity: None,
            clinical_context: None,
        }
    }

    pub fn about(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    pub fn in_context(mut self, context: ClinicalContext) -> Self {
        self.clinical_context = Some(context);
        self
    }
}

/// The visibility cutoff, enforced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TemporalFirewall {
    pub subject: String,
    pub decision_epoch: DecisionEpoch,
    admitted: Vec<TimelineEvent>,
}

#[derive(Deserialize)]
struct TemporalFirewallWire {
    subject: String,
    decision_epoch: DecisionEpoch,
    admitted: Vec<TimelineEvent>,
}

impl<'de> Deserialize<'de> for TemporalFirewall {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = TemporalFirewallWire::deserialize(deserializer)?;
        if wire.subject.trim().is_empty() || wire.subject.chars().any(char::is_control) {
            return Err(serde::de::Error::custom(
                TemporalError::InvalidIdentity {
                    field: "firewall subject",
                    value: wire.subject,
                },
            ));
        }
        let mut firewall = TemporalFirewall::new(wire.subject, wire.decision_epoch);
        for event in wire.admitted {
            firewall
                .admit(event)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(firewall)
    }
}

impl TemporalFirewall {
    pub fn new(subject: impl Into<String>, decision_epoch: DecisionEpoch) -> Self {
        TemporalFirewall {
            subject: subject.into(),
            decision_epoch,
            admitted: Vec::new(),
        }
    }

    /// Admit an event, or refuse it with the reason.
    ///
    /// Both refusals are checked here rather than at render time, because an event that reached the
    /// timeline is an event some later code will assume was checked.
    pub fn admit(&mut self, event: TimelineEvent) -> Result<(), TemporalError> {
        for (field, value) in [
            ("firewall subject", self.subject.as_str()),
            ("event id", event.event_id.as_str()),
            ("event subject", event.subject.as_str()),
        ] {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                return Err(TemporalError::InvalidIdentity {
                    field,
                    value: value.to_string(),
                });
            }
        }
        if event.subject != self.subject {
            return Err(TemporalError::SubjectMismatch {
                event: event.event_id,
                expected: self.subject.clone(),
                found: event.subject,
            });
        }
        if event.occurred_at > self.decision_epoch {
            return Err(TemporalError::FutureLeak {
                event: event.event_id,
                occurred: event.occurred_at.get(),
                decision: self.decision_epoch.get(),
            });
        }
        if event.kind.requires_clinical_context() && event.clinical_context.is_none() {
            return Err(TemporalError::ImagingWithoutClinicalContext {
                event: event.event_id,
            });
        }
        if self
            .admitted
            .iter()
            .any(|existing| existing.event_id == event.event_id)
        {
            return Err(TemporalError::DuplicateEvent {
                subject: self.subject.clone(),
                epoch: event.occurred_at.get(),
                event: event.event_id,
            });
        }
        self.admitted.push(event);
        Ok(())
    }

    /// The admitted timeline, ordered by epoch then event id.
    ///
    /// Sorted on read rather than on insert so the firewall's own storage order never becomes
    /// load-bearing, and so two firewalls fed the same events in different orders render the same
    /// timeline.
    pub fn timeline(&self) -> Vec<&TimelineEvent> {
        let mut events: Vec<&TimelineEvent> = self.admitted.iter().collect();
        events.sort_by(|left, right| {
            left.occurred_at
                .cmp(&right.occurred_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        events
    }

    /// The identities the admitted events cover, for the specimen and lesion closure.
    pub fn identities(&self) -> BTreeSet<String> {
        self.admitted
            .iter()
            .filter_map(|event| event.identity.clone())
            .collect()
    }

    /// The latest event at or before the decision epoch that could serve as a baseline.
    ///
    /// `None` when there is none, which 39.15 names as "baseline ambiguity" and which is a real
    /// state rather than a reason to reach for the nearest scan in either direction.
    pub fn baseline(&self, kind: EventKind) -> Option<&TimelineEvent> {
        self.timeline()
            .into_iter()
            .rfind(|event| event.kind == kind)
    }
}

/// Evidence that arrived after the decision epoch.
///
/// A separate type from the firewall, not a flag on it. Everything here is invisible to a capsule
/// builder by construction: there is no method on [`TemporalFirewall`] that reads a
/// [`RetrospectivePlane`], and a caller that wants to hand later evidence to the compiler has to
/// write code that visibly does so.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrospectivePlane {
    pub events: Vec<TimelineEvent>,
}

impl RetrospectivePlane {
    pub fn new() -> Self {
        RetrospectivePlane::default()
    }

    /// Accept an event, refusing one that belongs on the visible timeline instead.
    ///
    /// The check runs in the other direction too: putting a pre-cutoff event on the retrospective
    /// plane hides evidence the decision was entitled to, which is a quieter failure than a leak
    /// and just as wrong.
    pub fn reveal(
        &mut self,
        event: TimelineEvent,
        decision_epoch: DecisionEpoch,
    ) -> Result<(), TemporalError> {
        if event.occurred_at <= decision_epoch {
            return Err(TemporalError::RetrospectivePlaneRead(decision_epoch.get()));
        }
        self.events.push(event);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// One recorded diagnosis, with the wording used at the time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosisRecord {
    pub recorded_at: DecisionEpoch,
    /// The wording as recorded. 39.01 lists "historical wording when reclassification changed"
    /// among the things that must not be compressed away.
    pub wording: String,
    /// The classification system and version the wording belongs to.
    pub classification_system: String,
}

impl DiagnosisRecord {
    pub fn new(
        recorded_at: DecisionEpoch,
        wording: impl Into<String>,
        classification_system: impl Into<String>,
    ) -> Self {
        DiagnosisRecord {
            recorded_at,
            wording: wording.into(),
            classification_system: classification_system.into(),
        }
    }
}

/// An append-only diagnosis history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosisHistory {
    pub subject: String,
    records: Vec<DiagnosisRecord>,
}

#[derive(Deserialize)]
struct DiagnosisHistoryWire {
    subject: String,
    records: Vec<DiagnosisRecord>,
}

impl<'de> Deserialize<'de> for DiagnosisHistory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = DiagnosisHistoryWire::deserialize(deserializer)?;
        if wire.subject.trim().is_empty() {
            return Err(serde::de::Error::custom(
                "diagnosis history subject must not be empty",
            ));
        }
        if wire.records.is_empty() {
            return Err(serde::de::Error::custom(
                "diagnosis history must contain an original record",
            ));
        }
        if wire
            .records
            .windows(2)
            .any(|pair| pair[0].recorded_at >= pair[1].recorded_at)
        {
            return Err(serde::de::Error::custom(
                "diagnosis history records must be strictly ordered by epoch",
            ));
        }
        Ok(DiagnosisHistory {
            subject: wire.subject,
            records: wire.records,
        })
    }
}

impl DiagnosisHistory {
    pub fn new(subject: impl Into<String>, original: DiagnosisRecord) -> Self {
        DiagnosisHistory {
            subject: subject.into(),
            records: vec![original],
        }
    }

    /// Add a reclassification. Never replaces anything.
    ///
    /// A reclassification recorded at or before the previous record's epoch is refused: it would
    /// place a later belief earlier in the history, which is exactly the overwrite the invariant
    /// forbids, arriving through a timestamp instead of an assignment.
    pub fn reclassify(&mut self, record: DiagnosisRecord) -> Result<(), TemporalError> {
        let latest = self.records.last().expect("constructed with one record");
        if record.recorded_at <= latest.recorded_at {
            return Err(TemporalError::HistoricalDiagnosisOverwritten {
                subject: self.subject.clone(),
                epoch: record.recorded_at.get(),
                original: latest.recorded_at.get(),
            });
        }
        self.records.push(record);
        Ok(())
    }

    /// What was believed at an epoch. The question a historical decision's reader is asking.
    pub fn as_of(&self, epoch: DecisionEpoch) -> Option<&DiagnosisRecord> {
        self.records
            .iter()
            .rfind(|record| record.recorded_at <= epoch)
    }

    /// The latest classification. A different question from [`DiagnosisHistory::as_of`], and the
    /// reason both exist.
    pub fn current(&self) -> &DiagnosisRecord {
        self.records.last().expect("constructed with one record")
    }

    pub fn records(&self) -> &[DiagnosisRecord] {
        &self.records
    }

    pub fn was_reclassified(&self) -> bool {
        self.records.len() > 1
    }
}

/// The response vocabulary, including the states a clean vocabulary would force a compiler to round
/// away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum ResponseState {
    Complete,
    Partial,
    Stable,
    Progression,
    /// Some lesions improved and others progressed. A real, common state, and the one that becomes
    /// a progression when the vocabulary has no word for it.
    Mixed {
        improving: Vec<String>,
        worsening: Vec<String>,
    },
    /// Assessment could not be made: the confound is unresolved, the interval was wrong, the study
    /// was non-diagnostic. Not a synonym for stable.
    Unresolved {
        reason: String,
    },
}

impl ResponseState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResponseState::Complete => "complete",
            ResponseState::Partial => "partial",
            ResponseState::Stable => "stable",
            ResponseState::Progression => "progression",
            ResponseState::Mixed { .. } => "mixed",
            ResponseState::Unresolved { .. } => "unresolved",
        }
    }

    /// Whether the state is a determination at all.
    ///
    /// There is deliberately no `is_progression` shortcut that a mixed state could fall into. A
    /// caller that wants to know about progression must handle [`ResponseState::Mixed`] explicitly.
    pub fn is_determined(&self) -> bool {
        !matches!(self, ResponseState::Unresolved { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imaging(id: &str, at: u64) -> TimelineEvent {
        TimelineEvent::new(id, "subject/1", DecisionEpoch(at), EventKind::Imaging)
            .about("lesion/1")
            .in_context(ClinicalContext::on_treatment(["temozolomide"]).with_steroids("dex 4mg"))
    }

    fn firewall() -> TemporalFirewall {
        TemporalFirewall::new("subject/1", DecisionEpoch(10))
    }

    #[test]
    fn an_event_after_the_decision_epoch_is_refused_by_the_firewall() {
        let mut firewall = firewall();
        assert!(matches!(
            firewall.admit(imaging("mri/late", 11)),
            Err(TemporalError::FutureLeak {
                occurred: 11,
                decision: 10,
                ..
            })
        ));
        assert!(firewall.timeline().is_empty());
    }

    #[test]
    fn an_event_exactly_at_the_decision_epoch_is_admitted() {
        let mut firewall = firewall();
        assert!(firewall.admit(imaging("mri/now", 10)).is_ok());
        assert_eq!(firewall.timeline().len(), 1);
    }

    #[test]
    fn an_imaging_event_with_no_treatment_or_steroid_context_is_refused() {
        let mut firewall = firewall();
        let bare = TimelineEvent::new(
            "mri/bare",
            "subject/1",
            DecisionEpoch(4),
            EventKind::Imaging,
        );
        assert!(matches!(
            firewall.admit(bare),
            Err(TemporalError::ImagingWithoutClinicalContext { .. })
        ));
    }

    #[test]
    fn an_explicitly_untreated_context_satisfies_the_requirement_and_an_absent_one_does_not() {
        let mut firewall = firewall();
        let untreated =
            TimelineEvent::new("mri/pre", "subject/1", DecisionEpoch(1), EventKind::Imaging)
                .in_context(ClinicalContext::untreated());
        assert!(firewall.admit(untreated).is_ok());
    }

    #[test]
    fn a_non_imaging_event_needs_no_clinical_context() {
        let mut firewall = firewall();
        let assay =
            TimelineEvent::new("assay/idh", "subject/1", DecisionEpoch(2), EventKind::Assay);
        assert!(firewall.admit(assay).is_ok());
    }

    #[test]
    fn an_event_for_another_subject_cannot_cross_the_firewall() {
        let mut firewall = firewall();
        let event = TimelineEvent::new(
            "assay/other",
            "subject/2",
            DecisionEpoch(2),
            EventKind::Assay,
        );
        assert!(matches!(
            firewall.admit(event),
            Err(TemporalError::SubjectMismatch {
                expected,
                found,
                ..
            }) if expected == "subject/1" && found == "subject/2"
        ));
        assert!(firewall.timeline().is_empty());
    }

    #[test]
    fn deserializing_a_tampered_firewall_replays_admission_checks() {
        let mut firewall = firewall();
        firewall
            .admit(TimelineEvent::new(
                "assay/1",
                "subject/1",
                DecisionEpoch(2),
                EventKind::Assay,
            ))
            .expect("event is admitted");
        let mut encoded = serde_json::to_value(&firewall).expect("firewall serializes");
        encoded["admitted"][0]["occurred_at"] = serde_json::json!(20);
        assert!(serde_json::from_value::<TemporalFirewall>(encoded).is_err());
    }

    #[test]
    fn the_same_event_admitted_twice_is_refused() {
        let mut firewall = firewall();
        firewall.admit(imaging("mri/1", 4)).expect("admits");
        assert!(matches!(
            firewall.admit(imaging("mri/1", 4)),
            Err(TemporalError::DuplicateEvent { .. })
        ));
    }

    #[test]
    fn the_timeline_is_the_same_however_the_events_arrived() {
        let mut forwards = firewall();
        let mut backwards = firewall();
        for at in [2u64, 5, 9] {
            forwards
                .admit(imaging(&format!("mri/{at}"), at))
                .expect("admits");
        }
        for at in [9u64, 5, 2] {
            backwards
                .admit(imaging(&format!("mri/{at}"), at))
                .expect("admits");
        }
        let left: Vec<&str> = forwards
            .timeline()
            .iter()
            .map(|event| event.event_id.as_str())
            .collect();
        let right: Vec<&str> = backwards
            .timeline()
            .iter()
            .map(|event| event.event_id.as_str())
            .collect();
        assert_eq!(left, right);
    }

    #[test]
    fn the_baseline_is_the_latest_admitted_study_and_is_absent_when_there_is_none() {
        let mut firewall = firewall();
        assert!(firewall.baseline(EventKind::Imaging).is_none());
        firewall.admit(imaging("mri/1", 2)).expect("admits");
        firewall.admit(imaging("mri/2", 7)).expect("admits");
        assert_eq!(
            firewall
                .baseline(EventKind::Imaging)
                .map(|event| event.event_id.as_str()),
            Some("mri/2")
        );
    }

    #[test]
    fn later_evidence_goes_to_the_retrospective_plane_and_never_to_the_timeline() {
        let mut firewall = firewall();
        let mut plane = RetrospectivePlane::new();
        plane
            .reveal(imaging("mri/late", 20), DecisionEpoch(10))
            .expect("reveals");
        assert!(firewall.admit(imaging("mri/late", 20)).is_err());
        assert!(!plane.is_empty());
        assert!(firewall.timeline().is_empty());
    }

    #[test]
    fn a_pre_cutoff_event_may_not_be_hidden_on_the_retrospective_plane() {
        let mut plane = RetrospectivePlane::new();
        assert!(matches!(
            plane.reveal(imaging("mri/early", 3), DecisionEpoch(10)),
            Err(TemporalError::RetrospectivePlaneRead(10))
        ));
    }

    fn history() -> DiagnosisHistory {
        DiagnosisHistory::new(
            "subject/1",
            DiagnosisRecord::new(DecisionEpoch(2), "anaplastic astrocytoma", "WHO 2007"),
        )
    }

    #[test]
    fn a_reclassification_adds_a_record_and_leaves_the_historical_wording_intact() {
        let mut history = history();
        history
            .reclassify(DiagnosisRecord::new(
                DecisionEpoch(9),
                "astrocytoma, IDH-mutant, grade 3",
                "WHO 2021",
            ))
            .expect("reclassifies");
        assert_eq!(history.records().len(), 2);
        assert_eq!(history.records()[0].wording, "anaplastic astrocytoma");
        assert!(history.was_reclassified());
    }

    #[test]
    fn a_historical_decision_reads_the_wording_that_was_current_when_it_was_taken() {
        let mut history = history();
        history
            .reclassify(DiagnosisRecord::new(
                DecisionEpoch(9),
                "astrocytoma, IDH-mutant, grade 3",
                "WHO 2021",
            ))
            .expect("reclassifies");
        assert_eq!(
            history.as_of(DecisionEpoch(5)).map(|r| r.wording.as_str()),
            Some("anaplastic astrocytoma")
        );
        assert_eq!(
            history.current().wording,
            "astrocytoma, IDH-mutant, grade 3"
        );
    }

    #[test]
    fn a_reclassification_backdated_over_an_existing_record_is_refused() {
        let mut history = history();
        assert!(matches!(
            history.reclassify(DiagnosisRecord::new(
                DecisionEpoch(1),
                "glioblastoma",
                "WHO 2021"
            )),
            Err(TemporalError::HistoricalDiagnosisOverwritten { .. })
        ));
        assert_eq!(history.records().len(), 1);
    }

    #[test]
    fn deserializing_an_empty_diagnosis_history_is_refused() {
        let result = serde_json::from_value::<DiagnosisHistory>(serde_json::json!({
            "subject": "subject/1",
            "records": []
        }));
        assert!(result.is_err());
    }

    #[test]
    fn deserializing_backordered_diagnosis_records_is_refused() {
        let result = serde_json::from_value::<DiagnosisHistory>(serde_json::json!({
            "subject": "subject/1",
            "records": [
                {
                    "recorded_at": 9,
                    "wording": "later",
                    "classification_system": "WHO 2021"
                },
                {
                    "recorded_at": 2,
                    "wording": "earlier",
                    "classification_system": "WHO 2007"
                }
            ]
        }));
        assert!(result.is_err());
    }

    #[test]
    fn a_decision_predating_every_record_reads_no_diagnosis_rather_than_the_earliest_one() {
        assert!(history().as_of(DecisionEpoch(1)).is_none());
    }

    #[test]
    fn a_mixed_response_is_representable_and_is_not_a_progression() {
        let mixed = ResponseState::Mixed {
            improving: vec!["lesion/1".to_string()],
            worsening: vec!["lesion/2".to_string()],
        };
        assert_eq!(mixed.as_str(), "mixed");
        assert!(mixed.is_determined());
        assert_ne!(mixed, ResponseState::Progression);
    }

    #[test]
    fn an_unresolved_response_is_not_a_stable_one() {
        let unresolved = ResponseState::Unresolved {
            reason: "steroid confound unresolved at this interval".to_string(),
        };
        assert!(!unresolved.is_determined());
        assert!(ResponseState::Stable.is_determined());
        assert_ne!(unresolved, ResponseState::Stable);
    }

    #[test]
    fn the_admitted_identities_are_reported_for_the_specimen_closure() {
        let mut firewall = firewall();
        firewall.admit(imaging("mri/1", 2)).expect("admits");
        firewall
            .admit(
                TimelineEvent::new("spec/1", "subject/1", DecisionEpoch(3), EventKind::Specimen)
                    .about("specimen/a"),
            )
            .expect("admits");
        assert_eq!(
            firewall.identities(),
            BTreeSet::from(["lesion/1".to_string(), "specimen/a".to_string()])
        );
    }

    #[test]
    fn a_response_state_survives_a_json_round_trip_with_its_variant_intact() {
        let mixed = ResponseState::Mixed {
            improving: vec!["lesion/1".to_string()],
            worsening: vec!["lesion/2".to_string()],
        };
        let text = serde_json::to_string(&mixed).expect("serialises");
        assert!(text.contains("mixed"));
        let back: ResponseState = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, mixed);
    }
}
