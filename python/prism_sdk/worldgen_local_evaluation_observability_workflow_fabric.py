"""Worldgen P23 local evaluation/observability workflow fabric surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F13"; CONTRACT_VERSION="worldgen-local-evaluation-observability-workflow/1.0"
def worldgen_local_evaluation_observability_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def schedule_worldgen_local_evaluation_observability_workflow_fabric(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

