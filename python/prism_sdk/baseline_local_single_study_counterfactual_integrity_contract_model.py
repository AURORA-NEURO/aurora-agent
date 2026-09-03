"""Baseline P32 local single-study contract-model counterfactual-integrity feature F02."""
from __future__ import annotations
from typing import Any, Mapping
from .baseline_counterfactual_integrity_support import qualify, manifest
FEATURE_ID = "AFA-baseline-P32-F02"
CONTRACT_VERSION = "baseline-local-counterfactual-integrity-contract_model/1.0"
def baseline_local_counterfactual_integrity_contract_model_manifest() -> dict[str, Any]:
    return manifest(feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="contract-model")
def qualify_baseline_local_counterfactual_integrity_contract_model(request: Mapping[str, Any]) -> dict[str, Any]:
    return qualify(request, feature_id=FEATURE_ID, contract_version=CONTRACT_VERSION, scale="local single-study", mode="contract-model")
