//! The newly covered verifiers, projected onto the battery's [`Verdict`].
//!
//! The projections stay as thin as the first battery's: each reads the verifier's own answer and
//! translates it. Two of them do have to do something the first battery's adapters never needed —
//! deserialise the document into the typed request the verifier takes — and that is not an
//! adapter deciding a verdict, it is the boundary the shipping transport crosses too. A document
//! the typed reader refuses never reaches the verifier in production either, so a refusal there is
//! a rejection of the document and is reported as one.
//!
//! Where a verifier answers with a bare `bool` or with `Ok(())`, the projection cannot invent the
//! three rejection classes the battery distinguishes; it reports [`RejectionClass::Malformed`] and
//! the battery's class assertions record the collapse instead of hiding it.

use bioprism_bioworlds::CatalogReport;
use bioprism_conformance::{ConformanceCertificate, ConformanceError};
use bioprism_cookbook::CookbookReport;
use bioprism_devplat::{
    verify_domain_evidence_provider_external_payload_replay,
    verify_domain_evidence_provider_replay, verify_domain_workflow,
    verify_domain_workflow_portfolio, verify_workbench,
    DomainEvidenceProviderExternalPayloadReplayRequest, DomainEvidenceProviderReplayRequest,
    WorkbenchVerificationRequest,
};
use bioprism_examples::RegistryReport;
use bioprism_prism::{Attestation, ResultBundle};
use bioprism_receipts_audit::{RejectionClass, Verdict};
use bioprism_registry::BenchmarkPack;
use bioprism_repair::{AcceptanceReport, RepairPlan};
use serde_json::Value;

/// The prose `RepairPlan::from_json` uses for a `plan_id` that is not a string.
pub const REPAIR_PLAN_ABSENT_ID: &str = "repair plan needs a string \"plan_id\"";
/// The prose `RepairPlan::from_json` uses for a `plan_id` the body does not hash to.
pub const REPAIR_PLAN_ID_MISMATCH: &str = "declares plan_id";
/// The prose the workbench verifier uses for a report digest of the wrong shape.
pub const WORKBENCH_MALFORMED_DIGEST: &str = "expected_report_digest";
/// The prose the certificate verifier uses for a digest that is not there.
pub const CERTIFICATE_ABSENT_DIGEST: &str = "certificate has no certificate_sha256";
/// The prose the cookbook report's reader uses for a digest that is not there.
pub const COOKBOOK_ABSENT_DIGEST: &str = "missing field `digest`";
/// The prose both replay verifiers use for an expected digest of the wrong shape.
pub const REPLAY_MALFORMED_DIGEST: &str = "is not a valid lowercase SHA-256 digest";
/// The prose the replay readers use for an expected digest that is not there.
pub const REPLAY_ABSENT_DIGEST: &str = "missing field `expected_";
/// The two spellings the replay verifiers use for an expected digest present but blank. They
/// differ because the internal verifier bounds a JSON value and the external one bounds text.
pub const REPLAY_BLANK_DIGEST: [&str; 2] = ["must be a non-empty value", "must be non-empty text"];

/// The class a replay verifier's refusal falls into.
///
/// Both replay verifiers reach the battery through a typed reader, so an expected digest that is
/// absent is refused by the reader and one of the wrong shape is refused by the verifier. The two
/// answers are different sentences and are projected onto different classes; anything else is
/// reported as `malformed` rather than guessed at.
fn replay_class(reason: &str) -> RejectionClass {
    if reason.contains(REPLAY_MALFORMED_DIGEST) {
        RejectionClass::DigestMalformed
    } else if reason.contains(REPLAY_ABSENT_DIGEST)
        || (reason.contains("expected_")
            && REPLAY_BLANK_DIGEST
                .iter()
                .any(|blank| reason.contains(blank)))
    {
        RejectionClass::DigestAbsent
    } else {
        RejectionClass::Malformed
    }
}

fn attestation_verdict(attestation: Attestation) -> Verdict {
    match attestation {
        Attestation::Valid => Verdict::Accepted,
        Attestation::Mismatch {
            claimed,
            recomputed,
        } => Verdict::rejected(
            RejectionClass::DigestMismatch,
            format!("claims {claimed}, recomputes to {recomputed}"),
        ),
        Attestation::Malformed(detail) => {
            let class = if detail.contains("missing") {
                RejectionClass::DigestAbsent
            } else if detail.contains("not a 64-character lowercase hex digest") {
                RejectionClass::DigestMalformed
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, detail)
        }
    }
}

pub fn prism_result_bundle(document: &Value) -> Verdict {
    attestation_verdict(ResultBundle::verify(document))
}

pub fn registry_pack(document: &Value) -> Verdict {
    attestation_verdict(BenchmarkPack::verify(document))
}

pub fn conformance_certificate(document: &Value) -> Verdict {
    match ConformanceCertificate::verify(document) {
        Ok(()) => Verdict::Accepted,
        Err(ConformanceError::CertificateDigestMismatch {
            claimed,
            recomputed,
        }) => Verdict::rejected(
            RejectionClass::DigestMismatch,
            format!("claims {claimed}, recomputes to {recomputed}"),
        ),
        Err(ConformanceError::CertificateDigestMalformed { claimed }) => Verdict::rejected(
            RejectionClass::DigestMalformed,
            format!("{claimed:?} is not a digest"),
        ),
        Err(error) => {
            let reason = error.to_string();
            let class = if reason.contains(CERTIFICATE_ABSENT_DIGEST) {
                RejectionClass::DigestAbsent
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
    }
}

/// The class a `digest_is_intact()` document's refusal falls into.
///
/// Three documents in this workspace seal themselves with a `digest` field and check it with a
/// bare `bool`. The reader in front of the bool can still say a field is missing, so an absent
/// digest keeps its own class; everything the bool refuses collapses into one answer, which is
/// what the battery's recorded gaps on these three subjects say.
///
/// The root check is needed because `serde` reports a missing `digest` the same way wherever it
/// occurred, and two of these documents nest a per-slice `digest` inside the sealed one. Looking
/// for the field is disambiguating an answer the reader left ambiguous, not deciding the integrity
/// question: whether the body matches its digest is still entirely the verifier's to say.
fn bool_check_class(reason: &str, document: &Value) -> RejectionClass {
    let sealed_root_lost_its_digest = document
        .as_object()
        .is_some_and(|map| !map.contains_key("digest"));
    if reason.contains(COOKBOOK_ABSENT_DIGEST) && sealed_root_lost_its_digest {
        RejectionClass::DigestAbsent
    } else {
        RejectionClass::Malformed
    }
}

/// The cookbook report answers with a bare `bool`, so this projection has one rejection class to
/// report and says which of the three the battery would otherwise have distinguished it into.
pub fn cookbook_report(document: &Value) -> Verdict {
    match serde_json::from_value::<CookbookReport>(document.clone()) {
        Ok(report) if report.digest_is_intact() => Verdict::Accepted,
        Ok(_) => Verdict::rejected(
            RejectionClass::DigestMismatch,
            "digest_is_intact() is false; the check reports no class of its own",
        ),
        Err(error) => Verdict::rejected(
            bool_check_class(&error.to_string(), document),
            error.to_string(),
        ),
    }
}

/// The bioworlds catalogue report and the examples registry report answer the same way the
/// cookbook report does — a bool — so they share its projection and its recorded collapse.
pub fn bioworlds_catalog_report(document: &Value) -> Verdict {
    match serde_json::from_value::<CatalogReport>(document.clone()) {
        Ok(report) if report.digest_is_intact() => Verdict::Accepted,
        Ok(_) => Verdict::rejected(
            RejectionClass::DigestMismatch,
            "digest_is_intact() is false; the check reports no class of its own",
        ),
        Err(error) => Verdict::rejected(
            bool_check_class(&error.to_string(), document),
            error.to_string(),
        ),
    }
}

pub fn examples_registry_report(document: &Value) -> Verdict {
    match serde_json::from_value::<RegistryReport>(document.clone()) {
        Ok(report) if report.digest_is_intact() => Verdict::Accepted,
        Ok(_) => Verdict::rejected(
            RejectionClass::DigestMismatch,
            "digest_is_intact() is false; the check reports no class of its own",
        ),
        Err(error) => Verdict::rejected(
            bool_check_class(&error.to_string(), document),
            error.to_string(),
        ),
    }
}

pub fn repair_plan(document: &Value) -> Verdict {
    match RepairPlan::from_json(document) {
        Ok(_) => Verdict::Accepted,
        Err(error) => {
            let reason = error.to_string();
            let class = if reason.contains(REPAIR_PLAN_ABSENT_ID) {
                RejectionClass::DigestAbsent
            } else if reason.contains(REPAIR_PLAN_ID_MISMATCH) {
                RejectionClass::DigestMismatch
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
    }
}

pub fn repair_acceptance_report(document: &Value) -> Verdict {
    match AcceptanceReport::from_json(document) {
        Ok(_) => Verdict::Accepted,
        Err(error) => Verdict::rejected(RejectionClass::StructuralFailure, error.to_string()),
    }
}

/// The workflow verifier answers `Ok` with a projection whose `structural_valid` and
/// `replay.matched` carry the verdict; `ok` is a constant and is deliberately not read.
pub fn domain_workflow(catalogue: &Value, tools: &Value, request: &Value) -> Verdict {
    match verify_domain_workflow(catalogue, tools, request) {
        Ok(projection) => {
            let structural = projection["structural_valid"] == Value::Bool(true);
            let replayed = projection["replay"]["matched"] == Value::Bool(true);
            if structural && replayed {
                return Verdict::Accepted;
            }
            Verdict::rejected(
                RejectionClass::StructuralFailure,
                projection["mismatches"].to_string(),
            )
        }
        Err(error) => {
            let reason = error.to_string();
            let class = if reason.contains("must be a 64-character hexadecimal digest") {
                RejectionClass::DigestMalformed
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
    }
}

pub fn domain_workflow_portfolio(catalogue: &Value, tools: &Value, request: &Value) -> Verdict {
    match verify_domain_workflow_portfolio(catalogue, tools, request) {
        Ok(projection) => {
            let sealed = projection["portfolio_digest_matched"] == Value::Bool(true);
            // `valid` alone is not the verdict: the verifier reports `verified_without_replay`
            // for a portfolio whose replay requests went missing, and calls that valid. It is
            // the verifier's own distinction, and a projection that flattened it would let a
            // request lose its replay demand and still read as accepted.
            let replayed = projection["verification_status"] == Value::String("verified".into());
            if sealed && replayed {
                return Verdict::Accepted;
            }
            let class = if sealed {
                RejectionClass::StructuralFailure
            } else {
                RejectionClass::DigestMismatch
            };
            Verdict::rejected(
                class,
                format!(
                    "{} {}",
                    projection["verification_status"], projection["mismatches"]
                ),
            )
        }
        Err(error) => {
            let reason = error.to_string();
            let class = if reason.contains("must be a 64-character hexadecimal digest") {
                RejectionClass::DigestMalformed
            } else if reason.contains("portfolio_digest") {
                RejectionClass::DigestAbsent
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
    }
}

pub fn workbench(document: &Value) -> Verdict {
    let request: WorkbenchVerificationRequest = match serde_json::from_value(document.clone()) {
        Ok(request) => request,
        Err(error) => return Verdict::rejected(RejectionClass::Malformed, error.to_string()),
    };
    match verify_workbench(&request) {
        Ok(report) => {
            if report.valid && report.report_digest_matched == Some(true) {
                return Verdict::Accepted;
            }
            let codes: Vec<&str> = report
                .mismatches
                .iter()
                .map(|mismatch| mismatch.code.as_str())
                .collect();
            let class = if report.report_digest_matched == Some(false) {
                RejectionClass::DigestMismatch
            } else if report.report_digest_matched.is_none() {
                RejectionClass::DigestAbsent
            } else {
                RejectionClass::StructuralFailure
            };
            Verdict::rejected(class, codes.join(","))
        }
        Err(error) => {
            let reason = error.to_string();
            let class = if reason.contains(WORKBENCH_MALFORMED_DIGEST) {
                RejectionClass::DigestMalformed
            } else {
                RejectionClass::Malformed
            };
            Verdict::rejected(class, reason)
        }
    }
}

pub fn provider_replay(document: &Value) -> Verdict {
    let request: DomainEvidenceProviderReplayRequest =
        match serde_json::from_value(document.clone()) {
            Ok(request) => request,
            Err(error) => {
                return Verdict::rejected(replay_class(&error.to_string()), error.to_string())
            }
        };
    match verify_domain_evidence_provider_replay(&request) {
        Ok(verification) => {
            if verification.matched {
                return Verdict::Accepted;
            }
            Verdict::rejected(
                RejectionClass::DigestMismatch,
                verification.differences.join(","),
            )
        }
        Err(error) => Verdict::rejected(replay_class(&error.to_string()), error.to_string()),
    }
}

pub fn external_payload_replay(document: &Value) -> Verdict {
    let request: DomainEvidenceProviderExternalPayloadReplayRequest =
        match serde_json::from_value(document.clone()) {
            Ok(request) => request,
            Err(error) => {
                return Verdict::rejected(replay_class(&error.to_string()), error.to_string())
            }
        };
    match verify_domain_evidence_provider_external_payload_replay(&request) {
        Ok(verification) => {
            if verification.matched {
                return Verdict::Accepted;
            }
            Verdict::rejected(
                RejectionClass::DigestMismatch,
                verification.differences.join(","),
            )
        }
        Err(error) => Verdict::rejected(replay_class(&error.to_string()), error.to_string()),
    }
}

/// One verifier, ready to be pointed at any document.
pub type BoxedVerifier = Box<dyn Fn(&Value) -> Verdict>;

/// Every newly covered verifier as a name and a closure, for the cross-document confusion sweep.
pub fn all(catalogue: Value, tools: Value) -> Vec<(&'static str, BoxedVerifier)> {
    let workflow_catalogue = catalogue.clone();
    let workflow_tools = tools.clone();
    vec![
        (
            "prism_result_bundle",
            Box::new(prism_result_bundle) as BoxedVerifier,
        ),
        ("registry_pack", Box::new(registry_pack)),
        ("conformance_certificate", Box::new(conformance_certificate)),
        ("cookbook_report", Box::new(cookbook_report)),
        (
            "bioworlds_catalog_report",
            Box::new(bioworlds_catalog_report),
        ),
        (
            "examples_registry_report",
            Box::new(examples_registry_report),
        ),
        ("repair_plan", Box::new(repair_plan)),
        (
            "repair_acceptance_report",
            Box::new(repair_acceptance_report),
        ),
        (
            "domain_workflow_verification",
            Box::new(move |document: &Value| {
                domain_workflow(&workflow_catalogue, &workflow_tools, document)
            }),
        ),
        (
            "domain_workflow_portfolio_verification",
            Box::new(move |document: &Value| {
                domain_workflow_portfolio(&catalogue, &tools, document)
            }),
        ),
        ("workbench_verification", Box::new(workbench)),
        ("provider_replay_request", Box::new(provider_replay)),
        (
            "external_payload_replay_request",
            Box::new(external_payload_replay),
        ),
    ]
}
