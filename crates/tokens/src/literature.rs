//! Literature claim context (39.14).
//!
//! 39.14 compiles papers into a decision-specific claim graph. Its four invariants are all about
//! refusing to let a compact rendering imply more than the sources said.
//!
//! # A citation is not support
//!
//! The first invariant, and the one this module spends most of its type surface on.
//! [`CitationRelation`] separates [`CitationRelation::Supports`] from
//! [`CitationRelation::Cites`], and [`ClaimCluster::summarise`] returns
//! [`LiteratureError::CitationCountedAsSupport`] for a claim whose only edges are citations. A
//! citation records that one paper mentioned another; treating that as agreement is how a claim
//! acquires a hundred supporters overnight without anyone having measured anything twice.
//!
//! # Abstract-only evidence is labelled, structurally
//!
//! [`EvidenceDepth`] is a required field on every [`SourceRecord`], not an optional annotation, so
//! there is no way to record support without recording how much of the source was read.
//! [`SupportSummary::depth_breakdown`] carries it into the summary, because the label is only
//! useful if it survives aggregation.
//!
//! # Study families, not papers
//!
//! 39.14's "same cohort counted repeatedly" failure. Sources declare a
//! [`SourceRecord::study_family`], and [`SupportSummary::independent_families`] counts those rather
//! than papers. Four analyses of one cohort are one observation; a summary that says "four studies
//! support this" has multiplied one dataset by its publication count.
//!
//! # The cutoff is enforced on availability, not publication
//!
//! [`SourceRecord::available_at`] is when the evidence became *available*, which for a preprint
//! that was later published is the preprint date and for an embargoed dataset is the release date.
//! 39.14's fourth invariant names both "publication date and evidence availability", and
//! availability is the one that binds.
//!
//! # Not implemented
//!
//! No retrieval, no adapters, no claim extraction, no deduplication heuristic. A
//! [`LiteratureClaim`] is a value somebody else extracted, and the study family is a caller's
//! assertion this module checks the consequences of rather than infers.

use crate::error::LiteratureError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How much of a source the claim rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDepth {
    /// The full text was read, including methods.
    FullText,
    /// Only the abstract. 39.14 requires this be labelled and it is, in the type.
    AbstractOnly,
    /// Only the title.
    TitleOnly,
    /// Only bibliographic metadata: the source was never read at all.
    MetadataOnly,
}

impl EvidenceDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceDepth::FullText => "full_text",
            EvidenceDepth::AbstractOnly => "abstract_only",
            EvidenceDepth::TitleOnly => "title_only",
            EvidenceDepth::MetadataOnly => "metadata_only",
        }
    }

    /// Whether a depth can carry a methods-level support claim at all.
    ///
    /// An abstract states a conclusion; it does not let a reader check the population, the assay or
    /// the exclusions, which are the three things 39.14 requires stay attached.
    pub fn supports_methods_level_claim(self) -> bool {
        matches!(self, EvidenceDepth::FullText)
    }
}

/// What one source says about a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationRelation {
    /// The source reports evidence for the claim.
    Supports,
    /// The source reports evidence against it.
    Contradicts,
    /// The source cites the claim's origin without independently testing it. Contributes nothing to
    /// support and everything to the appearance of it.
    Cites,
    /// The source is where a method came from, not evidence about the claim.
    MethodSource,
    /// Nobody has classified the relation. Never counts as support.
    Unassessed,
}

impl CitationRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            CitationRelation::Supports => "supports",
            CitationRelation::Contradicts => "contradicts",
            CitationRelation::Cites => "cites",
            CitationRelation::MethodSource => "method_source",
            CitationRelation::Unassessed => "unassessed",
        }
    }

    pub fn is_support(self) -> bool {
        matches!(self, CitationRelation::Supports)
    }

    pub fn is_contradiction(self) -> bool {
        matches!(self, CitationRelation::Contradicts)
    }
}

/// One source's contribution to a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub source_id: String,
    pub relation: CitationRelation,
    pub depth: EvidenceDepth,
    /// The cohort or dataset family. Two papers on one cohort share this.
    pub study_family: String,
    /// Epoch at which this evidence became available, not necessarily its publication date.
    pub available_at: u64,
    /// A locator precise enough to check the claim against: a figure, a table, a section.
    pub locator: String,
}

impl SourceRecord {
    pub fn new(
        source_id: impl Into<String>,
        relation: CitationRelation,
        depth: EvidenceDepth,
        study_family: impl Into<String>,
        available_at: u64,
        locator: impl Into<String>,
    ) -> Self {
        SourceRecord {
            source_id: source_id.into(),
            relation,
            depth,
            study_family: study_family.into(),
            available_at,
            locator: locator.into(),
        }
    }
}

/// The qualifiers 39.14 requires stay attached to a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimQualifiers {
    /// The population the claim was established in.
    pub population: String,
    /// The assay or method that established it.
    pub assay: String,
    /// Cohort exclusions and split rules, which 39.01 lists as non-compressible.
    #[serde(default)]
    pub exclusions: Vec<String>,
}

impl ClaimQualifiers {
    pub fn new(population: impl Into<String>, assay: impl Into<String>) -> Self {
        ClaimQualifiers {
            population: population.into(),
            assay: assay.into(),
            exclusions: Vec::new(),
        }
    }

    pub fn excluding(mut self, exclusion: impl Into<String>) -> Self {
        self.exclusions.push(exclusion.into());
        self
    }
}

/// A claim with its qualifiers and sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureClaim {
    pub claim_id: String,
    pub statement: String,
    pub qualifiers: ClaimQualifiers,
    pub sources: Vec<SourceRecord>,
}

impl LiteratureClaim {
    /// Build a claim, checking the qualifier and locator requirements at construction.
    pub fn declare(
        claim_id: impl Into<String>,
        statement: impl Into<String>,
        qualifiers: ClaimQualifiers,
        sources: Vec<SourceRecord>,
    ) -> Result<Self, LiteratureError> {
        let claim_id = claim_id.into();
        if qualifiers.population.trim().is_empty() {
            return Err(LiteratureError::PopulationMissing { claim: claim_id });
        }
        if qualifiers.assay.trim().is_empty() {
            return Err(LiteratureError::AssayMissing { claim: claim_id });
        }
        if sources.iter().any(|source| source.locator.trim().is_empty()) {
            return Err(LiteratureError::LocatorMissing { claim: claim_id });
        }
        Ok(LiteratureClaim {
            claim_id,
            statement: statement.into(),
            qualifiers,
            sources,
        })
    }

    /// Sources available at or before the cutoff.
    ///
    /// A filter rather than an error, because dropping a post-cutoff source is the correct
    /// behaviour; [`LiteratureClaim::check_cutoff`] is the version that objects, for a caller
    /// validating a claim somebody else compiled.
    pub fn visible_at(&self, cutoff: u64) -> Vec<&SourceRecord> {
        self.sources
            .iter()
            .filter(|source| source.available_at <= cutoff)
            .collect()
    }

    /// Objects when any source postdates the cutoff.
    pub fn check_cutoff(&self, cutoff: u64) -> Result<(), LiteratureError> {
        if let Some(source) = self
            .sources
            .iter()
            .find(|source| source.available_at > cutoff)
        {
            return Err(LiteratureError::PostCutoffSourceIncluded {
                claim: self.claim_id.clone(),
                source_id: source.source_id.clone(),
                available: source.available_at,
                cutoff,
            });
        }
        Ok(())
    }
}

/// What a claim's evidence actually amounts to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportSummary {
    pub claim_id: String,
    /// Distinct study families reporting support. The honest count.
    pub supporting_families: BTreeSet<String>,
    /// Distinct study families reporting contradiction. Never collapsed into the support count.
    pub contradicting_families: BTreeSet<String>,
    /// Sources that only cite, kept visible so a reader can see what was excluded from support.
    pub citation_only: BTreeSet<String>,
    /// Sources whose relation nobody classified.
    pub unassessed: BTreeSet<String>,
    /// How deeply each supporting source was read.
    pub depth_breakdown: BTreeMap<String, usize>,
    /// True when every supporting source was read at full text.
    pub methods_level: bool,
}

impl SupportSummary {
    /// Independent supporting cohorts, which is what "how many studies" ought to mean.
    pub fn independent_families(&self) -> usize {
        self.supporting_families.len()
    }

    /// Whether contradiction survived into the summary. 39.01 forbids compressing conflicting
    /// evidence away, so a summary reporting support with contradiction hidden would be the exact
    /// defect.
    pub fn has_contradiction(&self) -> bool {
        !self.contradicting_families.is_empty()
    }
}

/// A claim and the evidence clustered around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCluster {
    pub claim: LiteratureClaim,
}

impl ClaimCluster {
    pub fn new(claim: LiteratureClaim) -> Self {
        ClaimCluster { claim }
    }

    /// Summarise the evidence as of a visibility cutoff.
    ///
    /// Refuses a claim whose only edges are citations. A claim with zero supporting *and* zero
    /// contradicting sources but a pile of citations is not weakly supported, it is unsupported,
    /// and the failure is typed so a compiler cannot render it as "widely cited" and move on.
    pub fn summarise(&self, cutoff: u64) -> Result<SupportSummary, LiteratureError> {
        let visible = self.claim.visible_at(cutoff);
        let has_evidence = visible
            .iter()
            .any(|source| source.relation.is_support() || source.relation.is_contradiction());
        let has_citations = visible
            .iter()
            .any(|source| source.relation == CitationRelation::Cites);
        if !has_evidence && has_citations {
            return Err(LiteratureError::CitationCountedAsSupport {
                claim: self.claim.claim_id.clone(),
            });
        }

        let mut supporting_families = BTreeSet::new();
        let mut contradicting_families = BTreeSet::new();
        let mut citation_only = BTreeSet::new();
        let mut unassessed = BTreeSet::new();
        let mut depth_breakdown: BTreeMap<String, usize> = BTreeMap::new();
        let mut methods_level = true;

        for source in visible {
            match source.relation {
                CitationRelation::Supports => {
                    supporting_families.insert(source.study_family.clone());
                    *depth_breakdown
                        .entry(source.depth.as_str().to_string())
                        .or_insert(0) += 1;
                    if !source.depth.supports_methods_level_claim() {
                        methods_level = false;
                    }
                }
                CitationRelation::Contradicts => {
                    contradicting_families.insert(source.study_family.clone());
                }
                CitationRelation::Cites => {
                    citation_only.insert(source.source_id.clone());
                }
                CitationRelation::Unassessed => {
                    unassessed.insert(source.source_id.clone());
                }
                CitationRelation::MethodSource => {}
            }
        }

        if supporting_families.is_empty() {
            methods_level = false;
        }

        Ok(SupportSummary {
            claim_id: self.claim.claim_id.clone(),
            supporting_families,
            contradicting_families,
            citation_only,
            unassessed,
            depth_breakdown,
            methods_level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualifiers() -> ClaimQualifiers {
        ClaimQualifiers::new("adult IDH-wildtype glioblastoma", "MGMT methylation PCR")
            .excluding("prior temozolomide exposure")
    }

    fn source(
        id: &str,
        relation: CitationRelation,
        depth: EvidenceDepth,
        family: &str,
        at: u64,
    ) -> SourceRecord {
        SourceRecord::new(id, relation, depth, family, at, format!("{id}#table-2"))
    }

    fn claim(sources: Vec<SourceRecord>) -> LiteratureClaim {
        LiteratureClaim::declare("claim/mgmt", "methylation predicts benefit", qualifiers(), sources)
            .expect("declares")
    }

    #[test]
    fn a_claim_supported_only_by_citations_is_refused_rather_than_rendered_as_widely_cited() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Cites, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Cites, EvidenceDepth::FullText, "fam/b", 2),
            source("p3", CitationRelation::Cites, EvidenceDepth::AbstractOnly, "fam/c", 3),
        ]));
        assert!(matches!(
            cluster.summarise(10),
            Err(LiteratureError::CitationCountedAsSupport { .. })
        ));
    }

    #[test]
    fn citations_are_kept_visible_beside_real_support_rather_than_counted_into_it() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Cites, EvidenceDepth::FullText, "fam/b", 2),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert_eq!(summary.independent_families(), 1);
        assert!(summary.citation_only.contains("p2"));
    }

    #[test]
    fn four_papers_on_one_cohort_count_as_one_independent_family() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "cohort/tcga", 1),
            source("p2", CitationRelation::Supports, EvidenceDepth::FullText, "cohort/tcga", 2),
            source("p3", CitationRelation::Supports, EvidenceDepth::FullText, "cohort/tcga", 3),
            source("p4", CitationRelation::Supports, EvidenceDepth::FullText, "cohort/tcga", 4),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert_eq!(summary.independent_families(), 1);
        assert_eq!(summary.depth_breakdown.get("full_text"), Some(&4));
    }

    #[test]
    fn abstract_only_support_is_labelled_and_blocks_a_methods_level_claim() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Supports, EvidenceDepth::AbstractOnly, "fam/b", 2),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert_eq!(summary.depth_breakdown.get("abstract_only"), Some(&1));
        assert!(!summary.methods_level);
    }

    #[test]
    fn support_read_entirely_at_full_text_carries_a_methods_level_label() {
        let cluster = ClaimCluster::new(claim(vec![source(
            "p1",
            CitationRelation::Supports,
            EvidenceDepth::FullText,
            "fam/a",
            1,
        )]));
        assert!(cluster.summarise(10).expect("summarises").methods_level);
    }

    #[test]
    fn contradiction_survives_into_the_summary_rather_than_being_averaged_into_support() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Contradicts, EvidenceDepth::FullText, "fam/b", 2),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert!(summary.has_contradiction());
        assert_eq!(summary.independent_families(), 1);
        assert!(summary.contradicting_families.contains("fam/b"));
    }

    #[test]
    fn an_unassessed_relation_never_counts_as_support() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Unassessed, EvidenceDepth::FullText, "fam/b", 2),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert_eq!(summary.independent_families(), 1);
        assert!(summary.unassessed.contains("p2"));
    }

    #[test]
    fn evidence_that_became_available_after_the_cutoff_is_not_visible() {
        let cluster = ClaimCluster::new(claim(vec![
            source("p1", CitationRelation::Supports, EvidenceDepth::FullText, "fam/a", 1),
            source("p2", CitationRelation::Supports, EvidenceDepth::FullText, "fam/b", 99),
        ]));
        let summary = cluster.summarise(10).expect("summarises");
        assert_eq!(summary.independent_families(), 1);
        assert!(!summary.supporting_families.contains("fam/b"));
    }

    #[test]
    fn a_claim_carrying_a_post_cutoff_source_is_reported_when_it_is_validated() {
        let claim = claim(vec![source(
            "p2",
            CitationRelation::Supports,
            EvidenceDepth::FullText,
            "fam/b",
            99,
        )]);
        assert!(matches!(
            claim.check_cutoff(10),
            Err(LiteratureError::PostCutoffSourceIncluded {
                available: 99,
                cutoff: 10,
                ..
            })
        ));
        assert!(claim.check_cutoff(100).is_ok());
    }

    #[test]
    fn a_claim_with_no_population_qualifier_cannot_be_declared() {
        assert!(matches!(
            LiteratureClaim::declare(
                "claim/x",
                "s",
                ClaimQualifiers::new("  ", "assay"),
                vec![]
            ),
            Err(LiteratureError::PopulationMissing { .. })
        ));
    }

    #[test]
    fn a_claim_with_no_assay_qualifier_cannot_be_declared() {
        assert!(matches!(
            LiteratureClaim::declare(
                "claim/x",
                "s",
                ClaimQualifiers::new("population", ""),
                vec![]
            ),
            Err(LiteratureError::AssayMissing { .. })
        ));
    }

    #[test]
    fn a_source_with_no_locator_cannot_be_declared_because_the_claim_could_not_be_checked() {
        let unlocatable = SourceRecord::new(
            "p1",
            CitationRelation::Supports,
            EvidenceDepth::FullText,
            "fam/a",
            1,
            "",
        );
        assert!(matches!(
            LiteratureClaim::declare("claim/x", "s", qualifiers(), vec![unlocatable]),
            Err(LiteratureError::LocatorMissing { .. })
        ));
    }

    #[test]
    fn cohort_exclusions_stay_attached_to_the_claim() {
        let claim = claim(vec![source(
            "p1",
            CitationRelation::Supports,
            EvidenceDepth::FullText,
            "fam/a",
            1,
        )]);
        assert_eq!(
            claim.qualifiers.exclusions,
            vec!["prior temozolomide exposure".to_string()]
        );
    }

    #[test]
    fn a_support_summary_survives_a_json_round_trip() {
        let cluster = ClaimCluster::new(claim(vec![source(
            "p1",
            CitationRelation::Supports,
            EvidenceDepth::FullText,
            "fam/a",
            1,
        )]));
        let summary = cluster.summarise(10).expect("summarises");
        let text = serde_json::to_string(&summary).expect("serialises");
        let back: SupportSummary = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, summary);
    }
}
