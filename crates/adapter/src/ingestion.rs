//! The only thing an adapter is allowed to return.
//!
//! Blueprint 40.17's outputs are an artifact inventory, a normalized graph fragment, a
//! loss/warning report and reader provenance — four things, delivered together or not at all.
//! [`Ingestion`] is that quadruple, with private fields and a single constructor, which is
//! what makes "facts without a loss report" unrepresentable rather than merely discouraged.
//! An adapter author who has not thought about loss must still write
//! `SemanticLoss::Unaudited { reason }`, and that string is then visible to every reviewer.
//!
//! The constructor also enforces invariant 4, "validation precedes publication": every fact
//! document is parsed with [`bioprism_world::Fact`] before the ingestion exists, so a
//! malformed fact is an adapter bug caught at the adapter boundary rather than a corrupt world
//! discovered at query time.

use crate::error::AdapterError;
use crate::location::SourceLocation;
use crate::loss::SemanticLoss;
use crate::source::SourceManifest;
use bioprism_ids::{to_canonical_bytes, ContentHash};
use bioprism_world::Fact;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// A completed ingest: what was produced, from what, and what was left behind.
#[derive(Debug, Clone, PartialEq)]
pub struct Ingestion {
    manifest: SourceManifest,
    facts: Vec<Value>,
    loss: SemanticLoss,
}

impl Ingestion {
    /// Validates and seals an ingest.
    ///
    /// Facts are sorted by id. Adapters walk sources in whatever order is natural for the
    /// format, and two orderings of the same facts are the same world; leaving the order to
    /// the adapter would make conformance's determinism check sensitive to iteration order
    /// rather than to semantics.
    pub fn new(
        manifest: SourceManifest,
        facts: Vec<Value>,
        loss: SemanticLoss,
    ) -> Result<Self, AdapterError> {
        manifest.validate()?;
        loss.validate_for_source(&manifest.source_id)?;
        let mut seen: BTreeMap<String, SourceLocation> = BTreeMap::new();
        let mut validated: Vec<(String, Value)> = Vec::with_capacity(facts.len());

        for document in facts {
            let location = fact_location(&manifest.source_id, &document);
            let fact = Fact::from_json(&document).map_err(|error| {
                AdapterError::malformed_fact(location.clone(), error.to_string())
            })?;
            let id = fact.id.to_string();
            if let Some(first) = seen.get(&id) {
                return Err(AdapterError::duplicate_fact_id(id, first.clone(), location));
            }
            seen.insert(id.clone(), location);
            validated.push((id, document));
        }

        validated.sort_by(|left, right| left.0.cmp(&right.0));

        Ok(Ingestion {
            manifest,
            facts: validated
                .into_iter()
                .map(|(_, document)| document)
                .collect(),
            loss,
        })
    }

    pub fn manifest(&self) -> &SourceManifest {
        &self.manifest
    }

    /// The emitted fact documents, in id order.
    pub fn facts(&self) -> &[Value] {
        &self.facts
    }

    /// The mandatory loss audit.
    pub fn loss(&self) -> &SemanticLoss {
        &self.loss
    }

    pub fn fact_count(&self) -> usize {
        self.facts.len()
    }

    /// Re-parses the facts into their typed form.
    ///
    /// Not cached, because the typed form is a view for callers that want one and the wire
    /// documents are what gets hashed and published. Every document here already parsed once
    /// in [`Ingestion::new`], so this cannot fail for a well-formed ingestion.
    pub fn typed_facts(&self) -> Result<Vec<Fact>, AdapterError> {
        self.facts
            .iter()
            .map(|document| {
                Fact::from_json(document).map_err(|error| {
                    AdapterError::malformed_fact(
                        fact_location(&self.manifest.source_id, document),
                        error.to_string(),
                    )
                })
            })
            .collect()
    }

    /// The canonical bytes of the whole ingestion.
    ///
    /// Determinism in 40.17 is a claim about the *output*, not just the facts: an adapter that
    /// emits identical facts with a fluctuating loss report is not deterministic, because two
    /// reviewers would reach different conclusions about the same world. The digest therefore
    /// covers manifest, facts and loss together.
    pub fn digest(&self) -> Result<ContentHash, AdapterError> {
        let bytes = self.canonical_bytes()?;
        Ok(ContentHash::of_bytes(&bytes))
    }

    /// Canonical serialization, used by the digest and by conformance's byte comparison.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, AdapterError> {
        let value = self.to_value()?;
        to_canonical_bytes(&value).map_err(|source| {
            AdapterError::canonical(SourceLocation::source(&self.manifest.source_id), source)
        })
    }

    /// The whole ingestion as one JSON document, suitable for writing next to the world it
    /// contributed to.
    pub fn to_value(&self) -> Result<Value, AdapterError> {
        let mut map = Map::new();
        map.insert(
            "manifest".to_string(),
            serde_json::to_value(&self.manifest).map_err(|error| {
                AdapterError::malformed_fact(
                    SourceLocation::source(&self.manifest.source_id),
                    error.to_string(),
                )
            })?,
        );
        map.insert("facts".to_string(), Value::Array(self.facts.clone()));
        map.insert(
            "semantic_loss".to_string(),
            serde_json::to_value(&self.loss).map_err(|error| {
                AdapterError::malformed_fact(
                    SourceLocation::source(&self.manifest.source_id),
                    error.to_string(),
                )
            })?,
        );
        Ok(Value::Object(map))
    }
}

/// Best available location for a fact document, used only in error messages.
fn fact_location(source_id: &str, document: &Value) -> SourceLocation {
    match document.get("id").and_then(Value::as_str) {
        Some(id) => SourceLocation::column(source_id, id),
        None => SourceLocation::source(source_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::FactDraft;
    use crate::loss::{LossAudit, SemanticLoss};
    use crate::source::SourceManifest;
    use bioprism_scope::ScopeKey;

    fn manifest() -> SourceManifest {
        SourceManifest {
            source_id: "demo".into(),
            declared_format: None,
            source_digest: ContentHash::of_bytes(b"demo"),
            byte_length: Some(4),
            adapter: "test".into(),
            adapter_version: "0.1.0".into(),
            profile_digest: None,
            provenance: None,
        }
    }

    fn fact(id: &str) -> Value {
        FactDraft::new(
            id,
            "age",
            Value::from(1),
            ScopeKey::new().exact("cohort", "C"),
            SourceLocation::source("demo"),
        )
        .build()
        .unwrap()
    }

    #[test]
    fn facts_are_sorted_by_id_so_walk_order_cannot_change_the_digest() {
        let audit = LossAudit::new().finish();
        let forward = Ingestion::new(
            manifest(),
            vec![fact("fact.a"), fact("fact.b")],
            audit.clone(),
        )
        .unwrap();
        let backward =
            Ingestion::new(manifest(), vec![fact("fact.b"), fact("fact.a")], audit).unwrap();
        assert_eq!(forward.digest().unwrap(), backward.digest().unwrap());
    }

    #[test]
    fn two_facts_with_the_same_id_are_refused_at_the_adapter_boundary() {
        let error = Ingestion::new(
            manifest(),
            vec![fact("fact.a"), fact("fact.a")],
            LossAudit::new().finish(),
        )
        .unwrap_err();
        assert!(matches!(error, AdapterError::DuplicateFactId { .. }));
    }

    #[test]
    fn a_fact_that_is_not_valid_under_the_world_schema_never_reaches_an_ingestion() {
        let malformed = serde_json::json!({ "id": "fact.x", "provides": "age" });
        let error = Ingestion::new(
            manifest(),
            vec![malformed],
            SemanticLoss::Unaudited {
                reason: "stub".into(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, AdapterError::MalformedFact { .. }));
    }

    #[test]
    fn the_digest_covers_the_loss_report_not_only_the_facts() {
        let facts = vec![fact("fact.a")];
        let lossless =
            Ingestion::new(manifest(), facts.clone(), LossAudit::new().finish()).unwrap();
        let unaudited = Ingestion::new(
            manifest(),
            facts,
            SemanticLoss::Unaudited {
                reason: "stub".into(),
            },
        )
        .unwrap();
        assert_eq!(lossless.facts(), unaudited.facts());
        assert_ne!(lossless.digest().unwrap(), unaudited.digest().unwrap());
    }

    #[test]
    fn an_invalid_manifest_is_refused_before_facts_are_published() {
        let mut manifest = manifest();
        manifest.adapter = "".into();
        let error = Ingestion::new(manifest, Vec::new(), LossAudit::new().finish()).unwrap_err();
        assert!(matches!(error, AdapterError::InvalidSource(_)));
    }

    #[test]
    fn loss_metadata_from_another_source_is_refused_before_publication() {
        let mut audit = LossAudit::new();
        audit.record(
            crate::loss::LossKind::UnmappedColumn,
            crate::loss::LossSeverity::Degrading,
            SourceLocation::column("other", "age"),
            "wrong source",
        );
        let error = Ingestion::new(manifest(), Vec::new(), audit.finish()).unwrap_err();
        assert!(matches!(error, AdapterError::InvalidLoss(_)));
    }
}
