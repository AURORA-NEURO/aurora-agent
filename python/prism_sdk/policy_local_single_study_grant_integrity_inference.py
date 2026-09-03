"""Policy P32 local single-study inference grant-integrity feature F01."""
from __future__ import annotations
from typing import Any, Mapping
from .policy_grant_integrity_support import qualify, manifest
FEATURE_ID = "AFA-policy-P32-F01"
CONTRACT_VERSION = "policy-local-grant-integrity-inference/1.0"
def policy_local_grant_integrity_inference_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="inference")
def qualify_policy_local_grant_integrity_inference(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="inference")
