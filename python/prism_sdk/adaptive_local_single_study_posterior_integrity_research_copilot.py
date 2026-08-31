"""Adaptive P32 local single-study research-copilot posterior-integrity feature F03."""
from __future__ import annotations
from typing import Any, Mapping
from .adaptive_posterior_integrity_support import qualify, manifest
FEATURE_ID = "AFA-adaptive-P32-F03"
CONTRACT_VERSION = "adaptive-local-posterior-integrity-research_copilot/1.0"
def adaptive_local_posterior_integrity_research_copilot_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
def qualify_adaptive_local_posterior_integrity_research_copilot(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
