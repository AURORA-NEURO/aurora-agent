"""PRISM P32 local single-study research-copilot evaluation-integrity feature F03."""
from __future__ import annotations
from typing import Any,Mapping
from .prism_evaluation_integrity_support import evaluate,manifest
FEATURE_ID="AFA-prism-P32-F03";CONTRACT_VERSION="prism-local-evaluation-integrity-research-copilot/1.0"
def prism_local_evaluation_integrity_research_copilot_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")
def evaluate_prism_local_evaluation_integrity_research_copilot(request:Mapping[str,Any])->dict[str,Any]:return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research-copilot")
