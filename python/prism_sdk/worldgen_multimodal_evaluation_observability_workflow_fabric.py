"""Worldgen P23 multimodal evaluation/observability workflow fabric surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F14"; CONTRACT_VERSION="worldgen-multimodal-evaluation-observability-workflow/1.0"
def worldgen_multimodal_evaluation_observability_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")
def schedule_worldgen_multimodal_evaluation_observability_workflow_fabric(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")

