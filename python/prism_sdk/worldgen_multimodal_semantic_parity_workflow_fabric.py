"""Worldgen P28 multimodal semantic parity workflow fabric surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F14"; CONTRACT_VERSION="worldgen-multimodal-semantic-parity-workflow_fabric/1.0"
def worldgen_multimodal_semantic_parity_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")
def compare_worldgen_multimodal_semantic_parity_workflow(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="workflow fabric")

