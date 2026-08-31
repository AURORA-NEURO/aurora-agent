"""Policy P32 local single-study research-copilot grant-integrity feature F03."""
from __future__ import annotations
from typing import Any, Mapping
from .policy_grant_integrity_support import qualify, manifest
FEATURE_ID = "AFA-policy-P32-F03"
CONTRACT_VERSION = "policy-local-grant-integrity-research_copilot/1.0"
def policy_local_grant_integrity_research_copilot_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
def qualify_policy_local_grant_integrity_research_copilot(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
