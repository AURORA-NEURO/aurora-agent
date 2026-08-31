"""Worldgen P21 throughput performance/reliability workflow-fabric surface."""
from .worldgen_performance_reliability_workflow_support import *
FEATURE_ID="AFA-worldgen-P21-F15"; CONTRACT_VERSION="worldgen-throughput-performance-reliability-workflow/1.0"
def worldgen_throughput_performance_reliability_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
def schedule_worldgen_throughput_performance_reliability_workflow(request): return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput")
