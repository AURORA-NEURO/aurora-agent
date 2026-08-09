//! Information flow between artifacts, and the only sanctioned way to go downhill.
//!
//! Blueprint 43.33: "outputs may move only to equal or more restrictive labels unless an
//! authorized declassification rule applies", with declassification defined as an "explicit
//! approved operator with leakage analysis and accountable principal" and the invariant
//! "declassification is explicit and versioned".
//!
//! Two operations live here and they are not symmetric. [`derive`] is free: fold the inputs with
//! [`PolicyLabel::join`] and the result is correct by construction, because the join is the least
//! upper bound and an upper bound is exactly what a derived artifact needs. Going the other way
//! requires a registered rule, a held authority, a written leakage analysis and an identified
//! principal, and it produces a [`DeclassificationReceipt`] that names all four.
//!
//! The guard that matters most is compartment retention. A declassification rule states the
//! compartments it is approved to clear, and any compartment on the input that the rule does not
//! name survives into the result. Without that, a rule written for one compartment quietly becomes
//! a general-purpose downgrade the first time somebody applies it to an artifact that picked up a
//! second compartment along the way — which is precisely how a join-based system is defeated.
//!
//! Not implemented: signatures on receipts, an approval workflow, or any check that the leakage
//! analysis is *correct*. This crate can insist that an analysis exists and is attributed; it
//! cannot read it. 43.33's "leakage analysis" is a human artifact and treating a non-empty string
//! as evidence of one is the honest limit of what a type system buys here.

use crate::decision::Refusal;
use crate::error::PolicyError;
use crate::label::PolicyLabel;
use crate::request::{Authority, Principal};
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The label a derived artifact must carry: the join of its inputs.
///
/// 13.16: "derived artifacts inherit the maximum sensitivity of sources unless an approved
/// transformation proves declassification." The join is stronger than a maximum — it also
/// intersects purposes and jurisdictions and takes the shortest retention — which is what the
/// classic two-low-inputs example requires.
pub fn derive<'a, I: IntoIterator<Item = &'a PolicyLabel>>(inputs: I) -> PolicyLabel {
    PolicyLabel::join_all(inputs)
}

/// Checks that information labelled `source` may become information labelled `destination`,
/// naming the axis that fails.
///
/// Called on separator messages and cache writes, per 43.33 step 5.
pub fn check_flow(source: &PolicyLabel, destination: &PolicyLabel) -> Result<(), Refusal> {
    if source.flows_to(destination) {
        return Ok(());
    }
    let (axis, detail) = if source.classification > destination.classification {
        (
            "classification",
            format!(
                "{} may not be relabelled {}",
                source.classification, destination.classification
            ),
        )
    } else if !source.compartments.is_subset(&destination.compartments) {
        let dropped: Vec<&str> = source
            .compartments
            .difference(&destination.compartments)
            .map(String::as_str)
            .collect();
        (
            "compartments",
            format!("compartments {dropped:?} would be dropped"),
        )
    } else if !destination.purposes.is_subset_of(&source.purposes) {
        (
            "purposes",
            format!(
                "destination permits {} which the source ({}) does not",
                destination.purposes, source.purposes
            ),
        )
    } else if !destination.residency.is_subset_of(&source.residency) {
        (
            "residency",
            format!(
                "destination permits {} which the source ({}) does not",
                destination.residency, source.residency
            ),
        )
    } else if source.export > destination.export {
        (
            "export",
            format!(
                "{} may not be relaxed to {}",
                source.export, destination.export
            ),
        )
    } else if source.min_cell_size > destination.min_cell_size {
        (
            "min_cell_size",
            format!(
                "threshold {} may not be relaxed to {}",
                source.min_cell_size, destination.min_cell_size
            ),
        )
    } else {
        (
            "retention",
            format!(
                "retention {} may not be extended to {}",
                source.retention, destination.retention
            ),
        )
    };

    Err(Refusal::FlowWouldDowngrade {
        axis: axis.to_string(),
        detail,
    })
}

/// An approved operator that lowers a label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclassificationRule {
    pub id: String,
    pub version: u32,
    /// The authority a principal must hold to invoke this rule.
    pub authority: Authority,
    /// The ceiling of the rule's proven range: the most restrictive input the leakage analysis
    /// covers. An input is in range when it flows to `from`, so the rule applies to everything at
    /// or below the point it was proven at and to nothing above it.
    ///
    /// The consequence worth being explicit about: an input carrying a compartment `from` does not
    /// name is *more* restrictive than the ceiling and is refused. A rule proven over aggregate
    /// counts of a general cohort does not silently acquire jurisdiction over the paediatric
    /// subset, because nobody analysed what those counts leak there. To cover a compartment, name
    /// it in `from`; to be allowed to remove it, name it in `releases` as well.
    pub from: PolicyLabel,
    /// The label the rule produces, before surviving compartments are added back.
    pub to: PolicyLabel,
    /// Compartments this rule is approved to clear. Anything else on the input survives.
    #[serde(default)]
    pub releases: BTreeSet<String>,
    /// The written analysis of what the transformation leaks. Required to be non-empty.
    pub leakage_analysis: String,
}

impl DeclassificationRule {
    pub fn new(
        id: impl Into<String>,
        version: u32,
        authority: Authority,
        from: PolicyLabel,
        to: PolicyLabel,
        leakage_analysis: impl Into<String>,
    ) -> Self {
        DeclassificationRule {
            id: id.into(),
            version,
            authority,
            from,
            to,
            releases: BTreeSet::new(),
            leakage_analysis: leakage_analysis.into(),
        }
    }

    pub fn releasing(mut self, compartment: impl Into<String>) -> Self {
        self.releases.insert(compartment.into());
        self
    }
}

/// The record left behind by an applied declassification.
///
/// Carries both labels, the surviving compartments, the accountable principal and the analysis
/// text, so an auditor reading the receipt alone can reconstruct what was approved and by whom
/// without access to the registry that held the rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclassificationReceipt {
    pub rule_id: String,
    pub version: u32,
    pub authority: Authority,
    pub principal: String,
    pub at: String,
    pub from: PolicyLabel,
    pub to: PolicyLabel,
    /// Compartments the rule was not approved to clear, and which therefore survived.
    pub retained_compartments: Vec<String>,
    pub leakage_analysis: String,
}

/// The versioned registry of approved declassification operators.
#[derive(Debug, Clone, Default)]
pub struct DeclassificationRegistry {
    rules: BTreeMap<(String, u32), DeclassificationRule>,
}

impl DeclassificationRegistry {
    pub fn new() -> Self {
        DeclassificationRegistry::default()
    }

    /// Registers a rule, rejecting the two shapes that would make declassification cosmetic: one
    /// with no leakage analysis, and one whose output is not actually lower than its input.
    pub fn register(&mut self, rule: DeclassificationRule) -> Result<(), PolicyError> {
        if rule.leakage_analysis.trim().is_empty() {
            return Err(PolicyError::UnanalysedDeclassification {
                id: rule.id,
                version: rule.version,
            });
        }
        if !rule.to.flows_to(&rule.from) || rule.to == rule.from {
            return Err(PolicyError::NotADeclassification {
                id: rule.id,
                version: rule.version,
            });
        }
        let key = (rule.id.clone(), rule.version);
        if let Some(existing) = self.rules.get(&key) {
            if *existing != rule {
                return Err(PolicyError::RuleVersionConflict {
                    id: rule.id,
                    version: rule.version,
                });
            }
            return Ok(());
        }
        self.rules.insert(key, rule);
        Ok(())
    }

    pub fn with_rule(mut self, rule: DeclassificationRule) -> Result<Self, PolicyError> {
        self.register(rule)?;
        Ok(self)
    }

    pub fn get(&self, id: &str, version: u32) -> Option<&DeclassificationRule> {
        self.rules.get(&(id.to_string(), version))
    }

    /// Applies a declassification.
    ///
    /// The version is part of the request, not resolved to "latest". 43.33 requires
    /// declassification to be versioned, and a caller that names a rule without a version is
    /// asking for whatever the registry happens to hold — which is how an approved operator
    /// silently changes meaning under an unchanged call site.
    pub fn apply(
        &self,
        rule_id: &str,
        version: u32,
        label: &PolicyLabel,
        principal: &Principal,
        at: Timestamp,
    ) -> Result<(PolicyLabel, DeclassificationReceipt), Refusal> {
        let rule = self
            .get(rule_id, version)
            .ok_or_else(|| Refusal::UnknownDeclassificationRule {
                rule_id: rule_id.to_string(),
                version,
            })?;

        if !principal.holds(&rule.authority) {
            return Err(Refusal::DeclassificationUnauthorized {
                rule_id: rule.id.clone(),
                version: rule.version,
                authority: rule.authority.clone(),
                principal: principal.id.clone(),
            });
        }

        if !label.flows_to(&rule.from) {
            return Err(Refusal::DeclassificationOutOfRange {
                rule_id: rule.id.clone(),
                version: rule.version,
                detail: format!(
                    "the input label is not within the proven range of this rule (classification \
                     {}, compartments {:?})",
                    label.classification, label.compartments
                ),
            });
        }

        let retained: BTreeSet<String> = label
            .compartments
            .difference(&rule.releases)
            .cloned()
            .collect();
        let mut result = rule.to.clone();
        result.compartments = result.compartments.union(&retained).cloned().collect();

        let receipt = DeclassificationReceipt {
            rule_id: rule.id.clone(),
            version: rule.version,
            authority: rule.authority.clone(),
            principal: principal.id.clone(),
            at: at.to_rfc3339(),
            from: label.clone(),
            to: result.clone(),
            retained_compartments: retained.into_iter().collect(),
            leakage_analysis: rule.leakage_analysis.clone(),
        };
        Ok((result, receipt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::{Classification, ExportPolicy};
    use crate::purpose::{Purpose, PurposeSet};
    use crate::request::Clearance;
    use crate::residency::Residency;

    fn at() -> Timestamp {
        Timestamp::parse("2026-08-08T00:00:00Z").expect("fixture timestamp parses")
    }

    fn steward_authority() -> Authority {
        Authority::new("authority.data-steward")
    }

    fn steward() -> Principal {
        Principal::new("p.steward", "steward", "eu")
            .with_clearance(Clearance::up_to(Classification::RestrictedDualUse))
            .holding(steward_authority())
    }

    fn unprivileged() -> Principal {
        Principal::new("p.analyst", "analyst", "eu")
            .with_clearance(Clearance::up_to(Classification::RestrictedDualUse))
    }

    fn controlled() -> PolicyLabel {
        PolicyLabel::public()
            .with_classification(Classification::ControlledGenomicOrImaging)
            .with_export(ExportPolicy::NoExport)
            .with_residency(Residency::only(["eu"]))
            .with_purposes(PurposeSet::of([Purpose::ResearchAnalysis]))
    }

    /// The ceiling the rule was proven at: controlled data, including the paediatric and
    /// site-confidential compartments.
    fn analysed_ceiling() -> PolicyLabel {
        controlled()
            .with_compartment("pediatric")
            .with_compartment("site_confidential")
    }

    fn aggregate_release() -> DeclassificationRule {
        DeclassificationRule::new(
            "declass.cohort-counts",
            2,
            steward_authority(),
            analysed_ceiling(),
            PolicyLabel::public()
                .with_classification(Classification::PublicAggregate)
                .with_export(ExportPolicy::Unrestricted)
                .with_residency(Residency::Anywhere)
                .with_purposes(PurposeSet::Any),
            "counts are suppressed below 11 and jittered; residual re-identification risk assessed \
             as negligible for cohorts above 200",
        )
    }

    fn registry() -> DeclassificationRegistry {
        DeclassificationRegistry::new()
            .with_rule(aggregate_release())
            .expect("rule registers")
    }

    #[test]
    fn deriving_from_two_inputs_yields_their_join_not_either_input() {
        let a = PolicyLabel::public().with_residency(Residency::only(["eu", "uk"]));
        let b = PolicyLabel::public().with_residency(Residency::only(["uk", "us"]));
        let derived = derive([&a, &b]);
        assert_eq!(derived.residency, Residency::only(["uk"]));
        assert!(check_flow(&a, &derived).is_ok());
        assert!(check_flow(&b, &derived).is_ok());
    }

    #[test]
    fn a_flow_that_would_drop_a_compartment_is_refused_naming_the_axis() {
        let source = PolicyLabel::public().with_compartment("pediatric");
        let destination = PolicyLabel::public();

        match check_flow(&source, &destination) {
            Err(Refusal::FlowWouldDowngrade { axis, detail }) => {
                assert_eq!(axis, "compartments");
                assert!(detail.contains("pediatric"));
            }
            other => panic!("expected a compartment downgrade refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_flow_that_would_widen_the_permitted_purposes_is_refused() {
        let source = PolicyLabel::public().with_purposes(PurposeSet::of([Purpose::ResearchAnalysis]));
        let destination = PolicyLabel::public().with_purposes(PurposeSet::Any);

        match check_flow(&source, &destination) {
            Err(Refusal::FlowWouldDowngrade { axis, .. }) => assert_eq!(axis, "purposes"),
            other => panic!("expected a purpose downgrade refusal, got {other:?}"),
        }
    }

    #[test]
    fn declassification_without_the_authority_is_refused() {
        let refusal = registry()
            .apply(
                "declass.cohort-counts",
                2,
                &controlled(),
                &unprivileged(),
                at(),
            )
            .expect_err("an analyst may not declassify");

        match refusal {
            Refusal::DeclassificationUnauthorized {
                authority,
                principal,
                ..
            } => {
                assert_eq!(authority, steward_authority());
                assert_eq!(principal, "p.analyst");
            }
            other => panic!("expected an authorisation refusal, got {other:?}"),
        }
    }

    #[test]
    fn declassification_cannot_clear_a_compartment_it_does_not_name() {
        let input = controlled().with_compartment("pediatric");

        let (result, receipt) = registry()
            .apply("declass.cohort-counts", 2, &input, &steward(), at())
            .expect("the input is within the analysed ceiling");

        assert!(
            result.compartments.contains("pediatric"),
            "a compartment the rule was not approved to release must survive declassification"
        );
        assert_eq!(receipt.retained_compartments, vec!["pediatric".to_string()]);
    }

    #[test]
    fn a_compartment_the_rule_does_name_is_cleared() {
        let registry = DeclassificationRegistry::new()
            .with_rule(aggregate_release().releasing("pediatric"))
            .expect("rule registers");
        let input = controlled().with_compartment("pediatric");

        let (result, receipt) = registry
            .apply("declass.cohort-counts", 2, &input, &steward(), at())
            .expect("the rule applies");

        assert!(!result.compartments.contains("pediatric"));
        assert!(receipt.retained_compartments.is_empty());
    }

    #[test]
    fn releasing_one_compartment_does_not_release_the_other_it_travelled_with() {
        let registry = DeclassificationRegistry::new()
            .with_rule(aggregate_release().releasing("pediatric"))
            .expect("rule registers");
        let input = controlled()
            .with_compartment("pediatric")
            .with_compartment("site_confidential");

        let (result, receipt) = registry
            .apply("declass.cohort-counts", 2, &input, &steward(), at())
            .expect("both compartments are inside the analysed ceiling");

        assert!(!result.compartments.contains("pediatric"));
        assert!(result.compartments.contains("site_confidential"));
        assert_eq!(
            receipt.retained_compartments,
            vec!["site_confidential".to_string()]
        );
    }

    #[test]
    fn an_input_carrying_a_compartment_the_rule_never_analysed_is_refused() {
        let input = controlled().with_compartment("dual_use");

        let refusal = registry()
            .apply("declass.cohort-counts", 2, &input, &steward(), at())
            .expect_err("dual-use was outside the analysed ceiling");

        assert!(matches!(refusal, Refusal::DeclassificationOutOfRange { .. }));
    }

    #[test]
    fn a_rule_without_leakage_analysis_is_rejected_at_registration() {
        let rule = DeclassificationRule::new(
            "declass.blank",
            1,
            steward_authority(),
            controlled(),
            PolicyLabel::public(),
            "   ",
        );
        assert!(matches!(
            DeclassificationRegistry::new().register(rule),
            Err(PolicyError::UnanalysedDeclassification { .. })
        ));
    }

    #[test]
    fn a_rule_that_does_not_lower_its_input_is_not_a_declassification() {
        let raising = DeclassificationRule::new(
            "declass.upward",
            1,
            steward_authority(),
            PolicyLabel::public(),
            PolicyLabel::public().with_classification(Classification::RestrictedDualUse),
            "analysed",
        );
        let identity = DeclassificationRule::new(
            "declass.noop",
            1,
            steward_authority(),
            controlled(),
            controlled(),
            "analysed",
        );

        for rule in [raising, identity] {
            assert!(matches!(
                DeclassificationRegistry::new().register(rule),
                Err(PolicyError::NotADeclassification { .. })
            ));
        }
    }

    #[test]
    fn a_rule_referenced_at_the_wrong_version_is_not_silently_resolved_to_the_latest() {
        let refusal = registry()
            .apply("declass.cohort-counts", 1, &controlled(), &steward(), at())
            .expect_err("version 1 does not exist");

        assert!(matches!(
            refusal,
            Refusal::UnknownDeclassificationRule { version: 1, .. }
        ));
        assert!(registry()
            .apply("declass.cohort-counts", 2, &controlled(), &steward(), at())
            .is_ok());
    }

    #[test]
    fn an_input_outside_the_rules_proven_range_is_refused_rather_than_approximated() {
        let hotter = controlled().with_classification(Classification::RestrictedDualUse);

        let refusal = registry()
            .apply("declass.cohort-counts", 2, &hotter, &steward(), at())
            .expect_err("the rule was proven for controlled data, not dual-use data");

        assert!(matches!(
            refusal,
            Refusal::DeclassificationOutOfRange { .. }
        ));
    }

    #[test]
    fn a_receipt_names_the_accountable_principal_the_authority_and_the_analysis() {
        let (_, receipt) = registry()
            .apply("declass.cohort-counts", 2, &controlled(), &steward(), at())
            .expect("applies");

        assert_eq!(receipt.principal, "p.steward");
        assert_eq!(receipt.authority, steward_authority());
        assert_eq!(receipt.version, 2);
        assert_eq!(receipt.at, "2026-08-08T00:00:00Z");
        assert!(receipt.leakage_analysis.contains("suppressed below 11"));
        assert_eq!(receipt.from, controlled());
    }
}
