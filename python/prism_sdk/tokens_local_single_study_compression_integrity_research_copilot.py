"""Tokens P32 local single-study research-copilot compression-integrity feature F03."""
from __future__ import annotations
from typing import Any, Mapping
from .tokens_compression_integrity_support import qualify, manifest
FEATURE_ID = "AFA-tokens-P32-F03"
CONTRACT_VERSION = "tokens-local-compression-integrity-research_copilot/1.0"
def tokens_local_compression_integrity_research_copilot_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
def qualify_tokens_local_compression_integrity_research_copilot(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="research-copilot")
