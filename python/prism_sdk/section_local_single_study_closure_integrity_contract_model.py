"""Section P32 local single-study contract-model closure-integrity feature F02."""
from __future__ import annotations
from typing import Any,Mapping
from .section_closure_integrity_support import compile_closure,manifest
FEATURE_ID="AFA-section-P32-F02";CONTRACT_VERSION="section-local-closure-integrity-contract-model/1.0"
def compile_section_local_closure_integrity_contract_model_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
def compile_section_local_closure_integrity_contract_model(request:Mapping[str,Any])->dict[str,Any]:return compile_closure(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract-model")
