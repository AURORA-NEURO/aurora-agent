"""Worldgen P29 throughput scale frontier workflow fabric surface."""
from .worldgen_scale_frontier_support import *
FEATURE_ID="AFA-worldgen-P29-F15"; CONTRACT_VERSION="worldgen-throughput-scale-frontier-workflow_fabric/1.0"
def worldgen_throughput_scale_frontier_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")
def evaluate_worldgen_throughput_scale_frontier_workflow(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")

