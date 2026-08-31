//! The alpha acceptance criteria as predicates, and the result of running them on this workspace.
//!
//! Implements blueprint 40.42 (Alpha Acceptance Criteria).
//!
//! # The module is a measurement, and the measurement disagrees with us
//!
//! 40.42 lists fourteen conditions, each of the form *an independent engineer can …*, and ends with
//! the sentence that makes it worth encoding: **no star count, model score, or generated-instance
//! count substitutes for these conditions.** A checklist in prose is checked by whoever is keenest
//! for it to pass, so the fourteen are written here as a closed enumeration with a typed verdict
//! and a typed basis, and [`report`] is the answer for the workspace as it stands:
//!
//! | verdict | count |
//! |---|---:|
//! | met | **0** |
//! | refuted | **2** |
//! | unverifiable from here | **12** |
//!
//! Three numbers, never added into a percentage, for the reason
//! `bioprism_safety::threat::Coverage` gives: deciding whether an unverifiable condition counts
//! would make either answer wrong.
//!
//! # The asymmetry that makes those numbers what they are
//!
//! This crate is a library. It has no filesystem, no shell, no network and no way to run a demo,
//! and the only artifacts it can see are the Rust types it links against and the workspace manifest
//! it embeds at compile time. Against that, an observation can usually **refute** a criterion and
//! almost never **confirm** one. "No workspace member is a TypeScript package" entails that nobody
//! can explain a failure through a graph UI; "`crates/fiber` exists" entails nothing about whether
//! a capsule compiles.
//!
//! So [`Finding::new`] enforces one rule: a verdict other than [`Verdict::Unverifiable`] requires a
//! [`Basis`] that entails it, and an author's say-so is not such a basis
//! ([`OpsError::AssertedAcceptance`]). Zero met is not a defect in the encoding. It is what the
//! encoding is for.
//!
//! # What "entails" means here, exactly
//!
//! Two bases entail, and both are checkable by somebody who distrusts this file:
//!
//! * [`Basis::LinkedType`] — a property of a type in a crate this one depends on, exercised by a
//!   test in this module. The signing refutation is the sharp case: the test matches
//!   `bioprism_safety::supply::SignatureStatus` with a single arm, so if a `Verified` variant is
//!   ever added the match stops being exhaustive and the test stops compiling. The refutation
//!   cannot go stale silently.
//! * [`Basis::WorkspaceManifest`] — read from the root `Cargo.toml`, which this module embeds with
//!   `include_str!`. That is the only file this crate reads, it is read at compile time, and
//!   [`workspace_members`] is the whole of what is extracted from it.
//!
//! Two do not: [`Basis::Author`], which is somebody talking, and [`Basis::NoObserver`], which is
//! this crate saying it cannot see.
//!
//! # Necessary is not sufficient, and the crate says which it is checking
//!
//! [`Criterion::necessary_members`] names the workspace crates without which a criterion is
//! certainly false. Their presence proves nothing and their absence proves the negative, which is
//! the only direction a manifest can argue in. Thirteen of the fourteen have every necessary member
//! present; the fourteenth is the graph UI, and that is one of the two refutations.
//!
//! # What is deliberately not implemented
//!
//! * **No test runner, no demo, no fixtures, no execution of anything.** Twelve of the fourteen
//!   conditions require running a workspace from a clean checkout, and a library that reported
//!   them met would be reporting somebody's intention.
//! * **No filesystem beyond one compile-time `include_str!` of the root manifest.** No walking of
//!   `crates/`, no reading of `.github/`, no parsing of source. In particular this module does
//!   *not* look at the CI workflow, so criterion 11 stays unverifiable even though a workflow file
//!   exists — its existence is not evidence that the minimized regression reruns offline.
//! * **No TOML parser.** [`workspace_members`] scans the one array it needs. A general parser would
//!   be a dependency this workspace cannot add offline, and the failure mode of the scan — reading
//!   the wrong array — is caught by a test that pins the member count and a known member.
//! * **No scoring, no weighting, no percentage, no pass/fail for the alpha as a whole.** 40.42 is a
//!   conjunction; a conjunction with twelve unknown terms has no value, and printing one anyway is
//!   what the module's closing sentence forbids.

use crate::error::OpsError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The root workspace manifest, read once at compile time.
///
/// The only file this crate reads. See the module docs for why it is the only one.
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

/// The workspace member paths, in manifest order.
///
/// Scans the `members` array of the embedded manifest. Every entry is a path such as
/// `crates/fiber`; nothing here opens a directory or checks that the path exists, because a member
/// the manifest lists and the disk lacks would fail the build long before this runs.
pub fn workspace_members() -> Vec<&'static str> {
    let mut members = Vec::new();
    let Some(start) = WORKSPACE_MANIFEST.find("members") else {
        return members;
    };
    let tail = &WORKSPACE_MANIFEST[start..];
    let Some(open) = tail.find('[') else {
        return members;
    };
    let Some(close) = tail.find(']') else {
        return members;
    };
    let body = &tail[open + 1..close];

    let bytes = body.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let from = index + 1;
            let Some(offset) = body[from..].find('"') else {
                break;
            };
            members.push(&body[from..from + offset]);
            index = from + offset + 1;
        } else {
            index += 1;
        }
    }
    members
}

/// Whether the workspace has a member at the given path.
pub fn has_member(path: &str) -> bool {
    workspace_members().contains(&path)
}

/// The fourteen conditions of 40.42, in the order the module states them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criterion {
    CleanCheckoutDemo,
    BuildWorldFromPinnedArtifacts,
    VerifyPartitionsAndTemporalFirewall,
    InspectGraphAndSpecimenLineage,
    CompileCapsuleAndOmissionArtifacts,
    ExecuteBothArchitecturesFromOneCell,
    ReproduceDeterministicOracleResults,
    LocaliseDivergenceAndReplayMinimised,
    SixDescendantsFromThreeMutationClasses,
    VerifySignedResultBundle,
    RerunMinimisedRegressionOfflineInCi,
    ExplainFailureViaGraphUiAndTable,
    DemonstrateNoLeakage,
    ReadLimitations,
}

impl Criterion {
    pub const ALL: [Criterion; 14] = [
        Criterion::CleanCheckoutDemo,
        Criterion::BuildWorldFromPinnedArtifacts,
        Criterion::VerifyPartitionsAndTemporalFirewall,
        Criterion::InspectGraphAndSpecimenLineage,
        Criterion::CompileCapsuleAndOmissionArtifacts,
        Criterion::ExecuteBothArchitecturesFromOneCell,
        Criterion::ReproduceDeterministicOracleResults,
        Criterion::LocaliseDivergenceAndReplayMinimised,
        Criterion::SixDescendantsFromThreeMutationClasses,
        Criterion::VerifySignedResultBundle,
        Criterion::RerunMinimisedRegressionOfflineInCi,
        Criterion::ExplainFailureViaGraphUiAndTable,
        Criterion::DemonstrateNoLeakage,
        Criterion::ReadLimitations,
    ];

    /// Position in 40.42's numbered list, from 1.
    pub fn index(self) -> usize {
        Criterion::ALL
            .iter()
            .position(|criterion| *criterion == self)
            .expect("every criterion is in ALL")
            + 1
    }

    /// The condition, in the blueprint's own words.
    pub fn statement(self) -> &'static str {
        match self {
            Criterion::CleanCheckoutDemo => {
                "clone the repository and run the demo without external services"
            }
            Criterion::BuildWorldFromPinnedArtifacts => {
                "build the radiogenomic integrity world from pinned artifacts"
            }
            Criterion::VerifyPartitionsAndTemporalFirewall => {
                "verify public/agent/evaluator partitions and temporal firewall"
            }
            Criterion::InspectGraphAndSpecimenLineage => {
                "inspect the biological graph and exact specimen/time lineage"
            }
            Criterion::CompileCapsuleAndOmissionArtifacts => {
                "compile a BioContext Capsule and omission/sufficiency artifacts"
            }
            Criterion::ExecuteBothArchitecturesFromOneCell => {
                "execute both full-context and graph-compiled architectures from the same cell"
            }
            Criterion::ReproduceDeterministicOracleResults => {
                "reproduce deterministic leakage/split oracle results"
            }
            Criterion::LocaliseDivergenceAndReplayMinimised => {
                "localize one first causal divergence and replay a minimized cell"
            }
            Criterion::SixDescendantsFromThreeMutationClasses => {
                "generate at least six validated descendants from three mutation classes"
            }
            Criterion::VerifySignedResultBundle => "verify a signed/checksummed result bundle",
            Criterion::RerunMinimisedRegressionOfflineInCi => {
                "rerun the minimized regression in CI without network access"
            }
            Criterion::ExplainFailureViaGraphUiAndTable => {
                "use the graph UI and table fallback to explain the failure"
            }
            Criterion::DemonstrateNoLeakage => {
                "demonstrate no hidden-label, future, secret, or controlled-data leakage"
            }
            Criterion::ReadLimitations => {
                "read limitations clearly stating research-only scope and unsupported claims"
            }
        }
    }

    /// Workspace members without which the criterion is certainly false.
    ///
    /// A necessary condition only. Presence argues nothing; absence refutes. The graph UI is the
    /// interesting entry: §40's reference technology baseline puts the graph UI in TypeScript and
    /// React, and its monorepo layout puts it at `apps/web`, so the member that would satisfy it
    /// cannot be a Rust crate under `crates/` at all — which is why
    /// [`Criterion::ExplainFailureViaGraphUiAndTable`] names a path no member can have. Neither of
    /// those two blueprint modules is cited by id anywhere in this crate; see the crate docs for
    /// why.
    pub fn necessary_members(self) -> &'static [&'static str] {
        match self {
            Criterion::CleanCheckoutDemo => &["crates/cli", "crates/examples"],
            Criterion::BuildWorldFromPinnedArtifacts => &["crates/worldgen", "crates/packs"],
            Criterion::VerifyPartitionsAndTemporalFirewall => &["crates/safety", "crates/scope"],
            Criterion::InspectGraphAndSpecimenLineage => &["crates/graph", "crates/world"],
            Criterion::CompileCapsuleAndOmissionArtifacts => &["crates/fiber", "crates/section"],
            Criterion::ExecuteBothArchitecturesFromOneCell => &["crates/prism", "crates/baseline"],
            Criterion::ReproduceDeterministicOracleResults => &["crates/oracle", "crates/ids"],
            Criterion::LocaliseDivergenceAndReplayMinimised => &["crates/trace", "crates/prism"],
            Criterion::SixDescendantsFromThreeMutationClasses => &["crates/mutation"],
            Criterion::VerifySignedResultBundle => &["crates/bundle", "crates/safety"],
            Criterion::RerunMinimisedRegressionOfflineInCi => &["crates/conformance"],
            Criterion::ExplainFailureViaGraphUiAndTable => &["apps/web"],
            Criterion::DemonstrateNoLeakage => &["crates/lab", "crates/policy"],
            Criterion::ReadLimitations => &["crates/onco"],
        }
    }

    /// The necessary members the manifest does not list.
    pub fn missing_members(self) -> Vec<&'static str> {
        self.necessary_members()
            .iter()
            .copied()
            .filter(|member| !has_member(member))
            .collect()
    }
}

impl fmt::Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "40.42.{} ({})", self.index(), self.statement())
    }
}

/// How a verdict was established.
///
/// `Serialize` and not `Deserialize`, like [`Finding`]: a basis reconstructed from a document
/// somebody wrote is an author's say-so wearing the shape of an observation, which is the one thing
/// this module exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// A property of a linked type, exercised by a test in this module.
    LinkedType {
        krate: &'static str,
        item: &'static str,
    },
    /// Read from the embedded root manifest.
    WorkspaceManifest,
    /// Somebody said so. Never supports a verdict.
    Author { who: &'static str },
    /// Nothing in a library can see it, and `because` says what would be needed.
    NoObserver { because: &'static str },
}

impl Basis {
    /// Whether this basis can carry a verdict other than [`Verdict::Unverifiable`].
    pub fn entails(&self) -> bool {
        matches!(self, Basis::LinkedType { .. } | Basis::WorkspaceManifest)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Basis::LinkedType { .. } => "linked_type",
            Basis::WorkspaceManifest => "workspace_manifest",
            Basis::Author { .. } => "author",
            Basis::NoObserver { .. } => "no_observer",
        }
    }
}

/// What this crate can say about one criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// An observation entails the condition holds. Rare, and currently never reached.
    Met,
    /// An observation entails the condition does not hold.
    Refuted,
    /// Nothing here can decide it. Not a failure of the workspace and not a pass.
    Unverifiable,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Met => "met",
            Verdict::Refuted => "refuted",
            Verdict::Unverifiable => "unverifiable",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One criterion, decided or explicitly not.
///
/// `Serialize` only. [`Finding::new`] is the sole route in, and a `Finding` that could be
/// deserialized would let a document declare a criterion met without any basis at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub criterion: Criterion,
    pub verdict: Verdict,
    pub basis: Basis,
    pub detail: String,
}

impl Finding {
    /// The only constructor, and it refuses a verdict its basis cannot carry.
    pub fn new(
        criterion: Criterion,
        verdict: Verdict,
        basis: Basis,
        detail: impl Into<String>,
    ) -> Result<Self, OpsError> {
        if verdict != Verdict::Unverifiable && !basis.entails() {
            return Err(OpsError::AssertedAcceptance {
                criterion: criterion.to_string(),
                basis: basis.as_str().to_string(),
            });
        }
        Ok(Finding {
            criterion,
            verdict,
            basis,
            detail: detail.into(),
        })
    }
}

/// Counts, never a percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AlphaSummary {
    pub met: usize,
    pub refuted: usize,
    pub unverifiable: usize,
}

impl AlphaSummary {
    pub fn total(&self) -> usize {
        self.met + self.refuted + self.unverifiable
    }
}

impl fmt::Display for AlphaSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} met, {} refuted, {} unverifiable from a library (of {})",
            self.met,
            self.refuted,
            self.unverifiable,
            self.total()
        )
    }
}

/// Runs the fourteen criteria against this workspace.
///
/// The verdicts are recomputed on every call from the embedded manifest and from properties of the
/// linked crates, so a change to either moves the report rather than leaving a stale constant
/// behind.
pub fn report() -> Vec<Finding> {
    Criterion::ALL
        .iter()
        .map(|criterion| decide(*criterion))
        .collect()
}

/// The report as three counts.
pub fn summary() -> AlphaSummary {
    let mut summary = AlphaSummary::default();
    for finding in report() {
        match finding.verdict {
            Verdict::Met => summary.met += 1,
            Verdict::Refuted => summary.refuted += 1,
            Verdict::Unverifiable => summary.unverifiable += 1,
        }
    }
    summary
}

fn decide(criterion: Criterion) -> Finding {
    let missing = criterion.missing_members();
    if !missing.is_empty() {
        return Finding::new(
            criterion,
            Verdict::Refuted,
            Basis::WorkspaceManifest,
            format!(
                "the workspace manifest lists no {}, which the condition cannot hold without; \
                 every member is a Rust crate under crates/, and the section's reference \
                 technology baseline puts this one in TypeScript and React",
                missing.join(" and ")
            ),
        )
        .expect("the manifest entails a refutation");
    }

    match criterion {
        Criterion::VerifySignedResultBundle => Finding::new(
            criterion,
            Verdict::Refuted,
            Basis::LinkedType {
                krate: "bioprism-safety",
                item: "supply::SignatureStatus",
            },
            "the checksum half holds — bioprism_ids::ContentHash verifies canonical bytes — and \
             the signature half cannot: SignatureStatus has exactly one variant, NotChecked, and \
             there is no key material anywhere in this workspace, so nothing can verify a \
             signature and no type can record that one was verified",
        )
        .expect("a linked type entails a refutation"),

        Criterion::ReadLimitations => Finding::new(
            criterion,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "whether a limitation is stated clearly is a judgement about prose",
            },
            "bioprism_safety::release::MedicalBoundary and bioprism-onco's typed research boundary \
             exist and refuse clinical outputs, which is necessary and is not the condition; the \
             condition is about what a reader can read",
        )
        .expect("unverifiable needs no entailing basis"),

        Criterion::DemonstrateNoLeakage => Finding::new(
            criterion,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "demonstrating absence of leakage requires executing the pack",
            },
            "one of the four clauses is held structurally here: crate::config makes a secret \
             unable to participate in an emitted artifact, so secret leakage into a bundle is not \
             representable. Hidden-label and future leakage belong to bioprism-lab and \
             bioprism-scope's temporal types and neither is exercised from a library",
        )
        .expect("unverifiable needs no entailing basis"),

        Criterion::RerunMinimisedRegressionOfflineInCi => Finding::new(
            criterion,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "this module does not read the CI configuration and could not run it",
            },
            "a workflow exists in the repository; its existence is not evidence that the minimized \
             regression reruns without network access, and deliberately nothing here looks at it",
        )
        .expect("unverifiable needs no entailing basis"),

        Criterion::ReproduceDeterministicOracleResults => Finding::new(
            criterion,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "the oracle runtime is not a dependency of this crate",
            },
            "determinism of the certificate digest across three implementations is recorded in \
             AGENTS.md and held by tests in the crates that produce it; from here it is somebody \
             else's assertion, which is exactly the basis that cannot carry a verdict",
        )
        .expect("unverifiable needs no entailing basis"),

        other => Finding::new(
            other,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "the condition is about an engineer operating a checkout, and this crate \
                          has no filesystem, no shell and no way to execute a workspace",
            },
            format!(
                "every necessary member is present ({}), which is necessary and not sufficient",
                other.necessary_members().join(", ")
            ),
        )
        .expect("unverifiable needs no entailing basis"),
    }
}

/// The report as a Markdown table, so a document cannot drift from the code.
pub fn markdown_table() -> String {
    let mut out = String::from("| # | condition | verdict | basis |\n|---:|---|---|---|\n");
    for finding in report() {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            finding.criterion.index(),
            finding.criterion.statement(),
            finding.verdict,
            finding.basis.as_str()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_safety::supply::SignatureStatus;

    #[test]
    fn the_embedded_manifest_yields_the_workspace_members_and_not_some_other_array() {
        let members = workspace_members();
        assert!(
            members.len() > 60,
            "the members array was expected to hold the whole workspace and held {}",
            members.len()
        );
        assert!(members.iter().all(|member| member.starts_with("crates/")));
        assert!(has_member("crates/ops"));
        assert!(has_member("crates/fiber"));
        assert!(!has_member("apps/web"));
    }

    #[test]
    fn an_acceptance_criterion_cannot_be_recorded_met_on_an_authors_say_so() {
        let error = Finding::new(
            Criterion::CleanCheckoutDemo,
            Verdict::Met,
            Basis::Author { who: "the author" },
            "it works on my machine",
        )
        .unwrap_err();
        match error {
            OpsError::AssertedAcceptance { basis, .. } => assert_eq!(basis, "author"),
            other => panic!("expected an asserted acceptance, got {other}"),
        }
    }

    #[test]
    fn a_refutation_also_needs_an_entailing_basis_not_only_a_pass() {
        assert!(Finding::new(
            Criterion::CleanCheckoutDemo,
            Verdict::Refuted,
            Basis::NoObserver {
                because: "cannot see"
            },
            "",
        )
        .is_err());
    }

    #[test]
    fn unverifiable_is_the_one_verdict_a_non_entailing_basis_can_carry() {
        assert!(Finding::new(
            Criterion::CleanCheckoutDemo,
            Verdict::Unverifiable,
            Basis::NoObserver {
                because: "no shell"
            },
            "",
        )
        .is_ok());
    }

    #[test]
    fn the_signature_refutation_stops_compiling_if_a_verified_variant_is_ever_added() {
        let status = SignatureStatus::NotChecked;
        let name = match status {
            SignatureStatus::NotChecked => "not-checked",
        };
        assert_eq!(name, status.to_string());
    }

    #[test]
    fn the_workspace_meets_none_of_the_fourteen_conditions_from_where_this_crate_can_look() {
        let summary = summary();
        assert_eq!(summary.total(), 14);
        assert_eq!(
            summary.met, 0,
            "an observation from a library can refute a criterion and almost never confirm one; \
             a nonzero count here means something is being confirmed that cannot be"
        );
        assert_eq!(summary.refuted, 2);
        assert_eq!(summary.unverifiable, 12);
    }

    #[test]
    fn the_two_refutations_are_the_graph_ui_and_the_signed_bundle() {
        let refuted: Vec<Criterion> = report()
            .into_iter()
            .filter(|finding| finding.verdict == Verdict::Refuted)
            .map(|finding| finding.criterion)
            .collect();
        assert_eq!(
            refuted,
            [
                Criterion::VerifySignedResultBundle,
                Criterion::ExplainFailureViaGraphUiAndTable,
            ]
        );
    }

    #[test]
    fn thirteen_of_the_fourteen_have_every_necessary_member_present() {
        let complete = Criterion::ALL
            .iter()
            .filter(|criterion| criterion.missing_members().is_empty())
            .count();
        assert_eq!(complete, 13);
        assert_eq!(
            Criterion::ExplainFailureViaGraphUiAndTable.missing_members(),
            ["apps/web"]
        );
    }

    #[test]
    fn every_criterion_carries_the_blueprints_own_words_and_its_position_in_the_list() {
        for (offset, criterion) in Criterion::ALL.iter().enumerate() {
            assert_eq!(criterion.index(), offset + 1);
            assert!(!criterion.statement().is_empty());
        }
        assert_eq!(
            Criterion::VerifySignedResultBundle.statement(),
            "verify a signed/checksummed result bundle"
        );
    }

    #[test]
    fn the_summary_is_three_counts_and_never_a_ratio() {
        let rendered = summary().to_string();
        assert!(rendered.contains("0 met"));
        assert!(rendered.contains("2 refuted"));
        assert!(!rendered.contains('%'));
    }

    #[test]
    fn the_markdown_table_is_regenerated_from_the_findings_and_cannot_drift() {
        let table = markdown_table();
        assert_eq!(table.lines().count(), 2 + Criterion::ALL.len());
        assert!(table.contains("| 10 | verify a signed/checksummed result bundle | refuted |"));
    }
}
