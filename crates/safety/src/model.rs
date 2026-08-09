//! The shipped threat model for the part of section 13 this crate owns.
//!
//! This is where the abstractions in [`crate::threat`] are made to say something. Everything above
//! is machinery; this module is the answer, and the answer is uncomfortable.
//!
//! # The result, stated plainly
//!
//! Of the threats in [`section_13`], the ones reported [`crate::threat::ThreatStatus::Mitigated`] are **all
//! threats against the platform's own honesty** — an unverified signature presented as verified,
//! tenant isolation presented as applied, an assertion filed as an observation, containment
//! reported on a partial blast radius. Not one of them is a threat against an attacker.
//!
//! That is not a defect in the model, it is the model working. A pure-Rust, single-process,
//! no-network library has exactly one security capability: it can make a false claim impossible to
//! construct. Every perimeter control in section 13 — the hardened container, the seccomp profile,
//! the encrypted holdout mount, the signed manifest, the tenant storage policy — belongs to
//! infrastructure this workspace does not contain, and appears here as
//! [`Mitigation::DeclaredOnly`] or
//! [`Mitigation::Absent`].
//!
//! A test in this module holds that property. If somebody later marks a perimeter threat as
//! enforced, `every_enforced_threat_defends_a_claim_rather_than_a_perimeter` fails.
//!
//! # One threat nobody has analysed
//!
//! `T-13.05-grader-steers-next-trial` carries [`AbsenceReason::NotAnalysed`], and
//! [`ThreatModel::unanalysed`] returns exactly it. The evaluator's typed claim reaches the control
//! plane, which selects the next trial's inputs, so a grader can shape what it will be asked to
//! grade. [`crate::boundary::BoundaryModel::feedback_loops`] finds the path. 13.05 discusses the
//! agent/evaluator diode and does not mention this route; 13.14 discusses reward hacking and does
//! not mention that the platform's own scheduler is a reward channel. Marking it as analysed would
//! require a control, and there isn't one.

use crate::threat::{
    AbsenceReason, Adversary, Asset, AttackClass, Capability, ControlRole, Mitigation, Threat,
    ThreatModel, Unrepresentable,
};

/// The adversaries 13.01 and 13.02 name, expressed as capability sets.
pub fn adversaries() -> Vec<Adversary> {
    vec![
        Adversary::new("pack-publisher", "a publisher of benchmark packs")
            .with(Capability::AuthorsContent)
            .with(Capability::ControlsBuildInput)
            .with(Capability::ObservesPublicSurface),
        Adversary::new("benchmarked-agent", "the agent under evaluation")
            .with(Capability::ExecutesInAgentSandbox)
            .with(Capability::ControlsPeerMessage)
            .with(Capability::AuthorsContent),
        Adversary::new("malicious-grader", "a submitted evaluator or oracle")
            .with(Capability::ExecutesInEvaluatorSandbox)
            .with(Capability::AuthorsContent),
        Adversary::new("compromised-dependency", "an upstream package or base image")
            .with(Capability::ControlsBuildInput),
        Adversary::new("result-submitter", "a party publishing a score")
            .with(Capability::SubmitsResults)
            .with(Capability::AuthorsContent)
            .with(Capability::ObservesPublicSurface),
        Adversary::new("insider", "a holder of a valid platform credential")
            .with(Capability::HoldsCredential)
            .with(Capability::AuthorsContent)
            .with(Capability::SubmitsResults)
            .with(Capability::ObservesPublicSurface),
        Adversary::new("malicious-tenant", "another organisation on the same deployment")
            .with(Capability::HoldsCredential)
            .with(Capability::ExecutesInAgentSandbox)
            .with(Capability::ObservesPublicSurface),
        Adversary::new("external-service", "a model provider or federation peer")
            .with(Capability::ControlsExternalService),
        Adversary::new("public-reader", "anyone reading the leaderboard")
            .with(Capability::ObservesPublicSurface),
    ]
}

/// Absent because a hypervisor, kernel, container runtime or network is required and none is here.
fn needs_infrastructure(component: &str) -> AbsenceReason {
    AbsenceReason::RequiresAbsentInfrastructure {
        component: component.to_string(),
    }
}

/// The model.
///
/// Each threat cites the blueprint module it comes from. Read the `declared_in` strings: several
/// point at a checkable function in this crate *and* say what that function does not bind.
pub fn section_13() -> ThreatModel {
    let mut model = ThreatModel::new();
    for adversary in adversaries() {
        model = model.with_adversary(adversary);
    }

    model
        .with_threat(
            Threat::new(
                "T-13.03-sandbox-escape",
                "13.03",
                Asset::WorkerFleet,
                AttackClass::SandboxEscape,
                "agent_sandbox",
                "Benchmark or agent code breaks out of its container and reaches the host.",
            )
            .requiring(Capability::ExecutesInAgentSandbox)
            .mitigated_by(Mitigation::declared(
                "hardened container or microVM on an isolated node",
                ControlRole::Preventative,
                "13.03 and 13.04; recorded as a request by bioprism_sdk::sandbox::SandboxDeclaration",
            ))
            .mitigated_by(Mitigation::absent(
                "seccomp, capability drop, user namespaces, cgroup limits",
                ControlRole::Preventative,
                needs_infrastructure("a kernel this library does not run against"),
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.03-ambient-credentials",
                "13.03",
                Asset::ProviderCredential,
                AttackClass::SecretExfiltration,
                "agent_sandbox",
                "A sandbox inherits a provider key, a cloud metadata endpoint or a registry \
                 credential from its host.",
            )
            .requiring(Capability::ExecutesInAgentSandbox)
            .mitigated_by(Mitigation::declared(
                "no ambient secrets; credentials arrive through a broker",
                ControlRole::Preventative,
                "13.03 and 13.08; the broker is crates/runtime's 05.08, not this crate's",
            ))
            .mitigated_by(Mitigation::absent(
                "scoped short-lived credential issuance",
                ControlRole::Preventative,
                AbsenceReason::OwnedElsewhere {
                    owner: "bioprism-runtime (05.08 effects, permissions and the secret broker)"
                        .into(),
                },
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.04-escape-detection",
                "13.04",
                Asset::WorkerFleet,
                AttackClass::SandboxEscape,
                "agent_sandbox",
                "An escape attempt runs to completion because nothing watches denied syscalls, \
                 namespace operations or device access.",
            )
            .requiring(Capability::ExecutesInAgentSandbox)
            .mitigated_by(Mitigation::absent(
                "syscall and integrity auditing",
                ControlRole::Detective,
                needs_infrastructure("an audit subsystem"),
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.05-holdout-read",
                "13.05",
                Asset::HiddenOracle,
                AttackClass::HiddenTestDiscovery,
                "agent_sandbox",
                "The agent reads hidden tests, holdout labels or oracle data from a shared mount \
                 or a reachable service.",
            )
            .requiring(Capability::ExecutesInAgentSandbox)
            .mitigated_by(Mitigation::declared(
                "two-sandbox separation with just-in-time hidden-asset mounts",
                ControlRole::Preventative,
                "13.05; modelled and checkable at bioprism_safety::boundary::ArtifactKind::\
                 forbidden_in, which binds only artifacts routed through BoundaryModel::deliver",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.05-grader-overreach",
                "13.05",
                Asset::ResultIntegrity,
                AttackClass::EvaluatorTampering,
                "evaluator_sandbox",
                "A grader publishes its own result, edits pack metadata, or chooses the population \
                 it is compared against.",
            )
            .requiring(Capability::ExecutesInEvaluatorSandbox)
            .mitigated_by(Mitigation::declared(
                "evaluator authority limited to claims and evidence",
                ControlRole::Preventative,
                "13.05; refused by bioprism_safety::boundary::EvaluatorAuthority::authorize for \
                 callers that route through it, and by nothing for callers that do not",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.05-grader-steers-next-trial",
                "13.05",
                Asset::ResultIntegrity,
                AttackClass::EvaluatorTampering,
                "control_plane",
                "The grader's typed claim reaches the scheduler, which composes the next trial's \
                 inputs, so a grader can shape what it will next be asked to grade. 13.05 states \
                 the diode between the two sandboxes and does not mention this route.",
            )
            .requiring(Capability::ExecutesInEvaluatorSandbox)
            .mitigated_by(Mitigation::absent(
                "grader-blind trial selection",
                ControlRole::Preventative,
                AbsenceReason::NotAnalysed,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.12-injection-to-tool",
                "13.12",
                Asset::PrivateTenantData,
                AttackClass::PromptOrToolInjection,
                "agent_sandbox",
                "Text in a retrieved document, a tool result or a peer-agent message supplies the \
                 arguments of a subsequent tool call.",
            )
            .requiring(Capability::AuthorsContent)
            .mitigated_by(Mitigation::declared(
                "provenance labelling and effect authorisation outside the model",
                ControlRole::Preventative,
                "13.12; provenance is modelled at bioprism_safety::provenance and the effect \
                 decision is crates/runtime's",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.13-authority-elevation",
                "13.13",
                Asset::ResultIntegrity,
                AttackClass::PromptOrToolInjection,
                "agent_sandbox",
                "Untrusted content is summarised into a system preamble and arrives with the \
                 authority of the summariser.",
            )
            .requiring(Capability::AuthorsContent)
            .mitigated_by(Mitigation::declared(
                "provenance ceilings and no authority gain through derivation",
                ControlRole::Preventative,
                "13.13; refused by bioprism_safety::provenance::ContextAssembly::add, which binds \
                 only assemblers that route through it",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.14-answer-leak",
                "13.14",
                Asset::ResultIntegrity,
                AttackClass::LeaderboardFraud,
                "catalog",
                "A benchmark ships its own reference answer inside the context it hands the agent.",
            )
            .requiring(Capability::AuthorsContent)
            .mitigated_by(Mitigation::declared(
                "pre-publication witness checks",
                ControlRole::Detective,
                "13.14; witnesses from bioprism_safety::integrity::check_answer_containment, which \
                 finds exact containment only",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.14-degenerate-oracle",
                "13.14",
                Asset::ResultIntegrity,
                AttackClass::LeaderboardFraud,
                "evaluator_sandbox",
                "A constant output satisfies the oracle across tasks, so a score is obtainable \
                 without solving anything.",
            )
            .requiring(Capability::ExecutesInAgentSandbox)
            .mitigated_by(Mitigation::declared(
                "degeneracy probes before a pack is promoted",
                ControlRole::Detective,
                "13.14; bioprism_safety::integrity::check_oracle_degeneracy returns \
                 underdetermined when nobody probed",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.14-author-submits-result",
                "13.14",
                Asset::PublisherIdentity,
                AttackClass::LeaderboardFraud,
                "catalog",
                "The party that authored the task also submits the result compared on it.",
            )
            .requiring(Capability::AuthorsContent)
            .requiring(Capability::SubmitsResults)
            .mitigated_by(Mitigation::declared(
                "author and result trust are separate",
                ControlRole::Preventative,
                "13.14; checked at bioprism_safety::integrity::check_author_submitter_separation",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.15-artifact-substitution",
                "13.15",
                Asset::RegistrySupplyChain,
                AttackClass::ArtifactSubstitution,
                "artifact_service",
                "A tag resolves to different bytes than it did when the manifest was written.",
            )
            .requiring(Capability::ControlsBuildInput)
            .mitigated_by(Mitigation::declared(
                "digest pinning, with no floating references in published runs",
                ControlRole::Preventative,
                "13.15; bioprism_safety::supply::Inventory::audit_for_publication checks the \
                 declaration and DigestObservation::verify compares two digests the caller supplied",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.15-signature-forgery",
                "13.15",
                Asset::SigningKey,
                AttackClass::SignatureAbuse,
                "public_registry_mirror",
                "A pack carries a signature that does not verify, or verifies against a revoked \
                 key, and is installed anyway.",
            )
            .requiring(Capability::ControlsBuildInput)
            .mitigated_by(Mitigation::absent(
                "signature verification against a trust root",
                ControlRole::Preventative,
                needs_infrastructure("key material, a trust root and a revocation source"),
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.15-signature-overclaim",
                "13.15",
                Asset::RegistrySupplyChain,
                AttackClass::MisleadingSecurityClaim,
                "catalog",
                "An operator reads a signed manifest and concludes the signature was checked.",
            )
            .requiring(Capability::ObservesPublicSurface)
            .mitigated_by(Mitigation::enforced(
                "SignatureStatus has one variant",
                ControlRole::Preventative,
                Unrepresentable::NoValueClaimsASignatureVerified,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.19-cross-tenant-access",
                "13.19",
                Asset::PrivateTenantData,
                AttackClass::CrossTenantAccess,
                "control_plane",
                "One tenant reads another's artifacts, cache entries, traces or search results.",
            )
            .requiring(Capability::HoldsCredential)
            .mitigated_by(Mitigation::absent(
                "tenant-scoped storage, cache and worker pools",
                ControlRole::Preventative,
                needs_infrastructure("storage, caches and a worker fleet"),
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.19-isolation-overclaim",
                "13.19",
                Asset::PrivateTenantData,
                AttackClass::MisleadingSecurityClaim,
                "control_plane",
                "A record states that a tenant was isolated when the only isolation is a manifest \
                 field.",
            )
            .requiring(Capability::ObservesPublicSurface)
            .mitigated_by(Mitigation::enforced(
                "TenantIsolation has one variant",
                ControlRole::Preventative,
                Unrepresentable::NoValueClaimsTenantIsolationWasApplied,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.20-audit-rewrite",
                "13.20",
                Asset::ResultIntegrity,
                AttackClass::EvaluatorTampering,
                "control_plane",
                "An actor with write access to the audit log rewrites it consistently, leaving no \
                 trace.",
            )
            .requiring(Capability::HoldsCredential)
            .mitigated_by(Mitigation::declared(
                "hash-linked batches with signed checkpoints and independent backup",
                ControlRole::Detective,
                "13.20; bioprism_safety::attest::AuditLog::verify recomputes the chain and detects \
                 an edited record, and detects nothing about a wholesale rewrite",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.20-assertion-as-observation",
                "13.20",
                Asset::ResultIntegrity,
                AttackClass::MisleadingSecurityClaim,
                "control_plane",
                "An audit row records somebody's claim in the same shape as a computed fact, and a \
                 reader cannot tell which is which.",
            )
            .requiring(Capability::ObservesPublicSurface)
            .mitigated_by(Mitigation::enforced(
                "Statement separates Observed from Asserted, and Observation is closed",
                ControlRole::Preventative,
                Unrepresentable::NoAssertionIsFiledAsAnObservation,
            ))
            .mitigated_by(Mitigation::enforced(
                "Attestation is sealed and its observed constructor refuses unwitnessable claims",
                ControlRole::Preventative,
                Unrepresentable::NoAttestationClaimsObservationWithoutOne,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.21-corpus-decay",
                "13.21",
                Asset::ResultIntegrity,
                AttackClass::MisleadingSecurityClaim,
                "trusted_review",
                "Confirmed red-team findings never become sentinels, so a fixed class regresses \
                 silently and the suite still passes.",
            )
            .requiring(Capability::HoldsCredential)
            .mitigated_by(Mitigation::declared(
                "each confirmed class becomes a minimised protected CI sentinel",
                ControlRole::Recovery,
                "13.21; bioprism_safety::disclosure::Finding::into_regression_cell refuses an \
                 unconfirmed finding and Corpus::uncovered lists boundaries with no sentinel",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.23-containment-overclaim",
                "13.23",
                Asset::ResultIntegrity,
                AttackClass::MisleadingSecurityClaim,
                "control_plane",
                "An incident is reported contained while the lineage query that found the affected \
                 results returned partial output.",
            )
            .requiring(Capability::HoldsCredential)
            .mitigated_by(Mitigation::enforced(
                "ContainmentReport is sealed behind the blast-radius gate",
                ControlRole::Preventative,
                Unrepresentable::NoContainmentReportExistsWithoutACompleteBlastRadius,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.05-diode-overclaim",
                "13.05",
                Asset::HiddenOracle,
                AttackClass::MisleadingSecurityClaim,
                "evaluator_sandbox",
                "A crossing record exists for a movement the trust model forbids, so a reviewer \
                 auditing the record set concludes the diode held.",
            )
            .requiring(Capability::ExecutesInEvaluatorSandbox)
            .mitigated_by(Mitigation::enforced(
                "Crossing is sealed behind BoundaryModel::deliver",
                ControlRole::Preventative,
                Unrepresentable::NoCrossingRecordExistsThatTheModelForbids,
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.25-clinical-output",
                "13.25",
                Asset::ProtectedHealthInformation,
                AttackClass::PrivacyLeakage,
                "public_api",
                "A research pack's output is presented as a personalised clinical recommendation, \
                 an urgency triage or a treatment selection.",
            )
            .requiring(Capability::AuthorsContent)
            .mitigated_by(Mitigation::declared(
                "prohibited clinical outputs are refused unconditionally",
                ControlRole::Preventative,
                "13.25; refused by bioprism_safety::release::MedicalBoundary::admit and by \
                 bioprism-onco's own typed boundary",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.26-dual-use-release",
                "13.26",
                Asset::RegistrySupplyChain,
                AttackClass::SupplyChainCompromise,
                "public_api",
                "A pack or architecture that materially uplifts a harmful capability is released \
                 because the risk dimensions nobody rated read as low.",
            )
            .requiring(Capability::AuthorsContent)
            .mitigated_by(Mitigation::declared(
                "a release gate that refuses an unrated dimension",
                ControlRole::Preventative,
                "13.26; bioprism_safety::release::ReleaseGate::decide errors before reading any \
                 rating when one is missing",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.26-suppression-as-safety",
                "13.26",
                Asset::PublisherIdentity,
                AttackClass::MisleadingSecurityClaim,
                "trusted_review",
                "Safety review is used to withhold the existence of a weakness rather than the \
                 detail of how to exploit it.",
            )
            .requiring(Capability::HoldsCredential)
            .mitigated_by(Mitigation::declared(
                "exploit detail may be withheld; existence may not",
                ControlRole::Preventative,
                "13.26; refused by bioprism_safety::release::withhold",
            )),
        )
        .with_threat(
            Threat::new(
                "T-13.01-declared-control-trusted",
                "13.01",
                Asset::UserMachine,
                AttackClass::MisleadingSecurityClaim,
                "user_client",
                "A developer running an untrusted pack locally reads this workspace's threat model, \
                 sees a mitigation column with an entry in every row, and concludes the pack is \
                 contained.",
            )
            .requiring(Capability::ObservesPublicSurface)
            .mitigated_by(Mitigation::enforced(
                "Enforcer names no runtime component, so no record can claim one",
                ControlRole::Preventative,
                Unrepresentable::NoValueNamesARuntimeEnforcer,
            ))
            .mitigated_by(Mitigation::enforced(
                "sdk Enforcement has no Enforced variant",
                ControlRole::Preventative,
                Unrepresentable::NoValueClaimsIsolationWasApplied,
            )),
        )
}

/// A short human-readable statement of the model's coverage, for a report header.
pub fn coverage_summary() -> String {
    let model = section_13();
    format!(
        "bioprism-safety section 13 model: {}",
        model.coverage().summary()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threat::ThreatStatus;

    #[test]
    fn every_enforced_threat_defends_a_claim_rather_than_a_perimeter() {
        for threat in section_13().threats {
            if threat.status() == ThreatStatus::Mitigated {
                assert_eq!(
                    threat.class,
                    AttackClass::MisleadingSecurityClaim,
                    "{} is reported as mitigated; this crate cannot enforce a perimeter control, \
                     so an enforced non-claim threat means somebody wrote a lie into the model",
                    threat.id
                );
            }
        }
    }

    #[test]
    fn no_perimeter_threat_in_the_shipped_model_is_reported_as_mitigated() {
        let model = section_13();
        for id in [
            "T-13.03-sandbox-escape",
            "T-13.03-ambient-credentials",
            "T-13.05-holdout-read",
            "T-13.15-signature-forgery",
            "T-13.19-cross-tenant-access",
        ] {
            let threat = model.threat(id).expect("present in the model");
            assert_ne!(threat.status(), ThreatStatus::Mitigated, "{id}");
        }
    }

    #[test]
    fn the_shipped_coverage_is_six_enforced_fifteen_declared_and_four_unmitigated() {
        let coverage = section_13().coverage();
        assert_eq!(coverage.mitigated, 6, "{}", coverage.summary());
        assert_eq!(coverage.declared_only, 15, "{}", coverage.summary());
        assert_eq!(coverage.unmitigated, 4, "{}", coverage.summary());
    }

    #[test]
    fn the_declared_only_threats_outnumber_the_mitigated_ones() {
        let coverage = section_13().coverage();
        assert!(
            coverage.declared_only + coverage.unmitigated > coverage.mitigated,
            "{}",
            coverage.summary()
        );
        assert_eq!(coverage.total(), section_13().threats.len());
    }

    #[test]
    fn the_only_unanalysed_threat_is_the_grader_feedback_loop() {
        let model = section_13();
        let unanalysed: Vec<String> = model
            .unanalysed()
            .iter()
            .map(|threat| threat.id.clone())
            .collect();
        assert_eq!(unanalysed, vec!["T-13.05-grader-steers-next-trial"]);
    }

    #[test]
    fn every_protected_asset_in_the_blueprint_carries_at_least_one_threat() {
        let model = section_13();
        let universe = [
            Asset::PublisherIdentity,
            Asset::SigningKey,
            Asset::ProviderCredential,
            Asset::HiddenOracle,
            Asset::ResultIntegrity,
            Asset::WorkerFleet,
            Asset::RegistrySupplyChain,
            Asset::PrivateTenantData,
            Asset::ProtectedHealthInformation,
            Asset::UserMachine,
        ];
        assert!(
            model.unthreatened_assets(&universe).is_empty(),
            "{:?}",
            model.unthreatened_assets(&universe)
        );
    }

    #[test]
    fn every_threat_is_mountable_by_at_least_one_modelled_adversary() {
        let model = section_13();
        let unreachable: Vec<&str> = model
            .unreachable_threats()
            .iter()
            .map(|threat| threat.id.as_str())
            .collect();
        assert!(unreachable.is_empty(), "{unreachable:?}");
    }

    #[test]
    fn the_benchmarked_agent_can_mount_the_escape_and_holdout_threats() {
        let model = section_13();
        let reachable: Vec<&str> = model
            .reachable_by("benchmarked-agent")
            .iter()
            .map(|threat| threat.id.as_str())
            .collect();
        assert!(reachable.contains(&"T-13.03-sandbox-escape"), "{reachable:?}");
        assert!(reachable.contains(&"T-13.05-holdout-read"), "{reachable:?}");
        assert!(
            !reachable.contains(&"T-13.15-signature-forgery"),
            "the agent does not control the build"
        );
    }

    #[test]
    fn every_residual_risk_acceptance_names_a_party() {
        section_13()
            .audit_acceptances()
            .expect("no anonymous acceptances in the shipped model");
    }

    #[test]
    fn relying_on_any_perimeter_threat_in_the_shipped_model_is_a_typed_error() {
        let model = section_13();
        let escape = model.threat("T-13.03-sandbox-escape").expect("present");
        assert!(escape.rely().is_err());
        let overclaim = model.threat("T-13.19-isolation-overclaim").expect("present");
        assert!(overclaim.rely().is_ok());
    }

    #[test]
    fn the_model_serialises_and_reloads_without_changing_any_threat_status() {
        let model = section_13();
        let json = serde_json::to_string(&model).expect("serialises");
        let back: ThreatModel = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back.coverage(), model.coverage());
        assert_eq!(back, model);
    }

    #[test]
    fn the_coverage_summary_names_all_three_counts_and_never_a_percentage() {
        let summary = coverage_summary();
        assert!(summary.contains("enforced"), "{summary}");
        assert!(summary.contains("declared-only"), "{summary}");
        assert!(summary.contains("unmitigated"), "{summary}");
        assert!(!summary.contains('%'), "{summary}");
    }

    #[test]
    fn every_threat_cites_the_blueprint_module_it_came_from() {
        for threat in section_13().threats {
            assert!(
                threat.module.starts_with("13."),
                "{} cites {:?}",
                threat.id,
                threat.module
            );
        }
    }
}
