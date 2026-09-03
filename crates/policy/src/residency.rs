//! Residency, export and the transports that cross a policy boundary.
//!
//! Blueprint 43.33 lists residency and export among the constraints that must hold *during*
//! compilation, and 36.06 (federated and data-local evaluation) says the honest way to satisfy
//! them is usually to move the computation rather than the data.
//!
//! The mechanism is borrowed from 43.05 rather than invented: `bioprism_scope` already insists
//! that carrying evidence to a scope which is not a refinement is a [`MappingKind::Transport`]
//! carrying a [`LossLedger`], never a free copy. A policy boundary crossing is exactly such a
//! move, so [`propose_transport`] returns a `ScopeMapping` whose ledger names what did not
//! travel and under which condition the remainder did. A caller that wants to relocate evidence
//! must therefore hold an object that says so; there is no API here that returns bare data on the
//! far side of a jurisdiction.
//!
//! Not implemented: any actual data movement, network egress control, or verification that a
//! remote pod honoured the ledger. This crate produces the declaration and the refusal. The
//! enforcement point in 36.01's architecture diagram is the trusted kernel, which is not this.

use crate::decision::Refusal;
use crate::label::{ExportPolicy, PolicyLabel};
use crate::lattice::describe_scope;
use bioprism_scope::{LossLedger, MappingKind, ScopeKey, ScopeMapping, ScopeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The scope dimension that names where evidence is allowed to sit.
///
/// `bioprism_scope`'s default registry already classifies `residency` as a `Policy` dimension, so
/// this crate reads the base rather than introducing a parallel vocabulary. A world that spells
/// residency some other way must bind this dimension as well; a scope that does not bind it has
/// not said where it is, and an unstated location is refused rather than assumed.
pub const RESIDENCY_DIMENSION: &str = "residency";

/// A named legal territory: a country, a data-protection region, or an institutional enclave.
///
/// Deliberately an opaque string. This crate has no map of the world and no model of which
/// regulations subsume which others, so it cannot decide that `eu-de` satisfies `eu`. Equality is
/// the only relation, and a permission set that means "either" must list both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Jurisdiction(pub String);

impl Jurisdiction {
    pub fn new(name: impl Into<String>) -> Self {
        Jurisdiction(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Jurisdiction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where an artifact is permitted to be.
///
/// [`Residency::Anywhere`] is the bottom of this axis and the identity of [`Residency::intersect`].
/// `Only` with an empty set is reachable and meaningful: it is the state two incompatible
/// residencies join to, and it means no site may hold the combination. 43.33 requires that this
/// surface as a typed inaccessible-evidence result, not as an empty answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Residency {
    Anywhere,
    Only(BTreeSet<Jurisdiction>),
}

impl Residency {
    pub fn only<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Residency::Only(names.into_iter().map(|n| Jurisdiction::new(n)).collect())
    }

    /// The state in which no site is permitted. Two disjoint residencies join to this.
    pub fn nowhere() -> Self {
        Residency::Only(BTreeSet::new())
    }

    pub fn admits(&self, site: &Jurisdiction) -> bool {
        match self {
            Residency::Anywhere => true,
            Residency::Only(set) => set.contains(site),
        }
    }

    pub fn is_nowhere(&self) -> bool {
        matches!(self, Residency::Only(set) if set.is_empty())
    }

    /// A deterministic permitted site, for planning a federated execution.
    ///
    /// `BTreeSet` ordering makes the choice reproducible rather than arbitrary-but-stable-today.
    /// A caller that cares which site runs the computation should name it instead of accepting
    /// this default.
    pub fn any_permitted_site(&self) -> Option<Jurisdiction> {
        match self {
            Residency::Anywhere => None,
            Residency::Only(set) => set.iter().next().cloned(),
        }
    }

    /// Sites permitted by both. The join direction: combining sources may only shrink the set of
    /// legal locations.
    pub fn intersect(&self, other: &Residency) -> Residency {
        match (self, other) {
            (Residency::Anywhere, rest) | (rest, Residency::Anywhere) => rest.clone(),
            (Residency::Only(a), Residency::Only(b)) => {
                Residency::Only(a.intersection(b).cloned().collect())
            }
        }
    }

    /// Sites permitted by either. Present so the lattice laws can be stated; not a safe operation
    /// on live evidence, since it invents permission neither source granted.
    pub fn union(&self, other: &Residency) -> Residency {
        match (self, other) {
            (Residency::Anywhere, _) | (_, Residency::Anywhere) => Residency::Anywhere,
            (Residency::Only(a), Residency::Only(b)) => {
                Residency::Only(a.union(b).cloned().collect())
            }
        }
    }

    pub fn is_subset_of(&self, wider: &Residency) -> bool {
        match (self, wider) {
            (_, Residency::Anywhere) => true,
            (Residency::Anywhere, Residency::Only(_)) => false,
            (Residency::Only(a), Residency::Only(b)) => a.is_subset(b),
        }
    }
}

impl fmt::Display for Residency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Residency::Anywhere => f.write_str("anywhere"),
            Residency::Only(set) => {
                let names: Vec<&str> = set.iter().map(Jurisdiction::as_str).collect();
                write!(f, "only[{}]", names.join(","))
            }
        }
    }
}

/// Reads the jurisdictions a scope claims to sit in.
///
/// A `OneOf` binding means the scope spans several territories, and all of them must be permitted
/// for a move into it to be legal — the weakest member decides, not the strongest.
pub fn declared_residency(scope: &ScopeKey) -> Option<BTreeSet<Jurisdiction>> {
    match scope.get(RESIDENCY_DIMENSION)? {
        ScopeValue::Exact(name) => Some([Jurisdiction::new(name.clone())].into_iter().collect()),
        ScopeValue::OneOf(names) => {
            Some(names.iter().map(|n| Jurisdiction::new(n.clone())).collect())
        }
        ScopeValue::Window(_) => None,
    }
}

/// Declares a move of labelled evidence from one scope to another.
///
/// Returns a [`ScopeMapping`], which is the vocabulary the rest of the system already understands:
/// a narrowing inside one territory is a [`MappingKind::Restriction`] and costs nothing, while a
/// boundary crossing is a [`MappingKind::Transport`] whose [`LossLedger`] states the export limit
/// it travelled under. The ledger is never empty on a transport, so `ScopeMapping::check` reports
/// `Sound` for exactly the moves that declared their price.
pub fn propose_transport(
    label: &PolicyLabel,
    from: &ScopeKey,
    to: &ScopeKey,
    justification: impl Into<String>,
) -> Result<ScopeMapping, Refusal> {
    let justification = justification.into();
    if justification.trim().is_empty() {
        return Err(Refusal::TransportWithoutJustification {
            to: describe_scope(to),
        });
    }

    let destinations = declared_residency(to).ok_or_else(|| Refusal::UndeclaredDestination {
        to: describe_scope(to),
        dimension: RESIDENCY_DIMENSION.to_string(),
    })?;

    if label.residency.is_nowhere() {
        return Err(Refusal::NoLegalExecutionPath {
            detail: "the joined residency of this evidence permits no site at all".to_string(),
        });
    }

    for site in &destinations {
        if !label.residency.admits(site) {
            return Err(Refusal::ResidencyViolation {
                site: site.clone(),
                permitted: label.residency.clone(),
            });
        }
    }

    if to.refines(from) {
        return Ok(ScopeMapping {
            from: from.clone(),
            to: to.clone(),
            kind: MappingKind::Restriction,
            loss: LossLedger::default(),
        });
    }

    if label.export == ExportPolicy::NoExport {
        return Err(Refusal::ExportForbidden {
            export: label.export,
            detail: format!("evidence may not leave the scope {}", describe_scope(from)),
        });
    }

    let mut loss = LossLedger::default()
        .conditioned_on(format!("classification={}", label.classification))
        .conditioned_on(format!("export={}", label.export))
        .conditioned_on(format!("purposes={}", label.purposes));

    for compartment in &label.compartments {
        loss = loss.conditioned_on(format!("compartment={compartment}"));
    }

    if label.export == ExportPolicy::AggregatesOnly {
        loss = loss
            .discarding("individual-level sections; only approved aggregates cross")
            .adding_uncertainty(
                "per-subject resolution is unavailable downstream of this boundary",
            );
    }

    if label.min_cell_size > 0 {
        loss = loss
            .discarding(format!(
                "cells with fewer than {} members are suppressed",
                label.min_cell_size
            ))
            .adding_uncertainty("suppressed cells are reported as bounded unknowns");
    }

    Ok(ScopeMapping {
        from: from.clone(),
        to: to.clone(),
        kind: MappingKind::Transport { justification },
        loss,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::Classification;
    use bioprism_scope::MappingCheck;

    fn eu_label() -> PolicyLabel {
        PolicyLabel::public()
            .with_classification(Classification::ControlledGenomicOrImaging)
            .with_residency(Residency::only(["eu"]))
            .with_export(ExportPolicy::AggregatesOnly)
    }

    #[test]
    fn a_policy_crossing_move_is_a_declared_transport_with_a_non_empty_loss_ledger() {
        let from = ScopeKey::new()
            .exact("cohort", "GLIOMA-EU")
            .exact("residency", "eu");
        let to = ScopeKey::new()
            .exact("cohort", "POOLED")
            .exact("residency", "eu");

        let mapping = propose_transport(&eu_label(), &from, &to, "approved EU-internal pooling")
            .expect("an in-territory pooling is legal");

        assert!(matches!(mapping.kind, MappingKind::Transport { .. }));
        assert!(!mapping.loss.is_empty(), "a transport must state its price");
        assert_eq!(mapping.check(), MappingCheck::Sound);
    }

    #[test]
    fn narrowing_inside_one_jurisdiction_is_a_restriction_not_a_transport() {
        let from = ScopeKey::new().exact("residency", "eu");
        let to = ScopeKey::new()
            .exact("residency", "eu")
            .exact("cohort", "GLIOMA-EU");

        let mapping =
            propose_transport(&eu_label(), &from, &to, "narrow to one cohort").expect("legal");

        assert_eq!(mapping.kind, MappingKind::Restriction);
        assert!(mapping.loss.is_empty(), "narrowing discards nothing");
        assert_eq!(mapping.check(), MappingCheck::Sound);
    }

    #[test]
    fn a_move_into_a_forbidden_jurisdiction_is_refused_rather_than_produced() {
        let from = ScopeKey::new().exact("residency", "eu");
        let to = ScopeKey::new()
            .exact("cohort", "POOLED")
            .exact("residency", "us");

        let refusal = propose_transport(&eu_label(), &from, &to, "central pooling")
            .expect_err("a US destination is outside the EU residency");

        assert!(matches!(refusal, Refusal::ResidencyViolation { .. }));
    }

    #[test]
    fn a_destination_that_does_not_say_where_it_is_is_refused_not_assumed_local() {
        let from = ScopeKey::new().exact("residency", "eu");
        let to = ScopeKey::new().exact("cohort", "POOLED");

        let refusal = propose_transport(&eu_label(), &from, &to, "central pooling")
            .expect_err("an unstated destination cannot be checked");

        assert!(matches!(refusal, Refusal::UndeclaredDestination { .. }));
    }

    #[test]
    fn a_multi_territory_destination_is_judged_by_its_weakest_member() {
        let from = ScopeKey::new().exact("residency", "eu");
        let to = ScopeKey::new().bind(
            "residency",
            ScopeValue::OneOf(["eu".to_string(), "us".to_string()].into_iter().collect()),
        );

        let refusal = propose_transport(&eu_label(), &from, &to, "regional mirror")
            .expect_err("one impermissible member is enough to refuse");

        assert!(
            matches!(refusal, Refusal::ResidencyViolation { site, .. } if site.as_str() == "us")
        );
    }

    #[test]
    fn a_no_export_label_cannot_be_transported_even_within_its_own_territory() {
        let label = eu_label().with_export(ExportPolicy::NoExport);
        let from = ScopeKey::new()
            .exact("cohort", "GLIOMA-EU")
            .exact("residency", "eu");
        let to = ScopeKey::new()
            .exact("cohort", "POOLED")
            .exact("residency", "eu");

        let refusal = propose_transport(&label, &from, &to, "approved pooling")
            .expect_err("no-export forbids the move even inside the jurisdiction");

        assert!(matches!(refusal, Refusal::ExportForbidden { .. }));
    }

    #[test]
    fn a_transport_nobody_justified_is_refused_so_the_ledger_cannot_be_a_formality() {
        let from = ScopeKey::new().exact("residency", "eu");
        let to = ScopeKey::new()
            .exact("cohort", "POOLED")
            .exact("residency", "eu");

        let refusal = propose_transport(&eu_label(), &from, &to, "   ")
            .expect_err("blank justification is not a justification");

        assert!(matches!(
            refusal,
            Refusal::TransportWithoutJustification { .. }
        ));
    }

    #[test]
    fn two_disjoint_residencies_intersect_to_nowhere_rather_than_to_anywhere() {
        let eu = Residency::only(["eu"]);
        let us = Residency::only(["us"]);
        let joined = eu.intersect(&us);
        assert!(joined.is_nowhere());
        assert!(!joined.admits(&Jurisdiction::new("eu")));
        assert!(!joined.admits(&Jurisdiction::new("us")));
    }
}
