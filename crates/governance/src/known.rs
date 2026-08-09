//! The three wire formats this workspace already ships, as descriptors.
//!
//! These are not examples. They are the governance objects for
//! `fiber-context-certificate/0.1`, `fiber-context-certificate/0.2-extended` and
//! `fiber-decision-section/0.1`, and the tests below hold each of them against the code that
//! actually emits the bytes. A descriptor that drifts from its format is worse than no descriptor,
//! because it would answer compatibility questions about a format nobody publishes.
//!
//! # Why every certificate field is hashed
//!
//! `ContextCertificate::verify` clones the document, removes exactly one key —
//! `certificate_sha256` — and hashes the rest. Every other key is therefore inside the artifact's
//! identity, including keys the reader has never heard of. That single behaviour fixes two things
//! this crate would otherwise have to guess: the certificate's declared compatibility mode is
//! [`CompatibilityMode::PreserveAndForward`] (any other mode would move the digest of a document
//! that merely passed through an older reader), and `certificate_sha256` is the only field with
//! [`DigestRole::Excluded`].
//!
//! # A discrepancy worth naming
//!
//! The published `schemas/fiber-v0.1/context_certificate.schema.json` declares ten properties. The
//! Rust and CPython implementations emit twelve: `source_hashes` and `limitations` appear in the
//! body, are hashed, and are absent from the published schema. The schema sets
//! `additionalProperties: true`, so nothing is *invalid* — but a reader built from the published
//! schema alone would treat two hashed fields as unknown, and under any mode but
//! preserve-and-forward would then compute a different digest. The descriptor below follows the
//! emitting code, and [`tests::the_published_json_schema_is_a_subset_of_what_the_code_emits`]
//! records the gap rather than papering over it.

use crate::descriptor::{FieldSpec, FieldType, SchemaDescriptor};
use crate::mode::CompatibilityMode;
use crate::version::SchemaId;
use bioprism_section::{
    CERTIFICATE_SCHEMA_VERSION, CERTIFICATE_SCHEMA_VERSION_EXTENDED, SECTION_SCHEMA_VERSION,
};

/// The key a certificate's own digest lives under, and the only certificate field outside the
/// hashed bytes.
pub const CERTIFICATE_DIGEST_KEY: &str = "certificate_sha256";

fn parse(constant: &str) -> SchemaId {
    SchemaId::parse(constant).expect("a shipped schema version constant is well formed")
}

fn version_marker(constant: &str) -> FieldSpec {
    FieldSpec::required("schema_version", FieldType::Const(constant.to_string()))
}

/// `fiber-context-certificate/0.1` — the profile three implementations agree on.
pub fn certificate_reference() -> SchemaDescriptor {
    SchemaDescriptor::new(
        parse(CERTIFICATE_SCHEMA_VERSION),
        CompatibilityMode::PreserveAndForward,
        vec![
            version_marker(CERTIFICATE_SCHEMA_VERSION),
            FieldSpec::required("world_id", FieldType::String),
            FieldSpec::required("query_id", FieldType::String),
            FieldSpec::required("selected_facts", FieldType::Array),
            FieldSpec::required("selected_factors", FieldType::Array),
            FieldSpec::required("protected_closure", FieldType::Array),
            FieldSpec::required("omissions", FieldType::Object),
            FieldSpec::required("plan", FieldType::Object),
            FieldSpec::required("oracle", FieldType::Object),
            FieldSpec::required("source_hashes", FieldType::Object),
            FieldSpec::required("limitations", FieldType::Array),
            FieldSpec::required(CERTIFICATE_DIGEST_KEY, FieldType::String)
                .excluded_from_digest(),
        ],
    )
    .expect("the shipped certificate field set is well formed")
}

/// `fiber-context-certificate/0.2-extended` — the reference field set plus the influence-classified
/// omission manifest and the sufficiency verdict derived from it.
///
/// Both additions are required and hashed, which is why the profile could not be a flag on the 0.1
/// format: it moves the digest, so it needs its own version. The `section` crate reached the same
/// conclusion independently and for the same reason; this descriptor is that decision written down
/// where a tool can check it.
pub fn certificate_extended() -> SchemaDescriptor {
    let mut fields = certificate_reference().fields().to_vec();
    fields[0] = version_marker(CERTIFICATE_SCHEMA_VERSION_EXTENDED);
    let digest_field = fields.pop().expect("the digest field is last");
    fields.push(FieldSpec::required("omission_manifest", FieldType::Object));
    fields.push(FieldSpec::required(
        "supports_sufficiency_claim",
        FieldType::Boolean,
    ));
    fields.push(digest_field);

    SchemaDescriptor::new(
        parse(CERTIFICATE_SCHEMA_VERSION_EXTENDED),
        CompatibilityMode::PreserveAndForward,
        fields,
    )
    .expect("the extended certificate field set is well formed")
}

/// `fiber-decision-section/0.1` — what the model actually sees.
///
/// A section carries no self-digest: consumers hash the whole document, so every field is
/// [`DigestRole::Hashed`] and there is no excluded key at all.
pub fn decision_section() -> SchemaDescriptor {
    SchemaDescriptor::new(
        parse(SECTION_SCHEMA_VERSION),
        CompatibilityMode::PreserveAndForward,
        vec![
            version_marker(SECTION_SCHEMA_VERSION),
            FieldSpec::required("world_id", FieldType::String),
            FieldSpec::required("query_id", FieldType::String),
            FieldSpec::required("decision_time", FieldType::String),
            FieldSpec::required("goal", FieldType::String),
            FieldSpec::required("selected_evidence", FieldType::Array),
            FieldSpec::required("selected_factors", FieldType::Array),
            FieldSpec::required("oracle", FieldType::Object),
            FieldSpec::required("unresolved_obligations", FieldType::Array),
            FieldSpec::required("refinement_frontier", FieldType::Array),
        ],
    )
    .expect("the shipped section field set is well formed")
}

/// Every governed format, for a caller that wants to check them all.
pub fn all() -> Vec<SchemaDescriptor> {
    vec![
        certificate_reference(),
        certificate_extended(),
        decision_section(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::{classify, CompatibilityClass};
    use crate::descriptor::DigestRole;
    use crate::diff::diff;
    use crate::version::VersionBump;
    use bioprism_section::{
        Backend, CertificateProfile, ContextCertificate, DecisionSection, InfluenceClass,
        OmissionGroup, OmissionManifest, OracleStatus, OracleVerdict, PlanDescriptor,
        ReferenceOmissions, SourceHashes,
    };
    use serde_json::Value;

    fn certificate() -> ContextCertificate {
        ContextCertificate {
            world_id: "world-1".into(),
            query_id: "query-1".into(),
            selected_facts: vec!["f1".into()],
            selected_factors: vec!["k1".into()],
            protected_closure: vec!["f1".into()],
            omissions: ReferenceOmissions {
                total_facts: 3,
                exploratory_facts: 1,
                classification: "bounded".into(),
                inaccessible_selected_before_cut: Vec::new(),
            },
            plan: PlanDescriptor {
                backend: Backend::BackwardFactorSliceReference,
                compiled_factor_count: 1,
                compiled_fact_count: 1,
                total_factor_count: 2,
                total_fact_count: 3,
                max_selected_factor_arity: 2,
                fallback: None,
            },
            oracle: OracleVerdict {
                status: OracleStatus::Valid,
                witnesses: Vec::new(),
                oracle_kind: "split-integrity".into(),
            },
            source_hashes: SourceHashes {
                world_sha256: "0".repeat(64),
                query_sha256: "1".repeat(64),
                decision_section_sha256: "2".repeat(64),
            },
            limitations: vec!["research use only".into()],
            manifest: OmissionManifest {
                groups: vec![OmissionGroup {
                    reason: "outside the protected closure and the factor scope".into(),
                    influence: InfluenceClass::Zero,
                    count: 1,
                    bound: None,
                    examples: vec!["f2".into()],
                }],
            },
        }
    }

    fn emitted_keys(document: &Value) -> Vec<String> {
        document
            .as_object()
            .expect("a certificate is an object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn the_reference_descriptor_declares_exactly_the_keys_the_code_emits() {
        let document = certificate()
            .to_json(CertificateProfile::Reference)
            .expect("a certificate serialises");
        let mut declared = certificate_reference().paths().join(",");
        let mut emitted = emitted_keys(&document).join(",");
        declared = sorted(&declared);
        emitted = sorted(&emitted);
        assert_eq!(
            declared, emitted,
            "the descriptor must describe the bytes that ship, not the bytes we remember"
        );
    }

    #[test]
    fn the_extended_descriptor_declares_exactly_the_keys_the_extended_profile_emits() {
        let document = certificate()
            .to_json(CertificateProfile::Extended)
            .expect("a certificate serialises");
        assert_eq!(
            sorted(&certificate_extended().paths().join(",")),
            sorted(&emitted_keys(&document).join(","))
        );
    }

    #[test]
    fn the_section_descriptor_declares_exactly_the_keys_the_section_emits() {
        let section = DecisionSection {
            world_id: "world-1".into(),
            query_id: "query-1".into(),
            decision_time: "2026-08-08T00:00:00Z".into(),
            goal: "decide".into(),
            selected_evidence: Vec::new(),
            selected_factors: Vec::new(),
            oracle: OracleVerdict {
                status: OracleStatus::Valid,
                witnesses: Vec::new(),
                oracle_kind: "split-integrity".into(),
            },
            unresolved_obligations: Vec::new(),
            refinement_frontier: Vec::new(),
        };
        assert_eq!(
            sorted(&decision_section().paths().join(",")),
            sorted(&emitted_keys(&section.to_json()).join(","))
        );
    }

    #[test]
    fn every_shipped_certificate_verifies_against_its_own_descriptor() {
        for (profile, schema) in [
            (CertificateProfile::Reference, certificate_reference()),
            (CertificateProfile::Extended, certificate_extended()),
        ] {
            let document = certificate().to_json(profile).expect("serialises");
            let check = schema.check_document(&document);
            assert!(
                check.is_clean(),
                "{}: missing {:?}, mistyped {:?}, unknown {:?}",
                schema.id,
                check.missing,
                check.mistyped,
                check.unknown
            );
        }
    }

    #[test]
    fn the_certificate_digest_field_is_the_only_one_outside_the_hashed_bytes() {
        let schema = certificate_reference();
        let excluded: Vec<&str> = schema
            .fields()
            .iter()
            .filter(|field| field.digest == DigestRole::Excluded)
            .map(|field| field.path.as_str())
            .collect();
        assert_eq!(excluded, [CERTIFICATE_DIGEST_KEY]);
        assert_eq!(schema.hashed_paths().len(), schema.paths().len() - 1);
    }

    #[test]
    fn the_extended_profile_is_a_breaking_change_that_the_shipped_version_bump_covers() {
        let change = diff(&certificate_reference(), &certificate_extended()).expect("lineage");
        let classification = classify(&change);

        assert_eq!(classification.class, CompatibilityClass::Breaking);
        assert_eq!(
            classification.digest_affecting().len(),
            2,
            "both additions are required and hashed"
        );
        assert_eq!(classification.required_bump, VersionBump::Major);
        assert_eq!(
            classification.observed_bump,
            VersionBump::Major,
            "0.1 -> 0.2 under a zero major is the breaking bump"
        );
        classification
            .version_gate()
            .expect("the shipped version bump is large enough for what changed");
        assert!(classification
            .assert_class(CompatibilityClass::Compatible)
            .is_err());
    }

    #[test]
    fn the_version_marker_is_not_reported_as_a_field_change() {
        let change = diff(&certificate_reference(), &certificate_extended()).expect("lineage");
        assert!(
            change.change_at("schema_version").is_none(),
            "the version string differs at every release by definition; reporting it would bury \
             the changes that mean something"
        );
        assert_eq!(change.changes.len(), 2);
    }

    #[test]
    fn the_published_json_schema_is_a_subset_of_what_the_code_emits() {
        const PUBLISHED: &str =
            include_str!("../../../schemas/fiber-v0.1/context_certificate.schema.json");
        let published: Value = serde_json::from_str(PUBLISHED).expect("the published schema parses");
        let required: Vec<&str> = published["required"]
            .as_array()
            .expect("the published schema lists required fields")
            .iter()
            .map(|entry| entry.as_str().expect("a field name is a string"))
            .collect();

        let declared = certificate_reference();
        for field in &required {
            assert!(
                declared.field(field).is_some(),
                "{field} is required by the published schema and absent from the descriptor"
            );
        }

        let undocumented: Vec<&str> = declared
            .paths()
            .into_iter()
            .filter(|path| !required.contains(path))
            .collect();
        assert_eq!(
            undocumented,
            ["source_hashes", "limitations"],
            "these two are emitted and hashed but absent from the published v0.1 schema; a reader \
             built from that schema alone would treat two hashed fields as unknown"
        );
    }

    #[test]
    fn every_governed_format_declares_a_version_marker_inside_its_hashed_bytes() {
        for schema in all() {
            let marker = schema
                .version_marker()
                .unwrap_or_else(|| panic!("{} declares no version marker", schema.id));
            assert_eq!(marker.path, "schema_version");
            assert_eq!(marker.digest, DigestRole::Hashed);
        }
    }

    fn sorted(joined: &str) -> String {
        let mut parts: Vec<&str> = joined.split(',').collect();
        parts.sort_unstable();
        parts.join(",")
    }
}
