"""Obligation P32 local single-study contract-model closure-gate feature F02."""
from __future__ import annotations
from typing import Any,Mapping
from .obligation_closure_gate_support import certify,manifest
FEATURE_ID="AFA-obligation-P32-F02";CONTRACT_VERSION="obligation-local-closure-gate-contract-model/1.0"
def obligation_local_closure_gate_contract_model_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
def certify_obligation_local_closure_gate_contract_model(request:Mapping[str,Any])->dict[str,Any]:return certify(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
