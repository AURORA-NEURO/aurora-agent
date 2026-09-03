"""Policy P32 local single-study workflow-fabric grant-integrity feature F04."""
from __future__ import annotations
from typing import Any, Mapping
from .policy_grant_integrity_support import qualify, manifest
FEATURE_ID = "AFA-policy-P32-F04"
CONTRACT_VERSION = "policy-local-grant-integrity-workflow_fabric/1.0"
def policy_local_grant_integrity_workflow_fabric_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="workflow-fabric")
def qualify_policy_local_grant_integrity_workflow_fabric(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="workflow-fabric")
