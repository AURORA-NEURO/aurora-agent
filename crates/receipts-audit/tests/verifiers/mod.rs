//! Every workspace verifier, projected onto the battery's [`Verdict`].
//!
//! The projections are deliberately thin. Each one reads the verifier's own answer — the
//! `CertificateVerification` variant, the `valid` / `digest_malformed` / `digest_match` flags of a
//! verification projection, the `failures` list of a bundle check — and translates it, without
//! inspecting the document itself. An adapter that peeked at the document to decide a class would
//! be answering the question the verifier is under test for.
//!
//! Two adapters map a rejection *reason string* onto a class, because the verifier signals the
//! difference between an absent digest and a shape-broken one only in prose.
//! `reason_strings_are_the_ones_the_adapters_key_on` in the battery pins those strings, so a
//! rewording breaks loudly instead of silently collapsing two classes into one.

use bioprism_autopilot::{verify_autopilot_report, AutopilotError};
use bioprism_devplat::{
    verify_delivery_receipt, verify_mission_evidence_bundle, DeliveryReceiptVerificationRequest,
    EvidenceBundleError,
};
use bioprism_receipts_audit::{RejectionClass, Verdict};
use bioprism_research::{verify_dossier, ResearchError};
use bioprism_section::{CertificateVerification, ContextCertificate};
use serde_json::Value;

/// The prose the certificate verifier uses for a digest that is not there.
pub const CERTIFICATE_ABSENT_DIGEST: &str = "missing certificate_sha256";
/// The prose the certificate verifier uses for a digest of the wrong shape.
pub const CERTIFICATE_MALFORMED_DIGEST: &str =
    "certificate_sha256 is not a 64-character lowercase hex digest";
/// The prose the autopilot verifier uses for a digest that is not a string.
pub const AUTOPILOT_ABSENT_DIGEST: &str = "report_sha256 must be a string";
/// The prose the dossier verifier uses for a digest that is not a string.
pub const DOSSIER_ABSENT_DIGEST: &str = "dossier_sha256 must be a string";
/// The prose the bundle verifier uses for a digest that is absent or blank.
pub const BUNDLE_ABSENT_DIGEST: &str = "bundle_digest must be a non-empty string";
/// The prose the bundle verifier uses for a digest of the wrong shape.
pub const BUNDLE_MALFORMED_DIGEST: &str =
    "bundle_digest must be a lowercase 64-character SHA-256 digest";

fn projection_verdict(projection: &Value, absent_reason: &str) -> Verdict {
    if projection["valid"] == Value::Bool(true) {
        return Verdict::Accepted;
    }
    if projection["digest_malformed"] == Value::Bool(true) {
        return Verdict::rejected(RejectionClass::DigestMalformed, absent_reason.to_string());
    }
    if projection["digest_match"] == Value::Bool(false) {
        return Verdict::rejected(
            RejectionClass::DigestMismatch,
            projection["recomputed_report_sha256"]
                .as_str()
                .or_else(|| projection["recomputed_dossier_sha256"].as_str())
                .unwrap_or("recomputed digest differs")
                .to_string(),
        );
    }
    Verdict::rejected(
        RejectionClass::StructuralFailure,
        projection.to_string(),
    )
}

pub fn certificate(document: &Value) -> Verdict {
    match ContextCertificate::verify(document) {
        Ok(CertificateVerification::Valid) => Verdict::Accepted,
        Ok(CertificateVerification::DigestMismatch {
            claimed,
            recomputed,
        }) => Verdict::rejected(
            RejectionClass::DigestMismatch,
            format!("claims {claimed}, recomputes to {recomputed}"),
        ),
        Ok(CertificateVerification::Malformed(reason)) => {
            let class = if reason.contains(CERTIFICATE_ABSENT_DIGEST) {
                RejectionClass::DigestAbsent
            } else if reason.contains(CERTIFICATE_MALFORMED_DIGEST) {
                RejectionClass::DigestMalformed
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
        Err(error) => Verdict::rejected(RejectionClass::Malformed, error.to_string()),
    }
}

pub fn autopilot(document: &Value) -> Verdict {
    match verify_autopilot_report(document) {
        Ok(projection) => projection_verdict(&projection, AUTOPILOT_ABSENT_DIGEST),
        Err(AutopilotError::InvalidAutopilotReport { reason }) => {
            let class = if reason.contains(AUTOPILOT_ABSENT_DIGEST) {
                RejectionClass::DigestAbsent
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
        Err(error) => Verdict::rejected(RejectionClass::Malformed, error.to_string()),
    }
}

pub fn dossier(document: &Value) -> Verdict {
    match verify_dossier(document) {
        Ok(projection) => projection_verdict(&projection, DOSSIER_ABSENT_DIGEST),
        Err(ResearchError::InvalidDossier { reason }) => {
            let class = if reason.contains(DOSSIER_ABSENT_DIGEST) {
                RejectionClass::DigestAbsent
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
        Err(error) => Verdict::rejected(RejectionClass::Malformed, error.to_string()),
    }
}

pub fn evidence_bundle(document: &Value) -> Verdict {
    match verify_mission_evidence_bundle(document) {
        Ok(projection) => {
            if projection["valid"] == Value::Bool(true) {
                return Verdict::Accepted;
            }
            let failures = projection["failures"].to_string();
            let class = if failures.contains("bundle_digest_mismatch") {
                RejectionClass::DigestMismatch
            } else {
                RejectionClass::StructuralFailure
            };
            Verdict::rejected(class, failures)
        }
        Err(EvidenceBundleError::Invalid { reason }) => {
            let class = if reason.contains(BUNDLE_ABSENT_DIGEST) {
                RejectionClass::DigestAbsent
            } else if reason.contains(BUNDLE_MALFORMED_DIGEST) {
                RejectionClass::DigestMalformed
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
        Err(error) => Verdict::rejected(RejectionClass::Malformed, error.to_string()),
    }
}

/// The delivery receipt is verified against the delivery audit it was derived from, so its
/// adapter is built once per battery run with that audit already in hand.
pub fn delivery_receipt(receipt: &Value, delivery: &Value) -> Verdict {
    match verify_delivery_receipt(&DeliveryReceiptVerificationRequest {
        receipt: receipt.clone(),
        delivery: delivery.clone(),
    }) {
        Ok(verification) => {
            if verification.valid {
                return Verdict::Accepted;
            }
            let codes: Vec<&str> = verification
                .findings
                .iter()
                .map(|finding| finding.code.as_str())
                .collect();
            let class = if verification.supplied_receipt_digest.is_none() {
                RejectionClass::DigestAbsent
            } else if codes.contains(&"receipt_digest_malformed") {
                RejectionClass::DigestMalformed
            } else if codes.contains(&"receipt_digest_mismatch") {
                RejectionClass::DigestMismatch
            } else {
                RejectionClass::StructuralFailure
            };
            Verdict::rejected(class, codes.join(","))
        }
        Err(reason) => Verdict::rejected(RejectionClass::Malformed, reason),
    }
}

/// One verifier, ready to be pointed at any document.
pub type BoxedVerifier<'a> = Box<dyn Fn(&Value) -> Verdict + 'a>;

/// Every verifier as a name and a closure, for the cross-document confusion sweep.
pub fn all(delivery: &Value) -> Vec<(&'static str, BoxedVerifier<'_>)> {
    vec![
        ("context_certificate", Box::new(certificate)),
        ("autopilot_report", Box::new(autopilot)),
        ("research_dossier", Box::new(dossier)),
        ("mission_evidence_bundle", Box::new(evidence_bundle)),
        (
            "delivery_receipt",
            Box::new(move |document: &Value| delivery_receipt(document, delivery)),
        ),
    ]
}
