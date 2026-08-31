//! Policy as a scope-indexed fiber.
//!
//! This is the headline of blueprint 43.33: access, consent, purpose, residency, export and
//! disclosure constraints are "scope-indexed semantics enforced during compilation and
//! execution—not as a final response filter". [`PolicyLattice::admits`] is callable while a
//! compiler is choosing what to select, before any Decision Section exists and long before
//! anything is rendered, and [`PolicyLattice::screen`] is the same check applied to a candidate
//! set so refused evidence never reaches the selector's scoring loop at all.
//!
//! ## The fiber
//!
//! Rules attach at scopes. A rule declared at `{residency: eu}` governs every scope that refines
//! it — `{residency: eu, cohort: GLIOMA-EU}`, `{residency: eu, cohort: GLIOMA-EU, subject: S001}` —
//! and the effective label at a scope is the [`PolicyLabel::join`] of every rule that applies
//! there. Policy restricts along the same refinement morphisms `bioprism_scope` already defines,
//! which is what makes this a fiber over the scope base rather than a lookup table beside it.
//!
//! Two consequences, both tested below and both worth stating because they pull in opposite
//! directions:
//!
//! * **Restriction monotonicity.** If `narrow` refines `wide` and at least one rule applies at
//!   `wide`, then the label at `narrow` is at least as restrictive as the label at `wide`. Every
//!   rule applicable at `wide` is applicable at `narrow` by transitivity of refinement, so the
//!   join at `narrow` is over a superset. Policy can only tighten as you descend.
//! * **Deny by default is not monotone, and cannot be.** A scope no rule claims is *unlabelled*,
//!   and every request against it is refused. That is 43.33's "unknown policy defaults to no
//!   export" taken to its conclusion. It is not expressible as a rule at the empty scope, because
//!   [`PolicyLabel::unknown`] is more restrictive than any real rule and joining it everywhere
//!   would deny everything. So the unlabelled state sits outside the lattice, and the two facts
//!   are stated separately rather than fudged into one.
//!
//! ## Tags
//!
//! `bioprism_world` facts carry free-form tags, and a world may encode policy in them. Registered
//! tags contribute a label that is *joined* with the scope's, so a tag can only tighten. An
//! unregistered tag contributes nothing and is reported by [`PolicyLattice::unregistered_tags`],
//! because silently honouring an unknown tag would let world content edit the policy — the attack
//! 13.12 and 36.08 both name.
//!
//! Not implemented: rule precedence or override. There is no way to say "this rule wins over that
//! one", only join. An override mechanism is how policy engines acquire the property that adding a
//! rule can loosen the result, and 43.33's invariant "policy requirements can only become stricter
//! downstream" reads as a prohibition on exactly that.

use crate::consent::Consent;
use crate::decision::{Admission, Decision, ExecutionMode, Obligation, Refusal};
use crate::error::PolicyError;
use crate::label::{ExportPolicy, PolicyLabel, Retention};
use crate::request::Request;
use crate::trace::{PolicyTrace, TraceEntry};
use bioprism_ids::ContentHash;
use bioprism_scope::{ScopeKey, Timestamp};
use bioprism_world::Fact;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const NANOS_PER_DAY: i128 = 86_400 * 1_000_000_000;

/// A policy rule bound to a scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    /// Versioned so that a change is visible in [`PolicyLattice::version`] and therefore in every
    /// cache key derived from it.
    pub version: u32,
    /// The coarsest scope this rule governs. Every refinement of it inherits the rule.
    pub scope: ScopeKey,
    pub label: PolicyLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<Consent>,
}

impl PolicyRule {
    pub fn new(id: impl Into<String>, version: u32, scope: ScopeKey, label: PolicyLabel) -> Self {
        PolicyRule {
            id: id.into(),
            version,
            scope,
            label,
            consent: None,
        }
    }

    pub fn under_consent(mut self, consent: Consent) -> Self {
        self.consent = Some(consent);
        self
    }

    /// Whether this rule governs `scope`.
    pub fn governs(&self, scope: &ScopeKey) -> bool {
        scope.refines(&self.scope)
    }

    /// The rule's contribution to the effective label, with its consent folded in.
    ///
    /// Folding consent into the label rather than checking it separately is what makes a
    /// prohibition propagate: the derived artifact's label already lacks the purpose, so every
    /// downstream join lacks it too, with no second traversal of the consent registry.
    pub fn contribution(&self) -> PolicyLabel {
        match &self.consent {
            None => self.label.clone(),
            Some(consent) => self
                .label
                .clone()
                .with_purposes(self.label.purposes.intersect(&consent.effective_purposes())),
        }
    }
}

/// The effective label at a scope, with the rules that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelResolution {
    pub label: PolicyLabel,
    /// Ids of contributing rules, in registration order. Empty means unlabelled.
    pub rules: Vec<String>,
    /// Consents that must be checked against the request's purpose and decision time.
    pub consents: Vec<Consent>,
}

impl LabelResolution {
    /// True when no rule claimed the scope. The label is then [`PolicyLabel::unknown`], which is
    /// reported for completeness but should not be relied on as a value: the correct handling is
    /// to refuse and say so.
    pub fn is_unlabelled(&self) -> bool {
        self.rules.is_empty()
    }
}

/// One fact that survived screening, with its admission.
#[derive(Debug, Clone, PartialEq)]
pub struct AdmittedFact<'a> {
    pub fact: &'a Fact,
    pub admission: Admission,
}

/// One fact that did not, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusedFact {
    pub fact_id: String,
    pub refusal: Refusal,
}

/// The result of screening a candidate set before selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Screening<'a> {
    pub admitted: Vec<AdmittedFact<'a>>,
    pub refused: Vec<RefusedFact>,
    pub trace: PolicyTrace,
}

impl<'a> Screening<'a> {
    /// The label any artifact derived from the admitted set must carry.
    ///
    /// The join, not the maximum classification: two facts each admissible on their own can
    /// combine into something no channel will take, and this is where that becomes visible.
    pub fn derived_label(&self) -> PolicyLabel {
        PolicyLabel::join_all(self.admitted.iter().map(|item| &item.admission.label))
    }

    pub fn facts(&self) -> impl Iterator<Item = &'a Fact> + '_ {
        self.admitted.iter().map(|item| item.fact)
    }

    /// Every obligation owed across the admitted set, deduplicated.
    pub fn obligations(&self) -> Vec<Obligation> {
        let mut all: Vec<Obligation> = Vec::new();
        for item in &self.admitted {
            for obligation in &item.admission.obligations {
                if !all.contains(obligation) {
                    all.push(obligation.clone());
                }
            }
        }
        all
    }

    pub fn is_complete(&self) -> bool {
        self.refused.is_empty()
    }
}

/// The scope-indexed policy fiber.
#[derive(Debug, Clone, Default)]
pub struct PolicyLattice {
    rules: Vec<PolicyRule>,
    tag_labels: BTreeMap<String, PolicyLabel>,
}

impl PolicyLattice {
    /// An empty lattice: every scope is unlabelled and every request is refused.
    pub fn new() -> Self {
        PolicyLattice::default()
    }

    /// Adds a rule, rejecting a second registration of the same id and version with different
    /// content. Re-registering identical content is idempotent so that composing rule packs does
    /// not require deduplication by the caller.
    pub fn register(&mut self, rule: PolicyRule) -> Result<(), PolicyError> {
        if let Some(existing) = self
            .rules
            .iter()
            .find(|candidate| candidate.id == rule.id && candidate.version == rule.version)
        {
            if *existing != rule {
                return Err(PolicyError::RuleVersionConflict {
                    id: rule.id,
                    version: rule.version,
                });
            }
            return Ok(());
        }
        self.rules.push(rule);
        Ok(())
    }

    pub fn with_rule(mut self, rule: PolicyRule) -> Result<Self, PolicyError> {
        self.register(rule)?;
        Ok(self)
    }

    /// Registers the label a world tag contributes.
    ///
    /// The contribution is joined, so a tag can tighten a fact's label and can never loosen it.
    pub fn register_tag(&mut self, tag: impl Into<String>, label: PolicyLabel) {
        self.tag_labels.insert(tag.into(), label);
    }

    pub fn with_tag(mut self, tag: impl Into<String>, label: PolicyLabel) -> Self {
        self.register_tag(tag, label);
        self
    }

    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// A content hash of the whole rule set.
    ///
    /// 43.33: "policy changes invalidate cached sections and messages." A cache entry stamped with
    /// this version is invalid the moment any rule, consent or tag mapping changes, without anyone
    /// having to work out which entries were affected.
    pub fn version(&self) -> String {
        let body = json!({
            "rules": self.rules,
            "tag_labels": self.tag_labels,
        });
        ContentHash::of_value(&body)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default()
    }

    /// Rules governing a scope, in registration order.
    pub fn applicable(&self, scope: &ScopeKey) -> Vec<&PolicyRule> {
        self.rules
            .iter()
            .filter(|rule| rule.governs(scope))
            .collect()
    }

    /// The effective label at a scope: the join of every applicable rule's contribution.
    pub fn resolve(&self, scope: &ScopeKey) -> LabelResolution {
        let applicable = self.applicable(scope);
        if applicable.is_empty() {
            return LabelResolution {
                label: PolicyLabel::unknown(),
                rules: Vec::new(),
                consents: Vec::new(),
            };
        }
        let mut label = PolicyLabel::public();
        let mut rules = Vec::new();
        let mut consents = Vec::new();
        for rule in applicable {
            label = label.join(&rule.contribution());
            rules.push(format!("{}@{}", rule.id, rule.version));
            if let Some(consent) = &rule.consent {
                consents.push(consent.clone());
            }
        }
        LabelResolution {
            label,
            rules,
            consents,
        }
    }

    /// The effective label of a world fact: its scope's label joined with every registered tag.
    pub fn resolve_fact(&self, fact: &Fact) -> LabelResolution {
        let mut resolution = self.resolve(&fact.scope);
        let mut tagged = Vec::new();
        for tag in &fact.tags {
            if let Some(label) = self.tag_labels.get(tag.as_str()) {
                tagged.push((tag.clone(), label));
            }
        }
        if tagged.is_empty() {
            return resolution;
        }
        if resolution.is_unlabelled() {
            resolution.label = PolicyLabel::public();
        }
        for (tag, label) in tagged {
            resolution.label = resolution.label.join(label);
            resolution.rules.push(format!("tag:{tag}"));
        }
        resolution
    }

    /// Tags on a fact that no rule explains.
    ///
    /// Reported rather than honoured. A conformance gate that wants tag-encoded policy to be
    /// complete can fail on a non-empty result; a gate that does not care can ignore it. What it
    /// must not do is let an unknown tag silently mean nothing while looking like it meant
    /// something.
    pub fn unregistered_tags(&self, fact: &Fact) -> Vec<String> {
        fact.tags
            .iter()
            .filter(|tag| !self.tag_labels.contains_key(tag.as_str()))
            .cloned()
            .collect()
    }

    /// The compilation-time judgement: may this request touch evidence valid in this scope?
    ///
    /// Called by a compiler *while selecting*. Nothing here depends on a rendered answer, a
    /// Decision Section, or the size of the result, so a selector can consult it per candidate and
    /// never place refused evidence in scoring range.
    pub fn admits(&self, evidence_scope: &ScopeKey, request: &Request) -> Decision {
        let resolution = self.resolve(evidence_scope);
        self.judge(&resolution, request, evidence_scope)
    }

    /// The same judgement for a world fact, including its tags.
    pub fn admits_fact(&self, fact: &Fact, request: &Request) -> Decision {
        let resolution = self.resolve_fact(fact);
        self.judge(&resolution, request, &fact.scope)
    }

    /// Screens a candidate set, keeping the refusals rather than dropping them.
    ///
    /// The refused list and the trace are the difference between a policy filter and a silent
    /// omission: a caller can convert them into an [`bioprism_section::OmissionGroup`] classified
    /// [`bioprism_section::InfluenceClass::InaccessibleByPolicy`] and a set of
    /// [`bioprism_section::UnresolvedObligation`]s, so the decision states its gap.
    pub fn screen<'a, I>(&self, facts: I, request: &Request) -> Screening<'a>
    where
        I: IntoIterator<Item = &'a Fact>,
    {
        let mut admitted = Vec::new();
        let mut refused = Vec::new();
        let mut trace = PolicyTrace::new(self.version());

        for fact in facts {
            let subject = fact.id.as_str().to_string();
            match self.admits_fact(fact, request) {
                Decision::Admit(admission) => {
                    trace.push(TraceEntry::admitted(
                        subject,
                        describe_scope(&fact.scope),
                        &admission,
                    ));
                    admitted.push(AdmittedFact { fact, admission });
                }
                Decision::Refuse(refusal) => {
                    trace.push(TraceEntry::refused(
                        subject.clone(),
                        describe_scope(&fact.scope),
                        refusal.clone(),
                    ));
                    refused.push(RefusedFact {
                        fact_id: subject,
                        refusal,
                    });
                }
            }
        }

        Screening {
            admitted,
            refused,
            trace,
        }
    }

    fn judge(&self, resolution: &LabelResolution, request: &Request, scope: &ScopeKey) -> Decision {
        if resolution.is_unlabelled() {
            return Decision::Refuse(Refusal::UnlabelledEvidence {
                scope: describe_scope(scope),
            });
        }
        let label = &resolution.label;

        for consent in &resolution.consents {
            if let Err(refusal) = consent.check(request.purpose, request.at) {
                return Decision::Refuse(refusal);
            }
        }

        if !label.purposes.admits(request.purpose) {
            return Decision::Refuse(Refusal::PurposeNotPermittedByLabel {
                requested: request.purpose,
                permitted: label.purposes.clone(),
            });
        }

        let clearance = &request.principal.clearance;
        if label.classification > clearance.max_classification {
            return Decision::Refuse(Refusal::ClearanceInsufficient {
                principal: request.principal.id.clone(),
                required: label.classification,
                held: clearance.max_classification,
            });
        }
        let missing = clearance.missing_compartments(&label.compartments);
        if !missing.is_empty() {
            return Decision::Refuse(Refusal::CompartmentNotCleared {
                principal: request.principal.id.clone(),
                missing,
            });
        }

        if label.classification > request.channel.ceiling() {
            return Decision::Refuse(Refusal::ChannelCeilingExceeded {
                channel: request.channel,
                classification: label.classification,
                ceiling: request.channel.ceiling(),
            });
        }

        if request.channel.is_egress() && label.export == ExportPolicy::NoExport {
            return Decision::Refuse(Refusal::ExportForbidden {
                export: label.export,
                detail: format!("channel {} leaves the steward's control", request.channel),
            });
        }

        if request.channel.is_persistent() && label.retention == Retention::Days(0) {
            return Decision::Refuse(Refusal::RetentionForbidsPersistence {
                channel: request.channel,
            });
        }

        let mode = if label.residency.admits(&request.principal.site) {
            ExecutionMode::Central
        } else {
            match label.residency.any_permitted_site() {
                Some(site) if request.channel == crate::request::Channel::LocalCompute => {
                    ExecutionMode::Local { site }
                }
                Some(site) => ExecutionMode::Federated { site },
                None => {
                    return Decision::Refuse(Refusal::NoLegalExecutionPath {
                        detail: format!(
                            "the joined residency of scope {} permits no site",
                            describe_scope(scope)
                        ),
                    })
                }
            }
        };

        Decision::Admit(Admission {
            label: label.clone(),
            obligations: obligations_for(label, request, &mode, &self.version()),
            mode,
            rules: resolution.rules.clone(),
        })
    }
}

fn obligations_for(
    label: &PolicyLabel,
    request: &Request,
    mode: &ExecutionMode,
    version: &str,
) -> Vec<Obligation> {
    let mut obligations = vec![Obligation::RecordAccess];
    let push = |obligation: Obligation, into: &mut Vec<Obligation>| {
        if !into.contains(&obligation) {
            into.push(obligation);
        }
    };

    match mode {
        ExecutionMode::Central => {}
        ExecutionMode::Local { site } => push(
            Obligation::ComputeAt { site: site.clone() },
            &mut obligations,
        ),
        ExecutionMode::Federated { site } => {
            push(
                Obligation::ComputeAt { site: site.clone() },
                &mut obligations,
            );
            push(Obligation::AggregatesOnly, &mut obligations);
            push(Obligation::EmitLocalCertificate, &mut obligations);
        }
    }

    if request.channel.is_egress() && label.export == ExportPolicy::AggregatesOnly {
        push(Obligation::AggregatesOnly, &mut obligations);
    }

    if label.min_cell_size > 0 {
        push(
            Obligation::SuppressSmallCells {
                threshold: label.min_cell_size,
            },
            &mut obligations,
        );
    }

    if request.channel.is_persistent() {
        if let Some(days) = label.retention.days() {
            let at = Timestamp::from_nanos_utc(
                request.at.as_nanos_utc() + i128::from(days) * NANOS_PER_DAY,
            );
            push(
                Obligation::DeleteBy {
                    at: at.to_rfc3339(),
                },
                &mut obligations,
            );
        }
    }

    if request.channel == crate::request::Channel::Cache {
        push(
            Obligation::KeyCacheToPolicyVersion {
                version: version.to_string(),
            },
            &mut obligations,
        );
    }

    obligations
}

/// A stable, human-readable rendering of a scope, for refusals and traces.
pub(crate) fn describe_scope(scope: &ScopeKey) -> String {
    let parts: Vec<String> = scope
        .iter()
        .map(|(dimension, value)| format!("{dimension}={}", value.describe()))
        .collect();
    if parts.is_empty() {
        "{}".to_string()
    } else {
        format!("{{{}}}", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::Classification;
    use crate::purpose::{Purpose, PurposeSet};
    use crate::request::{Channel, Clearance, Principal};
    use crate::residency::Residency;
    use serde_json::Value;

    fn at(text: &str) -> Timestamp {
        Timestamp::parse(text).expect("fixture timestamp parses")
    }

    fn now() -> Timestamp {
        at("2026-08-08T00:00:00Z")
    }

    fn fact(id: &str, scope: Value, tags: &[&str]) -> Fact {
        Fact::from_json(&json!({
            "id": id,
            "provides": "some_variable",
            "value": 1,
            "scope": scope,
            "tags": tags,
            "provenance": ["fixture"],
        }))
        .expect("fixture fact parses")
    }

    fn investigator() -> Principal {
        Principal::new("p.investigator", "investigator", "eu")
            .with_clearance(Clearance::up_to(Classification::ControlledGenomicOrImaging))
    }

    fn eu_lattice() -> PolicyLattice {
        let consent = Consent::new(
            "consent.glioma-eu.v3",
            PurposeSet::of([Purpose::ResearchAnalysis, Purpose::MethodDevelopment]),
        );
        PolicyLattice::new()
            .with_rule(
                PolicyRule::new(
                    "rule.eu-baseline",
                    1,
                    ScopeKey::new().exact("residency", "eu"),
                    PolicyLabel::public()
                        .with_classification(Classification::InstitutionalConfidential)
                        .with_residency(Residency::only(["eu"]))
                        .with_export(ExportPolicy::AggregatesOnly),
                )
                .under_consent(consent),
            )
            .expect("rule registers")
            .with_rule(PolicyRule::new(
                "rule.eu-genomic",
                1,
                ScopeKey::new()
                    .exact("residency", "eu")
                    .exact("assay", "wgs"),
                PolicyLabel::public()
                    .with_classification(Classification::ControlledGenomicOrImaging)
                    .with_min_cell_size(11),
            ))
            .expect("rule registers")
    }

    fn research_request(channel: Channel) -> Request {
        Request::new(investigator(), Purpose::ResearchAnalysis, channel, now())
    }

    #[test]
    fn a_narrower_scope_never_receives_a_weaker_label_than_a_coarser_one() {
        let lattice = eu_lattice();
        let wide = ScopeKey::new().exact("residency", "eu");
        let narrow = ScopeKey::new()
            .exact("residency", "eu")
            .exact("assay", "wgs")
            .exact("subject", "S001");

        assert!(narrow.refines(&wide));
        let coarse = lattice.resolve(&wide);
        let fine = lattice.resolve(&narrow);

        assert!(!coarse.is_unlabelled());
        assert!(
            coarse.label.flows_to(&fine.label),
            "descending the fiber may only tighten policy"
        );
        assert!(fine.label.classification > coarse.label.classification);
    }

    #[test]
    fn a_scope_no_rule_claims_is_unlabelled_and_refuses_every_request() {
        let lattice = eu_lattice();
        let orphan = ScopeKey::new()
            .exact("residency", "us")
            .exact("cohort", "X");

        let resolution = lattice.resolve(&orphan);
        assert!(resolution.is_unlabelled());

        for channel in Channel::ALL {
            let decision = lattice.admits(&orphan, &research_request(channel));
            assert!(matches!(
                decision.refusal(),
                Some(Refusal::UnlabelledEvidence { .. })
            ));
        }
    }

    #[test]
    fn a_purpose_outside_the_consent_is_refused_naming_the_constraint() {
        let lattice = eu_lattice();
        let scope = ScopeKey::new().exact("residency", "eu");
        let request = Request::new(
            investigator(),
            Purpose::ModelTraining,
            Channel::LocalCompute,
            now(),
        );

        let decision = lattice.admits(&scope, &request);

        let refusal = decision.refusal().expect("training is not consented");
        assert_eq!(refusal.constraint(), "purpose_not_consented");
        assert!(refusal.to_string().contains("model_training"));
    }

    #[test]
    fn a_clearance_below_the_evidence_classification_is_refused_naming_both_levels() {
        let lattice = eu_lattice();
        let scope = ScopeKey::new()
            .exact("residency", "eu")
            .exact("assay", "wgs");
        let junior = Principal::new("p.junior", "analyst", "eu")
            .with_clearance(Clearance::up_to(Classification::InstitutionalConfidential));
        let request = Request::new(
            junior,
            Purpose::ResearchAnalysis,
            Channel::LocalCompute,
            now(),
        );

        match lattice.admits(&scope, &request).refusal() {
            Some(Refusal::ClearanceInsufficient { required, held, .. }) => {
                assert_eq!(*required, Classification::ControlledGenomicOrImaging);
                assert_eq!(*held, Classification::InstitutionalConfidential);
            }
            other => panic!("expected a clearance refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_compartment_is_named_even_when_the_clearance_ceiling_is_high() {
        let lattice = PolicyLattice::new()
            .with_rule(PolicyRule::new(
                "rule.peds",
                1,
                ScopeKey::new().exact("cohort", "PEDS"),
                PolicyLabel::public().with_compartment("pediatric"),
            ))
            .expect("rule registers");
        let scope = ScopeKey::new().exact("cohort", "PEDS");
        let senior = Principal::new("p.senior", "pi", "eu")
            .with_clearance(Clearance::up_to(Classification::RestrictedDualUse));
        let request = Request::new(
            senior,
            Purpose::ResearchAnalysis,
            Channel::LocalCompute,
            now(),
        );

        match lattice.admits(&scope, &request).refusal() {
            Some(Refusal::CompartmentNotCleared { missing, .. }) => {
                assert_eq!(missing, &vec!["pediatric".to_string()]);
            }
            other => panic!("expected a compartment refusal, got {other:?}"),
        }
    }

    #[test]
    fn controlled_evidence_is_refused_on_the_cache_and_public_channels() {
        let lattice = eu_lattice();
        let scope = ScopeKey::new()
            .exact("residency", "eu")
            .exact("assay", "wgs");

        for channel in [
            Channel::Cache,
            Channel::ModelPrompt,
            Channel::PublicArtifact,
        ] {
            let decision = lattice.admits(&scope, &research_request(channel));
            assert!(
                matches!(
                    decision.refusal(),
                    Some(Refusal::ChannelCeilingExceeded { .. })
                ),
                "channel {channel} must not carry controlled evidence"
            );
        }

        assert!(lattice
            .admits(&scope, &research_request(Channel::LocalCompute))
            .is_admitted());
    }

    #[test]
    fn evidence_outside_the_principals_jurisdiction_becomes_a_federated_plan_not_a_refusal() {
        let lattice = eu_lattice();
        let scope = ScopeKey::new().exact("residency", "eu");
        let offshore = Principal::new("p.us", "analyst", "us")
            .with_clearance(Clearance::up_to(Classification::ControlledGenomicOrImaging));
        let request = Request::new(
            offshore,
            Purpose::ResearchAnalysis,
            Channel::FederatedAggregate,
            now(),
        );

        let admission = lattice
            .admits(&scope, &request)
            .admission()
            .cloned()
            .expect("a federated path exists");

        assert_eq!(
            admission.mode,
            ExecutionMode::Federated {
                site: crate::residency::Jurisdiction::new("eu")
            }
        );
        assert!(admission.obligations.contains(&Obligation::AggregatesOnly));
        assert!(admission
            .obligations
            .contains(&Obligation::EmitLocalCertificate));
    }

    #[test]
    fn evidence_whose_join_permits_no_site_yields_a_typed_no_legal_execution_path() {
        let lattice = PolicyLattice::new()
            .with_rule(PolicyRule::new(
                "rule.eu",
                1,
                ScopeKey::new().exact("cohort", "POOLED"),
                PolicyLabel::public().with_residency(Residency::only(["eu"])),
            ))
            .expect("registers")
            .with_rule(PolicyRule::new(
                "rule.us",
                1,
                ScopeKey::new().exact("cohort", "POOLED"),
                PolicyLabel::public().with_residency(Residency::only(["us"])),
            ))
            .expect("registers");
        let scope = ScopeKey::new().exact("cohort", "POOLED");

        match lattice
            .admits(&scope, &research_request(Channel::LocalCompute))
            .refusal()
        {
            Some(Refusal::NoLegalExecutionPath { .. }) => {}
            other => panic!("expected no-legal-path, got {other:?}"),
        }
    }

    #[test]
    fn changing_a_rule_changes_the_lattice_version_so_stamped_caches_invalidate() {
        let before = eu_lattice();
        let after = before
            .clone()
            .with_rule(PolicyRule::new(
                "rule.extra",
                1,
                ScopeKey::new().exact("residency", "eu"),
                PolicyLabel::public().with_min_cell_size(20),
            ))
            .expect("registers");

        assert_ne!(before.version(), after.version());
        assert_eq!(before.version(), eu_lattice().version());
        assert!(!before.version().is_empty());
    }

    #[test]
    fn registering_the_same_id_and_version_with_different_content_is_a_typed_error() {
        let mut lattice = PolicyLattice::new();
        let scope = ScopeKey::new().exact("cohort", "A");
        lattice
            .register(PolicyRule::new(
                "rule.a",
                1,
                scope.clone(),
                PolicyLabel::public(),
            ))
            .expect("first registration");

        lattice
            .register(PolicyRule::new(
                "rule.a",
                1,
                scope.clone(),
                PolicyLabel::public(),
            ))
            .expect("identical re-registration is idempotent");

        let conflict = lattice.register(PolicyRule::new(
            "rule.a",
            1,
            scope,
            PolicyLabel::public().with_compartment("pediatric"),
        ));
        assert!(matches!(
            conflict,
            Err(PolicyError::RuleVersionConflict { .. })
        ));
    }

    #[test]
    fn screening_refuses_during_selection_so_a_refused_fact_is_never_returned() {
        let lattice = eu_lattice();
        let admissible = fact("fact.open", json!({"residency": "eu"}), &[]);
        let controlled = fact(
            "fact.genomic",
            json!({"residency": "eu", "assay": "wgs"}),
            &[],
        );
        let orphan = fact("fact.orphan", json!({"cohort": "UNKNOWN"}), &[]);
        let candidates = [&admissible, &controlled, &orphan];

        let screening = lattice.screen(candidates, &research_request(Channel::ModelPrompt));

        let ids: Vec<&str> = screening.facts().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["fact.open"]);
        assert_eq!(screening.refused.len(), 2);
        assert!(!screening.is_complete());
        assert_eq!(screening.trace.entries().len(), 3);
    }

    #[test]
    fn the_derived_label_of_a_screening_is_the_join_of_what_was_admitted() {
        let lattice = PolicyLattice::new()
            .with_rule(PolicyRule::new(
                "rule.eu",
                1,
                ScopeKey::new().exact("cohort", "A"),
                PolicyLabel::public().with_residency(Residency::only(["eu", "uk"])),
            ))
            .expect("registers")
            .with_rule(PolicyRule::new(
                "rule.uk",
                1,
                ScopeKey::new().exact("cohort", "B"),
                PolicyLabel::public().with_residency(Residency::only(["uk", "us"])),
            ))
            .expect("registers");
        let a = fact("fact.a", json!({"cohort": "A"}), &[]);
        let b = fact("fact.b", json!({"cohort": "B"}), &[]);

        let request = Request::new(
            Principal::new("p", "analyst", "uk"),
            Purpose::ResearchAnalysis,
            Channel::LocalCompute,
            now(),
        );
        let screening = lattice.screen([&a, &b], &request);

        assert_eq!(screening.admitted.len(), 2);
        assert_eq!(screening.derived_label().residency, Residency::only(["uk"]));
    }

    #[test]
    fn a_registered_tag_can_only_tighten_a_facts_label_never_loosen_it() {
        let mut lattice = eu_lattice();
        lattice.register_tag(
            "pediatric",
            PolicyLabel::public().with_compartment("pediatric"),
        );
        lattice.register_tag("public_summary", PolicyLabel::public());

        let tagged = fact(
            "fact.peds",
            json!({"residency": "eu"}),
            &["pediatric", "public_summary"],
        );
        let plain = fact("fact.plain", json!({"residency": "eu"}), &[]);

        let strict = lattice.resolve_fact(&tagged).label;
        let base = lattice.resolve_fact(&plain).label;

        assert!(base.flows_to(&strict));
        assert!(strict.compartments.contains("pediatric"));
        assert_eq!(base.classification, strict.classification);
    }

    #[test]
    fn an_unregistered_tag_is_reported_rather_than_silently_honoured() {
        let lattice = eu_lattice();
        let tagged = fact(
            "fact.claim",
            json!({"residency": "eu"}),
            &["public", "declassified", "safe_to_export"],
        );

        assert_eq!(
            lattice.unregistered_tags(&tagged),
            vec![
                "declassified".to_string(),
                "public".to_string(),
                "safe_to_export".to_string()
            ]
        );
        let resolved = lattice.resolve_fact(&tagged).label;
        assert_eq!(resolved.export, ExportPolicy::AggregatesOnly);
    }

    #[test]
    fn a_cache_admission_is_stamped_with_the_policy_version() {
        let lattice = PolicyLattice::new()
            .with_rule(PolicyRule::new(
                "rule.public",
                1,
                ScopeKey::new().exact("cohort", "A"),
                PolicyLabel::public().with_retention(Retention::Days(30)),
            ))
            .expect("registers");
        let scope = ScopeKey::new().exact("cohort", "A");

        let admission = lattice
            .admits(&scope, &research_request(Channel::Cache))
            .admission()
            .cloned()
            .expect("public evidence may be cached");

        assert!(admission
            .obligations
            .contains(&Obligation::KeyCacheToPolicyVersion {
                version: lattice.version()
            }));
        assert!(admission.obligations.contains(&Obligation::DeleteBy {
            at: "2026-09-07T00:00:00Z".to_string()
        }));
    }

    #[test]
    fn a_zero_day_retention_forbids_the_persistent_channels_but_not_local_compute() {
        let lattice = PolicyLattice::new()
            .with_rule(PolicyRule::new(
                "rule.ephemeral",
                1,
                ScopeKey::new().exact("cohort", "A"),
                PolicyLabel::public().with_retention(Retention::Days(0)),
            ))
            .expect("registers");
        let scope = ScopeKey::new().exact("cohort", "A");

        assert!(matches!(
            lattice
                .admits(&scope, &research_request(Channel::Cache))
                .refusal(),
            Some(Refusal::RetentionForbidsPersistence { .. })
        ));
        assert!(lattice
            .admits(&scope, &research_request(Channel::LocalCompute))
            .is_admitted());
    }
}
