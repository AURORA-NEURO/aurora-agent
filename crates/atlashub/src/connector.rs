//! Data connector registry — blueprint 34.12.
//!
//! 34.12 wants adapters *"to public, controlled, institutional, and bring-your-own-data resources
//! without centralizing everything"*, and asks each to declare schema and modality support,
//! authentication mode, data use and egress, query operations, execution locality, version and
//! health, and sample artifacts. Read as a form to fill in, that is a metadata schema. Read as a
//! contract, it is three predicates, and this module is those three.
//!
//! # A connector declares what it can fetch, at what scope, with what loss
//!
//! The three parts compose two crates rather than restating them.
//!
//! **What it can fetch, and at what scope** — [`Connector::modalities`] and the source and target
//! of [`Connector::mapping`], in `bioprism-scope`'s vocabulary. A request is matched against a
//! connector with the refinement order rather than by string equality, so a connector that serves
//! `site=A` genuinely cannot answer a question about `site=B` even when both are called "MRI".
//!
//! **With what loss** — `bioprism-adapter` owns ingestion and the reporting of semantic loss; the
//! shared type is [`ScopeMapping`], whose [`LossLedger`] is exactly that report.
//! [`Connector::declare`] refuses a mapping whose [`ScopeMapping::check`] is not
//! [`MappingCheck::Sound`], and adds one rule of its own that `bioprism-scope` leaves open: an
//! [`MappingKind::Aggregation`] with an empty ledger is also refused, because summarising many
//! sub-scopes into one always discards the variation between them, and a connector that says
//! otherwise is describing a different operation.
//!
//! **Whether it may be relied on** — `bioprism-sdk` established that a plugin with no conformance
//! evidence is not selectable for load-bearing work. [`select`] applies that rule verbatim:
//! [`Use::LoadBearing`] requires a [`Conformance`] record whose scope *contains* the request's, and
//! [`Use::Exploratory`] does not but says so in the [`Selection`] it returns. The asymmetry matters:
//! a researcher poking at a new source should not be blocked, and a published result should not
//! rest on the poke.
//!
//! # Egress is a gate, not a field
//!
//! [`Egress`] is ordered by permissiveness and checked as an admission rule. A connector that
//! permits aggregate-only export cannot satisfy a request for record-level data at any price;
//! [`ConnectorError::EgressRefused`] names both sides. This is the same shape as
//! [`crate::voe::Privacy`] — a boundary that no numerator crosses — and the same reason: the
//! alternative is an exchange rate on data protection.
//!
//! # Not implemented
//!
//! No fetching, no authentication, no query execution, no credential storage, no schema inference.
//! [`AuthMode`] records which of 34.12's four modes a connector needs so that a planner can tell
//! which requests will require a human; nothing here obtains a credential. No conformance *suite* —
//! 34.12 lists "connector conformance" as a product metric and defines no suite, so
//! [`Conformance`] is a record of somebody else's run, identified by digest, with a denominator
//! that cannot be zero. No sample artifacts: they are files.

use crate::error::ConnectorError;
use bioprism_ids::ContentHash;
use bioprism_scope::{LossLedger, MappingCheck, MappingKind, ScopeKey, ScopeMapping};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Names a registered connector.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectorId(String);

impl ConnectorId {
    pub fn new(id: impl Into<String>) -> ConnectorId {
        ConnectorId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How much data may leave the source, ordered from tightest to loosest.
///
/// The `Ord` derive follows declaration order and is the whole enforcement mechanism: `permitted >=
/// requested` is the admission test. A deployment that wants a different lattice changes this one
/// derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Egress {
    /// Nothing leaves. Computation happens at the source; see [`crate::federated`].
    None,
    /// Only summaries leave, subject to whatever small-cell policy the site applies.
    AggregateOnly,
    /// Individual records may be exported.
    RecordLevel,
}

impl Egress {
    pub fn as_str(self) -> &'static str {
        match self {
            Egress::None => "none",
            Egress::AggregateOnly => "aggregate-only",
            Egress::RecordLevel => "record-level",
        }
    }

    pub fn permits(self, requested: Egress) -> bool {
        self >= requested
    }
}

impl fmt::Display for Egress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 34.12's "authentication mode", recorded so a planner can predict which requests need a human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// Open data. The only mode a no-key demo can use.
    Anonymous,
    ApiKey,
    /// A data-access committee stands between the request and the data.
    ControlledAccess,
    /// Credentials exist only inside the site and never reach a coordinator.
    SiteLocal,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthMode::Anonymous => "anonymous",
            AuthMode::ApiKey => "api-key",
            AuthMode::ControlledAccess => "controlled-access",
            AuthMode::SiteLocal => "site-local",
        }
    }

    /// Whether a request can proceed without a human in the loop.
    pub fn is_unattended(self) -> bool {
        matches!(self, AuthMode::Anonymous | AuthMode::ApiKey)
    }
}

/// 34.12's "version and health".
///
/// Mirrors the three states [`crate::card::WorldHealth`] uses, deliberately as a separate type: a
/// quarantined connector and a quarantined world are quarantined for different reasons and are
/// repaired by different people, and one enum shared between them would invite one dashboard that
/// conflates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Active,
    /// Reachable, but its last conformance run is old enough that its behaviour is unverified.
    Stale,
    /// Known to be wrong. Not selectable at all.
    Quarantined,
}

impl Health {
    pub fn as_str(self) -> &'static str {
        match self {
            Health::Active => "active",
            Health::Stale => "stale",
            Health::Quarantined => "quarantined",
        }
    }
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Evidence that somebody ran a conformance suite against this connector, and where.
///
/// The scope is the point. Evidence gathered inside `site=A, modality=mri` says nothing about
/// `site=B`, and [`Conformance::covers`] uses `bioprism-scope`'s refinement order to say so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conformance {
    pub suite: String,
    /// The scope the suite exercised. A request is covered only if it refines this.
    pub at_scope: ScopeKey,
    passed: u32,
    total: u32,
    /// Identifies the run, so a reader can find out what "passed" meant.
    pub evidence: ContentHash,
}

impl Conformance {
    pub fn recorded(
        connector: &ConnectorId,
        suite: impl Into<String>,
        at_scope: ScopeKey,
        passed: u32,
        total: u32,
        evidence: ContentHash,
    ) -> Result<Conformance, ConnectorError> {
        if total == 0 {
            return Err(ConnectorError::EmptyConformance {
                connector: connector.to_string(),
            });
        }
        Ok(Conformance {
            suite: suite.into(),
            at_scope,
            passed: passed.min(total),
            total,
            evidence,
        })
    }

    pub fn passed(&self) -> u32 {
        self.passed
    }

    pub fn total(&self) -> u32 {
        self.total
    }

    pub fn is_complete(&self) -> bool {
        self.passed == self.total
    }

    /// Whether this evidence reaches the requested scope.
    pub fn covers(&self, request: &ScopeKey) -> bool {
        request.refines(&self.at_scope)
    }
}

/// A registered connector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connector {
    id: ConnectorId,
    modalities: BTreeSet<String>,
    mapping: ScopeMapping,
    auth: AuthMode,
    egress: Egress,
    health: Health,
    conformance: Vec<Conformance>,
}

impl Connector {
    /// Register a connector, or refuse it.
    ///
    /// The refusals here are the ones that make the registry worth having: a connector that
    /// declares no modality can never be matched, and a connector that reshapes data without
    /// saying what it discarded will silently degrade every result downstream of it.
    pub fn declare(
        id: ConnectorId,
        modalities: impl IntoIterator<Item = String>,
        mapping: ScopeMapping,
        auth: AuthMode,
        egress: Egress,
    ) -> Result<Connector, ConnectorError> {
        let modalities: BTreeSet<String> = modalities.into_iter().collect();
        if modalities.is_empty() {
            return Err(ConnectorError::FetchesNothing {
                connector: id.to_string(),
            });
        }

        match mapping.check() {
            MappingCheck::Sound => {}
            MappingCheck::UndeclaredLoss => {
                return Err(ConnectorError::UndeclaredSemanticLoss {
                    connector: id.to_string(),
                    kind: kind_name(&mapping.kind).to_string(),
                })
            }
            MappingCheck::MisdeclaredRestriction => {
                return Err(ConnectorError::UndeclaredSemanticLoss {
                    connector: id.to_string(),
                    kind: "misdeclared restriction".to_string(),
                })
            }
        }

        if matches!(mapping.kind, MappingKind::Aggregation { .. }) && mapping.loss.is_empty() {
            return Err(ConnectorError::UndeclaredSemanticLoss {
                connector: id.to_string(),
                kind: kind_name(&mapping.kind).to_string(),
            });
        }

        Ok(Connector {
            id,
            modalities,
            mapping,
            auth,
            egress,
            health: Health::Active,
            conformance: Vec::new(),
        })
    }

    pub fn in_health(mut self, health: Health) -> Connector {
        self.health = health;
        self
    }

    pub fn with_conformance(mut self, conformance: Conformance) -> Connector {
        self.conformance.push(conformance);
        self
    }

    pub fn id(&self) -> &ConnectorId {
        &self.id
    }

    pub fn modalities(&self) -> impl Iterator<Item = &str> {
        self.modalities.iter().map(String::as_str)
    }

    pub fn mapping(&self) -> &ScopeMapping {
        &self.mapping
    }

    /// What using this connector costs in fidelity. Never absent: a connector with a lossless
    /// mapping carries an empty ledger, and an empty ledger is only legal on a restriction.
    pub fn semantic_loss(&self) -> &LossLedger {
        &self.mapping.loss
    }

    pub fn auth(&self) -> AuthMode {
        self.auth
    }

    pub fn egress(&self) -> Egress {
        self.egress
    }

    pub fn health(&self) -> Health {
        self.health
    }

    pub fn conformance(&self) -> &[Conformance] {
        &self.conformance
    }

    /// The conformance record that covers a scope, if any.
    pub fn conformance_for(&self, scope: &ScopeKey) -> Option<&Conformance> {
        self.conformance.iter().find(|c| c.covers(scope))
    }
}

fn kind_name(kind: &MappingKind) -> &'static str {
    match kind {
        MappingKind::Restriction => "restriction",
        MappingKind::Transport { .. } => "transport",
        MappingKind::Aggregation { .. } => "aggregation",
        MappingKind::Extension { .. } => "extension",
    }
}

/// What the caller intends to do with the data.
///
/// `bioprism-sdk`'s rule in two variants: evidence is required for the first and not the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Use {
    /// A published result will rest on this.
    LoadBearing,
    /// Somebody is looking around.
    Exploratory,
}

/// What a caller wants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fetch {
    pub modality: String,
    pub scope: ScopeKey,
    /// The loosest egress the caller needs. Asking for less than you need is the safe direction.
    pub egress: Egress,
    pub purpose: Use,
}

impl Fetch {
    pub fn new(modality: impl Into<String>, scope: ScopeKey, purpose: Use) -> Fetch {
        Fetch {
            modality: modality.into(),
            scope,
            egress: Egress::AggregateOnly,
            purpose,
        }
    }

    pub fn needing(mut self, egress: Egress) -> Fetch {
        self.egress = egress;
        self
    }
}

/// A connector chosen for a request, and everything the choice costs.
///
/// [`Selection::loss`] is a copy rather than a reference so that a plan can be serialized and
/// audited after the registry it came from is gone. [`Selection::conformance`] is `None` only for
/// [`Use::Exploratory`]; a load-bearing selection always carries its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub connector: ConnectorId,
    pub purpose: Use,
    pub loss: LossLedger,
    pub conformance: Option<Conformance>,
    /// Set when the selection was made without conformance evidence. Present so a downstream
    /// reader sees the caveat even if they never look at [`Selection::conformance`].
    pub unverified: bool,
}

/// Choose a connector, or say precisely why none will do.
///
/// Candidates are considered in id order so the choice is reproducible, and the *last* refusal is
/// reported when several connectors fail differently — deliberately, because the refusals are
/// ordered from least to most specific and the most specific one is the one worth reading.
pub fn select(connectors: &[Connector], request: &Fetch) -> Result<Selection, ConnectorError> {
    let mut ordered: Vec<&Connector> = connectors.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));

    let mut last: Option<ConnectorError> = None;

    for connector in ordered {
        match consider(connector, request) {
            Ok(selection) => return Ok(selection),
            Err(err) => last = Some(err),
        }
    }

    Err(last.unwrap_or(ConnectorError::ModalityNotDeclared {
        connector: "<none registered>".to_string(),
        modality: request.modality.clone(),
    }))
}

fn consider(connector: &Connector, request: &Fetch) -> Result<Selection, ConnectorError> {
    let name = connector.id.to_string();

    if connector.health == Health::Quarantined {
        return Err(ConnectorError::NotSelectable {
            connector: name,
            health: connector.health.to_string(),
        });
    }

    if !connector.modalities.contains(&request.modality) {
        return Err(ConnectorError::ModalityNotDeclared {
            connector: name,
            modality: request.modality.clone(),
        });
    }

    if !connector.egress.permits(request.egress) {
        return Err(ConnectorError::EgressRefused {
            connector: name,
            permitted: connector.egress.to_string(),
            requested: request.egress.to_string(),
        });
    }

    let conformance = connector.conformance_for(&request.scope);

    if request.purpose == Use::LoadBearing {
        if connector.conformance.is_empty() {
            return Err(ConnectorError::NoConformanceEvidence { connector: name });
        }
        if conformance.is_none() {
            return Err(ConnectorError::ConformanceOutOfScope { connector: name });
        }
        if connector.health == Health::Stale {
            return Err(ConnectorError::NotSelectable {
                connector: name,
                health: connector.health.to_string(),
            });
        }
    }

    Ok(Selection {
        connector: connector.id.clone(),
        purpose: request.purpose,
        loss: connector.mapping.loss.clone(),
        conformance: conformance.cloned(),
        unverified: conformance.is_none(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_scope::AggregationOperator;

    fn source() -> ScopeKey {
        ScopeKey::new().exact("site", "A")
    }

    fn target() -> ScopeKey {
        ScopeKey::new().exact("site", "A").exact("modality", "mri")
    }

    fn restriction() -> ScopeMapping {
        ScopeMapping {
            from: source(),
            to: target(),
            kind: MappingKind::Restriction,
            loss: LossLedger::default(),
        }
    }

    fn id(name: &str) -> ConnectorId {
        ConnectorId::new(name)
    }

    fn conformance(connector: &ConnectorId, at: ScopeKey) -> Conformance {
        Conformance::recorded(
            connector,
            "connector-conformance-v1",
            at,
            40,
            40,
            ContentHash::of_bytes(b"run"),
        )
        .unwrap()
    }

    fn plain(name: &str) -> Connector {
        Connector::declare(
            id(name),
            ["mri".to_string()],
            restriction(),
            AuthMode::Anonymous,
            Egress::RecordLevel,
        )
        .unwrap()
    }

    #[test]
    fn a_connector_that_declares_no_modality_is_not_registrable() {
        let err = Connector::declare(
            id("empty"),
            Vec::<String>::new(),
            restriction(),
            AuthMode::Anonymous,
            Egress::None,
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorError::FetchesNothing { .. }));
    }

    #[test]
    fn a_transport_mapping_with_an_empty_loss_ledger_is_refused() {
        let mapping = ScopeMapping {
            from: source(),
            to: ScopeKey::new().exact("site", "B"),
            kind: MappingKind::Transport {
                justification: "sites are similar".to_string(),
            },
            loss: LossLedger::default(),
        };
        let err = Connector::declare(
            id("x"),
            ["mri".to_string()],
            mapping,
            AuthMode::Anonymous,
            Egress::AggregateOnly,
        )
        .unwrap_err();
        assert_eq!(
            err,
            ConnectorError::UndeclaredSemanticLoss {
                connector: "x".to_string(),
                kind: "transport".to_string()
            }
        );
    }

    #[test]
    fn an_aggregating_connector_that_reports_no_loss_is_refused() {
        let mapping = ScopeMapping {
            from: target(),
            to: source(),
            kind: MappingKind::Aggregation {
                operator: AggregationOperator::Mean,
            },
            loss: LossLedger::default(),
        };
        let err = Connector::declare(
            id("x"),
            ["mri".to_string()],
            mapping,
            AuthMode::Anonymous,
            Egress::AggregateOnly,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ConnectorError::UndeclaredSemanticLoss { ref kind, .. } if kind == "aggregation"
        ));
    }

    #[test]
    fn an_aggregating_connector_that_names_what_it_discarded_is_registrable() {
        let mapping = ScopeMapping {
            from: target(),
            to: source(),
            kind: MappingKind::Aggregation {
                operator: AggregationOperator::Mean,
            },
            loss: LossLedger::default().discarding("per-slice variation"),
        };
        let connector = Connector::declare(
            id("x"),
            ["mri".to_string()],
            mapping,
            AuthMode::Anonymous,
            Egress::AggregateOnly,
        )
        .unwrap();
        assert!(!connector.semantic_loss().is_empty());
    }

    #[test]
    fn a_mapping_that_calls_a_widening_a_restriction_is_refused() {
        let mapping = ScopeMapping {
            from: target(),
            to: source(),
            kind: MappingKind::Restriction,
            loss: LossLedger::default(),
        };
        assert!(Connector::declare(
            id("x"),
            ["mri".to_string()],
            mapping,
            AuthMode::Anonymous,
            Egress::None,
        )
        .is_err());
    }

    #[test]
    fn conformance_with_a_zero_denominator_is_not_constructible() {
        let err = Conformance::recorded(
            &id("x"),
            "suite",
            target(),
            0,
            0,
            ContentHash::of_bytes(b"r"),
        )
        .unwrap_err();
        assert!(matches!(err, ConnectorError::EmptyConformance { .. }));
    }

    #[test]
    fn a_connector_with_no_conformance_evidence_is_not_selectable_for_load_bearing_work() {
        let connectors = vec![plain("a")];
        let request = Fetch::new("mri", target(), Use::LoadBearing);
        assert!(matches!(
            select(&connectors, &request),
            Err(ConnectorError::NoConformanceEvidence { .. })
        ));
    }

    #[test]
    fn the_same_connector_is_selectable_for_exploration_and_says_it_is_unverified() {
        let connectors = vec![plain("a")];
        let request = Fetch::new("mri", target(), Use::Exploratory);
        let selection = select(&connectors, &request).unwrap();
        assert!(selection.unverified);
        assert!(selection.conformance.is_none());
    }

    #[test]
    fn conformance_gathered_in_a_narrower_scope_does_not_license_a_wider_request() {
        let narrow = target().exact("scanner", "s1");
        let connector = plain("a").with_conformance(conformance(&id("a"), narrow));
        let request = Fetch::new("mri", target(), Use::LoadBearing);
        assert!(matches!(
            select(&[connector], &request),
            Err(ConnectorError::ConformanceOutOfScope { .. })
        ));
    }

    #[test]
    fn conformance_gathered_in_a_wider_scope_does_license_a_narrower_request() {
        let connector = plain("a").with_conformance(conformance(&id("a"), source()));
        let request = Fetch::new("mri", target(), Use::LoadBearing);
        let selection = select(&[connector], &request).unwrap();
        assert!(!selection.unverified);
        assert!(selection.conformance.is_some());
    }

    #[test]
    fn an_aggregate_only_connector_cannot_satisfy_a_record_level_request() {
        let connector = Connector::declare(
            id("a"),
            ["mri".to_string()],
            restriction(),
            AuthMode::ControlledAccess,
            Egress::AggregateOnly,
        )
        .unwrap()
        .with_conformance(conformance(&id("a"), source()));
        let request = Fetch::new("mri", target(), Use::LoadBearing).needing(Egress::RecordLevel);
        assert_eq!(
            select(&[connector], &request).unwrap_err(),
            ConnectorError::EgressRefused {
                connector: "a".to_string(),
                permitted: "aggregate-only".to_string(),
                requested: "record-level".to_string(),
            }
        );
    }

    #[test]
    fn a_no_egress_connector_still_serves_a_request_that_needs_nothing_out() {
        let connector = Connector::declare(
            id("a"),
            ["mri".to_string()],
            restriction(),
            AuthMode::SiteLocal,
            Egress::None,
        )
        .unwrap();
        let request = Fetch::new("mri", target(), Use::Exploratory).needing(Egress::None);
        assert!(select(&[connector], &request).is_ok());
    }

    #[test]
    fn a_quarantined_connector_is_not_selectable_even_for_exploration() {
        let connector = plain("a").in_health(Health::Quarantined);
        let request = Fetch::new("mri", target(), Use::Exploratory);
        assert!(matches!(
            select(&[connector], &request),
            Err(ConnectorError::NotSelectable { .. })
        ));
    }

    #[test]
    fn a_stale_connector_may_be_explored_but_not_relied_on() {
        let connector = plain("a")
            .in_health(Health::Stale)
            .with_conformance(conformance(&id("a"), source()));
        let explore = Fetch::new("mri", target(), Use::Exploratory);
        assert!(select(std::slice::from_ref(&connector), &explore).is_ok());
        let rely = Fetch::new("mri", target(), Use::LoadBearing);
        assert!(matches!(
            select(&[connector], &rely),
            Err(ConnectorError::NotSelectable { .. })
        ));
    }

    #[test]
    fn a_request_for_an_undeclared_modality_names_the_modality() {
        let connectors = vec![plain("a")];
        let request = Fetch::new("pathology", target(), Use::Exploratory);
        assert_eq!(
            select(&connectors, &request).unwrap_err(),
            ConnectorError::ModalityNotDeclared {
                connector: "a".to_string(),
                modality: "pathology".to_string()
            }
        );
    }

    #[test]
    fn selection_is_deterministic_when_several_connectors_would_do() {
        let a = plain("zzz").with_conformance(conformance(&id("zzz"), source()));
        let b = plain("aaa").with_conformance(conformance(&id("aaa"), source()));
        let request = Fetch::new("mri", target(), Use::LoadBearing);
        let first = select(&[a.clone(), b.clone()], &request).unwrap();
        let second = select(&[b, a], &request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.connector, ConnectorId::new("aaa"));
    }

    #[test]
    fn a_selection_carries_the_loss_ledger_forward_so_a_plan_can_be_audited_later() {
        let mapping = ScopeMapping {
            from: source(),
            to: ScopeKey::new().exact("site", "B"),
            kind: MappingKind::Transport {
                justification: "harmonised protocol".to_string(),
            },
            loss: LossLedger::default()
                .discarding("scanner make")
                .adding_uncertainty("harmonisation residual"),
        };
        let connector = Connector::declare(
            id("a"),
            ["mri".to_string()],
            mapping,
            AuthMode::ApiKey,
            Egress::AggregateOnly,
        )
        .unwrap();
        let request = Fetch::new("mri", ScopeKey::new().exact("site", "B"), Use::Exploratory);
        let selection = select(&[connector], &request).unwrap();
        let json = serde_json::to_string(&selection).unwrap();
        assert!(json.contains("scanner make"));
        assert!(json.contains("harmonisation residual"));
    }

    #[test]
    fn selecting_from_an_empty_registry_names_the_modality_nobody_serves() {
        let request = Fetch::new("mri", target(), Use::Exploratory);
        assert!(matches!(
            select(&[], &request),
            Err(ConnectorError::ModalityNotDeclared { .. })
        ));
    }

    #[test]
    fn an_anonymous_connector_is_the_only_kind_a_no_key_demo_can_use_unattended() {
        assert!(AuthMode::Anonymous.is_unattended());
        assert!(!AuthMode::ControlledAccess.is_unattended());
        assert!(!AuthMode::SiteLocal.is_unattended());
    }
}
