//! Fail-closed admission audit for untrusted code and research artifacts.
//!
//! The audit is deliberately an admission contract, not a process launcher. It checks that an
//! artifact has an identity and lineage, that an execution profile is isolated and bounded, that
//! every capability is explicitly approved, and that produced artifacts remain quarantined until
//! an independently evidenced release decision. It never mounts a filesystem, resolves a host
//! path, opens a socket, reads a secret, or executes the declared runtime.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SANDBOX_MANIFEST_SCHEMA: &str = "bioprism-sandbox/0.1";
pub const SANDBOX_AUDIT_SCHEMA: &str = "bioprism-sandbox-audit/0.1";
pub const MAX_ARTIFACTS: usize = 4096;
pub const MAX_PROFILES: usize = 4096;
pub const MAX_CAPABILITIES: usize = 16384;
pub const MAX_MOUNTS: usize = 16384;
pub const MAX_OUTPUTS: usize = 8192;
pub const MAX_LIST: usize = 32768;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxManifest {
    pub schema: String,
    pub system: SandboxSystem,
    #[serde(default)]
    pub artifacts: Vec<SandboxArtifact>,
    #[serde(default)]
    pub profiles: Vec<SandboxExecutionProfile>,
    #[serde(default)]
    pub capabilities: Vec<SandboxCapability>,
    #[serde(default)]
    pub outputs: Vec<SandboxOutput>,
    #[serde(default)]
    pub policies: SandboxPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSystem {
    pub id: String,
    pub version: String,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxArtifactKind {
    SourceCode,
    Notebook,
    Dataset,
    Model,
    Container,
    Package,
    Plugin,
    GeneratedOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxTrust {
    Untrusted,
    Internal,
    Reviewed,
    Trusted,
}

impl SandboxTrust {
    pub fn requires_hardening(self) -> bool {
        matches!(self, Self::Untrusted | Self::Internal)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxArtifact {
    pub id: String,
    pub kind: SandboxArtifactKind,
    pub digest: String,
    pub source: String,
    pub producer: String,
    pub trust: SandboxTrust,
    #[serde(default)]
    pub inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxNetworkMode {
    Deny,
    Allowlist,
    Unrestricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMountMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxMount {
    pub id: String,
    pub source_artifact: String,
    pub target: String,
    pub mode: SandboxMountMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxResourceLimits {
    pub cpu_millis: Option<u64>,
    pub memory_mb: Option<u64>,
    pub wall_time_seconds: Option<u64>,
    pub processes: Option<u32>,
    pub output_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecutionProfile {
    pub id: String,
    pub artifact: String,
    pub runtime: String,
    pub image_digest: Option<String>,
    pub environment_digest: Option<String>,
    pub user: String,
    pub rootless: bool,
    pub read_only_root: bool,
    pub no_privilege_escalation: bool,
    pub network: SandboxNetworkMode,
    #[serde(default)]
    pub network_allowlist: Vec<String>,
    #[serde(default)]
    pub mounts: Vec<SandboxMount>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub resources: SandboxResourceLimits,
    pub output_quarantine: bool,
    pub release_requires_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxCapabilityKind {
    FilesystemRead,
    FilesystemWrite,
    NetworkEgress,
    NetworkIngress,
    SecretAccess,
    ProcessSpawn,
    DeviceAccess,
    KernelAccess,
    Clock,
    Randomness,
    ArtifactPublish,
}

impl SandboxCapabilityKind {
    fn dangerous(self) -> bool {
        matches!(
            self,
            Self::FilesystemWrite
                | Self::NetworkEgress
                | Self::NetworkIngress
                | Self::SecretAccess
                | Self::ProcessSpawn
                | Self::DeviceAccess
                | Self::KernelAccess
                | Self::ArtifactPublish
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapability {
    pub id: String,
    pub profile: String,
    pub kind: SandboxCapabilityKind,
    pub target: String,
    pub decision: SandboxDecision,
    pub evidence_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub id: String,
    pub profile: String,
    pub artifact: String,
    pub digest: String,
    pub destination: String,
    pub quarantined: bool,
    pub released: bool,
    pub reviewed: bool,
    pub review_evidence: Option<String>,
    #[serde(default)]
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicies {
    pub default_deny: bool,
    pub require_digests: bool,
    pub require_lineage: bool,
    pub require_rootless: bool,
    pub require_read_only_root: bool,
    pub require_no_privilege_escalation: bool,
    pub require_network_allowlist: bool,
    pub require_resource_limits: bool,
    pub require_quarantine: bool,
    pub require_output_review: bool,
    pub require_reproducible_environment: bool,
}

impl Default for SandboxPolicies {
    fn default() -> Self {
        Self {
            default_deny: true,
            require_digests: true,
            require_lineage: true,
            require_rootless: true,
            require_read_only_root: true,
            require_no_privilege_escalation: true,
            require_network_allowlist: true,
            require_resource_limits: true,
            require_quarantine: true,
            require_output_review: true,
            require_reproducible_environment: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxIssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxIssue {
    pub code: String,
    pub severity: SandboxIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxArtifactAudit {
    pub artifact_id: String,
    pub digest_valid: bool,
    pub lineage_valid: bool,
    pub source_valid: bool,
    pub trust: SandboxTrust,
    pub hardening_required: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxProfileAudit {
    pub profile_id: String,
    pub artifact_valid: bool,
    pub isolation_valid: bool,
    pub network_valid: bool,
    pub mounts_valid: bool,
    pub capabilities_valid: bool,
    pub resources_valid: bool,
    pub output_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapabilityAudit {
    pub capability_id: String,
    pub profile_valid: bool,
    pub target_valid: bool,
    pub approved: bool,
    pub dangerous: bool,
    pub evidence_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxBoundaryAudit {
    pub profile_id: String,
    pub default_deny: bool,
    pub network_mode: SandboxNetworkMode,
    pub allowlist_valid: bool,
    pub host_paths_rejected: bool,
    pub dangerous_capabilities: usize,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResourceAudit {
    pub profile_id: String,
    pub cpu_bounded: bool,
    pub memory_bounded: bool,
    pub wall_time_bounded: bool,
    pub processes_bounded: bool,
    pub output_bounded: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxOutputAudit {
    pub output_id: String,
    pub profile_valid: bool,
    pub artifact_valid: bool,
    pub digest_valid: bool,
    pub lineage_valid: bool,
    pub quarantined: bool,
    pub review_valid: bool,
    pub release_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxCounts {
    pub artifacts: usize,
    pub untrusted_artifacts: usize,
    pub profiles: usize,
    pub isolated_profiles: usize,
    pub capabilities: usize,
    pub approved_capabilities: usize,
    pub dangerous_capabilities: usize,
    pub outputs: usize,
    pub quarantined_outputs: usize,
    pub released_outputs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub system_id: String,
    pub counts: SandboxCounts,
    pub artifact_audits: Vec<SandboxArtifactAudit>,
    pub profile_audits: Vec<SandboxProfileAudit>,
    pub capability_audits: Vec<SandboxCapabilityAudit>,
    pub boundary_audits: Vec<SandboxBoundaryAudit>,
    pub resource_audits: Vec<SandboxResourceAudit>,
    pub output_audits: Vec<SandboxOutputAudit>,
    pub issues: Vec<SandboxIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("cannot canonicalize sandbox manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize sandbox manifest: {0}")]
    Serialization(String),
}

impl SandboxManifest {
    pub fn digest(&self) -> Result<ContentHash, SandboxError> {
        let value = serde_json::to_value(self)
            .map_err(|error| SandboxError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<SandboxAudit, SandboxError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut artifacts = BTreeMap::<String, &SandboxArtifact>::new();
        let mut profiles = BTreeMap::<String, &SandboxExecutionProfile>::new();
        let mut capabilities = BTreeMap::<String, &SandboxCapability>::new();
        let mut outputs = BTreeMap::<String, &SandboxOutput>::new();

        bound(
            &mut issues,
            "artifacts",
            self.artifacts.len(),
            MAX_ARTIFACTS,
        );
        bound(&mut issues, "profiles", self.profiles.len(), MAX_PROFILES);
        bound(
            &mut issues,
            "capabilities",
            self.capabilities.len(),
            MAX_CAPABILITIES,
        );
        bound(
            &mut issues,
            "mounts",
            self.profiles
                .iter()
                .map(|profile| profile.mounts.len())
                .sum(),
            MAX_MOUNTS,
        );
        bound(&mut issues, "outputs", self.outputs.len(), MAX_OUTPUTS);
        if self.schema != SANDBOX_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!("expected {SANDBOX_MANIFEST_SCHEMA}, got {}", self.schema),
                "regenerate the admission declaration with the published schema",
            );
        }
        for (field, value) in [
            ("system.id", &self.system.id),
            ("system.version", &self.system.version),
            ("system.owner", &self.system.owner),
        ] {
            if !valid_text(value) {
                blocking(
                    &mut issues,
                    "system_identity_missing",
                    field,
                    format!("{field} must be non-empty"),
                    "declare the system identity and accountable owner",
                );
            }
        }

        for artifact in &self.artifacts {
            if !insert_unique(&mut artifacts, &artifact.id, "artifact", &mut issues) {
                continue;
            }
            artifacts.insert(artifact.id.clone(), artifact);
            if artifact.inputs.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "artifact.inputs",
                    artifact.inputs.len(),
                    MAX_LIST,
                );
            }
            if !valid_text(&artifact.id)
                || !valid_text(&artifact.source)
                || !valid_text(&artifact.producer)
            {
                blocking(
                    &mut issues,
                    "artifact_identity_missing",
                    &artifact.id,
                    "artifact id, source, and producer are required",
                    "bind the artifact to an accountable producer and source",
                );
            }
            let digest_valid = valid_digest(&artifact.digest);
            if self.policies.require_digests && !digest_valid {
                blocking(
                    &mut issues,
                    "artifact_digest_invalid",
                    &artifact.id,
                    "artifact digest must be a 64-character hexadecimal content address",
                    "hash the exact artifact bytes before admission",
                );
            }
            let mut seen_inputs = BTreeSet::new();
            for input in &artifact.inputs {
                if !seen_inputs.insert(input) {
                    blocking(
                        &mut issues,
                        "artifact_input_duplicate",
                        &artifact.id,
                        format!("input `{input}` is repeated"),
                        "retain each lineage edge exactly once",
                    );
                }
                if !self
                    .artifacts
                    .iter()
                    .any(|candidate| candidate.id == *input)
                {
                    blocking(
                        &mut issues,
                        "artifact_input_unknown",
                        &artifact.id,
                        format!("input `{input}` is not declared"),
                        "declare every parent artifact or remove the unsupported edge",
                    );
                }
            }
            if self.policies.require_lineage
                && artifact.kind != SandboxArtifactKind::SourceCode
                && artifact.inputs.is_empty()
            {
                blocking(
                    &mut issues,
                    "artifact_lineage_missing",
                    &artifact.id,
                    "derived or executable artifact has no declared parent lineage",
                    "record the content-addressed inputs that produced the artifact",
                );
            }
        }
        let mut visited_artifacts = BTreeSet::new();
        let mut cyclic_artifacts = BTreeSet::new();
        for artifact_id in artifacts.keys() {
            let mut path = Vec::new();
            artifact_lineage_cycle_nodes(
                artifact_id,
                &artifacts,
                &mut path,
                &mut visited_artifacts,
                &mut cyclic_artifacts,
            );
        }
        for artifact_id in &cyclic_artifacts {
            blocking(
                &mut issues,
                "artifact_lineage_cycle",
                artifact_id,
                "artifact parent lineage contains a cycle",
                "retain an acyclic source-to-derived artifact graph",
            );
        }

        for capability in &self.capabilities {
            if !insert_unique(&mut capabilities, &capability.id, "capability", &mut issues) {
                continue;
            }
            capabilities.insert(capability.id.clone(), capability);
            if !valid_text(&capability.id)
                || !valid_text(&capability.profile)
                || !valid_text(&capability.target)
            {
                blocking(
                    &mut issues,
                    "capability_identity_missing",
                    &capability.id,
                    "capability id, profile, and target are required",
                    "name the exact profile and resource target for the capability",
                );
            }
            if !profiles.is_empty()
                && !self
                    .profiles
                    .iter()
                    .any(|profile| profile.id == capability.profile)
            {
                blocking(
                    &mut issues,
                    "capability_profile_unknown",
                    &capability.id,
                    format!("profile `{}` is not declared", capability.profile),
                    "bind each capability to one declared execution profile",
                );
            }
            if capability.target == "*" || capability.target.contains("..") {
                blocking(
                    &mut issues,
                    "capability_target_broad",
                    &capability.id,
                    "wildcard or traversal capability targets are not bounded",
                    "name an exact allowlisted resource or explicitly deny the capability",
                );
            }
            if capability.kind.dangerous()
                && capability.decision == SandboxDecision::Allow
                && !valid_digest_option(capability.evidence_digest.as_ref())
            {
                blocking(
                    &mut issues,
                    "dangerous_capability_evidence_missing",
                    &capability.id,
                    "allowed dangerous capability has no decision evidence digest",
                    "attach an independent, content-addressed approval or deny the capability",
                );
            }
        }

        for profile in &self.profiles {
            if !insert_unique(&mut profiles, &profile.id, "profile", &mut issues) {
                continue;
            }
            profiles.insert(profile.id.clone(), profile);
            for (field, count) in [
                ("profile.network_allowlist", profile.network_allowlist.len()),
                ("profile.mounts", profile.mounts.len()),
                ("profile.capabilities", profile.capabilities.len()),
            ] {
                bound(&mut issues, field, count, MAX_LIST);
            }
            let artifact_valid = artifacts.contains_key(&profile.artifact);
            if !artifact_valid {
                blocking(
                    &mut issues,
                    "profile_artifact_unknown",
                    &profile.id,
                    format!("artifact `{}` is not declared", profile.artifact),
                    "bind the profile to a declared, digested artifact",
                );
            }
            if !valid_text(&profile.runtime) || !valid_text(&profile.user) {
                blocking(
                    &mut issues,
                    "profile_identity_missing",
                    &profile.id,
                    "runtime and non-empty execution user are required",
                    "declare the runtime and a non-root identity",
                );
            }
            if profile.user == "root" {
                blocking(
                    &mut issues,
                    "root_execution_user",
                    &profile.id,
                    "profile requests root execution",
                    "run as a dedicated non-root identity",
                );
            }
            if self.policies.require_rootless && !profile.rootless {
                blocking(
                    &mut issues,
                    "rootless_required",
                    &profile.id,
                    "profile is not rootless",
                    "enable the runtime's rootless isolation mode",
                );
            }
            if self.policies.require_read_only_root && !profile.read_only_root {
                blocking(
                    &mut issues,
                    "read_only_root_required",
                    &profile.id,
                    "profile can write its root filesystem",
                    "mount the root filesystem read-only and use explicit output mounts",
                );
            }
            if self.policies.require_no_privilege_escalation && !profile.no_privilege_escalation {
                blocking(
                    &mut issues,
                    "privilege_escalation_allowed",
                    &profile.id,
                    "profile does not disable privilege escalation",
                    "set no-new-privileges or the equivalent runtime control",
                );
            }
            let network_valid = match profile.network {
                SandboxNetworkMode::Deny => profile.network_allowlist.is_empty(),
                SandboxNetworkMode::Allowlist => {
                    !profile.network_allowlist.is_empty()
                        && profile
                            .network_allowlist
                            .iter()
                            .all(|item| valid_network_target(item))
                }
                SandboxNetworkMode::Unrestricted => false,
            };
            if !network_valid {
                blocking(
                    &mut issues,
                    "network_boundary_invalid",
                    &profile.id,
                    "network mode is unrestricted, contradictory, or has an unbounded allowlist",
                    "deny networking or enumerate bounded destinations without wildcards",
                );
            }
            if self.policies.require_network_allowlist
                && profile.network == SandboxNetworkMode::Unrestricted
            {
                blocking(
                    &mut issues,
                    "network_allowlist_required",
                    &profile.id,
                    "unrestricted networking is forbidden for admitted research artifacts",
                    "use deny or a reviewed destination allowlist",
                );
            }
            let mut mount_ids = BTreeSet::new();
            for mount in &profile.mounts {
                if !mount_ids.insert(mount.id.to_ascii_lowercase()) {
                    blocking(
                        &mut issues,
                        "mount_duplicate",
                        &profile.id,
                        format!("mount `{}` is repeated", mount.id),
                        "give each mount one stable identity",
                    );
                }
                if !valid_path(&mount.target) {
                    blocking(
                        &mut issues,
                        "mount_target_unsafe",
                        &profile.id,
                        format!(
                            "mount target `{}` is absolute, traverses, or names a host namespace",
                            mount.target
                        ),
                        "use a private, normalized target below the sandbox root",
                    );
                }
                if !artifacts.contains_key(&mount.source_artifact) {
                    blocking(
                        &mut issues,
                        "mount_source_unknown",
                        &profile.id,
                        format!(
                            "mount source artifact `{}` is not declared",
                            mount.source_artifact
                        ),
                        "mount only content-addressed declared artifacts",
                    );
                }
                if mount.mode == SandboxMountMode::ReadWrite
                    && !profile.capabilities.iter().any(|capability_id| {
                        capabilities
                            .get(capability_id)
                            .map(|capability| {
                                capability.kind == SandboxCapabilityKind::FilesystemWrite
                                    && capability.decision == SandboxDecision::Allow
                                    && capability.target == mount.target
                            })
                            .unwrap_or(false)
                    })
                {
                    blocking(
                        &mut issues,
                        "write_mount_without_capability",
                        &profile.id,
                        format!("read-write mount `{}` has no matching approved filesystem capability", mount.target),
                        "bind every write mount to an exact approved capability or make it read-only",
                    );
                }
            }
            let capabilities_valid = profile.capabilities.iter().all(|capability_id| {
                capabilities
                    .get(capability_id)
                    .map(|capability| {
                        capability.profile == profile.id
                            && capability.decision == SandboxDecision::Allow
                            && (!capability.kind.dangerous()
                                || valid_digest_option(capability.evidence_digest.as_ref()))
                    })
                    .unwrap_or(false)
            });
            if !capabilities_valid {
                blocking(
                    &mut issues,
                    "profile_capability_invalid",
                    &profile.id,
                    "profile references an unknown, denied, cross-profile, or unevidenced capability",
                    "declare only exact approved capabilities belonging to this profile",
                );
            }
            let image_valid = valid_digest_option(profile.image_digest.as_ref());
            let environment_valid = valid_digest_option(profile.environment_digest.as_ref());
            if self.policies.require_reproducible_environment
                && (!image_valid || !environment_valid)
            {
                blocking(
                    &mut issues,
                    "reproducible_environment_missing",
                    &profile.id,
                    "profile lacks a valid image and environment digest",
                    "pin both the runtime image and dependency environment by digest",
                );
            }
            let resources_valid = resources_bounded(&profile.resources);
            if self.policies.require_resource_limits && !resources_valid {
                blocking(
                    &mut issues,
                    "resource_limits_missing",
                    &profile.id,
                    "one or more CPU, memory, wall-time, process, or output ceilings are absent or zero",
                    "declare positive finite limits for every resource dimension",
                );
            }
            if self.policies.require_quarantine && !profile.output_quarantine {
                blocking(
                    &mut issues,
                    "output_quarantine_missing",
                    &profile.id,
                    "profile can emit results without quarantine",
                    "route every output through an isolated quarantine before release review",
                );
            }
            if self.policies.require_output_review && !profile.release_requires_review {
                blocking(
                    &mut issues,
                    "output_review_missing",
                    &profile.id,
                    "profile can release output without independent review",
                    "require a separate release review before publication or downstream use",
                );
            }
        }

        for output in &self.outputs {
            if !insert_unique(&mut outputs, &output.id, "output", &mut issues) {
                continue;
            }
            outputs.insert(output.id.clone(), output);
            if output.parents.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "output.parents",
                    output.parents.len(),
                    MAX_LIST,
                );
            }
            let profile_valid = profiles.contains_key(&output.profile);
            let artifact_valid = artifacts.contains_key(&output.artifact);
            if !profile_valid || !artifact_valid {
                blocking(
                    &mut issues,
                    "output_reference_unknown",
                    &output.id,
                    "output profile and artifact must both be declared",
                    "retain an explicit profile and artifact lineage for every output",
                );
            }
            let digest_valid = valid_digest(&output.digest);
            if self.policies.require_digests && !digest_valid {
                blocking(
                    &mut issues,
                    "output_digest_invalid",
                    &output.id,
                    "output digest is not a valid content address",
                    "hash the exact produced bytes before any release decision",
                );
            }
            let lineage_valid = !output.parents.is_empty()
                && output
                    .parents
                    .iter()
                    .all(|parent| artifacts.contains_key(parent));
            if self.policies.require_lineage && !lineage_valid {
                blocking(
                    &mut issues,
                    "output_lineage_missing",
                    &output.id,
                    "output has no closed parent artifact lineage",
                    "record every content-addressed input that contributed to the output",
                );
            }
            let review_valid = !output.released
                || (output.reviewed && valid_digest_option(output.review_evidence.as_ref()));
            if self.policies.require_output_review && !review_valid {
                blocking(
                    &mut issues,
                    "released_output_unreviewed",
                    &output.id,
                    "released output lacks independent review evidence",
                    "keep it quarantined or attach a separate content-addressed release review",
                );
            }
            if self.policies.require_quarantine && output.released && !output.quarantined {
                blocking(
                    &mut issues,
                    "released_output_not_quarantined",
                    &output.id,
                    "output was released without first entering quarantine",
                    "make quarantine a mandatory predecessor of release",
                );
            }
            if !valid_text(&output.destination) || output.destination == "*" {
                blocking(
                    &mut issues,
                    "output_destination_unbounded",
                    &output.id,
                    "output destination is empty, invalid, or wildcarded",
                    "name the bounded destination and its release purpose",
                );
            }
        }

        let artifact_audits = self
            .artifacts
            .iter()
            .take(MAX_LIST)
            .map(|artifact| {
                let digest_valid = valid_digest(&artifact.digest);
                let lineage_valid = artifact
                    .inputs
                    .iter()
                    .all(|input| artifacts.contains_key(input))
                    && !cyclic_artifacts.contains(&artifact.id)
                    && (!self.policies.require_lineage
                        || artifact.kind == SandboxArtifactKind::SourceCode
                        || !artifact.inputs.is_empty());
                let source_valid = valid_text(&artifact.source) && valid_text(&artifact.producer);
                SandboxArtifactAudit {
                    artifact_id: artifact.id.clone(),
                    digest_valid,
                    lineage_valid,
                    source_valid,
                    trust: artifact.trust,
                    hardening_required: artifact.trust.requires_hardening(),
                    ready: digest_valid && lineage_valid && source_valid,
                }
            })
            .collect::<Vec<_>>();
        let capability_audits = self
            .capabilities
            .iter()
            .take(MAX_LIST)
            .map(|capability| {
                let profile_valid = profiles
                    .get(&capability.profile)
                    .map(|profile| profile.capabilities.iter().any(|id| id == &capability.id))
                    .unwrap_or(false);
                let target_valid = valid_text(&capability.target)
                    && capability.target != "*"
                    && !capability.target.contains("..");
                let approved = capability.decision == SandboxDecision::Allow;
                let evidence_valid = !capability.kind.dangerous()
                    || valid_digest_option(capability.evidence_digest.as_ref());
                SandboxCapabilityAudit {
                    capability_id: capability.id.clone(),
                    profile_valid,
                    target_valid,
                    approved,
                    dangerous: capability.kind.dangerous(),
                    evidence_valid,
                    ready: profile_valid && target_valid && approved && evidence_valid,
                }
            })
            .collect::<Vec<_>>();
        let profile_audits = self
            .profiles
            .iter()
            .take(MAX_LIST)
            .map(|profile| {
                let artifact_valid = artifacts.contains_key(&profile.artifact);
                let isolation_valid = profile.rootless
                    && profile.read_only_root
                    && profile.no_privilege_escalation
                    && profile.user != "root";
                let network_valid = match profile.network {
                    SandboxNetworkMode::Deny => profile.network_allowlist.is_empty(),
                    SandboxNetworkMode::Allowlist => {
                        !profile.network_allowlist.is_empty()
                            && profile
                                .network_allowlist
                                .iter()
                                .all(|item| valid_network_target(item))
                    }
                    SandboxNetworkMode::Unrestricted => false,
                };
                let mounts_valid = profile.mounts.iter().all(|mount| valid_path(&mount.target));
                let capabilities_valid = profile.capabilities.iter().all(|id| {
                    capability_audits
                        .iter()
                        .any(|capability| capability.capability_id == *id && capability.ready)
                });
                let resources_valid = resources_bounded(&profile.resources);
                let output_valid = profile.output_quarantine && profile.release_requires_review;
                SandboxProfileAudit {
                    profile_id: profile.id.clone(),
                    artifact_valid,
                    isolation_valid,
                    network_valid,
                    mounts_valid,
                    capabilities_valid,
                    resources_valid,
                    output_valid,
                    ready: artifact_valid
                        && isolation_valid
                        && network_valid
                        && mounts_valid
                        && capabilities_valid
                        && resources_valid
                        && output_valid,
                }
            })
            .collect::<Vec<_>>();
        let boundary_audits = self
            .profiles
            .iter()
            .take(MAX_LIST)
            .map(|profile| SandboxBoundaryAudit {
                profile_id: profile.id.clone(),
                default_deny: self.policies.default_deny,
                network_mode: profile.network,
                allowlist_valid: profile.network != SandboxNetworkMode::Unrestricted
                    && (profile.network == SandboxNetworkMode::Deny
                        || profile
                            .network_allowlist
                            .iter()
                            .all(|item| valid_network_target(item))),
                host_paths_rejected: profile.mounts.iter().all(|mount| valid_path(&mount.target)),
                dangerous_capabilities: profile
                    .capabilities
                    .iter()
                    .filter(|id| {
                        capabilities
                            .get(*id)
                            .map(|capability| capability.kind.dangerous())
                            .unwrap_or(false)
                    })
                    .count(),
                ready: self.policies.default_deny
                    && profile.network != SandboxNetworkMode::Unrestricted
                    && profile.mounts.iter().all(|mount| valid_path(&mount.target)),
            })
            .collect::<Vec<_>>();
        let resource_audits = self
            .profiles
            .iter()
            .take(MAX_LIST)
            .map(|profile| SandboxResourceAudit {
                profile_id: profile.id.clone(),
                cpu_bounded: profile.resources.cpu_millis.is_some_and(|value| value > 0),
                memory_bounded: profile.resources.memory_mb.is_some_and(|value| value > 0),
                wall_time_bounded: profile
                    .resources
                    .wall_time_seconds
                    .is_some_and(|value| value > 0),
                processes_bounded: profile.resources.processes.is_some_and(|value| value > 0),
                output_bounded: profile
                    .resources
                    .output_bytes
                    .is_some_and(|value| value > 0),
                ready: resources_bounded(&profile.resources),
            })
            .collect::<Vec<_>>();
        let output_audits = self
            .outputs
            .iter()
            .take(MAX_LIST)
            .map(|output| {
                let profile_valid = profiles.contains_key(&output.profile);
                let artifact_valid = artifacts.contains_key(&output.artifact);
                let digest_valid = valid_digest(&output.digest);
                let lineage_valid = !output.parents.is_empty()
                    && output
                        .parents
                        .iter()
                        .all(|parent| artifacts.contains_key(parent));
                let review_valid = !output.released
                    || (output.reviewed && valid_digest_option(output.review_evidence.as_ref()));
                let release_valid = !output.released || (output.quarantined && review_valid);
                SandboxOutputAudit {
                    output_id: output.id.clone(),
                    profile_valid,
                    artifact_valid,
                    digest_valid,
                    lineage_valid,
                    quarantined: output.quarantined,
                    review_valid,
                    release_valid,
                    ready: profile_valid
                        && artifact_valid
                        && digest_valid
                        && lineage_valid
                        && (!self.policies.require_quarantine || output.quarantined)
                        && review_valid
                        && release_valid,
                }
            })
            .collect::<Vec<_>>();
        issues.sort_by(|left, right| {
            left.subject
                .cmp(&right.subject)
                .then(left.code.cmp(&right.code))
                .then(left.detail.cmp(&right.detail))
        });
        let counts = SandboxCounts {
            artifacts: self.artifacts.len(),
            untrusted_artifacts: self
                .artifacts
                .iter()
                .filter(|artifact| artifact.trust == SandboxTrust::Untrusted)
                .count(),
            profiles: self.profiles.len(),
            isolated_profiles: profile_audits
                .iter()
                .filter(|profile| profile.isolation_valid)
                .count(),
            capabilities: self.capabilities.len(),
            approved_capabilities: self
                .capabilities
                .iter()
                .filter(|capability| capability.decision == SandboxDecision::Allow)
                .count(),
            dangerous_capabilities: self
                .capabilities
                .iter()
                .filter(|capability| capability.kind.dangerous())
                .count(),
            outputs: self.outputs.len(),
            quarantined_outputs: self
                .outputs
                .iter()
                .filter(|output| output.quarantined)
                .count(),
            released_outputs: self.outputs.iter().filter(|output| output.released).count(),
        };
        let valid = !issues
            .iter()
            .any(|issue| issue.severity == SandboxIssueSeverity::Blocking);
        Ok(SandboxAudit {
            schema: SANDBOX_AUDIT_SCHEMA.to_string(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            system_id: self.system.id.clone(),
            counts,
            artifact_audits,
            profile_audits,
            capability_audits,
            boundary_audits,
            resource_audits,
            output_audits,
            issues,
            guarantees: vec![
                "artifact and output content addresses are checked before admission".into(),
                "profiles are checked for rootless, read-only, no-escalation, network, mount, capability, and resource boundaries".into(),
                "dangerous capabilities require exact targets and independent evidence".into(),
                "outputs remain lineage-bound and quarantine-aware through release".into(),
            ],
            limitations: vec![
                "the audit does not execute code, mount a filesystem, open a network, or read a secret".into(),
                "runtime enforcement, kernel policy, scanner results, credential revocation, and operator response remain external".into(),
                "a valid declaration is admission evidence, not proof that an external runtime enforced it".into(),
            ],
        })
    }
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.len() <= 256
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn valid_digest_option(value: Option<&String>) -> bool {
    value.map(|digest| valid_digest(digest)).unwrap_or(false)
}

fn artifact_lineage_cycle_nodes(
    artifact_id: &str,
    artifacts: &BTreeMap<String, &SandboxArtifact>,
    path: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
    cyclic: &mut BTreeSet<String>,
) -> bool {
    if let Some(start) = path.iter().position(|current| current == artifact_id) {
        cyclic.extend(path[start..].iter().cloned());
        return true;
    }
    if visited.contains(artifact_id) {
        return false;
    }
    path.push(artifact_id.to_string());
    let found = artifacts
        .get(artifact_id)
        .map(|artifact| {
            artifact.inputs.iter().any(|input| {
                artifacts.contains_key(input)
                    && artifact_lineage_cycle_nodes(input, artifacts, path, visited, cyclic)
            })
        })
        .unwrap_or(false);
    path.pop();
    visited.insert(artifact_id.to_string());
    found
}

fn valid_path(value: &str) -> bool {
    value == value.trim()
        && !value.chars().any(char::is_control)
        && value.starts_with('/')
        && value != "/"
        && !value.contains("..")
        && !value.contains('\\')
        && !value.starts_with("/proc")
        && !value.starts_with("/sys")
        && !value.starts_with("/dev")
}

fn valid_network_target(value: &str) -> bool {
    valid_text(value)
        && value != "*"
        && value != "0.0.0.0/0"
        && value != "::/0"
        && !value.contains("*")
        && !value.contains("..")
}

fn resources_bounded(resources: &SandboxResourceLimits) -> bool {
    resources.cpu_millis.is_some_and(|value| value > 0)
        && resources.memory_mb.is_some_and(|value| value > 0)
        && resources.wall_time_seconds.is_some_and(|value| value > 0)
        && resources.processes.is_some_and(|value| value > 0)
        && resources.output_bytes.is_some_and(|value| value > 0)
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, &T>,
    id: &str,
    kind: &str,
    issues: &mut Vec<SandboxIssue>,
) -> bool {
    if map
        .keys()
        .any(|existing| existing == id || existing.eq_ignore_ascii_case(id))
    {
        blocking(
            issues,
            "duplicate_identifier",
            id,
            format!("duplicate {kind} identifier `{id}`"),
            format!("assign one stable identifier to each {kind}"),
        );
        false
    } else {
        true
    }
}

fn bound(issues: &mut Vec<SandboxIssue>, field: &str, actual: usize, maximum: usize) {
    if actual > maximum {
        blocking(
            issues,
            "input_bound_exceeded",
            field,
            format!("{field} contains {actual} entries; maximum is {maximum}"),
            "split the admission request into bounded manifests",
        );
    }
}

fn blocking(
    issues: &mut Vec<SandboxIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SandboxIssue {
        code: code.into(),
        severity: SandboxIssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> SandboxManifest {
        SandboxManifest {
            schema: SANDBOX_MANIFEST_SCHEMA.into(),
            system: SandboxSystem {
                id: "aurora-sandbox".into(),
                version: "0.1.0".into(),
                owner: "platform".into(),
            },
            artifacts: vec![
                SandboxArtifact {
                    id: "source".into(),
                    kind: SandboxArtifactKind::SourceCode,
                    digest: "a".repeat(64),
                    source: "repo/source.py".into(),
                    producer: "ci".into(),
                    trust: SandboxTrust::Reviewed,
                    inputs: vec![],
                },
                SandboxArtifact {
                    id: "dataset".into(),
                    kind: SandboxArtifactKind::Dataset,
                    digest: "b".repeat(64),
                    source: "registry/dataset".into(),
                    producer: "registry".into(),
                    trust: SandboxTrust::Untrusted,
                    inputs: vec!["source".into()],
                },
            ],
            profiles: vec![SandboxExecutionProfile {
                id: "profile".into(),
                artifact: "dataset".into(),
                runtime: "oci".into(),
                image_digest: Some("c".repeat(64)),
                environment_digest: Some("d".repeat(64)),
                user: "runner".into(),
                rootless: true,
                read_only_root: true,
                no_privilege_escalation: true,
                network: SandboxNetworkMode::Allowlist,
                network_allowlist: vec!["packages.example".into()],
                mounts: vec![SandboxMount {
                    id: "input".into(),
                    source_artifact: "dataset".into(),
                    target: "/inputs/data".into(),
                    mode: SandboxMountMode::ReadOnly,
                }],
                capabilities: vec!["network".into()],
                resources: SandboxResourceLimits {
                    cpu_millis: Some(1000),
                    memory_mb: Some(1024),
                    wall_time_seconds: Some(60),
                    processes: Some(8),
                    output_bytes: Some(1_000_000),
                },
                output_quarantine: true,
                release_requires_review: true,
            }],
            capabilities: vec![SandboxCapability {
                id: "network".into(),
                profile: "profile".into(),
                kind: SandboxCapabilityKind::NetworkEgress,
                target: "packages.example".into(),
                decision: SandboxDecision::Allow,
                evidence_digest: Some("e".repeat(64)),
            }],
            outputs: vec![SandboxOutput {
                id: "result".into(),
                profile: "profile".into(),
                artifact: "dataset".into(),
                digest: "f".repeat(64),
                destination: "quarantine".into(),
                quarantined: true,
                released: false,
                reviewed: false,
                review_evidence: None,
                parents: vec!["dataset".into()],
            }],
            policies: SandboxPolicies::default(),
        }
    }

    #[test]
    fn valid_manifest_preserves_artifact_isolation_capability_resource_and_output_layers() {
        let audit = manifest().audit().expect("audit");
        assert!(audit.valid, "issues: {:?}", audit.issues);
        assert_eq!(audit.counts.untrusted_artifacts, 1);
        assert!(audit.profile_audits[0].ready);
        assert!(audit.boundary_audits[0].ready);
        assert!(audit.resource_audits[0].ready);
        assert!(audit.output_audits[0].ready);
    }

    #[test]
    fn unsafe_runtime_and_unbounded_capability_fail_closed_with_layered_findings() {
        let mut value = manifest();
        value.profiles[0].rootless = false;
        value.profiles[0].network = SandboxNetworkMode::Unrestricted;
        value.profiles[0].resources.memory_mb = None;
        value.capabilities[0].target = "*".into();
        value.capabilities[0].evidence_digest = None;
        let audit = value.audit().expect("audit");
        assert!(!audit.valid);
        for code in [
            "rootless_required",
            "network_boundary_invalid",
            "resource_limits_missing",
            "capability_target_broad",
            "dangerous_capability_evidence_missing",
        ] {
            assert!(
                audit.issues.iter().any(|issue| issue.code == code),
                "missing {code}: {:?}",
                audit.issues
            );
        }
    }

    #[test]
    fn released_output_requires_quarantine_and_independent_review_evidence() {
        let mut value = manifest();
        value.outputs[0].released = true;
        value.outputs[0].quarantined = false;
        value.outputs[0].reviewed = false;
        let audit = value.audit().expect("audit");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "released_output_unreviewed"));
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "released_output_not_quarantined"));
    }

    #[test]
    fn artifact_parent_cycles_are_not_admitted_as_lineage() {
        let mut value = manifest();
        value.artifacts[1].inputs = vec!["dataset".into()];

        let audit = value.audit().expect("audit");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "artifact_lineage_cycle"));
        assert!(!audit.artifact_audits[1].lineage_valid);
        assert!(!audit.artifact_audits[1].ready);
    }

    #[test]
    fn sandbox_admission_rejects_noncanonical_digests_and_control_metadata() {
        let mut value = manifest();
        value.system.owner = "platform\u{0000}runner".into();
        value.artifacts[0].digest = "A".repeat(64);
        value.profiles[0].image_digest = Some("B".repeat(64));
        value.outputs[0].digest = "C".repeat(64);
        value.outputs[0].destination = "quarantine\u{000b}store".into();

        let audit = value.audit().expect("audit");
        assert!(!audit.valid);
        for code in [
            "system_identity_missing",
            "artifact_digest_invalid",
            "reproducible_environment_missing",
            "output_digest_invalid",
            "output_destination_unbounded",
        ] {
            assert!(
                audit.issues.iter().any(|issue| issue.code == code),
                "missing {code}"
            );
        }
        assert!(!valid_digest(&"A".repeat(64)));
        assert!(valid_digest(&"a".repeat(64)));
    }

    #[test]
    fn sandbox_admission_rejects_case_colliding_artifacts() {
        let mut value = manifest();
        let mut duplicate = value.artifacts[0].clone();
        duplicate.id = "SOURCE".into();
        value.artifacts.push(duplicate);

        let audit = value.audit().expect("audit");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "duplicate_identifier"));
    }

    #[test]
    fn sandbox_admission_rejects_padded_and_controlled_mount_targets() {
        let mut padded = manifest();
        padded.profiles[0].mounts[0].target = "/inputs/data ".into();
        let audit = padded.audit().expect("audit");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "mount_target_unsafe"));

        let mut controlled = manifest();
        controlled.profiles[0].mounts[0].target = "/inputs/data\n".into();
        let audit = controlled.audit().expect("audit");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "mount_target_unsafe"));
    }
}
