"""Worldgen P25 local contract-frontier workflow fabric surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F13"; CONTRACT_VERSION="worldgen-local-contract-frontier-workflow_fabric/1.0"
def worldgen_local_contract_frontier_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def admit_worldgen_local_contract_frontier_workflow(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

