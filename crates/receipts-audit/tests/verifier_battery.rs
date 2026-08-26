//! The battery extended to thirteen more verifiers, against a source-checked inventory.
//!
//! Same generators, same expectations, same reporting discipline as `receipt_battery.rs`, over a
//! different set of subjects: thirteen verifiers the first battery did not reach. Each test
//! states one property, runs it over all of them, and pins what it measured, so a generator that
//! quietly stopped producing cases cannot turn this file green by doing nothing.
//!
//! This file does not reach every verifier in the workspace, and it no longer says it does.
//! `every_document_verifier_in_the_workspace_is_covered_or_recorded` is what keeps that honest: it
//! scans `crates/*/src` for verifier entry points and fails unless each one is either driven by a
//! battery or written down with a reason. Ten of them are document verifiers neither battery
//! reaches; they are named in `UNCOVERED_DOCUMENT_VERIFIERS` rather than left to be found later.
//!
//! # Two shapes of subject
//!
//! Most of these documents are sealed the way the first battery's are: one field holds a SHA-256
//! digest over everything else, and the verifier recomputes it. Two are not. The workflow
//! verification request is a *replay* comparison — it re-derives the retained document from the
//! caller's request and compares field by field — and the repair acceptance report carries no
//! integrity claim at all. Both are still measured, for what they do claim rather than for what a
//! sealed receipt would claim; `an_acceptance_report_seals_nothing` puts a number on the second.

mod verifier_adapters;
mod verifier_documents;

use bioprism_ids::to_canonical_string;
use bioprism_receipts_audit::{
    mutators, rng::SplitMix64, run_battery, walk, BatteryConfig, Expect, RejectionClass, Verdict,
    ANY_CLASS,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// One seed for the whole battery, repeated in every failure message, so a reported hole is a
/// complete reproduction recipe: this seed, that document type, that pointer.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

struct Subject {
    label: &'static str,
    document: Value,
    verify: Box<dyn Fn(&Value) -> Verdict>,
    config: BatteryConfig,
    /// The field whose value seals the document, where it has one.
    sealing_digest: Option<&'static str>,
    /// For a document that seals nothing, the number of semantic mutations its verifier accepts.
    ///
    /// A count rather than a gap list, because the gap is not a defect in a particular check: the
    /// document makes no integrity claim at all, so the battery is measuring the size of a silence
    /// rather than finding a broken promise.
    unsealed_accepted_cases: Option<usize>,
}

impl Subject {
    fn positions(&self) -> Vec<String> {
        walk::strided(&walk::pointers(&self.document), self.config.position_cap).0
    }
}

fn subjects() -> Vec<Subject> {
    let catalogue = verifier_documents::workflow_catalogue();
    let tools = verifier_documents::workflow_tool_definitions();
    let portfolio_catalogue = catalogue.clone();
    let portfolio_tools = tools.clone();
    vec![
        Subject {
            label: "prism_result_bundle",
            document: verifier_documents::prism_result_bundle(),
            verify: Box::new(verifier_adapters::prism_result_bundle),
            config: BatteryConfig::exhaustive("prism_result_bundle", SEED)
                .sealed_by("/bundle_sha256"),
            sealing_digest: Some("/bundle_sha256"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "registry_pack",
            document: verifier_documents::registry_pack(),
            verify: Box::new(verifier_adapters::registry_pack),
            config: BatteryConfig::exhaustive("registry_pack", SEED).sealed_by("/pack_sha256"),
            sealing_digest: Some("/pack_sha256"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "conformance_certificate",
            document: verifier_documents::conformance_certificate(),
            verify: Box::new(verifier_adapters::conformance_certificate),
            config: BatteryConfig::exhaustive("conformance_certificate", SEED)
                .sealed_by("/certificate_sha256"),
            sealing_digest: Some("/certificate_sha256"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "cookbook_report",
            document: verifier_documents::cookbook_report(),
            verify: Box::new(verifier_adapters::cookbook_report),
            config: BatteryConfig::exhaustive("cookbook_report", SEED).sealed_by("/digest"),
            sealing_digest: Some("/digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "bioworlds_catalog_report",
            document: verifier_documents::bioworlds_catalog_report(),
            verify: Box::new(verifier_adapters::bioworlds_catalog_report),
            config: BatteryConfig::exhaustive("bioworlds_catalog_report", SEED)
                .sealed_by("/digest"),
            sealing_digest: Some("/digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "examples_registry_report",
            document: verifier_documents::examples_registry_report(),
            verify: Box::new(verifier_adapters::examples_registry_report),
            config: BatteryConfig::exhaustive("examples_registry_report", SEED)
                .sealed_by("/digest"),
            sealing_digest: Some("/digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            // `plan_id` is deliberately not declared as this subject's sealing digest, because it
            // is not a digest: it is `repair-<issue id>-<first twelve hex digits of the body
            // digest>`, and the refinement that turns "refused" into "refused as digest_malformed"
            // reads the named field as a content hash. The plan's own seal is swept exhaustively by
            // `every_offset_of_the_repair_plans_truncated_body_digest_is_caught`, which also states
            // the width that seal actually has.
            label: "repair_plan",
            document: verifier_documents::repair_plan(),
            verify: Box::new(verifier_adapters::repair_plan),
            config: BatteryConfig::exhaustive("repair_plan", SEED),
            sealing_digest: None,
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "repair_acceptance_report",
            document: verifier_documents::repair_acceptance_report(),
            verify: Box::new(verifier_adapters::repair_acceptance_report),
            config: BatteryConfig::exhaustive("repair_acceptance_report", SEED),
            sealing_digest: None,
            unsealed_accepted_cases: Some(195),
        },
        Subject {
            label: "domain_workflow_verification",
            document: verifier_documents::domain_workflow_verification(),
            verify: Box::new(move |document| {
                verifier_adapters::domain_workflow(&catalogue, &tools, document)
            }),
            config: BatteryConfig::exhaustive("domain_workflow_verification", SEED),
            sealing_digest: None,
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "domain_workflow_portfolio_verification",
            document: verifier_documents::domain_workflow_portfolio_verification(),
            verify: Box::new(move |document| {
                verifier_adapters::domain_workflow_portfolio(
                    &portfolio_catalogue,
                    &portfolio_tools,
                    document,
                )
            }),
            config: BatteryConfig::exhaustive("domain_workflow_portfolio_verification", SEED)
                .sealed_by("/portfolio/portfolio_digest"),
            sealing_digest: Some("/portfolio/portfolio_digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "workbench_verification",
            document: verifier_documents::workbench_verification(),
            verify: Box::new(verifier_adapters::workbench),
            config: BatteryConfig::exhaustive("workbench_verification", SEED)
                .sealed_by("/expected_report_digest"),
            sealing_digest: Some("/expected_report_digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            // Both replay requests carry several independent expected digests, and several more
            // inside the observation those digests cover, so `body_edits_reported_as` is widened
            // back to every class. The rule it replaces — that an edit to the body must never be
            // blamed on a digest — assumes there is one digest to blame; here an edit that puts a
            // malformed value in *another* digest field really is a malformed digest, and refusing
            // it as one is the right answer rather than a misattribution.
            label: "provider_replay_request",
            document: verifier_documents::provider_replay_request(),
            verify: Box::new(verifier_adapters::provider_replay),
            config: BatteryConfig::exhaustive("provider_replay_request", SEED)
                .sealed_by("/expected_payload_digest")
                .body_edits_reported_as(ANY_CLASS),
            sealing_digest: Some("/expected_payload_digest"),
            unsealed_accepted_cases: None,
        },
        Subject {
            label: "external_payload_replay_request",
            document: verifier_documents::external_payload_replay_request(),
            verify: Box::new(verifier_adapters::external_payload_replay),
            config: BatteryConfig::exhaustive("external_payload_replay_request", SEED)
                .sealed_by("/expected_receipt_digest")
                .body_edits_reported_as(ANY_CLASS),
            sealing_digest: Some("/expected_receipt_digest"),
            unsealed_accepted_cases: None,
        },
    ]
}

/// A case the battery generates, a verifier does not answer the way the generator claimed, and
/// this repository has decided not to close — recorded so the exemption is visible, justified, and
/// load-bearing.
///
/// A gap on this list is asserted twice over: cases matching it are excused from the hole count,
/// *and* the gap must still fire. Closing the underlying behaviour without deleting the entry
/// fails the battery, so the list cannot rot into a set of stale excuses.
struct KnownGap {
    label: &'static str,
    mutator: &'static str,
    /// The pointer this gap covers. With `subtree`, the root of the subtree it covers; with a
    /// leading `*`, the suffix every position it covers ends with; with a leading `**`, a segment
    /// every position it covers contains. The last two are how one entry names a field that
    /// repeats once per element of a list, where the indices in the middle of the pointer vary.
    pointer: &'static str,
    subtree: bool,
    reason: &'static str,
}

impl KnownGap {
    fn matches(&self, label: &str, mutator: &str, pointer: &str) -> bool {
        if self.label != label || self.mutator != mutator {
            return false;
        }
        if let Some(segment) = self.pointer.strip_prefix("**") {
            return pointer.contains(segment);
        }
        if let Some(suffix) = self.pointer.strip_prefix('*') {
            return pointer.ends_with(suffix);
        }
        if !self.subtree {
            return self.pointer == pointer;
        }
        pointer == self.pointer
            || pointer
                .strip_prefix(self.pointer)
                .is_some_and(|rest| rest.starts_with('/'))
    }
}

/// Why an unrecognised key on a caller's request envelope is not treated as tampering.
const ENVELOPE_KEY: &str = "the envelope a caller hands the tool, not the sealed document inside \
    it. The MCP surface passes its arguments through untouched and writes its own ok, workflow and \
    schema-version fields onto what it returns, so a caller who retains a response and sends it \
    back carries keys this kernel never produced. Refusing an unrecognised key here would refuse \
    requests the shipped server accepts today";

/// Why the shape of the caller's replay request is not itself under the seal.
const REPLAY_REQUEST_SHAPE: &str = "inside the caller's replay request, which is an input to be \
    re-instantiated rather than a document under check. Two requests that instantiate to the same \
    workflow are the same request as far as this comparison is concerned — a step with no \
    `arguments` and a step with empty ones produce one instantiation — so the property the \
    verifier owns is that the *retained* document matches what the request reproduces, not that \
    the request was written one particular way";

/// Why deleting a defaulted field is not an edit a digest could catch.
const DEFAULTED_FIELD: &str =
    "every position this covers carries `#[serde(default)]`, so deleting \
    the field and writing its default value are the same document to the reader: the recomputation \
    sees the default either way, and there is no difference left for a digest to name";

/// Why a null sealing digest is answered without a class on the two replay requests.
const NULL_SEALING_DIGEST: &str = "a null sealing digest reaches these verifiers through a typed \
    reader that answers `invalid type: null, expected a string` without naming the field, so there \
    is nothing in the answer to attribute the defect to the digest with. The emptied case on the \
    external request is likewise answered as a bounds violation on the field's text rather than as \
    an absent digest";

/// Why the leaf types of the examples observation tree are not tightened here.
const FOREIGN_LEAF_TYPE: &str =
    "a leaf whose type belongs to `bioprism-section`, not to the crate \
    that owns this verifier. Refusing an unrecognised key on it means changing how \
    `LeakageWitness` and `UnresolvedObligation` are read everywhere, including inside the context \
    certificate the fiber canon is gated on, so it is reported here rather than done in passing \
    from a battery over a different document";

/// Why a float that a reader normalises is outside the seal of a `digest_is_intact()` report.
const RESERIALISED_FLOAT: &str =
    "the general defect behind every `digest_is_intact()` report: the \
    digest is recomputed by re-serialising the *parsed* struct, so anything the reader normalises \
    away is outside the seal. Refusing an unknown field closed the large instance of that; this is \
    the remainder. The field is an `f64`, JSON `0` and `0.0` are different canonical bytes and the \
    same `f64`, and so a document whose float was written as an integer literal verifies against a \
    digest taken over the other spelling. Closing it means a hand-written deserializer per numeric \
    field, which is replacing the reader rather than repairing it";

/// The claim-posture and lineage hole, stated once and referenced by every case that finds it.
const PROVIDER_UNCOVERED: &str =
    "claim_posture and parent_digests are covered by none of the five \
    expected digests this replay compares. payload_digest names the payload only; intake_digest is \
    taken over an observation object that omits both fields; and normalization_digest cannot see \
    them because `DomainEvidenceProviderNormalization::intake_arguments`, which does carry \
    claim_posture, is `#[serde(skip)]`. Covering them means changing what intake_digest hashes, \
    and that digest is a published wire value recorded in intake artifacts and reconciliation \
    records across the workspace, so this is reported rather than closed";

/// Why an unrecognised key on the caller-supplied halves of a workbench verification
/// request is not treated as tampering.
const WORKBENCH_REQUEST_INPUT: &str = "a caller-supplied input to the verification rather \
    than part of the document under seal. `expected_report_digest` covers `/report` and \
    nothing else; `session`, `ci_replay` and `policy` are handed in beside it so the verifier \
    can replay the run, and they are compared against the report rather than hashed. \
    Refusing an unrecognised key on them would reject a forward-compatible request without \
    protecting any digest, so the reader drops it and this entry records the decision";

const KNOWN_GAPS: [KnownGap; 34] = [
    KnownGap {
        label: "cookbook_report",
        mutator: "digest_length_change",
        pointer: "/digest",
        subtree: false,
        reason:
            "CookbookReport's only integrity check is `digest_is_intact() -> bool`, and a bool \
                 cannot separate a digest of the wrong shape from one of the wrong value. Every \
                 shape defect is therefore answered as a mismatch — the class reserved for \
                 evidence that the body moved. Closing it means giving the report a classifying \
                 verifier, which is new public surface rather than a repair to a defect",
    },
    KnownGap {
        label: "cookbook_report",
        mutator: "digest_case_change",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: an uppercased digest is not a digest, and is reported as a wrong \
                 one",
    },
    KnownGap {
        label: "cookbook_report",
        mutator: "unicode_confusable_string",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: a digest carrying a homoglyph is not hex, and is reported as a \
                 wrong digest rather than as a malformed one",
    },
    KnownGap {
        label: "cookbook_report",
        mutator: "empty_or_null_substitution",
        pointer: "/digest",
        subtree: false,
        reason:
            "the same bool, plus its reader: an emptied digest is answered as a mismatch and a \
                 null one as a type error, and neither is answered as an absent or malformed \
                 digest",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "digest_length_change",
        pointer: "/digest",
        subtree: false,
        reason:
            "CookbookReport's only integrity check is `digest_is_intact() -> bool`, and a bool \
                 cannot separate a digest of the wrong shape from one of the wrong value. Every \
                 shape defect is therefore answered as a mismatch — the class reserved for \
                 evidence that the body moved. Closing it means giving the report a classifying \
                 verifier, which is new public surface rather than a repair to a defect",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "digest_case_change",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: an uppercased digest is not a digest, and is reported as a wrong \
                 one",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "unicode_confusable_string",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: a digest carrying a homoglyph is not hex, and is reported as a \
                 wrong digest rather than as a malformed one",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "empty_or_null_substitution",
        pointer: "/digest",
        subtree: false,
        reason:
            "the same bool, plus its reader: an emptied digest is answered as a mismatch and a \
                 null one as a type error, and neither is answered as an absent or malformed \
                 digest",
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "digest_length_change",
        pointer: "/digest",
        subtree: false,
        reason:
            "CookbookReport's only integrity check is `digest_is_intact() -> bool`, and a bool \
                 cannot separate a digest of the wrong shape from one of the wrong value. Every \
                 shape defect is therefore answered as a mismatch — the class reserved for \
                 evidence that the body moved. Closing it means giving the report a classifying \
                 verifier, which is new public surface rather than a repair to a defect",
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "digest_case_change",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: an uppercased digest is not a digest, and is reported as a wrong \
                 one",
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "unicode_confusable_string",
        pointer: "/digest",
        subtree: false,
        reason: "the same bool: a digest carrying a homoglyph is not hex, and is reported as a \
                 wrong digest rather than as a malformed one",
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "empty_or_null_substitution",
        pointer: "/digest",
        subtree: false,
        reason:
            "the same bool, plus its reader: an emptied digest is answered as a mismatch and a \
                 null one as a type error, and neither is answered as an absent or malformed \
                 digest",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "unexpected_key",
        pointer: "*/check",
        subtree: false,
        reason: "an internally tagged enum, where `serde` cannot enforce `deny_unknown_fields` at \
                 all: the tagged representation buffers the content before it knows which variant \
                 it is reading, so there is no point at which an unrecognised key could be \
                 refused. Every other position inside a slice report now refuses one",
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "required_key_deletion",
        pointer: "*/structure/separating_depth",
        subtree: false,
        reason: "an `Option` whose value in this catalogue is `None`. Deleting the field and \
                 writing `null` deserialise to the same profile and re-serialise to the same \
                 bytes, so there is no difference left for the digest to name",
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "unexpected_key",
        pointer: "**/witnesses/",
        subtree: false,
        reason: FOREIGN_LEAF_TYPE,
    },
    KnownGap {
        label: "examples_registry_report",
        mutator: "unexpected_key",
        pointer: "**/unresolved_obligations/",
        subtree: false,
        reason: FOREIGN_LEAF_TYPE,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "unexpected_key",
        pointer: "/session",
        subtree: true,
        reason: WORKBENCH_REQUEST_INPUT,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "unexpected_key",
        pointer: "/ci_replay",
        subtree: true,
        reason: WORKBENCH_REQUEST_INPUT,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "unexpected_key",
        pointer: "/policy",
        subtree: false,
        reason: WORKBENCH_REQUEST_INPUT,
    },
    KnownGap {
        label: "domain_workflow_verification",
        mutator: "unexpected_key",
        pointer: "",
        subtree: false,
        reason: ENVELOPE_KEY,
    },
    KnownGap {
        label: "domain_workflow_verification",
        mutator: "unexpected_key",
        pointer: "/replay_request",
        subtree: true,
        reason: REPLAY_REQUEST_SHAPE,
    },
    KnownGap {
        label: "domain_workflow_verification",
        mutator: "required_key_deletion",
        pointer: "/replay_request",
        subtree: true,
        reason: REPLAY_REQUEST_SHAPE,
    },
    KnownGap {
        label: "domain_workflow_verification",
        mutator: "unexpected_key",
        pointer: "/instantiation",
        subtree: false,
        reason: "the replay compares every field it produced against the retained document, and a \
                 key the replay did not produce has nothing to compare against. It is left \
                 uncompared on purpose: the transport writes `preflight_report` onto an \
                 instantiation after this kernel returns it, so treating a key the replay does not \
                 produce as tampering would refuse every workflow the server hands back",
    },
    KnownGap {
        label: "domain_workflow_portfolio_verification",
        mutator: "unexpected_key",
        pointer: "",
        subtree: false,
        reason: ENVELOPE_KEY,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "unexpected_key",
        pointer: "",
        subtree: false,
        reason: ENVELOPE_KEY,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "unexpected_key",
        pointer: "/report",
        subtree: false,
        reason: "the retained report's own root, where `developer_workbench` writes ok, workflow \
                 and workbench_schema_version before returning it. `bioprism-devplat`'s own \
                 workbench registry strips exactly those three before hashing, so refusing them \
                 here would refuse the document the shipped tool produces. Every position *inside* \
                 the report is covered: the structs it is built from refuse a field they do not \
                 declare",
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "required_key_deletion",
        pointer: "/policy",
        subtree: true,
        reason: DEFAULTED_FIELD,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "required_key_deletion",
        pointer: "/session",
        subtree: true,
        reason: DEFAULTED_FIELD,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "required_key_deletion",
        pointer: "/report",
        subtree: true,
        reason: DEFAULTED_FIELD,
    },
    KnownGap {
        label: "workbench_verification",
        mutator: "required_key_deletion",
        pointer: "/ci_replay",
        subtree: true,
        reason: DEFAULTED_FIELD,
    },
    KnownGap {
        label: "external_payload_replay_request",
        mutator: "sibling_swap",
        pointer: "/domains",
        subtree: false,
        reason: "the receipt collects `domains` into a BTreeSet and re-emits it sorted, so the \
                 order a caller wrote them in is not part of the receipt's identity and two \
                 orderings really are the same request",
    },
    KnownGap {
        label: "external_payload_replay_request",
        mutator: "array_reordering",
        pointer: "/domains",
        subtree: false,
        reason: "the same set: `domains` is a set on the wire and an array only in JSON",
    },
    KnownGap {
        label: "external_payload_replay_request",
        mutator: "empty_or_null_substitution",
        pointer: "/expected_receipt_digest",
        subtree: false,
        reason: NULL_SEALING_DIGEST,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "empty_or_null_substitution",
        pointer: "/expected_payload_digest",
        subtree: false,
        reason: NULL_SEALING_DIGEST,
    },
];

/// Gaps that are holes: real, reproducible defects, left open because closing one would move a
/// published wire value or replace a reader rather than repair it. Kept separate from
/// [`KNOWN_GAPS`] so a reader can tell an explained boundary from an unclosed defect at a glance.
const OPEN_HOLES: [KnownGap; 10] = [
    KnownGap {
        label: "examples_registry_report",
        mutator: "numeric_boundary",
        pointer: "**/protected_recall",
        subtree: false,
        reason: RESERIALISED_FLOAT,
    },
    KnownGap {
        label: "bioworlds_catalog_report",
        mutator: "numeric_boundary",
        pointer: "*/structure/tag_camouflage_fraction",
        subtree: false,
        reason: RESERIALISED_FLOAT,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "digest_byte_flip",
        pointer: "/parent_digests",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "array_element_deletion",
        pointer: "/parent_digests",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "required_key_deletion",
        pointer: "/parent_digests",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "required_key_deletion",
        pointer: "/claim_posture",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "sibling_swap",
        pointer: "/claim_posture",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "array_element_deletion",
        pointer: "/claim_posture",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "array_reordering",
        pointer: "/claim_posture",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
    KnownGap {
        label: "provider_replay_request",
        mutator: "unicode_confusable_string",
        pointer: "/claim_posture",
        subtree: true,
        reason: PROVIDER_UNCOVERED,
    },
];

/// Positions where an unrecognised key cannot be refused at all, because `serde` will not combine
/// `deny_unknown_fields` with `flatten` and these requests flatten their observation into the
/// root. A limit of the reader, not a decision this repository made.
const FLATTENED_ENVELOPES: [(&str, &str); 2] = [
    ("provider_replay_request", ""),
    ("provider_replay_request", "/claim_posture"),
];

/// Accepted cases, counted apart by the reason this battery tolerates each one.
///
/// A recorded gap names a subject, a family and a pointer, so a stale one fails its own entry. An
/// unsealed subject can name none of those — its document makes no integrity claim, so there is no
/// broken promise to point at — and all it can offer is how many cases it let through. Folding the
/// two into one total would let a new acceptance on the unsealed subject cancel out a recorded gap
/// that stopped firing, and both would pass unseen. They are tallied apart and pinned apart, per
/// subject.
#[derive(Default)]
struct Excused {
    recorded: usize,
    unsealed: BTreeMap<&'static str, usize>,
}

impl Excused {
    fn accept(&mut self, subject: &Subject, case: &mutators::Mutation) {
        self.accept_with(subject, case, "");
    }

    /// `note` is appended to the refusal message where a family has something of its own to say
    /// about why an acceptance is wrong.
    fn accept_with(&mut self, subject: &Subject, case: &mutators::Mutation, note: &str) {
        if excused_here(subject.label, case.mutator, &case.pointer) {
            self.recorded += 1;
            return;
        }
        assert!(
            subject.unsealed_accepted_cases.is_some(),
            "{} (seed {SEED}): {} verified anyway{note}",
            subject.label,
            case.description
        );
        *self.unsealed.entry(subject.label).or_default() += 1;
    }

    fn pin(&self, recorded: usize, unsealed: &[(&str, usize)], family: &str) {
        let observed: Vec<(&str, usize)> = self
            .unsealed
            .iter()
            .map(|(label, count)| (*label, *count))
            .collect();
        assert_eq!(
            (self.recorded, observed.as_slice()),
            (recorded, unsealed),
            "{family}: the recorded-gap total and every unsealed subject's own total are pinned \
             separately, so an acceptance that appeared on a document sealing nothing cannot hide \
             behind a recorded gap that stopped firing"
        );
    }
}

fn excused_here(label: &str, mutator: &str, pointer: &str) -> bool {
    KNOWN_GAPS
        .iter()
        .any(|gap| gap.matches(label, mutator, pointer))
        || OPEN_HOLES
            .iter()
            .any(|gap| gap.matches(label, mutator, pointer))
        || (mutator == "unexpected_key" && FLATTENED_ENVELOPES.contains(&(label, pointer)))
}

/// The three documents whose only integrity check is a `bool`, so none of them can name the class
/// of a shape defect in its own digest.
const BOOL_CHECKED_REPORTS: [&str; 3] = [
    "cookbook_report",
    "bioworlds_catalog_report",
    "examples_registry_report",
];

fn wrong_digest(document: &Value, pointer: &str) -> Value {
    let claimed = walk::get(document, pointer)
        .and_then(Value::as_str)
        .expect("the sealing digest is a string");
    let replacement: String = claimed
        .chars()
        .map(|c| if c == 'a' { 'b' } else { 'a' })
        .collect();
    assert_ne!(replacement, claimed);
    walk::with_replacement(document, pointer, Value::String(replacement))
        .expect("the sealing digest is replaceable")
}

// -- the boundary of this file's claim -----------------------------------------------------------

// Every verifier entry point in the workspace, classified, so the claim above has to stay true.
//
// `every_document_verifier_in_the_workspace_is_covered_or_recorded` scans `crates/*/src` for the
// two shapes an entry point takes here — a `pub fn` whose name begins with `verify`, and
// `pub fn digest_is_intact` — and fails when a site appears in none of the four lists below. A
// verifier added tomorrow therefore has to be classified before this file goes green again.
//
// The scan's bound, stated because it is real: it finds functions by name, so a verifying
// *constructor* is invisible to it. `RepairPlan::from_json` and `AcceptanceReport::from_json` are
// two this battery already drives; `WorldTape` verifies its chain from a `#[serde(try_from)]`
// reader, which no name pattern would catch. The key is the path under `crates/` and the function
// name, so several sites that share both are one entry.

/// The entry points this battery's subjects drive, each named with its subject label.
const COVERED_HERE: [(&str, &str); 11] = [
    (
        "prism/src/bundle.rs::verify",
        "prism_result_bundle",
    ),
    (
        "registry/src/pack.rs::verify",
        "registry_pack",
    ),
    (
        "conformance/src/suite.rs::verify",
        "conformance_certificate",
    ),
    (
        "cookbook/src/report.rs::digest_is_intact",
        "cookbook_report",
    ),
    (
        "bioworlds/src/catalog.rs::digest_is_intact",
        "bioworlds_catalog_report",
    ),
    (
        "examples/src/registry.rs::digest_is_intact",
        "examples_registry_report",
    ),
    (
        "devplat/src/workflow.rs::verify_domain_workflow",
        "domain_workflow_verification",
    ),
    (
        "devplat/src/workflow.rs::verify_domain_workflow_portfolio",
        "domain_workflow_portfolio_verification",
    ),
    (
        "devplat/src/workbench.rs::verify_workbench",
        "workbench_verification",
    ),
    (
        "devplat/src/domain_evidence_provider.rs::verify_domain_evidence_provider_replay",
        "provider_replay_request",
    ),
    (
        "devplat/src/domain_evidence_provider_external.rs::verify_domain_evidence_provider_external_payload_replay",
        "external_payload_replay_request",
    ),
];

/// The entry points `receipt_battery.rs` drives.
const COVERED_BY_THE_RECEIPT_BATTERY: [(&str, &str); 5] = [
    ("section/src/certificate.rs::verify", "context_certificate"),
    (
        "autopilot/src/report.rs::verify_autopilot_report",
        "autopilot_report",
    ),
    (
        "research/src/dossier.rs::verify_dossier",
        "research_dossier",
    ),
    (
        "devplat/src/evidence_bundle.rs::verify_mission_evidence_bundle",
        "mission_evidence_bundle",
    ),
    (
        "devplat/src/delivery_receipt.rs::verify_delivery_receipt",
        "delivery_receipt and delivery_audit_behind_a_fixed_receipt",
    ),
];

/// Entry points that answer something other than whether a serialized document is intact.
///
/// A battery of document mutations has nothing to say to any of these: there is no document, or
/// the integrity claim belongs to a chain, a key, or a live struct rather than to bytes on a wire.
const NOT_A_DOCUMENT_VERIFIER: [(&str, &str); 31] = [
    (
        "bundle/src/attestation.rs::verify",
        "a MAC tag over a key and purpose preimage, not a document",
    ),
    (
        "bundle/src/attestation.rs::verify_for",
        "the same MAC check, additionally bound to a purpose",
    ),
    (
        "bundle/src/attestation.rs::verify_for_or_error",
        "verify_for rewrapped as a Result",
    ),
    (
        "bundle/src/mac.rs::verify_against",
        "a constant-time compare of two 32-byte tags",
    ),
    (
        "bundle/src/signature.rs::verify",
        "an Ed25519 signature over a preimage",
    ),
    (
        "bundle/src/signature.rs::verify_for",
        "the same signature check, bound to a purpose",
    ),
    (
        "bundle/src/signature.rs::verify_for_or_error",
        "verify_for rewrapped as a Result",
    ),
    (
        "bundle/src/trust.rs::verify_attestation",
        "a registry trust relationship: role, validity, revocation",
    ),
    (
        "bundle/src/audit.rs::verify_chain",
        "walks an append-only link chain rather than one document",
    ),
    (
        "bundle/src/audit.rs::verify",
        "binds a checkpoint to a chain head, then checks a MAC tag",
    ),
    (
        "bundle/src/bundle.rs::verify_with_registry",
        "a signature wrapper plus registry trust policy",
    ),
    (
        "safety/src/attest.rs::verify",
        "recomputes every link digest in a hash chain",
    ),
    (
        "ledger/src/ledger.rs::verify_chain",
        "a hash chain plus compaction-removal accounting",
    ),
    (
        "ledger/src/ledger.rs::verify_causal_acyclicity",
        "a DAG property: every parent precedes its child",
    ),
    (
        "runtime/src/tape.rs::verify_chain",
        "recomputes the step-linked tape chain",
    ),
    (
        "runtime/src/tape.rs::verify_checkpoint",
        "checks a checkpoint head against this tape's chain",
    ),
    (
        "weave/src/kernel.rs::verify_chain",
        "delegates to the ledger's chain walk",
    ),
    (
        "weave/src/ledger.rs::verify_chain",
        "recomputes every event link in the chain",
    ),
    (
        "graph/src/view.rs::verify",
        "rechecks provenance binding to a section the caller already holds",
    ),
    (
        "bioir/src/evidence.rs::verify_artifact",
        "hashes external artifact bytes, explicitly not this object",
    ),
    (
        "scale/src/cas.rs::verify",
        "rehashes opaque blobs against their content addresses",
    ),
    (
        "scale/src/split.rs::verify_item_assignment",
        "a family-straddle check on an item table; no digest",
    ),
    (
        "safety/src/supply.rs::verify",
        "compares two caller-supplied strings; this crate fetches nothing",
    ),
    (
        "routing/src/architecture.rs::verify_label",
        "an enum's label against its strategy's name, in memory",
    ),
    (
        "bioethics/src/validation.rs::verify",
        "evidence completeness and authorship; no digest anywhere",
    ),
    (
        "repair/src/verify.rs::verify",
        "evaluates plan criteria against a live world, not a document",
    ),
    (
        "repair/src/verify.rs::verify_successor",
        "the same, and the succession is recorded, never verified",
    ),
    (
        "cookbook/src/book.rs::verify",
        "resolves crate and test references against a live workspace",
    ),
    (
        "cookbook/src/verify.rs::verify_cookbook",
        "the free function behind Cookbook::verify",
    ),
    (
        "conformance/src/fixture.rs::verify",
        "reports drift that load() already recorded on a live struct",
    ),
    (
        "fabric/src/synth.rs::verifying",
        "a builder that inserts a role edge; the name prefix is a coincidence",
    ),
];

/// Document verifiers neither battery reaches. Recorded, not excused.
///
/// Each one reads a serialized document and checks its own integrity, which is exactly what these
/// generators are built to attack. They are listed so the gap is a number someone can act on
/// rather than a silence, and so the module doc above cannot quietly regrow its old claim.
const UNCOVERED_DOCUMENT_VERIFIERS: [(&str, &str); 10] = [
    (
        "bioworlds/src/slice.rs::digest_is_intact",
        "a per-slice self-seal; the catalogue report's own check never recurses into it",
    ),
    (
        "examples/src/report.rs::digest_is_intact",
        "a per-slice self-seal; the registry report's own check never recurses into it",
    ),
    (
        "bundle/src/bundle.rs::verify",
        "three sites: two attestation wrappers, plus the one that recomputes every manifest \
         entry digest of a shipped result bundle",
    ),
    (
        "factory/src/snapshot.rs::verify_digest",
        "a job-store checkpoint's embedded state_digest against its body",
    ),
    (
        "factory/src/authority.rs::verify",
        "an execution-authority snapshot: bounds, event chain, embedded state_digest",
    ),
    (
        "ledger/src/projection.rs::verify_state",
        "a projection checkpoint's carried state against its recorded state_digest",
    ),
    (
        "scale/src/escrow.rs::verify",
        "a published reveal: recomputes the commitment from salt and payload",
    ),
    (
        "stewardship/src/predeclaration.rs::verify",
        "a published pre-registration plan against its sealed content hash",
    ),
    (
        "registry/src/index.rs::verify_all",
        "every stored pack read back off disk, plus its filing address",
    ),
    (
        "bioeval/src/credit.rs::verify",
        "replays the rule on the evidence, catching an edited fraction in a serialised award",
    ),
];

/// Every `pub fn verify...` and `pub fn digest_is_intact` under `crates/*/src`.
///
/// Returned as `<path under crates/>::<fn name>`, with path separators normalised, so the keys
/// read the same on every platform.
fn verifier_entry_points(crates: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack: Vec<PathBuf> = fs::read_dir(crates)
        .expect("the workspace has a crates directory")
        .map(|entry| {
            entry
                .expect("a crate directory is readable")
                .path()
                .join("src")
        })
        .filter(|source| source.is_dir())
        .collect();
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).expect("a source directory is readable") {
            let path = entry.expect("a directory entry is readable").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let relative = path
                .strip_prefix(crates)
                .expect("every scanned file sits under crates/")
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path).expect("a source file is readable");
            for line in text.lines() {
                let Some(rest) = line.trim_start().strip_prefix("pub fn ") else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect();
                if name.starts_with("verify") || name == "digest_is_intact" {
                    found.insert(format!("{relative}::{name}"));
                }
            }
        }
    }
    found
}

#[test]
fn every_document_verifier_in_the_workspace_is_covered_or_recorded() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits inside crates/");
    let found = verifier_entry_points(crates);
    assert!(
        found.len() > 40,
        "the scan found only {} entry points, which reads as a scan that stopped walking the tree \
         rather than a workspace that lost its verifiers",
        found.len()
    );

    let classified: BTreeSet<&str> = COVERED_HERE
        .iter()
        .chain(COVERED_BY_THE_RECEIPT_BATTERY.iter())
        .chain(NOT_A_DOCUMENT_VERIFIER.iter())
        .chain(UNCOVERED_DOCUMENT_VERIFIERS.iter())
        .map(|(key, _)| *key)
        .collect();

    let unlisted: Vec<&str> = found
        .iter()
        .map(String::as_str)
        .filter(|key| !classified.contains(key))
        .collect();
    assert!(
        unlisted.is_empty(),
        "the workspace gained verifier entry points this file has never been told about. Each one \
         is either a document verifier a battery should drive or something else entirely, and \
         until it is classified this file's account of its own coverage is out of date:\n{}",
        unlisted.join("\n")
    );

    let departed: Vec<&str> = classified
        .iter()
        .copied()
        .filter(|key| !found.contains(*key))
        .collect();
    assert!(
        departed.is_empty(),
        "these entries name verifier entry points the workspace no longer has. A stale entry is an \
         excuse the scan can never test, so delete it rather than leaving it:\n{}",
        departed.join("\n")
    );

    assert_eq!(
        (
            COVERED_HERE.len(),
            COVERED_BY_THE_RECEIPT_BATTERY.len(),
            NOT_A_DOCUMENT_VERIFIER.len(),
            UNCOVERED_DOCUMENT_VERIFIERS.len(),
        ),
        (11, 5, 31, 10),
        "eleven entry points driven here, five by the first battery, thirty-one that verify \
         something other than a document, and ten document verifiers no battery reaches yet"
    );
}

// -- the starting point -------------------------------------------------------------------------

#[test]
fn every_newly_covered_verifier_accepts_the_document_its_own_producer_emits() {
    let mut checked = 0;
    for subject in subjects() {
        let verdict = (subject.verify)(&subject.document);
        assert_eq!(
            verdict,
            Verdict::Accepted,
            "{}: a battery over a document its verifier already refuses would report holes that \
             are artefacts of the starting point, and this one was {verdict}",
            subject.label
        );
        checked += 1;
    }
    assert_eq!(checked, 13, "thirteen newly covered verifiers");
}

// -- the headline property ----------------------------------------------------------------------

#[test]
fn the_whole_battery_finds_no_hole_outside_the_gaps_this_repository_has_named() {
    let mut total_cases = 0;
    let mut total_positions = 0;
    let mut bounds = Vec::new();
    let mut gaps_fired = vec![0usize; KNOWN_GAPS.len()];
    let mut holes_fired = vec![0usize; OPEN_HOLES.len()];
    let mut flattened_fired = vec![0usize; FLATTENED_ENVELOPES.len()];
    for subject in subjects() {
        let outcome = run_battery(&subject.document, &subject.config, &subject.verify);
        assert!(
            outcome.canonicalisation_violations.is_empty(),
            "{}",
            outcome.report()
        );
        assert_eq!(
            outcome.baseline,
            Verdict::Accepted,
            "{}: {}",
            subject.label,
            outcome.baseline
        );

        if let Some(expected) = subject.unsealed_accepted_cases {
            assert_eq!(
                outcome.holes.len(),
                expected,
                "{}: this document seals nothing, and the battery pins how much that leaves \
                 unchecked. The number moved, which means either the document or its reader \
                 changed:\n{}",
                subject.label,
                outcome.report()
            );
        } else {
            let mut unexplained = Vec::new();
            for hole in &outcome.holes {
                if let Some(index) = KNOWN_GAPS
                    .iter()
                    .position(|gap| gap.matches(hole.label, hole.mutator, &hole.pointer))
                {
                    gaps_fired[index] += 1;
                } else if let Some(index) = OPEN_HOLES
                    .iter()
                    .position(|gap| gap.matches(hole.label, hole.mutator, &hole.pointer))
                {
                    holes_fired[index] += 1;
                } else if let Some(index) = FLATTENED_ENVELOPES.iter().position(|entry| {
                    hole.mutator == "unexpected_key"
                        && *entry == (hole.label, hole.pointer.as_str())
                }) {
                    flattened_fired[index] += 1;
                } else {
                    unexplained.push(hole.to_string());
                }
            }
            assert!(
                unexplained.is_empty(),
                "{}\n{}",
                outcome.coverage.bound_statement(),
                unexplained.join("\n")
            );
        }

        assert!(
            outcome.coverage.is_exhaustive(),
            "{} is bounded to every {}th pointer, {} of {}; a hole this battery does not report \
             is then a position it never visited rather than a refusal it observed",
            subject.label,
            outcome.coverage.position_step,
            outcome.coverage.positions_covered,
            outcome.coverage.positions_total
        );
        assert_eq!(
            subject.positions().len(),
            walk::pointers(&subject.document).len(),
            "{} visits fewer positions than it has",
            subject.label
        );
        assert!(
            outcome.coverage.cases > 300,
            "{} generated only {} cases",
            subject.label,
            outcome.coverage.cases
        );
        total_cases += outcome.coverage.cases;
        total_positions += outcome.coverage.positions_covered;
        bounds.push(outcome.coverage.bound_statement());
    }
    for (gap, fired) in KNOWN_GAPS.iter().zip(&gaps_fired) {
        assert!(
            *fired > 0,
            "the recorded gap on {} / {} / {:?} no longer fires — delete the entry rather than \
             leaving a stale excuse in the battery. Its reason was: {}",
            gap.label,
            gap.mutator,
            gap.pointer,
            gap.reason
        );
    }
    for (hole, fired) in OPEN_HOLES.iter().zip(&holes_fired) {
        assert!(
            *fired > 0,
            "the open hole on {} / {} / {:?} no longer reproduces — it was closed, and this entry \
             should have gone with the fix. Its reason was: {}",
            hole.label,
            hole.mutator,
            hole.pointer,
            hole.reason
        );
    }
    for ((label, pointer), fired) in FLATTENED_ENVELOPES.iter().zip(&flattened_fired) {
        assert!(
            *fired > 0,
            "the flattened envelope on {label} / {pointer:?} no longer accepts an unrecognised \
             key. Either the reader stopped flattening its observation into the root or the \
             position moved, and this entry should have gone with that change rather than staying \
             as an excuse the battery never reaches"
        );
    }
    assert_eq!(
        (total_cases, total_positions),
        (47_976, 5_221),
        "the battery's coverage is a pinned claim; bounds were:\n{}",
        bounds.join("\n")
    );
}

// -- digest integrity, exhaustively -------------------------------------------------------------

#[test]
fn every_single_byte_digest_mutation_is_caught_at_every_offset_of_every_digest_field() {
    let mut fields = 0;
    let mut offsets = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let positions = walk::pointers(&subject.document);
        let found = mutators::digest_pointers_among(&subject.document, &positions);
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::digest_byte_flips(&subject.document, &found, &mut rng);
        assert_eq!(
            cases.len(),
            found.len() * mutators::DIGEST_CHARS,
            "{}: digest coverage must be exhaustive over offsets, never sampled",
            subject.label
        );
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        fields += found.len();
        offsets += cases.len();
    }
    assert_eq!(
        (fields, offsets),
        (164, 10_496),
        "one hundred and sixty-four digest fields across thirteen documents, each checked at all \
         64 offsets"
    );
    excused.pin(64, &[("repair_acceptance_report", 64)], "digest byte flip");
}

#[test]
fn every_offset_of_the_repair_plans_truncated_body_digest_is_caught() {
    let document = verifier_documents::repair_plan();
    assert_eq!(
        verifier_adapters::repair_plan(&document),
        Verdict::Accepted,
        "the sweep starts from a plan its reader accepts"
    );
    assert!(
        mutators::digest_pointers(&document)
            .iter()
            .all(|pointer| pointer != "/plan_id"),
        "the shape detector must not be finding this seal, which is exactly why the sweep is \
         written out here instead of coming from `digest_byte_flips`"
    );

    let plan_id = document["plan_id"]
        .as_str()
        .expect("the plan carries its id")
        .to_string();
    let (start, end) = verifier_documents::repair_plan_id_digest_span();
    assert_eq!(
        end - start,
        12,
        "a repair plan is sealed by twelve hex characters — forty-eight bits, not the two hundred \
         and fifty-six a sha256 field carries"
    );

    let mut checked = 0;
    for offset in start..end {
        let current = plan_id.as_bytes()[offset] as char;
        for replacement in "0123456789abcdef".chars() {
            if replacement == current {
                continue;
            }
            let mut mutated = plan_id.clone();
            mutated.replace_range(offset..offset + 1, &replacement.to_string());
            let candidate = walk::with_replacement(&document, "/plan_id", Value::String(mutated))
                .expect("the id is replaceable");
            let verdict = verifier_adapters::repair_plan(&candidate);
            assert_eq!(
                verdict.class(),
                Some(RejectionClass::DigestMismatch),
                "(seed {SEED}) offset {offset} of the plan id, {current} -> {replacement}, was \
                 answered {verdict}"
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked,
        12 * 15,
        "every offset against every other hex digit"
    );
}

#[test]
fn a_truncated_extended_or_recased_digest_is_caught_at_every_digest_field() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let positions = walk::pointers(&subject.document);
        let found = mutators::digest_pointers_among(&subject.document, &positions);
        let mut cases = mutators::digest_length_changes(&subject.document, &found);
        cases.extend(mutators::digest_case_changes(&subject.document, &found));
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 1_148,
        "one hundred and sixty-four digest fields, seven shape mutations each"
    );
    excused.pin(0, &[("repair_acceptance_report", 7)], "digest shape");
}

// -- canonicalisation invariance ----------------------------------------------------------------

#[test]
fn object_key_reordering_never_changes_a_verdict_at_any_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let baseline = (subject.verify)(&subject.document);
        assert_eq!(baseline, Verdict::Accepted, "{}", subject.label);
        let baseline_bytes = to_canonical_string(&subject.document).expect("canonicalises");
        let mut rng = SplitMix64::new(SEED);
        let cases =
            mutators::object_key_reorderings(&subject.document, &subject.positions(), &mut rng);
        assert!(
            !cases.is_empty(),
            "{} produced no reordering",
            subject.label
        );
        for case in &cases {
            assert_eq!(case.expect, Expect::VerdictUnchanged);
            let reordered = case.applied(&subject.document);
            assert_eq!(
                to_canonical_string(&reordered).expect("canonicalises"),
                baseline_bytes,
                "{} (seed {SEED}): {} moved the canonical bytes — canonicalisation is not \
                 key-order invariant, and every digest in this workspace is an artefact of one \
                 serializer",
                subject.label,
                case.description
            );
            assert_eq!(
                (subject.verify)(&reordered),
                baseline,
                "{} (seed {SEED}): {} changed the verdict",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 1_271,
        "reordering cases across thirteen documents"
    );
}

#[test]
fn array_reordering_always_changes_a_verdict_at_any_position() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::array_reorderings(&subject.document, &subject.positions(), &mut rng);
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept_with(
                    &subject,
                    case,
                    " — JSON arrays are ordered and a digest that ignores their order is not \
                     naming the document",
                );
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 541,
        "array reordering cases across thirteen documents"
    );
    excused.pin(3, &[("repair_acceptance_report", 4)], "array reordering");
}

// -- absent, malformed, and mismatching digests stay three different answers ---------------------

#[test]
fn a_document_whose_sealing_digest_is_absent_is_rejected_distinctly_from_one_whose_digest_is_wrong()
{
    let mut checked = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest else {
            continue;
        };
        let stripped =
            walk::with_removal(&subject.document, pointer).expect("the digest field is removable");
        let absent = (subject.verify)(&stripped);
        let wrong = (subject.verify)(&wrong_digest(&subject.document, pointer));
        assert_eq!(
            absent.class(),
            Some(RejectionClass::DigestAbsent),
            "{}: a missing {pointer} must be reported as missing, not as tampering — got {absent}",
            subject.label
        );
        assert_eq!(
            wrong.class(),
            Some(RejectionClass::DigestMismatch),
            "{}: a wrong {pointer} must be reported as a mismatch — got {wrong}",
            subject.label
        );
        assert_ne!(absent.class(), wrong.class(), "{}", subject.label);
        checked += 1;
    }
    assert_eq!(
        checked, 10,
        "ten of the thirteen documents carry a sealing digest"
    );
}

#[test]
fn a_shape_broken_sealing_digest_is_rejected_as_malformed_and_never_as_tampering() {
    let mut checked = 0;
    let mut excused = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest else {
            continue;
        };
        for broken in [
            "NOT-64-LOWERCASE-HEX-CHARACTERS",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
            "abc",
        ] {
            let document =
                walk::with_replacement(&subject.document, pointer, Value::String(broken.into()))
                    .expect("the digest field is replaceable");
            let verdict = (subject.verify)(&document);
            if BOOL_CHECKED_REPORTS.contains(&subject.label) {
                assert_eq!(
                    verdict.class(),
                    Some(RejectionClass::DigestMismatch),
                    "the recorded gap says a bool cannot name this class, and here it named one — \
                     delete the entry"
                );
                excused += 1;
            } else {
                assert_eq!(
                    verdict.class(),
                    Some(RejectionClass::DigestMalformed),
                    "{}: {pointer} = {broken:?} is a defect in the claimed digest, not evidence \
                     that the body moved — got {verdict}",
                    subject.label
                );
            }
            checked += 1;
        }
    }
    assert_eq!(
        (checked, excused),
        (30, 9),
        "ten sealed documents, three shape defects each"
    );
}

#[test]
fn deleting_any_field_at_any_visited_position_is_rejected_and_never_silently_accepted() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let mut cases = mutators::required_key_deletions(&subject.document, &subject.positions());
        cases.extend(mutators::array_element_deletions(
            &subject.document,
            &subject.positions(),
        ));
        assert!(!cases.is_empty(), "{} produced no deletion", subject.label);
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 5_208, "deletion cases across thirteen documents");
    excused.pin(23, &[("repair_acceptance_report", 16)], "deletion");
}

// -- numbers, strings, and structure ------------------------------------------------------------

#[test]
fn replacing_any_visited_value_with_an_empty_string_or_null_is_rejected() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let cases = mutators::empty_or_null_substitutions(&subject.document, &subject.positions());
        assert!(
            !cases.is_empty(),
            "{} produced no substitution",
            subject.label
        );
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 10_400,
        "empty-or-null cases across thirteen documents"
    );
    excused.pin(0, &[("repair_acceptance_report", 21)], "empty or null");
}

#[test]
fn a_string_replaced_by_a_confusable_form_is_rejected_at_every_visited_position() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let cases = mutators::unicode_confusable_strings(&subject.document, &subject.positions());
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 10_579,
        "confusable cases across thirteen documents"
    );
    excused.pin(16, &[("repair_acceptance_report", 81)], "confusable string");
}

#[test]
fn a_swapped_pair_of_same_typed_siblings_is_rejected_at_every_visited_container() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::sibling_swaps(&subject.document, &subject.positions(), &mut rng);
        assert!(!cases.is_empty(), "{} produced no swap", subject.label);
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept_with(
                    &subject,
                    case,
                    " — no key was added or removed, only the binding between a name and a value",
                );
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 1_018, "sibling swaps across thirteen documents");
    excused.pin(2, &[("repair_acceptance_report", 3)], "sibling swap");
}

#[test]
fn an_unexpected_key_at_any_level_is_rejected_except_where_a_recorded_gap_says_otherwise() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::unexpected_keys(&subject.document, &subject.positions(), &mut rng);
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 1_576,
        "unexpected-key cases across thirteen documents"
    );
    excused.pin(218, &[], "unexpected key");
}

#[test]
fn an_object_written_with_a_duplicate_key_resolves_to_a_document_that_is_rejected() {
    let mut cases_run = 0;
    let mut excused = Excused::default();
    for subject in subjects() {
        let cases = mutators::wire_duplicate_keys(&subject.document, &subject.positions());
        assert!(!cases.is_empty(), "{} produced no duplicate", subject.label);
        for case in &cases {
            if (subject.verify)(&case.applied(&subject.document)).is_accepted() {
                excused.accept(&subject, case);
            }
        }
        cases_run += cases.len();
    }
    assert_eq!(
        cases_run, 780,
        "duplicate-key cases across thirteen documents, every one of them refused"
    );
    excused.pin(0, &[], "duplicate key");
}

// -- what an unsealed document leaves unchecked --------------------------------------------------

#[test]
fn an_acceptance_report_seals_nothing_and_the_battery_measures_how_much_that_leaves_unchecked() {
    let document = verifier_documents::repair_acceptance_report();
    assert_eq!(
        verifier_adapters::repair_acceptance_report(&document),
        Verdict::Accepted
    );
    assert!(
        document
            .as_object()
            .expect("object")
            .keys()
            .all(|key| !key.ends_with("_digest") && !key.ends_with("_sha256")
                || key == "world_sha256"),
        "an acceptance report carries no digest over itself; if one was added, seal this subject \
         with it instead of measuring the silence"
    );

    let outcome = run_battery(
        &document,
        &BatteryConfig::exhaustive("repair_acceptance_report", SEED),
        &|value| verifier_adapters::repair_acceptance_report(value),
    );
    assert_eq!(
        (outcome.coverage.cases, outcome.holes.len()),
        (359, 195),
        "the reader refuses undeclared keys and rederives outcome and admissibility from the item \
         statuses, and that is all it checks: 195 of 359 semantic mutations are read back into a \
         report that verifies, including every one of the 64 offsets of the world_sha256 the \
         report was evaluated against"
    );

    let rebound = walk::with_replacement(&document, "/world_sha256", Value::String("f".repeat(64)))
        .expect("the world digest is replaceable");
    assert_eq!(
        verifier_adapters::repair_acceptance_report(&rebound),
        Verdict::Accepted,
        "the world identity a repair was judged against is recorded and never checked — the crate \
         says as much in `verify_successor`'s own documentation, and this is what that costs"
    );
}

// -- cross-document confusion and idempotence ---------------------------------------------------

#[test]
fn a_document_fed_to_a_verifier_that_does_not_own_it_is_always_rejected() {
    let catalogue = verifier_documents::workflow_catalogue();
    let tools = verifier_documents::workflow_tool_definitions();
    let library = verifier_documents::library();
    let mut confusions = 0;
    for (verifier_name, verify) in verifier_adapters::all(catalogue, tools) {
        for (document_name, document) in &library {
            let verdict = verify(document);
            if *document_name == verifier_name {
                assert_eq!(
                    verdict,
                    Verdict::Accepted,
                    "{verifier_name} must accept its own document"
                );
                continue;
            }
            assert!(
                !verdict.is_accepted(),
                "{verifier_name} accepted a {document_name}"
            );
            confusions += 1;
        }
    }
    assert_eq!(
        confusions,
        13 * 12,
        "thirteen verifiers against twelve foreign documents each"
    );
}

#[test]
fn verifying_the_same_document_twice_yields_the_same_verdict_on_acceptance_and_on_refusal() {
    let mut checked = 0;
    for subject in subjects() {
        let first = (subject.verify)(&subject.document);
        let second = (subject.verify)(&subject.document);
        assert_eq!(
            first, second,
            "{}: not idempotent when it accepts",
            subject.label
        );

        // Idempotence has to survive refusal too: a verifier that memoised its first answer would
        // pass the check above and still report a tampered document differently the second time.
        let tampered = match subject.sealing_digest {
            Some(pointer) => wrong_digest(&subject.document, pointer),
            None => {
                let position = walk::pointers(&subject.document)
                    .into_iter()
                    .find(|pointer| {
                        matches!(walk::get(&subject.document, pointer), Some(Value::String(text)) if !text.is_empty())
                    })
                    .expect("every unsealed subject has a string to move");
                walk::with_replacement(
                    &subject.document,
                    &position,
                    Value::String("moved after the fact".into()),
                )
                .expect("the position is replaceable")
            }
        };
        let first = (subject.verify)(&tampered);
        let second = (subject.verify)(&tampered);
        assert_eq!(
            first, second,
            "{}: not idempotent when it refuses",
            subject.label
        );
        checked += 1;
    }
    assert_eq!(checked, 13);
}

// -- the adapters' assumptions ------------------------------------------------------------------

#[test]
fn the_rejection_reason_strings_the_adapters_key_on_are_the_ones_the_verifiers_emit() {
    let stripped = walk::with_removal(
        &verifier_documents::conformance_certificate(),
        "/certificate_sha256",
    )
    .expect("removable");
    let error = bioprism_conformance::ConformanceCertificate::verify(&stripped)
        .expect_err("a certificate without its digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifier_adapters::CERTIFICATE_ABSENT_DIGEST),
        "{error}"
    );

    let stripped =
        walk::with_removal(&verifier_documents::cookbook_report(), "/digest").expect("removable");
    let error = serde_json::from_value::<bioprism_cookbook::CookbookReport>(stripped)
        .expect_err("a report without its digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifier_adapters::COOKBOOK_ABSENT_DIGEST),
        "{error}"
    );

    let broken = walk::with_replacement(
        &verifier_documents::provider_replay_request(),
        "/expected_payload_digest",
        Value::String("nope".into()),
    )
    .expect("replaceable");
    let request: bioprism_devplat::DomainEvidenceProviderReplayRequest =
        serde_json::from_value(broken).expect("the request still parses");
    let error = bioprism_devplat::verify_domain_evidence_provider_replay(&request)
        .expect_err("a shape-broken expected digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifier_adapters::REPLAY_MALFORMED_DIGEST),
        "{error}"
    );

    let stripped = walk::with_removal(
        &verifier_documents::provider_replay_request(),
        "/expected_shape_digest",
    )
    .expect("removable");
    let error =
        serde_json::from_value::<bioprism_devplat::DomainEvidenceProviderReplayRequest>(stripped)
            .expect_err("a replay request without an expected digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifier_adapters::REPLAY_ABSENT_DIGEST),
        "{error}"
    );

    let stripped =
        walk::with_removal(&verifier_documents::repair_plan(), "/plan_id").expect("removable");
    let error = bioprism_repair::RepairPlan::from_json(&stripped)
        .expect_err("a plan without its id is refused");
    assert!(
        error
            .to_string()
            .contains(verifier_adapters::REPAIR_PLAN_ABSENT_ID),
        "{error}"
    );
}
