//! The adversarial half of blueprint section 13: threat model, trust boundaries, and the typed
//! refusal to claim a control that does not exist.
//!
//! Implements 13.01 and 13.02 (security architecture and threat model), 13.03 and 13.04 (sandbox
//! and untrusted code), 13.05 (evaluator and grader trust boundary), 13.12 and 13.13 (prompt
//! injection and untrusted content), 13.14 (evaluator attacks, benchmark poisoning and reward
//! hacking), 13.15 (supply chain and artifact security), 13.19 (tenant isolation), 13.20 (audit log
//! transparency and attestation), 13.21 (security testing and red team), 13.22 and 13.23 (incident
//! response and forensics), 13.24 (responsible disclosure), 13.25 (medical research boundary) and
//! 13.26 (dual-use release gates).
//!
//! # This crate is a threat model, not a security control
//!
//! Read this paragraph before anything else, because everything below depends on it.
//! `bioprism-safety` is a library of plain Rust types compiled into its caller's address space. It
//! spawns no process, opens no socket, touches no filesystem, holds no key, and makes no syscall
//! beyond allocation. It **cannot** sandbox a pack, isolate a tenant, verify a signature, stop an
//! egress, revoke a credential, quarantine an artifact or detect an injection.
//!
//! Every one of those is a control section 13 requires and this workspace does not have. The
//! deliberate design decision is that a type here may *model* such a control, and no type here may
//! *claim* one:
//!
//! | The control section 13 asks for | What this crate offers instead |
//! |---|---|
//! | Untrusted code is isolated (13.03, 13.04) | [`threat::Mitigation::DeclaredOnly`]; [`threat::Enforcer`] has no variant naming a runtime enforcer |
//! | The evaluator/agent data diode (13.05) | [`boundary::BoundaryModel::influence_paths`], returning the paths that exist — including one the blueprint does not mention |
//! | Injection detection (13.13) | Nothing. There is no detector, on purpose: [`provenance`] models where content came from and where it ended up |
//! | Signature verification (13.15) | [`supply::SignatureStatus`], one variant, `NotChecked` |
//! | Tenant isolation (13.19) | [`boundary::TenantIsolation`], one variant, `DeclaredOnly` |
//! | Tamper-evident audit (13.20) | A real hash chain that detects an edited record and not a rewritten log |
//! | Containment (13.22, 13.23) | [`incident::ContainmentRequest`]. There is no `ContainmentPerformed` type |
//!
//! [`model::section_13`] is the shipped instance. It carries 25 threats and scores **6 enforced,
//! 15 declared-only, 4 unmitigated** — three numbers that are never added together and never turned
//! into a percentage, because deciding whether a declaration counts would make either answer wrong.
//! Its result is the crate's actual finding: **every one of those 6 is a threat against the
//! platform's own honesty**, not against an attacker. Two tests hold it — one pins the counts, one
//! fails if any perimeter threat is ever marked enforced.
//!
//! # Shape
//!
//! * [`threat`] — assets, adversary capabilities, attack classes, and [`threat::Mitigation`] with
//!   its three states. [`threat::Threat::rely`] is the gate: relying on a declaration is a typed
//!   error, not a judgement call.
//! * [`boundary`] — trust zones and a directed influence graph with channels and
//!   [`boundary::EdgeScope`] on the edges. Unmodelled edges are closed.
//! * [`provenance`] — [`provenance::Authority`] ceilings by [`provenance::Provenance`], no authority
//!   gain through derivation, and [`provenance::InjectionPath`] as the finding.
//! * [`integrity`] — 13.14's witnesses, with [`integrity::IntegrityStatus::Underdetermined`] for a
//!   check nobody could run.
//! * [`supply`] — pins, the two bills of materials, and the unchecked signature.
//! * [`attest`] — [`attest::Statement`] separating observed from asserted, over a hash-linked log.
//! * [`disclosure`] — the red-team corpus and the epoch-driven vulnerability ladder.
//! * [`incident`] — blast radius, [`incident::LineageCompleteness`], and the containment gate.
//! * [`release`] — the medical research boundary and the dual-use gate.
//! * [`model`] — section 13 as data.
//!
//! # What this crate deliberately does not implement
//!
//! This list is the most important documentation in the crate. A missing capability that is stated
//! is a limitation; one that is implied to exist is a lie.
//!
//! **Execution and isolation.** No process spawning, no container, no microVM, no WebAssembly, no
//! seccomp, no capability drop, no user namespace, no AppArmor or SELinux profile, no cgroup, no
//! pid limit, no fork-bomb protection, no `/proc` restriction, no ptrace boundary, no mount, no
//! ephemeral root, no cleanup verification, no worker quarantine. 13.03 and 13.04 in full.
//!
//! **Network.** No egress control, no allowlist, no DNS policy, no proxy, no TLS, no request
//! inspection. 13.09 and 13.10 are not this crate's, and would not be implementable here if they
//! were.
//!
//! **Cryptography and identity.** No hashing beyond `bioprism-ids`' SHA-256 over in-memory bytes;
//! no signing, no verification, no key generation, storage, rotation or revocation, no trust root,
//! no certificate, no authentication, no authorization decision, no secret broker. 13.06, 13.07 and
//! 13.08 belong to `crates/runtime` (05.08); nothing here reimplements or wraps them.
//!
//! **Data classification and privacy.** No labels, no information-flow lattice, no consent, no
//! purpose binding, no residency, no redaction, no de-identification, no small-cell suppression.
//! 13.16, 13.17 and 13.18 are `crates/policy`'s, together with section 36. This crate does not
//! classify anything and must not be used to.
//!
//! **Detection.** No injection detector, no classifier, no heuristic, no pattern list, no anomaly
//! model, no malware scanner, no vulnerability scanner, no secret scanner, no fuzzer, no canary
//! generation. Where section 13 asks for detection, this crate models provenance and coverage and
//! reports which categories nobody claimed.
//!
//! **Storage and orchestration.** No database, no artifact store, no registry, no scheduler, no
//! worker pool, no lease, no job, no cache, no backup, no retention policy, no export.
//! [`attest::AuditLog`] is a `Vec`.
//!
//! **Time.** No clock anywhere. Every lifecycle, timeline and audit record advances on an
//! operator-supplied epoch, as `bioprism-governance` does, so that a sequence is reproducible and
//! is not orderable by whoever compromised the host.
//!
//! **Randomness.** None. Every function here is deterministic.
//!
//! **Process.** No intake queue, no notification, no incident channel, no commander, no postmortem,
//! no bounty, no reviewer identity, no conflict register, no appeal. Where section 13 describes
//! people doing things, this crate holds the record they would fill in and reports which fields are
//! empty.
//!
//! # Where the blueprint asserts a control it does not specify
//!
//! Section 13 is `Planned` throughout, and 67.5% of its lines are boilerplate shared across all 26
//! modules; the distinguishing content runs about 21 lines each. Several of those lines assert a
//! property without saying how it is decided. Named here rather than invented:
//!
//! * **13.01, 13.02** give a "control philosophy" and five "security levels" and never say which
//!   controls are required at which level. [`threat::Mitigation`] therefore carries no level.
//! * **13.04** requires publishing "the supported isolation grade, not just 'sandboxed'" and never
//!   defines the grades. `bioprism_sdk::sandbox::IsolationClass` has three, taken from 13.03's
//!   prose, and they order requests rather than delivered properties.
//! * **13.05** requires "predeclared priority" for contradictory evaluator claims and never says
//!   what predeclares it. Not modelled.
//! * **13.13** gives a four-level instruction precedence and says nothing about a segment with no
//!   declared provenance. [`provenance::Segment`] therefore requires one.
//! * **13.14** asks for "benchmark canaries" without saying what makes a canary detectable, and for
//!   "multi-oracle checks" without saying how disagreement resolves.
//! * **13.15** requires "purpose-separated" signatures without enumerating the purposes.
//! * **13.20** requires "narrow" attestations and gives three examples, not a grammar.
//!   [`attest::AttestationClaim`] closes the set at those three plus the one this crate can witness.
//! * **13.23** requires a "blast-radius query through lineage" and never states what to do when the
//!   query comes back partial. [`incident::LineageCompleteness`] is this crate's answer: a partial
//!   answer cannot support a containment claim.
//! * **13.26** blocks a promotion whose unsafe-action rate rises "beyond thresholds" and states no
//!   threshold. [`release::PromotionCandidate::promote`] refuses any increase and any unmeasured
//!   metric, and its docs say the rule is this crate's rather than the specification's.
//! * **13.26** likewise gives nine risk dimensions and no aggregation rule.
//!   [`release::ReleaseGate::decide`] uses two-high-blocks / one-high-conditions, written in the
//!   open for exactly that reason.
//!
//! # Boundary
//!
//! Research and developer infrastructure. [`release::MedicalBoundary`] refuses clinical outputs
//! unconditionally and has no override, matching `bioprism-onco`'s typed research boundary.

pub mod attest;
pub mod boundary;
pub mod evidence_surveillance_copilot;
pub mod disclosure;
pub mod error;
pub mod incident;
pub mod integrity;
pub mod mechanism_workflow;
pub mod model;
pub mod provenance;
pub mod release;
pub mod supply;
pub mod threat;

pub use error::{ErrorFamily, SafetyError};
pub use evidence_surveillance_copilot::{
    assure_evidence_surveillance, evidence_surveillance_copilot_manifest,
    EvidenceObservation, EvidenceSurveillanceError, EvidenceWatchRequest, QualifiedEvidenceSet,
    CONTRACT_VERSION as EVIDENCE_SURVEILLANCE_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_SURVEILLANCE_COPILOT_FEATURE_ID,
    INPUT_SCHEMA as EVIDENCE_SURVEILLANCE_COPILOT_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EVIDENCE_SURVEILLANCE_COPILOT_OUTPUT_SCHEMA,
};
pub use mechanism_workflow::{
    orchestrate_mechanism_workflow, safety_workflow_manifest, MechanismCandidate,
    MechanismWorkflowError, MechanismWorkflowReceipt, MechanismWorkflowRequest, SafetyDisposition,
    WorkflowEvidenceState, CONTRACT_VERSION as MECHANISM_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as MECHANISM_WORKFLOW_FEATURE_ID,
};
pub use threat::{Mitigation, Threat, ThreatModel, ThreatStatus, Unrepresentable};
