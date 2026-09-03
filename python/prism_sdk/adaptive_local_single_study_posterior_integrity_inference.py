"""Adaptive P32 local single-study inference posterior-integrity feature F01."""
from __future__ import annotations
from typing import Any, Mapping
from .adaptive_posterior_integrity_support import qualify, manifest
FEATURE_ID = "AFA-adaptive-P32-F01"
CONTRACT_VERSION = "adaptive-local-posterior-integrity-inference/1.0"
def adaptive_local_posterior_integrity_inference_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="inference")
def qualify_adaptive_local_posterior_integrity_inference(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="inference")
