//! A deterministic artifact and schema oracle (31.02).
//!
//! 31.02's purpose is "machine-checkable invariants as the first line of evaluation before any
//! semantic judge", and its required functions begin with validating file structure, checksums,
//! identifiers, units, and required fields. This oracle does the structural half of that:
//! required fields, their JSON types, and an optional content-hash check over a declared payload.
//!
//! The checksum uses `bioprism_ids::ContentHash`, which hashes the *canonical* bytes of the value.
//! That matters: a hash over `serde_json`'s default rendering would depend on key order and float
//! formatting, so an artifact that round-tripped through a different language would appear
//! corrupted. Canonicalising first is what makes this check mean "the payload changed" rather than
//! "the serialiser changed".
//!
//! # Abstention
//!
//! If none of the required fields is present at all, the oracle returns
//! [`Position::NotEvaluable`] rather than a pile of `MissingField` contradictions. Being handed
//! the wrong artifact and being handed a broken one are different situations, and 31.01 provides
//! `not-evaluable` precisely so that the first does not have to be reported as the second.
//!
//! Not implemented: units, coordinate systems, and identifier grammars. 31.02 lists DICOM affine
//! consistency and VCF reference compatibility as typical applications; those need format-specific
//! readers that belong with the adapters, not here.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde_json::Value;

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::{Confidence, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::{OracleId, OracleManifest, OracleRef, OracleVersion};
use crate::oracle::Oracle;
use crate::plane::Plane;
use crate::time::ValidityWindow;

/// The JSON types a required field may be declared as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    String,
    Number,
    Bool,
    Array,
    Object,
}

impl FieldType {
    fn matches(self, value: &Value) -> bool {
        match self {
            FieldType::String => value.is_string(),
            FieldType::Number => value.is_number(),
            FieldType::Bool => value.is_boolean(),
            FieldType::Array => value.is_array(),
            FieldType::Object => value.is_object(),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::Bool => "bool",
            FieldType::Array => "array",
            FieldType::Object => "object",
        }
    }
}

fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Checks required fields, their types, and an optional declared content hash.
pub struct SchemaOracle {
    manifest: OracleManifest,
    required: BTreeMap<String, FieldType>,
    checksum: Option<Checksum>,
}

struct Checksum {
    hash_field: String,
    payload_field: String,
}

impl SchemaOracle {
    /// Builds a schema oracle at [`EvidenceTier::Deterministic`], establishing only
    /// [`Plane::Artifact`].
    pub fn new(
        id: impl Into<String>,
        version: OracleVersion,
        validity: ValidityWindow,
    ) -> Result<Self, OracleError> {
        let manifest = OracleManifest::new(
            OracleRef::new(OracleId::parse(id)?, version),
            EvidenceTier::Deterministic,
            [Plane::Artifact],
            [],
            validity,
        )?
        .disclaiming_the_rest()
        .with_failure_mode(
            "checks only the fields it was configured with; an artifact can satisfy every declared \
             field and still be missing something nobody thought to declare",
        );

        Ok(SchemaOracle {
            manifest,
            required: BTreeMap::new(),
            checksum: None,
        })
    }

    pub fn require(mut self, pointer: impl Into<String>, ty: FieldType) -> Self {
        self.required.insert(pointer.into(), ty);
        self
    }

    /// Declares that `hash_field` holds the content hash of `payload_field`.
    pub fn with_checksum(
        mut self,
        hash_field: impl Into<String>,
        payload_field: impl Into<String>,
    ) -> Self {
        self.checksum = Some(Checksum {
            hash_field: hash_field.into(),
            payload_field: payload_field.into(),
        });
        self
    }

    pub fn manifest_mut(&mut self) -> &mut OracleManifest {
        &mut self.manifest
    }

    fn checksum_findings(&self, evidence: &Evidence) -> Result<Vec<Finding>, OracleError> {
        let Some(checksum) = &self.checksum else {
            return Ok(Vec::new());
        };
        let (Some(declared), Some(payload)) = (
            evidence.field(&checksum.hash_field),
            evidence.field(&checksum.payload_field),
        ) else {
            return Ok(vec![Finding::NotApplicable {
                check: format!("checksum({})", checksum.payload_field),
                reason: "the hash field or the payload field is absent".to_string(),
            }]);
        };
        let Some(declared) = declared.as_str() else {
            return Ok(vec![Finding::TypeMismatch {
                pointer: checksum.hash_field.clone(),
                expected: "string".to_string(),
                actual: describe(declared).to_string(),
            }]);
        };

        let computed = ContentHash::of_value(payload)?;
        if computed.as_str() == declared {
            Ok(Vec::new())
        } else {
            Ok(vec![Finding::ChecksumMismatch {
                pointer: checksum.payload_field.clone(),
                declared: declared.to_string(),
                computed: computed.to_string(),
            }])
        }
    }
}

impl Oracle for SchemaOracle {
    fn manifest(&self) -> &OracleManifest {
        &self.manifest
    }

    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError> {
        let present = self
            .required
            .keys()
            .filter(|pointer| evidence.field(pointer).is_some())
            .count();

        if !self.required.is_empty() && present == 0 {
            return Ok(Judgement::from_manifest(
                &self.manifest,
                &evidence.at,
                Position::NotEvaluable,
                Confidence::CERTAIN,
            )
            .with_rationale(
                "none of the declared fields is present; this looks like a different artifact \
                 rather than a defective one",
            ));
        }

        let mut findings = Vec::new();
        for (pointer, ty) in &self.required {
            match evidence.field(pointer) {
                None => findings.push(Finding::MissingField {
                    pointer: pointer.clone(),
                }),
                Some(value) if !ty.matches(value) => findings.push(Finding::TypeMismatch {
                    pointer: pointer.clone(),
                    expected: ty.as_str().to_string(),
                    actual: describe(value).to_string(),
                }),
                Some(_) => {}
            }
        }
        findings.extend(self.checksum_findings(evidence)?);

        let violated = findings.iter().any(Finding::is_violation);
        let position = if violated {
            Position::Contradicted
        } else {
            Position::Supported
        };

        Ok(
            Judgement::from_manifest(&self.manifest, &evidence.at, position, Confidence::CERTAIN)
                .with_findings(findings)
                .with_rationale(format!(
                    "checked {} declared field(s) and {} checksum(s)",
                    self.required.len(),
                    usize::from(self.checksum.is_some())
                )),
        )
    }
}
