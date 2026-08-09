//! WeaveIR schema conformance against blueprint 23.38's worked examples.
//!
//! Every fixture below is the JSON printed in 23.38, copied rather than paraphrased. A test that
//! deserialises a document the module invented would prove nothing about interoperability.

use bioprism_weavelang::ir::*;
use serde_json::{json, Value};

fn canonical(value: &Value) -> String {
    String::from_utf8(bioprism_ids::to_canonical_bytes(value).expect("canonicalises"))
        .expect("canonical JSON is UTF-8")
}

#[test]
fn the_canonical_event_envelope_of_23_38_deserialises_and_re_serialises() {
    let source = json!({
      "weaveVersion": "0.1.0",
      "eventId": "urn:uuid:5fb7d9db-ec6f-42c3-b35f-02b0640f30a8",
      "eventType": "aurora.weave.act.claim.v1",
      "source": "agent:investigator-2",
      "threadId": "thread:repair-42",
      "programId": "sha256:program",
      "choreographyState": "evidence-gathering",
      "logicalClock": 37,
      "causalParents": ["event:36"],
      "time": "2026-08-07T22:10:03Z",
      "schema": "aurora:epistemic/claim@1.0.0",
      "securityLabel": { "level": "confidential", "compartments": ["project-x"] },
      "payloadRef": "sha256:payload",
      "idempotencyKey": "claim:repair-42:idempotency-regenerated:v1",
      "signature": { "method": "ed25519", "keyId": "did:key:...", "value": "..." }
    });

    let event: WeaveEvent = serde_json::from_value(source).expect("23.38's envelope must load");
    assert_eq!(event.logical_clock, 37);
    assert_eq!(event.security_label.compartments, vec!["project-x"]);
    assert_eq!(event.time.as_deref(), Some("2026-08-07T22:10:03Z"));

    let round_tripped: WeaveEvent =
        serde_json::from_value(serde_json::to_value(&event).expect("serialises"))
            .expect("round trips");
    assert_eq!(round_tripped, event);
}

#[test]
fn a_signature_is_dropped_on_load_because_it_cannot_be_inside_what_it_signs() {
    let source = json!({
      "weaveVersion": "0.1.0",
      "eventId": "urn:uuid:5fb7d9db-ec6f-42c3-b35f-02b0640f30a8",
      "eventType": "aurora.weave.act.claim.v1",
      "source": "agent:investigator-2",
      "threadId": "thread:repair-42",
      "programId": "sha256:program",
      "choreographyState": "evidence-gathering",
      "logicalClock": 37,
      "causalParents": ["event:36"],
      "schema": "aurora:epistemic/claim@1.0.0",
      "securityLabel": { "level": "confidential", "compartments": ["project-x"] },
      "payloadRef": "sha256:payload",
      "idempotencyKey": "claim:repair-42:idempotency-regenerated:v1",
      "signature": { "method": "ed25519", "keyId": "did:key:...", "value": "..." }
    });

    let event: WeaveEvent = serde_json::from_value(source).expect("loads");
    let bytes = String::from_utf8(event.signing_bytes().expect("canonicalises")).expect("utf-8");
    assert!(
        !bytes.contains("signature"),
        "23.38: signatures are excluded from the signed payload they wrap"
    );
    assert!(bytes.contains("\"logicalClock\":37"));
}

#[test]
fn an_event_omits_time_entirely_when_no_caller_supplied_one() {
    let event = WeaveEvent {
        weave_version: WEAVE_EVENT_VERSION.to_string(),
        event_id: derive_event_id("thread:t", 1, "sha256:p"),
        event_type: event_type_for(bioprism_weave::ActKind::Claim),
        source: "role:Investigator".to_string(),
        thread_id: "thread:t".to_string(),
        program_id: "urn:weave:program:p@sha256:0".to_string(),
        choreography_state: "start".to_string(),
        logical_clock: 1,
        causal_parents: Vec::new(),
        time: None,
        schema: "aurora:epistemic/claim@1.0.0".to_string(),
        security_label: SecurityLabel::new("public"),
        payload_ref: "sha256:p".to_string(),
        idempotency_key: "k".to_string(),
    };
    let bytes = String::from_utf8(event.signing_bytes().expect("canonicalises")).expect("utf-8");
    assert!(
        !bytes.contains("\"time\""),
        "an absent timestamp must be absent, not an empty string"
    );
    assert_eq!(event.event_type, "aurora.weave.act.claim.v1");
}

#[test]
fn a_derived_event_identifier_is_a_function_of_thread_clock_and_payload() {
    let first = derive_event_id("thread:repair-42", 37, "sha256:payload");
    let second = derive_event_id("thread:repair-42", 37, "sha256:payload");
    let different_clock = derive_event_id("thread:repair-42", 38, "sha256:payload");
    assert_eq!(first, second);
    assert_ne!(first, different_clock);
    assert!(first.starts_with("urn:weave:event:sha256:"));
}

#[test]
fn the_claim_payload_of_23_38_deserialises_and_keeps_its_confidence_exact() {
    let source = json!({
      "propositionId": "claim:idempotency-key-regenerated",
      "rendering": "The retry path regenerates the idempotency key.",
      "endorser": "agent:investigator-2",
      "evidence": [
        {
          "artifact": "sha256:file",
          "locator": {
            "kind": "source-range",
            "path": "payments/idempotency.py",
            "startLine": 44,
            "endLine": 61
          },
          "relation": "supports"
        }
      ],
      "assumptions": ["assumption:retry-path-active"],
      "confidence": {
        "kind": "calibrated-probability",
        "value": 0.78,
        "profile": "sha256:calibration-profile"
      }
    });

    let payload: ClaimPayload = serde_json::from_value(source).expect("23.38's claim must load");
    assert_eq!(payload.evidence[0].locator.start_line, 44);
    assert_eq!(payload.confidence.kind, "calibrated-probability");

    let encoded = canonical(&serde_json::to_value(&payload).expect("serialises"));
    assert!(
        encoded.contains("\"value\":0.78"),
        "the canonical encoder must reproduce the probability exactly: {encoded}"
    );
}

#[test]
fn the_context_capsule_of_23_38_deserialises_with_its_budgets_and_omissions() {
    let source = json!({
      "capsuleId": "ctx:repair-42:14",
      "baseHash": "sha256:previous-capsule",
      "recipient": "role:skeptic",
      "objective": "Find a blocking counterexample to the current hypothesis.",
      "successContract": "challenge-or-attest@1",
      "worldSnapshot": "sha256:world",
      "commonGround": {
        "verified": ["fact:duplicate-payment-reproduced"],
        "working": ["assumption:single-region"]
      },
      "focus": {
        "claim": "claim:idempotency-key-regenerated",
        "support": ["evidence:17"],
        "counterevidence": []
      },
      "openObligations": ["obligation:commit-order"],
      "grants": ["grant:repo-read"],
      "budgets": { "tokens": 12000, "toolCalls": 8 },
      "omissions": [{ "scope": "frontend", "reason": "no causal dependency found" }]
    });

    let capsule: ContextCapsuleIr = serde_json::from_value(source).expect("23.38's capsule loads");
    assert_eq!(capsule.budgets.get("tokens"), Some(&12000));
    assert_eq!(capsule.budgets.get("toolCalls"), Some(&8));
    assert_eq!(capsule.omissions[0].scope, "frontend");
    assert!(
        capsule.focus.counterevidence.is_empty(),
        "an empty counterevidence list is not the same as an absent one"
    );
}

#[test]
fn the_commitment_event_of_23_38_deserialises() {
    let source = json!({
      "commitmentId": "commitment:patch-42",
      "creditor": "role:lead",
      "debtor": "agent:patcher-1",
      "antecedent": { "event": "proposal:accepted" },
      "deliverableType": "aurora:git/patch@1",
      "deadline": "2026-08-07T22:30:00Z",
      "qualityPredicates": ["tests.unit.pass", "security.no-high-findings"],
      "authority": ["grant:branch-write-42"],
      "budgetLease": "lease:patcher-42",
      "remedy": "reassign-and-revert"
    });

    let commitment: CommitmentIr = serde_json::from_value(source).expect("loads");
    assert_eq!(commitment.antecedent.event, "proposal:accepted");
    assert_eq!(commitment.quality_predicates.len(), 2);
}

#[test]
fn the_continuation_of_23_38_deserialises_with_its_r2_fidelity() {
    let source = json!({
      "continuationId": "continuation:repair-42:decision-14",
      "fidelity": "R2",
      "worldSnapshot": "sha256:world",
      "localRoleState": "sha256:role-state",
      "epistemicCheckpoint": "sha256:evidence-ledger",
      "commitmentCheckpoint": "sha256:commitment-ledger",
      "openObligations": ["obligation:commit-order"],
      "grants": ["grant:repo-read"],
      "budgetLease": "lease:investigation-42",
      "resumeInputSchema": "aurora:weave/next-action@1",
      "expectedOutputSchema": "aurora:weave/world-delta@1",
      "invariants": ["working-tree-clean", "tests-baseline-fails"]
    });

    let continuation: ContinuationIr = serde_json::from_value(source).expect("loads");
    assert_eq!(continuation.fidelity, ResumeGrade::R2);
    assert_eq!(
        continuation.fidelity.to_fidelity(),
        bioprism_weave::Fidelity::Lossy
    );
    assert_eq!(continuation.invariants.len(), 2);
}

#[test]
fn the_molecule_card_of_23_38_deserialises_with_required_and_forbidden_effects() {
    let source = json!({
      "name": "reliable-repair",
      "version": "0.1.0",
      "interfaces": ["aurora:repair/fix@1"],
      "programHash": "sha256:program",
      "policyHash": "sha256:policy",
      "choreographyHash": "sha256:choreography",
      "effects": {
        "required": ["repo.read", "branch.write", "test.run"],
        "forbidden": ["main.merge", "deploy.production"]
      },
      "verifiedProfile": "prism://profile/sha256:...",
      "adapters": ["a2a-v1", "mcp-2026-07-28", "local"]
    });

    let card: MoleculeCard = serde_json::from_value(source).expect("loads");
    assert_eq!(card.effects.required.len(), 3);
    assert_eq!(card.effects.forbidden, vec!["main.merge", "deploy.production"]);
    assert_eq!(card.adapters.len(), 3);
}

#[test]
fn a_content_reference_names_its_algorithm_the_way_23_37_requires() {
    let reference = content_ref(&json!({"a": 1})).expect("hashes");
    assert!(reference.starts_with("sha256:"));
    assert_eq!(reference.len(), "sha256:".len() + 64);
    assert_eq!(reference, content_ref(&json!({"a": 1})).expect("hashes"));
}

#[test]
fn a_clearance_must_dominate_both_the_level_and_the_compartments() {
    let restricted_patient = SecurityLabel {
        level: "restricted".to_string(),
        compartments: vec!["patient-data".to_string()],
    };
    let restricted_site = SecurityLabel {
        level: "restricted".to_string(),
        compartments: vec!["site-7".to_string()],
    };
    let both = SecurityLabel {
        level: "restricted".to_string(),
        compartments: vec!["patient-data".to_string(), "site-7".to_string()],
    };

    assert!(!restricted_patient.dominates(&restricted_site));
    assert!(both.dominates(&restricted_patient));
    assert!(both.dominates(&restricted_site));
    assert!(!restricted_patient.dominates(&both));
    assert!(SecurityLabel::new("confidential").dominates(&SecurityLabel::new("internal")));
    assert!(!SecurityLabel::new("internal").dominates(&SecurityLabel::new("confidential")));
}

#[test]
fn the_ir_document_keeps_snake_case_while_the_event_envelope_keeps_camel_case() {
    let ir = bioprism_weavelang::compile::compile(
        bioprism_weavelang::reference::COMPLETE_PROGRAM,
    )
    .expect("compiles");
    let encoded = canonical(&serde_json::to_value(&ir).expect("serialises"));

    // 23.03's document, verbatim field names.
    for field in [
        "weave_ir_version",
        "program_id",
        "package_lock",
        "state_graph",
        "evaluation_hooks",
        "actor_role",
        "payload_type",
    ] {
        assert!(encoded.contains(&format!("\"{field}\"")), "missing {field}");
    }
    // 23.38's envelope, verbatim field names, in the same crate and deliberately different.
    assert!(!encoded.contains("\"weaveIrVersion\""));
}
