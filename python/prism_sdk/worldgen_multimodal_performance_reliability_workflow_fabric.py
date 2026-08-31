"""Worldgen P21 multimodal performance/reliability workflow-fabric surface."""
from .worldgen_performance_reliability_workflow_support import *
FEATURE_ID="AFA-worldgen-P21-F14"; CONTRACT_VERSION="worldgen-multimodal-performance-reliability-workflow/1.0"
def worldgen_multimodal_performance_reliability_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def schedule_worldgen_multimodal_performance_reliability_workflow(request): return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
