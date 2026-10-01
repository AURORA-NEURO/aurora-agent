//! Transitive dependency and locality-leakage audit for glioma research-object releases.
//!
//! The auditor walks an export candidate's declared dependency graph before serialization. It
//! detects protected payloads, local-only references, embedded secrets, path escapes, missing
//! dependencies, and cycles while returning only identifiers, paths, severities, and digests.
//! A blocked audit is a hard release stop; no export bytes are produced by this feature.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseDependencyLeakageAudit1@1";
pub const MAX_NODES: usize = 2_048;
pub const MAX_DEPTH: usize = 128;
pub const MAX_PREFIXES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyExportScope {
    LocalOnly,
    AggregateMetadata,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageKind {
    ProtectedPayload,
    DirectIdentifier,
    LocalOnlyReference,
    EmbeddedSecret,
    PathEscape,
    MissingDependency,
    DependencyCycle,
    DepthExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDependencyNode {
    pub node_id: String,
    pub path: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub dependency_order: Vec<String>,
    pub export_scope: DependencyExportScope,
    pub requested_export: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub embedded_secret: bool,
    pub symlink_escape: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDependencyLeakageRequest {
    pub candidate_manifest_digest: ContentHash,
    pub root_order: Vec<String>,
    pub nodes: Vec<ReleaseDependencyNode>,
    pub allowed_export_prefixes: Vec<String>,
    pub max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseLeakFinding {
    pub finding_id: String,
    pub kind: LeakageKind,
    pub severity: LeakageSeverity,
    pub node_id: String,
    pub dependency_path: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageAuditDisposition {
    Clear,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseDependencyLeakageAudit {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_manifest_digest: ContentHash,
    pub root_order: Vec<String>,
    pub traversal_order: Vec<String>,
    pub findings: Vec<ReleaseLeakFinding>,
    pub critical_finding_order: Vec<String>,
    pub warning_finding_order: Vec<String>,
    pub omitted_local_order: Vec<String>,
    pub export_blocked: bool,
    pub disposition: LeakageAuditDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LeakageAuditError {
    #[error("release leakage audit request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release leakage audit output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release leakage audit digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 512
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn output_body(output: &ReleaseDependencyLeakageAudit) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "candidate_manifest_digest": output.candidate_manifest_digest,
        "root_order": output.root_order,
        "traversal_order": output.traversal_order,
        "findings": output.findings,
        "critical_finding_order": output.critical_finding_order,
        "warning_finding_order": output.warning_finding_order,
        "omitted_local_order": output.omitted_local_order,
        "export_blocked": output.export_blocked,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &ReleaseDependencyLeakageRequest) -> Result<(), LeakageAuditError> {
    if !valid_hash(&request.candidate_manifest_digest)
        || request.root_order.is_empty()
        || !canonical(&request.root_order)
        || request.nodes.is_empty()
        || request.nodes.len() > MAX_NODES
        || request.allowed_export_prefixes.is_empty()
        || request.allowed_export_prefixes.len() > MAX_PREFIXES
        || request
            .allowed_export_prefixes
            .iter()
            .any(|prefix| !safe_text(prefix) || prefix.contains("..") || !prefix.starts_with('/'))
        || request.max_depth == 0
        || request.max_depth > MAX_DEPTH
    {
        return Err(LeakageAuditError::InvalidRequest(
            "manifest digest, sorted roots/nodes, bounded export prefixes, and depth are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for node in &request.nodes {
        if !safe_text(&node.node_id)
            || !ids.insert(node.node_id.clone())
            || !safe_text(&node.path)
            || node.path.contains("..")
            || !safe_text(&node.content_type)
            || !valid_hash(&node.content_hash)
            || !canonical(&node.dependency_order)
            || node
                .dependency_order
                .iter()
                .any(|dependency| !safe_text(dependency))
        {
            return Err(LeakageAuditError::InvalidRequest(format!(
                "dependency node {} is malformed or duplicated",
                node.node_id
            )));
        }
    }
    if request.root_order.iter().any(|root| !ids.contains(root)) {
        return Err(LeakageAuditError::InvalidRequest(
            "every export root must name a supplied dependency node".into(),
        ));
    }
    Ok(())
}

impl ReleaseDependencyLeakageAudit {
    pub fn validate(&self) -> Result<(), LeakageAuditError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_hash(&self.candidate_manifest_digest)
            || !canonical(&self.root_order)
            || !canonical(&self.traversal_order)
            || !canonical(&self.critical_finding_order)
            || !canonical(&self.warning_finding_order)
            || !canonical(&self.omitted_local_order)
            || self.findings.iter().any(|finding| {
                !safe_text(&finding.finding_id)
                    || !safe_text(&finding.node_id)
                    || !safe_text(&finding.reason)
                    || finding.dependency_path.is_empty()
                    || finding
                        .dependency_path
                        .iter()
                        .any(|node_id| !safe_text(node_id))
                    || finding
                        .dependency_path
                        .windows(2)
                        .any(|pair| pair[0] == pair[1])
            })
            || self.export_blocked != !self.critical_finding_order.is_empty()
            || self.digest.as_str().len() != 64
        {
            return Err(LeakageAuditError::InvalidOutput(
                "audit identity, finding ordering, path, blocker, or digest invariants are invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| LeakageAuditError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LeakageAuditError::InvalidOutput(
                "leakage audit digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn path_allowed(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

struct LeakageTraversal<'a> {
    request: &'a ReleaseDependencyLeakageRequest,
    node_map: &'a BTreeMap<String, &'a ReleaseDependencyNode>,
    traversal: BTreeSet<String>,
    findings: Vec<ReleaseLeakFinding>,
    omitted: BTreeSet<String>,
    finding_sequence: usize,
}

impl LeakageTraversal<'_> {
    fn emit(
        &mut self,
        node_id: &str,
        path: &[String],
        kind: LeakageKind,
        severity: LeakageSeverity,
        reason: String,
    ) {
        self.finding_sequence += 1;
        self.findings.push(ReleaseLeakFinding {
            finding_id: format!("leak-{:06}", self.finding_sequence),
            kind,
            severity,
            node_id: node_id.into(),
            dependency_path: path.to_vec(),
            reason,
        });
    }

    fn visit(&mut self, node_id: &str, path: &mut Vec<String>, depth: usize) {
        if depth > self.request.max_depth {
            self.emit(
                node_id,
                path,
                LeakageKind::DepthExceeded,
                LeakageSeverity::Critical,
                "dependency traversal exceeded the configured depth".into(),
            );
            return;
        }
        if path.iter().any(|ancestor| ancestor == node_id) {
            self.emit(
                node_id,
                path,
                LeakageKind::DependencyCycle,
                LeakageSeverity::Critical,
                "dependency cycle prevents a finite export closure".into(),
            );
            return;
        }
        let Some(node) = self.node_map.get(node_id).map(|node| (*node).clone()) else {
            self.emit(
                node_id,
                path,
                LeakageKind::MissingDependency,
                LeakageSeverity::Critical,
                "declared dependency is absent from the candidate graph".into(),
            );
            return;
        };
        path.push(node_id.into());
        self.traversal.insert(node_id.into());
        if node.requested_export && node.export_scope == DependencyExportScope::LocalOnly {
            self.emit(
                &node.node_id,
                path,
                LeakageKind::LocalOnlyReference,
                LeakageSeverity::Critical,
                "local-only node is requested for export".into(),
            );
        }
        if node.requested_export && node.contains_human_data {
            self.emit(
                &node.node_id,
                path,
                LeakageKind::ProtectedPayload,
                LeakageSeverity::Critical,
                "human-data flag is present on an exported node".into(),
            );
        }
        if node.requested_export && node.contains_direct_identifiers {
            self.emit(
                &node.node_id,
                path,
                LeakageKind::DirectIdentifier,
                LeakageSeverity::Critical,
                "direct-identifier flag is present on an exported node".into(),
            );
        }
        if node.requested_export && node.embedded_secret {
            self.emit(
                &node.node_id,
                path,
                LeakageKind::EmbeddedSecret,
                LeakageSeverity::Critical,
                "secret scanner marked an exported node".into(),
            );
        }
        if node.requested_export
            && (node.symlink_escape
                || !path_allowed(&node.path, &self.request.allowed_export_prefixes))
        {
            self.emit(
                &node.node_id,
                path,
                LeakageKind::PathEscape,
                LeakageSeverity::Critical,
                "export path escapes the declared allow-list".into(),
            );
        }
        if !node.requested_export && node.export_scope == DependencyExportScope::LocalOnly {
            self.omitted.insert(node.node_id.clone());
        }
        for dependency in &node.dependency_order {
            self.visit(dependency, path, depth + 1);
        }
        path.pop();
    }
}

/// Traverse an export candidate's dependency graph and fail closed on transitive leakage.
pub fn audit_glioma_release_dependency_leakage(
    request: &ReleaseDependencyLeakageRequest,
) -> Result<ReleaseDependencyLeakageAudit, LeakageAuditError> {
    validate_request(request)?;
    let node_map = request
        .nodes
        .iter()
        .map(|node| (node.node_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let mut audit = LeakageTraversal {
        request,
        node_map: &node_map,
        traversal: BTreeSet::new(),
        findings: Vec::new(),
        omitted: BTreeSet::new(),
        finding_sequence: 0,
    };
    for root in &request.root_order {
        audit.visit(root, &mut Vec::new(), 0);
    }
    let traversal = audit.traversal;
    let mut findings = audit.findings;
    let omitted = audit.omitted;
    findings.sort_by(|left, right| left.finding_id.cmp(&right.finding_id));
    let critical = findings
        .iter()
        .filter(|finding| finding.severity == LeakageSeverity::Critical)
        .map(|finding| finding.finding_id.clone())
        .collect::<Vec<_>>();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == LeakageSeverity::Warning)
        .map(|finding| finding.finding_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !critical.is_empty() {
        LeakageAuditDisposition::Blocked
    } else if !warnings.is_empty() {
        LeakageAuditDisposition::Warning
    } else {
        LeakageAuditDisposition::Clear
    };
    let mut output = ReleaseDependencyLeakageAudit {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_manifest_digest: request.candidate_manifest_digest.clone(),
        root_order: request.root_order.clone(),
        traversal_order: traversal.into_iter().collect(),
        findings,
        critical_finding_order: critical,
        warning_finding_order: warnings,
        omitted_local_order: omitted.into_iter().collect(),
        export_blocked: disposition == LeakageAuditDisposition::Blocked,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-leakage-audit"),
    };
    output.digest = ContentHash::of_value(&output_body(&output))
        .map_err(|error| LeakageAuditError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn node(id: &str, dependencies: Vec<&str>) -> ReleaseDependencyNode {
        ReleaseDependencyNode {
            node_id: id.into(),
            path: format!("/release/{id}.json"),
            content_type: "application/json".into(),
            content_hash: hash(id),
            dependency_order: dependencies.into_iter().map(str::to_string).collect(),
            export_scope: DependencyExportScope::Public,
            requested_export: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            embedded_secret: false,
            symlink_escape: false,
        }
    }

    fn request(nodes: Vec<ReleaseDependencyNode>) -> ReleaseDependencyLeakageRequest {
        ReleaseDependencyLeakageRequest {
            candidate_manifest_digest: hash("manifest"),
            root_order: vec!["root".into()],
            nodes,
            allowed_export_prefixes: vec!["/release/".into()],
            max_depth: 8,
        }
    }

    #[test]
    fn clean_transitive_graph_is_clear() {
        let output = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["upstream"]),
            node("upstream", vec![]),
        ]))
        .unwrap();
        assert_eq!(output.disposition, LeakageAuditDisposition::Clear);
        assert!(output.findings.is_empty());
        assert!(output.validate().is_ok());
    }

    #[test]
    fn protected_secret_and_path_fixtures_block_export() {
        let mut root = node("root", vec!["escape", "protected", "secret"]);
        root.path = "/release/root.json".into();
        let mut secret = node("secret", vec![]);
        secret.embedded_secret = true;
        let mut protected = node("protected", vec![]);
        protected.contains_human_data = true;
        let mut escape = node("escape", vec![]);
        escape.symlink_escape = true;
        let output = audit_glioma_release_dependency_leakage(&request(vec![
            root, secret, protected, escape,
        ]))
        .unwrap();
        assert_eq!(output.disposition, LeakageAuditDisposition::Blocked);
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == LeakageKind::EmbeddedSecret));
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == LeakageKind::ProtectedPayload));
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == LeakageKind::PathEscape));
    }

    #[test]
    fn local_only_transitive_dependency_is_omitted_or_blocks_when_exported() {
        let mut local = node("local", vec![]);
        local.export_scope = DependencyExportScope::LocalOnly;
        local.requested_export = false;
        let output = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["local"]),
            local,
        ]))
        .unwrap();
        assert!(output.omitted_local_order.contains(&"local".into()));
        assert!(!output.export_blocked);
        let mut exported = node("local", vec![]);
        exported.export_scope = DependencyExportScope::LocalOnly;
        let blocked = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["local"]),
            exported,
        ]))
        .unwrap();
        assert!(blocked.export_blocked);
    }

    #[test]
    fn missing_and_cyclic_dependencies_are_hard_failures() {
        let output = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["missing"]),
            node("cycle", vec!["root"]),
        ]))
        .unwrap();
        assert!(output
            .findings
            .iter()
            .any(|finding| finding.kind == LeakageKind::MissingDependency));
        let cyclic = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["cycle"]),
            node("cycle", vec!["root"]),
        ]))
        .unwrap();
        assert!(cyclic
            .findings
            .iter()
            .any(|finding| finding.kind == LeakageKind::DependencyCycle));
    }

    #[test]
    fn non_exported_local_node_never_serializes_a_payload() {
        let mut local = node("local", vec![]);
        local.export_scope = DependencyExportScope::LocalOnly;
        local.requested_export = false;
        let output = audit_glioma_release_dependency_leakage(&request(vec![
            node("root", vec!["local"]),
            local,
        ]))
        .unwrap();
        assert!(output.omitted_local_order.contains(&"local".into()));
        assert!(output
            .findings
            .iter()
            .all(|finding| !finding.reason.contains("payload")));
    }
}
