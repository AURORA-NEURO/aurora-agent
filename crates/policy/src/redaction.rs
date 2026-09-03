//! Redaction with receipts.
//!
//! Blueprint 43.33's runtime contract is explicit about the shape: redaction is "semantic
//! replacement or omission reason, not string deletion alone". 39.19 asks for "redaction/existence
//! nodes" and a "privacy exposure ledger" as separate outputs from the authorized subgraph.
//!
//! The failure this module exists to prevent is the one the whole project exists to prevent: a
//! view from which something was removed looking exactly like a view from which nothing was. Three
//! things follow from that.
//!
//! * [`RedactedView`] records the rules it *evaluated*, not only the ones that fired. A view with
//!   an empty receipt list and a non-empty evaluated list means "we looked and nothing matched".
//!   A view with both empty means "nobody looked". [`RedactedView::was_evaluated`] separates them,
//!   and [`RedactedView::to_json`] emits the block unconditionally so a downstream reader that
//!   never heard of this crate still sees the difference.
//! * A redacted item stays in the view, carrying a replacement value that says what happened and
//!   under which rule. Dropping the item would make the view shorter and otherwise identical to a
//!   view where the item never existed.
//! * A [`RedactionReceipt`] names the rule id, its version and the rationale. "Redacted for policy
//!   reasons" is not a receipt; it is a shrug with a timestamp.
//!
//! [`SmallCellRule`] implements the worked micro-example of 43.33: when a cell is small enough to
//! expose identity, the local pod returns a bounded unknown rather than an unsafe aggregate.
//!
//! Not implemented: any inference-attack model. This crate suppresses a cell because a threshold
//! says so, and cannot tell whether releasing several thresholded cells reconstructs the
//! suppressed one. Differential privacy accounting, k-anonymity over quasi-identifiers and
//! linkage-attack analysis (36.02) all belong to a layer that is not here.

use crate::error::PolicyError;
use crate::label::PolicyLabel;
use bioprism_section::EvidenceCapsule;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// What replaced the withheld content.
///
/// Every variant except [`Replacement::Omitted`] leaves something behind that a reader can reason
/// about — an existence claim, a coarser value, an aggregate — which is what "semantic replacement"
/// means. `Omitted` is still visible, because its receipt is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "replacement", rename_all = "snake_case")]
pub enum Replacement {
    /// The value is gone and only the receipt remains.
    Omitted,
    /// The reader learns that a value exists here and that they may not see it.
    ExistenceOnly,
    /// Replaced by a coarser value: an age band instead of a date, a region instead of a site.
    Generalized { to: String },
    /// Replaced by a summary over a group, computed by the named operator.
    Aggregated { operator: String },
    /// Withheld because the cell was too small to release (36.02, 36.03).
    SmallCellSuppressed { threshold: u32 },
}

impl Replacement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Replacement::Omitted => "omitted",
            Replacement::ExistenceOnly => "existence_only",
            Replacement::Generalized { .. } => "generalized",
            Replacement::Aggregated { .. } => "aggregated",
            Replacement::SmallCellSuppressed { .. } => "small_cell_suppressed",
        }
    }
}

/// A versioned rule that withholds content matching a compartment or tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionRule {
    pub id: String,
    pub version: u32,
    /// Compartments on the label, or tags on the item, that trigger this rule.
    pub applies_to: BTreeSet<String>,
    pub replacement: Replacement,
    /// Compartments this rule is approved to clear once it has fired. Empty is the safe default:
    /// redacting content does not by itself make the artifact leave the compartment.
    #[serde(default)]
    pub clears: BTreeSet<String>,
    /// Why. Appears verbatim in every receipt this rule produces.
    pub rationale: String,
}

impl RedactionRule {
    pub fn new(
        id: impl Into<String>,
        version: u32,
        applies_to: impl IntoIterator<Item = &'static str>,
        replacement: Replacement,
        rationale: impl Into<String>,
    ) -> Self {
        RedactionRule {
            id: id.into(),
            version,
            applies_to: applies_to.into_iter().map(str::to_string).collect(),
            replacement,
            clears: BTreeSet::new(),
            rationale: rationale.into(),
        }
    }

    pub fn clearing(mut self, compartment: impl Into<String>) -> Self {
        self.clears.insert(compartment.into());
        self
    }

    fn matches(&self, triggers: &BTreeSet<String>) -> bool {
        !self.applies_to.is_disjoint(triggers)
    }
}

/// The record of one redaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionReceipt {
    /// The evidence id that was redacted.
    pub target: String,
    pub rule_id: String,
    pub rule_version: u32,
    pub replacement: Replacement,
    /// Which trigger matched, so a reader can tell paediatric suppression from dual-use
    /// suppression without consulting the rule.
    pub matched: Vec<String>,
    pub rationale: String,
}

/// A projected view plus the ledger of what it withheld.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactedView {
    /// Rules that were consulted, whether or not they fired.
    pub rules_evaluated: Vec<String>,
    pub receipts: Vec<RedactionReceipt>,
    pub items: Vec<EvidenceCapsule>,
    /// The label of the view after any compartments the rules were approved to clear.
    pub label: PolicyLabel,
}

impl RedactedView {
    /// Whether redaction rules were run at all.
    ///
    /// The whole point: `!was_evaluated()` and `receipts.is_empty()` are different states and a
    /// consumer must be able to tell them apart before trusting a view that looks complete.
    pub fn was_evaluated(&self) -> bool {
        !self.rules_evaluated.is_empty()
    }

    /// Rules ran and nothing matched. A positive claim, unlike an empty receipt list on its own.
    pub fn nothing_matched(&self) -> bool {
        self.was_evaluated() && self.receipts.is_empty()
    }

    pub fn is_redacted(&self) -> bool {
        !self.receipts.is_empty()
    }

    /// The wire form, with the redaction block always present.
    ///
    /// Emitted even when empty for the same reason `bioprism_weave` populates `withheld` even when
    /// nothing was withheld: absence of a field reads as absence of the concern.
    pub fn to_json(&self) -> Value {
        json!({
            "items": self.items,
            "redaction": {
                "evaluated": self.rules_evaluated,
                "applied": self.receipts.len(),
                "receipts": self.receipts,
            },
            "label": self.label,
        })
    }
}

/// An ordered set of redaction rules applied to a view.
#[derive(Debug, Clone, Default)]
pub struct RedactionPlan {
    rules: Vec<RedactionRule>,
}

impl RedactionPlan {
    pub fn new() -> Self {
        RedactionPlan::default()
    }

    /// Adds a rule, rejecting one with no rationale: its receipts would be blank and a blank
    /// receipt is indistinguishable from no receipt to the person reading it.
    pub fn register(&mut self, rule: RedactionRule) -> Result<(), PolicyError> {
        if rule.rationale.trim().is_empty() {
            return Err(PolicyError::UnexplainedRedaction {
                id: rule.id,
                version: rule.version,
            });
        }
        self.rules.push(rule);
        Ok(())
    }

    pub fn with_rule(mut self, rule: RedactionRule) -> Result<Self, PolicyError> {
        self.register(rule)?;
        Ok(self)
    }

    pub fn rules(&self) -> &[RedactionRule] {
        &self.rules
    }

    /// Applies every rule to every item.
    ///
    /// An item's triggers are its own tags joined with the label's compartments, so a rule can
    /// respond either to what the item claims about itself or to what the policy fiber says about
    /// its scope. The first matching rule wins per item: later rules would only re-redact an
    /// already-replaced value while adding a receipt that describes nothing.
    pub fn apply(&self, items: &[EvidenceCapsule], label: &PolicyLabel) -> RedactedView {
        let rules_evaluated: Vec<String> = self
            .rules
            .iter()
            .map(|rule| format!("{}@{}", rule.id, rule.version))
            .collect();

        let mut receipts = Vec::new();
        let mut projected = Vec::new();
        let mut cleared: BTreeSet<String> = BTreeSet::new();

        for item in items {
            let mut triggers: BTreeSet<String> = item.tags.iter().cloned().collect();
            triggers.extend(label.compartments.iter().cloned());

            match self.rules.iter().find(|rule| rule.matches(&triggers)) {
                None => projected.push(item.clone()),
                Some(rule) => {
                    let matched: Vec<String> =
                        rule.applies_to.intersection(&triggers).cloned().collect();
                    receipts.push(RedactionReceipt {
                        target: item.id.clone(),
                        rule_id: rule.id.clone(),
                        rule_version: rule.version,
                        replacement: rule.replacement.clone(),
                        matched,
                        rationale: rule.rationale.clone(),
                    });
                    cleared.extend(rule.clears.iter().cloned());
                    let mut redacted = item.clone();
                    redacted.value = replacement_value(rule);
                    projected.push(redacted);
                }
            }
        }

        let mut result_label = label.clone();
        if !receipts.is_empty() {
            result_label.compartments = result_label
                .compartments
                .difference(&cleared)
                .cloned()
                .collect();
        }

        RedactedView {
            rules_evaluated,
            receipts,
            items: projected,
            label: result_label,
        }
    }
}

/// The value that stands in for withheld content.
///
/// Never `null` and never a deleted key. A reader that does not know this crate still sees an
/// object saying a policy rule replaced something and which rule it was.
fn replacement_value(rule: &RedactionRule) -> Value {
    let mut block = json!({
        "kind": rule.replacement.as_str(),
        "rule": format!("{}@{}", rule.id, rule.version),
        "rationale": rule.rationale,
    });
    match &rule.replacement {
        Replacement::Generalized { to } => {
            block["generalized_to"] = json!(to);
        }
        Replacement::Aggregated { operator } => {
            block["operator"] = json!(operator);
        }
        Replacement::SmallCellSuppressed { threshold } => {
            block["threshold"] = json!(threshold);
        }
        Replacement::Omitted | Replacement::ExistenceOnly => {}
    }
    json!({ "policy_redaction": block })
}

/// Small-cell suppression.
///
/// 43.33's worked micro-example: "If a small cell would expose identity, the local pod returns a
/// bounded unknown rather than an unsafe aggregate."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmallCellRule {
    pub threshold: u32,
}

/// What a local pod may return for one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "release", rename_all = "snake_case")]
pub enum CellRelease {
    Exact {
        count: u64,
    },
    /// The count is known to be below `below` and is not released.
    BoundedUnknown {
        below: u32,
    },
}

impl SmallCellRule {
    pub fn new(threshold: u32) -> Self {
        SmallCellRule { threshold }
    }

    /// Decides what may leave the pod for a cell of size `count`.
    ///
    /// A count of zero is suppressed along with the rest. In a rare-disease or paediatric cohort,
    /// "nobody here has this variant" is itself a disclosure, and releasing exact zeros while
    /// suppressing small positives lets an attacker separate the two by asking twice.
    pub fn release(&self, count: u64) -> CellRelease {
        if count < u64::from(self.threshold) {
            CellRelease::BoundedUnknown {
                below: self.threshold,
            }
        } else {
            CellRelease::Exact { count }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn capsule(id: &str, tags: &[&str], value: Value) -> EvidenceCapsule {
        EvidenceCapsule::from_raw_fact(&json!({
            "id": id,
            "provides": "measurement",
            "value": value,
            "scope": {"cohort": "PEDS"},
            "tags": tags,
            "provenance": ["fixture"],
        }))
    }

    fn peds_plan() -> RedactionPlan {
        RedactionPlan::new()
            .with_rule(RedactionRule::new(
                "redact.peds-dob",
                3,
                ["pediatric"],
                Replacement::Generalized {
                    to: "age_band_5y".to_string(),
                },
                "exact dates of birth in a paediatric cohort are directly identifying",
            ))
            .expect("rule registers")
    }

    #[test]
    fn a_view_with_nothing_redacted_is_distinguishable_from_a_view_nobody_evaluated() {
        let items = [capsule("fact.a", &[], json!(1))];

        let evaluated = peds_plan().apply(&items, &PolicyLabel::public());
        let unevaluated = RedactionPlan::new().apply(&items, &PolicyLabel::public());

        assert!(evaluated.was_evaluated());
        assert!(evaluated.nothing_matched());
        assert!(!unevaluated.was_evaluated());
        assert!(!unevaluated.nothing_matched());
        assert_eq!(evaluated.items, unevaluated.items);
        assert_ne!(evaluated.to_json(), unevaluated.to_json());
    }

    #[test]
    fn the_redaction_block_is_present_in_the_wire_form_even_when_empty() {
        let view = peds_plan().apply(&[], &PolicyLabel::public());
        let wire = view.to_json();
        assert!(wire.get("redaction").is_some());
        assert_eq!(wire["redaction"]["applied"], json!(0));
        assert!(!wire["redaction"]["evaluated"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn redaction_replaces_semantically_rather_than_deleting_the_value() {
        let items = [capsule("fact.dob", &["pediatric"], json!("2019-04-11"))];

        let view = peds_plan().apply(&items, &PolicyLabel::public());

        assert_eq!(view.items.len(), 1, "the item stays in the view");
        let value = &view.items[0].value;
        assert!(!value.is_null());
        assert_eq!(value["policy_redaction"]["kind"], json!("generalized"));
        assert_eq!(
            value["policy_redaction"]["generalized_to"],
            json!("age_band_5y")
        );
        assert_eq!(
            value["policy_redaction"]["rule"],
            json!("redact.peds-dob@3")
        );
    }

    #[test]
    fn every_redaction_records_the_rule_version_and_rationale_that_caused_it() {
        let items = [capsule("fact.dob", &["pediatric"], json!("2019-04-11"))];

        let view = peds_plan().apply(&items, &PolicyLabel::public());

        assert_eq!(view.receipts.len(), 1);
        let receipt = &view.receipts[0];
        assert_eq!(receipt.target, "fact.dob");
        assert_eq!(receipt.rule_id, "redact.peds-dob");
        assert_eq!(receipt.rule_version, 3);
        assert_eq!(receipt.matched, vec!["pediatric".to_string()]);
        assert!(receipt.rationale.contains("identifying"));
    }

    #[test]
    fn a_label_compartment_triggers_redaction_even_when_the_item_carries_no_tag() {
        let items = [capsule("fact.untagged", &[], json!("2019-04-11"))];
        let label = PolicyLabel::public().with_compartment("pediatric");

        let view = peds_plan().apply(&items, &label);

        assert!(view.is_redacted());
        assert_eq!(view.receipts[0].matched, vec!["pediatric".to_string()]);
    }

    #[test]
    fn redaction_does_not_clear_a_compartment_unless_the_rule_is_approved_to() {
        let items = [capsule("fact.dob", &["pediatric"], json!("2019-04-11"))];
        let label = PolicyLabel::public().with_compartment("pediatric");

        let conservative = peds_plan().apply(&items, &label);
        assert!(conservative.label.compartments.contains("pediatric"));

        let approved = RedactionPlan::new()
            .with_rule(
                RedactionRule::new(
                    "redact.peds-dob",
                    3,
                    ["pediatric"],
                    Replacement::Generalized {
                        to: "age_band_5y".to_string(),
                    },
                    "generalisation to five-year bands was reviewed and approved",
                )
                .clearing("pediatric"),
            )
            .expect("rule registers")
            .apply(&items, &label);
        assert!(!approved.label.compartments.contains("pediatric"));
    }

    #[test]
    fn a_rule_without_a_rationale_is_rejected_so_receipts_cannot_be_blank() {
        let rule = RedactionRule::new("redact.blank", 1, ["pediatric"], Replacement::Omitted, "  ");
        assert!(matches!(
            RedactionPlan::new().register(rule),
            Err(PolicyError::UnexplainedRedaction { .. })
        ));
    }

    #[test]
    fn a_cell_below_the_threshold_returns_a_bounded_unknown_not_a_count() {
        let rule = SmallCellRule::new(11);
        assert_eq!(rule.release(10), CellRelease::BoundedUnknown { below: 11 });
        assert_eq!(rule.release(11), CellRelease::Exact { count: 11 });
        assert_eq!(rule.release(4000), CellRelease::Exact { count: 4000 });
    }

    #[test]
    fn a_zero_cell_is_suppressed_too_so_absence_cannot_be_read_off_the_release_shape() {
        let rule = SmallCellRule::new(11);
        assert_eq!(rule.release(0), CellRelease::BoundedUnknown { below: 11 });
        assert_eq!(rule.release(0), rule.release(3));
    }
}
