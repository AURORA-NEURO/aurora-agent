//! Federated and bring-your-own-data evaluation — blueprint 34.17.
//!
//! 34.17 wants *"standardized evaluation where sensitive imaging, genomics, pathology, or pediatric
//! data remain under local control"*, and lists a signed worker, pack pull with policy
//! verification, local execution, aggregate or encrypted return, small-cell protection, site
//! attestation and cross-site meta-analysis. Most of that is deployment. One sentence in it is a
//! type:
//!
//! > A federated result carries **less** evidence than a centralised one, and must say what it
//! > could not check.
//!
//! Everything in this module is that sentence.
//!
//! # Why "less", and why it is not a matter of degree
//!
//! When the data does not move, the coordinator never sees a record. It therefore cannot audit the
//! cohort, cannot check whether the same patient appears at two sites, cannot verify that the
//! site's own suppression was applied before the number was computed rather than after, and cannot
//! confirm that the local data is what the world card said it was. Those are not risks that a
//! careful site can eliminate — they are consequences of the architecture, and a federated result
//! that reported none of them would be claiming it had been centralised.
//!
//! So [`SiteResult::report`] adds [`Unchecked::RecordLevelAudit`] to every site result
//! unconditionally, [`pool`] adds [`Unchecked::CohortOverlapAcrossSites`] to every pooled result
//! unconditionally, and a bring-your-own-data run adds [`Unchecked::ProvenanceOfLocalData`] —
//! the direct consequence of [`crate::card`]: the coordinator cannot verify a provenance rung it
//! never saw.
//!
//! [`FederatedResult`] has no `centralise`, no `promote`, and no `From` impl in either direction.
//! Its `evaluation` field is a one-variant [`Evaluation`] that serializes as `"federated"` on every
//! result this module can produce, and deserialization refuses a result whose unchecked set is
//! empty.
//!
//! # Pooling never buys evidence back
//!
//! [`pool`] takes the **union** of every site's unchecked set. Adding a site can only enlarge it.
//! This is the checkable form of "less evidence": a reader who sees a twelve-site result and
//! assumes the scale bought rigour is corrected by a longer list of things nobody checked, not a
//! shorter one.
//!
//! # Trust does not transit, applied to sites
//!
//! `bioprism-hubapi` made a tier a property of the registry that computed it, with no function of
//! shape `(TrustStanding, RegistryId) -> TrustStanding`. This module consumes that type and cannot
//! mint one: [`SiteParticipation::admit`] takes a
//! [`TrustStanding`] somebody else established and
//! rejects it unless it *already* holds in the participating site, with
//! [`FederatedError::StandingDoesNotHoldAtSite`]. A coordinator that vouched for a pack in its own
//! registry has said nothing about whether a hospital may run it.
//!
//! A site is identified by [`RegistryId`] rather than by a new `SiteId`. Introducing a parallel
//! identity space would mean the standing's registry and the site's name could not be compared at
//! all, and the check above would become a string match on two unrelated namespaces.
//!
//! # Not implemented
//!
//! No cryptography, no signed worker, no secure aggregation, no differential privacy, no encrypted
//! return channel, no transport of any kind. 34.17 wants all of them and every one is deployment.
//! No small-cell *threshold*: [`SmallCellPolicy`] takes it from the caller, because the right
//! number is a function of jurisdiction and of how many pediatric subgroups a cohort has, and a
//! default here would be silently wrong somewhere. No meta-analytic estimator beyond the
//! sample-size weighted mean of [`FederatedResult::weighted_mean`], which is documented as an
//! assumption rather than offered as a method: heterogeneity, random effects and between-site
//! variance all need a model that 34.17 does not supply.

use crate::error::FederatedError;
use bioprism_hubapi::federation::{Basis, TrustStanding};
use bioprism_hubapi::RegistryId;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// What a result's evidence base was.
///
/// One variant, because this module produces only one kind. A `Centralised` variant belongs to
/// whatever type describes a run where the coordinator held the records, and the absence of a
/// conversion between the two types is the point rather than an omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evaluation {
    Federated,
}

/// Something a federated run could not verify.
///
/// A closed set plus an escape hatch. Closed because the first five are architectural — they follow
/// from the data not moving, and a site cannot opt out of them — and open at the end because a
/// particular deployment will have its own, and forcing those into the wrong variant is worse than
/// admitting a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "unchecked", rename_all = "snake_case")]
pub enum Unchecked {
    /// The coordinator never saw a record. On every site result, always.
    RecordLevelAudit,
    /// Whether the same subject contributed at two sites. On every pooled result, always.
    CohortOverlapAcrossSites,
    /// Whether the site's inputs overlap a hidden holdout.
    LeakageAgainstHiddenHoldout,
    /// Whether suppression was applied before the statistic was computed or after.
    SmallCellSuppressionAtSource,
    /// Whether the sites are estimating the same quantity at all.
    InputDistributionShift,
    /// Who read the images, and whether the reader pool differs by site.
    OracleReaderIdentity,
    /// The world card's provenance rung could not be verified because the data is the site's own.
    ProvenanceOfLocalData,
    Other {
        what: String,
    },
}

impl fmt::Display for Unchecked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unchecked::RecordLevelAudit => f.write_str("record-level audit"),
            Unchecked::CohortOverlapAcrossSites => f.write_str("cohort overlap across sites"),
            Unchecked::LeakageAgainstHiddenHoldout => {
                f.write_str("leakage against the hidden holdout")
            }
            Unchecked::SmallCellSuppressionAtSource => {
                f.write_str("small-cell suppression at source")
            }
            Unchecked::InputDistributionShift => f.write_str("input distribution shift"),
            Unchecked::OracleReaderIdentity => f.write_str("oracle reader identity"),
            Unchecked::ProvenanceOfLocalData => f.write_str("provenance of local data"),
            Unchecked::Other { what } => f.write_str(what),
        }
    }
}

/// Where the data a site evaluated on came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum DataOrigin {
    /// The site ran the pack against data the pack described.
    PackShipped,
    /// 34.17's bring-your-own-data case. The world card digest is recorded when the site has one;
    /// `None` is the honest common case and produces an extra unchecked aspect either way, because
    /// a digest the coordinator cannot resolve is not a verification.
    BringYourOwn {
        world_card: Option<ContentHash>,
    },
}

/// A site admitted to a federated run.
///
/// Constructed only from a [`TrustStanding`] that already holds at the site. The digest is copied
/// out because comparability is about content: two sites that ran different bytes answered
/// different questions, whatever the pack was called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteParticipation {
    site: RegistryId,
    pack_digest: String,
    basis: Basis,
}

impl SiteParticipation {
    /// Admit a site, or refuse because the standing it offered belongs to somebody else.
    pub fn admit(
        site: RegistryId,
        standing: &TrustStanding,
    ) -> Result<SiteParticipation, FederatedError> {
        if !standing.holds_in(&site) {
            return Err(FederatedError::StandingDoesNotHoldAtSite {
                holds_in: standing.registry().to_string(),
                site: site.to_string(),
            });
        }
        Ok(SiteParticipation {
            site,
            pack_digest: standing.subject().digest.clone(),
            basis: standing.basis().clone(),
        })
    }

    pub fn site(&self) -> &RegistryId {
        &self.site
    }

    pub fn pack_digest(&self) -> &str {
        &self.pack_digest
    }

    /// Whether the site's standing was computed from the pack or deferred to a peer.
    ///
    /// Reported rather than gated: a site that adopted its standing may still participate, and a
    /// reader of the pooled result is entitled to know that its admission rested on somebody
    /// else's review.
    pub fn basis(&self) -> &Basis {
        &self.basis
    }
}

/// The minimum cell size a site will report on.
///
/// No default. 34.17 says "small-cell protection" and names no number, and this crate is not the
/// right place to guess a jurisdiction's disclosure rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmallCellPolicy {
    minimum: u64,
}

impl SmallCellPolicy {
    pub fn of(minimum: u64) -> Result<SmallCellPolicy, FederatedError> {
        if minimum == 0 {
            return Err(FederatedError::VacuousSmallCellPolicy);
        }
        Ok(SmallCellPolicy { minimum })
    }

    pub fn minimum(self) -> u64 {
        self.minimum
    }
}

/// What a site computed locally, before the policy is applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    pub n: u64,
    pub value: f64,
}

impl Aggregate {
    pub fn new(n: u64, value: f64) -> Aggregate {
        Aggregate { n, value }
    }
}

/// What leaves the site.
///
/// [`Reported::Suppressed`] carries no value field, so there is nothing to accidentally read. This
/// is the same shape `bioprism-atlas` uses for an unmeasured capability: the absence of a number,
/// not a number plus a flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reported", rename_all = "snake_case")]
pub enum Reported {
    Value { n: u64, value: f64 },
    Suppressed { below: u64 },
}

impl Reported {
    /// `None` for a suppressed cell. There is no `value_or_zero`, and a suppressed cell must never
    /// be plotted at the origin.
    pub fn value(&self) -> Option<f64> {
        match self {
            Reported::Value { value, .. } => Some(*value),
            Reported::Suppressed { .. } => None,
        }
    }

    pub fn n(&self) -> Option<u64> {
        match self {
            Reported::Value { n, .. } => Some(*n),
            Reported::Suppressed { .. } => None,
        }
    }

    pub fn is_suppressed(&self) -> bool {
        matches!(self, Reported::Suppressed { .. })
    }
}

/// One site's contribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteResult {
    pub participation: SiteParticipation,
    pub origin: DataOrigin,
    pub reported: Reported,
    unchecked: BTreeSet<Unchecked>,
}

impl SiteResult {
    /// Apply the small-cell policy and record what the site could not let anybody check.
    ///
    /// [`Unchecked::RecordLevelAudit`] is added whatever the caller passes, and
    /// [`Unchecked::ProvenanceOfLocalData`] is added for a bring-your-own-data run. Neither is
    /// removable, because neither is a choice the site made.
    pub fn report(
        participation: SiteParticipation,
        origin: DataOrigin,
        aggregate: Aggregate,
        policy: SmallCellPolicy,
        unchecked: impl IntoIterator<Item = Unchecked>,
    ) -> SiteResult {
        let reported = if aggregate.n < policy.minimum() {
            Reported::Suppressed {
                below: policy.minimum(),
            }
        } else {
            Reported::Value {
                n: aggregate.n,
                value: aggregate.value,
            }
        };

        let mut unchecked: BTreeSet<Unchecked> = unchecked.into_iter().collect();
        unchecked.insert(Unchecked::RecordLevelAudit);
        if matches!(origin, DataOrigin::BringYourOwn { .. }) {
            unchecked.insert(Unchecked::ProvenanceOfLocalData);
        }

        SiteResult {
            participation,
            origin,
            reported,
            unchecked,
        }
    }

    pub fn unchecked(&self) -> &BTreeSet<Unchecked> {
        &self.unchecked
    }
}

/// A pooled federated result.
///
/// Fields are private and the only constructor is [`pool`]. Deserialization routes through a shadow
/// that refuses an empty unchecked set, so a result cannot be edited into one that claims full
/// evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FederatedResultShadow")]
pub struct FederatedResult {
    evaluation: Evaluation,
    pack_digest: String,
    contributing: Vec<SiteResult>,
    unchecked: BTreeSet<Unchecked>,
}

#[derive(Deserialize)]
struct FederatedResultShadow {
    evaluation: Evaluation,
    pack_digest: String,
    contributing: Vec<SiteResult>,
    unchecked: BTreeSet<Unchecked>,
}

impl TryFrom<FederatedResultShadow> for FederatedResult {
    type Error = FederatedError;

    fn try_from(shadow: FederatedResultShadow) -> Result<Self, Self::Error> {
        if shadow.unchecked.is_empty() {
            return Err(FederatedError::ClaimsNothingUnchecked);
        }
        if shadow.contributing.is_empty() {
            return Err(FederatedError::NoSites);
        }
        Ok(FederatedResult {
            evaluation: shadow.evaluation,
            pack_digest: shadow.pack_digest,
            contributing: shadow.contributing,
            unchecked: shadow.unchecked,
        })
    }
}

impl FederatedResult {
    /// Always [`Evaluation::Federated`]. There is no path that changes it.
    pub fn evaluation(&self) -> Evaluation {
        self.evaluation
    }

    pub fn pack_digest(&self) -> &str {
        &self.pack_digest
    }

    pub fn sites(&self) -> impl Iterator<Item = &RegistryId> {
        self.contributing.iter().map(|s| s.participation.site())
    }

    pub fn contributing(&self) -> &[SiteResult] {
        &self.contributing
    }

    /// The sites that suppressed their cell. Named rather than counted, so a reader can see whether
    /// the suppression is concentrated in one subgroup.
    pub fn suppressed(&self) -> Vec<&RegistryId> {
        self.contributing
            .iter()
            .filter(|s| s.reported.is_suppressed())
            .map(|s| s.participation.site())
            .collect()
    }

    /// Everything nobody checked. Never empty.
    pub fn unchecked(&self) -> &BTreeSet<Unchecked> {
        &self.unchecked
    }

    /// The sample-size weighted mean, and the only pooling this crate performs.
    ///
    /// It refuses while any site suppressed its cell, because the alternative is to pool over a
    /// number that does not exist. It assumes every site estimates the same quantity — an
    /// assumption that is itself in [`FederatedResult::unchecked`] as
    /// [`Unchecked::InputDistributionShift`], added by [`pool`] on every result — and it does not
    /// estimate between-site variance, produce an interval, or apply a random-effects model.
    pub fn weighted_mean(&self) -> Result<f64, FederatedError> {
        if let Some(site) = self.contributing.iter().find(|s| s.reported.is_suppressed()) {
            return Err(FederatedError::SuppressedCellCannotPool {
                site: site.participation.site().to_string(),
            });
        }
        let mut total_n: u64 = 0;
        let mut total: f64 = 0.0;
        for site in &self.contributing {
            let (Some(n), Some(v)) = (site.reported.n(), site.reported.value()) else {
                continue;
            };
            total_n = total_n.saturating_add(n);
            total += v * n as f64;
        }
        if total_n == 0 {
            return Err(FederatedError::NoSites);
        }
        Ok(total / total_n as f64)
    }
}

/// Combine site results into one federated result, or say why they do not combine.
///
/// The unchecked set of the output is the union of every input's, plus
/// [`Unchecked::CohortOverlapAcrossSites`] and [`Unchecked::InputDistributionShift`], which pooling
/// itself creates. It is never smaller than any input's.
pub fn pool(results: &[SiteResult]) -> Result<FederatedResult, FederatedError> {
    if results.is_empty() {
        return Err(FederatedError::NoSites);
    }

    let mut ordered: Vec<&SiteResult> = results.iter().collect();
    ordered.sort_by(|a, b| a.participation.site().cmp(b.participation.site()));

    let mut seen: BTreeMap<&RegistryId, ()> = BTreeMap::new();
    for site in &ordered {
        if seen.insert(site.participation.site(), ()).is_some() {
            return Err(FederatedError::DuplicateSite {
                site: site.participation.site().to_string(),
            });
        }
    }

    let ours = ordered[0].participation.pack_digest().to_string();
    for site in &ordered {
        if site.participation.pack_digest() != ours {
            return Err(FederatedError::NotComparable {
                site: site.participation.site().to_string(),
                ours: ours.clone(),
                theirs: site.participation.pack_digest().to_string(),
            });
        }
    }

    let mut unchecked: BTreeSet<Unchecked> = BTreeSet::new();
    for site in &ordered {
        unchecked.extend(site.unchecked().iter().cloned());
    }
    unchecked.insert(Unchecked::CohortOverlapAcrossSites);
    unchecked.insert(Unchecked::InputDistributionShift);

    Ok(FederatedResult {
        evaluation: Evaluation::Federated,
        pack_digest: ours,
        contributing: ordered.into_iter().cloned().collect(),
        unchecked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A standing this crate cannot mint.
    ///
    /// `TrustStanding::earned` needs a `bioprism_registry::TierAssessment` and `adopt` needs a
    /// tier, and neither is reachable from here — which is exactly the property under test. The
    /// tests therefore deserialize one, the same way a deployment would receive it.
    fn standing(registry: &str, digest: &str) -> TrustStanding {
        let json = format!(
            r#"{{"registry":"{registry}","subject":{{"name":"bioatlas/glioma","version":"1.2.3","digest":"{digest}"}},"tier":"reviewed","basis":{{"basis":"earned"}}}}"#
        );
        serde_json::from_str(&json).expect("standing fixture")
    }

    fn site(name: &str) -> RegistryId {
        RegistryId::parse(name).unwrap()
    }

    fn participation(name: &str, digest: &str) -> SiteParticipation {
        SiteParticipation::admit(site(name), &standing(name, digest)).unwrap()
    }

    fn policy() -> SmallCellPolicy {
        SmallCellPolicy::of(10).unwrap()
    }

    fn result_at(name: &str, digest: &str, n: u64, value: f64) -> SiteResult {
        SiteResult::report(
            participation(name, digest),
            DataOrigin::PackShipped,
            Aggregate::new(n, value),
            policy(),
            [],
        )
    }

    #[test]
    fn a_standing_earned_at_another_registry_does_not_authorise_a_site() {
        let err = SiteParticipation::admit(site("hospital-b"), &standing("hospital-a", "d1"))
            .unwrap_err();
        assert_eq!(
            err,
            FederatedError::StandingDoesNotHoldAtSite {
                holds_in: "hospital-a".to_string(),
                site: "hospital-b".to_string(),
            }
        );
    }

    #[test]
    fn a_standing_that_holds_at_the_site_admits_it_and_records_the_basis() {
        let p = participation("hospital-a", "d1");
        assert_eq!(p.site(), &site("hospital-a"));
        assert_eq!(p.pack_digest(), "d1");
        assert!(p.basis().is_earned());
    }

    #[test]
    fn every_site_result_records_that_no_record_level_audit_was_possible() {
        let r = result_at("a", "d1", 100, 0.8);
        assert!(r.unchecked().contains(&Unchecked::RecordLevelAudit));
    }

    #[test]
    fn a_bring_your_own_data_site_also_records_that_provenance_was_unverifiable() {
        let r = SiteResult::report(
            participation("a", "d1"),
            DataOrigin::BringYourOwn { world_card: None },
            Aggregate::new(50, 0.5),
            policy(),
            [],
        );
        assert!(r.unchecked().contains(&Unchecked::ProvenanceOfLocalData));
    }

    #[test]
    fn a_world_card_digest_the_coordinator_cannot_resolve_is_still_unverified_provenance() {
        let r = SiteResult::report(
            participation("a", "d1"),
            DataOrigin::BringYourOwn {
                world_card: Some(ContentHash::of_bytes(b"card")),
            },
            Aggregate::new(50, 0.5),
            policy(),
            [],
        );
        assert!(r.unchecked().contains(&Unchecked::ProvenanceOfLocalData));
    }

    #[test]
    fn an_aggregate_below_the_small_cell_threshold_carries_no_value() {
        let r = result_at("a", "d1", 3, 0.9);
        assert!(r.reported.is_suppressed());
        assert_eq!(r.reported.value(), None);
        assert_eq!(r.reported.n(), None);
    }

    #[test]
    fn a_suppressed_cell_serialises_without_a_value_key() {
        let r = result_at("a", "d1", 3, 0.9);
        let json = serde_json::to_value(&r.reported).unwrap();
        assert!(json.get("value").is_none());
        assert_eq!(json["reported"], "suppressed");
    }

    #[test]
    fn a_small_cell_policy_of_zero_is_not_constructible() {
        assert_eq!(
            SmallCellPolicy::of(0).unwrap_err(),
            FederatedError::VacuousSmallCellPolicy
        );
    }

    #[test]
    fn a_suppressed_site_cannot_be_pooled_into_a_point_estimate() {
        let pooled = pool(&[result_at("a", "d1", 100, 0.8), result_at("b", "d1", 2, 0.1)]).unwrap();
        assert!(matches!(
            pooled.weighted_mean(),
            Err(FederatedError::SuppressedCellCannotPool { .. })
        ));
        assert_eq!(pooled.suppressed(), vec![&site("b")]);
    }

    #[test]
    fn pooling_unions_the_unchecked_set_and_never_shrinks_it() {
        let a = SiteResult::report(
            participation("a", "d1"),
            DataOrigin::PackShipped,
            Aggregate::new(100, 0.8),
            policy(),
            [Unchecked::OracleReaderIdentity],
        );
        let b = SiteResult::report(
            participation("b", "d1"),
            DataOrigin::PackShipped,
            Aggregate::new(100, 0.6),
            policy(),
            [Unchecked::LeakageAgainstHiddenHoldout],
        );
        let one = pool(std::slice::from_ref(&a)).unwrap();
        let both = pool(&[a, b]).unwrap();
        for aspect in one.unchecked() {
            assert!(both.unchecked().contains(aspect));
        }
        assert!(both.unchecked().len() > one.unchecked().len());
        assert!(both.unchecked().contains(&Unchecked::OracleReaderIdentity));
        assert!(both
            .unchecked()
            .contains(&Unchecked::LeakageAgainstHiddenHoldout));
    }

    #[test]
    fn pooling_always_adds_cohort_overlap_and_distribution_shift() {
        let pooled = pool(&[result_at("a", "d1", 100, 0.8)]).unwrap();
        assert!(pooled
            .unchecked()
            .contains(&Unchecked::CohortOverlapAcrossSites));
        assert!(pooled
            .unchecked()
            .contains(&Unchecked::InputDistributionShift));
    }

    #[test]
    fn sites_that_ran_different_pack_digests_are_not_comparable() {
        let err = pool(&[result_at("a", "d1", 100, 0.8), result_at("b", "d2", 100, 0.7)])
            .unwrap_err();
        assert!(matches!(err, FederatedError::NotComparable { .. }));
    }

    #[test]
    fn the_same_site_may_not_contribute_twice() {
        let err = pool(&[result_at("a", "d1", 100, 0.8), result_at("a", "d1", 90, 0.7)])
            .unwrap_err();
        assert_eq!(
            err,
            FederatedError::DuplicateSite {
                site: "a".to_string()
            }
        );
    }

    #[test]
    fn pooling_no_sites_is_refused_rather_than_returning_an_empty_result() {
        assert_eq!(pool(&[]).unwrap_err(), FederatedError::NoSites);
    }

    #[test]
    fn pooling_is_deterministic_whatever_order_the_sites_arrive_in() {
        let a = result_at("zebra", "d1", 100, 0.8);
        let b = result_at("alpha", "d1", 100, 0.6);
        let one = pool(&[a.clone(), b.clone()]).unwrap();
        let two = pool(&[b, a]).unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn the_weighted_mean_weights_by_sample_size() {
        let pooled = pool(&[
            result_at("a", "d1", 300, 1.0),
            result_at("b", "d1", 100, 0.0),
        ])
        .unwrap();
        assert_eq!(pooled.weighted_mean().unwrap(), 0.75);
    }

    #[test]
    fn a_pooled_result_is_still_federated_however_many_sites_contributed() {
        let pooled = pool(&[
            result_at("a", "d1", 100, 0.8),
            result_at("b", "d1", 100, 0.7),
            result_at("c", "d1", 100, 0.6),
        ])
        .unwrap();
        assert_eq!(pooled.evaluation(), Evaluation::Federated);
        let json = serde_json::to_value(&pooled).unwrap();
        assert_eq!(json["evaluation"], "federated");
    }

    #[test]
    fn a_federated_result_edited_to_claim_nothing_unchecked_will_not_deserialize() {
        let pooled = pool(&[result_at("a", "d1", 100, 0.8)]).unwrap();
        let mut value = serde_json::to_value(&pooled).unwrap();
        value["unchecked"] = serde_json::json!([]);
        let back = serde_json::from_value::<FederatedResult>(value);
        assert!(back.is_err());
    }

    #[test]
    fn a_federated_result_round_trips_with_its_unchecked_set_intact() {
        let pooled = pool(&[result_at("a", "d1", 100, 0.8)]).unwrap();
        let json = serde_json::to_string(&pooled).unwrap();
        let back: FederatedResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.unchecked(), pooled.unchecked());
        assert_eq!(back, pooled);
    }
}
