//! The Context Certificate: an immutable receipt for a compiled context.
//!
//! Blueprint 43.26, non-negotiable: *no "optimized context" is published without a certificate*.
//! The certificate binds every source by hash, states what was omitted and why, names the
//! backend and any fallback, and carries the replay block.
//!
//! Two profiles exist. [`CertificateProfile::Reference`] emits exactly the
//! `fiber-context-certificate/0.1` field set the CPython reference produces, so hashes match
//! across implementations. [`CertificateProfile::Extended`] adds the influence-classified
//! omission manifest that 43.26 actually requires but the v0.1 wire format has no room for;
//! because it changes the hashed bytes it carries a different schema version.

use crate::omission::OmissionManifest;
use crate::plan::PlanDescriptor;
use crate::verdict::OracleVerdict;
use bioprism_ids::{CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const CERTIFICATE_SCHEMA_VERSION: &str = "fiber-context-certificate/0.1";
pub const CERTIFICATE_SCHEMA_VERSION_EXTENDED: &str = "fiber-context-certificate/0.2-extended";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateProfile {
    /// Byte-compatible with the CPython reference runtime.
    Reference,
    /// Adds the influence-classified omission manifest and the sufficiency verdict.
    Extended,
}

/// The v0.1 omission summary.
///
/// This is a *count-and-a-string*, which is why [`CertificateProfile::Extended`] exists: a
/// classification string cannot distinguish "provably cannot matter" from "nobody checked", and
/// 43.26 requires that distinction before any sufficiency claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceOmissions {
    pub total_facts: usize,
    pub exploratory_facts: usize,
    /// A schema literal, not a computed verdict about this compile's omissions.
    ///
    /// Every producer of a `fiber-context-certificate/0.1` writes the same string here whatever
    /// the omitted population turns out to contain, so it cannot be read as a claim that each
    /// omitted fact met the condition it names — and on a world with a shadowed variable, some do
    /// not. The honest reading is that the v0.1 wire carries a count and a fixed label; the
    /// per-class verdict lives in [`crate::OmissionManifest`], which
    /// [`CertificateProfile::Extended`] emits and this profile has no room for.
    pub classification: String,
    pub inaccessible_selected_before_cut: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceHashes {
    pub world_sha256: String,
    pub query_sha256: String,
    pub decision_section_sha256: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextCertificate {
    pub world_id: String,
    pub query_id: String,
    pub selected_facts: Vec<String>,
    pub selected_factors: Vec<String>,
    pub protected_closure: Vec<String>,
    pub omissions: ReferenceOmissions,
    pub plan: PlanDescriptor,
    pub oracle: OracleVerdict,
    pub source_hashes: SourceHashes,
    pub limitations: Vec<String>,
    pub manifest: OmissionManifest,
}

impl ContextCertificate {
    /// The certificate body, before its own hash is attached.
    fn body(&self, profile: CertificateProfile) -> Value {
        let mut map = Map::new();
        let version = match profile {
            CertificateProfile::Reference => CERTIFICATE_SCHEMA_VERSION,
            CertificateProfile::Extended => CERTIFICATE_SCHEMA_VERSION_EXTENDED,
        };
        map.insert("schema_version".into(), json!(version));
        map.insert("world_id".into(), json!(self.world_id));
        map.insert("query_id".into(), json!(self.query_id));
        map.insert("selected_facts".into(), json!(self.selected_facts));
        map.insert("selected_factors".into(), json!(self.selected_factors));
        map.insert("protected_closure".into(), json!(self.protected_closure));
        map.insert(
            "omissions".into(),
            serde_json::to_value(&self.omissions).expect("omissions are serialisable"),
        );
        map.insert("plan".into(), plan_to_json(&self.plan));
        map.insert(
            "oracle".into(),
            serde_json::to_value(&self.oracle).expect("verdict is serialisable"),
        );
        map.insert(
            "source_hashes".into(),
            serde_json::to_value(&self.source_hashes).expect("hashes are serialisable"),
        );
        map.insert("limitations".into(), json!(self.limitations));

        if profile == CertificateProfile::Extended {
            map.insert(
                "omission_manifest".into(),
                serde_json::to_value(&self.manifest).expect("manifest is serialisable"),
            );
            map.insert(
                "supports_sufficiency_claim".into(),
                json!(self.manifest.supports_sufficiency_claim()),
            );
        }

        Value::Object(map)
    }

    /// The full certificate, with `certificate_sha256` computed over the body.
    pub fn to_json(&self, profile: CertificateProfile) -> Result<Value, CanonicalError> {
        let body = self.body(profile);
        let digest = ContentHash::of_value(&body)?;
        let mut map = body.as_object().expect("body is an object").clone();
        map.insert("certificate_sha256".into(), json!(digest.as_str()));
        Ok(Value::Object(map))
    }

    pub fn digest(&self, profile: CertificateProfile) -> Result<ContentHash, CanonicalError> {
        ContentHash::of_value(&self.body(profile))
    }

    /// Recomputes the embedded digest and checks it, the way a consumer must before trusting a
    /// certificate it did not produce.
    ///
    /// A `certificate_sha256` that is not a 64-character lowercase hex digest is
    /// [`CertificateVerification::Malformed`], never
    /// [`CertificateVerification::DigestMismatch`]. The two answers accuse different parties: a
    /// mismatch says the body moved after the digest was taken, and a shape defect says the
    /// claimed digest was never a digest. Reporting the second as the first would report tampering
    /// on the strength of a typo, and the recomputed value it printed alongside would be evidence
    /// of nothing.
    pub fn verify(document: &Value) -> Result<CertificateVerification, CanonicalError> {
        let Some(map) = document.as_object() else {
            return Ok(CertificateVerification::Malformed("not an object".into()));
        };
        let Some(claimed) = map.get("certificate_sha256").and_then(Value::as_str) else {
            return Ok(CertificateVerification::Malformed(
                "missing certificate_sha256".into(),
            ));
        };
        if ContentHash::parse(claimed.to_string()).is_err() {
            return Ok(CertificateVerification::Malformed(
                "certificate_sha256 is not a 64-character lowercase hex digest".into(),
            ));
        }
        let mut body = map.clone();
        body.remove("certificate_sha256");
        let recomputed = ContentHash::of_value(&Value::Object(body))?;
        if recomputed.as_str() == claimed {
            Ok(CertificateVerification::Valid)
        } else {
            Ok(CertificateVerification::DigestMismatch {
                claimed: claimed.to_string(),
                recomputed: recomputed.as_str().to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateVerification {
    Valid,
    DigestMismatch { claimed: String, recomputed: String },
    Malformed(String),
}

impl CertificateVerification {
    pub fn is_valid(&self) -> bool {
        matches!(self, CertificateVerification::Valid)
    }
}

fn plan_to_json(plan: &PlanDescriptor) -> Value {
    let mut map = Map::new();
    map.insert("backend".into(), json!(plan.backend.as_str()));
    map.insert("compiled_factor_count".into(), json!(plan.compiled_factor_count));
    map.insert("compiled_fact_count".into(), json!(plan.compiled_fact_count));
    map.insert("total_factor_count".into(), json!(plan.total_factor_count));
    map.insert("total_fact_count".into(), json!(plan.total_fact_count));
    map.insert(
        "max_selected_factor_arity".into(),
        json!(plan.max_selected_factor_arity),
    );
    map.insert(
        "fallback".into(),
        match &plan.fallback {
            None => Value::Null,
            Some(fallback) => serde_json::to_value(fallback).expect("fallback is serialisable"),
        },
    );
    Value::Object(map)
}
