"""Tokens P32 local single-study workflow-fabric compression-integrity feature F04."""
from __future__ import annotations
from typing import Any, Mapping
from .tokens_compression_integrity_support import qualify, manifest
FEATURE_ID = "AFA-tokens-P32-F04"
CONTRACT_VERSION = "tokens-local-compression-integrity-workflow_fabric/1.0"
def tokens_local_compression_integrity_workflow_fabric_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="workflow-fabric")
def qualify_tokens_local_compression_integrity_workflow_fabric(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="workflow-fabric")
