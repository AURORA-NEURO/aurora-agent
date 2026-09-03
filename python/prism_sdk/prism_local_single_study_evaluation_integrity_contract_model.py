"""PRISM P32 local single-study contract-model evaluation-integrity feature F02."""
from __future__ import annotations
from typing import Any,Mapping
from .prism_evaluation_integrity_support import evaluate,manifest
FEATURE_ID="AFA-prism-P32-F02";CONTRACT_VERSION="prism-local-evaluation-integrity-contract-model/1.0"
def prism_local_evaluation_integrity_contract_model_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
def evaluate_prism_local_evaluation_integrity_contract_model(request:Mapping[str,Any])->dict[str,Any]:return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
