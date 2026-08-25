//! Operational contracts of blueprint §40, as predicates over values.
//!
//! Implements 40.10 (Configuration, Secrets and Feature Flags), 40.34 (Observability, Telemetry and
//! Audit), 40.35 (Performance, Capacity and Load Model), 40.42 (Alpha Acceptance Criteria), the two
//! invariants of 40.39 (Security Threat Model and Hardening) that `bioprism-safety` does not
//! already own, and the single invariant of 40.38 (Deployment Profiles and Infrastructure) that is
//! a predicate rather than a property of infrastructure. Failures are classified under 40.36's
//! taxonomy by way of `bioprism-services`, which owns it.
//!
//! # The first job was a classification, and it changed the answer
//!
//! Thirteen §40 modules were uncovered. `crates/stewardship` did this for §14 and found twelve of
//! eighteen were process rather than code, using a test worth reusing: *is the module's detailed
//! design a set of predicates over an artifact, or a description of what people do?* Applied here,
//! the thirteen split three ways rather than two, and the third bucket is the one worth arguing
//! about.
//!
//! | § | module | verdict |
//! |---|---|---|
//! | 40.10 | Configuration, Secrets and Feature Flags | **code** — [`config`], [`flags`] |
//! | 40.34 | Observability, Telemetry and Audit | **code** — [`telemetry`] |
//! | 40.35 | Performance, Capacity and Load Model | **code** — [`capacity`] |
//! | 40.39 | Security Threat Model and Hardening | **code, partial** — 2 of 5 invariants, [`hardening`] |
//! | 40.42 | Alpha Acceptance Criteria | **code** — [`alpha`] |
//! | 40.38 | Deployment Profiles and Infrastructure | **code, partial** — 1 of 4 invariants, [`config::DeploymentProfile`] |
//! | — | TypeScript SDK Contract | code, **covered by the in-tree dependency-free Fetch client in `typescript/`** |
//! | — | CI/CD and Release Automation | code, **in a workflow file, and its three checkable clauses are owned elsewhere** |
//! | — | Reference Technology Baseline | process |
//! | — | Monorepo and Package Layout | process |
//! | — | First 100 Implementation Tickets | process |
//! | — | Engineering ADR Register and Decision Process | process |
//! | — | Ownership, RACI and Maintainer Boundaries | process |
//!
//! Six cited, seven not. The seven are named by title above and their ids are deliberately **not**
//! written anywhere in this crate in the dotted form `tools/coverage.sh` matches, which is the
//! convention `crates/stewardship` established for the same situation. Writing them down would move
//! a coverage number without moving a capability, and `docs/COVERAGE.md` exists to stop exactly
//! that.
//!
//! ## Why each of the seven is not cited
//!
//! **The reference technology baseline** is a table of choices with reasons: Python 3.12, FastAPI,
//! Typer, Pydantic, DuckDB, OpenTelemetry. There is no predicate in it. It is also the sharpest
//! single finding about §40, because this workspace is Rust with no server, no HTTP and no
//! OpenTelemetry — the "build-ready" section's technology module describes an implementation nobody
//! built, and `docs/ADR-001-language-strategy.md` is where that decision actually lives.
//!
//! **The monorepo and package layout** is a directory tree for a `uv` workspace of Python packages,
//! plus one genuinely checkable sentence: *packages may depend only to the right/downstream
//! according to the graph.* That sentence is 40.04's domain-boundary rule in a second voice, and
//! `bioprism-services` owns 40.04 and has already audited this workspace against it. Implementing
//! it here would produce a second layering register.
//!
//! **The first 100 implementation tickets** is a ticket table. It is process by any reading, and it
//! is also the least machine-readable document in the section; see the not-build-ready list below.
//!
//! **The engineering ADR register** is the closest call of the seven, because its trigger list —
//! create an ADR when changing canonical semantics, storage truth, public schemas, replay fidelity,
//! release gates or dependency direction — reads like a predicate over a change. Everything else in
//! it is a workflow: minimum fields, review dates, supersession links, and a sentence deferring to
//! another section as authoritative. The one predicate is `bioprism-governance`'s already: a change
//! that moves an artifact digest is breaking, and that crate refuses an author's contrary claim
//! rather than asking for a document. A `RequiresAdr` type here would assert that an ADR was
//! written.
//!
//! **Ownership, RACI and maintainer boundaries** is a table of who reviews what, with one rule that
//! is not: *no single maintainer may both author hidden evaluator state and approve release claims
//! for the same benchmark family without independent review.* That is separation of duties, and
//! `crates/stewardship` and `bioprism-registry` both already hold it. Not reimplemented.
//!
//! **The TypeScript SDK contract** is genuinely code-bearing and the code is TypeScript. Its first
//! invariant — generated code is pinned to the API schema hash — presupposes an OpenAPI artifact,
//! and `crates/services` established that this workspace has no server and no transport because
//! 40.03 requires the same services to run in-process. There is nothing to pin to. Writing a Rust
//! type named after a browser client would be fiction.
//!
//! **CI/CD and release automation** is code that lives in a workflow file, and a workflow runner is
//! not a library. Its four invariants decompose into work three crates already do: schema and
//! migration checks before publication are `bioprism-governance`'s, quality gates are
//! `bioprism-infra`'s, and release gates and signing are `bioprism-safety`'s — where signing comes
//! out as `SignatureStatus::NotChecked`, one variant, because no key material exists.
//!
//! The expectation this crate was given was that five modules would be code-bearing: 40.10, 40.34,
//! 40.35, 40.39 and 40.42. That reading was right, and 40.38 is one more: its first invariant,
//! *deployment changes providers, not object semantics*, is a predicate over a configuration layer
//! and is enforced at [`config::ConfigStack::resolve`], while its other three need infrastructure.
//!
//! # The axis the crate is built on
//!
//! One distinction runs through [`config`], [`flags`] and [`config::DeploymentProfile`]:
//! [`config::Influence`] records whether a value participates in what a computation *emits* or only
//! in how it gets there. From it follow three rules that are shapes rather than checks:
//!
//! * a **secret** may not be `Emitted`, because the artifact digest would then depend on a value no
//!   result bundle may record — [`config::SettingSpec::secret`] fixes the influence and no
//!   constructor offers the alternative;
//! * a **flag** that changes what a compile emits is an artifact version, not a runtime toggle —
//!   [`flags::Flag::toggle`] and [`flags::Flag::variant`] are the only two constructors, and the
//!   second requires a version string;
//! * a **deployment profile** may bind only operational settings, so switching profiles provably
//!   cannot move [`config::EffectiveConfig::emitted_fingerprint`].
//!
//! This is `bioprism-governance`'s digest rule in a different currency. That crate is not a
//! dependency — it owns schema evolution, and a second classifier is the thing this workspace must
//! not grow — so [`flags::affects_emitted_artifact`] restates the predicate over flag declarations,
//! says so at its definition, and mirrors each of `affects_digest`'s asymmetries so the two can be
//! read against each other.
//!
//! # The alpha acceptance result, which is the crate's actual finding
//!
//! [`alpha`] encodes 40.42's fourteen conditions as predicates and runs them against this
//! workspace. The answer is **0 met, 2 refuted, 12 unverifiable from a library**, and it ships as
//! it is:
//!
//! * **Refuted — a signed result bundle.** `bioprism_safety::supply::SignatureStatus` has exactly
//!   one variant, `NotChecked`. The checksum half of the condition holds; the signature half cannot,
//!   and a test in [`alpha`] matches the enum with a single arm so the refutation stops compiling if
//!   a `Verified` variant is ever added.
//! * **Refuted — explaining a failure through the graph UI.** No workspace member is a TypeScript
//!   package; every member is a Rust crate under `crates/`, read out of the root manifest which
//!   [`alpha`] embeds at compile time.
//! * **Unverifiable — the other twelve.** Each is of the form *an independent engineer can …* and
//!   needs a checkout, a shell and a running demo. None is a defect in the workspace and none is a
//!   pass.
//!
//! Zero met is not a failure of the encoding. An observation available to a library can refute a
//! criterion and almost never confirm one, so [`alpha::Finding::new`] requires an entailing basis
//! for any verdict except `Unverifiable`, and an author's say-so is not one. The three counts are
//! never added into a percentage, for the reason `bioprism_safety::threat::Coverage` gives.
//!
//! # How much of §40 is template
//!
//! `crates/services` measured §40 at 36.8% and called it the lowest-content section in the
//! blueprint. Measured again here, independently, with the method stated so it can be disagreed
//! with: **a line counts as template when its exact text, after trimming trailing whitespace,
//! appears in at least 23 of §40's 45 modules**, blank lines excluded, the section index excluded
//! from both the corpus and the threshold.
//!
//! | | non-empty lines | shared | template |
//! |---|---:|---:|---:|
//! | §40 entire (45 modules) | 3806 | 1400 | **36.8%** |
//! | these thirteen | 1022 | 328 | **32.1%** |
//! | the seven of them in the progressive-disclosure template | 690 | 273 | **39.6%** |
//! | the six of them hand-written | 332 | 55 | **16.6%** |
//! | the other 32 modules | 2784 | 1072 | 38.5% |
//!
//! 36.8% reproduces `bioprism_services::SECTION_40_BOILERPLATE_PERCENT` to the tenth, and a test in
//! `tests/contracts.rs` asserts the two constants are equal so that a later change to either method
//! shows up as a failure rather than as two numbers in two files.
//!
//! **Sensitivity, and the thing that has to be stated.** The measurement is sensitive to one
//! decision and insensitive to the other:
//!
//! | variation | §40 | these thirteen |
//! |---|---:|---:|
//! | as published above (YAML front matter counted, index excluded) | 36.8% | 32.1% |
//! | YAML front matter **excluded** | 32.0% | 26.7% |
//! | section index included in the corpus | 36.6% | 32.1% |
//!
//! So counting the front matter is worth **4.8 points** on §40 and 5.4 on these thirteen, and
//! including the index is worth 0.2. Front matter is counted here, and the reason is that its seven
//! keys are genuinely repeated text carrying no module-specific content beyond `title` and
//! `module_id` — but a reader who thinks a machine-readable header is not boilerplate should read
//! the second row, and that row is why two agents measuring the same section can differ by five
//! points without either being wrong.
//!
//! **These thirteen are below the section average, and the average hides the split.** 32.1% looks
//! like more content than §40 as a whole until the thirteen are separated: the seven written in the
//! progressive-disclosure template sit at 39.6%, *above* the section average, and the six
//! hand-written ones sit at 16.6%. The 32.1% is an artefact of the mix. Within the seven, per-module
//! template runs 37.9% to 41.5% — a 3.6-point spread across seven documents, which is what a
//! template looks like — and roughly 57 to 64 lines per module are unique to it. Those lines are
//! four invariant sentences, four input names, four output names, five failure phrases and an
//! interface list with no signatures.
//!
//! # Where a build-ready module turns out not to be build-ready
//!
//! Stated so a reader can disagree rather than discover.
//!
//! 1. **40.42 is an acceptance gate that nothing can run.** Fourteen conditions, every one phrased
//!    as what a person can do, no command, no artifact, no threshold, and no machine-readable form.
//!    Encoding it produced twelve `Unverifiable` verdicts, which is a property of the specification
//!    and not of the workspace.
//! 2. **40.42's tenth condition is two conditions joined by a slash.** "Verify a signed/checksummed
//!    result bundle" is satisfiable in its second half today and unsatisfiable in its first half in
//!    any workspace without key material. A conjunction written as an alternation cannot be scored.
//! 3. **The seven template-form modules specify no types.** 40.10 lists "Settings model",
//!    "SecretBroker", "FeatureGate" and "config CLI" under Interfaces; 40.34 lists "EventSink" and
//!    "OTel bridge"; 40.35 lists "benchmark CLI" and "profiling hooks". Not one carries a signature,
//!    a field, a cardinality or a type. Every decision in this crate — the precedence order, what a
//!    secret reference is as a value, what a flag version is, what makes a query bounded — is absent
//!    from the modules that are supposed to be build-ready, and each is documented at its definition
//!    as this crate's rather than the specification's.
//! 4. **40.10's stated invariants imply an unstated one that its own verification plan needs.**
//!    *Secrets are referenced, not serialized* plus *result bundles include nonsecret effective
//!    settings* jointly forbid a secret from participating in an emitted artifact, and without that
//!    third rule the module's own "reproducible fingerprint" verification item is unsatisfiable. The
//!    module never states it; [`config`] does, and says it is inferring.
//! 5. **The dependency lists mix two numbering schemes with no marker.** 40.34 depends on "09 event
//!    ledger", "13 security" and "03 service graph": the first and third are §40-internal, the
//!    second is section 13, and 40.13 is the CLI command tree. 40.38 is worse — "12
//!    infrastructure", "36 security", "32 conformance" — where 40.12 is identity and authorization
//!    and 40.36 is the error taxonomy, so two of its three dependencies resolve to the wrong module
//!    if read locally. 40.39 happens to disambiguate by writing "13 security section" in words.
//!    `crates/services` found that 40.03's process graph contains a cycle when read literally; this
//!    is the same class of defect in the citation apparatus rather than in a diagram.
//! 6. **40.38's four invariants are one predicate and three infrastructure properties.** Only
//!    *deployment changes providers, not object semantics* can be decided without a deployment. The
//!    other three — local mode needs no external service, protected mode denies egress, hosted
//!    metadata does not imply artifact access — are statements about a running system, and a type
//!    asserting any of them would be claiming a control.
//! 7. **The ticket table cites contracts in three vocabularies.** Its "Primary contract" column
//!    holds dotted §40 ids, at least one dotted id from a different section, and bare directory
//!    names such as `03_CORE_SPECIFICATIONS`. No tool can resolve that column, which is most of what
//!    a ticket-to-contract mapping is for.
//! 8. **The section's own technology and layout modules describe a different implementation.** A
//!    Python `uv` workspace with FastAPI and Typer, against a Rust Cargo workspace with no server.
//!    That is a legitimate decision recorded in `docs/ADR-001-language-strategy.md`; what is not
//!    legitimate is a section labelled build-ready whose baseline nobody built to.
//! 9. **The ADR register defers its own authority.** "The existing `20_ARCHITECTURE_DECISIONS`
//!    corpus remains authoritative" — a build-ready module that is a pointer.
//!
//! # What is deliberately not implemented
//!
//! This list is the most important documentation in the crate. A missing capability that is stated
//! is a limitation; one that is implied to exist is a lie.
//!
//! **No server, no transport, no network, no socket, no HTTP, no OpenTelemetry, no exporter.**
//! Nothing here opens a connection or serialises a response to a client. 40.34's "export to
//! configured backend" step ends at a `Vec`, and its `telemetry backend down` failure is
//! consequently not modelled, because nothing here can be down. No type in this crate should be
//! read as serving traffic.
//!
//! **No infrastructure.** No deployment, no provider binding, no health check, no readiness probe,
//! no container, no image, no cluster, no CI runner, no workflow, no release pipeline, no artifact
//! publication. [`config::DeploymentProfile`] is a configuration layer with one rule attached; it
//! deploys nothing.
//!
//! **No filesystem, no environment, no vault, no secret resolution.** A [`config::Source`] is a map
//! somebody built. [`config::SecretRef`] is a locator, [`config::SecretLease`] carries no value and
//! has no `resolve`, and there is no secret *value* type anywhere in this crate. The single
//! exception to "no filesystem" is [`alpha`]'s compile-time `include_str!` of the root manifest,
//! which is stated at its definition and is the only file this crate reads.
//!
//! **No clock, no timer, no wall-clock latency, no randomness.** Every ordering advances on a
//! caller-supplied `bioprism_infra::Epoch`, as `bioprism-governance`, `bioprism-ledger` and
//! `bioprism-infra` already do. [`capacity`] counts abstract work units per epoch and measures no
//! duration; [`hardening::Effect::Clock`] exists so that reading a clock is a declarable effect,
//! not so that anything here reads one.
//!
//! **No measurement, no benchmark, no load generator, no profiler.** [`capacity::Basis::Measured`]
//! records a caller's claim about where a number came from and does not verify it. Nothing here
//! executes a workload.
//!
//! **No second threat model, no second classifier, no second layering register, no second
//! leaderboard.** See the crate boundaries below.
//!
//! **No enforcement of anything.** [`hardening::EffectDeclaration::permits`] is a predicate over
//! two values; a caller that never asks it is unaffected by it. It hooks no syscall, wraps no
//! socket, resolves no symlink and observes nothing a process does. This is the same standing
//! `bioprism_safety::Mitigation::DeclaredOnly` has, and it is labelled the same way.
//!
//! # What the neighbours own, and are not reimplemented here
//!
//! * **`bioprism-safety`** owns the threat model, trust boundaries, provenance, supply chain and
//!   attestation. [`hardening::coverage`] cites it for three of 40.39's five invariants and a test
//!   asserts that nothing in this crate moves safety's own count of 6 enforced, 15 declared-only, 4
//!   unmitigated. [`telemetry::audit_statement`] uses safety's `Statement`, and returns `Asserted`
//!   rather than `Observed` because safety's `Observation` enum is closed over safety's own
//!   computations and none of its five variants describes an operational metric — a finding about
//!   the boundary, recorded rather than worked around.
//! * **`bioprism-services`** owns the 40.36 error taxonomy and the nine §40 service contracts.
//!   [`error::OpsError`] classifies into `ErrorClass`, `Retryability` and `Invalidates` rather than
//!   growing a taxonomy that would drift.
//! * **`bioprism-governance`** owns schema evolution and the rule that a digest-moving change
//!   cannot be compatible. Restated for flags, cited, not imported.
//! * **`bioprism-infra`** owns storage, caching, invalidation, quality gates, quota and the
//!   `Epoch`. [`capacity`] stores nothing and evicts nothing.
//! * **`bioprism-scale`** owns effective size and the cost forecast that cannot drop it.
//!   [`capacity`] is the same shape in a different unit and shares no code.
//! * **`bioprism-scope`** owns the typed scope base. [`telemetry::RedactionPolicy`] is keyed by
//!   `ScopeClass` so the workspace has one classification vocabulary rather than two.
//! * **`bioprism-cli`** owns command surfaces. 40.10 names a `config` CLI and there is none here.
//!
//! # Boundary
//!
//! Research and developer infrastructure. Nothing in this crate diagnoses, triages, enrols or
//! deploys, and no predicate here authorises crossing a data-use, consent, privacy or clinical
//! boundary.

pub mod alpha;
pub mod capacity;
pub mod config;
pub mod error;
pub mod flags;
pub mod hardening;
pub mod knowledge_representation_assurance;
pub mod retrieval_assurance;
pub mod telemetry;

pub use alpha::{AlphaSummary, Basis, Criterion, Finding, Verdict};
pub use capacity::{
    ArtifactHandling, Assumption, Bound, CapacityModel, CapacityProjection, Concession,
    DegradationPlan, Demand, Operation, Qualified, Saturation, Workload,
};
pub use config::{
    Binding, ConfigStack, DeploymentProfile, EffectiveConfig, Influence, Layer, Resolution, Schema,
    SecretLease, SecretRef, SecretSource, SettingKey, SettingKind, SettingSpec, SettingValue,
    Source, ValueType,
};
pub use error::OpsError;
pub use flags::{
    affects_emitted_artifact, classify, DecisionLog, Flag, FlagChange, FlagChangeClass,
    FlagDecision, FlagId, FlagPin, FlagRegistry, FlagShape, FlagVerdict, PinnedRun,
};
pub use hardening::{
    audit_credentials, coverage, AmbientFinding, ControlCoverage, ControlOwner, CredentialAudit,
    Effect, EffectDeclaration,
};
pub use knowledge_representation_assurance::{
    assure_knowledge_representation, KnowledgeAssuranceDisposition, KnowledgeAssuranceError,
    KnowledgeRepresentationAssuranceReceipt, KnowledgeRepresentationAssuranceRequest,
    CONTRACT_VERSION as KNOWLEDGE_REPRESENTATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
};
pub use retrieval_assurance::{
    assure_retrieval, retrieval_assurance_manifest, EvidenceSynthesisReceipt,
    RetrievalAssuranceError, RetrievalCandidate, RetrievalDisposition, ScopedRetrievalQuery,
    CONTRACT_VERSION as RETRIEVAL_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_ASSURANCE_FEATURE_ID,
};
pub use telemetry::{
    audit_statement, Derivation, DomainEvent, ExportBatch, Field, LabelBudget, MetricDefinition,
    MetricValue, Observations, Projected, RedactionPolicy, Sample, Sampling, SemanticLoss,
    SignalId, TelemetryRecord, TraceId, Treatment,
};

/// The share of §40's 45 modules that is template rather than content.
///
/// A line counts as template when its exact text appears in at least 23 of the 45, blank lines
/// excluded, YAML front matter counted, the section index excluded. Measured independently of
/// `bioprism_services::SECTION_40_BOILERPLATE_PERCENT` and equal to it; see the crate docs for how
/// sensitive the number is to each of those choices.
pub const SECTION_40_BOILERPLATE_PERCENT: f64 = 36.8;

/// The same measure over the thirteen modules this crate was given.
///
/// Below the section average, and the average conceals the split: see
/// [`TEMPLATE_FORM_BOILERPLATE_PERCENT`] and [`HAND_WRITTEN_BOILERPLATE_PERCENT`].
pub const THESE_THIRTEEN_BOILERPLATE_PERCENT: f64 = 32.1;

/// The seven of the thirteen written in §40's progressive-disclosure template.
pub const TEMPLATE_FORM_BOILERPLATE_PERCENT: f64 = 39.6;

/// The six of the thirteen written by hand.
pub const HAND_WRITTEN_BOILERPLATE_PERCENT: f64 = 16.6;

/// The headline measure with YAML front matter excluded from the corpus.
///
/// Published beside the headline because the gap between the two conventions is larger than the gap
/// between §40 and most other sections, and an agent that quietly picked one would be reporting a
/// number nobody could reproduce.
pub const SECTION_40_BOILERPLATE_PERCENT_WITHOUT_FRONT_MATTER: f64 = 32.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thirteen_look_denser_in_content_than_the_section_only_because_of_their_mix() {
        const { assert!(THESE_THIRTEEN_BOILERPLATE_PERCENT < SECTION_40_BOILERPLATE_PERCENT) };
        const {
            assert!(
                TEMPLATE_FORM_BOILERPLATE_PERCENT > SECTION_40_BOILERPLATE_PERCENT,
                "the seven template-form modules carry more boilerplate than the section as a \
                 whole, and the thirteen-module average conceals it"
            )
        };
        const { assert!(HAND_WRITTEN_BOILERPLATE_PERCENT < SECTION_40_BOILERPLATE_PERCENT / 2.0) };

        let published = [
            SECTION_40_BOILERPLATE_PERCENT,
            THESE_THIRTEEN_BOILERPLATE_PERCENT,
            TEMPLATE_FORM_BOILERPLATE_PERCENT,
            HAND_WRITTEN_BOILERPLATE_PERCENT,
        ];
        assert!(published.iter().all(|share| (0.0..=100.0).contains(share)));
    }

    #[test]
    fn counting_yaml_front_matter_is_worth_almost_five_points_and_has_to_be_stated() {
        let delta =
            SECTION_40_BOILERPLATE_PERCENT - SECTION_40_BOILERPLATE_PERCENT_WITHOUT_FRONT_MATTER;
        assert!(
            (4.7..=4.9).contains(&delta),
            "the front-matter convention moved the headline by {delta} points; two agents \
             disagreeing by that much are using two methods, not finding two facts"
        );
    }

    #[test]
    fn the_crate_re_exports_every_type_a_caller_needs_to_state_an_acceptance_verdict() {
        let finding = alpha::report()
            .into_iter()
            .find(|finding| finding.criterion == Criterion::ExplainFailureViaGraphUiAndTable)
            .expect("in the report");
        assert_eq!(finding.verdict, Verdict::Refuted);
        assert!(matches!(finding.basis, Basis::WorkspaceManifest));
    }

    #[test]
    fn the_seven_process_modules_are_named_in_prose_and_their_ids_appear_nowhere_in_this_crate() {
        let sources = [
            include_str!("lib.rs"),
            include_str!("alpha.rs"),
            include_str!("capacity.rs"),
            include_str!("config.rs"),
            include_str!("error.rs"),
            include_str!("flags.rs"),
            include_str!("hardening.rs"),
            include_str!("telemetry.rs"),
        ];
        for number in [1u8, 2, 15, 40, 41, 43, 45] {
            let id = format!("40.{number:02}");
            for source in sources {
                assert!(
                    !source.contains(&id),
                    "{id} is a process module for this crate's purposes, and writing its id in \
                     dotted form would move a coverage number without moving a capability"
                );
            }
        }
    }
}
