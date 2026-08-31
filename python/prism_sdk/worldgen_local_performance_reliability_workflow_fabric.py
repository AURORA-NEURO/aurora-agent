"""Worldgen P21 local performance/reliability workflow-fabric surface."""
from .worldgen_performance_reliability_workflow_support import *
FEATURE_ID="AFA-worldgen-P21-F13"; CONTRACT_VERSION="worldgen-local-performance-reliability-workflow/1.0"
def worldgen_local_performance_reliability_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def schedule_worldgen_local_performance_reliability_workflow(request): return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
