"""Worldgen P25 throughput contract-frontier workflow fabric surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F15"; CONTRACT_VERSION="worldgen-throughput-contract-frontier-workflow_fabric/1.0"
def worldgen_throughput_contract_frontier_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")
def admit_worldgen_throughput_contract_frontier_workflow(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",mode="workflow fabric")

