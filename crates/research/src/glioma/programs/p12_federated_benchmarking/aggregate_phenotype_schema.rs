//! Federated aggregate phenotype harmonization for preclinical glioma comparisons.
//!
//! The schema keeps site-local mappings and uncertainty visible. Suppressed, missing, unmapped,
//! and incompatible values are never represented as zero, and comparable mappings are pooled only
//! when a benchmark policy explicitly permits it.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaAggregatePhenotypeSummary1@1";
pub const MAX_FIELDS: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhenotypeMappingKind {
    Exact,
    Comparable,
    NonComparable,
    Unmapped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPhenotypeField {
    pub field_id: String,
    pub local_concept: String,
    pub target_concept: String,
    pub value: String,
    pub unit: String,
    pub mapping: PhenotypeMappingKind,
    pub source_digest: ContentHash,
    pub suppressed: bool,
    pub source_count: u32,
    pub uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatePhenotypePolicy {
    pub schema_version: String,
    pub estimand: String,
    pub required_concept_order: Vec<String>,
    pub min_source_count: u32,
    pub max_uncertainty_milli: u16,
    pub allow_comparable_pool: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatePhenotypeRequest {
    pub site_id: String,
    pub local_dictionary_digest: ContentHash,
    pub fields: Vec<LocalPhenotypeField>,
    pub policy: AggregatePhenotypePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregatePhenotypeDisposition {
    Comparable,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatePhenotypeSummary {
    pub feature_id: String,
    pub output_schema: String,
    pub site_id: String,
    pub schema_version: String,
    pub estimand: String,
    pub comparable_concept_order: Vec<String>,
    pub non_comparable_concept_order: Vec<String>,
    pub suppressed_concept_order: Vec<String>,
    pub unmapped_concept_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub local_dictionary_digest: ContentHash,
    pub observed_source_count: u32,
    pub max_uncertainty_milli: u16,
    pub disposition: AggregatePhenotypeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AggregatePhenotypeError {
    #[error("aggregate phenotype request is invalid: {0}")]
    InvalidRequest(String),
    #[error("aggregate phenotype summary is invalid: {0}")]
    InvalidOutput(String),
    #[error("aggregate phenotype digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn protected(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "patient",
        "human",
        "diagnosis",
        "treatment",
        "triage",
        "enrollment",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn digest_input(output: &AggregatePhenotypeSummary) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "site_id": output.site_id,
        "schema_version": output.schema_version,
        "estimand": output.estimand,
        "comparable_concept_order": output.comparable_concept_order,
        "non_comparable_concept_order": output.non_comparable_concept_order,
        "suppressed_concept_order": output.suppressed_concept_order,
        "unmapped_concept_order": output.unmapped_concept_order,
        "omission_order": output.omission_order,
        "local_dictionary_digest": output.local_dictionary_digest,
        "observed_source_count": output.observed_source_count,
        "max_uncertainty_milli": output.max_uncertainty_milli,
        "disposition": output.disposition,
    })
}

impl AggregatePhenotypeSummary {
    pub fn validate(&self) -> Result<(), AggregatePhenotypeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.site_id)
            || self.schema_version.trim().is_empty()
            || self.estimand.trim().is_empty()
            || !canonical(&self.comparable_concept_order)
            || !canonical(&self.non_comparable_concept_order)
            || !canonical(&self.suppressed_concept_order)
            || !canonical(&self.unmapped_concept_order)
            || !canonical(&self.omission_order)
            || self.local_dictionary_digest.as_str().len() != 64
            || self.max_uncertainty_milli > 1_000
        {
            return Err(AggregatePhenotypeError::InvalidOutput(
                "summary identity, ordering, digest, or bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AggregatePhenotypeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AggregatePhenotypeError::InvalidOutput(
                "aggregate phenotype digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &AggregatePhenotypeRequest) -> Result<(), AggregatePhenotypeError> {
    if !identifier(&request.site_id)
        || request.local_dictionary_digest.as_str().len() != 64
        || request.fields.is_empty()
        || request.fields.len() > MAX_FIELDS
        || request.policy.schema_version.trim().is_empty()
        || request.policy.estimand.trim().is_empty()
        || request.policy.required_concept_order.is_empty()
        || !canonical(&request.policy.required_concept_order)
        || request
            .policy
            .required_concept_order
            .iter()
            .any(|concept| !identifier(concept) || protected(concept))
        || request.policy.max_uncertainty_milli > 1_000
    {
        return Err(AggregatePhenotypeError::InvalidRequest(
            "bounded site identity, dictionary, fields, estimand, concepts, and uncertainty policy are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for field in &request.fields {
        if !identifier(&field.field_id)
            || !ids.insert(field.field_id.clone())
            || !identifier(&field.local_concept)
            || !identifier(&field.target_concept)
            || protected(&field.local_concept)
            || protected(&field.target_concept)
            || field.value.contains('\0')
            || field.value.trim().is_empty()
            || field.value.len() > 512
            || !identifier(&field.unit)
            || field.source_digest.as_str().len() != 64
            || field.source_count == 0
            || field.uncertainty_milli > 1_000
        {
            return Err(AggregatePhenotypeError::InvalidRequest(
                "phenotype fields require unique non-protected identifiers, values, units, source evidence, and bounds".into(),
            ));
        }
    }
    Ok(())
}

/// Harmonize one site's phenotype fields into an aggregate-only schema while preserving gaps.
pub fn compile_glioma_aggregate_phenotype_summary(
    request: &AggregatePhenotypeRequest,
) -> Result<AggregatePhenotypeSummary, AggregatePhenotypeError> {
    validate_request(request)?;
    let required = request
        .policy
        .required_concept_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut grouped = BTreeMap::<String, Vec<&LocalPhenotypeField>>::new();
    for field in &request.fields {
        grouped
            .entry(field.target_concept.clone())
            .or_default()
            .push(field);
    }
    let mut comparable = BTreeSet::new();
    let mut non_comparable = BTreeSet::new();
    let mut suppressed = BTreeSet::new();
    let mut unmapped = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut source_count = 0_u32;
    let mut max_uncertainty = 0_u16;
    for concept in &required {
        let Some(fields) = grouped.get(concept) else {
            unmapped.insert(concept.clone());
            omissions.insert(format!("{concept}:missing"));
            continue;
        };
        if fields.iter().any(|field| field.suppressed) {
            suppressed.insert(concept.clone());
            omissions.insert(format!("{concept}:suppressed-not-zero"));
            continue;
        }
        if fields
            .iter()
            .any(|field| field.mapping == PhenotypeMappingKind::Unmapped)
        {
            unmapped.insert(concept.clone());
            omissions.insert(format!("{concept}:unmapped"));
            continue;
        }
        if fields
            .iter()
            .any(|field| field.mapping == PhenotypeMappingKind::NonComparable)
        {
            non_comparable.insert(concept.clone());
            omissions.insert(format!("{concept}:non-comparable"));
            continue;
        }
        if fields
            .iter()
            .any(|field| field.source_count < request.policy.min_source_count)
        {
            omissions.insert(format!("{concept}:source-count-below-policy"));
            non_comparable.insert(concept.clone());
            continue;
        }
        if fields
            .iter()
            .any(|field| field.uncertainty_milli > request.policy.max_uncertainty_milli)
        {
            omissions.insert(format!("{concept}:uncertainty-threshold"));
            non_comparable.insert(concept.clone());
            continue;
        }
        let has_comparable = fields
            .iter()
            .any(|field| field.mapping == PhenotypeMappingKind::Comparable);
        if has_comparable && !request.policy.allow_comparable_pool {
            non_comparable.insert(concept.clone());
            omissions.insert(format!("{concept}:comparable-pooling-disabled"));
            continue;
        }
        if fields
            .iter()
            .map(|field| field.unit.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            > 1
        {
            non_comparable.insert(concept.clone());
            omissions.insert(format!("{concept}:unit-conflict"));
            continue;
        }
        comparable.insert(concept.clone());
        for field in fields {
            source_count = source_count.saturating_add(field.source_count);
            max_uncertainty = max_uncertainty.max(field.uncertainty_milli);
        }
    }
    let disposition = if comparable.is_empty() {
        AggregatePhenotypeDisposition::Blocked
    } else if comparable.len() == required.len() {
        AggregatePhenotypeDisposition::Comparable
    } else {
        AggregatePhenotypeDisposition::Partial
    };
    let mut output = AggregatePhenotypeSummary {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        site_id: request.site_id.clone(),
        schema_version: request.policy.schema_version.clone(),
        estimand: request.policy.estimand.clone(),
        comparable_concept_order: comparable.into_iter().collect(),
        non_comparable_concept_order: non_comparable.into_iter().collect(),
        suppressed_concept_order: suppressed.into_iter().collect(),
        unmapped_concept_order: unmapped.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        local_dictionary_digest: request.local_dictionary_digest.clone(),
        observed_source_count: source_count,
        max_uncertainty_milli: max_uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-aggregate-phenotype"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AggregatePhenotypeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn field(id: &str, mapping: PhenotypeMappingKind) -> LocalPhenotypeField {
        LocalPhenotypeField {
            field_id: id.into(),
            local_concept: "invasion".into(),
            target_concept: "invasion".into(),
            value: "0.5".into(),
            unit: "score".into(),
            mapping,
            source_digest: hash(id),
            suppressed: false,
            source_count: 4,
            uncertainty_milli: 200,
        }
    }

    fn request() -> AggregatePhenotypeRequest {
        AggregatePhenotypeRequest {
            site_id: "site-a".into(),
            local_dictionary_digest: hash("dictionary"),
            fields: vec![field("invasion-field", PhenotypeMappingKind::Exact)],
            policy: AggregatePhenotypePolicy {
                schema_version: "phenotype-1".into(),
                estimand: "mean-invasion-score".into(),
                required_concept_order: vec!["invasion".into()],
                min_source_count: 2,
                max_uncertainty_milli: 500,
                allow_comparable_pool: false,
            },
        }
    }

    #[test]
    fn exact_field_is_comparable() {
        let output = compile_glioma_aggregate_phenotype_summary(&request()).unwrap();
        assert_eq!(
            output.disposition,
            AggregatePhenotypeDisposition::Comparable
        );
        assert_eq!(output.observed_source_count, 4);
        output.validate().unwrap();
    }

    #[test]
    fn suppressed_value_is_not_zero() {
        let mut request = request();
        request.fields[0].suppressed = true;
        let output = compile_glioma_aggregate_phenotype_summary(&request).unwrap();
        assert_eq!(output.disposition, AggregatePhenotypeDisposition::Blocked);
        assert!(output
            .omission_order
            .iter()
            .any(|item| item.contains("suppressed-not-zero")));
    }

    #[test]
    fn comparable_mapping_requires_explicit_policy() {
        let mut request = request();
        request.fields[0].mapping = PhenotypeMappingKind::Comparable;
        let output = compile_glioma_aggregate_phenotype_summary(&request).unwrap();
        assert!(output
            .omission_order
            .iter()
            .any(|item| item.contains("pooling-disabled")));
    }

    #[test]
    fn unmapped_concept_is_explicit() {
        let mut request = request();
        request.fields[0].mapping = PhenotypeMappingKind::Unmapped;
        let output = compile_glioma_aggregate_phenotype_summary(&request).unwrap();
        assert!(output.unmapped_concept_order.contains(&"invasion".into()));
    }

    #[test]
    fn mutation_breaks_digest() {
        let mut output = compile_glioma_aggregate_phenotype_summary(&request()).unwrap();
        output.estimand = "tampered".into();
        assert!(matches!(
            output.validate(),
            Err(AggregatePhenotypeError::InvalidOutput(_))
        ));
    }
}
