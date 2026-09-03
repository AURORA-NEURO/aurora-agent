"""Worldgen P28 local semantic parity workflow fabric surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F13"; CONTRACT_VERSION="worldgen-local-semantic-parity-workflow_fabric/1.0"
def worldgen_local_semantic_parity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")
def compare_worldgen_local_semantic_parity_workflow(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="workflow fabric")

