"""Policy P32 local single-study contract-model grant-integrity feature F02."""
from __future__ import annotations
from typing import Any, Mapping
from .policy_grant_integrity_support import qualify, manifest
FEATURE_ID = "AFA-policy-P32-F02"
CONTRACT_VERSION = "policy-local-grant-integrity-contract_model/1.0"
def policy_local_grant_integrity_contract_model_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="contract-model")
def qualify_policy_local_grant_integrity_contract_model(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="contract-model")
