"""PRISM P32 local single-study workflow-fabric evaluation-integrity feature F04."""
from __future__ import annotations
from typing import Any,Mapping
from .prism_evaluation_integrity_support import evaluate,manifest
FEATURE_ID="AFA-prism-P32-F04";CONTRACT_VERSION="prism-local-evaluation-integrity-workflow-fabric/1.0"
def prism_local_evaluation_integrity_workflow_fabric_manifest()->dict[str,Any]:return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow-fabric")
def evaluate_prism_local_evaluation_integrity_workflow_fabric(request:Mapping[str,Any])->dict[str,Any]:return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow-fabric")
