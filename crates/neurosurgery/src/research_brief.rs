//! Deterministic, source-linked research briefs for the provider-free neurosurgical agent.
//!
//! The existing packet and context surfaces preserve public records for a caller-owned model.
//! This module adds a useful local synthesis pass without pretending to be a model: it groups
//! already validated records into specialty-specific topic lanes, reports the exact terms that
//! matched each record, and keeps truncation, missing abstracts, and unverified source text
//! explicit. It never labels a patient, estimates an outcome, recommends care, or treats a
//! population record as case evidence.
//!
//! The topic vocabulary is an intentionally small, review-facing protocol. It is not a clinical
//! ontology and it is not a claim that a keyword establishes a diagnosis or a treatment effect.

use crate::{
    CaseRequest, NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery,
    PublicLiteratureQueryHit, RealDataFreshnessQuery, RealDataQuery, RealDataQueryHit,
    RealGliomaBundle, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const NEUROSURGICAL_RESEARCH_BRIEF_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-research-brief/0.1";
pub const MAX_RESEARCH_BRIEF_TOPICS: usize = 24;
pub const MAX_RESEARCH_BRIEF_RECORDS_PER_TOPIC: usize = 32;
pub const MAX_RESEARCH_BRIEF_FOCUS_TERMS: usize = 32;
const DEFAULT_RESEARCH_BRIEF_TOPICS: usize = 12;
const DEFAULT_RESEARCH_BRIEF_RECORDS_PER_TOPIC: usize = 8;
const MAX_FOCUS_TERM_BYTES: usize = 96;

fn default_topic_limit() -> usize {
    DEFAULT_RESEARCH_BRIEF_TOPICS
}

fn default_record_limit() -> usize {
    DEFAULT_RESEARCH_BRIEF_RECORDS_PER_TOPIC
}

/// Which validated public snapshot supplied a brief. Exactly one source is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchBriefSource {
    RealGlioma,
    PublicLiterature,
}

/// Bounded controls for deterministic topic extraction. The two source queries are mutually
/// exclusive and let a caller narrow the already local snapshot without triggering retrieval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalResearchBriefQuery {
    #[serde(default)]
    pub real_data_query: Option<RealDataQuery>,
    #[serde(default)]
    pub public_literature_query: Option<PublicLiteratureQuery>,
    /// Additional caller-provided lexical terms. They are reported verbatim and never inferred
    /// from a patient value or from a model.
    #[serde(default)]
    pub focus_terms: Vec<String>,
    #[serde(default = "default_topic_limit")]
    pub max_topics: usize,
    #[serde(default = "default_record_limit")]
    pub max_records_per_topic: usize,
    /// Include bounded abstract excerpts in each matched record. Excerpts remain untrusted
    /// source text and are never rewritten by this module.
    #[serde(default)]
    pub include_abstracts: bool,
    /// Optional caller-owned retrieval-age policy. It is evaluated against the same bundle and
    /// included in the digest-bound report; no host clock is consulted.
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
}

impl Default for NeurosurgicalResearchBriefQuery {
    fn default() -> Self {
        Self {
            real_data_query: None,
            public_literature_query: None,
            focus_terms: Vec::new(),
            max_topics: default_topic_limit(),
            max_records_per_topic: default_record_limit(),
            include_abstracts: false,
            freshness: None,
        }
    }
}

/// A source record included in a topic lane. `matched_terms` explains the lexical inclusion;
/// it is not a relevance score or an assertion that the record supports a medical conclusion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchBriefRecord {
    pub source: ResearchBriefSource,
    pub specialty: Specialty,
    pub record_kind: String,
    pub record_id: String,
    pub title: String,
    pub source_id: String,
    pub source_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>,
    pub matched_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_excerpt: Option<String>,
}

/// A review-facing topic lane. Counts describe the bounded local scan, not evidence strength.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchBriefTopic {
    pub topic_id: String,
    pub label: String,
    pub terms: Vec<String>,
    pub matched_record_count: usize,
    pub returned_record_count: usize,
    pub truncated: bool,
    pub source_ids: Vec<String>,
    pub publication_type_counts: Vec<ResearchBriefCount>,
    pub abstract_count: usize,
    pub records: Vec<ResearchBriefRecord>,
}

/// Deterministic categorical count used for publication tags and source kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchBriefCount {
    pub label: String,
    pub count: usize,
}

/// Why a reviewer should not treat a topic lane as complete or clinically interpretable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchBriefUnknown {
    pub code: String,
    pub scope: String,
    pub detail: String,
}

/// Digest-bound output that a local model, research worker, or human reviewer can consume
/// without an API key. The brief is a structured extraction, not a generated medical answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalResearchBriefReport {
    pub schema_version: String,
    pub brief_digest: String,
    pub request_digest: String,
    pub source: ResearchBriefSource,
    pub specialty: Specialty,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: NeurosurgicalResearchBriefQuery,
    pub topics: Vec<ResearchBriefTopic>,
    pub topic_count: usize,
    pub non_empty_topic_count: usize,
    pub total_match_count: usize,
    pub total_returned_count: usize,
    pub cross_topic_record_count: usize,
    pub source_query_truncated: bool,
    pub unknowns: Vec<ResearchBriefUnknown>,
    pub review_prompts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<crate::RealDataFreshnessReport>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl NeurosurgicalResearchBriefReport {
    /// Validate a persisted brief without fetching or re-reading any source bytes.
    ///
    /// This is a structural contract: it checks the digest, source/query posture, topic counts,
    /// bounded records, and explicit unknowns. It does not decide whether a lexical match is
    /// clinically relevant or whether a cited study applies to a patient.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != NEUROSURGICAL_RESEARCH_BRIEF_SCHEMA_VERSION
            || !is_sha256_hex(&self.brief_digest)
            || !is_sha256_hex(&self.request_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || self.generated_at.trim().is_empty()
            || self.topic_count != self.topics.len()
            || self.topic_count == 0
            || self.topic_count > self.query.max_topics
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.review_prompts.is_empty()
            || self.limitations.is_empty()
        {
            return Err(brief_rejected("research brief envelope is invalid"));
        }
        validate_query(&self.query)?;
        match self.source {
            ResearchBriefSource::RealGlioma => {
                if self.specialty != Specialty::Glioma
                    || self.query.public_literature_query.is_some()
                {
                    return Err(brief_rejected(
                        "real-glioma brief source or query scope is invalid",
                    ));
                }
            }
            ResearchBriefSource::PublicLiterature => {
                if self.query.real_data_query.is_some()
                    || self
                        .query
                        .public_literature_query
                        .as_ref()
                        .is_some_and(|query| query.specialty != Some(self.specialty))
                {
                    return Err(brief_rejected(
                        "public-literature brief source or query scope is invalid",
                    ));
                }
            }
        }
        let mut topic_ids = BTreeSet::new();
        let mut total_matches = 0usize;
        let mut total_returned = 0usize;
        let mut record_occurrences = BTreeMap::<String, usize>::new();
        for topic in &self.topics {
            if topic.topic_id.trim().is_empty()
                || !topic_ids.insert(topic.topic_id.clone())
                || topic.label.trim().is_empty()
                || topic.terms.is_empty()
                || topic
                    .terms
                    .iter()
                    .any(|term| term.trim().is_empty() || term.chars().any(char::is_control))
                || topic.matched_record_count < topic.returned_record_count
                || topic.truncated
                    != (topic.matched_record_count > self.query.max_records_per_topic)
                || topic.records.len() != topic.returned_record_count
                || topic.returned_record_count > self.query.max_records_per_topic
                || topic.abstract_count
                    != topic
                        .records
                        .iter()
                        .filter(|record| record.abstract_excerpt.is_some())
                        .count()
            {
                return Err(brief_rejected("research brief topic bounds are invalid"));
            }
            let expected_source_ids = topic
                .records
                .iter()
                .map(|record| record.source_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if topic.source_ids != expected_source_ids
                || topic.publication_type_counts
                    != count_labels(
                        topic
                            .records
                            .iter()
                            .flat_map(|record| record.publication_types.iter().cloned()),
                    )
            {
                return Err(brief_rejected(
                    "research brief topic summaries do not match their records",
                ));
            }
            let mut topic_record_ids = BTreeSet::new();
            for record in &topic.records {
                if record.source != self.source
                    || record.specialty != self.specialty
                    || record.record_kind.trim().is_empty()
                    || record.record_id.trim().is_empty()
                    || !topic_record_ids.insert(record.record_id.clone())
                    || record.title.trim().is_empty()
                    || record.source_id.trim().is_empty()
                    || !record.source_uri.starts_with("https://")
                    || record
                        .record_uri
                        .as_deref()
                        .is_some_and(|uri| !uri.starts_with("https://"))
                    || record.matched_terms.is_empty()
                    || record.matched_terms.iter().any(|term| {
                        !topic.terms.iter().any(|candidate| candidate == term)
                            || term.trim().is_empty()
                    })
                {
                    return Err(brief_rejected(
                        "research brief record provenance is invalid",
                    ));
                }
                *record_occurrences
                    .entry(record.record_id.clone())
                    .or_default() += 1;
            }
            total_matches += topic.matched_record_count;
            total_returned += topic.returned_record_count;
        }
        let expected_cross_topic_count = record_occurrences
            .values()
            .filter(|count| **count > 1)
            .count();
        if self.non_empty_topic_count
            != self
                .topics
                .iter()
                .filter(|topic| topic.matched_record_count > 0)
                .count()
            || self.total_match_count != total_matches
            || self.total_returned_count != total_returned
            || self.cross_topic_record_count != expected_cross_topic_count
            || self.cross_topic_record_count > self.total_returned_count
            || self.unknowns.iter().any(|unknown| {
                unknown.code.trim().is_empty()
                    || unknown.scope.trim().is_empty()
                    || unknown.detail.trim().is_empty()
            })
            || self
                .review_prompts
                .iter()
                .any(|prompt| prompt.trim().is_empty())
        {
            return Err(brief_rejected("research brief summary counts are invalid"));
        }
        if let Some(freshness) = self.freshness.as_ref() {
            if freshness.bundle_digest != self.bundle_digest
                || !is_sha256_hex(&freshness.freshness_digest)
                || freshness.provider != "none"
                || freshness.network
                || freshness.effect != "read_only"
                || !freshness.provenance_bound
                || freshness.synthetic_data
                || !freshness.human_review_required
            {
                return Err(brief_rejected(
                    "research brief freshness binding is invalid",
                ));
            }
        }
        if self.brief_digest != digest_report(self)? {
            return Err(brief_rejected(
                "research brief digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild a brief from the exact request, source snapshot, and persisted query.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        if self.request_digest != digest_json(request)? || self.specialty != request.specialty {
            return Err(brief_rejected("research brief request binding is invalid"));
        }
        let expected = match self.source {
            ResearchBriefSource::RealGlioma => real_data
                .ok_or_else(|| brief_rejected("research brief requires its real-glioma source"))?
                .research_brief(request, &self.query)?,
            ResearchBriefSource::PublicLiterature => public_literature
                .ok_or_else(|| {
                    brief_rejected("research brief requires its public-literature source")
                })?
                .research_brief(request, &self.query)?,
        };
        if &expected != self {
            return Err(brief_rejected(
                "research brief does not replay to the exact supplied inputs",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct BriefHit {
    source: ResearchBriefSource,
    specialty: Specialty,
    record_kind: String,
    record_id: String,
    title: String,
    source_id: String,
    source_uri: String,
    record_uri: Option<String>,
    publication_date: Option<String>,
    abstract_excerpt: Option<String>,
    publication_types: Vec<String>,
    mesh_terms: Vec<String>,
    searchable: String,
}

#[derive(Debug, Clone)]
struct TopicSpec {
    topic_id: String,
    label: String,
    terms: Vec<String>,
}

impl RealGliomaBundle {
    /// Build a deterministic brief from a validated, population-level glioma snapshot.
    pub fn research_brief(
        &self,
        request: &CaseRequest,
        query: &NeurosurgicalResearchBriefQuery,
    ) -> Result<NeurosurgicalResearchBriefReport, NeurosurgeryError> {
        validate_query(query)?;
        if query.public_literature_query.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a real glioma brief cannot carry a public-literature query".to_string(),
            });
        }
        let source_query = query
            .real_data_query
            .clone()
            .unwrap_or_else(|| RealDataQuery {
                limit: crate::real_data::MAX_QUERY_HITS_PUBLIC,
                ..RealDataQuery::default()
            });
        let result = self.query(&source_query)?;
        let hits = result.hits.iter().map(real_hit).collect::<Vec<_>>();
        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness| self.freshness_report(freshness))
            .transpose()?;
        build_report(
            request,
            query,
            ResearchBriefSource::RealGlioma,
            self.generated_at.clone(),
            result.bundle_digest,
            result.truncated,
            hits,
            freshness,
        )
    }
}

impl PublicLiteratureBundle {
    /// Build a deterministic brief from a validated, cross-specialty PubMed snapshot.
    pub fn research_brief(
        &self,
        request: &CaseRequest,
        query: &NeurosurgicalResearchBriefQuery,
    ) -> Result<NeurosurgicalResearchBriefReport, NeurosurgeryError> {
        validate_query(query)?;
        if query.real_data_query.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a public-literature brief cannot carry a real-data query".to_string(),
            });
        }
        let source_query =
            query
                .public_literature_query
                .clone()
                .unwrap_or_else(|| PublicLiteratureQuery {
                    specialty: Some(request.specialty),
                    limit: crate::public_literature::MAX_QUERY_HITS_PUBLIC,
                    ..PublicLiteratureQuery::default()
                });
        if source_query.specialty.is_none() {
            // A literature mission must remain scoped to the request lane. An explicit `None`
            // would otherwise turn a specialty brief into an all-lane corpus scan.
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a public-literature brief query must name the request specialty"
                    .to_string(),
            });
        }
        if source_query.specialty != Some(request.specialty) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature brief query specialty must match the request"
                    .to_string(),
            });
        }
        let result = self.query(&source_query)?;
        let hits = result.hits.iter().map(public_hit).collect::<Vec<_>>();
        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness| self.freshness_report(freshness))
            .transpose()?;
        build_report(
            request,
            query,
            ResearchBriefSource::PublicLiterature,
            self.generated_at.clone(),
            result.bundle_digest,
            result.truncated,
            hits,
            freshness,
        )
    }
}

fn real_hit(hit: &RealDataQueryHit) -> BriefHit {
    let mut searchable = format!("{} {}", hit.title, hit.record_id);
    if let Some(status) = &hit.status {
        searchable.push(' ');
        searchable.push_str(status);
    }
    if let Some(alteration_type) = &hit.molecular_alteration_type {
        searchable.push(' ');
        searchable.push_str(alteration_type);
    }
    if let Some(datatype) = &hit.datatype {
        searchable.push(' ');
        searchable.push_str(datatype);
    }
    searchable.push(' ');
    searchable.push_str(hit.abstract_excerpt.as_deref().unwrap_or_default());
    searchable.push(' ');
    searchable.push_str(&hit.publication_types.join(" "));
    searchable.push(' ');
    searchable.push_str(&hit.mesh_terms.join(" "));
    BriefHit {
        source: ResearchBriefSource::RealGlioma,
        specialty: Specialty::Glioma,
        record_kind: hit.record_kind.slug().to_string(),
        record_id: hit.record_id.clone(),
        title: hit.title.clone(),
        source_id: hit.source_id.clone(),
        source_uri: hit.source_uri.clone(),
        record_uri: None,
        publication_date: None,
        abstract_excerpt: hit.abstract_excerpt.clone(),
        publication_types: hit.publication_types.clone(),
        mesh_terms: hit.mesh_terms.clone(),
        searchable: searchable.to_lowercase(),
    }
}

fn public_hit(hit: &PublicLiteratureQueryHit) -> BriefHit {
    let searchable = format!(
        "{} {} {} {} {}",
        hit.title,
        hit.journal,
        hit.abstract_excerpt.as_deref().unwrap_or_default(),
        hit.publication_types.join(" "),
        hit.mesh_terms.join(" ")
    );
    BriefHit {
        source: ResearchBriefSource::PublicLiterature,
        specialty: hit.specialty,
        record_kind: "literature_article".to_string(),
        record_id: hit.pmid.clone(),
        title: hit.title.clone(),
        source_id: hit.source_id.clone(),
        source_uri: hit.source_uri.clone(),
        record_uri: Some(hit.record_uri.clone()),
        publication_date: hit.publication_date.clone(),
        abstract_excerpt: hit.abstract_excerpt.clone(),
        publication_types: hit.publication_types.clone(),
        mesh_terms: hit.mesh_terms.clone(),
        searchable: searchable.to_lowercase(),
    }
}

// The report constructor keeps the independently digest-bound inputs explicit so callers can
// audit source/query/freshness provenance without hiding them in an opaque context object.
#[allow(clippy::too_many_arguments)]
fn build_report(
    request: &CaseRequest,
    query: &NeurosurgicalResearchBriefQuery,
    source: ResearchBriefSource,
    generated_at: String,
    bundle_digest: String,
    source_query_truncated: bool,
    hits: Vec<BriefHit>,
    freshness: Option<crate::RealDataFreshnessReport>,
) -> Result<NeurosurgicalResearchBriefReport, NeurosurgeryError> {
    validate_query(query)?;
    let request_digest = digest_json(request)?;
    let specs = topic_specs(request.specialty, &query.focus_terms);
    let topic_count = specs.len().min(query.max_topics);
    let mut topics = Vec::with_capacity(topic_count);
    let mut unknowns = Vec::new();
    if source_query_truncated {
        unknowns.push(ResearchBriefUnknown {
            code: "source_query_truncated".to_string(),
            scope: "source_query".to_string(),
            detail: "the caller's bounded source query returned fewer records than matched; topic counts are lower bounds for the local snapshot".to_string(),
        });
    }
    for spec in specs.into_iter().take(query.max_topics) {
        let mut matched = hits
            .iter()
            .filter_map(|hit| {
                let matched_terms = matching_terms(&hit.searchable, &spec.terms);
                if matched_terms.is_empty() {
                    None
                } else {
                    Some((hit, matched_terms))
                }
            })
            .collect::<Vec<_>>();
        matched.sort_by(|(left, _), (right, _)| {
            left.record_kind
                .cmp(&right.record_kind)
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        let matched_record_count = matched.len();
        let truncated = matched_record_count > query.max_records_per_topic;
        if matched_record_count == 0 {
            unknowns.push(ResearchBriefUnknown {
                code: "topic_no_local_match".to_string(),
                scope: spec.topic_id.clone(),
                detail: "no record in the bounded local source query matched the declared topic terms; this is not evidence that the topic is absent from the wider literature".to_string(),
            });
        }
        if truncated {
            unknowns.push(ResearchBriefUnknown {
                code: "topic_truncated".to_string(),
                scope: spec.topic_id.clone(),
                detail: format!(
                    "{} local records matched, but only {} are emitted by the caller bound",
                    matched_record_count, query.max_records_per_topic
                ),
            });
        }
        let records = matched
            .into_iter()
            .take(query.max_records_per_topic)
            .map(|(hit, matched_terms)| brief_record(hit, matched_terms, query.include_abstracts))
            .collect::<Vec<_>>();
        let source_ids = records
            .iter()
            .map(|record| record.source_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let publication_type_counts = count_labels(
            records
                .iter()
                .flat_map(|record| record.publication_types.iter().cloned()),
        );
        let abstract_count = records
            .iter()
            .filter(|record| record.abstract_excerpt.is_some())
            .count();
        if query.include_abstracts && !records.is_empty() && abstract_count < records.len() {
            unknowns.push(ResearchBriefUnknown {
                code: "abstract_unavailable".to_string(),
                scope: spec.topic_id.clone(),
                detail: format!(
                    "{} emitted record(s) have no abstract excerpt in the snapshot; metadata presence is not equivalent to a readable full text",
                    records.len() - abstract_count
                ),
            });
        }
        topics.push(ResearchBriefTopic {
            topic_id: spec.topic_id,
            label: spec.label,
            terms: spec.terms,
            matched_record_count,
            returned_record_count: records.len(),
            truncated,
            source_ids,
            publication_type_counts,
            abstract_count,
            records,
        });
    }
    let non_empty_topic_count = topics
        .iter()
        .filter(|topic| topic.matched_record_count > 0)
        .count();
    let total_match_count = topics.iter().map(|topic| topic.matched_record_count).sum();
    let total_returned_count = topics.iter().map(|topic| topic.returned_record_count).sum();
    let cross_topic_record_count = topics
        .iter()
        .flat_map(|topic| topic.records.iter().map(|record| record.record_id.clone()))
        .fold(BTreeMap::<String, usize>::new(), |mut counts, id| {
            *counts.entry(id).or_default() += 1;
            counts
        })
        .values()
        .filter(|count| **count > 1)
        .count();
    let mut review_prompts = request.specialty.profile().evidence_questions;
    review_prompts.push(
        "Verify every lexical match against the cited source, cohort scope, publication design, and applicability before synthesis".to_string(),
    );
    review_prompts.push(
        "Treat empty or truncated lanes as unresolved evidence obligations, not negative findings"
            .to_string(),
    );
    let mut report = NeurosurgicalResearchBriefReport {
        schema_version: NEUROSURGICAL_RESEARCH_BRIEF_SCHEMA_VERSION.to_string(),
        brief_digest: String::new(),
        request_digest,
        source,
        specialty: request.specialty,
        bundle_digest,
        generated_at,
        query: query.clone(),
        topic_count: topics.len(),
        non_empty_topic_count,
        total_match_count,
        total_returned_count,
        cross_topic_record_count,
        source_query_truncated,
        unknowns,
        review_prompts,
        freshness,
        topics,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "topic membership is lexical extraction from title, abstract excerpt, and public indexing metadata; it is not relevance ranking, fact checking, diagnosis, prognosis, treatment advice, or causal inference".to_string(),
            "population and citation records remain separate from caller-supplied case observations; no record is promoted to patient evidence".to_string(),
            "empty, truncated, and abstract-missing lanes are explicit unknowns; the brief never imputes a negative finding".to_string(),
            "the brief never fetches URLs, invokes a model, opens credentials, accesses patient files, or writes durable state".to_string(),
        ],
    };
    report.brief_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(query: &NeurosurgicalResearchBriefQuery) -> Result<(), NeurosurgeryError> {
    if query.real_data_query.is_some() && query.public_literature_query.is_some() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "a research brief accepts one source query, not both".to_string(),
        });
    }
    if !(1..=MAX_RESEARCH_BRIEF_TOPICS).contains(&query.max_topics) {
        return Err(NeurosurgeryError::TooMany {
            field: "research_brief.max_topics",
            found: query.max_topics,
            max: MAX_RESEARCH_BRIEF_TOPICS,
        });
    }
    if !(1..=MAX_RESEARCH_BRIEF_RECORDS_PER_TOPIC).contains(&query.max_records_per_topic) {
        return Err(NeurosurgeryError::TooMany {
            field: "research_brief.max_records_per_topic",
            found: query.max_records_per_topic,
            max: MAX_RESEARCH_BRIEF_RECORDS_PER_TOPIC,
        });
    }
    if query.focus_terms.len() > MAX_RESEARCH_BRIEF_FOCUS_TERMS {
        return Err(NeurosurgeryError::TooMany {
            field: "research_brief.focus_terms",
            found: query.focus_terms.len(),
            max: MAX_RESEARCH_BRIEF_FOCUS_TERMS,
        });
    }
    for term in &query.focus_terms {
        if term.trim().is_empty()
            || term.len() > MAX_FOCUS_TERM_BYTES
            || term.chars().any(char::is_control)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "research brief focus_terms must be non-empty, bounded, and control-free"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn topic_specs(specialty: Specialty, focus_terms: &[String]) -> Vec<TopicSpec> {
    let mut specs = match specialty {
        Specialty::Glioma => vec![
            spec(
                "integrated_molecular_identity",
                "integrated molecular identity",
                &["idh", "1p/19q", "h3", "mgmt", "tert", "egfr", "methylation"],
            ),
            spec(
                "genomic_assays",
                "genomic assay and platform context",
                &[
                    "sequencing",
                    "rna",
                    "expression",
                    "copy number",
                    "mutation",
                    "molecular profile",
                ],
            ),
            spec(
                "radiographic_context",
                "radiographic and imaging context",
                &[
                    "mri",
                    "magnetic resonance",
                    "imaging",
                    "radiographic",
                    "perfusion",
                    "diffusion",
                ],
            ),
            spec(
                "histopathology",
                "histopathology and specimen context",
                &["histolog", "patholog", "biopsy", "specimen", "tissue"],
            ),
            spec(
                "clinical_trials",
                "clinical-trial and registry context",
                &[
                    "clinical trial",
                    "trial",
                    "randomized",
                    "registry",
                    "recruiting",
                ],
            ),
            spec(
                "population_outcomes",
                "population outcome and follow-up context",
                &["survival", "outcome", "follow-up", "cohort", "progression"],
            ),
            spec(
                "tumor_microenvironment",
                "tumour microenvironment and immune context",
                &[
                    "immune",
                    "microenvironment",
                    "immun",
                    "macrophage",
                    "t cell",
                ],
            ),
            spec(
                "treatment_effect_context",
                "treatment-effect and confounding context",
                &[
                    "pseudoprogression",
                    "radiation necrosis",
                    "treatment effect",
                    "response",
                    "recurrence",
                ],
            ),
        ],
        Specialty::CranialBase => vec![
            spec(
                "skull_base_anatomy",
                "skull-base anatomy and compartments",
                &["skull base", "clivus", "cavernous", "sella", "petrous"],
            ),
            spec(
                "cranial_nerves_vessels",
                "cranial-nerve and vascular relationships",
                &["cranial nerve", "vascular", "artery", "carotid", "venous"],
            ),
            spec(
                "endoscopic_corridors",
                "endoscopic and corridor context",
                &["endoscopic", "transnasal", "corridor", "approach"],
            ),
            spec(
                "reconstruction",
                "reconstruction and postoperative context",
                &[
                    "reconstruction",
                    "flap",
                    "cerebrospinal",
                    "csf",
                    "postoperative",
                ],
            ),
        ],
        Specialty::Craniosynostosis => vec![
            spec(
                "suture_patterns",
                "suture pattern and cranial-shape context",
                &[
                    "suture",
                    "craniosynostosis",
                    "scaphocephaly",
                    "brachycephaly",
                    "trigonocephaly",
                ],
            ),
            spec(
                "syndromic_genetics",
                "syndromic and genetic context",
                &["syndromic", "fgfr", "twist", "genetic", "mutation"],
            ),
            spec(
                "cranial_volume_orbit",
                "cranial-volume and orbital context",
                &[
                    "cranial volume",
                    "intracranial pressure",
                    "orbit",
                    "orbital",
                    "venous",
                ],
            ),
            spec(
                "developmental_function",
                "developmental and functional context",
                &[
                    "development",
                    "neurodevelopment",
                    "airway",
                    "vision",
                    "hearing",
                ],
            ),
        ],
        Specialty::Encephalocele => vec![
            spec(
                "defect_anatomy",
                "defect and tissue-content anatomy",
                &[
                    "encephalocele",
                    "defect",
                    "meningo",
                    "neural tissue",
                    "herniation",
                ],
            ),
            spec(
                "skull_base_csf",
                "skull-base and cerebrospinal-fluid context",
                &["skull base", "csf", "cerebrospinal", "leak", "cranial"],
            ),
            spec(
                "congenital_associations",
                "congenital and associated-anomaly context",
                &[
                    "congenital",
                    "anomaly",
                    "malformation",
                    "syndrome",
                    "prenatal",
                ],
            ),
            spec(
                "repair_outcomes",
                "repair and longitudinal outcome context",
                &[
                    "repair",
                    "reconstruction",
                    "outcome",
                    "follow-up",
                    "recurrence",
                ],
            ),
        ],
        Specialty::SpinaBifida => vec![
            spec(
                "dysraphism_phenotype",
                "spinal-dysraphism phenotype",
                &[
                    "spina bifida",
                    "dysraphism",
                    "myelomeningocele",
                    "meningocele",
                    "open neural tube",
                ],
            ),
            spec(
                "cord_tethering",
                "cord, conus, and tethering context",
                &["tethered", "tethering", "conus", "cord", "syrinx"],
            ),
            spec(
                "functional_domains",
                "neurologic and functional domains",
                &[
                    "motor", "sensory", "bladder", "bowel", "urologic", "function",
                ],
            ),
            spec(
                "developmental_longitudinal",
                "developmental and longitudinal context",
                &[
                    "prenatal",
                    "fetal",
                    "development",
                    "longitudinal",
                    "follow-up",
                ],
            ),
        ],
        Specialty::ChiariMalformation => vec![
            spec(
                "craniocervical_anatomy",
                "craniocervical-junction anatomy",
                &[
                    "chiari",
                    "craniocervical",
                    "foramen magnum",
                    "tonsil",
                    "brainstem",
                ],
            ),
            spec(
                "csf_flow_syrinx",
                "cerebrospinal-fluid flow and syrinx context",
                &["csf", "cerebrospinal", "flow", "syringomyelia", "syrinx"],
            ),
            spec(
                "associated_conditions",
                "associated-condition context",
                &[
                    "scoliosis",
                    "connective tissue",
                    "hydrocephalus",
                    "sleep",
                    "swallow",
                ],
            ),
            spec(
                "symptom_function_alignment",
                "symptom and functional alignment",
                &["symptom", "neurologic", "function", "outcome", "follow-up"],
            ),
        ],
    };
    if !focus_terms.is_empty() {
        let terms = focus_terms
            .iter()
            .map(|term| term.trim().to_lowercase())
            .collect::<Vec<_>>();
        specs.push(TopicSpec {
            topic_id: "caller_focus".to_string(),
            label: "caller-declared focus terms".to_string(),
            terms,
        });
    }
    specs
}

fn spec(id: &str, label: &str, terms: &[&str]) -> TopicSpec {
    TopicSpec {
        topic_id: id.to_string(),
        label: label.to_string(),
        terms: terms.iter().map(|term| (*term).to_string()).collect(),
    }
}

fn matching_terms(searchable: &str, terms: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    terms
        .iter()
        .filter(|term| searchable.contains(term.as_str()) && seen.insert((*term).clone()))
        .cloned()
        .collect()
}

fn brief_record(
    hit: &BriefHit,
    matched_terms: Vec<String>,
    include_abstract: bool,
) -> ResearchBriefRecord {
    ResearchBriefRecord {
        source: hit.source,
        specialty: hit.specialty,
        record_kind: hit.record_kind.clone(),
        record_id: hit.record_id.clone(),
        title: hit.title.clone(),
        source_id: hit.source_id.clone(),
        source_uri: hit.source_uri.clone(),
        record_uri: hit.record_uri.clone(),
        publication_date: hit.publication_date.clone(),
        matched_terms,
        publication_types: hit.publication_types.clone(),
        mesh_terms: hit.mesh_terms.clone(),
        abstract_excerpt: include_abstract
            .then(|| hit.abstract_excerpt.clone())
            .flatten(),
    }
}

fn count_labels(labels: impl Iterator<Item = String>) -> Vec<ResearchBriefCount> {
    let mut counts = BTreeMap::new();
    for label in labels {
        *counts.entry(label).or_insert(0usize) += 1;
    }
    counts
        .into_iter()
        .map(|(label, count)| ResearchBriefCount { label, count })
        .collect()
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_report(report: &NeurosurgicalResearchBriefReport) -> Result<String, NeurosurgeryError> {
    let mut copy = report.clone();
    copy.brief_digest.clear();
    digest_json(&copy)
}

fn brief_rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
